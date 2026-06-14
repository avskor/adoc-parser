//! The `replacements` substitution pass (typographic replacements).
//!
//! Asciidoctor runs `replacements` after `quotes`/`attributes`, over the whole
//! post-quotes string. We reuse the legacy engine's
//! [`crate::inline::apply_typographic_replacements`] across the entire working
//! buffer in a single pass.
//!
//! The sentinel control bytes that mark formatting spans
//! ([`TAG_LEAD`](super::tokenize::TAG_LEAD)/[`TAG_TAIL`](super::tokenize::TAG_TAIL))
//! act exactly like the `<`/`>` of Asciidoctor's spliced `<tag>`s: they are
//! neither word nor space characters, so an edge-anchored replacement (the
//! spaced em-dash `--`) treats a span boundary as a non-edge, while the true
//! buffer ends are real line edges. Passing `(true, true)` therefore reproduces
//! both Asciidoctor's whole-string behaviour AND the legacy parser's top-level
//! per-run behaviour (where `edges_are_line_boundaries` is set):
//!
//! - `*--*` → `<strong>--</strong>` — the `--` is flanked by sentinel bytes, not
//!   spaces, so no spaced em-dash forms (mirrors legacy's inner reparse).
//! - top-level `-- x` → spaced em-dash — the `--` sits at the real buffer start.
//! - `*don't*` → `<strong>don't</strong>` — the apostrophe is flanked by
//!   alphanumerics regardless of the span.
//!
//! A sentinel never collides with a replacement: its body is ASCII digits framed
//! by control bytes (none of the trigger characters `- . ( ' \``), and no
//! replacement output contains a control byte, so sentinels pass through intact.
//!
//! Character-reference restoration (`&#167;` survival) is NOT done here — it is
//! tied to passthrough/specialchars handling and lands in a later pass. Inputs
//! containing char references therefore still diverge from legacy and fall back
//! through the differential-equality gate.

use std::borrow::Cow;

use super::tokenize::Work;

/// Apply typographic replacements across the whole working buffer.
pub(super) fn run(work: &mut Work) {
    if let Cow::Owned(s) = crate::inline::apply_typographic_replacements(&work.buf, true, true) {
        work.buf = s;
    }
}
