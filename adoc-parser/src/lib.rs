mod attributes;
mod block;
mod event;
pub mod inline;
mod parser;
pub mod preprocessor;
mod scanner;
mod subst;

pub use event::*;
pub use inline::{InlineOptions, InlineParser};
pub use parser::Parser;
pub use scanner::icon_default_alt;
pub use preprocessor::{apply_level_offset, preprocess, preprocess_with_attrs, resolve_includes, resolve_includes_with_source};

/// Byte length of a syntactically valid HTML character reference beginning at
/// `bytes[start]` (the `&`), or `0` if none starts there. Recognises decimal
/// (`&#NNN;`), hex (`&#xHHH;`), and named (`&name;`) forms — a syntactic mirror
/// of Asciidoctor's `CharRefRx`, not a lookup against a fixed entity table.
///
/// Exposed for renderers: when escaping a value where Asciidoctor preserves
/// already-formed entities (a link `href`, an image `alt`), a `&` that begins a
/// valid reference must be copied verbatim rather than re-escaped to `&amp;`.
pub fn char_ref_len(bytes: &[u8], start: usize) -> usize {
    subst::char_ref_len(bytes, start)
}
