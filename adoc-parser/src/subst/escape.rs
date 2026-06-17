//! The escape (`\`) pass (Asciidoctor's per-substitution `\\?` capture).
//!
//! Runs AFTER passthrough extraction (so a backslash inside `+…+`/`pass:[…]` is
//! already sealed in an opaque sentinel and never reaches this pass — it is
//! verbatim passthrough content, not an escape) and BEFORE the attribute/quote
//! passes, so an escaped trigger is neutralised before the pass that would
//! otherwise consume it. Asciidoctor has no standalone escape substitution —
//! each substitution's regex captures an optional leading `\` and strips it when
//! the construct matches. The legacy recursive parser instead collects the cases
//! into [`crate::inline::InlineState::handle_inline_escape`]; this pass mirrors
//! that collected behaviour for the (non-marker) constructs it can handle safely
//! in a flat scan.
//!
//! Every recognised escape drops the backslash and replaces the protected
//! character(s) with a [`TagToken::Literal`](super::tokenize::TagToken::Literal)
//! sentinel. A `Literal` is opaque to every later pass (its bytes are non-word
//! control bytes) AND coalesces with the surrounding text at tokenize time, so
//! `\{name}` → one `Text("{name}")` event, exactly as the legacy parser emits it
//! (drop backslash, escaped char merges into the next text flush).
//!
//! ## Handled (drop backslash, literal char):
//!
//! - **typographic** `\--` `\->` `\=>` `\<-` `\<=` `\...` `\(C)` `\(R)` `\(TM)`
//!   — the pattern is kept literal, bypassing the `replacements` pass.
//! - **`\"`** / **`\'`** smart-quote openers — the `"`/`'` plus its backtick are
//!   kept literal, before the `:double`/`:single` quote passes.
//! - **`\{`/`\[`/`\<`/`\'`** — the attribute-ref / bracket / `<` / apostrophe is
//!   kept literal. These are safe because none of them is a *closing* span
//!   marker: hiding one inside a `Literal` cannot tear an enclosing span apart.
//! - **`\&#…;`** — an escaped character reference (`\&#174;`, `\&copy;`): the
//!   backslash drops and the reference becomes a `Text` event (escaped `&`),
//!   restored as a [`CharRef`](super::tokenize::TagToken::CharRef) leaf with
//!   `raw = false`. Sealing it here also stops [`super::char_refs`] from treating
//!   it as a *surviving* (passthrough) reference.
//! - **`\name:target[…]`** — an escaped inline macro (`\link:u[t]`,
//!   `\indexterm2:[term]`, …, the names from
//!   [`crate::inline::InlineState::inline_macro_escape_len`]): the backslash drops
//!   and the whole macro form stays literal, sealed as a
//!   [`Macro`](super::tokenize::TagToken::Macro) leaf (one standalone `Text`
//!   event, NOT a coalescing `Literal` — the legacy parser emits the escaped
//!   macro as its own event). Gated on `MACROS`. Sealing it here stops the later
//!   [`super::macros`] pass from firing on the form. The block-macro `\image::…`
//!   and any form whose target/content already holds a sentinel are declined.
//! - **`\((…))`** — an escaped index-term shorthand (gated on `MACROS`): the
//!   backslash drops and the whole `((…))` match stays literal (a `Macro` leaf —
//!   its own `Text` event, not coalesced), or — for `\(((…)))` — literal parens
//!   around a FLOW index term of the inner text. Sealing it here stops the later
//!   [`super::macros`] pass from reading the `((…))` as an index term. Declined
//!   when the content already holds a sentinel.
//! - **`\\MM…MM`** — a doubled backslash before an unconstrained marker pair
//!   (`**`/`__`/`##`/<double backtick>, gated on `QUOTES`): both backslashes drop;
//!   the open and close marker pairs are sealed as their own `Text` events (`Macro`
//!   leaves) while the content between them is left in the buffer to flow through
//!   the remaining passes (`\\__func__` → `__func__`, `\\__a*b*c__` →
//!   `__a<strong>b</strong>c__`). Mirrors Asciidoctor's cascading gsub (the
//!   unconstrained pass strips one backslash, the constrained pass the other).
//!
//! ## Handled by the quote / passthrough passes (their span-aware home):
//!
//! - **`\+` (single-plus)** — escaping a passthrough opener is folded into the
//!   passthrough pass ([`super::passthrough`]), which runs before this one: a
//!   `\+…+` whose `+…+` would form a single-plus passthrough drops the backslash
//!   there. A `\+` that forms no passthrough is left for *this* pass's blanket
//!   arm to keep literal. A doubled (or longer) backslash run before a single-plus
//!   (`\\+plus+` → `\+plus+`) drops only the marker-adjacent backslash there too.
//!   The `\++`/`\+++` doubled-marker forms stay deferred.
//! - **`\pass:SPEC[…]`** — the escaped pass macro is also folded into the
//!   passthrough pass: the backslash drops and the `pass:SPEC[` prefix is kept
//!   literal while the bracketed content flows through the remaining subs (it is
//!   NOT a verbatim leaf), so it cannot be handled here as a plain literal. A
//!   doubled (or longer) backslash run (`\\pass:` → `\pass:`) likewise drops only
//!   the macro-adjacent backslash there.
//! - **quote/super/sub marker escapes `\*` `\_` `` \` `` `\#` `\^` `\~`** — these
//!   are folded into each quote substitution ([`super::quotes`]), exactly as
//!   Asciidoctor folds the `\\?` capture: a backslash is only an escape at the
//!   point a span would open, so a `\` already *inside* an open span (the content
//!   of `` `\` ``) stays literal content. They CANNOT be handled in this
//!   escape-FIRST pass, which would hide a span's closing marker and tear it
//!   apart (`a (`\`) and (`]`) b`). A doubled (or longer) backslash run before a
//!   single marker (`\\*bold*` → `\*bold*`) is folded there too — only the
//!   marker-adjacent backslash is consumed, and only when a span would form. The
//!   doubled-MARKER (`\MM…MM`, e.g. `\**`) form stays deferred.
//! - **`\https://…` (`http`/`https`/`ftp`/`irc`) autolink escape** — folded into
//!   the [`super::macros`] pass (the autolink's home): the backslash drops only
//!   where an unescaped autolink could open (a real boundary, or immediately
//!   inside a constrained quote span). It needs the left-boundary look-back AND,
//!   for the `` `\http…` `` form, the span-formation check that only the macros
//!   pass has the context for. Handling it in this escape-FIRST pass would drop
//!   the backslash without that context.
//!
//! ## Double-backslash (`\\X`) — Asciidoctor's `\\?`/`\\{0,2}` capture
//!
//! A doubled backslash consumes exactly the ONE backslash adjacent to the
//! construct, leaving any leading backslashes literal. Handled across the passes:
//! the constrained markers (`\\*bold*`), super/sub, `\\pass:`, and `\\+` in their
//! span-aware passes (see above); the macro / char-ref / index forms (`\\image:…`,
//! `\\&#…;`, `\\((…))`) here — this arm emits the first backslash and re-enters on
//! the second, which then runs the single-escape logic; the unconstrained marker
//! pair (`\\**…**`) via [`doubled_marker_escape`]; and the double-plus passthrough
//! (`\\++…++`) in the passthrough pass. A bare `\\` (no following construct, e.g.
//! `\\ ` / `\\x`) correctly keeps BOTH backslashes — Asciidoctor does not collapse
//! it.
//!
//! ## Deferred (FORCE still diverges from Asciidoctor — both pathological there):
//!
//! - `\\link:URL[text]` with an autolink-able URL target: Asciidoctor still
//!   renders it as a link (a file/anchor target like `\\link:foo.html[t]` is kept
//!   literal as expected — only the URL form is special); we keep it literal.
//! - `\\+++…+++` (triple-plus): Asciidoctor's own close-marker handling of the
//!   escaped triple-plus is inconsistent, so only the double-plus form is ported.

