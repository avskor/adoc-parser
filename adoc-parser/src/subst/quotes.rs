//! The `quotes` substitution as a sequence of flat gsub-style passes.
//!
//! Each pass scans the current working buffer once, left to right, and splices
//! sentinel tags around every match of one quote type, then continues *after*
//! the match (non-overlapping, like Ruby `String#gsub`). Passes run in
//! Asciidoctor's `QUOTE_SUBS` order so that an earlier type (e.g. strong)
//! rewrites the string before a later type (e.g. monospace) scans it — the
//! mechanism behind cross-span overlap.
//!
//! Boundary rules are ported from the legacy recursive parser
//! (`crate::inline`), which already encodes most of Asciidoctor's open/close
//! assertions. Sentinel bytes count as non-word boundary characters (like the
//! `<`/`>` of a spliced tag in Asciidoctor), which falls out naturally because
//! they are neither alphanumeric nor `_`. The one place the engine is *more*
//! Asciidoctor-faithful than legacy is the constrained-span close search (see
//! [`find_valid_close_constrained`]): legacy stops at the first inner marker and
//! abandons the span if it cannot close, whereas Asciidoctor's lazy
//! `(\S|\S.*?\S)` absorbs that marker and keeps scanning for a later valid
//! close. The engine is now the default, so this Asciidoctor-faithful behaviour
//! is adopted directly (it is the `outline.adoc` flip).
//!
//! ## Scope
//!
//! Implemented: strong/monospace/emphasis/mark (constrained + unconstrained,
//! with `[attrlist]` prefixes), superscript/subscript, and curved smart quotes
//! `"`…`"`/`'`…`'` (the `:double`/`:single` substitutions, run between strong
//! and monospace so the leading-edge suppression of monospace/emphasis/mark
//! falls out of the pass order). Each constrained/simple-pair pass also folds in
//! Asciidoctor's `\\?` quote-marker escape (`\*`/`\_`/`` \` ``/`\#`/`\^`/`\~`):
//! the backslash is dropped only when an unescaped marker would open a span at
//! that position (so it is span-aware, leaving `` `\` `` intact), otherwise the
//! `\marker` is kept literal. The doubled-marker (`\MM…MM`) and double-backslash
//! (`\\M`) forms stay deferred. **Not** implemented here (handled by other
//! passes or left for later phases; the engine's caller falls back to the legacy
//! parser whenever the result would differ): the macros pass.

