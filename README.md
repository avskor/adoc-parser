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
| **adoc-compat-tests** | Structural conformance vs `asciidoc-parsing-lab` ASG fixtures (233/233 passing) |
| **adoc-html-tests** | HTML-output compatibility vs Asciidoctor (semantic DOM comparison) |

The `adoc-compat-tests` figure measures *structural* conformance — whether the parser produces
the expected event/AST structure for the `asciidoc-parsing-lab` reference fixtures. Byte-level
HTML-output compatibility with Asciidoctor is a separate, ongoing concern, exercised by
`adoc-html-tests`.

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
cargo test -p adoc-compat-tests  # ASG structural conformance
cargo test -p adoc-html-tests    # HTML-output compatibility vs Asciidoctor
```
