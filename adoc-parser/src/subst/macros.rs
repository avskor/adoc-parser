//! Inline-macro extraction pass (Asciidoctor `macros` sub).
//!
//! Phase 2 (macros) ports the legacy inline macros into the sequential engine.
//! So far implemented:
//!
//! - **cross-reference** (1/N) — `xref:target[label]` and the `<<target>>` /
//!   `<<target,label>>` shorthand → a `CrossReference` tag;
//! - **link family** (2/N) — the `link:url[attrs]` macro, the `mailto:email[attrs]`
//!   macro (with `?subject=&body=` query encoding), bare URL autolinks
//!   (`http://`/`https://`/`ftp://`/`irc://`, with the optional `[label]` form),
//!   and bare email autolinks (`user@host.tld`) → a `Link` tag;
//! - **inline image** (3/N) — the `image:target[attrs]` macro → an `InlineImage`
//!   tag (a leaf carrying the parsed image attributes; the `image::` block form is
//!   left to the block scanner);
//! - **leaf macros** (4/N) — the `icon:name[attrs]` macro → an `Icon` tag, and the
//!   STEM family (`stem:[…]` / `latexmath:[…]` / `asciimath:[…]`) → a `Stem` tag.
//!   Like the inline image these carry no re-parsed label: the name/variant goes on
//!   the tag and the attrlist / math content becomes a single raw `Text` event.
//!   The STEM content honours the `\]` escape (unescaped to `]`).
//!
//! Every match is lifted out of the working buffer into a tag sentinel pointing at
//! a [`TagToken::Macro`] leaf that holds the macro's `Start`, its label events, and
//! its `End`, so the later attribute/quote/replacement passes cannot reach inside
//! it. (The remaining macro families — footnote/UI/anchor/index-term — are ported
//! in subsequent phases; the experimental UI macros — `kbd:`/`btn:`/`menu:` — need
//! the `:experimental:` option threaded through the pipeline, deferred with them.)
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

use crate::attributes::{parse_image_attrs, parse_link_attrs};
use crate::event::{Event, SubstitutionSet, Tag, TagEnd};
use crate::inline::url_encode_into;

use super::tokenize::{sentinel_end, utf8_char_len, Work, TAG_LEAD};

