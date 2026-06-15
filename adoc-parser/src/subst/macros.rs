//! Inline-macro extraction pass (Asciidoctor `macros` sub).
//!
//! Phase 2 (macros, 1/N) implements the **cross-reference** constructs — the
//! `xref:target[label]` macro and the `<<target>>` / `<<target,label>>` shorthand,
//! both of which the legacy parser turns into a `CrossReference` tag. Every match
//! is lifted out of the working buffer into a tag sentinel pointing at a
//! [`TagToken::Macro`] leaf that holds the macro's `Start`, its label events, and
//! its `End`, so the later attribute/quote/replacement passes cannot reach inside
//! it. (The remaining macro families — link/image/footnote/icon/UI/stem/anchor/
//! autolink/email/index-term — are ported in subsequent phases.)
//!
//! ## Where it runs in the pipeline
//!
//! After passthrough/escape/char-ref extraction and **before** `attributes`. The
//! before-attributes ordering matters: the legacy parser consumes a macro whole,
//! so an attribute reference inside the macro target (`xref:{anchor}[]`) stays a
//! literal part of the target rather than being lifted into its own
//! `AttributeReference`. Running `attributes` first would extract the `{anchor}`
//! into a sentinel and the macro span would then carry it (and be skipped — see
//! below), diverging from legacy. A top-level `{name}` (not inside a macro) is
//! untouched here and picked up by the later `attributes` pass.
//!
//! ## Label re-parsing mirrors `push_macro_label`
//!
//! The legacy parser re-parses an explicit label with `subs.without(MACROS)` via
//! [`crate::inline::InlineState::push_macro_label`]. This pass reproduces that by
//! running the engine's own [`super::run_pipeline`] on the raw label text with
//! `MACROS` cleared (so a nested macro stays literal and the recursion always
//! terminates). An empty label is not re-parsed: an empty `xref:x[]` emits the
//! target as the link text (`Text(target)`, the legacy `None` branch), exactly
//! the single `Text` event the renderer needs to swap in the auto-generated xref
//! text.
//!
//! ## Sentinel-free span guard
//!
//! Because earlier passes already lifted passthroughs/escapes/char-refs into
//! sentinels, a macro whose source span now contains one (`xref:x[+raw+]`, a
//! passthrough inside the label) no longer has the raw text the legacy parser
//! would re-parse. Rather than emit a mismatched leaf, the pass declines such a
//! span (leaving it literal); the differential-equality gate then falls back to
//! legacy. The common case — plain targets and text/quote labels — carries no
//! sentinels and is extracted normally.

use std::borrow::Cow;

use crate::event::{Event, SubstitutionSet, Tag, TagEnd};

use super::tokenize::{sentinel_end, utf8_char_len, Work, TAG_LEAD};

/// Extract every cross-reference macro from `work.buf` into sentinels.
pub(super) fn extract(work: &mut Work, subs: SubstitutionSet) {
    let src = std::mem::take(&mut work.buf);
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;

    while i < bytes.len() {
        // Step over an existing sentinel verbatim — a macro never starts inside
        // an already-extracted passthrough/escape/char-ref leaf.
        if bytes[i] == TAG_LEAD {
            let end = sentinel_end(bytes, i);
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }

        // xref:target[label]
        if bytes[i] == b'x' && src[i..].starts_with("xref:") {
            if let Some((events, end)) = try_xref(&src, i, subs) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            // Not a valid xref → advance past the 'x' (the rest stays literal),
            // mirroring the legacy parser's `pos += 1` on a failed macro match.
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // <<target>> / <<target,label>>
        if bytes[i] == b'<' && bytes.get(i + 1) == Some(&b'<') {
            if let Some((events, end)) = try_cross_ref(&src, i, subs) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            // Not a valid cross reference → advance past one '<'.
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // Copy the whole UTF-8 character verbatim (a single byte would corrupt
        // multibyte text).
        let len = utf8_char_len(bytes[i]);
        out.push_str(&src[i..i + len]);
        i += len;
    }

    work.buf = out;
}

/// At an `xref:` (caller guarantees the prefix), try to match `xref:target[label]`.
/// Returns the macro's event sequence plus the index just past the closing `]`,
/// or `None` to leave the `xref:` literal. Mirror of
/// [`crate::inline::InlineState::try_xref_macro`].
fn try_xref(src: &str, start: usize, subs: SubstitutionSet) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 5..]; // after "xref:"
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.find(']')?;
    if bracket_end <= bracket_start {
        return None;
    }
    let target = &rest[..bracket_start];
    let label_text = &rest[bracket_start + 1..bracket_end];
    if target.is_empty() {
        return None;
    }
    let end = start + 5 + bracket_end + 1;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    // Empty brackets → no explicit label (legacy `None`); a non-empty label is an
    // explicit one.
    let label = (!label_text.is_empty()).then_some(label_text);
    Some((build_cross_reference(target, label, subs), end))
}