use std::borrow::Cow;

use crate::event::{Event, SubstitutionSet};

use super::char_refs::char_ref_len;
use super::tokenize::{sentinel_end, utf8_char_len, Work, TAG_LEAD, TAG_TAIL};

/// Apply backslash escapes across the raw working buffer (run before any pass
/// that inserts sentinels).
///
/// `subs` gates the macro-escape arm on `MACROS` (mirroring the legacy
/// `inline_macro_escape_len`, which is a no-op without it).
pub(super) fn run(work: &mut Work, subs: SubstitutionSet) {
    let macros_on = subs.has(SubstitutionSet::MACROS);
    let quotes_on = subs.has(SubstitutionSet::QUOTES);
    let old = std::mem::take(&mut work.buf);
    let bytes = old.as_bytes();
    let mut out = String::with_capacity(old.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'\\' {
            let len = utf8_char_len(bytes[i]);
            out.push_str(&old[i..i + len]);
            i += len;
            continue;
        }

        // `bytes[i] == b'\\'`. Decide which escape (if any) applies.
        match bytes.get(i + 1).copied() {
            // Trailing backslash: literal.
            None => {
                out.push('\\');
                i += 1;
            }
            // `\\MM…MM` — a doubled backslash before an unconstrained marker pair
            // (`**`/`__`/`##`/<double backtick>): both backslashes drop, the open
            // and close marker pairs stay literal (each its own `Text` event, like
            // the legacy parser) while the content between them flows through the
            // remaining passes. Mirrors Asciidoctor's cascading gsub: the
            // unconstrained pass strips one backslash, the constrained pass the
            // other (`\\__func__` → `__func__`, `\\__a*b*c__` →
            // `__a<strong>b</strong>c__`). Gated on QUOTES, like the legacy arm.
            // Any OTHER `\\` form falls to the else-branch below, which emits one
            // backslash and re-enters on the second (see there).
            Some(b'\\') => {
                if let Some((consumed, open, inner, close)) =
                    doubled_marker_escape(&old, bytes, i, quotes_on)
                {
                    out.push_str(&work.macro_sentinel(vec![Event::Text(Cow::Owned(open.to_string()))]));
                    out.push_str(inner); // raw inner content flows through later passes
                    out.push_str(&work.macro_sentinel(vec![Event::Text(Cow::Owned(close.to_string()))]));
                    i += consumed;
                } else {
                    // Not an unconstrained marker pair: emit the FIRST backslash
                    // literally and advance ONE byte, re-entering the match on the
                    // second backslash so it runs the single-escape logic below.
                    // That consumes exactly the one backslash adjacent to a
                    // macro/char-ref/index construct (`\\image:…` → `\image:…`,
                    // `\\&#…;` → `\&#…;`, `\\((…))` → `\((…))`), mirroring
                    // Asciidoctor's `\\?` capture. For a quote marker the second
                    // backslash falls to the blanket arm and stays literal (the
                    // marker is then resolved by the later quotes pass), and a bare
                    // `\\` (no following construct) keeps BOTH backslashes — exactly
                    // Asciidoctor, which does not collapse a bare double backslash.
                    out.push('\\');
                    i += 1;
                }
            }
            Some(m) => {
                let plen = typographic_escape_len(bytes, i);
                let cref = if m == b'&' { char_ref_len(bytes, i + 1) } else { 0 };
                let mlen = if macros_on { macro_escape_len(bytes, i + 1) } else { 0 };
                if plen > 0 {
                    // Typographic pattern (arm: bypass `replacements`).
                    out.push_str(&work.literal_sentinel(old[i + 1..i + 1 + plen].to_string()));
                    i += 1 + plen;
                } else if mlen > 0 {
                    // `\name:target[…]` — an escaped inline macro (`\link:u[t]`,
                    // `\indexterm2:[term]`, …): drop the backslash and keep the WHOLE
                    // macro form literal, so the not-yet-run `macros` pass never fires
                    // on it. Mirrors the legacy `inline_macro_escape_len` arm.
                    //
                    // Stored as a `Macro` leaf (one standalone `Text` event) rather
                    // than a coalescing `Literal`: the legacy parser pushes the
                    // escaped macro as its OWN `Text` event, so it does NOT merge with
                    // the following run (`\link:u[t] more` → two `Text`s). A
                    // `Literal` would coalesce and diverge. `macro_escape_len` already
                    // declined any form whose target/content was contaminated by an
                    // earlier sentinel (so the leaf text is the verbatim source).
                    let macro_text = old[i + 1..i + 1 + mlen].to_string();
                    out.push_str(&work.macro_sentinel(vec![Event::Text(Cow::Owned(macro_text))]));
                    i += 1 + mlen;
                } else if cref > 0 {
                    // `\&#174;` / `\&copy;` — escaped character reference: drop the
                    // backslash and keep the reference as a literal `Text` event
                    // (renderer escapes its `&` to `&amp;`), mirroring the legacy
                    // `char_ref_len_at` escape arm. Emitting it as a `CharRef`
                    // leaf with `raw = false` reproduces both the escaping AND the
                    // separate `Text` event the legacy parser pushes for the
                    // reference (it is NOT coalesced, unlike a `Literal`). Sealing
                    // it here also keeps the bare-reference survival pass from
                    // re-extracting this `&` as an `InlinePassthrough`.
                    out.push_str(&work.char_ref_sentinel(old[i + 1..i + 1 + cref].to_string(), false));
                    i += 1 + cref;
                } else if (m == b'"' || m == b'\'') && bytes.get(i + 2).copied() == Some(b'`') {
                    // `\"`` `` / `\'`` `` — smart-quote opener: quote + backtick literal.
                    out.push_str(&work.literal_sentinel(old[i + 1..i + 3].to_string()));
                    i += 3;
                } else if let Some((consumed, evs)) = index_escape(&old, bytes, i, macros_on) {
                    // `\((…))` — escaped index-term shorthand: drop the backslash
                    // and keep the whole `((…))` match literal (a non-concealed
                    // term), or — for `\(((…)))` — literal parens around a FLOW
                    // index term of the inner text (Asciidoctor "escape concealed
                    // index term, but process nested flow index term"). Sealed as a
                    // `Macro` leaf so it is its OWN event (the legacy parser pushes
                    // the escaped match as a separate `Text`, not coalesced) and the
                    // later `macros` pass never fires on the `((…))`.
                    out.push_str(&work.macro_sentinel(evs));
                    i += consumed;
                } else if matches!(m, b'{' | b'[' | b'<' | b'\'') {
                    // Generic single-character escapes for NON-marker characters
                    // (attribute ref / bracket / `<` / apostrophe): drop the
                    // backslash, keep the character literal. These never serve as a
                    // span's closing marker, so hiding them in a `Literal` cannot
                    // break an enclosing span — unlike the quote markers below.
                    out.push_str(&work.literal_sentinel((m as char).to_string()));
                    i += 2;
                } else {
                    // Everything else keeps the backslash literal. This includes
                    // the quote/super/sub markers `\*` `\_` `` \` `` `\#` `\^` `\~`:
                    // they CANNOT be escaped in a standalone scan, because a `\`
                    // sitting inside an already-open span (e.g. the content of
                    // `` `\` ``) is content, not an escape — yet a flat scan would
                    // hide the span's own closing marker and tear the span apart.
                    // Their `\\?` capture belongs inside the quote passes
                    // (Asciidoctor's true model); deferred to a later session. Also
                    // covers `\+` (deferred to the passthrough pass), `\x`, `\"`
                    // (no backtick), and the deferred macro/char-ref forms
                    // (`\pass:` `\link:` `\&#…;` …).
                    out.push('\\');
                    i += 1;
                }
            }
        }
    }

    work.buf = out;
}