/// Extract every supported inline macro from `work.buf` into sentinels.
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

        // link:url[attrs]
        if bytes[i] == b'l' && src[i..].starts_with("link:") {
            if let Some((events, end)) = try_link(&src, i, subs) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // mailto:email[attrs]
        if bytes[i] == b'm' && src[i..].starts_with("mailto:") {
            if let Some((events, end)) = try_mailto(&src, i, subs) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // image:target[attrs] (but not the `image::` block form, which the block
        // scanner owns — the inline pass leaves it literal).
        if bytes[i] == b'i' && src[i..].starts_with("image:") && !src[i..].starts_with("image::") {
            if let Some((events, end)) = try_image(&src, i) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // icon:name[attrs]
        if bytes[i] == b'i' && src[i..].starts_with("icon:") {
            if let Some((events, end)) = try_icon(&src, i) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // stem:[content] / latexmath:[content] / asciimath:[content] (the three
        // STEM-macro spellings; each requires the `[` directly after the colon, so
        // there is no target part).
        if bytes[i] == b's' && src[i..].starts_with("stem:[") {
            if let Some((events, end)) = try_stem(&src, i, 5, "stem") {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }
        if bytes[i] == b'l' && src[i..].starts_with("latexmath:[") {
            if let Some((events, end)) = try_stem(&src, i, 10, "latexmath") {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }
        if bytes[i] == b'a' && src[i..].starts_with("asciimath:[") {
            if let Some((events, end)) = try_stem(&src, i, 10, "asciimath") {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
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

        // Bare URL autolink (http://, https://, ftp://, irc://), optionally
        // followed by a `[label]` attrlist.
        if matches!(bytes[i], b'h' | b'f' | b'i') && scheme_at(&src, i) {
            if let Some((events, end)) = try_autolink(&src, i, subs) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // Bare email autolink (user@host.tld): the local part has already been
        // copied to `out`, so on a match we truncate it back off before splicing
        // in the link sentinel.
        if bytes[i] == b'@' {
            if let Some((events, local_start, end)) = try_email(&src, i, subs) {
                out.truncate(out.len() - (i - local_start));
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
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

/// At a `link:` (caller guarantees the prefix), try to match `link:url[attrs]`.
/// Mirror of [`crate::inline::InlineState::try_link_macro`] for the plain form.
/// The legacy `link:++url++[…]` passthrough-in-URL variant is declined here: by
/// macros-time the `++url++` is already a passthrough sentinel, so the span guard
/// trips and the gate falls back to legacy (which still handles it).
fn try_link(src: &str, start: usize, subs: SubstitutionSet) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 5..]; // after "link:"
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.find(']')?;
    if bracket_end <= bracket_start {
        return None;
    }
    let url = &rest[..bracket_start];
    let content = &rest[bracket_start + 1..bracket_end];
    if url.is_empty() {
        return None;
    }
    let end = start + 5 + bracket_end + 1;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    let attrs = parse_link_attrs(content);
    // An empty attrlist text marks a "bare" link (visible text = the target).
    let is_bare = attrs.text.is_empty();
    let label = (!is_bare).then_some(attrs.text);
    let events = build_link(
        url.to_string(),
        attrs.window,
        attrs.nofollow,
        attrs.role,
        is_bare,
        label,
        url,
        subs,
    );
    Some((events, end))
}

/// At a `mailto:` (caller guarantees the prefix), try to match
/// `mailto:email[attrs]`. Mirror of
/// [`crate::inline::InlineState::try_mailto_macro`]: positional attrs 2/3 become
/// `?subject=&body=` query parameters, percent-encoded. A mailto link is never
/// "bare" (the visible text falls back to the email, not the `mailto:` URL).
fn try_mailto(src: &str, start: usize, subs: SubstitutionSet) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 7..]; // after "mailto:"
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.find(']')?;
    if bracket_end <= bracket_start {
        return None;
    }
    let email = &rest[..bracket_start];
    let content = &rest[bracket_start + 1..bracket_end];
    if email.is_empty() {
        return None;
    }
    let end = start + 7 + bracket_end + 1;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    let base = &src[start..start + 7 + bracket_start]; // "mailto:email"
    let attrs = parse_link_attrs(content);
    let url = match (attrs.subject, attrs.body) {
        (None, None) => base.to_string(),
        (subject, body) => {
            let mut u = String::from(base);
            let mut sep = '?';
            if let Some(s) = subject {
                u.push(sep);
                u.push_str("subject=");
                url_encode_into(&mut u, s);
                sep = '&';
            }
            if let Some(b) = body {
                u.push(sep);
                u.push_str("body=");
                url_encode_into(&mut u, b);
            }
            u
        }
    };
    let label = (!attrs.text.is_empty()).then_some(attrs.text);
    let events = build_link(url, attrs.window, attrs.nofollow, attrs.role, false, label, email, subs);
    Some((events, end))
}

/// At an `image:` (caller guarantees the prefix and that it is not the `image::`
/// block form), try to match `image:target[attrs]`. Mirror of
/// [`crate::inline::InlineState::try_inline_image`]. The inline image is a *leaf*
/// macro: unlike a link or cross-reference it carries no re-parsed label, only the
/// parsed image-attribute fields, so the `Start(InlineImage)`/`End` pair is built
/// directly (no `MACROS`-cleared sub-pipeline). An empty target is accepted — the
/// donor has no such guard, so `image:[alt]` still matches (the renderer makes the
/// `<img>` from the attrs). The tag fields are owned `Cow`s (`== Cow::Borrowed`
/// legacy by `PartialEq`, so the gate adopts).
fn try_image(src: &str, start: usize) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 6..]; // after "image:"
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.find(']')?;
    if bracket_end <= bracket_start {
        return None;
    }
    let target = &rest[..bracket_start];
    let content = &rest[bracket_start + 1..bracket_end];
    let end = start + 6 + bracket_end + 1;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    let a = parse_image_attrs(content);
    let events = vec![
        Event::Start(Tag::InlineImage {
            target: Cow::Owned(target.to_string()),
            alt: Cow::Owned(a.alt.to_string()),
            width: a.width.map(owned),
            height: a.height.map(owned),
            align: a.align.map(owned),
            float: a.float.map(owned),
            link: a.link.map(owned),
            role: a.role.map(owned),
            title: a.title.map(owned),
        }),
        Event::End(TagEnd::InlineImage),
    ];
    Some((events, end))
}

/// Owning conversion helper for the optional `&str` image-attribute fields.
fn owned(s: &str) -> Cow<'static, str> {
    Cow::Owned(s.to_string())
}

/// At an `icon:` (caller guarantees the prefix), try to match `icon:name[attrs]`.
/// Mirror of [`crate::inline::InlineState::try_icon_macro`] (and its
/// `parse_target_bracket_macro` helper). The inline icon is a *leaf* macro: the
/// name becomes the tag and the attrlist text (when non-empty) a single raw
/// `Text` event — neither is re-parsed through the engine. The closing `]` is the
/// first one after the `[`; an empty name declines.
fn try_icon(src: &str, start: usize) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 5..]; // after "icon:"
    let bracket_start = rest.find('[')?;
    let name = &rest[..bracket_start];
    if name.is_empty() {
        return None;
    }
    let bracket_end = rest.find(']')?;
    if bracket_end <= bracket_start {
        return None;
    }
    let attrs = &rest[bracket_start + 1..bracket_end];
    let end = start + 5 + bracket_end + 1;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    let mut events: Vec<Event<'static>> = vec![Event::Start(Tag::Icon {
        name: Cow::Owned(name.to_string()),
    })];
    if !attrs.is_empty() {
        events.push(Event::Text(Cow::Owned(attrs.to_string())));
    }
    events.push(Event::End(TagEnd::Icon));
    Some((events, end))
}