use super::tokenize::{
    desentinelize, sentinel_end, utf8_char_len, SpanKind, TagToken, Work, TAG_LEAD, TAG_TAIL,
};
use crate::inline::InlineOptions;

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Run every quote pass over `work`, in Asciidoctor `QUOTE_SUBS` order.
pub(super) fn run_all(work: &mut Work, options: InlineOptions) {
    // strong
    pass_unconstrained(work, b'*', SpanKind::Strong, SpanKind::Strong);
    pass_constrained(work, b'*', SpanKind::Strong, SpanKind::Strong);
    // curved smart quotes — run after strong and before monospace (Asciidoctor
    // `QUOTE_SUBS` order). Strong opening before this pass is why strong is *not*
    // suppressed at a smart-quote leading edge, while the monospace/emphasis/mark
    // passes below it are. Under `:compat-mode:` Asciidoctor swaps the modern
    // backtick-delimited quotes for the AsciiDoc.py forms and inserts single-quote
    // emphasis (`asciidoctor.rb:469-485` `QUOTE_SUBS[true]`): the compat slots run
    // in the order `` ``..'' `` (double) → `'..'` (emphasis) → `` `..' `` (single).
    // The modern `"`…`"`/`'`…`'` forms are *not* recognised in compat (e.g.
    // `"`page`"` renders as `"<code>page</code>"`, the backtick pair below taking
    // the inner span), so they are gated off.
    //
    // The modern forms are *constrained* in Asciidoctor (`QUOTE_SUBS[false]`
    // `:double`/`:single`: `(^|[^\w;:}])OPEN(\S|\S.*?\S)CLOSE(?!\w)`), so they route
    // through the same [`pass_compat_curved`] machinery as the compat forms — open
    // marker `"`/`'` + `` ` ``, close marker `` ` `` + `"`/`'`. Without the left
    // boundary a `'` after a word char (e.g. the inner apostrophe of `` `'a'` ``)
    // would falsely open a smart quote that swallows a following monospace span.
    if options.compat_mode {
        pass_compat_curved(work, b"``", b"''", "\u{201C}", "\u{201D}");
        pass_constrained(work, b'\'', SpanKind::Emphasis, SpanKind::Emphasis);
        pass_compat_curved(work, b"`", b"'", "\u{2018}", "\u{2019}");
    } else {
        pass_compat_curved(work, b"\"`", b"`\"", "\u{201C}", "\u{201D}");
        pass_compat_curved(work, b"'`", b"`'", "\u{2018}", "\u{2019}");
    }
    // monospace (runs before emphasis/mark). Kept active in compat too: Asciidoctor
    // extracts `` `code` `` as a literal-monospace passthrough before `QUOTE_SUBS`
    // runs, so a bare backtick pair still renders `<code>` (the literal-vs-subs
    // difference of the content is the separate compat-backtick passthrough class).
    pass_unconstrained(work, b'`', SpanKind::Monospace, SpanKind::Monospace);
    pass_constrained(work, b'`', SpanKind::Monospace, SpanKind::Monospace);
    // compat-mode plus-sign monospace: `+text+` (constrained) and `++text++`
    // (unconstrained) render as monospace, occupying the slots Asciidoctor's
    // `QUOTE_SUBS[true]` (`substitutors.rb:477-479`) puts at the monospace
    // position — after smart quotes, before emphasis. Gated on `:compat-mode:`;
    // outside compat these markers are passthroughs (handled in `passthrough`).
    if options.compat_mode {
        pass_unconstrained(work, b'+', SpanKind::Monospace, SpanKind::Monospace);
        pass_constrained(work, b'+', SpanKind::Monospace, SpanKind::Monospace);
    }
    // emphasis
    pass_unconstrained(work, b'_', SpanKind::Emphasis, SpanKind::Emphasis);
    pass_constrained(work, b'_', SpanKind::Emphasis, SpanKind::Emphasis);
    // mark / highlight: bare is Highlight, an attrlist turns it into a span
    pass_unconstrained(work, b'#', SpanKind::Highlight, SpanKind::InlineSpan);
    pass_constrained(work, b'#', SpanKind::Highlight, SpanKind::InlineSpan);
    // superscript then subscript
    pass_simple_pair(work, b'^', SpanKind::Superscript);
    pass_simple_pair(work, b'~', SpanKind::Subscript);
}

/// Parsed `[attrlist]` content: the first positional attribute as an optional
/// id and a list of roles. Returns `None` when the attrlist yields neither
/// (e.g. `[]`), matching the legacy `try_inline_attr_span` rejection.
///
/// `pub(super)` so the `passthrough` pass can reuse it for the `[x-]`
/// literal-monospace marker (`[<attrs> x-]` carries a leading role). NOTE this
/// is narrower than Asciidoctor's `parse_quoted_text_attributes`: it reads only
/// the first positional attribute (a bare role or a `.role`/`#id` shorthand),
/// not named `role=`/`id=` forms — which inline `[… x-]` roles never use in the
/// corpus.
pub(super) fn parse_attrs(attr_content: &str) -> Option<(Option<String>, Vec<String>)> {
    let first = attr_content
        .split(',')
        .next()
        .unwrap_or(attr_content)
        .trim();
    if first.is_empty() {
        return None;
    }
    let (id, roles) = if first.starts_with('.') || first.starts_with('#') {
        parse_shorthand(first)
    } else {
        (None, vec![first.to_string()])
    };
    if id.is_none() && roles.is_empty() {
        return None;
    }
    Some((id, roles))
}

/// Mirror `InlineState::parse_inline_shorthand`: `#id.role1.role2`.
fn parse_shorthand(s: &str) -> (Option<String>, Vec<String>) {
    let mut id = None;
    let mut rest = s;
    if let Some(stripped) = rest.strip_prefix('#') {
        rest = stripped;
        let end = rest.find('.').unwrap_or(rest.len());
        let id_str = &rest[..end];
        if !id_str.is_empty() {
            id = Some(id_str.to_string());
        }
        rest = &rest[end..];
    }
    let mut roles = Vec::new();
    for part in rest.split('.') {
        if !part.is_empty() {
            roles.push(part.to_string());
        }
    }
    (id, roles)
}