/// Length of a typographic pattern following the backslash at `backslash`, or 0
/// if none. Port of [`crate::inline::InlineState::typographic_escape_len`]:
/// `\--` is an escape only where an unescaped `--` would be replaced (spaced or
/// word-flanked); there is no `---` rule.
fn typographic_escape_len(bytes: &[u8], backslash: usize) -> usize {
    let p = backslash + 1; // position after the backslash
    if p >= bytes.len() {
        return 0;
    }
    match bytes[p] {
        b'-' if p + 1 < bytes.len() && bytes[p + 1] == b'-' => {
            let after = bytes.get(p + 2).copied();
            let spaced_ok = matches!(after, None | Some(b' ') | Some(b'\n'));
            let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
            let word_ok = backslash > 0
                && is_word(bytes[backslash - 1])
                && matches!(after, Some(b) if is_word(b));
            if spaced_ok || word_ok { 2 } else { 0 }
        }
        b'-' if p + 1 < bytes.len() && bytes[p + 1] == b'>' => 2, // \->
        b'=' if p + 1 < bytes.len() && bytes[p + 1] == b'>' => 2, // \=>
        b'<' if p + 1 < bytes.len() && (bytes[p + 1] == b'-' || bytes[p + 1] == b'=') => 2, // \<- \<=
        b'.' if p + 2 < bytes.len() && bytes[p + 1] == b'.' && bytes[p + 2] == b'.' => 3,
        b'(' if p + 2 < bytes.len()
            && bytes[p + 2] == b')'
            && (bytes[p + 1] == b'C' || bytes[p + 1] == b'R') =>
        {
            3
        }
        b'(' if p + 3 < bytes.len()
            && bytes[p + 1] == b'T'
            && bytes[p + 2] == b'M'
            && bytes[p + 3] == b')' =>
        {
            4
        }
        _ => 0,
    }
}

