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

/// Character-reference-preserving escape, shared core of the two public variants
/// below. Escapes `<`/`>`/`&` (and, when `quotes` is set, `"`) like
/// [`html_escape`], EXCEPT a `&` that begins a syntactically valid reference
/// (`&#NNN;` / `&#xHHH;` / `&name;`) is copied VERBATIM rather than re-escaped to
/// `&amp;`. A bare `&` (`?a=1&b=2`) still becomes `&amp;`.
fn escape_preserving_refs(output: &mut String, text: &str, quotes: bool) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'&' => {
                let len = adoc_parser::char_ref_len(bytes, i);
                if len > 0 {
                    // ASCII-only reference, so `i..i + len` is a char boundary.
                    output.push_str(&text[i..i + len]);
                    i += len;
                } else {
                    output.push_str("&amp;");
                    i += 1;
                }
            }
            b'<' => {
                output.push_str("&lt;");
                i += 1;
            }
            b'>' => {
                output.push_str("&gt;");
                i += 1;
            }
            b'"' if quotes => {
                output.push_str("&quot;");
                i += 1;
            }
            // Drop NUL — see `html_escape` (guards the xref sentinel, D5).
            b'\0' => i += 1,
            // Any other byte: copy the whole UTF-8 char. Every byte handled above
            // is ASCII, so it never occurs inside a multi-byte sequence and `i`
            // always sits on a char boundary here.
            b => {
                let l = if b < 0x80 {
                    1
                } else if b >> 5 == 0b110 {
                    2
                } else if b >> 4 == 0b1110 {
                    3
                } else {
                    4
                };
                output.push_str(&text[i..i + l]);
                i += l;
            }
        }
    }
}

/// Attribute-value escape (escapes `"`, like [`html_escape`]) that preserves an
/// already-formed character reference. For a link/image `href`/`alt`/`src`, an
/// icon's `fa-…` class list, and any other verbatim attribute/`"`-bearing context
/// whose value may carry an entity Asciidoctor keeps verbatim.
///
/// Asciidoctor treats a character reference written inside a URL, an `alt`
/// attribute, or a `kbd:`/`menu:`/`icon:` macro's verbatim content as an
/// already-formed entity (its `replacements`/passthrough pass ran over the value
/// first), so it must not be escaped a second time: `link:a&#167;b[t]` →
/// `href="a&#167;b"`, `menu:File[Save As&#8230;]` → `<b class="menuitem">Save
/// As&#8230;</b>`, while a bare `&` (`?a=1&b=2`) still becomes `&amp;`.
pub(crate) fn html_escape_preserving_refs(output: &mut String, text: &str) {
    escape_preserving_refs(output, text, true);
}

/// Like [`html_escape_preserving_refs`] but does NOT escape `"` — for text
/// content (not attributes), e.g. a `btn:[…]` label or an `indexterm2:[…]` flow
/// term, mirroring the `specialcharacters` set that leaves `"` untouched.
pub(crate) fn html_escape_text_preserving_refs(output: &mut String, text: &str) {
    escape_preserving_refs(output, text, false);
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

/// Like [`write_attr`] but preserves already-formed character references in the
/// value — for a link/image `href` whose target may carry an entity Asciidoctor
/// keeps verbatim (`link:a&#167;b[t]` → `href="a&#167;b"`). See
/// [`html_escape_preserving_refs`].
pub(crate) fn write_attr_href(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    html_escape_preserving_refs(output, value);
    output.push('"');
}
