//! The `post_replacements` substitution pass (hard line breaks).
//!
//! Asciidoctor runs `post_replacements` last, over the whole rendered string,
//! turning a trailing ` +` line break into `<br>`. We scan the working buffer
//! left to right and splice a hard-break sentinel for each match, mirroring the
//! legacy parser's `check_hard_break`:
//!
//! - ` +\n` anywhere → hard break (the ` +` and the `\n` are both consumed).
//! - ` +` at the very end of the buffer → hard break.
//!
//! The end-of-buffer case is what Asciidoctor's `edges_are_line_boundaries`
//! flag guards in the legacy engine: a trailing ` +` is a break only at a true
//! line edge, never inside a reparsed span. In the string-rewriting model that
//! flag becomes unnecessary — a ` +` that sits inside a span is followed by the
//! span's closing sentinel (a `TAG_LEAD` byte), so it is neither at buffer end
//! nor before a `\n` and stays literal automatically (`*x +*` →
//! `<strong>x +</strong>`, while top-level `*x* +` → `…</strong><br>`).
//!
//! A `+` that begins a passthrough (`+…+`) is not handled here — passthrough
//! extraction is a separate, earlier pass. Until it lands, such inputs diverge
//! from legacy and fall back through the gate.

use super::tokenize::{sentinel_end, utf8_char_len, Work, TAG_LEAD};

/// Splice hard-break sentinels for every ` +` line break in the buffer.
pub(super) fn run(work: &mut Work) {
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

        // ` +` followed by `\n`, or at the true end of the buffer.
        if bytes[i] == b' ' && bytes.get(i + 1).copied() == Some(b'+') {
            let after = bytes.get(i + 2).copied();
            if after == Some(b'\n') {
                out.push_str(&work.break_sentinel());
                i += 3; // consume ` +\n`
                continue;
            }
            if i + 2 == bytes.len() {
                out.push_str(&work.break_sentinel());
                i += 2; // consume ` +` at the line edge
                continue;
            }
        }

        let len = utf8_char_len(bytes[i]);
        out.push_str(&old[i..i + len]);
        i += len;
    }

    work.buf = out;
}
