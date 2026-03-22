use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::{DateTime, Local};
use clap::Parser;

#[derive(Parser)]
#[command(name = "adoc", about = "Convert AsciiDoc to HTML")]
struct Cli {
    /// Input file (reads from stdin if omitted)
    input: Option<PathBuf>,

    /// Output file (writes to stdout if omitted)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Generate an HTML fragment instead of a full standalone document
    #[arg(long)]
    no_standalone: bool,

    /// Set a document attribute (e.g. -a nofooter -a icons=font)
    #[arg(short = 'a', long = "attribute", value_name = "NAME[=VALUE]")]
    attributes: Vec<String>,
}

const MAX_INCLUDE_DEPTH: usize = 10;

fn parse_level_offset(attrs: &str) -> i8 {
    for part in attrs.split(',') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("leveloffset=") {
            let value = value.trim();
            if let Ok(n) = value.parse::<i8>() {
                return n;
            }
        }
    }
    0
}

fn resolve_includes(
    content: &str,
    base_dir: &Path,
    source_file: &str,
    depth: usize,
    seen: &mut HashSet<PathBuf>,
) -> Result<String, String> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err(format!("include depth exceeds maximum of {MAX_INCLUDE_DEPTH}"));
    }

    let mut result = String::with_capacity(content.len());
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("include::")
            && let Some(bracket_start) = rest.find('[')
            && let Some(bracket_end) = rest.rfind(']')
            && bracket_end > bracket_start
        {
            let path_str = &rest[..bracket_start];
            let attrs = &rest[bracket_start + 1..bracket_end];

            if !path_str.is_empty() {
                let file_path = base_dir.join(path_str);
                match fs::read_to_string(&file_path) {
                    Ok(file_content) => {
                        let canonical = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
                        if seen.contains(&canonical) {
                            result.push_str(&format!("Unresolved directive in {source_file} - include::{path_str}[{attrs}]\n"));
                            continue;
                        }
                        seen.insert(canonical.clone());
                        let child_dir = canonical.parent().unwrap_or(base_dir);
                        let child_name = canonical.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| source_file.to_string());
                        let resolved =
                            resolve_includes(&file_content, child_dir, &child_name, depth + 1, seen)?;
                        seen.remove(&canonical);

                        let offset = parse_level_offset(attrs);
                        let adjusted = adoc_parser::apply_level_offset(&resolved, offset);
                        result.push_str(&adjusted);
                        result.push('\n');
                    }
                    Err(_) => {
                        result.push_str(&format!("Unresolved directive in {source_file} - include::{path_str}[{attrs}]\n"));
                    }
                }
                continue;
            }
        }
        if line.starts_with("\\include::") {
            // Escaped include directive at start of line — strip the leading backslash
            result.push_str(&line[1..]);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    // Remove trailing newline if original didn't end with one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    Ok(result)
}

fn run(cli: Cli) -> Result<(), String> {
    let input = match &cli.input {
        Some(path) => fs::read_to_string(path)
            .map_err(|e| format!("failed to read '{}': {e}", path.display()))?,
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("failed to read stdin: {e}"))?;
            buf
        }
    };

    let base_dir = cli
        .input
        .as_ref()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    let mut seen = HashSet::new();
    if let Some(ref path) = cli.input
        && let Ok(canonical) = path.canonicalize()
    {
        seen.insert(canonical);
    }
    let source_name = cli.input.as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<stdin>".to_string());
    let resolved = resolve_includes(&input, base_dir, &source_name, 0, &mut seen)?;

    let mut initial_attrs: HashMap<String, Option<String>> = HashMap::new();
    let mut locked_attrs: HashSet<String> = HashSet::new();
    let mut html_attrs: HashMap<String, String> = HashMap::new();

    for attr_str in &cli.attributes {
        if let Some(name) = attr_str.strip_suffix('!') {
            initial_attrs.insert(name.to_string(), None);
            locked_attrs.insert(name.to_string());
        } else if let Some(name) = attr_str.strip_prefix('!') {
            initial_attrs.insert(name.to_string(), None);
            locked_attrs.insert(name.to_string());
        } else if let Some((name, value)) = attr_str.split_once('=') {
            initial_attrs.insert(name.to_string(), Some(value.to_string()));
            locked_attrs.insert(name.to_string());
            html_attrs.insert(name.to_string(), value.to_string());
        } else {
            initial_attrs.insert(attr_str.to_string(), Some(String::new()));
            locked_attrs.insert(attr_str.to_string());
            html_attrs.insert(attr_str.to_string(), String::new());
        }
    }

    let preprocessed = adoc_parser::preprocess_with_attrs(&resolved, &initial_attrs, &locked_attrs);

    let last_updated = cli.input.as_ref().and_then(|p| {
        let meta = fs::metadata(p).ok()?;
        let mtime = meta.modified().ok()?;
        let dt: DateTime<Local> = mtime.into();
        Some(dt.format("%Y-%m-%d %H:%M:%S %z").to_string())
    });

    let html = if cli.no_standalone {
        adoc_html::to_html_with_options(&preprocessed, adoc_html::HtmlOptions {
            attributes: html_attrs,
            ..Default::default()
        })
    } else {
        adoc_html::to_html_with_options(&preprocessed, adoc_html::HtmlOptions {
            standalone: true,
            last_updated,
            attributes: html_attrs,
            ..Default::default()
        })
    };

    match &cli.output {
        Some(path) => fs::write(path, &html)
            .map_err(|e| format!("failed to write '{}': {e}", path.display()))?,
        None => io::stdout()
            .write_all(html.as_bytes())
            .map_err(|e| format!("failed to write stdout: {e}"))?,
    }

    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("adoc: {msg}");
            ExitCode::FAILURE
        }
    }
}
