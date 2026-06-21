//! Inline-macro extraction pass (Asciidoctor `macros` sub).
//!
//! Phase 2 (macros) ports the legacy inline macros into the sequential engine.
//! So far implemented:
//!
//! - **cross-reference** (1/N) — `xref:target[label]` and the `<<target>>` /
//!   `<<target,label>>` shorthand → a `CrossReference` tag. The `<<…>>` target
//!   must begin with `[\p{Word}#/.:{]` (Asciidoctor's `InlineXrefMacroRx`), so a
//!   leading `<` (`<<<`), `"`, `-`, or space declines and the angle brackets stay
//!   literal; the legacy parser lacks this guard but never reaches a `<<` inside a
//!   span, so the engine is the more Asciidoctor-faithful one on the divergent forms;
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
//! - **anchor + index-term** (5/N) — the anchor family (`[[id]]` / `[[id,label]]`,
//!   the `[[[id]]]` bibliography form, and the `anchor:id[label]` macro) → an
//!   `Anchor` tag / a `BibliographyAnchor` event; and the index-term family (the
//!   `((term))` flow / `(((primary, secondary)))` concealed shorthand, the
//!   `indexterm:[…]` concealed macro, and the `indexterm2:[term]` flow macro) →
//!   `IndexTerm` / `ConcealedIndexTerm` events. All are leaves: the id/label/term
//!   text is stored verbatim, never re-parsed.
//! - **footnote** (6/N) — `footnote:[text]` / `footnote:id[text]` define a footnote
//!   (`Footnote`), and `footnote:id[]` references an existing one (`FootnoteRef`).
//!   A leaf: the text is verbatim (the registry/numbering/foot-list live in the
//!   renderer, shared by both engines).
//! - **UI macros** (7/N) — the `:experimental:`-gated `kbd:[keys]` (`Keyboard`),
//!   `btn:[label]` (`Button`), and `menu:target[items]` (`Menu`). Dispatched only
//!   when `options.experimental` is set; otherwise the bytes flow through as plain
//!   text (Asciidoctor registers these only under `:experimental:`). Leaves: the
//!   keys/label/items are verbatim `Text` the renderer splits (`+`/`,` for keys,
//!   `>` for the menu sequence).
//!
//! This pass also folds in the **escaped autolink** `\http://…` (the escape pass's
//! span-aware home for it): the backslash drops where an unescaped autolink could
//! open and the URL stays literal text. See [`autolink_open_boundary`].
//!
//! Every match is lifted out of the working buffer into a tag sentinel pointing at
//! a [`TagToken::Macro`] leaf that holds the macro's `Start`, its label events, and
//! its `End`, so the later attribute/quote/replacement passes cannot reach inside
//! it. With the UI macros ported, every inline macro family the legacy parser
//! recognises now has a home here.
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
//! ## Sentinel handling in a macro span
//!
//! Because earlier passes already lifted passthroughs/escapes/char-refs into
//! sentinels, a macro whose source span now contains one (`xref:x[+raw+]`, a
//! passthrough inside the label) no longer has the raw text the legacy parser
//! would re-parse. There are two cases:
//!
//! - **Sentinel in a re-parsed LABEL** (`link:`/`mailto:`/`xref:`/`<<>>`/
//!   URL`[…]`): handled natively. [`reparse_label`] re-parses the label with a
//!   *seeded* sub-pipeline ([`super::run_pipeline_seeded`]) whose tag table is a
//!   clone of the outer one, so the sentinel resolves against its leaf and the
//!   label events match what the top-level tokenizer would emit — mirroring
//!   Asciidoctor, where a passthrough placeholder survives the label's
//!   `subs.without(:macros)` re-substitution and is restored globally at the end.
//! - **Sentinel in a VERBATIM leaf** (image alt & target, icon name & attrs, stem
//!   content, anchor id & label, index-term text, UI key/label/menu item): handled
//!   natively. [`restore_verbatim`] splices a passthrough's protected content and
//!   an escaped `Literal` back into the verbatim string — exactly what
//!   Asciidoctor's global restore leaves in the attribute — so `image:i.png[++a
//!   b++]` → `alt="a b"` and `kbd:[++Ctrl++]` → a single `<kbd>Ctrl</kbd>`. Any
//!   delimiter split (`,` for anchor/index parts, `>` for menus) happens on the
//!   SOURCE first, so a passthrough's protected delimiter stays inside one part. A
//!   char-ref still punts (its verbatim-vs-escaped treatment is family-specific).
//! - **Sentinel in a verbatim link/cross-ref TARGET or non-label attribute** (the
//!   id/URL/email and role/window/subject/body those tags store without
//!   re-parsing): the verbatim source is gone, so the pass declines the span AND
//!   records the punt via [`target_has_sentinel`]/[`attr_has_sentinel`] (the link
//!   family's targets accept the richer [`reconstruct_link_target`]/
//!   [`passthrough_url`] reconstruction instead); [`super::try_parse`] falls back
//!   to legacy for the whole paragraph.
//!
//! The one family still punting on any span sentinel via [`span_has_sentinel`] is
//! **footnote**: the renderer (`render_footnote_text`) RE-PARSES the footnote text
//! through the full inline pass, so it needs the *raw* source (`++raw++`), not the
//! restored content — which the sentinel cannot reproduce (the passthrough marker
//! count is lost). Legacy keeps the raw text, so the punt renders correctly;
//! converting it needs the parser↔renderer redesign that carries pre-parsed
//! footnote events. The common case — plain targets and text/quote labels — carries
//! no sentinels and is extracted normally.

use std::borrow::Cow;

use crate::attributes::{LinkKind, parse_image_attrs, parse_link_attrs};
use crate::event::{Event, MenuPart, SubstitutionSet, Tag, TagEnd};
use crate::inline::{url_encode_into, InlineOptions};

use super::flag_decline;
use super::tokenize::{desentinelize, sentinel_end, utf8_char_len, TagToken, Work, TAG_LEAD};

/// Record that a macro was declined because its span contained a sentinel — the
/// shared [`super::flag_decline`] makes [`super::try_parse`] fall back to legacy.
/// The ~17 punt sites are free functions taking `&str` (not `&mut Work`), so a
/// thread-local flag avoids threading a parameter through every macro matcher.
fn flag_punt() {
    flag_decline();
}

