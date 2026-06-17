//! Passthrough extraction pass (Asciidoctor `extract_passthroughs`).
//!
//! Runs FIRST in the pipeline, before quotes. Every passthrough form
//! (`+++…+++`, `++…++`, `+…+`, and the bare `pass:[…]` macro) is lifted out of
//! the working buffer and replaced with a single tag sentinel pointing at a
//! [`TagToken::Passthrough`] leaf. Because the sentinel bytes are opaque
//! non-word control bytes, the later quotes/replacements/post-replacement
//! passes cannot reach inside the extracted content — exactly how Asciidoctor
//! protects passthroughs by extracting them up front. The tokenizer restores
//! each leaf verbatim as `InlinePassthrough` (raw) or `Text` (html-escaped)
//! pieces.
//!
//! The matching mirrors the legacy recursive parser's `handle_inline_passthrough`
//! / `try_*_passthrough` / `try_pass_macro` so the events it produces match what
//! the legacy parser would have produced. The bare `pass:[…]` becomes a verbatim
//! [`TagToken::Passthrough`] leaf; the spec'd `pass:SPEC[…]` (`pass:q[…]`,
//! `pass:c,a[…]`, …) re-runs exactly its spec'd substitutions over the bracketed
//! content (via the engine's own [`super::run_pipeline`], mirroring
//! `push_pass_spec_content`) and seals the resulting event sequence as one opaque
//! [`TagToken::Macro`] leaf — see [`try_pass_spec_macro`].
//!
//! Two backslash escapes are folded into this pass (Asciidoctor's `\\?` capture):
//! the escaped single-plus `\+…+` and the escaped pass macro `\pass:SPEC[…]`.
//! Both drop the backslash and leave the would-be passthrough's content in the
//! buffer to flow through the later passes rather than being extracted — see the
//! arms in [`extract`]. The double-backslash `\\++…++` marker escape is handled
//! too; the single-backslash doubled-marker forms (`\++`/`\+++`) and the other
//! double-backslash variants (`\\+`/`\\pass:`/`\\+++`) are deferred — `\++…`
//! flags a decline (Asciidoctor's own handling is self-inconsistent) so
//! [`super::try_parse`] falls back to legacy.

use crate::event::{Event, SubstitutionSet};
use crate::inline::InlineOptions;

use super::tokenize::{utf8_char_len, PassPiece, Work};