/// Find the index of the constrained closing `marker` at or after
/// `search_start`, skipping sentinel regions. Mirrors
/// `find_closing_constrained`: the marker must be past `search_start` (non-empty
/// content) and not be the first of a doubled marker.
fn find_closing_constrained(bytes: &[u8], marker: u8, search_start: usize) -> Option<usize> {
    let mut i = search_start;
    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            i = sentinel_end(bytes, i);
            continue;
        }
        if bytes[i] == marker && i > search_start && bytes.get(i + 1).copied() != Some(marker) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find the first *valid* constrained closing `marker` for content starting at
/// `content_start`, looping past candidate markers that fail
/// [`constrained_close_ok`]. This mirrors Asciidoctor's lazy `(\S|\S.*?\S)`
/// content capture: a marker preceded by whitespace (content would end in a
/// space) or whose trailing lookahead `(?!\p{Word}…)` fails is *not* a close —
/// the `.` in `.*?` absorbs it into the content and the regex keeps scanning for
/// a later marker. The legacy parser stops at the first marker and abandons the
/// span when that one is invalid, so this is strictly more Asciidoctor-faithful;
/// the engine being the default, this behaviour is adopted directly.
fn find_valid_close_constrained(
    bytes: &[u8],
    marker: u8,
    content_start: usize,
    mono_extra: bool,
) -> Option<usize> {
    let mut from = content_start;
    while let Some(pos) = find_closing_constrained(bytes, marker, from) {
        if constrained_close_ok(bytes, marker, content_start, pos, mono_extra) {
            return Some(pos);
        }
        // This marker cannot close the span — keep scanning past it (find the
        // next marker strictly after `pos`).
        from = pos;
    }
    None
}