/// Extract every supported inline macro from `work.buf` into sentinels.
pub(super) fn extract(work: &mut Work, subs: SubstitutionSet, options: InlineOptions) {
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
            if let Some((events, end)) = try_xref(&src, i, subs, work, options) {
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
            if let Some((events, end)) = try_link(&src, i, subs, work, options) {
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
            if let Some((events, end)) = try_mailto(&src, i, subs, work, options) {
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
            if let Some((events, end)) = try_image(&src, i, work) {
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
            if let Some((events, end)) = try_icon(&src, i, work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // indexterm2:[term] (the "flow" macro form — the term renders in place).
        // Checked before `indexterm:` because the two prefixes diverge only at the
        // `2`, so the order is immaterial; mirroring the legacy dispatch keeps it
        // obvious.
        if bytes[i] == b'i' && src[i..].starts_with("indexterm2:") {
            if let Some((events, end)) = try_indexterm2(&src, i, work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // indexterm:[primary, secondary, tertiary] (the concealed macro form).
        if bytes[i] == b'i' && src[i..].starts_with("indexterm:") {
            if let Some((events, end)) = try_indexterm(&src, i, work) {
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
            if let Some((events, end)) = try_stem(&src, i, 5, "stem", work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }
        if bytes[i] == b'l' && src[i..].starts_with("latexmath:[") {
            if let Some((events, end)) = try_stem(&src, i, 10, "latexmath", work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }
        // anchor:id[xreflabel] (the inline-anchor macro form of `[[id]]`).
        if bytes[i] == b'a' && src[i..].starts_with("anchor:") {
            if let Some((events, end)) = try_anchor_macro(&src, i, work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        if bytes[i] == b'a' && src[i..].starts_with("asciimath:[") {
            if let Some((events, end)) = try_stem(&src, i, 10, "asciimath", work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // footnote:[text] / footnote:id[text] / footnote:id[] (a reference to an
        // already-defined footnote). Fires on `f` before the bare `ftp://` autolink
        // arm below; `footnote:` never satisfies `scheme_at`, so the order is only
        // for clarity.
        if bytes[i] == b'f' && src[i..].starts_with("footnote:") {
            if let Some((events, end)) = try_footnote(&src, i) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // Experimental UI macros — `kbd:[keys]`, `btn:[label]`, and
        // `menu:target[items]` — dispatched ONLY when `:experimental:` is set
        // (`options.experimental`). With the attribute unset the prefix bytes are
        // copied verbatim and the `[…]` interior flows through the later passes as
        // ordinary text, matching Asciidoctor (which registers these macros only
        // under `:experimental:`). The legacy parser instead *skips* the whole
        // `[…]` so its interior is never re-substituted; that is a legacy-only
        // quirk with no corpus coverage, so on the rare input where it would differ
        // the engine now keeps its own Asciidoctor-faithful result. Each macro is a
        // leaf: the content/items become
        // a single raw `Text` the renderer splits (`+`/`,` for keys, `>` for menu
        // items) — never re-parsed, mirroring `try_kbd_macro`/`try_btn_macro`/
        // `try_menu_macro`. A failed match advances one byte (legacy `pos += 1`).
        if options.experimental && bytes[i] == b'k' && src[i..].starts_with("kbd:") {
            if let Some((events, end)) = try_kbd(&src, i, work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }
        if options.experimental && bytes[i] == b'b' && src[i..].starts_with("btn:") {
            if let Some((events, end)) = try_btn(&src, i, work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }
        if options.experimental && bytes[i] == b'm' && src[i..].starts_with("menu:") {
            if let Some((events, end)) = try_menu(&src, i, work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }
        // Escaped quoted inline menu (`\"…"`): drop the backslash, keep the quoted
        // string literal (no menu). Only when the `"…"` would otherwise match the
        // quoted-menu rule; otherwise fall through so `\` is copied verbatim by the
        // catch-all. Mirrors `InlineMenuRx`'s leading-`\` branch (`next $&.slice 1,
        // …`). `escape` leaves a `\"` not followed by a backtick intact, so the
        // backslash reaches here.
        if options.experimental
            && bytes[i] == b'\\'
            && bytes.get(i + 1) == Some(&b'"')
            && let Some(end) = quoted_menu_span_end(&src, i + 1)
        {
            out.push_str(&src[i + 1..end]); // literal `"…"`, backslash dropped
            i = end;
            continue;
        }
        // Quoted inline menu (`"File > Edit > Copy"`): a double-quoted run whose
        // content starts with `[\w&]` and holds a space-flanked `>` becomes a menu
        // sequence. Detected here, BEFORE the `quotes` pass turns the `"`
        // typographic (Asciidoctor runs `InlineMenuRx` in `sub_macros`, so the menu
        // wins over smart quotes). On a non-match the `"` is copied literally for
        // the later smart-quote pass. We match a literal `>` (the renderer escapes
        // to `&gt;`), the pre-specialchars stand-in for Asciidoctor's `&gt;`.
        if options.experimental && bytes[i] == b'"' {
            if let Some((events, end)) = try_quoted_menu(&src, i, subs, work, options) {
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
            if let Some((events, end)) = try_cross_ref(&src, i, subs, work, options) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            // Not a valid cross reference → advance past one '<'.
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // Bibliography anchor `[[[id]]]` / `[[[id, label]]]` and the plain anchor
        // `[[id]]` / `[[id,xreflabel]]`. Only the `[[`-doubled form is a macro; a
        // single `[` opens the quotes attrlist span (`[.role]#x#`, handled later by
        // the quotes pass), so the dispatch fires solely on a following `[`. The
        // triple-bracket bibliography form is checked first (it is a superset).
        if bytes[i] == b'[' && bytes.get(i + 1) == Some(&b'[') {
            if bytes.get(i + 2) == Some(&b'[') {
                if let Some((events, end)) = try_bibliography_anchor(&src, i, work) {
                    out.push_str(&work.macro_sentinel(events));
                    i = end;
                    continue;
                }
            } else if let Some((events, end)) = try_anchor(&src, i, work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            // Not a valid anchor → advance past one '[' (mirrors the legacy
            // `pos += 1` so the second `[` is re-examined on the next iteration).
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // Index term: concealed `(((primary, secondary)))` or flow `((term))`. One
        // pattern (Asciidoctor `\(\((.+?)\)\)(?!\))`); the matched content's own
        // enclosing parens decide the form.
        if bytes[i] == b'(' && bytes.get(i + 1) == Some(&b'(') {
            if let Some((events, end)) = try_index_term(&src, i, work) {
                out.push_str(&work.macro_sentinel(events));
                i = end;
                continue;
            }
            // Not a valid index term → advance past one '('.
            out.push_str(&src[i..i + 1]);
            i += 1;
            continue;
        }

        // Escaped bare autolink `\http://…` (also `https`/`ftp`/`irc`): drop the
        // backslash so the URL stays literal text. The backslash is NOT copied to
        // `out` but is LEFT in `src`, so when the scheme is re-examined on the next
        // iteration the autolink arm below sees `\` as the preceding byte, fails
        // `autolink_open_boundary` (a backslash is not a span marker), and declines
        // to link — exactly the legacy
        // `handle_inline_escape` arm, whose dropped-but-retained backslash likewise
        // blocks the URL. Fires only where an unescaped autolink could open: at a
        // real boundary (start / whitespace / `<>()[];`) OR immediately inside a
        // constrained quote span that opens here (`` `\http…` `` / `*\http…*`), the
        // pre-`quotes` stand-in for Asciidoctor's `>`-after-`<code>` boundary.
        // Without one (`word\http…`, `\\http…`) the backslash stays literal.
        if bytes[i] == b'\\'
            && matches!(bytes.get(i + 1), Some(b'h' | b'f' | b'i'))
            && scheme_at(&src, i + 1)
            && autolink_open_boundary(work, bytes, i)
        {
            i += 1; // drop the backslash; the scheme is left literal below
            continue;
        }

        // Bare URL autolink (http://, https://, ftp://, irc://), optionally
        // followed by a `[label]` attrlist.
        if matches!(bytes[i], b'h' | b'f' | b'i') && scheme_at(&src, i) {
            if let Some((events, end, strip_angle)) = try_autolink(work, &src, i, subs, options) {
                // `<https://…>`: drop the leading `<` already copied to `out` (the
                // closing `>` is already past `end`). See `try_autolink`.
                if strip_angle && out.ends_with('<') {
                    out.truncate(out.len() - 1);
                }
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
            if let Some((events, local_start, end)) = try_email(&src, i, subs, options) {
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

/// Find the byte index (within `s`) of the `]` that closes a bracketed inline
/// macro whose opening `[` is at byte index `open`. A `]` immediately preceded by
/// a backslash is *escaped* and does not close the macro — it is part of the
/// content (Asciidoctor's `(.*?[^\\])?\]` rule). Returns `None` when no unescaped
/// `]` follows the `[`. Shared by every bracketed macro (`pass`, `link`, `xref`,
/// `mailto`, `image`, `icon`, `footnote`, `kbd`/`btn`/`menu`, stem) so the escape
/// is honoured uniformly; pair with [`unescape_close_bracket`] on the content.
/// `pub(super)` so [`super::passthrough`] shares it for `pass:[…]`/`pass:SPEC[…]`.
pub(super) fn find_macro_close_bracket(s: &str, open: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = open + 1;
    loop {
        let off = bytes[i..].iter().position(|&b| b == b']')?;
        let at = i + off;
        // `at == open + 1` is `]` directly after `[` (empty content): never
        // escaped (the preceding byte is `[`). Otherwise a preceding `\` escapes it.
        if at > open + 1 && bytes[at - 1] == b'\\' {
            i = at + 1;
        } else {
            return Some(at);
        }
    }
}

/// Unescape `\]` → `]` in extracted bracketed-macro content. No-op (and no
/// allocation) when no escaped bracket is present. Mirrors Asciidoctor unescaping
/// the captured attrlist/label/verbatim text before any further processing
/// (`text.gsub ESC_R_SB, R_SB`). `pub(super)` so [`super::passthrough`] shares it.
pub(super) fn unescape_close_bracket(s: &str) -> Cow<'_, str> {
    if s.contains("\\]") {
        Cow::Owned(s.replace("\\]", "]"))
    } else {
        Cow::Borrowed(s)
    }
}

/// At an `xref:` (caller guarantees the prefix), try to match `xref:target[label]`.
/// Returns the macro's event sequence plus the index just past the closing `]`,
/// or `None` to leave the `xref:` literal. Mirror of
/// [`crate::inline::InlineState::try_xref_macro`].
fn try_xref(
    src: &str,
    start: usize,
    subs: SubstitutionSet,
    work: &Work,
    options: InlineOptions,
) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 5..]; // after "xref:"
    let bracket_start = rest.find('[')?;
    let bracket_end = find_macro_close_bracket(rest, bracket_start)?;
    let target = &rest[..bracket_start];
    let label_text = unescape_close_bracket(&rest[bracket_start + 1..bracket_end]);
    if target.is_empty() {
        return None;
    }
    let end = start + 5 + bracket_end + 1;
    // A sentinel in the TARGET still punts: the verbatim id/path the
    // `CrossReference` tag carries is no longer present. A sentinel in the LABEL is
    // re-parsed natively by [`build_cross_reference`] (seeded with the outer table),
    // so it no longer forces the whole span to legacy.
    if target_has_sentinel(target) {
        return None;
    }
    // Empty brackets → no explicit label (legacy `None`); a non-empty label is an
    // explicit one.
    let label = (!label_text.is_empty()).then_some(label_text.as_ref());
    Some((build_cross_reference(target, label, &work.tags, subs, options), end))
}

/// Whether `content` begins with a character Asciidoctor accepts as the first
/// byte of a cross-reference / `xref:` target: `[\p{Word}#/.:{]` (a word char —
/// approximated as a Unicode alphanumeric or `_` — or one of `#`, `/`, `.`, `:`,
/// `{`). An empty string, or a leading `<`/`"`/`-`/space, is rejected.
fn xref_target_start_ok(content: &str) -> bool {
    matches!(
        content.chars().next(),
        Some(c) if c.is_alphanumeric() || matches!(c, '_' | '#' | '/' | '.' | ':' | '{')
    )
}

/// At a `<<` (caller guarantees the prefix), try to match `<<target>>` /
/// `<<target,label>>`. Mirror of
/// [`crate::inline::InlineState::try_cross_reference`].
fn try_cross_ref(
    src: &str,
    start: usize,
    subs: SubstitutionSet,
    work: &Work,
    options: InlineOptions,
) -> Option<(Vec<Event<'static>>, usize)> {
    let after_open = start + 2;
    let rest = &src[after_open..];
    let close = rest.find(">>")?;
    let content = &rest[..close];
    if content.is_empty() {
        return None;
    }
    // Asciidoctor's `InlineXrefMacroRx` requires the target to begin with
    // `[\p{Word}#/.:{]`; a leading `<` (so `<<<` declines and stays literal text,
    // e.g. inside a `` `<<<` `` monospace span), `"`, `-`, or whitespace is not a
    // valid cross-reference start. The legacy recursive parser has no such guard,
    // but it never reaches a `<<` buried inside a constrained span (the span is
    // consumed and re-parsed first), so on those forms the engine declines the link
    // and now matches Asciidoctor. The dispatcher advances by a single `<` on
    // `None`, so a later valid `<<` in the same run still matches (`<<<b>>` →
    // literal `<` + xref `#b`).
    if !xref_target_start_ok(content) {
        return None;
    }
    let end = after_open + close + 2;
    // With a comma: trim both target and label. Without: the whole content is the
    // target (untrimmed), no explicit label.
    let (target, label) = if let Some((t, l)) = content.split_once(',') {
        (t.trim(), Some(l.trim()))
    } else {
        (content, None)
    };
    // A leading '#' is an explicit-anchor marker, not part of the id.
    let target = target.strip_prefix('#').unwrap_or(target);
    // A sentinel in the target id still punts (its verbatim source is gone); a
    // sentinel in the label is re-parsed natively (seeded) by `build_cross_reference`.
    if target_has_sentinel(target) {
        return None;
    }
    Some((build_cross_reference(target, label, &work.tags, subs, options), end))
}

/// Build the `Start(CrossReference) … End` event sequence. `label` is the raw
/// explicit label text (`None` for the bracket-less / empty-bracket form). The
/// `CrossReference` tag carries `target` and an `is_some()`-significant `label`
/// field (only its presence drives the renderer). The label *events* are
/// re-parsed with `MACROS` cleared, matching
/// `push_macro_label`; an empty explicit label (`<<a,>>`) yields no label events
/// (as `push_macro_label("")` does), while the no-label form emits the target as
/// the link text.
fn build_cross_reference(
    target: &str,
    label: Option<&str>,
    seed: &[TagToken],
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'static>> {
    let mut events: Vec<Event<'static>> = Vec::new();
    events.push(Event::Start(Tag::CrossReference {
        target: Cow::Owned(target.to_string()),
        // Only the field's presence drives the renderer (the label *events* below
        // carry the visible text), but desentinelize it so a label that swallowed a
        // passthrough/escape/char-ref leaf carries the restored source rather than a
        // raw `TAG_LEAD … TAG_TAIL` control sequence. No-sentinel labels are
        // byte-unchanged (the helper fast-paths them).
        label: label.map(|l| Cow::Owned(desentinelize(seed, l))),
    }));
    match label {
        None => events.push(Event::Text(Cow::Owned(target.to_string()))),
        Some(l) if !l.is_empty() => {
            // Re-parse the label exactly as `push_macro_label` does: full subs
            // minus MACROS (so a nested macro stays literal and recursion ends).
            for e in reparse_label(l, seed, subs, options) {
                events.push(e);
            }
        }
        Some(_) => {} // empty explicit label → no events (mirrors push_macro_label(""))
    }
    events.push(Event::End(TagEnd::CrossReference));
    events
}

/// At a `link:` (caller guarantees the prefix), try to match `link:url[attrs]`.
/// Mirror of [`crate::inline::InlineState::try_link_macro`].
///
/// The legacy parser also has a `link:++url++[…]` special case: a passthrough
/// wraps a URL whose special characters (`[`, ` `, repeating `__`) would
/// otherwise break the macro or trip another sub. By macros-time that `++url++`
/// is already a [`TagToken::Passthrough`] leaf, so the URL part of the span is a
/// lone sentinel. When it is, [`passthrough_url`] reconstructs the verbatim URL
/// from the leaf and the link is built with it — reproducing the legacy special
/// case (Asciidoctor's general `extract_passthroughs`-then-match mechanism). Any
/// other sentinel shape in the span (a sentinel inside the *label*, or a URL part
/// that is more than one bare passthrough sentinel) declines (recording the punt
/// via [`super::flag_decline`]), so the paragraph falls back to legacy.
fn try_link(
    src: &str,
    start: usize,
    subs: SubstitutionSet,
    work: &Work,
    options: InlineOptions,
) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 5..]; // after "link:"
    let bracket_start = rest.find('[')?;
    let bracket_end = find_macro_close_bracket(rest, bracket_start)?;
    let url_part = &rest[..bracket_start];
    let content = unescape_close_bracket(&rest[bracket_start + 1..bracket_end]);
    if url_part.is_empty() {
        return None;
    }
    let end = start + 5 + bracket_end + 1;
    // The URL is normally plain text; two sentinel forms are supported: a
    // passthrough-protected target (`link:++url++[…]`, reconstructed verbatim) and
    // an escaped typographic pattern sealed as a `Literal` (`link:a\...b[…]`,
    // reconstructed via [`reconstruct_link_target`]). Any other sentinel shape in
    // the URL part punts.
    let url: Cow<str> = if url_part.as_bytes().contains(&TAG_LEAD) {
        if let Some(u) = passthrough_url(work, url_part) {
            Cow::Owned(u)
        } else if let Some(u) = reconstruct_link_target(work, url_part) {
            Cow::Owned(u)
        } else {
            flag_punt();
            return None;
        }
    } else {
        Cow::Borrowed(url_part)
    };
    if url.is_empty() {
        return None;
    }
    let attrs = parse_link_attrs(&content, LinkKind::Link);
    // A sentinel in a non-label attribute (role/window) is stored verbatim on the
    // tag with no native reconstruction, so it still punts; one confined to the
    // label text is re-parsed natively (seeded) by `build_link`.
    if attr_has_sentinel(attrs.window) || attr_has_sentinel(attrs.role) {
        return None;
    }
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
        &url,
        &work.tags,
        subs,
        options,
    );
    Some((events, end))
}

/// If `url_part` is exactly one passthrough sentinel, reconstruct the verbatim URL
/// the legacy `link:++url++[…]` special case used. An earlier pass already lifted
/// the `++url++` (or any other passthrough form) out of the buffer into a
/// [`TagToken::Passthrough`] leaf, so the URL is the concatenation of the leaf's
/// verbatim pieces — Asciidoctor's restored target. Returns `None` when `url_part`
/// is not a lone sentinel (e.g. text surrounds it, or two sentinels) or the
/// sentinel is not a passthrough, so the caller declines and the paragraph falls
/// back to legacy.
fn passthrough_url(work: &Work, url_part: &str) -> Option<String> {
    let bytes = url_part.as_bytes();
    if bytes.first() != Some(&TAG_LEAD) {
        return None;
    }
    let end = sentinel_end(bytes, 0);
    if end != bytes.len() {
        return None; // text surrounds the sentinel within the URL part
    }
    let idx: usize = url_part[1..end - 1].parse().ok()?;
    match work.tags.get(idx)? {
        TagToken::Passthrough(pieces) => {
            let mut url = String::new();
            for p in pieces {
                url.push_str(&p.text);
            }
            Some(url)
        }
        _ => None,
    }
}

/// At a `mailto:` (caller guarantees the prefix), try to match
/// `mailto:email[attrs]`. Mirror of
/// [`crate::inline::InlineState::try_mailto_macro`]: positional attrs 2/3 become
/// `?subject=&body=` query parameters, percent-encoded. A mailto link is never
/// "bare" (the visible text falls back to the email, not the `mailto:` URL).
fn try_mailto(
    src: &str,
    start: usize,
    subs: SubstitutionSet,
    work: &Work,
    options: InlineOptions,
) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 7..]; // after "mailto:"
    let bracket_start = rest.find('[')?;
    let bracket_end = find_macro_close_bracket(rest, bracket_start)?;
    let email = &rest[..bracket_start];
    let content = unescape_close_bracket(&rest[bracket_start + 1..bracket_end]);
    if email.is_empty() {
        return None;
    }
    let end = start + 7 + bracket_end + 1;
    // The email goes verbatim into the URL, so a sentinel there still punts.
    if target_has_sentinel(email) {
        return None;
    }
    let base = &src[start..start + 7 + bracket_start]; // "mailto:email"
    let attrs = parse_link_attrs(&content, LinkKind::Mailto);
    // `subject`/`body` are URL-encoded verbatim and `role`/`window` stored verbatim,
    // none with native reconstruction — a sentinel in any of them still punts. Only
    // the label text (`attrs.text`) gains the seeded native re-parse.
    if attr_has_sentinel(attrs.subject)
        || attr_has_sentinel(attrs.body)
        || attr_has_sentinel(attrs.window)
        || attr_has_sentinel(attrs.role)
    {
        return None;
    }
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
    let events = build_link(
        url,
        attrs.window,
        attrs.nofollow,
        attrs.role,
        false,
        label,
        email,
        &work.tags,
        subs,
        options,
    );
    Some((events, end))
}

/// At an `image:` (caller guarantees the prefix and that it is not the `image::`
/// block form), try to match `image:target[attrs]`. Mirror of
/// [`crate::inline::InlineState::try_inline_image`]. The inline image is a *leaf*
/// macro: unlike a link or cross-reference it carries no re-parsed label, only the
/// parsed image-attribute fields, so the `Start(InlineImage)`/`End` pair is built
/// directly (no `MACROS`-cleared sub-pipeline). An empty target is accepted — the
/// donor has no such guard, so `image:[alt]` still matches (the renderer makes the
/// `<img>` from the attrs). The tag fields are owned `Cow`s, semantically equal
/// to the legacy parser's borrowed ones. A passthrough/escape sentinel in the
/// target or attrlist is restored verbatim ([`restore_verbatim`]) so
/// `image:i.png[++a b++]` → `alt="a b"`; a char-ref still punts.
fn try_image(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 6..]; // after "image:"
    let bracket_start = rest.find('[')?;
    let bracket_end = find_macro_close_bracket(rest, bracket_start)?;
    let target = restore_verbatim(work, Cow::Borrowed(&rest[..bracket_start]))?;
    let content = restore_verbatim(work, unescape_close_bracket(&rest[bracket_start + 1..bracket_end]))?;
    let end = start + 6 + bracket_end + 1;
    let a = parse_image_attrs(&content);
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
/// first one after the `[`; an empty name declines. A passthrough/escape sentinel
/// in the name or attrs is restored verbatim ([`restore_verbatim`]); a char-ref punts.
fn try_icon(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 5..]; // after "icon:"
    let bracket_start = rest.find('[')?;
    let name = restore_verbatim(work, Cow::Borrowed(&rest[..bracket_start]))?;
    if name.is_empty() {
        return None;
    }
    let bracket_end = find_macro_close_bracket(rest, bracket_start)?;
    let attrs = restore_verbatim(work, unescape_close_bracket(&rest[bracket_start + 1..bracket_end]))?;
    let end = start + 5 + bracket_end + 1;
    let mut events: Vec<Event<'static>> = vec![Event::Start(Tag::Icon {
        name: Cow::Owned(name.into_owned()),
    })];
    if !attrs.is_empty() {
        events.push(Event::Text(Cow::Owned(attrs.into_owned())));
    }
    events.push(Event::End(TagEnd::Icon));
    Some((events, end))
}

/// At a `footnote:` (caller guarantees the prefix), try to match
/// `footnote:[text]` / `footnote:id[text]` / `footnote:id[]`. Mirror of
/// [`crate::inline::InlineState::try_footnote_macro`]. A *leaf* macro: the text is
/// stored verbatim on the event and never re-parsed, because the footnote registry,
/// numbering, and the document-foot list all live in the renderer (shared by both
/// engines) — none of it is inline-parser state.
///
/// The form splits as: an id is every byte before the first `[` and must be
/// non-empty; the content runs to the first *unescaped* `]` (a `\]` is part of the
/// content, unescaped to `]` — Asciidoctor's `\]`-honouring rule, shared via
/// [`find_macro_close_bracket`]/[`unescape_close_bracket`]). A named macro with
/// empty content (`footnote:id[]`) is a *reference* to an already-defined footnote
/// (`FootnoteRef`); every other form *defines* one (`Footnote`). Asciidoctor's
/// `InlineFootnoteMacroRx` is also stricter on the id (`[\p{Word}-]+`) and accepts
/// the deprecated `footnoteref:` spelling; the legacy parser does neither, and this
/// engine deliberately mirrors the legacy form there, so those rare forms match
/// legacy (and, like legacy, diverge from Asciidoctor).
///
/// Unlike the other verbatim leaves, footnote still declines on ANY span sentinel
/// (the [`span_has_sentinel`] guard) rather than restoring it: the renderer
/// (`render_footnote_text`) RE-PARSES the stored text through the full inline pass,
/// so it needs the *raw* source (`footnote:[++raw++]` must reach the renderer as
/// `++raw++` for the passthrough to survive). [`restore_verbatim`] would hand back
/// the restored *content* (`raw`), which the renderer would then re-substitute
/// (`__` → emphasis) — wrong. The raw markers cannot be reconstructed from the
/// passthrough leaf, so the punt (legacy keeps the raw text) is the correct
/// behaviour until footnotes carry pre-parsed events across the parser↔renderer
/// boundary.
fn try_footnote(src: &str, start: usize) -> Option<(Vec<Event<'static>>, usize)> {
    let after_prefix = start + 9; // after "footnote:"
    let rest = src.get(after_prefix..)?;
    if rest.is_empty() {
        return None;
    }
    // Anonymous (`footnote:[…]`) vs named (`footnote:id[…]`): the id is every byte
    // before the first `[`, and must be non-empty.
    let (id, bracket_rest) = if rest.starts_with('[') {
        (None, rest)
    } else {
        let bracket_pos = rest.find('[')?;
        let id = &rest[..bracket_pos];
        if id.is_empty() {
            return None;
        }
        (Some(id), &rest[bracket_pos..])
    };
    let bracket_end = find_macro_close_bracket(bracket_rest, 0)?; // `]` within `bracket_rest`
    let content = unescape_close_bracket(&bracket_rest[1..bracket_end]);
    let id_len = id.map_or(0, str::len);
    let end = after_prefix + id_len + 1 + bracket_end;
    if span_has_sentinel(src, start, end) {
        return None;
    }
    let events: Vec<Event<'static>> = match (id, content.is_empty()) {
        // `footnote:id[]` — a reference to an existing definition.
        (Some(id_str), true) => vec![Event::FootnoteRef {
            id: Cow::Owned(id_str.to_string()),
        }],
        // `footnote:[text]` / `footnote:id[text]` — defines a footnote.
        _ => vec![Event::Footnote {
            id: id.map(|s| Cow::Owned(s.to_string())),
            text: Cow::Owned(content.into_owned()),
        }],
    };
    Some((events, end))
}

/// Shared body for the two bracket-only UI macros `kbd:[keys]` and `btn:[label]`.
/// Both require the `[` directly after the prefix (no target part), take the
/// content up to the first `]`, and DECLINE on empty content — mirroring the
/// legacy `parse_bracket_macro` + `try_kbd_macro`/`try_btn_macro`. The content is a
/// single raw `Text` (the renderer's `kbd_mode` splits keys on `+`/`,`; the button
/// label renders through the normal text path) and is never re-parsed. A
/// passthrough/escape sentinel in the content is restored verbatim
/// ([`restore_verbatim`]) — so `kbd:[++Ctrl++]` yields a single `<kbd>Ctrl</kbd>`
/// instead of the legacy fallback's `+`-split mangling — while a char-ref punts.
/// Caller (dispatch) guarantees the prefix and that `:experimental:` is set.
fn try_bracket_ui(
    src: &str,
    start: usize,
    prefix_len: usize,
    open: Tag<'static>,
    close: TagEnd,
    work: &Work,
) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + prefix_len..];
    if !rest.starts_with('[') {
        return None;
    }
    let bracket_end = find_macro_close_bracket(rest, 0)?;
    let content = restore_verbatim(work, unescape_close_bracket(&rest[1..bracket_end]))?;
    if content.is_empty() {
        return None;
    }
    let end = start + prefix_len + bracket_end + 1;
    Some((
        vec![
            Event::Start(open),
            Event::Text(Cow::Owned(content.into_owned())),
            Event::End(close),
        ],
        end,
    ))
}

/// `kbd:[keys]` — the keyboard UI macro (`kbd:` is 4 bytes). See [`try_bracket_ui`].
fn try_kbd(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    try_bracket_ui(src, start, 4, Tag::Keyboard, TagEnd::Keyboard, work)
}

/// `btn:[label]` — the button UI macro (`btn:` is 4 bytes). See [`try_bracket_ui`].
fn try_btn(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    try_bracket_ui(src, start, 4, Tag::Button, TagEnd::Button, work)
}

/// `menu:target[items]` — the menu UI macro. Mirror of
/// [`crate::inline::InlineState::try_menu_macro`] (and its
/// `parse_target_bracket_macro` helper): the target is every byte before the first
/// `[` and must be non-empty; the items run to the first `]` and, when non-empty,
/// become a single raw `Text` (the renderer splits the menu sequence on `>`).
/// Declines on an empty target or a `]` at/before the `[`. A passthrough/escape
/// sentinel in the target or items is restored verbatim ([`restore_verbatim`]); a
/// char-ref punts. Caller (dispatch) guarantees the `menu:` prefix and that
/// `:experimental:` is set.
fn try_menu(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 5..]; // after "menu:"
    let bracket_start = rest.find('[')?;
    let target = restore_verbatim(work, Cow::Borrowed(&rest[..bracket_start]))?;
    if target.is_empty() {
        return None;
    }
    let bracket_end = find_macro_close_bracket(rest, bracket_start)?;
    let items = restore_verbatim(work, unescape_close_bracket(&rest[bracket_start + 1..bracket_end]))?;
    let end = start + 5 + bracket_end + 1;
    let mut events: Vec<Event<'static>> = vec![Event::Start(Tag::Menu {
        target: Cow::Owned(target.into_owned()),
    })];
    if !items.is_empty() {
        events.push(Event::Text(Cow::Owned(items.into_owned())));
    }
    events.push(Event::End(TagEnd::Menu));
    Some((events, end))
}

/// At an opening `"` (byte index `open`), test whether the double-quoted run is a
/// quoted-menu candidate per Asciidoctor `InlineMenuRx`
/// (`/\\?"([\w&][^"]*?[ \n]+&gt;[ \n]+[^"]*)"/`). The close is the FIRST `"` after
/// `open` (`[^"]*` forbids an embedded quote); the content must start with `[\w&]`
/// and hold at least one space/newline-flanked `>` ([`has_spaced_gt`]). We match a
/// literal `>` because specialchars (`>`→`&gt;`) is not a pipeline pass — the HTML
/// escape happens only in the renderer. Returns the index just past the closing
/// `"`. Used both by the `"` arm (build the menu) and the `\"` arm (strip escape).
fn quoted_menu_span_end(src: &str, open: usize) -> Option<usize> {
    let rel = src[open + 1..].find('"')?;
    let close = open + 1 + rel;
    let content = &src[open + 1..close];
    let first = *content.as_bytes().first()?;
    if !(first.is_ascii_alphanumeric() || first == b'_' || first == b'&') {
        return None;
    }
    if !has_spaced_gt(content) {
        return None;
    }
    Some(close + 1)
}

/// Whether `content` holds a `>` with a space or newline IMMEDIATELY on both sides
/// (Asciidoctor's `[ \n]+&gt;[ \n]+` — space/newline only, never a tab). The first
/// content byte is `[\w&]` (guaranteed by [`quoted_menu_span_end`]), so a `>` can
/// never sit at index 0.
fn has_spaced_gt(content: &str) -> bool {
    let b = content.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'>' {
            let left = i > 0 && matches!(b[i - 1], b' ' | b'\n');
            let right = matches!(b.get(i + 1), Some(b' ' | b'\n'));
            if left && right {
                return true;
            }
        }
    }
    false
}

/// Build a quoted inline menu sequence from the `"…"` run opening at `start`.
/// Splits the content on EVERY `>` and strips each segment (Asciidoctor
/// `menu, *submenus = $1.split('&gt;').map(&:strip); menuitem = submenus.pop`);
/// the first segment is the menu, the last the menuitem, the rest submenus. Each
/// segment is re-parsed through the full pipeline (MACROS kept ON) so an inner
/// `icon:`/`image:`/`link:`/quote renders inside its `<b>` — mirroring
/// Asciidoctor's later whole-buffer macro pass over the generated menuseq. The
/// `[^"]*` content forbids an inner `"`, so the `"`-arm cannot re-fire on a
/// segment and the recursion terminates. Declines on a sentinel in the span
/// (corpus-unreachable; the inner `icon:` is parsed here, not lifted earlier).
fn try_quoted_menu(
    src: &str,
    start: usize,
    subs: SubstitutionSet,
    work: &Work,
    options: InlineOptions,
) -> Option<(Vec<Event<'static>>, usize)> {
    let end = quoted_menu_span_end(src, start)?;
    let content = &src[start + 1..end - 1];
    let segments: Vec<&str> = content.split('>').map(str::trim).collect();
    Some((build_menuseq(&segments, &work.tags, subs, options), end))
}

/// Emit the structural menuseq events for `segments` (≥2, guaranteed by the
/// space-flanked `>`): `Start(MenuSeq)`, one `MenuPart{role}` per segment with its
/// re-parsed inline events nested inside, `End(MenuSeq)`. Roles: index 0 = `Menu`,
/// last = `Item`, middle = `Submenu`. An empty segment emits an empty part. A
/// segment that swallowed a passthrough/escape/char-ref sentinel is re-parsed
/// seeded ([`reparse_seeded`]) so it resolves against the outer leaf.
fn build_menuseq(
    segments: &[&str],
    seed: &[TagToken],
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'static>> {
    let mut events: Vec<Event<'static>> = vec![Event::Start(Tag::MenuSeq)];
    let last = segments.len() - 1;
    for (i, seg) in segments.iter().enumerate() {
        let role = if i == 0 {
            MenuPart::Menu
        } else if i == last {
            MenuPart::Item
        } else {
            MenuPart::Submenu
        };
        events.push(Event::Start(Tag::MenuPart { role }));
        if !seg.is_empty() {
            // Full subs (MACROS ON) so a nested macro/quote renders inside the part.
            events.extend(reparse_seeded(seg, seed, subs, options));
        }
        events.push(Event::End(TagEnd::MenuPart));
    }
    events.push(Event::End(TagEnd::MenuSeq));
    events
}

/// At a STEM-macro prefix (`stem:[` / `latexmath:[` / `asciimath:[`, caller
/// guarantees the `[` follows directly), try to match the bracketed content.
/// Mirror of [`crate::inline::InlineState::try_stem_macro`] (and its
/// `parse_bracket_macro_escaped` helper): a `]` immediately preceded by `\` does
/// not close the macro, and every `\]` in the content is unescaped to `]`
/// (Asciidoctor's `(.*?[^\\])?\]` rule). A *leaf* macro carrying the variant; the
/// (unescaped) content becomes a single raw `Text` event, not re-parsed. The
/// escape pass leaves `\]` untouched (its blanket arm keeps the backslash
/// literal), so the escaped bracket survives intact to here; a passthrough or
/// escaped `Literal` the earlier passes lifted from inside is restored verbatim
/// ([`restore_verbatim`]) so `stem:[++x++]` → `\$x\$`, while a char-ref still punts.
fn try_stem(
    src: &str,
    start: usize,
    prefix_len: usize,
    variant: &str,
    work: &Work,
) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + prefix_len..]; // starts with '[' (dispatch guaranteed)
    let bracket_end = find_macro_close_bracket(rest, 0)?;
    let end = start + prefix_len + bracket_end + 1;
    let content = restore_verbatim(work, unescape_close_bracket(&rest[1..bracket_end]))?;
    let mut events: Vec<Event<'static>> = vec![Event::Start(Tag::Stem {
        variant: Cow::Owned(variant.to_string()),
    })];
    if !content.is_empty() {
        events.push(Event::Text(Cow::Owned(content.into_owned())));
    }
    events.push(Event::End(TagEnd::Stem));
    Some((events, end))
}

/// At a `[[` (caller guarantees the doubled bracket and that it is not the
/// `[[[` bibliography form), try to match the anchor `[[id]]` / `[[id,xreflabel]]`.
/// Mirror of [`crate::inline::InlineState::try_anchor`]. A *leaf* macro: the id and
/// xreflabel are stored verbatim on the tag (never re-parsed), so the
/// `Start(Anchor)`/`End` pair is built directly. The comma form trims the id's
/// trailing whitespace and the label's leading whitespace, dropping an empty label.
/// The id/label are split on the SOURCE comma (a sentinel never contains one) and
/// only then restored verbatim ([`restore_verbatim`]) so a passthrough's protected
/// comma stays inside one part; a char-ref still punts.
fn try_anchor(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 2..]; // after "[["
    let close = rest.find("]]")?;
    let content = &rest[..close];
    if content.is_empty() {
        return None;
    }
    let (id_raw, label_raw) = match content.split_once(',') {
        Some((i, l)) => {
            let l = l.trim_start();
            (i.trim_end(), (!l.is_empty()).then_some(l))
        }
        None => (content, None),
    };
    if id_raw.is_empty() {
        return None;
    }
    let end = start + 2 + close + 2;
    let id = restore_verbatim(work, Cow::Borrowed(id_raw))?;
    if id.is_empty() {
        return None;
    }
    let label = match label_raw {
        Some(l) => Some(restore_verbatim(work, Cow::Borrowed(l))?.into_owned()),
        None => None,
    };
    let events = vec![
        Event::Start(Tag::Anchor {
            id: Cow::Owned(id.into_owned()),
            label: label.map(Cow::Owned),
        }),
        Event::End(TagEnd::Anchor),
    ];
    Some((events, end))
}

