# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Tools

Use context7 MCP server to fetch up-to-date documentation for libraries and frameworks before answering questions or writing code that depends on them.

When working with Rust code, use rust-analyzer LSP for code navigation:
go-to-definition, find-references, diagnostics. Do not use text search
to find function and type definitions — use LSP instead.

## Git Workflow

CRITICAL: Never make commits directly on `master`. Before starting ANY task, ALWAYS:
1. `git checkout master && git pull`
2. `git checkout -b <branch-name>` (e.g., `feat/add-auth`, `fix/parse-error`)

Only then begin writing code. Before every commit run `cargo clippy --workspace` (fix all warnings), `cargo test -p adoc-compat-tests -- --nocapture` (all 233 compatibility tests against `vendor/asciidoc-parsing-lab` must pass), and `cargo test -p adoc-html-tests -- --nocapture` (HTML compatibility tests vs Asciidoctor).

## Build & Test Commands

```bash
cargo build --workspace          # Build everything
cargo test --workspace           # Run all tests
cargo test -p adoc-parser        # Test core parser only
cargo test -p adoc-html          # Test HTML backend only
cargo test -p adoc-parser -- scanner::tests::test_is_delimiter   # Run single test
cargo test -p adoc-compat-tests -- --nocapture  # Compatibility tests (233 cases from asciidoc-parsing-lab)
cargo test -p adoc-html-tests -- --nocapture    # HTML compatibility tests vs Asciidoctor
cargo clippy --workspace         # Lint
cargo build -p adoc-wasm --target wasm32-unknown-unknown         # WASM build
```

## Architecture

Rust 2024 edition Cargo workspace with crates:

- **adoc-parser** — core pull parser, zero external dependencies
- **adoc-html** — HTML renderer consuming parser events
- **adoc-wasm** — thin wasm-bindgen wrapper exposing `to_html`
- **adoc-html-tests** — HTML compatibility tests comparing adoc-html output against Asciidoctor reference HTML using semantic DOM comparison (scraper + similar)

### Two-Phase Pull Parser

The parser uses an iterator-based event stream (like pulldown-cmark) with `Start(Tag)`/`End(TagEnd)` pairs:

1. **BlockScanner** (`block.rs`) — line-by-line scanning producing block-level events (sections, paragraphs, lists, delimited blocks). Tracks nesting via `context_stack: Vec<BlockContext>`.

2. **InlineParser** (`inline.rs`) — character-by-character parsing of `Event::Text` content into inline formatting events (bold, italic, links, etc.). Stateless, called by `Parser` on each text event.

3. **Parser** (`parser.rs`) — wraps `BlockScanner`, intercepts `Event::Text(Cow::Borrowed(s))` and feeds it through `InlineParser`, buffering results in `inline_buffer`.

### Event Buffer Pattern

Both `BlockScanner` and `Parser` use `Vec<Event>` as a reversed stack — events are pushed in reverse order and popped for O(1) FIFO delivery. When building event sequences, push content events first (buffer bottom), then `Start` event, then any prefix events (title, close) on top so they're emitted first.

### Key Types

- `CowStr<'a> = Cow<'a, str>` — zero-copy borrowing from input
- `Event<'a>` — Start, End, Text, Code, SoftBreak, HardBreak, ThematicBreak, PageBreak, Attribute, AttributeReference
- `Tag<'a>` — carries data (level, id, url, etc.); `TagEnd` — Copy, no borrowed data
- `BlockContext` — Section, DelimitedBlock, UnorderedList, OrderedList, ListItem
- `scanner.rs` — stateless detection functions (`is_delimiter`, `strip_section_marker`, `is_list_marker_*`, etc.)

### HTML Backend

`adoc-html` converts events to HTML via `HtmlRenderer` with a `tag_stack` for tracking open elements. Public API: `to_html(input) -> String` and `push_html(buf, iter)`.