/// At a STEM-macro prefix (`stem:[` / `latexmath:[` / `asciimath:[`, caller
/// guarantees the `[` follows directly), try to match the bracketed content.
/// Mirror of [`crate::inline::InlineState::try_stem_macro`] (and its
/// `parse_bracket_macro_escaped` helper): a `]` immediately preceded by `\` does
/// not close the macro, and every `\]` in the content is unescaped to `]`
/// (Asciidoctor's `(.*?[^\\])?\]` rule). A *leaf* macro carrying the variant; the
/// (unescaped) content becomes a single raw `Text` event, not re-parsed. The
/// escape pass leaves `\]` untouched (its blanket arm keeps the backslash
/// literal), so the escaped bracket survives intact to here; any escape/
/// passthrough/char-ref the earlier passes *did* lift from inside trips the
/// sentinel guard and declines.
fn try_stem(
    src: &str,
    start: usize,
    prefix_len: usize,
    variant: &str,
) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + prefix_len..]; // starts with '[' (dispatch guaranteed)
    let bytes = rest.as_bytes();
    let mut i = 1;
    let bracket_end = loop {
        let off = bytes[i..].iter().position(|&b| b == b']')?;
        let at = i + off;
        if bytes[at - 1] == b'\\' {
            i = at + 1;
        } else {
            break at;
        }
    };
    let end = start + prefix_len + bracket_end + 1;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    let inner = &rest[1..bracket_end];
    let content = inner.replace("\\]", "]"); // no-op when no escaped bracket
    let mut events: Vec<Event<'static>> = vec![Event::Start(Tag::Stem {
        variant: Cow::Owned(variant.to_string()),
    })];
    if !content.is_empty() {
        events.push(Event::Text(Cow::Owned(content)));
    }
    events.push(Event::End(TagEnd::Stem));
    Some((events, end))
}