/// At a `[[[` (caller guarantees the triple bracket), try to match the
/// bibliography anchor `[[[id]]]` / `[[[id, label]]]`. Mirror of
/// [`crate::inline::InlineState::try_bibliography_anchor`]. A *leaf* emitting a
/// standalone `BibliographyAnchor` event. With a comma, both id and label are
/// fully trimmed and an *empty* label is still `Some` (unlike the plain anchor),
/// matching the donor.
fn try_bibliography_anchor(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let after_open = start + 3; // after "[[["
    let rest = &src[after_open..];
    let close = rest.find("]]]")?;
    let content = &rest[..close];
    if content.is_empty() {
        return None;
    }
    let (id_raw, label_raw) = if let Some((i, l)) = content.split_once(',') {
        let id = i.trim();
        if id.is_empty() {
            return None;
        }
        (id, Some(l.trim()))
    } else {
        (content, None)
    };
    let end = after_open + close + 3;
    // Split on the source comma, then restore each part verbatim (passthrough /
    // escape); a char-ref punts.
    let id = restore_verbatim(work, Cow::Borrowed(id_raw))?;
    let label = match label_raw {
        Some(l) => Some(restore_verbatim(work, Cow::Borrowed(l))?.into_owned()),
        None => None,
    };
    let events = vec![Event::BibliographyAnchor {
        id: Cow::Owned(id.into_owned()),
        label: label.map(Cow::Owned),
    }];
    Some((events, end))
}