/// Find the index of the unconstrained closing `marker``marker` at or after
/// `search_start`, skipping sentinel regions.
fn find_closing_unconstrained(bytes: &[u8], marker: u8, search_start: usize) -> Option<usize> {
    let mut i = search_start;
    while i + 1 < bytes.len() {
        if bytes[i] == TAG_LEAD {
            i = sentinel_end(bytes, i);
            continue;
        }
        if bytes[i] == marker && bytes[i + 1] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Detect whether a *bare* constrained span opens at the marker `bytes[i]`
/// (`== marker`), returning the closing-marker position. This is the detection
/// half of the bare-marker arm in [`pass_constrained`], factored out so the
/// escape branch can reuse it: a `\marker…` is an escaped span (drop the
/// backslash, keep the markers literal) only when an *unescaped* `marker…` would
/// have opened a span here — exactly Asciidoctor's `\\?`-capture rule, where the
/// escape is honoured only when the quote regex actually matches.
///
/// `pub(super)` so the `macros` pass can reuse it: an escaped autolink
/// (`` `\http…` ``) drops its backslash only when the preceding marker actually
/// opens a constrained span here — the pre-`quotes` equivalent of Asciidoctor's
/// `>`-after-`<code>` autolink boundary.
pub(super) fn constrained_open_close(tags: &[TagToken], bytes: &[u8], i: usize, marker: u8) -> Option<usize> {
    let open_boundary = i == 0 || !is_word(bytes[i - 1]);
    if !open_boundary
        || bytes.get(i + 1).copied() == Some(marker)
        || smart_quote_leading_edge(tags, bytes, marker, i)
    {
        return None;
    }
    let content_start = i + 1;
    if content_start >= bytes.len() || bytes[content_start] == b' ' {
        return None;
    }
    find_valid_close_constrained(bytes, marker, content_start, true)
}

/// Detect whether a superscript/subscript simple pair opens at `bytes[i]`
/// (`== marker`), returning the closing-marker position. Detection half of the
/// arm in [`pass_simple_pair`], reused by the escape branch (see
/// [`constrained_open_close`]) and by the `macros` pass's escaped-autolink arm.
pub(super) fn simple_pair_open_close(bytes: &[u8], i: usize, marker: u8) -> Option<usize> {
    let content_start = i + 1;
    let mut j = content_start;
    while j < bytes.len() {
        if bytes[j] == TAG_LEAD {
            j = sentinel_end(bytes, j);
            continue;
        }
        if bytes[j] == marker && j > content_start {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Common close-side checks for a constrained span whose content is
/// `bytes[content_start..close_pos]` and whose marker is `marker`.
fn constrained_close_ok(bytes: &[u8], marker: u8, content_start: usize, close_pos: usize, mono_extra: bool) -> bool {
    // content must not be empty (guaranteed by find_closing) nor end with a space
    if close_pos == content_start || bytes[close_pos - 1] == b' ' {
        return false;
    }
    let after_close = close_pos + 1;
    if after_close < bytes.len() && is_word(bytes[after_close]) {
        return false;
    }
    // Constrained monospace forbids `"`/`'`/backtick immediately after the close
    // (Asciidoctor's `(?![\w"'`])`). The attrlist path does not apply this extra
    // rule (mirrors the legacy `try_inline_attr_span`).
    if mono_extra && marker == b'`' && matches!(bytes.get(after_close), Some(b'"' | b'\'' | b'`')) {
        return false;
    }
    true
}

/// `marker``…``marker``marker` — unconstrained constrained-free pair, optionally
/// prefixed by `[attrlist]` (no open boundary, may appear mid-word).
fn pass_unconstrained(work: &mut Work, marker: u8, bare_kind: SpanKind, attr_kind: SpanKind) {
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

        // [attrlist]MM…MM (no open-boundary requirement when unconstrained)
        if bytes[i] == b'['
            && let Some((id, roles, content_start, close_pos)) =
                attrlist_unconstrained(&work.tags, &old, bytes, i, marker)
        {
            out.push_str(&work.open_sentinel(attr_kind, id, roles));
            out.push_str(&old[content_start..close_pos]);
            out.push_str(&work.close_sentinel(attr_kind));
            i = close_pos + 2;
            continue;
        }

        // bare MM…MM
        if bytes[i] == marker && bytes.get(i + 1).copied() == Some(marker) {
            let content_start = i + 2;
            if content_start < bytes.len()
                && let Some(close_pos) = find_closing_unconstrained(bytes, marker, content_start)
                && close_pos > content_start
            {
                out.push_str(&work.open_sentinel(bare_kind, None, Vec::new()));
                out.push_str(&old[content_start..close_pos]);
                out.push_str(&work.close_sentinel(bare_kind));
                i = close_pos + 2;
                continue;
            }
        }

        let len = utf8_char_len(bytes[i]);
        out.push_str(&old[i..i + len]);
        i += len;
    }

    work.buf = out;
}

/// `[attrlist]` immediately followed by `markermarker…markermarker`.
fn attrlist_unconstrained(
    tags: &[TagToken],
    old: &str,
    bytes: &[u8],
    lbrack: usize,
    marker: u8,
) -> Option<(Option<String>, Vec<String>, usize, usize)> {
    let rbrack = find_attr_close(bytes, lbrack)?;
    let (id, roles) = parse_attrs(&desentinelize(tags, &old[lbrack + 1..rbrack]))?;
    let marker_pos = rbrack + 1;
    if bytes.get(marker_pos).copied() != Some(marker)
        || bytes.get(marker_pos + 1).copied() != Some(marker)
    {
        return None;
    }
    let content_start = marker_pos + 2;
    if content_start >= bytes.len() {
        return None;
    }
    let close_pos = find_closing_unconstrained(bytes, marker, content_start)?;
    if close_pos == content_start {
        return None;
    }
    Some((id, roles, content_start, close_pos))
}

/// `marker…marker` — constrained pair, optionally prefixed by `[attrlist]`,
/// requiring a non-word (or start/sentinel) open boundary.
fn pass_constrained(work: &mut Work, marker: u8, bare_kind: SpanKind, attr_kind: SpanKind) {
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

        // Escaped marker `\M…` — Asciidoctor folds the `\\?` escape into this
        // quote substitution, so the backslash is honoured only when an unescaped
        // `M…` would open a span here. When it would (`\*bold*`), drop the
        // backslash and emit the markers literal with the content left raw for the
        // later passes (`\*_em_*` → `*<em>em</em>*`); when it would not
        // (`\* not bold`, `\#tag`), keep `\M` literal. A doubled backslash before
        // the marker (`\\*bold*`, … `\\\\*bold*`) consumes exactly the ONE
        // backslash adjacent to the marker (the `\\?` capture sits right before
        // it), leaving any leading backslashes as literal boundary chars
        // (`\\*bold*` → `\*bold*`, `\\\*bold*` → `\\*bold*`); each leading `\` is
        // copied verbatim by the fall-through until this arm fires on the adjacent
        // one. The escape is still honoured ONLY when an unescaped span would form
        // here (`constrained_open_close` below), so a doubled backslash inside a
        // span whose own close is invalid (`_\\*a*_` — the inner `*` cannot close
        // before the word char `_`) leaves BOTH backslashes literal, matching
        // Asciidoctor. The `\MM…MM` doubled-marker form stays deferred (excluded by
        // the `i + 2 != marker` guard).
        if bytes[i] == b'\\'
            && bytes.get(i + 1).copied() == Some(marker)
            && bytes.get(i + 2).copied() != Some(marker)
        {
            let mpos = i + 1;
            if let Some(close_pos) = constrained_open_close(&work.tags, bytes, mpos, marker) {
                out.push(marker as char);
                out.push_str(&old[mpos + 1..close_pos]);
                out.push(marker as char);
                i = close_pos + 1;
            } else {
                out.push('\\');
                out.push(marker as char);
                i += 2;
            }
            continue;
        }

        let open_boundary = i == 0 || !is_word(bytes[i - 1]);

        // [attrlist]M…M (constrained: open boundary required)
        if open_boundary
            && bytes[i] == b'['
            && let Some((id, roles, content_start, close_pos)) =
                attrlist_constrained(&work.tags, &old, bytes, i, marker)
        {
            out.push_str(&work.open_sentinel(attr_kind, id, roles));
            out.push_str(&old[content_start..close_pos]);
            out.push_str(&work.close_sentinel(attr_kind));
            i = close_pos + 1;
            continue;
        }

        // bare M…M (not the first of a doubled marker — that is the
        // unconstrained pass's job). A constrained monospace/emphasis/mark cannot
        // open at the leading edge of a smart quote (mirrors the legacy
        // `smart_quote_leading_edge`): they run after `:double`/`:single`, so the
        // byte before them is that pass's opening-quote sentinel.
        if bytes[i] == marker
            && let Some(close_pos) = constrained_open_close(&work.tags, bytes, i, marker)
        {
            out.push_str(&work.open_sentinel(bare_kind, None, Vec::new()));
            out.push_str(&old[i + 1..close_pos]);
            out.push_str(&work.close_sentinel(bare_kind));
            i = close_pos + 1;
            continue;
        }

        let len = utf8_char_len(bytes[i]);
        out.push_str(&old[i..i + len]);
        i += len;
    }

    work.buf = out;
}

/// `[attrlist]` immediately followed by a constrained `marker…marker`.
fn attrlist_constrained(
    tags: &[TagToken],
    old: &str,
    bytes: &[u8],
    lbrack: usize,
    marker: u8,
) -> Option<(Option<String>, Vec<String>, usize, usize)> {
    let rbrack = find_attr_close(bytes, lbrack)?;
    let (id, roles) = parse_attrs(&desentinelize(tags, &old[lbrack + 1..rbrack]))?;
    let marker_pos = rbrack + 1;
    // The char after `]` must be the marker. A *doubled* marker (`[.path]__x_`) is
    // NOT rejected here: Asciidoctor runs the unconstrained pass (`__…__`) over the
    // whole string before the constrained one, so any `[attr]__…__` that could be
    // unconstrained has already been consumed — only forms whose unconstrained
    // close (`__`) is missing survive to here, and Asciidoctor's constrained
    // `_(\S|\S.*?\S)_` then matches them with the leading second marker folded into
    // the content (`[.path]__config_` → `<em class="path">_config</em>`). Mirroring
    // that fall-through requires no doubled-marker guard; the single opening marker
    // is consumed and the rest (incl. the second marker) becomes content.
    if bytes.get(marker_pos).copied() != Some(marker) {
        return None;
    }
    let content_start = marker_pos + 1;
    if content_start >= bytes.len() || bytes[content_start] == b' ' {
        return None;
    }
    let close_pos = find_valid_close_constrained(bytes, marker, content_start, false)?;
    Some((id, roles, content_start, close_pos))
}

/// At an inner `[` (byte index `lbrack`, `bytes[lbrack] == b'['`) inside a
/// link-family macro label, detect whether an *attributed* inline span opens
/// there — an `[attrlist]` immediately followed by a constrained or unconstrained
/// `*`/`` ` ``/`_`/`#` quoted run — and, if so, return the byte index just past the
/// span's closing marker(s).
///
/// Asciidoctor runs `quotes` before `macros`, so such a span is rewritten to a
/// `<span>`/`<strong>`/… (its brackets consumed) before the inline-link regex
/// scans for the label's closing `]`. We run `macros` first, so
/// [`super::macros::find_link_label_close`] calls this to skip past the whole span
/// and keep scanning for the real `]`. The marker set and gating mirror
/// [`pass_unconstrained`]/[`pass_constrained`]: an unconstrained `[attr]MM…MM`
/// needs no open boundary, while a constrained `[attr]M…M` requires the byte
/// before `[` to be a non-word boundary (superscript `^`/subscript `~` take no
/// attrlist, so they are not tried). `tags` resolves any extracted sentinel in
/// the attrlist.
pub(super) fn attributed_span_end(
    tags: &[TagToken],
    src: &str,
    bytes: &[u8],
    lbrack: usize,
) -> Option<usize> {
    // Unconstrained `[attrlist]MM…MM` — no open-boundary requirement. The close is
    // the first of the doubled markers, so the span ends two bytes past it.
    for &marker in b"*`_#" {
        if let Some((_, _, _, close_pos)) = attrlist_unconstrained(tags, src, bytes, lbrack, marker)
        {
            return Some(close_pos + 2);
        }
    }
    // Constrained `[attrlist]M…M` — requires an open boundary (mirrors the gate in
    // `pass_constrained`). The close is a single marker, so the span ends one byte
    // past it.
    let open_boundary = lbrack == 0 || !is_word(bytes[lbrack - 1]);
    if open_boundary {
        for &marker in b"*`_#" {
            if let Some((_, _, _, close_pos)) =
                attrlist_constrained(tags, src, bytes, lbrack, marker)
            {
                return Some(close_pos + 1);
            }
        }
    }
    None
}

/// Find the `]` that closes an attrlist opened at `lbrack` (`[`), refusing to
/// cross a newline (mirrors `try_inline_attr_span`).
fn find_attr_close(bytes: &[u8], lbrack: usize) -> Option<usize> {
    let mut j = lbrack + 1;
    while j < bytes.len() {
        match bytes[j] {
            b']' => return Some(j),
            b'\n' => return None,
            _ => j += 1,
        }
    }
    None
}

/// Constrained curved quote pass for both the modern backtick-delimited smart
/// quotes (`` "`…`" ``/`` '`…`' ``) and the AsciiDoc.py compat forms
/// (`` ``…'' ``/`` `…' ``), whose open and close markers differ and may be one or
/// two bytes. Constrained (Asciidoctor `QUOTE_SUBS` `:double`/`:single`,
/// `(^|[^\w;:}])OPEN(\S|\S#{CC_ALL}*?\S)CLOSE(?!\w)`): the open marker must sit at
/// a left boundary, the byte after it must be non-space, the content character
/// before the close must be non-space, and the close must be followed by a
/// non-word character. The lazy inner group means the *first* boundary-valid
/// close wins, so inner apostrophes/markers are absorbed. The curly replacements
/// are emitted as `SmartQuote` sentinels (literal `Text` once tokenized).
fn pass_compat_curved(
    work: &mut Work,
    open: &[u8],
    close: &[u8],
    open_curly: &'static str,
    close_curly: &'static str,
) {
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

        if bytes[i..].starts_with(open)
            && compat_open_boundary(bytes, i)
            && bytes
                .get(i + open.len())
                .is_some_and(|&b| !b.is_ascii_whitespace())
            && let Some(close_pos) =
                find_compat_curved_close(bytes, i + open.len(), close)
        {
            let content_start = i + open.len();
            out.push_str(&work.smart_quote_sentinel(open_curly, true));
            out.push_str(&old[content_start..close_pos]);
            out.push_str(&work.smart_quote_sentinel(close_curly, false));
            i = close_pos + close.len();
            continue;
        }

        let len = utf8_char_len(bytes[i]);
        out.push_str(&old[i..i + len]);
        i += len;
    }

    work.buf = out;
}

/// Left boundary for a constrained compat curved quote: start of buffer, or the
/// preceding byte is neither a word char nor one of `;`/`:`/`}` (Asciidoctor
/// `(^|[^\w;:}])`). Sentinel bytes are non-word and so count as a boundary.
fn compat_open_boundary(bytes: &[u8], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let p = bytes[i - 1];
    !is_word(p) && p != b';' && p != b':' && p != b'}'
}

/// Find the close marker ending a compat curved quote at or after `content_start`,
/// skipping sentinel regions. The content before the close must be non-empty and
/// not end in a space, and the close must not be followed by a word character
/// (Asciidoctor's lazy `(\S|\S.*?\S)CLOSE(?!\w)`).
fn find_compat_curved_close(bytes: &[u8], content_start: usize, close: &[u8]) -> Option<usize> {
    let mut i = content_start;
    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            i = sentinel_end(bytes, i);
            continue;
        }
        if i > content_start && bytes[i..].starts_with(close) {
            let prev = bytes[i - 1];
            let after = bytes.get(i + close.len()).copied();
            if !prev.is_ascii_whitespace() && after.is_none_or(|b| !is_word(b)) {
                return Some(i);
            }
        }
        i += utf8_char_len(bytes[i]);
    }
    None
}