/// Extract every passthrough span from `work.buf` into sentinels.
///
/// `subs` is consulted only for the hard-break interaction: when
/// `post_replacements` is active, a ` +` immediately before a `\n` is a hard
/// line break (the legacy parser consumes it at the space, before the `+` can
/// open a single-plus passthrough), so the single-plus form must not claim it —
/// it is left in the buffer for the [`super::post_replacements`] pass.
pub(super) fn extract(work: &mut Work, subs: SubstitutionSet, options: InlineOptions) {
    let guard_hard_break = subs.has(SubstitutionSet::POST_REPLACEMENTS);
    let src = std::mem::take(&mut work.buf);
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        // Escaped double-plus passthrough `\\++…++`: Asciidoctor's
        // `(\\{0,2})(\+{2,3})` capture consumes BOTH backslashes when a `++…++`
        // passthrough would form here, leaving the `++` markers LITERAL while the
        // content flows through the remaining subs — the double-plus analogue of
        // the unconstrained `\\**…**` marker escape (`\\++*x*++` →
        // `++<strong>x</strong>++`, `\\++a<b++` → `++a&lt;b++`, `\\++pp++` →
        // `++pp++`). Seal each `++` as its own literal `Text` leaf (a `Macro` leaf,
        // opaque to re-extraction and non-coalescing like the marker escape) and
        // leave the content raw. Restricted to exactly `++` (not `+++`): the
        // triple-plus `\\+++…+++` form stays deferred — Asciidoctor's own handling
        // of it is inconsistent. A longer backslash run cascades naturally (each
        // leading `\` is copied by the fall-through until this arm fires on the
        // adjacent pair).
        if b == b'\\'
            && bytes.get(i + 1).copied() == Some(b'\\')
            && bytes.get(i + 2).copied() == Some(b'+')
            && bytes.get(i + 3).copied() == Some(b'+')
            && bytes.get(i + 4).copied() != Some(b'+')
            && let Some((_, end)) = try_double_plus(&src, i + 2)
        {
            let close = end - 2; // position of the closing `++`
            out.push_str(&work.macro_sentinel(vec![Event::Text("++".into())]));
            out.push_str(&src[i + 4..close]); // raw content flows through later passes
            out.push_str(&work.macro_sentinel(vec![Event::Text("++".into())]));
            i = end;
            continue;
        }

        // Deferred: a SINGLE-backslash escaped double/triple-plus marker (`\++…` /
        // `\+++…`). Asciidoctor escapes the marker (the `++`/`+++` stays literal),
        // but its handling of these forms is self-inconsistent (the parsing-lab ASG
        // and the Ruby HTML disagree), so the engine does not reproduce them yet:
        // flag a decline and let [`super::try_parse`] fall back to the legacy
        // parser, which still has the raw source. The DOUBLE-backslash `\\++…++`
        // form above IS handled; this only catches the single-backslash marker.
        if b == b'\\'
            && bytes.get(i + 1).copied() == Some(b'+')
            && bytes.get(i + 2).copied() == Some(b'+')
        {
            super::flag_decline();
            out.push('\\');
            i += 1;
            continue;
        }

        // Escaped single-plus passthrough `\+…+`: Asciidoctor folds the `\\?`
        // escape into the passthrough match, so the backslash is honoured only
        // when an unescaped `+…+` would form a single-plus passthrough here. When
        // it would, drop the backslash and emit the opening `+` literal; the
        // content and closing `+` are NOT extracted — they flow through the normal
        // substitutions (`\+*b*+` → `+<strong>b</strong>+`), exactly as the
        // escaped form renders. When it would not (`\+nopass`, `a\+b+c`), the `\+`
        // is left for the [`super::escape`] pass to keep literal. A doubled
        // backslash (`\\+plus+`) consumes only the one backslash adjacent to the
        // `+` (the `\\?` capture sits right before it), leaving any leading
        // backslashes as literal — each is copied by the fall-through until this
        // arm fires on the adjacent one (`\\+plus+` → `\+plus+`). The doubled
        // (`\++`/`\+++`) marker form stays deferred.
        if b == b'\\'
            && bytes.get(i + 1).copied() == Some(b'+')
            && bytes.get(i + 2).copied() != Some(b'+')
            && try_single_plus(&src, bytes, i + 1, guard_hard_break).is_some()
        {
            out.push('+');
            i += 2;
            continue;
        }

        // Escaped pass macro `\pass:SPEC[…]`: Asciidoctor folds the `\\?` escape
        // into the pass-macro match, so the backslash drops and extraction is
        // skipped, BUT the bracketed content still runs the remaining
        // substitutions — it is NOT a verbatim passthrough. Emit `pass:SPEC[` as
        // literal text (it flows through the later passes as plain text) and
        // resume scanning right after the `[`, so the content is processed
        // normally (`\pass:c[*b*]` → `pass:c[<strong>b</strong>]`,
        // `` `\pass:[]` `` → `<code>pass:[]</code>`). Without the escape the
        // `pass:[…]` arm below would lift the whole macro into a sentinel, leaving
        // the bare backslash behind. A double (or longer) backslash run before
        // `pass:` (`\\pass:SPEC[…]`) consumes only the one backslash adjacent to
        // the macro (the `\\?` capture sits right before it), leaving any leading
        // backslashes literal — each is copied by the fall-through until this arm
        // fires on the adjacent one (`\\pass:c[*b*]` → `\pass:c[<strong>b</strong>]`,
        // `\\\pass:[x]` → `\\pass:[x]`). Mirrors the legacy `pass_escape_prefix_len`
        // arm of `handle_inline_escape`.
        if b == b'\\'
            && let Some(prefix_len) = pass_escape_prefix_len(&src, i + 1)
        {
            out.push_str(&src[i + 1..i + 1 + prefix_len]); // drop `\`, keep `pass:SPEC[`
            i += 1 + prefix_len;
            continue;
        }

        // `+`-delimited passthroughs: +++ / ++ / + (triple retries as double,
        // mirroring `handle_inline_passthrough`).
        if b == b'+'
            && let Some((pieces, end)) = try_plus(&src, bytes, i, guard_hard_break)
        {
            out.push_str(&work.passthrough_sentinel(pieces));
            i = end;
            continue;
        }

        // `pass:[…]` macro (bare form only).
        if b == b'p'
            && let Some((pieces, end)) = try_pass_macro(&src, i)
        {
            out.push_str(&work.passthrough_sentinel(pieces));
            i = end;
            continue;
        }

        // `pass:SPEC[…]` macro (spec'd form, e.g. `pass:q[…]`, `pass:c,a[…]`).
        // Unlike the bare form its content is NOT verbatim: it is re-run through
        // exactly the spec'd substitutions and the resulting events are sealed as
        // a single opaque leaf, mirroring Asciidoctor (which extracts the macro up
        // front and applies its subs at restore time). Tried after the bare arm,
        // which declines spec'd specs.
        if b == b'p'
            && let Some((events, end)) = try_pass_spec_macro(&src, i, options)
        {
            out.push_str(&work.macro_sentinel(events));
            i = end;
            continue;
        }

        // No passthrough here: copy the character verbatim. A `+` that began no
        // form falls through to here, consuming exactly one byte — mirroring the
        // legacy `self.pos += 1` fall-through so a later `+` still gets a chance.
        let len = utf8_char_len(b);
        out.push_str(&src[i..i + len]);
        i += len;
    }

    work.buf = out;
}