/// At an `anchor:` (caller guarantees the prefix), try to match the inline-anchor
/// macro `anchor:id[]` / `anchor:id[xreflabel]` — equivalent to `[[id]]`. Mirror of
/// [`crate::inline::InlineState::try_anchor_macro`]. The id must be a valid
/// Asciidoctor anchor id ([`crate::scanner::is_valid_anchor_id`]); the bracket
/// content is the xreflabel (reference text, never rendered in place), stored
/// verbatim. A *leaf*.
fn try_anchor_macro(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 7..]; // after "anchor:"
    let bracket = rest.find('[')?;
    let id = &rest[..bracket];
    // Asciidoctor's `InlineAnchorRx` requires the id to match
    // `[CC_ALPHA_:][CC_WORD\-:.]*` immediately before `[`; an invalid run
    // (`anchor:<id>[…]`, `anchor:1abc[…]`) is not an anchor and stays literal. A
    // sentinel byte in the id (a passthrough/escape there) also fails this, so the
    // id never needs restoring — only the xreflabel can carry one.
    if !crate::scanner::is_valid_anchor_id(id) {
        return None;
    }
    let bracket_end = find_macro_close_bracket(rest, bracket)?;
    let label_text = restore_verbatim(work, unescape_close_bracket(&rest[bracket + 1..bracket_end]))?;
    let end = start + 7 + bracket_end + 1;
    let label = (!label_text.is_empty()).then(|| Cow::Owned(label_text.into_owned()));
    let events = vec![
        Event::Start(Tag::Anchor {
            id: Cow::Owned(id.to_string()),
            label,
        }),
        Event::End(TagEnd::Anchor),
    ];
    Some((events, end))
}

