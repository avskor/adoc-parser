# adoc-parser

AsciiDoc parser and HTML converter written in Rust.

Two-phase pull parser (like pulldown-cmark) with `Start`/`End` event pairs:
block-level scanning followed by inline parsing.

## Project structure

| Crate | Description |
|---|---|
| **adoc-parser** | Core pull parser, zero external dependencies |
| **adoc-html** | HTML renderer consuming parser events |
| **adoc-wasm** | Thin wasm-bindgen wrapper exposing `to_html` |
| **adoc-cli** | CLI tool (`adoc`) for converting files |
| **adoc-compat-tests** | Compatibility test suite (233/233 passing) |

## Build

```bash
cargo build --workspace
```

WASM build:

```bash
cargo build -p adoc-wasm --target wasm32-unknown-unknown
```

## CLI usage

Convert file to stdout:

```bash
cargo run -p adoc-cli -- document.adoc
```

Convert file to file:

```bash
cargo run -p adoc-cli -- document.adoc -o output.html
```

Read from stdin:

```bash
cat document.adoc | cargo run -p adoc-cli
```

## Testing

```bash
cargo test --workspace           # Run all tests
cargo clippy --workspace         # Lint
```

Run tests for a specific crate:

```bash
cargo test -p adoc-parser        # Core parser
cargo test -p adoc-html          # HTML backend
cargo test -p adoc-compat-tests  # Compatibility tests
```
