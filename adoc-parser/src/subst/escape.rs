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
//!
//! ## Handled by the quote / passthrough passes (their span-aware home):
//!
//! - **`\+` (single-plus)** — escaping a passthrough opener is folded into the
//!   passthrough pass ([`super::passthrough`]), which runs before this one: a
//!   `\+…+` whose `+…+` would form a single-plus passthrough drops the backslash
//!   there. A `\+` that forms no passthrough is left for *this* pass's blanket
//!   arm to keep literal. The `\++`/`\+++` doubled forms stay deferred.
//! - **`\pass:SPEC[…]`** — the escaped pass macro is also folded into the
//!   passthrough pass: the backslash drops and the `pass:SPEC[` prefix is kept
//!   literal while the bracketed content flows through the remaining subs (it is
//!   NOT a verbatim leaf), so it cannot be handled here as a plain literal. The
//!   `\\pass:` double-backslash form stays deferred.
//! - **quote/super/sub marker escapes `\*` `\_` `` \` `` `\#` `\^` `\~`** — these
//!   are folded into each quote substitution ([`super::quotes`]), exactly as
//!   Asciidoctor folds the `\\?` capture: a backslash is only an escape at the
//!   point a span would open, so a `\` already *inside* an open span (the content
//!   of `` `\` ``) stays literal content. They CANNOT be handled in this
//!   escape-FIRST pass, which would hide a span's closing marker and tear it
//!   apart (`a (`\`) and (`]`) b`). The doubled-marker (`\MM…MM`) form stays
//!   deferred.
//!
//! ## Deferred (backslash left untouched; the gate falls back, FORCE diverges):
//!
//! - `\\` (escaped backslash, and the `\\**`/`\\pass:` double-backslash forms),
//! - the `\https://…` autolink escape (relies on a left-boundary look-back) and
//!   the `\((…))` index-term-shorthand escape (concealed-vs-flow logic) — distinct
//!   code paths from the `\name:target[…]` macro escape handled above.

use std::borrow::Cow;

use crate::event::{Event, SubstitutionSet};

use super::char_refs::char_ref_len;
use super::tokenize::{utf8_char_len, Work, TAG_LEAD, TAG_TAIL};

/// Apply backslash escapes across the raw working buffer (run before any pass
/// that inserts sentinels).
///
/// `subs` gates the macro-escape arm on `MACROS` (mirroring the legacy
/// `inline_macro_escape_len`, which is a no-op without it).
pub(super) fn run(work: &mut Work, subs: SubstitutionSet) {
    let macros_on = subs.has(SubstitutionSet::MACROS);
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
            // `\\` — escaped backslash and the `\\**`/`\\pass:` forms are not yet
            // ported; leave BOTH backslashes literal (re-examining the second as
            // an escape introducer would mis-handle the deferred cases).
            Some(b'\\') => {
                out.push_str("\\\\");
                i += 2;
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
                    // (`\pass:` `\link:` `\&#…;` `\(( ` …).
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
