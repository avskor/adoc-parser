//! HTML escaping and text-shaping helpers shared by all renderer modules.

use crate::*;

pub(crate) fn html_escape(output: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            // Drop NUL: invalid in HTML, and reserved for the internal xref
            // placeholder sentinel (\x00XREF_N\x00). Stripping it from user text
            // keeps that sentinel collision-proof in finish() (D5).
            '\0' => {}
            _ => output.push(ch),
        }
    }
}

/// Like `html_escape` but does NOT escape `"` — for use in text content (not attributes).
pub(crate) fn html_escape_text(output: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            // Drop NUL — see `html_escape` (guards the xref sentinel, D5).
            '\0' => {}
            _ => output.push(ch),
        }
    }
}

/// Emit `text` with every embedded newline turned into a hard break
/// (`<br>\n`), used for paragraphs carrying the `hardbreaks` option. The final
/// line gets no trailing `<br>`. When `escape` is set, each line is HTML-escaped
/// (the `<br>` markers are inserted afterwards so they survive escaping).
pub(crate) fn push_hardbreaks_text(output: &mut String, text: &str, escape: bool) {
    let mut first = true;
    for line in text.split('\n') {
        if !first {
            output.push_str("<br>\n");
        }
        first = false;
        if escape {
            html_escape_text(output, line);
        } else {
            output.push_str(line);
        }
    }
}

/// Drop spaces/tabs that immediately precede a newline. The parser combines a
/// multi-line paragraph into one Text event with embedded `\n` line breaks;
/// Asciidoctor rstrips every source line, so trailing whitespace before each
/// break must not survive. Whitespace at the very end (no trailing `\n`) is
/// left intact — it may sit mid-line before an inline element, not at a break.
/// Borrows when there is nothing to strip (no allocation in the common case).
pub(crate) fn rstrip_line_trailing_ws(text: &str) -> CowStr<'_> {
    let has_ws_before_nl = text
        .as_bytes()
        .windows(2)
        .any(|w| matches!(w[0], b' ' | b'\t') && w[1] == b'\n');
    if !has_ws_before_nl {
        return CowStr::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for segment in text.split_inclusive('\n') {
        match segment.strip_suffix('\n') {
            Some(line) => {
                out.push_str(line.trim_end_matches([' ', '\t']));
                out.push('\n');
            }
            None => out.push_str(segment),
        }
    }
    CowStr::Owned(out)
}

/// Emit a single HTML attribute ` name="value"` with the value HTML-escaped.
/// Canonical path for any attribute carrying a non-constant value — keeps the
/// "everything written into an attribute goes through html_escape" rule structural (D1 root).
pub(crate) fn write_attr(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    html_escape(output, value);
    output.push('"');
}
