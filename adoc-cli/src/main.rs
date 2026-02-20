use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(name = "adoc", about = "Convert AsciiDoc to HTML")]
struct Cli {
    /// Input file (reads from stdin if omitted)
    input: Option<PathBuf>,

    /// Output file (writes to stdout if omitted)
    #[arg(short, long)]
    output: Option<PathBuf>,
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
                let canonical = file_path.canonicalize().map_err(|e| {
                    format!("failed to resolve include '{}': {e}", file_path.display())
                })?;

                if seen.contains(&canonical) {
                    return Err(format!(
                        "circular include detected: '{}'",
                        canonical.display()
                    ));
                }

                let file_content = fs::read_to_string(&canonical).map_err(|e| {
                    format!("failed to read include '{}': {e}", canonical.display())
                })?;

                seen.insert(canonical.clone());
                let child_dir = canonical.parent().unwrap_or(base_dir);
                let resolved =
                    resolve_includes(&file_content, child_dir, depth + 1, seen)?;
                seen.remove(&canonical);

                let offset = parse_level_offset(attrs);
                let adjusted = adoc_parser::apply_level_offset(&resolved, offset);
                result.push_str(&adjusted);
                result.push('\n');
                continue;
            }
        }
        result.push_str(line);
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
    let resolved = resolve_includes(&input, base_dir, 0, &mut seen)?;
    let preprocessed = adoc_parser::preprocess(&resolved);

    let html = adoc_html::to_html(&preprocessed);

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