/// Closing `))` position for an index term, relative to the content start. Mirror
/// of [`crate::inline::InlineState::index_term_close`] (Asciidoctor's non-greedy
/// `(.+?)\)\)(?!\))`): the first `))` whose follower is yet another `)` slides
/// forward by one, extending the content (`a)))` → content `a)`).
fn index_term_close(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let mut close = rest.find("))")?;
    while bytes.get(close + 2) == Some(&b')') {
        close += 1;
    }
    if close == 0 { None } else { Some(close) }
}

/// At a `((` (caller guarantees the doubled paren), try to match an index term.
/// Mirror of [`crate::inline::InlineState::try_index_term`]: the matched content's
/// own enclosing parens decide the form — both → concealed (comma-split) term, one
/// → literal paren beside a flow term, neither → a plain flow term. A *leaf*: all
/// pieces are stored verbatim, and a literal `(`/`)` becomes its own `Text` event
/// just as the donor pushes it (the tokenizer emits the leaf's events without
/// coalescing).
fn try_index_term(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let after_open = start + 2; // after "(("
    let rest = &src[after_open..];
    let close = index_term_close(rest)?;
    let content = &rest[..close];
    let end = after_open + close + 2;
    // The enclosing parens are literal source bytes (a sentinel never is one); the
    // term text inside is restored verbatim (passthrough/escape), char-ref punts.
    let starts = content.starts_with('(');
    let ends = content.ends_with(')');
    let term = |t: &str| restore_verbatim(work, Cow::Owned(t.to_string())).map(Cow::into_owned);
    let events = if starts && ends {
        concealed_index_term(&content[1..content.len() - 1], work)?
    } else if starts {
        vec![
            Event::Text(Cow::Borrowed("(")),
            Event::IndexTerm {
                text: Cow::Owned(term(&content[1..])?),
            },
        ]
    } else if ends {
        vec![
            Event::IndexTerm {
                text: Cow::Owned(term(&content[..content.len() - 1])?),
            },
            Event::Text(Cow::Borrowed(")")),
        ]
    } else {
        vec![Event::IndexTerm {
            text: Cow::Owned(term(content)?),
        }]
    };
    Some((events, end))
}