/// Whether an autolink scheme (`http://`/`https://`/`ftp://`/`irc://`) begins at
/// byte `i`.
fn scheme_at(src: &str, i: usize) -> bool {
    let rest = &src[i..];
    rest.starts_with("http://")
        || rest.starts_with("https://")
        || rest.starts_with("ftp://")
        || rest.starts_with("irc://")
}

/// Whether byte offset `i` is a valid left boundary for a bare autolink. Mirror of
/// [`crate::inline::InlineState::at_autolink_boundary`]: the preceding character
/// must be whitespace or one of `<>()[];`, or the start of the buffer. Boundary
/// characters are all ASCII and never extracted into sentinels, so checking the
/// preceding byte reproduces the legacy decision; an extracted construct leaves a
/// `TAG_TAIL` byte (not a boundary), matching the legacy parser's non-boundary
/// view of the same position. (A multibyte preceding char is treated as a
/// non-boundary; a Unicode-whitespace boundary is rare and the gate declines it.)
fn at_autolink_boundary(bytes: &[u8], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = bytes[i - 1];
    prev.is_ascii_whitespace() || matches!(prev, b'<' | b'>' | b'(' | b')' | b'[' | b']' | b';')
}

/// At an autolink scheme (caller guarantees `scheme_at`), try to match a bare URL
/// autolink, optionally followed by a `[label]` attrlist. Mirror of
/// [`crate::inline::InlineState::try_autolink`].
fn try_autolink(src: &str, start: usize, subs: SubstitutionSet) -> Option<(Vec<Event<'static>>, usize)> {
    if !at_autolink_boundary(src.as_bytes(), start) {
        return None;
    }
    let rest = &src[start..];
    let url_end = rest
        .find(|c: char| c.is_whitespace() || c == '[' || c == ']' || c == '<' || c == '>')
        .unwrap_or(rest.len());
    let mut url = &rest[..url_end];
    if url.len() <= 8 {
        return None;
    }
    // Trailing punctuation is stripped only from BARE urls — the `URL[text]` macro
    // form keeps it. Check for the attrlist on the UNSTRIPPED url first.
    let bracket_follows = rest[url_end..].starts_with('[') && rest[url_end..].contains(']');
    if !bracket_follows {
        while url.len() > 8
            && matches!(
                url.as_bytes()[url.len() - 1],
                b'.' | b',' | b';' | b':' | b'!' | b'?' | b')'
            )
        {
            url = &url[..url.len() - 1];
        }
    }
    let url_len = url.len();
    let after_url = &rest[url_len..];

    // URL[text] form.
    if after_url.starts_with('[')
        && let Some(close) = after_url.find(']')
    {
        let end = start + url_len + close + 1;
        if span_has_sentinel(src, start, end) {
            return None;
        }
        let attrs = parse_link_attrs(&after_url[1..close]);
        let is_bare = attrs.text.is_empty();
        let label = (!is_bare).then_some(attrs.text);
        let events = build_link(
            url.to_string(),
            attrs.window,
            attrs.nofollow,
            attrs.role,
            is_bare,
            label,
            url,
            subs,
        );
        return Some((events, end));
    }

    // Bare form.
    let end = start + url_len;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    let events = build_link(url.to_string(), None, false, None, true, None, url, subs);
    Some((events, end))
}