/// Length of an escaped inline-macro form `name:target[…]` beginning at `p` (the
/// byte right after the backslash), or 0 if none. Port of
/// [`crate::inline::InlineState::inline_macro_escape_len`]: a recognised name
/// (`link:`/`xref:`/`image:`/… — each uniquely delimited by its colon), a
/// non-whitespace target up to an opening `[`, and a closing `]` somewhere after
/// it; the returned length spans `p` through that `]` inclusive.
///
/// The block-macro form `name::` is rejected (so `\image::play.png[]` is not
/// treated as an escaped inline image — it stays a literal backslash). Unlike the
/// legacy scan over raw input, this runs after passthrough/escape/char-ref
/// extraction, so the target or bracketed content may already hold a sentinel; a
/// sentinel byte inside either run means the would-be literal text no longer
/// matches the source the legacy parser kept verbatim, so the match is declined
/// (returns 0) and the gate falls back to legacy.
fn macro_escape_len(bytes: &[u8], p: usize) -> usize {
    if p >= bytes.len() {
        return 0;
    }
    // Longest-first is not required (each name is uniquely delimited by its
    // colon), but keep indexterm2 before indexterm for clarity.
    const NAMES: [&[u8]; 12] = [
        b"stem:", b"latexmath:", b"asciimath:", b"link:", b"xref:", b"mailto:",
        b"icon:", b"indexterm2:", b"indexterm:", b"footnote:", b"image:", b"anchor:",
    ];
    let rest = &bytes[p..];
    let Some(name_len) = NAMES.iter().find_map(|n| rest.starts_with(n).then_some(n.len())) else {
        return 0;
    };
    // Reject the block-macro form `name::` (e.g. image::target[]).
    if rest.get(name_len) == Some(&b':') {
        return 0;
    }
    // Target: a run of non-whitespace characters up to the opening bracket. A
    // sentinel byte here means an earlier pass lifted part of the target into a
    // leaf — decline (the legacy parser saw verbatim source).
    let mut i = name_len;
    while let Some(&c) = rest.get(i) {
        if matches!(c, b'[' | b' ' | b'\t' | b'\n') {
            break;
        }
        if c == TAG_LEAD || c == TAG_TAIL {
            return 0;
        }
        i += 1;
    }
    // Require an opening bracket immediately, then a closing bracket after it.
    if rest.get(i) != Some(&b'[') {
        return 0;
    }
    i += 1; // past '['
    while let Some(&c) = rest.get(i) {
        if c == b']' {
            return i + 1; // length from p to the closing ']' inclusive
        }
        if c == TAG_LEAD || c == TAG_TAIL {
            return 0;
        }
        i += 1;
    }
    0
}