/// At an `indexterm:` (caller guarantees the prefix), try to match the concealed
/// index-term macro `indexterm:[primary, secondary, tertiary]`. Mirror of
/// [`crate::inline::InlineState::try_indexterm_macro`]: requires the `[` directly
/// after the colon, splits the content into up to three trimmed terms, and emits a
/// `ConcealedIndexTerm` (invisible in the flow). A *leaf*.
fn try_indexterm(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 10..]; // after "indexterm:"
    if !rest.starts_with('[') {
        return None;
    }
    let bracket_end = find_macro_close_bracket(rest, 0)?;
    let content = unescape_close_bracket(&rest[1..bracket_end]);
    if content.is_empty() {
        return None;
    }
    let end = start + 10 + bracket_end + 1;
    Some((concealed_index_term(&content, work)?, end))
}

/// At an `indexterm2:` (caller guarantees the prefix), try to match the flow
/// index-term macro `indexterm2:[term]`. Mirror of
/// [`crate::inline::InlineState::try_indexterm2_macro`]: the whole bracket content
/// is the rendered term. A *leaf*.
fn try_indexterm2(src: &str, start: usize, work: &Work) -> Option<(Vec<Event<'static>>, usize)> {
    let rest = &src[start + 11..]; // after "indexterm2:"
    if !rest.starts_with('[') {
        return None;
    }
    let bracket_end = find_macro_close_bracket(rest, 0)?;
    let content = restore_verbatim(work, unescape_close_bracket(&rest[1..bracket_end]))?;
    if content.is_empty() {
        return None;
    }
    let end = start + 11 + bracket_end + 1;
    let events = vec![Event::IndexTerm {
        text: Cow::Owned(content.into_owned()),
    }];
    Some((events, end))
}

/// Build a `ConcealedIndexTerm` from the raw comma-separated term list (shared by
/// the `(((…)))` shorthand and the `indexterm:[…]` macro). Up to three trimmed
/// parts: primary, optional secondary, optional tertiary.
fn concealed_index_term(inner: &str, work: &Work) -> Option<Vec<Event<'static>>> {
    // Split on the SOURCE commas (a sentinel never holds one), then restore each
    // part verbatim so a passthrough's protected comma stays within one term.
    let mut parts = inner.splitn(3, ',');
    let restore = |p: &str| restore_verbatim(work, Cow::Owned(p.trim().to_string())).map(Cow::into_owned);
    let primary = restore(parts.next().unwrap())?;
    let secondary = match parts.next() {
        Some(s) => Some(restore(s)?),
        None => None,
    };
    let tertiary = match parts.next() {
        Some(s) => Some(restore(s)?),
        None => None,
    };
    Some(vec![Event::ConcealedIndexTerm {
        primary: Cow::Owned(primary),
        secondary: secondary.map(Cow::Owned),
        tertiary: tertiary.map(Cow::Owned),
    }])
}