/// At an `@` (caller guarantees the byte), try to match a bare email autolink
/// `user@host.tld`. Mirror of
/// [`crate::inline::InlineState::try_email_autolink`]. Returns the link events,
/// the start of the local part (so the caller can truncate the already-copied
/// local part off `out`), and the index just past the domain. The backward scan
/// stops at any non-local-part byte, which includes the `TAG_LEAD`/`TAG_TAIL`
/// control bytes, so an earlier-pass sentinel bounds it exactly as the legacy
/// `text_start` flush boundary would.
fn try_email(
    src: &str,
    at_pos: usize,
    subs: SubstitutionSet,
) -> Option<(Vec<Event<'static>>, usize, usize)> {
    let bytes = src.as_bytes();

    // Backward scan for the local part (a-zA-Z0-9._+-).
    let mut local_start = at_pos;
    while local_start > 0 {
        let b = bytes[local_start - 1];
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-') {
            local_start -= 1;
        } else {
            break;
        }
    }
    if local_start == at_pos {
        return None; // empty local part
    }

    // Forward scan for the domain (a-zA-Z0-9.-), which must contain a dot.
    let mut domain_end = at_pos + 1;
    let mut has_dot = false;
    while domain_end < bytes.len() {
        let b = bytes[domain_end];
        if b.is_ascii_alphanumeric() || b == b'-' {
            domain_end += 1;
        } else if b == b'.' {
            has_dot = true;
            domain_end += 1;
        } else {
            break;
        }
    }
    if !has_dot {
        return None;
    }
    let domain = &src[at_pos + 1..domain_end];
    if domain.starts_with('.') || domain.starts_with('-') || domain.ends_with('.') || domain.ends_with('-')
    {
        return None;
    }

    let email = &src[local_start..domain_end];
    // Asciidoctor does not mark email autolinks as bare.
    let events = build_link(format!("mailto:{email}"), None, false, None, false, None, email, subs);
    Some((events, local_start, domain_end))
}

/// Build the `Start(Link) … End` event sequence shared by the link macro, mailto
/// macro, and URL/email autolinks. `label` (when `Some`) is re-parsed with
/// `MACROS` cleared via [`push_label`], mirroring `push_macro_label`; otherwise
/// `bare_text` is emitted as the visible text. The tag fields are owned `Cow`s
/// (`== Cow::Borrowed` legacy by `PartialEq`, so the gate adopts).
#[allow(clippy::too_many_arguments)]
fn build_link(
    url: String,
    window: Option<&str>,
    nofollow: bool,
    role: Option<&str>,
    is_bare: bool,
    label: Option<&str>,
    bare_text: &str,
    subs: SubstitutionSet,
) -> Vec<Event<'static>> {
    let mut events: Vec<Event<'static>> = vec![Event::Start(Tag::Link {
        url: Cow::Owned(url),
        window: window.map(|w| Cow::Owned(w.to_string())),
        nofollow,
        is_bare,
        role: role.map(|r| Cow::Owned(r.to_string())),
    })];
    match label {
        Some(l) => push_label(l, subs, &mut events),
        None => events.push(Event::Text(Cow::Owned(bare_text.to_string()))),
    }
    events.push(Event::End(TagEnd::Link));
    events
}

/// Re-parse a macro label's raw text exactly as `push_macro_label` does — the
/// engine's own pipeline with `MACROS` cleared (so a nested macro stays literal
/// and recursion terminates). An empty label yields no events, matching
/// `push_macro_label("")`. Callers only pass `Some(label)` for a non-empty label
/// (an empty attrlist text routes through the bare branch), but the guard keeps
/// the mirror exact.
fn push_label(text: &str, subs: SubstitutionSet, events: &mut Vec<Event<'static>>) {
    if text.is_empty() {
        return;
    }
    let inner: Vec<Event<'static>> = super::run_pipeline(text, subs.without(SubstitutionSet::MACROS));
    events.extend(inner);
}

/// Whether `src[start..end]` (a candidate macro span) contains a tag sentinel —
/// i.e. an earlier pass already lifted a passthrough/escape/char-ref out of it,
/// so the raw text the legacy parser would re-parse is gone.
fn span_has_sentinel(src: &str, start: usize, end: usize) -> bool {
    src.as_bytes()[start..end].contains(&TAG_LEAD)
}