/// `\((…))` index-term-shorthand escape (gated on MACROS). Port of the legacy
/// [`crate::inline::InlineState::handle_inline_escape`] index arm: the backslash
/// drops and the whole `((…))` match stays literal (a non-concealed term), while
/// `\(((…)))` keeps the outer parens literal around a FLOW index term of the
/// inner text. Returns the byte length consumed from the backslash at `i` and the
/// events to seal as a `Macro` leaf, or `None` when no `((` opener / closing `))`
/// is present (the backslash then stays literal). Declines when the matched
/// content already holds a sentinel (an earlier pass lifted part of it into a
/// leaf — the legacy parser saw verbatim source), so the gate falls back.
fn index_escape(old: &str, bytes: &[u8], i: usize, macros_on: bool) -> Option<(usize, Vec<Event<'static>>)> {
    if !macros_on || bytes.get(i + 1) != Some(&b'(') || bytes.get(i + 2) != Some(&b'(') {
        return None;
    }
    let content_start = i + 3;
    let close = index_term_close(&old[content_start..])?;
    let content = &old[content_start..content_start + close];
    if content.bytes().any(|b| b == TAG_LEAD || b == TAG_TAIL) {
        return None;
    }
    let events = if content.starts_with('(') && content.ends_with(')') {
        // `\(((…)))` — escaped concealed term: literal parens around a flow term.
        vec![
            Event::Text(Cow::Owned("(".to_string())),
            Event::IndexTerm { text: Cow::Owned(content[1..content.len() - 1].to_string()) },
            Event::Text(Cow::Owned(")".to_string())),
        ]
    } else {
        // The whole match minus the backslash, kept literal (`((…))`).
        vec![Event::Text(Cow::Owned(old[i + 1..content_start + close + 2].to_string()))]
    };
    Some((content_start + close + 2 - i, events))
}

