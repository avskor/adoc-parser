//! The `replacements` substitution pass (typographic replacements).
//!
//! Asciidoctor runs `replacements` after `quotes`/`attributes`, over the whole
//! post-quotes string. We reuse the legacy engine's
//! [`crate::inline::apply_typographic_replacements`] over the working buffer.
//!
//! The sentinel control bytes that mark formatting spans
//! ([`TAG_LEAD`](super::tokenize::TAG_LEAD)/[`TAG_TAIL`](super::tokenize::TAG_TAIL))
//! act exactly like the `<`/`>` of Asciidoctor's spliced `<tag>`s: they are
//! neither word nor space characters, so an edge-anchored replacement (the
//! spaced em-dash `--`) treats a span boundary as a non-edge, while the true
//! buffer ends are real line edges:
//!
//! - `*--*` → `<strong>--</strong>` — the `--` is flanked by sentinel bytes, not
//!   spaces, so no spaced em-dash forms (mirrors legacy's inner reparse, where
//!   span content is parsed with `edges_are_line_boundaries` cleared).
//! - top-level `-- x` → spaced em-dash — the `--` sits at the real buffer start.
//! - `*don't*` → `<strong>don't</strong>` — the apostrophe is flanked by
//!   alphanumerics regardless of the span.
//!
//! ## Attribute-reference sentinels are run boundaries
//!
//! There is one sentinel kind that is NOT a `<tag>`: an attribute reference
//! ([`TagToken::AttrRef`]) or inline assignment ([`TagToken::AttrSet`]). The
//! legacy parser emits each of those as its own event, which *splits* the text
//! run — and the split edge counts as a line boundary for the spaced em-dash
//! (`left_is_boundary = start != 0`, `right_is_boundary = end < len`). So
//! `{empty}--{empty}` becomes a spaced em-dash in legacy (the `--` run sits
//! between two events, both edges boundaries), whereas a single whole-buffer
//! pass would see the `--` flanked by sentinel bytes and leave it literal.
//!
//! To mirror legacy exactly we split the buffer at each AttrRef/AttrSet sentinel
//! and apply the replacements to every segment independently — each segment's
//! edges that abut such a split become real edges (the function's
//! `left_is_boundary`/`right_is_boundary` flags), while the sentinels stay
//! verbatim between the segments. Quote/passthrough/macro sentinels remain
//! *inside* a segment, so they keep their `<tag>`-like non-boundary treatment.
//! A buffer with no AttrRef/AttrSet sentinel is a single segment — byte-for-byte
//! the previous whole-buffer behaviour.
//!
//! A sentinel never collides with a replacement: its body is ASCII digits framed
//! by control bytes (none of the trigger characters `- . ( ' \``), and no
//! replacement output contains a control byte, so sentinels pass through intact.
//!
//! Character-reference survival (`&#167;`) is NOT done here — it is handled by
//! the earlier [`super::char_refs`] pass, which lifts each valid reference into a
//! sentinel before this pass runs, so a reference never reaches replacements as
//! live text.

use std::borrow::Cow;

use super::tokenize::{sentinel_end, TagToken, Work, TAG_LEAD};

/// Apply typographic replacements over the working buffer.
///
/// The buffer is split at every attribute-reference / attribute-set sentinel
/// (see the module docs) and each segment is processed independently with both
/// edges treated as real line boundaries — the same `(true, true)` the engine
/// uses for the top-level buffer. Without such a sentinel the whole buffer is a
/// single segment, identical to the previous whole-buffer pass.
pub(super) fn run(work: &mut Work) {
    // Fast path: no AttrRef/AttrSet sentinel → one segment over the whole buffer.
    if !has_boundary_sentinel(work) {
        if let Cow::Owned(s) = crate::inline::apply_typographic_replacements(&work.buf, true, true) {
            work.buf = s;
        }
        return;
    }

    let bytes = work.buf.as_bytes();
    let mut out = String::with_capacity(work.buf.len());
    let mut seg_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            let end = sentinel_end(bytes, i);
            if is_boundary_sentinel(work, bytes, i, end) {
                // Flush the segment before the split, then keep the sentinel verbatim.
                out.push_str(&apply_segment(&work.buf[seg_start..i]));
                out.push_str(&work.buf[i..end]);
                seg_start = end;
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out.push_str(&apply_segment(&work.buf[seg_start..]));
    work.buf = out;
}

/// Apply the legacy replacement function to one segment with both edges treated
/// as real line boundaries (the engine's top-level convention).
fn apply_segment(segment: &str) -> Cow<'_, str> {
    crate::inline::apply_typographic_replacements(segment, true, true)
}

/// Whether the buffer contains any AttrRef/AttrSet sentinel (the only kinds that
/// split a text run in the legacy parser).
fn has_boundary_sentinel(work: &Work) -> bool {
    let bytes = work.buf.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            let end = sentinel_end(bytes, i);
            if is_boundary_sentinel(work, bytes, i, end) {
                return true;
            }
            i = end;
        } else {
            i += 1;
        }
    }
    false
}

/// Whether the sentinel spanning `bytes[start..end]` (with `bytes[start] ==
/// TAG_LEAD`) indexes an AttrRef/AttrSet token.
fn is_boundary_sentinel(work: &Work, bytes: &[u8], start: usize, end: usize) -> bool {
    // Parse the decimal index between TAG_LEAD and TAG_TAIL.
    let mut idx = 0usize;
    let mut j = start + 1;
    while j < end && bytes[j].is_ascii_digit() {
        idx = idx * 10 + (bytes[j] - b'0') as usize;
        j += 1;
    }
    matches!(
        work.tags.get(idx),
        Some(TagToken::AttrRef { .. }) | Some(TagToken::AttrSet { .. })
    )
}