/// True when a constrained monospace/emphasis/mark marker at position `i` is
/// immediately preceded by a smart-quote *opening* sentinel and so must not open
/// (the legacy `smart_quote_leading_edge`). Strong (`*`) ran before the smart
/// quote pass and is never affected; superscript/subscript have no open
/// assertion and use a different path.
fn smart_quote_leading_edge(tags: &[TagToken], bytes: &[u8], marker: u8, i: usize) -> bool {
    if !matches!(marker, b'`' | b'_' | b'#') {
        return false;
    }
    match sentinel_index_before(bytes, i) {
        Some(idx) => matches!(tags.get(idx), Some(TagToken::SmartQuote { opening: true, .. })),
        None => false,
    }
}

/// If buffer position `i` is immediately preceded by a complete sentinel
/// (`TAG_LEAD <decimal> TAG_TAIL`), return the sentinel's tag index. Returns
/// `None` for any malformed sequence (never produced by this engine).
fn sentinel_index_before(bytes: &[u8], i: usize) -> Option<usize> {
    if i == 0 || bytes[i - 1] != TAG_TAIL {
        return None;
    }
    let tail = i - 1;
    let mut j = tail;
    loop {
        if j == 0 {
            return None;
        }
        j -= 1;
        match bytes[j] {
            TAG_LEAD => {
                let digits = &bytes[j + 1..tail];
                if digits.is_empty() {
                    return None;
                }
                let mut idx = 0usize;
                for &d in digits {
                    if !d.is_ascii_digit() {
                        return None;
                    }
                    idx = idx * 10 + (d - b'0') as usize;
                }
                return Some(idx);
            }
            d if d.is_ascii_digit() => {}
            _ => return None,
        }
    }
}

