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
//!
//! ## Handled by the quote / passthrough passes (their span-aware home):
//!
//! - **`\+` (single-plus)** — escaping a passthrough opener is folded into the
//!   passthrough pass ([`super::passthrough`]), which runs before this one: a
//!   `\+…+` whose `+…+` would form a single-plus passthrough drops the backslash
//!   there. A `\+` that forms no passthrough is left for *this* pass's blanket
//!   arm to keep literal. The `\++`/`\+++` doubled forms stay deferred.
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
//! - macro escapes `\pass:SPEC[…]`, `\link:`, `\footnote:`, `\((…))`,
//!   `\https://…` — these need the not-yet-ported macros pass.

use super::char_refs::char_ref_len;
use super::tokenize::{utf8_char_len, Work};

/// Apply backslash escapes across the raw working buffer (run before any pass
/// that inserts sentinels).
pub(super) fn run(work: &mut Work) {
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
                if plen > 0 {
                    // Typographic pattern (arm: bypass `replacements`).
                    out.push_str(&work.literal_sentinel(old[i + 1..i + 1 + plen].to_string()));
                    i += 1 + plen;
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