/// At a `+`, try `+++` then `++` (triple falls back to double like Asciidoctor's
/// `(\+\+\+?)…\1`), else a constrained single `+`.
fn try_plus(
    src: &str,
    bytes: &[u8],
    i: usize,
    guard_hard_break: bool,
) -> Option<(Vec<PassPiece>, usize)> {
    let next = bytes.get(i + 1).copied();
    let next2 = bytes.get(i + 2).copied();
    if next == Some(b'+') && next2 == Some(b'+') {
        // +++ (raw), retrying as ++ on failure.
        return try_triple_plus(src, i).or_else(|| try_double_plus(src, i));
    }
    if next == Some(b'+') {
        return try_double_plus(src, i);
    }
    try_single_plus(src, bytes, i, guard_hard_break)
}

/// `+++text+++` — raw passthrough (no subs). Mirror `try_triple_plus_passthrough`.
fn try_triple_plus(src: &str, i: usize) -> Option<(Vec<PassPiece>, usize)> {
    let after_open = i + 3;
    let rest = src.get(after_open..)?;
    let close = rest.find("+++")?;
    if close == 0 {
        return None;
    }
    let inner = &rest[..close];
    Some((
        vec![PassPiece { text: inner.to_string(), raw: true }],
        after_open + close + 3,
    ))
}

/// `++text++` — passthrough with the `specialcharacters` sub only (escaped),
/// so the leaf is emitted as `Text`. Empty content (`++++`) yields no piece but
/// still consumes a sentinel slot, preserving the surrounding text split.
/// Mirror `try_double_plus_passthrough`.
fn try_double_plus(src: &str, i: usize) -> Option<(Vec<PassPiece>, usize)> {
    let after_open = i + 2;
    let rest = src.get(after_open..)?;
    let close = rest.find("++")?;
    let inner = &rest[..close];
    let pieces = if inner.is_empty() {
        Vec::new()
    } else {
        vec![PassPiece { text: inner.to_string(), raw: false }]
    };
    Some((pieces, after_open + close + 2))
}

