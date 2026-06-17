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

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Run every quote pass over `work`, in Asciidoctor `QUOTE_SUBS` order.
pub(super) fn run_all(work: &mut Work) {
    // strong
    pass_unconstrained(work, b'*', SpanKind::Strong, SpanKind::Strong);
    pass_constrained(work, b'*', SpanKind::Strong, SpanKind::Strong);
    // curved smart quotes `"`…`"` / `'`…`'` — run after strong and before
    // monospace (Asciidoctor `QUOTE_SUBS` order). Strong opening before this pass
    // is why strong is *not* suppressed at a smart-quote leading edge, while the
    // monospace/emphasis/mark passes below it are.
    pass_smart_quotes(work, b'"', "\u{201C}", "\u{201D}");
    pass_smart_quotes(work, b'\'', "\u{2018}", "\u{2019}");
    // monospace (runs before emphasis/mark)
    pass_unconstrained(work, b'`', SpanKind::Monospace, SpanKind::Monospace);
    pass_constrained(work, b'`', SpanKind::Monospace, SpanKind::Monospace);
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
fn parse_attrs(attr_content: &str) -> Option<(Option<String>, Vec<String>)> {
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
    // single marker only (a doubled marker is the unconstrained form, which the
    // earlier pass owns; legacy does not fall back from unconstrained to
    // constrained for the attrlist form)
    if bytes.get(marker_pos).copied() != Some(marker)
        || bytes.get(marker_pos + 1).copied() == Some(marker)
    {
        return None;
    }
    let content_start = marker_pos + 1;
    if content_start >= bytes.len() || bytes[content_start] == b' ' {
        return None;
    }
    let close_pos = find_valid_close_constrained(bytes, marker, content_start, false)?;
    Some((id, roles, content_start, close_pos))
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

/// Curved/smart quotes: `quote`+`` ` `` … `` ` ``+`quote` → an opening curly
/// `quote` text, the (already strong-processed) inner content, and a closing
/// curly `quote` text. Mirrors the legacy `try_smart_quotes`: there is no
/// open-boundary assertion, the close is the first `` ` ``+`quote` after the
/// (non-empty) content, and the curly characters are emitted as literal `Text`.
fn pass_smart_quotes(work: &mut Work, quote: u8, open_curly: &'static str, close_curly: &'static str) {
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

        if bytes[i] == quote && bytes.get(i + 1).copied() == Some(b'`') {
            let content_start = i + 2;
            if let Some(close_pos) = find_smart_quote_close(bytes, quote, content_start)
                && close_pos > content_start
            {
                out.push_str(&work.smart_quote_sentinel(open_curly, true));
                out.push_str(&old[content_start..close_pos]);
                out.push_str(&work.smart_quote_sentinel(close_curly, false));
                i = close_pos + 2; // skip closing `` ` `` + `quote`
                continue;
            }
        }

        let len = utf8_char_len(bytes[i]);
        out.push_str(&old[i..i + len]);
        i += len;
    }

    work.buf = out;
}

/// Find the `` ` ``+`quote` that closes a smart quote at or after `search_start`,
/// skipping sentinel regions. Mirrors `find_smart_quote_close`.
fn find_smart_quote_close(bytes: &[u8], quote: u8, search_start: usize) -> Option<usize> {
    let mut i = search_start;
    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            i = sentinel_end(bytes, i);
            continue;
        }
        if bytes[i] == b'`' && bytes.get(i + 1).copied() == Some(quote) {
            return Some(i);
        }
        i += 1;
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