/// Whether an autolink scheme (`http://`/`https://`/`file://`/`ftp://`/`irc://`)
/// begins at byte `i`. Mirrors Asciidoctor's `InlineLinkRx` scheme group
/// `(?:https?|file|ftp|irc)://` — each requires the `://` (so `file:relative` is
/// not a scheme).
fn scheme_at(src: &str, i: usize) -> bool {
    let rest = &src[i..];
    rest.starts_with("http://")
        || rest.starts_with("https://")
        || rest.starts_with("file://")
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
/// non-boundary, mirroring legacy; a Unicode-whitespace boundary is rare and not
/// corpus-covered.)
fn at_autolink_boundary(bytes: &[u8], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = bytes[i - 1];
    prev.is_ascii_whitespace() || matches!(prev, b'<' | b'>' | b'(' | b')' | b'[' | b']' | b';')
}

/// Whether an autolink could open at byte offset `i` — the shared boundary test
/// for both the bare autolink ([`try_autolink`]) and its escaped form (the
/// `\https://` arm). Extends [`at_autolink_boundary`] (start / whitespace /
/// `<>()[];`) with the constrained-quote case the legacy parser reaches via
/// recursion.
///
/// An autolink also opens immediately after a constrained quote marker that
/// actually forms a span here. This is the pre-`quotes` stand-in for
/// Asciidoctor's `>`-after-`<code>` boundary: `quotes` runs after `macros`, so
/// the `<code>`/`<strong>`/… tag that would precede the URL is not materialised
/// yet — but the quote-span detectors ([`super::quotes::constrained_open_close`]
/// / [`super::quotes::simple_pair_open_close`]) report whether the span forms,
/// which is exactly when that boundary would exist (`` `http…` `` → autolink
/// inside `<code>`; `*http…*`/`_…_`/`#…#`/`^…^`/`~…~` likewise). The
/// span-formation check returns false when the marker opens no span
/// (`` word`http… ``, `a*\http…`), matching Asciidoctor and the legacy parser.
///
/// For the escaped form the caller passes the backslash offset; the backslash is
/// not a span marker, so the URL one byte further right never sees the span
/// boundary and stays literal (`` `\http…` `` → literal URL inside `<code>`).
fn autolink_open_boundary(work: &Work, bytes: &[u8], i: usize) -> bool {
    autolink_url_limit(work, bytes, i).is_some()
}

/// As [`autolink_open_boundary`], but on success also reports the exclusive byte
/// offset at which the URL scan must stop. A plain boundary (start / whitespace /
/// `<>()[];`) imposes no extra limit and returns `Some(bytes.len())`. A
/// constrained-span open returns `Some(close)` — the span's closing-marker offset
/// — because `macros` runs before `quotes`, so the URL would otherwise swallow the
/// still-literal closing marker (`` `http://x` `` must link only `http://x`, not
/// `` http://x` ``). This is the pre-`quotes` stand-in for the `<` of `</code>`
/// that bounds the URL in Asciidoctor's later `macros` pass.
fn autolink_url_limit(work: &Work, bytes: &[u8], i: usize) -> Option<usize> {
    if at_autolink_boundary(bytes, i) {
        return Some(bytes.len());
    }
    if i == 0 {
        return None;
    }
    let marker = bytes[i - 1];
    match marker {
        b'`' | b'*' | b'_' | b'#' => super::quotes::constrained_open_close(&work.tags, bytes, i - 1, marker),
        b'^' | b'~' => super::quotes::simple_pair_open_close(bytes, i - 1, marker),
        _ => None,
    }
}

/// At an autolink scheme (caller guarantees `scheme_at`), try to match a bare URL
/// autolink, optionally followed by a `[label]` attrlist. Mirror of
/// [`crate::inline::InlineState::try_autolink`]. The left-boundary test and URL
/// scan limit both come from [`autolink_url_limit`], so a URL immediately inside a
/// constrained quote span (`` `http…` ``) links exactly as it does after `<code>`
/// materialises in Asciidoctor's later `macros` pass.
///
/// The third tuple element is `strip_angle`: `true` for the angle-bracketed bare
/// form `<https://…>`, signalling the caller to drop the preceding `<` it already
/// copied (the closing `>` is consumed via the returned `end`). See the
/// `preceded_by_angle` block below.
fn try_autolink(
    work: &Work,
    src: &str,
    start: usize,
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Option<(Vec<Event<'static>>, usize, bool)> {
    let limit = autolink_url_limit(work, src.as_bytes(), start)?;
    let rest = &src[start..];
    // When `start` opens a constrained span, cap the scan at the span's closing
    // marker (`limit`): `macros` runs before `quotes`, so that marker is still a
    // literal byte the URL terminator set (whitespace / `[` `]` `<` `>`) would not
    // stop on. The cap is the pre-`quotes` stand-in for the `<` of `</code>`.
    let scan_end = (limit - start).min(rest.len());
    let url_end = rest[..scan_end]
        .find(|c: char| c.is_whitespace() || c == '[' || c == ']' || c == '<' || c == '>')
        .unwrap_or(scan_end);
    let mut url = &rest[..url_end];
    if url.len() <= 8 {
        return None;
    }
    // Trailing punctuation is stripped only from BARE urls — the `URL[text]` macro
    // form keeps it. Check for the attrlist on the UNSTRIPPED url first.
    let bracket_follows = rest[url_end..].starts_with('[') && rest[url_end..].contains(']');

    // Angle-bracketed bare URL (`<https://…>`): when the scheme is immediately
    // preceded by `<` and the URL is closed by `>` (no `[label]`), Asciidoctor
    // consumes BOTH angle brackets and links the URL bare — and, unlike the
    // unbracketed form, it KEEPS trailing punctuation (the `>` is the hard
    // boundary, so `<https://a.org/b.>` links the trailing dot). Without a closing
    // `>` (the URL runs into whitespace/EOL) Asciidoctor declines to link at all,
    // leaving the `<` and the URL literal. The `<url[text]>` macro form is NOT
    // stripped here — its brackets stay literal around the resulting link (handled
    // by the URL[text] arm below). The email autolink is unaffected (`<a@b.com>`
    // keeps its brackets), since this is the URL path only.
    let preceded_by_angle = start > 0 && src.as_bytes()[start - 1] == b'<';
    if preceded_by_angle && !bracket_follows {
        if rest.as_bytes().get(url_end) == Some(&b'>') {
            let end = start + url_end + 1; // consume the closing `>`
            let Some(target) = reconstruct_link_target(work, url) else {
                flag_punt();
                return None;
            };
            let events = build_link(
                target.clone(),
                None,
                false,
                None,
                true,
                None,
                &target,
                &work.tags,
                subs,
                options,
            );
            return Some((events, end, true));
        }
        return None;
    }

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
        && let Some(close) = find_macro_close_bracket(after_url, 0)
    {
        let end = start + url_len + close + 1;
        let content = unescape_close_bracket(&after_url[1..close]);
        // The URL part may carry an escaped-typographic `Literal` sentinel
        // (`a\...b`) or a plain `...` Asciidoctor would have curled; reconstruct it.
        let Some(target) = reconstruct_link_target(work, url) else {
            flag_punt();
            return None;
        };
        let attrs = parse_link_attrs(&content, LinkKind::Link);
        // A sentinel in a non-label attribute (role/window) still punts; one in the
        // label text is re-parsed natively (seeded), mirroring `try_link`.
        if attr_has_sentinel(attrs.window) || attr_has_sentinel(attrs.role) {
            return None;
        }
        let is_bare = attrs.text.is_empty();
        let label = (!is_bare).then_some(attrs.text);
        let events = build_link(
            target.clone(),
            attrs.window,
            attrs.nofollow,
            attrs.role,
            is_bare,
            label,
            &target,
            &work.tags,
            subs,
            options,
        );
        return Some((events, end, false));
    }

    // Bare form.
    let end = start + url_len;
    let Some(target) = reconstruct_link_target(work, url) else {
        flag_punt();
        return None;
    };
    let events = build_link(
        target.clone(),
        None,
        false,
        None,
        true,
        None,
        &target,
        &work.tags,
        subs,
        options,
    );
    Some((events, end, false))
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
    options: InlineOptions,
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
    // Asciidoctor does not mark email autolinks as bare. An email autolink has no
    // explicit label (visible text is the email), so the seed table is unused.
    let events = build_link(
        format!("mailto:{email}"),
        None,
        false,
        None,
        false,
        None,
        email,
        &[],
        subs,
        options,
    );
    Some((events, local_start, domain_end))
}

/// Build the `Start(Link) … End` event sequence shared by the link macro, mailto
/// macro, and URL/email autolinks. `label` (when `Some`) is re-parsed with
/// `MACROS` cleared via [`push_label`], mirroring `push_macro_label`; otherwise
/// `bare_text` is emitted as the visible text. The tag fields are owned `Cow`s,
/// semantically equal to the legacy parser's borrowed ones.
#[allow(clippy::too_many_arguments)]
fn build_link(
    url: String,
    window: Option<&str>,
    nofollow: bool,
    role: Option<&str>,
    is_bare: bool,
    label: Option<&str>,
    bare_text: &str,
    seed: &[TagToken],
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'static>> {
    let mut events: Vec<Event<'static>> = vec![Event::Start(Tag::Link {
        url: Cow::Owned(url),
        window: window.map(|w| Cow::Owned(w.to_string())),
        nofollow,
        is_bare,
        role: role.map(|r| Cow::Owned(r.to_string())),
    })];
    match label {
        Some(l) => push_label(l, seed, subs, options, &mut events),
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
fn push_label(
    text: &str,
    seed: &[TagToken],
    subs: SubstitutionSet,
    options: InlineOptions,
    events: &mut Vec<Event<'static>>,
) {
    if text.is_empty() {
        return;
    }
    events.extend(reparse_label(text, seed, subs, options));
}

/// Re-parse a macro label's raw text with `MACROS` cleared (mirroring
/// `push_macro_label`). When the text already carries an earlier-extracted
/// sentinel (a passthrough / escaped `Literal` / char-ref the link or
/// cross-reference label swallowed), the re-parse is *seeded* with the outer tag
/// table so the sentinel resolves against its leaf rather than being mis-read by
/// a fresh inner table — the native replacement for the old "sentinel in the
/// label → punt" guard. The common sentinel-free label takes the plain pipeline.
fn reparse_label(
    text: &str,
    seed: &[TagToken],
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'static>> {
    reparse_seeded(text, seed, subs.without(SubstitutionSet::MACROS), options)
}

/// Re-parse `text` through the engine, seeding the inner tag table with `seed`
/// (the outer table) when `text` already carries a sentinel so it resolves against
/// the outer passthrough/escape/char-ref leaf; otherwise the plain pipeline. The
/// caller chooses `subs` — [`reparse_label`] clears `MACROS`, the quoted-menu parts
/// keep them on (a nested macro renders inside a menu segment).
fn reparse_seeded(
    text: &str,
    seed: &[TagToken],
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'static>> {
    if text.as_bytes().contains(&TAG_LEAD) {
        super::run_pipeline_seeded(text, seed, subs, options)
    } else {
        super::run_pipeline(text, subs, options)
    }
}

/// Whether `src[start..end]` (a candidate macro span) contains a tag sentinel —
/// i.e. an earlier pass already lifted a passthrough/escape/char-ref out of it,
/// so the raw text the legacy parser would re-parse is gone. Every caller treats
/// `true` as a decline, so this records the punt ([`flag_punt`]) on the way out,
/// signalling [`super::try_parse`] to fall back to legacy for the paragraph.
fn span_has_sentinel(src: &str, start: usize, end: usize) -> bool {
    let has = src.as_bytes()[start..end].contains(&TAG_LEAD);
    if has {
        flag_punt();
    }
    has
}

/// Whether a macro *target* substring (a verbatim id / URL / email the tag stores
/// without re-parsing) carries a sentinel. Unlike a label — which the engine can
/// now re-parse natively against the seeded tag table — a target's lost verbatim
/// source still forces a punt, so a `true` records it ([`flag_punt`]) for
/// [`super::try_parse`] to fall back to legacy. (The link family's targets accept
/// the richer [`reconstruct_link_target`] / [`passthrough_url`] reconstruction
/// instead of this blanket decline.)
fn target_has_sentinel(target: &str) -> bool {
    let has = target.as_bytes().contains(&TAG_LEAD);
    if has {
        flag_punt();
    }
    has
}

/// Whether an optional verbatim link attribute (role / window — stored on the tag
/// without re-parsing) carries a sentinel. Records the punt on `true`, since only
/// the *label* text gains native seeded re-parsing; a sentinel that landed in a
/// non-label attribute has no reconstruction here and falls back to legacy.
fn attr_has_sentinel(attr: Option<&str>) -> bool {
    let has = attr.is_some_and(|s| s.as_bytes().contains(&TAG_LEAD));
    if has {
        flag_punt();
    }
    has
}

/// Reconstruct an inline-link target that carries an escaped-typographic
/// `Literal` sentinel, restoring the backslash-stripped literal Asciidoctor's
/// `replacements` pass leaves for `\...` / `\--` / `\(C)` / … inside a URL.
///
/// The earlier `escape` pass seals an escaped typographic pattern as a
/// [`TagToken::Literal`] whose content is the pattern WITHOUT the backslash
/// (`\...` → `Literal("...")`) — exactly what Asciidoctor's `/\\?\.\.\./`-style
/// rules produce (strip the leading `\`, keep the literal). Because this pipeline
/// extracts macros before that sentinel would otherwise force a punt, the link
/// previously fell back to legacy and kept the raw `\...`; here we splice the
/// `Literal` content back into the target so `compare/v1.5.6\...v1.5.6.1` links
/// with the byte-for-byte `compare/v1.5.6...v1.5.6.1` Asciidoctor emits.
///
/// Plain runs are copied VERBATIM. An unescaped `...` is deliberately NOT curled
/// to an ellipsis here: a URL synthesised from a resolved attribute reference is
/// re-parsed after its surrounding text has already been through `replacements`
/// once, so curling again would double-substitute (`v2.0.25\...` →
/// `v2.0.25…​`). Leaving plain runs untouched keeps those (already-substituted)
/// targets correct; the only cost is that a top-level literal URL with an
/// unescaped `...` is not curled (rare, and Asciidoctor-faithful curling would
/// regress the far more common resolved-attribute form).
///
/// Returns `None` — signalling the caller to punt to legacy, preserving the
/// previous [`span_has_sentinel`] decline — when the span carries any sentinel
/// that is NOT a `Literal`: passthrough / attribute-reference / char-ref / macro
/// targets are out of scope and still fall back.
fn reconstruct_link_target(work: &Work, span: &str) -> Option<String> {
    let bytes = span.as_bytes();
    if !bytes.contains(&TAG_LEAD) {
        // Fast path: no sentinel — the raw target is already final.
        return Some(span.to_string());
    }
    let mut out = String::with_capacity(span.len());
    let mut seg_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            let end = sentinel_end(bytes, i);
            if seg_start < i {
                out.push_str(&span[seg_start..i]);
            }
            // Parse the decimal token index between TAG_LEAD and TAG_TAIL.
            let mut idx = 0usize;
            let mut j = i + 1;
            while j < end && bytes[j].is_ascii_digit() {
                idx = idx * 10 + (bytes[j] - b'0') as usize;
                j += 1;
            }
            match work.tags.get(idx)? {
                TagToken::Literal(s) => out.push_str(s),
                _ => return None,
            }
            seg_start = end;
            i = end;
        } else {
            i += 1;
        }
    }
    if seg_start < bytes.len() {
        out.push_str(&span[seg_start..]);
    }
    Some(out)
}