/// Offset of the closing `))` (its first byte) within `rest`, greedily absorbing
/// any further trailing `)`, or `None` for an empty/absent match. Port of
/// [`crate::inline::InlineState::index_term_close`].
fn index_term_close(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let mut close = rest.find("))")?;
    while bytes.get(close + 2) == Some(&b')') {
        close += 1;
    }
    if close == 0 { None } else { Some(close) }
}

/// `\\MM…MM` doubled-marker escape (unconstrained `**`/`__`/`##`/<double
/// backtick>, gated on QUOTES). Port of the legacy `\\` unconstrained arm: both
/// backslashes drop, the open/close marker pairs stay literal while the content
/// flows. Returns the byte length consumed from the first backslash at `i`, plus
/// borrowed slices of the open marker, the inner content, and the close marker;
/// `None` when no closing pair forms (the `\\` then stays literal).
fn doubled_marker_escape<'b>(
    old: &'b str,
    bytes: &[u8],
    i: usize,
    quotes_on: bool,
) -> Option<(usize, &'b str, &'b str, &'b str)> {
    if !quotes_on {
        return None;
    }
    let marker = *bytes.get(i + 2)?;
    if !matches!(marker, b'*' | b'_' | b'#' | b'`') || bytes.get(i + 3) != Some(&marker) {
        return None;
    }
    let content_start = i + 4;
    let close_pos = find_closing_unconstrained(bytes, marker, content_start)?;
    if close_pos <= content_start {
        return None; // empty content — no span forms (mirrors the legacy guard)
    }
    Some((
        close_pos + 2 - i,
        &old[i + 2..content_start],     // open "MM"
        &old[content_start..close_pos], // inner content (flows through later passes)
        &old[close_pos..close_pos + 2], // close "MM"
    ))
}

/// Offset of the next unconstrained `MM` marker pair at or after `search_start`,
/// or `None`. Port of [`crate::inline::InlineState::find_closing_unconstrained`]
/// for the escape buffer: passthroughs are already sealed in sentinels here (and
/// hold no marker byte), so the scan only steps over a sentinel rather than
/// re-detecting `+…+`/`pass:[…]` spans.
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
