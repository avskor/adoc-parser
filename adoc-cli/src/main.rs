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
    let source_name = cli
        .input
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    let resolved =
        adoc_parser::resolve_includes_with_source(&input, base_dir, source_name.as_deref());

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

    // Intrinsic document attributes derived from the input: docname/docfile/
    // docdir/docfilesuffix from the path, docdate/doctime/docdatetime from its
    // mtime (now when reading stdin; docdir falls back to the cwd). Explicit
    // -a values win; header attribute entries override the date family like
    // Asciidoctor (docname/docfile/docdir are locked there — known limit).
    let cli_attr_names: HashSet<&str> = cli
        .attributes
        .iter()
        .map(|s| {
            let s = s.strip_prefix('!').unwrap_or(s);
            let s = s.split_once('=').map_or(s.as_ref(), |(n, _)| n);
            s.strip_suffix('!').unwrap_or(s)
        })
        .collect();
    let mut seed = |name: &str, value: String| {
        if cli_attr_names.contains(name) {
            return;
        }
        initial_attrs.insert(name.to_string(), Some(value.clone()));
        html_attrs.insert(name.to_string(), value);
    };
    let input_mtime: DateTime<Local> = cli
        .input
        .as_ref()
        .and_then(|p| fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .map_or_else(Local::now, Into::into);
    let docdate = input_mtime.format("%Y-%m-%d").to_string();
    let doctime = input_mtime.format("%H:%M:%S %z").to_string();
    seed("docdatetime", format!("{docdate} {doctime}"));
    seed("docdate", docdate);
    seed("doctime", doctime);
    let now = Local::now();
    let localdate = now.format("%Y-%m-%d").to_string();
    let localtime = now.format("%H:%M:%S %z").to_string();
    seed("localdatetime", format!("{localdate} {localtime}"));
    seed("localdate", localdate);
    seed("localtime", localtime);
    if let Some(path) = &cli.input {
        let abs = path.canonicalize().unwrap_or_else(|_| path.clone());
        if let Some(stem) = abs.file_stem() {
            seed("docname", stem.to_string_lossy().into_owned());
        }
        if let Some(ext) = abs.extension() {
            seed("docfilesuffix", format!(".{}", ext.to_string_lossy()));
        }
        if let Some(dir) = abs.parent() {
            seed("docdir", dir.to_string_lossy().into_owned());
        }
        seed("docfile", abs.to_string_lossy().into_owned());
    } else if let Ok(cwd) = std::env::current_dir() {
        seed("docdir", cwd.to_string_lossy().into_owned());
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
