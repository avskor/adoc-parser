//! The character-reference survival pass.
//!
//! Asciidoctor's `specialcharacters` substitution escapes every `&` to `&amp;`,
//! and the later `replacements` substitution then *restores* a valid HTML
//! character reference (`&#167;` / `&copy;` / `&#x2026;`) back to a bare `&…;`.
//! The net effect is that a valid reference survives intact while an invalid one
//! (`&#1;`, a bare `&`, an unterminated `&foo`) stays escaped. The legacy
//! recursive parser collapses that two-step dance into a single check
//! ([`crate::inline::InlineState::char_ref_len_at`]) that emits a survived
//! reference as an `InlinePassthrough` (so the renderer leaves the `&` alone)
//! and lets everything else fall through to a `Text` event (which the renderer
//! escapes). This pass mirrors that: it extracts each valid reference into an
//! opaque [`TagToken::CharRef`](super::tokenize::TagToken::CharRef) leaf
//! (`raw = true` → `InlinePassthrough`), leaving invalid `&`s as literal text.
//!
//! ## Ordering
//!
//! Runs after [`super::escape`] (so an escaped `\&#…;` is already sealed in its
//! own leaf and is not re-extracted here) and BEFORE the attribute/quote passes.
//! Extracting first is what stops the `#` inside a decimal/hex reference
//! (`&#167;`) from being taken for a `mark`/highlight marker by the quote pass —
//! the legacy parser likewise consumes the whole reference atomically before any
//! marker at that position is considered. The leaf's sentinel bytes are non-word
//! boundary characters, so they are transparent to the later boundary logic.
//!
//! Gated (in [`super::run_pipeline`]) on both `specialcharacters` AND
//! `replacements` being active — the exact `preserve_char_refs` condition of the
//! legacy parser; a verbatim block (specialchars but no replacements) keeps its
//! references escaped, so the pass is skipped there.

use super::tokenize::{sentinel_end, utf8_char_len, Work, TAG_LEAD};

/// Extract every valid character reference into an opaque survival leaf.
pub(super) fn run(work: &mut Work) {
    let old = std::mem::take(&mut work.buf);
    let bytes = old.as_bytes();
    let mut out = String::with_capacity(old.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            let end = sentinel_end(bytes, i);
            out.push_str(&old[i..end]);
            i = end;
            continue;
        }

        if bytes[i] == b'&' {
            let len = char_ref_len(bytes, i);
            if len > 0 {
                // References are ASCII-only, so `i..i + len` is a char boundary.
                out.push_str(&work.char_ref_sentinel(old[i..i + len].to_string(), true));
                i += len;
                continue;
            }
        }

        let len = utf8_char_len(bytes[i]);
        out.push_str(&old[i..i + len]);
        i += len;
    }

    work.buf = out;
}

/// If a valid HTML character reference begins at byte `start` (where
/// `bytes[start]` is `&`), returns its total byte length (including the leading
/// `&` and trailing `;`); otherwise 0. Port of
/// [`crate::inline::InlineState::char_ref_len_at`] / Asciidoctor's `CharRefRx`:
/// named `[A-Za-z][A-Za-z]+\d{0,2}`, decimal `#\d{2,6}`, or hex
/// `#x[0-9A-Fa-f]{2,}`, each terminated by `;`. ASCII-only, so byte indexing is
/// safe. Shared with [`super::escape`] for the `\&#…;` escape.
pub(super) fn char_ref_len(bytes: &[u8], start: usize) -> usize {
    if bytes.get(start) != Some(&b'&') {
        return 0;
    }
    let mut i = start + 1;
    if bytes.get(i) == Some(&b'#') {
        i += 1;
        if matches!(bytes.get(i), Some(b'x' | b'X')) {
            // hex: at least 2 hex digits
            i += 1;
            let hex_start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_hexdigit) {
                i += 1;
            }
            if i - hex_start < 2 {
                return 0;
            }
        } else {
            // decimal: 2..=6 digits
            let dec_start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if !(2..=6).contains(&(i - dec_start)) {
                return 0;
            }
        }
    } else {
        // named: a letter, then at least one more letter, then 0..=2 trailing digits
        let name_start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_alphabetic) {
            i += 1;
        }
        if i - name_start < 2 {
            return 0;
        }
        let mut digits = 0;
        while digits < 2 && bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            digits += 1;
        }
    }
    if bytes.get(i) == Some(&b';') { i + 1 - start } else { 0 }
}