/// `+text+` — constrained single-plus passthrough. Mirror
/// `try_single_plus_passthrough`: the opening `+` must not follow a word char,
/// the content's first char must not be a space, and the closing `+` obeys the
/// constrained-close rule (not preceded by `+`/space, not followed by `+`/word).
/// A `pass:[…]` macro inside the span is extracted first, so a `+` in its
/// brackets cannot close. Caller guarantees `bytes[i] == b'+'` and
/// `bytes[i + 1] != b'+'`.
fn try_single_plus(
    src: &str,
    bytes: &[u8],
    i: usize,
    guard_hard_break: bool,
) -> Option<(Vec<PassPiece>, usize)> {
    // Opening '+' must not follow a word character.
    if i > 0 {
        let prev = bytes[i - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' {
            return None;
        }
    }

    let after_open = i + 1;
    match bytes.get(after_open) {
        None | Some(b' ') => return None,
        _ => {}
    }

    // A ` +\n` is a hard line break, not a passthrough open: the legacy parser
    // consumes it at the preceding space (post_replacements) before the `+` can
    // start a single-plus span. Leave it for the post_replacements pass.
    if guard_hard_break
        && i > 0
        && bytes[i - 1] == b' '
        && bytes.get(after_open).copied() == Some(b'\n')
    {
        return None;
    }

    // Find the constrained closing '+', skipping `pass:[…]` regions.
    let s = &src[after_open..];
    let sb = s.as_bytes();
    let mut close = None;
    let mut k = 0;
    while k < sb.len() {
        let c = sb[k];
        if c == b'p'
            && let Some(skip) = crate::scanner::pass_macro_span_len(s, k)
        {
            k += skip;
            continue;
        }
        if c == b'+' && k > 0 {
            let preceded_by_plus = sb[k - 1] == b'+';
            let preceded_by_space = sb[k - 1] == b' ';
            let next = sb.get(k + 1).copied();
            let followed_by_plus = next == Some(b'+');
            let followed_by_word = next.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_');
            if !preceded_by_plus && !preceded_by_space && !followed_by_plus && !followed_by_word {
                close = Some(k);
                break;
            }
        }
        k += 1;
    }
    let close = close?;
    let inner = &src[after_open..after_open + close];
    Some((single_plus_pieces(inner), after_open + close + 1))
}

/// Port of `InlineState::push_single_plus_content`: the content of a single-plus
/// passthrough is literal `Text`, except embedded `pass:[…]` macros, which
/// become raw `InlinePassthrough` (or `Text` when the spec keeps specialchars).
fn single_plus_pieces(inner: &str) -> Vec<PassPiece> {
    let bytes = inner.as_bytes();
    let mut pieces = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'p'
            && let Some(skip) = crate::scanner::pass_macro_span_len(inner, i)
        {
            if i > text_start {
                pieces.push(PassPiece { text: inner[text_start..i].to_string(), raw: false });
            }
            let spec_len = crate::scanner::pass_spec_len(&inner[i..], 5).unwrap_or(0);
            let spec = &inner[i + 5..i + 5 + spec_len];
            // content sits between "pass:SPEC[" and the trailing "]"
            let content = &inner[i + 5 + spec_len + 1..i + skip - 1];
            let escaped = !spec.is_empty()
                && crate::inline::pass_spec_to_subs(spec).has(SubstitutionSet::SPECIALCHARS);
            pieces.push(PassPiece { text: content.to_string(), raw: !escaped });
            i += skip;
            text_start = i;
            continue;
        }
        i += 1;
    }
    if text_start < inner.len() {
        pieces.push(PassPiece { text: inner[text_start..].to_string(), raw: false });
    }
    pieces
}

/// Length of the `pass:SPEC[` prefix of an escaped pass macro `\pass:SPEC[…]`
/// beginning at byte `p` (the position right after the backslash), or `None` if
/// `p` does not start one. Port of
/// [`crate::inline::InlineState::pass_escape_prefix_len`]: requires the `pass:`
/// literal, an optional lowercase subs spec, and an opening `[`. The returned
/// length spans `pass:` through that `[` inclusive; the bracketed content and the
/// trailing `]` are NOT part of it (they flow through the remaining passes).
fn pass_escape_prefix_len(src: &str, p: usize) -> Option<usize> {
    let rest = src.get(p..)?;
    if !rest.starts_with("pass:") {
        return None;
    }
    let spec_len = crate::scanner::pass_spec_len(rest, 5)?;
    Some(5 + spec_len + 1) // "pass:" + spec + "["
}

