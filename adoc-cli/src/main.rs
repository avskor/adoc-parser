use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
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

    let html = adoc_html::to_html(&input);

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