/// At a `<<` (caller guarantees the prefix), try to match `<<target>>` /
/// `<<target,label>>`. Mirror of
/// [`crate::inline::InlineState::try_cross_reference`].
fn try_cross_ref(
    src: &str,
    start: usize,
    subs: SubstitutionSet,
) -> Option<(Vec<Event<'static>>, usize)> {
    let after_open = start + 2;
    let rest = &src[after_open..];
    let close = rest.find(">>")?;
    let content = &rest[..close];
    if content.is_empty() {
        return None;
    }
    let end = after_open + close + 2;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    // With a comma: trim both target and label. Without: the whole content is the
    // target (untrimmed), no explicit label.
    let (target, label) = if let Some((t, l)) = content.split_once(',') {
        (t.trim(), Some(l.trim()))
    } else {
        (content, None)
    };
    // A leading '#' is an explicit-anchor marker, not part of the id.
    let target = target.strip_prefix('#').unwrap_or(target);
    Some((build_cross_reference(target, label, subs), end))
}

/// Build the `Start(CrossReference) … End` event sequence. `label` is the raw
/// explicit label text (`None` for the bracket-less / empty-bracket form). The
/// `CrossReference` tag carries `target` and an `is_some()`-significant `label`
/// field (only its presence drives the renderer; its text is compared by the
/// gate). The label *events* are re-parsed with `MACROS` cleared, matching
/// `push_macro_label`; an empty explicit label (`<<a,>>`) yields no label events
/// (as `push_macro_label("")` does), while the no-label form emits the target as
/// the link text.
fn build_cross_reference(
    target: &str,
    label: Option<&str>,
    subs: SubstitutionSet,
) -> Vec<Event<'static>> {
    let mut events: Vec<Event<'static>> = Vec::new();
    events.push(Event::Start(Tag::CrossReference {
        target: Cow::Owned(target.to_string()),
        label: label.map(|l| Cow::Owned(l.to_string())),
    }));
    match label {
        None => events.push(Event::Text(Cow::Owned(target.to_string()))),
        Some(l) if !l.is_empty() => {
            // Re-parse the label exactly as `push_macro_label` does: full subs
            // minus MACROS (so a nested macro stays literal and recursion ends).
            let inner: Vec<Event<'static>> =
                super::run_pipeline(l, subs.without(SubstitutionSet::MACROS));
            for e in inner {
                events.push(e);
            }
        }
        Some(_) => {} // empty explicit label → no events (mirrors push_macro_label(""))
    }
    events.push(Event::End(TagEnd::CrossReference));
    events
}

/// Whether `src[start..end]` (a candidate macro span) contains a tag sentinel —
/// i.e. an earlier pass already lifted a passthrough/escape/char-ref out of it,
/// so the raw text the legacy parser would re-parse is gone.
fn span_has_sentinel(src: &str, start: usize, end: usize) -> bool {
    src.as_bytes()[start..end].contains(&TAG_LEAD)
}