/// `pass:[…]` macro, bare form only (no subs spec → raw verbatim). Mirror
/// `try_pass_macro` for `spec_len == 0`. A spec'd `pass:SPEC[…]` is handled by
/// [`try_pass_spec_macro`] instead: this returns `None` so the dispatch falls
/// through to that arm.
fn try_pass_macro(src: &str, i: usize) -> Option<(Vec<PassPiece>, usize)> {
    let rest = src.get(i..)?;
    if !rest.starts_with("pass:") {
        return None;
    }
    let spec_len = crate::scanner::pass_spec_len(src, i + 5)?;
    if spec_len != 0 {
        return None; // spec'd form: see `try_pass_spec_macro`
    }
    let after = &src[i + 5..];
    if !after.starts_with('[') {
        return None;
    }
    let bracket_end = after.find(']')?;
    let inner = &after[1..bracket_end];
    Some((
        vec![PassPiece { text: inner.to_string(), raw: true }],
        i + 5 + bracket_end + 1,
    ))
}

/// `pass:SPEC[…]` macro with a non-empty subs spec (`pass:q[…]`, `pass:c,a[…]`,
/// `pass:quotes[…]`). Mirror of the legacy `try_pass_macro` +
/// `push_pass_spec_content`: the spec maps to a [`SubstitutionSet`] via
/// [`crate::inline::pass_spec_to_subs`], the bracketed content runs to the first
/// `]` (no `\]` unescaping — same as `parse_bracket_macro`), and the content is
/// re-run through exactly those substitutions. The resulting events are returned
/// for sealing as one opaque [`super::tokenize::TagToken::Macro`] leaf, so the
/// later passes (quotes/replacements/…) cannot reach inside the protected
/// content — exactly how Asciidoctor's `extract_passthroughs` lifts the macro out
/// before any substitution runs. `spec_len == 0` (bare form) is declined here; it
/// is handled verbatim by [`try_pass_macro`].
fn try_pass_spec_macro(
    src: &str,
    i: usize,
    options: InlineOptions,
) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = src.get(i..)?;
    if !rest.starts_with("pass:") {
        return None;
    }
    let spec_len = crate::scanner::pass_spec_len(src, i + 5)?;
    if spec_len == 0 {
        return None; // bare form handled by `try_pass_macro`
    }
    let after = &src[i + 5 + spec_len..];
    if !after.starts_with('[') {
        return None;
    }
    let bracket_end = after.find(']')?;
    let content = &after[1..bracket_end];
    let spec = &src[i + 5..i + 5 + spec_len];
    let set = crate::inline::pass_spec_to_subs(spec);
    let end = i + 5 + spec_len + bracket_end + 1;
    Some((pass_spec_events(content, set, options), end))
}

/// Run `content` through exactly the substitutions in `set` and downgrade the
/// plain-text runs to `InlinePassthrough` when `set` lacks `specialcharacters`,
/// so the renderer emits them unescaped — mirroring the legacy
/// `push_pass_spec_content`. The inner [`super::run_pipeline`] reproduces the
/// recursive reparse the legacy parser does with a fresh `InlineState`. Empty
/// content yields no events (the legacy parser emits nothing for `pass:q[]`),
/// and the engine's own empty-buffer guard would otherwise materialise a stray
/// empty `Text`.
fn pass_spec_events(
    content: &str,
    set: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'static>> {
    if content.is_empty() {
        return Vec::new();
    }
    let escape = set.has(SubstitutionSet::SPECIALCHARS);
    let inner: Vec<Event<'static>> = super::run_pipeline(content, set, options);
    inner
        .into_iter()
        .map(|ev| match ev {
            Event::Text(t) if !escape => Event::InlinePassthrough(t),
            ev => ev,
        })
        .collect()
}