/// `marker…marker` superscript/subscript: no boundary assertion, content must be
/// non-empty (mirrors `try_simple_pair`). No attrlist support.
fn pass_simple_pair(work: &mut Work, marker: u8, kind: SpanKind) {
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

        // Escaped marker `\^…` / `\~…` — honour the escape only when an unescaped
        // pair would form here (see the constrained-pass escape branch). A doubled
        // backslash (`\\^sup^`) consumes only the one backslash adjacent to the
        // marker, leaving leading backslashes literal — same cascade as the
        // constrained pass.
        if bytes[i] == b'\\'
            && bytes.get(i + 1).copied() == Some(marker)
            && bytes.get(i + 2).copied() != Some(marker)
        {
            let mpos = i + 1;
            if let Some(close_pos) = simple_pair_open_close(bytes, mpos, marker) {
                out.push(marker as char);
                out.push_str(&old[mpos + 1..close_pos]);
                out.push(marker as char);
                i = close_pos + 1;
            } else {
                out.push('\\');
                out.push(marker as char);
                i += 2;
            }
            continue;
        }

        if bytes[i] == marker
            && let Some(close_pos) = simple_pair_open_close(bytes, i, marker)
        {
            out.push_str(&work.open_sentinel(kind, None, Vec::new()));
            out.push_str(&old[i + 1..close_pos]);
            out.push_str(&work.close_sentinel(kind));
            i = close_pos + 1;
            continue;
        }

        let len = utf8_char_len(bytes[i]);
        out.push_str(&old[i..i + len]);
        i += len;
    }

    work.buf = out;
}