/// Restore a verbatim macro content/target (image alt & target, icon name &
/// attrs, stem content, anchor id & label, index-term text, UI key/label/menu
/// item) that carries an earlier-pass sentinel back to its source text. A
/// passthrough's protected content and an escaped `Literal` are spliced in —
/// exactly what Asciidoctor's global passthrough restore leaves in the macro's
/// verbatim attribute, so `image:i.png[++a b++]` → `alt="a b"` and
/// `kbd:[++Ctrl++]` → a single `<kbd>Ctrl</kbd>`.
///
/// Returns `None` (caller punts to legacy) on any OTHER sentinel: a char
/// reference (`&#…;`), whose verbatim-versus-escaped treatment is family-specific
/// — stem html-escapes it, an `alt` keeps it literal — and which is rare enough
/// inside a verbatim macro that the legacy fallback is left to handle it; or an
/// unexpected structural token (which cannot arise at macros-time anyway). The
/// no-sentinel fast path returns the input unchanged.
fn restore_verbatim<'a>(work: &Work, s: Cow<'a, str>) -> Option<Cow<'a, str>> {
    let bytes = s.as_bytes();
    if !bytes.contains(&TAG_LEAD) {
        return Some(s); // fast path: no sentinel, already final
    }
    let mut out = String::with_capacity(s.len());
    let mut seg_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            let end = sentinel_end(bytes, i);
            if seg_start < i {
                out.push_str(&s[seg_start..i]);
            }
            let idx: usize = s[i + 1..end - 1].parse().ok()?;
            match work.tags.get(idx)? {
                TagToken::Passthrough(pieces) => {
                    for p in pieces {
                        out.push_str(&p.text);
                    }
                }
                TagToken::Literal(t) => out.push_str(t),
                // char-ref / structural → punt (family-specific or impossible here)
                _ => {
                    flag_punt();
                    return None;
                }
            }
            seg_start = end;
            i = end;
        } else {
            i += 1;
        }
    }
    if seg_start < bytes.len() {
        out.push_str(&s[seg_start..]);
    }
    Some(Cow::Owned(out))
}

#[cfg(test)]
mod tests {
    use super::{find_macro_close_bracket, unescape_close_bracket};
    use std::borrow::Cow;

    #[test]
    fn find_close_bracket_honours_escape() {
        // `[` at index 0; plain close.
        assert_eq!(find_macro_close_bracket("[abc]", 0), Some(4));
        // Escaped `\]` does not close; the next unescaped `]` does.
        assert_eq!(find_macro_close_bracket("[a\\]b]", 0), Some(5));
        // Empty content: `]` right after `[` is never treated as escaped.
        assert_eq!(find_macro_close_bracket("[]", 0), Some(1));
        // A non-zero open offset (target before the `[`).
        assert_eq!(find_macro_close_bracket("tgt[a\\]b]", 3), Some(8));
        // No unescaped close → None (trailing `\]` keeps escaping).
        assert_eq!(find_macro_close_bracket("[a\\]", 0), None);
        // Double backslash then `]`: the `]` is still escaped (Asciidoctor's
        // `[^\\]\]` looks only at the single preceding byte), so we skip it and
        // require a later unescaped `]`.
        assert_eq!(find_macro_close_bracket("[a\\\\]x]", 0), Some(6));
    }

    #[test]
    fn unescape_close_bracket_only_touches_escaped_bracket() {
        // No escape → borrowed, byte-identical.
        assert!(matches!(unescape_close_bracket("a]b? no"), Cow::Borrowed("a]b? no")));
        // `\]` → `]`; a lone `\` not before `]` is preserved.
        assert_eq!(unescape_close_bracket("a\\]b\\c"), "a]b\\c");
        // Double escape: each `\]` collapses independently.
        assert_eq!(unescape_close_bracket("x\\]y\\]z"), "x]y]z");
    }
}
