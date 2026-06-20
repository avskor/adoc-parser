use std::borrow::Cow;

/// Strip a single leading UTF-8 BOM (`U+FEFF`), mirroring Asciidoctor's Reader,
/// which removes it before any parsing. Zero-copy: returns a sub-slice. Only the
/// leading BOM is removed; a BOM elsewhere in the text is left intact (matches
/// Asciidoctor).
pub fn strip_bom(input: &str) -> &str {
    input.strip_prefix('\u{feff}').unwrap_or(input)
}

pub fn split_lines(input: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = input.as_bytes();
    let len = bytes.len();

    let mut i = 0;
    while i < len {
        if bytes[i] == b'\n' {
            let end = if i > 0 && bytes[i - 1] == b'\r' {
                i - 1
            } else {
                i
            };
            lines.push(&input[start..end]);
            start = i + 1;
        }
        i += 1;
    }

    if start <= len {
        lines.push(&input[start..len]);
    }

    lines
}

pub fn is_blank(line: &str) -> bool {
    line.chars().all(|c| c == ' ' || c == '\t')
}

pub fn count_leading(line: &str, ch: char) -> usize {
    line.chars().take_while(|&c| c == ch).count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterType {
    Listing,    // ----
    Literal,    // ....
    Example,    // ====
    Sidebar,    // ****
    Quote,      // ____
    Open,       // --
    Comment,    // ////
    Passthrough,// ++++
}

pub fn is_delimiter(line: &str) -> Option<(DelimiterType, usize)> {
    let trimmed = line.trim_end();
    if trimmed.len() < 2 {
        return None;
    }

    let bytes = trimmed.as_bytes();
    let first = bytes[0];
    let all_same = bytes.iter().all(|&b| b == first);

    if !all_same {
        return None;
    }

    let len = trimmed.len();

    match first {
        b'-' if len == 2 => Some((DelimiterType::Open, 2)),
        b'-' if len >= 4 => Some((DelimiterType::Listing, len)),
        b'.' if len >= 4 => Some((DelimiterType::Literal, len)),
        b'=' if len >= 4 => Some((DelimiterType::Example, len)),
        b'*' if len >= 4 => Some((DelimiterType::Sidebar, len)),
        b'_' if len >= 4 => Some((DelimiterType::Quote, len)),
        b'/' if len >= 4 => Some((DelimiterType::Comment, len)),
        b'+' if len >= 4 => Some((DelimiterType::Passthrough, len)),
        _ => None,
    }
}

pub fn strip_section_marker(line: &str) -> Option<(u8, &str)> {
    // Asciidoctor's `SectionTitleRx` (`/^=={0,5}[ \t]+(\S.*?)[ \t]*$/`) is anchored
    // at column 0, so any leading whitespace disqualifies the line as a section
    // title — it falls through to a literal paragraph instead. Operate on `line`
    // directly (no `trim_start`): a leading space makes `count_leading` return 0.
    let level = count_leading(line, '=');
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with(' ') {
        return None;
    }
    let title = rest[1..].trim();
    if title.is_empty() {
        return None;
    }
    Some((level as u8, title))
}

pub fn is_thematic_break(line: &str) -> bool {
    line.trim() == "'''" || is_markdown_thematic_break(line)
}

/// Markdown-style thematic break, matching Asciidoctor's
/// `/^ {0,3}([-*_])( *)\1\2\1$/`: up to 3 leading spaces, then one of `-`, `*`
/// or `_` repeated three times with identical (possibly empty) spacing between
/// each occurrence. The line is right-trimmed first (Asciidoctor reads lines
/// rstripped). Exactly three markers — `----` (four) is a listing delimiter and
/// `--` (two) an open block, neither of which matches.
fn is_markdown_thematic_break(line: &str) -> bool {
    let bytes = line.trim_end().as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i > 3 {
        return false;
    }
    let rest = &bytes[i..];
    let Some(&c) = rest.first() else { return false };
    if c != b'-' && c != b'*' && c != b'_' {
        return false;
    }
    // Spaces between the first and second marker define the gap; it must repeat
    // identically between the second and third.
    let mut j = 1;
    while rest.get(j) == Some(&b' ') {
        j += 1;
    }
    let gap = j - 1;
    if rest.get(j) != Some(&c) {
        return false;
    }
    j += 1;
    for _ in 0..gap {
        if rest.get(j) != Some(&b' ') {
            return false;
        }
        j += 1;
    }
    if rest.get(j) != Some(&c) {
        return false;
    }
    j += 1;
    j == rest.len()
}

pub fn is_page_break(line: &str) -> bool {
    line.trim() == "<<<"
}

pub fn is_block_title(line: &str) -> Option<&str> {
    let trimmed = line.trim_end();
    if trimmed.starts_with('.') && !trimmed.starts_with("..") {
        let rest = &trimmed[1..];
        if !rest.is_empty() && !rest.starts_with(' ') {
            return Some(rest);
        }
    }
    None
}

pub fn is_block_attribute(line: &str) -> Option<&str> {
    let trimmed = line.trim_end();
    // Block attribute lines must not have leading whitespace
    if trimmed.starts_with(' ') || trimmed.starts_with('\t') {
        return None;
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        // Block anchor `[[id]]` / `[[id,reftext]]`: the WHOLE line must be the
        // anchor (Asciidoctor BlockAnchorRx). `[[id]]text` is a paragraph with
        // an inline anchor, not an attribute line.
        if inner.starts_with('[') {
            let interior = trimmed.strip_prefix("[[").and_then(|s| s.strip_suffix("]]"));
            return match interior {
                Some(i) if !i.is_empty() && !i.contains('[') && !i.contains(']') => Some(inner),
                _ => None,
            };
        }
        // Asciidoctor BlockAttributeListRx: empty `[]`, or the first char must
        // be a word char, `{`, `,`, `.`, `#`, `"`, `'` or `%`.
        if let Some(first) = inner.chars().next()
            && !(first.is_ascii_alphanumeric() || matches!(first, '_' | '{' | ',' | '.' | '#' | '"' | '\'' | '%'))
        {
            return None;
        }
        Some(inner)
    } else {
        None
    }
}

pub fn is_attribute_entry(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_end();
    if !trimmed.starts_with(':') {
        return None;
    }
    let rest = &trimmed[1..];
    let end = rest.find(':')?;
    if end == 0 {
        return None;
    }
    let after_sep = &rest[end + 1..];
    // Mirror Asciidoctor's AttributeEntryRx value clause `:name:(?:[ \t]+value)?$`:
    // after the separator colon there must be whitespace or end-of-line. A `::`
    // (or `:value` with no leading space) is NOT an attribute entry — e.g.
    // `:context:: desc` is a description-list term `:context`, and
    // `:foo:bar:: desc` is the term `:foo:bar`.
    if !after_sep.is_empty() && !after_sep.starts_with(' ') && !after_sep.starts_with('\t') {
        return None;
    }
    let name = &rest[..end];
    let value = after_sep.trim_start();
    Some((name, value))
}

pub fn is_list_marker_unordered(line: &str) -> Option<(u8, &str)> {
    let trimmed = line.trim_start();
    // The returned number is a MARKER IDENTITY, not a nesting level: Asciidoctor
    // nests unordered lists by matching the literal marker against the open list
    // stack (a marker that matches an ancestor is a sibling; an unmatched one
    // nests). Star markers use their `*`-count as identity (`*`→1, `**`→2, …).
    // The hyphen `-` is a SEPARATE marker family, so it gets identity 0 (out of
    // band below the star counts) — otherwise `- x` under `* y` would collide
    // with `*` at 1 and render flat instead of nesting (probes /tmp/p_un*).
    if let Some(rest) = trimmed.strip_prefix("- ") {
        let text = rest.trim_start();
        if text.is_empty() {
            return None;
        }
        return Some((0, text));
    }
    let stars = count_leading(trimmed, '*');
    if stars == 0 {
        return None;
    }
    let rest = &trimmed[stars..];
    if !rest.starts_with(' ') {
        return None;
    }
    let text = rest[1..].trim_start();
    if text.is_empty() {
        return None;
    }
    Some((stars as u8, text))
}

pub fn is_list_marker_ordered(line: &str) -> Option<(u8, &str)> {
    let trimmed = line.trim_start();
    // Numbered marker: `N. text` (depth 1)
    if let Some(dot_pos) = trimmed.find(". ") {
        let prefix = &trimmed[..dot_pos];
        if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
            return Some((1, trimmed[dot_pos + 2..].trim_start()));
        }
    }
    let dots = count_leading(trimmed, '.');
    if dots == 0 {
        return None;
    }
    let rest = &trimmed[dots..];
    if !rest.starts_with(' ') {
        return None;
    }
    Some((dots as u8, rest[1..].trim_start()))
}

pub fn is_admonition(line: &str) -> Option<(&str, &str)> {
    let labels = ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"];
    for label in &labels {
        if let Some(rest) = line.strip_prefix(label)
            && let Some(rest) = rest.strip_prefix(": ") {
                return Some((label, rest.trim()));
        }
    }
    None
}

pub fn is_line_comment(line: &str) -> bool {
    let trimmed = line.trim_end();
    trimmed.starts_with("//") && is_delimiter(trimmed).is_none()
}

/// Match a block media macro line against Asciidoctor's `BlockMediaMacroRx`
/// (`^(image|video|audio)::(\S|\S.*?\S)\[(.+)?\]$`). The line must END with
/// `]` (after rstrip) — trailing content such as `image::x[] <.>` demotes the
/// line to a paragraph; the target must be non-empty with no leading/trailing
/// whitespace (internal whitespace is allowed). Returns `(target, attrs)`.
fn match_block_media<'a>(line: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(prefix)?;
    // `\]$` — the macro must end with the closing bracket.
    let inner = rest.strip_suffix(']')?;
    let bracket_start = inner.find('[')?;
    let target = &inner[..bracket_start];
    if target.is_empty()
        || target.starts_with(char::is_whitespace)
        || target.ends_with(char::is_whitespace)
    {
        return None;
    }
    let attrs = &inner[bracket_start + 1..];
    Some((target, attrs))
}

pub fn is_block_image(line: &str) -> Option<(&str, &str)> {
    match_block_media(line, "image::")
}

pub fn is_block_video(line: &str) -> Option<(&str, &str)> {
    match_block_media(line, "video::")
}

pub fn is_block_audio(line: &str) -> Option<(&str, &str)> {
    match_block_media(line, "audio::")
}

pub fn is_description_list_marker(line: &str) -> Option<(u8, &str, &str)> {
    let trimmed = line.trim_end();
    // Find the first occurrence of "::" that is a valid marker
    // We need to find 2-4 consecutive colons where:
    // - term before is non-empty (trimmed)
    // - after colons: end of line or space followed by description
    let bytes = trimmed.as_bytes();
    let len = bytes.len();

    let mut i = 0;
    while i < len {
        if bytes[i] == b':' && i + 1 < len && bytes[i + 1] == b':' {
            // Count consecutive colons
            let colon_start = i;
            let mut colon_count = 0;
            while i < len && bytes[i] == b':' {
                colon_count += 1;
                i += 1;
            }

            if !(2..=4).contains(&colon_count) {
                continue;
            }

            let term = trimmed[..colon_start].trim();
            if term.is_empty() {
                continue;
            }

            // After colons: must be end of line or space
            if i < len && bytes[i] != b' ' {
                continue;
            }

            let desc = if i < len {
                trimmed[i..].trim_start()
            } else {
                ""
            };

            let depth = (colon_count - 1) as u8;
            return Some((depth, term, desc));
        }
        i += 1;
    }

    None
}

/// Check if a value string ends with a line continuation marker.
///
/// Returns `Some((value_without_continuation, is_hard_wrap))`:
/// - `"value \\"` → `Some(("value", false))` — soft wrap (joined with space)
/// - `"value + \\"` → `Some(("value", true))` — hard wrap (joined with newline)
/// - `"value"` → `None`
/// - `"value\\"` (no space before `\`) → `None`
pub fn strip_line_continuation(value: &str) -> Option<(&str, bool)> {
    if !value.ends_with('\\') {
        return None;
    }
    let before_backslash = &value[..value.len() - 1];
    // Hard wrap: ` + \`
    if let Some(rest) = before_backslash.strip_suffix(" + ") {
        return Some((rest, true));
    }
    // Soft wrap: ` \`
    if before_backslash.ends_with(' ') {
        return Some((before_backslash.trim_end(), false));
    }
    None
}

pub fn is_list_continuation(line: &str) -> bool {
    line.trim() == "+"
}

/// Matches the TOC block macro `toc::[]` / `toc::[attrs]`, mirroring
/// Asciidoctor's `BlockTocMacroRx = /^toc::\[(#{CC_ANY}+)?\]$/`: the line
/// (after trimming) must be exactly the macro — a leading prefix or a trailing
/// remainder disqualifies it. Used as a block-boundary guard, so it stays a
/// `bool`; [`toc_macro_attrs`] extracts the bracket content for the parser.
pub fn is_toc_macro(line: &str) -> bool {
    toc_macro_attrs(line).is_some()
}

/// Returns the bracket content of a TOC block macro: `""` for `toc::[]`,
/// `"levels=1"` for `toc::[levels=1]`, or `None` when the line is not a TOC
/// macro. The bracket content is left raw for the caller to parse as an
/// attribute list.
pub fn toc_macro_attrs(line: &str) -> Option<&str> {
    line.trim().strip_prefix("toc::[")?.strip_suffix(']')
}

pub fn is_include_directive(line: &str) -> Option<(&str, &str)> {
    // Mirrors Asciidoctor's IncludeDirectiveRx
    // (`^(\\)?include::([^\s\[](?:[^\[]*[^\s\[])?)\[(.+)?\]$`): the directive
    // must start at column 0 and the closing `]` must end the line (reader
    // lines are right-trimmed) — a trailing remainder makes the line plain
    // text. The target cannot contain `[` or start/end with whitespace.
    let rest = line.trim_end().strip_prefix("include::")?;
    let rest = rest.strip_suffix(']')?;
    let bracket_start = rest.find('[')?;
    let path = &rest[..bracket_start];
    if path.is_empty()
        || path.starts_with(char::is_whitespace)
        || path.ends_with(char::is_whitespace)
    {
        return None;
    }
    let attrs = &rest[bracket_start + 1..];
    Some((path, attrs))
}

/// Type of callout marker found in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalloutMarker {
    /// `<N>` — standard numbered callout; `<.>` → number 0 (autonumber)
    Standard(u32),
    /// `<!--N-->` — XML comment callout; `<!--.-->` → number 0 (autonumber)
    XmlComment(u32),
}

pub fn strip_callout_markers(line: &str) -> (Cow<'_, str>, Vec<CalloutMarker>) {
    let mut markers = Vec::new();
    let mut end = line.len();
    // Byte position (in `line`) of a single escaping backslash to drop. An
    // escaped marker (`\<1>` / `\<!--1-->`) is NOT a conum: Asciidoctor keeps
    // it literal and removes the backslash (`CalloutSourceRx` escape group).
    let mut escape_at: Option<usize> = None;

    loop {
        let trimmed = line[..end].trim_end();
        if trimmed.is_empty() {
            break;
        }

        // Try XML comment callout: <!--N--> or <!--.-->
        if let Some(before_close) = trimmed.strip_suffix("-->") {
            if let Some(open_pos) = before_close.rfind("<!--") {
                let inner = &before_close[open_pos + 4..];
                let num = if inner == "." {
                    Some(0u32)
                } else if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
                    inner.parse::<u32>().ok()
                } else {
                    None
                };
                if let Some(n) = num {
                    if open_pos > 0 && trimmed.as_bytes()[open_pos - 1] == b'\\' {
                        // Escaped — keep the marker literal, drop the backslash.
                        escape_at = Some(open_pos - 1);
                        break;
                    }
                    markers.push(CalloutMarker::XmlComment(n));
                    end = open_pos;
                    continue;
                }
            }
            break;
        }

        // Try standard callout: <N> or <.>
        if !trimmed.ends_with('>') {
            break;
        }
        let open = match trimmed[..trimmed.len() - 1].rfind('<') {
            Some(pos) => pos,
            None => break,
        };
        let inner = &trimmed[open + 1..trimmed.len() - 1];
        let num = if inner == "." {
            Some(0u32)
        } else if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
            inner.parse::<u32>().ok()
        } else {
            None
        };
        match num {
            Some(n) => {
                if open > 0 && trimmed.as_bytes()[open - 1] == b'\\' {
                    // Escaped — keep the marker literal, drop the backslash.
                    escape_at = Some(open - 1);
                    break;
                }
                markers.push(CalloutMarker::Standard(n));
                end = open;
            }
            None => break,
        }
    }

    markers.reverse();
    match escape_at {
        None => (Cow::Borrowed(&line[..end]), markers),
        Some(bs) => {
            // Drop the single escaping backslash; the marker text stays.
            let mut s = String::with_capacity(end - 1);
            s.push_str(&line[..bs]);
            s.push_str(&line[bs + 1..end]);
            (Cow::Owned(s), markers)
        }
    }
}

/// Byte offset in `text` at which a leading-comment callout *guard* begins —
/// the line-comment prefix Asciidoctor strips before a conum. Mirrors
/// `CalloutSourceRx` group 1 (`((?://|#|--|;;) ?)?`): a comment token
/// (`//`, `#`, `--`, `;;`) immediately before the marker, optionally followed
/// by a single space. Returns `text.len()` when there is no guard.
///
/// `text` is the run preceding the first callout on a verbatim line (e.g.
/// `"require 'asciidoctor' # "`). The guard slice `&text[offset..]` keeps the
/// trailing space (`"# "`); the part before it (`"require 'asciidoctor' "`)
/// retains the space that sat before the comment token. Asciidoctor allows at
/// most one space between the comment token and the marker, so two spaces
/// (`"x #  "`) do not form a guard — matching the engine.
pub fn callout_guard_offset(text: &str) -> usize {
    let trimmed = text.strip_suffix(' ').unwrap_or(text);
    let token_len = if trimmed.ends_with("//") || trimmed.ends_with(";;") || trimmed.ends_with("--")
    {
        2
    } else if trimmed.ends_with('#') {
        1
    } else {
        return text.len();
    };
    trimmed.len() - token_len
}

pub fn is_callout_list_item(line: &str) -> Option<(u32, &str)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('<') {
        return None;
    }
    let close = trimmed.find('>')?;
    if close < 2 {
        return None;
    }
    let inner = &trimmed[1..close];
    // Autonumbered: `<.>` → number 0 (will be assigned by caller)
    let n = if inner == "." {
        0
    } else {
        if !inner.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        inner.parse::<u32>().ok()?
    };
    let rest = &trimmed[close + 1..];
    if rest.is_empty() {
        return Some((n, ""));
    }
    if !rest.starts_with(' ') {
        return None;
    }
    Some((n, rest[1..].trim()))
}

pub fn parse_checklist_marker(text: &str) -> (Option<bool>, &str) {
    if let Some(rest) = text.strip_prefix("[x] ") {
        (Some(true), rest)
    } else if let Some(rest) = text.strip_prefix("[*] ") {
        (Some(true), rest)
    } else if let Some(rest) = text.strip_prefix("[ ] ") {
        (Some(false), rest)
    } else {
        (None, text)
    }
}

pub fn is_table_delimiter(line: &str) -> bool {
    // Asciidoctor accepts a prefix char followed by THREE OR MORE `=` (`|===`,
    // `|====`, …); the rest of the line after the prefix must be all `=` (no
    // trailing content). Open and close delimiters need not be the same length.
    // The prefix selects the cell format/separator: `|` PSV (native, `|`
    // separator), `,` CSV, `:` DSV, `!` nested-table PSV (`!` separator — used
    // for tables inside an AsciiDoc `a` cell so the inner separator differs from
    // the enclosing `|`).
    let trimmed = line.trim();
    match trimmed.as_bytes().first() {
        Some(b'|' | b',' | b':' | b'!') => {
            let rest = &trimmed[1..];
            rest.len() >= 3 && rest.bytes().all(|b| b == b'=')
        }
        _ => false,
    }
}

use crate::event::{CellStyle, HAlign, VAlign};

#[derive(Debug, Clone, PartialEq)]
pub struct CellSpec<'a> {
    pub content: Cow<'a, str>,
    /// Duplication factor (`3*|x` → the cell repeated 3 times). Kept
    /// unexpanded while the cell may still receive continuation lines —
    /// asciidoctor copies a duplicated cell with its complete content.
    pub duplication: u8,
    pub colspan: u8,
    pub rowspan: u8,
    pub style: CellStyle,
    /// True when the style came from an explicit style char in the cell spec
    /// (including `d`/`v`, which map to Default): an explicit style wins over
    /// the column's style, an unspecified one inherits it.
    pub style_explicit: bool,
    pub halign: HAlign,
    pub valign: VAlign,
    /// True when the cell spec carried an explicit horizontal-align operator
    /// (`<`/`^`/`>`). An explicit alignment wins over the column default; an
    /// unspecified one inherits it. Needed because the default `Left` value is
    /// indistinguishable from an explicit `<` without this flag.
    pub halign_explicit: bool,
    /// True when the cell spec carried an explicit vertical-align operator
    /// (`.<`/`.^`/`.>`). See [`CellSpec::halign_explicit`].
    pub valign_explicit: bool,
}

/// Cells parsed from one line of a psv table, plus any text that appeared
/// before the first `|` — that text continues the last cell of the previous
/// line (asciidoctor joins it to the cell content with a newline).
#[derive(Debug, Clone, PartialEq)]
pub struct TableLineCells<'a> {
    pub continuation: Option<Cow<'a, str>>,
    pub cells: Vec<CellSpec<'a>>,
}

/// A separator char immediately preceded by `\` is an escaped cell separator: it
/// does not split cells, and exactly one backslash is consumed (`\|` → `|`,
/// `\\|` → `\|` in one cell — probe-verified). `sep` is the table's cell
/// separator byte (`|` for native/CSV-less PSV tables, `!` for nested tables).
pub fn unescape_cell_sep(s: &str, sep: u8) -> Cow<'_, str> {
    let sep_ch = sep as char;
    let mut escaped = String::with_capacity(2);
    escaped.push('\\');
    escaped.push(sep_ch);
    if s.contains(&escaped) {
        Cow::Owned(s.replace(&escaped, &sep_ch.to_string()))
    } else {
        Cow::Borrowed(s)
    }
}

/// Byte offset of the first unescaped `sep` in `s`, if any.
fn find_unescaped_sep(s: &str, sep: u8) -> Option<usize> {
    let bytes = s.as_bytes();
    (0..bytes.len()).find(|&i| bytes[i] == sep && (i == 0 || bytes[i - 1] != b'\\'))
}

/// Split `s` at unescaped `sep` separators (escaped `\sep` stays inside a part).
fn split_unescaped_sep(s: &str, sep: u8) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    for i in 0..bytes.len() {
        if bytes[i] == sep && (i == 0 || bytes[i - 1] != b'\\') {
            parts.push(&s[start..i]);
            start = i + 1;
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Parse alignment prefix from a cell specifier (prefix before first `|`).
/// Reads `[<^>]` for halign, then `.[<^>]` for valign from the beginning.
/// Returns `(remaining, halign, valign, halign_explicit, valign_explicit)`;
/// the explicit flags record whether each operator was actually present.
pub fn parse_cell_align_prefix(s: &str) -> (&str, HAlign, VAlign, bool, bool) {
    let mut rest = s;
    let mut halign = HAlign::default();
    let mut valign = VAlign::default();
    let mut halign_explicit = false;
    let mut valign_explicit = false;

    // Parse halign: <, ^, >
    if let Some(stripped) = rest.strip_prefix('<') {
        halign = HAlign::Left;
        halign_explicit = true;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('^') {
        halign = HAlign::Center;
        halign_explicit = true;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('>') {
        halign = HAlign::Right;
        halign_explicit = true;
        rest = stripped;
    }

    // Parse valign: .<, .^, .>
    if let Some(stripped) = rest.strip_prefix(".<") {
        valign = VAlign::Top;
        valign_explicit = true;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix(".^") {
        valign = VAlign::Middle;
        valign_explicit = true;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix(".>") {
        valign = VAlign::Bottom;
        valign_explicit = true;
        rest = stripped;
    }

    (rest, halign, valign, halign_explicit, valign_explicit)
}

/// Parse alignment suffix from the end of a segment (content between pipes).
/// The alignment spec for the NEXT cell sits at the END of the segment, after content.
/// Pattern at end: `[<^>]` for halign, then `.[<^>]` for valign.
/// Only valid if preceded by space or at start of string.
/// Returns `(remaining_content, halign, valign, halign_explicit, valign_explicit)`;
/// the explicit flags record whether each operator was actually present.
pub fn parse_cell_align_suffix(s: &str) -> (&str, HAlign, VAlign, bool, bool) {
    let trimmed = s.trim_end();
    if trimmed.is_empty() {
        return (s, HAlign::default(), VAlign::default(), false, false);
    }

    let bytes = trimmed.as_bytes();
    let mut end = trimmed.len();
    let mut halign = HAlign::default();
    let mut valign = VAlign::default();
    let mut halign_explicit = false;
    let mut valign_explicit = false;

    // Try to parse valign from end: .< .^ .>
    if end >= 2 && bytes[end - 2] == b'.' {
        match bytes[end - 1] {
            b'<' => { valign = VAlign::Top; end -= 2; valign_explicit = true; }
            b'^' => { valign = VAlign::Middle; end -= 2; valign_explicit = true; }
            b'>' => { valign = VAlign::Bottom; end -= 2; valign_explicit = true; }
            _ => {}
        }
    }

    // Try to parse halign from end: < ^ >
    if end >= 1 {
        match bytes[end - 1] {
            b'<' => { halign = HAlign::Left; end -= 1; halign_explicit = true; }
            b'^' => { halign = HAlign::Center; end -= 1; halign_explicit = true; }
            b'>' => { halign = HAlign::Right; end -= 1; halign_explicit = true; }
            _ => {}
        }
    }

    // Only valid if preceded by space or at start of string
    if halign_explicit || valign_explicit {
        let remaining = &trimmed[..end];
        if remaining.is_empty() || remaining.ends_with(' ') {
            return (
                &s[..s.len() - (trimmed.len() - end)],
                halign,
                valign,
                halign_explicit,
                valign_explicit,
            );
        }
    }

    (s, HAlign::default(), VAlign::default(), false, false)
}

/// Parse a span modifier from the end of a segment (the part before `|`).
/// Pattern: `(\d+)?(?:\.(\d+))?\+` at the end of the string.
/// Returns `(remaining_content, colspan, rowspan)`.
pub fn parse_span_spec(s: &str) -> (&str, u8, u8) {
    let trimmed = s.trim_end();
    let Some(rest) = trimmed.strip_suffix('+') else {
        return (s, 1, 1);
    };

    // Parse backwards from the `+`: optional `(\d+)?(?:\.(\d+))?`
    // Full pattern: `<colspan>.<rowspan>+` or `.<rowspan>+` or `<colspan>+`
    let mut colspan: u8 = 1;
    let mut rowspan: u8 = 1;

    if let Some(dot_pos) = rest.rfind('.') {
        let after_dot = &rest[dot_pos + 1..];
        let before_dot = &rest[..dot_pos];
        // after_dot must be all digits (rowspan)
        if !after_dot.is_empty() && after_dot.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(r) = after_dot.parse::<u8>() {
                rowspan = r.max(1);
            }
            // before_dot: either empty or all digits (colspan), preceded by content
            // We need to find where the spec starts in before_dot
            let spec_start = before_dot.len()
                - before_dot
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_digit())
                    .count();
            let digits = &before_dot[spec_start..];
            let content = &s[..s.len() - (trimmed.len() - spec_start)];
            if !digits.is_empty()
                && let Ok(c) = digits.parse::<u8>()
            {
                colspan = c.max(1);
            }
            return (content.trim_end(), colspan, rowspan);
        }
        // Not valid rowspan digits after dot — check if whole thing is just colspan
    }

    // No dot — try plain `<colspan>+`
    let spec_start = rest.len()
        - rest
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .count();
    let digits = &rest[spec_start..];
    let content = &s[..s.len() - (trimmed.len() - spec_start)];
    if !digits.is_empty() {
        if let Ok(c) = digits.parse::<u8>() {
            colspan = c.max(1);
        }
        return (content.trim_end(), colspan, 1);
    }

    // `+` alone is not a span spec
    (s, 1, 1)
}

/// Parse a cell content style suffix from the end of a segment (before `|`).
/// Valid style chars: `a` (AsciiDoc), `h` (Header), `e` (Emphasis),
/// `m` (Monospace), `s` (Strong), `l` (Literal), plus `d` (explicit
/// default) and `v` (verse — rendered as a default paragraph).
/// The style char is valid only if it is the last char and before it
/// is either nothing or a `+` (part of span spec).
/// The returned bool is true when a style char was consumed (explicit
/// style: wins over the column's style).
pub fn parse_cell_style_suffix(s: &str) -> (&str, CellStyle, bool) {
    if s.is_empty() {
        return (s, CellStyle::Default, false);
    }
    let last_byte = s.as_bytes()[s.len() - 1];
    let style = match last_byte {
        b'a' => CellStyle::AsciiDoc,
        b'h' => CellStyle::Header,
        b'e' => CellStyle::Emphasis,
        b'm' => CellStyle::Monospace,
        b's' => CellStyle::Strong,
        b'l' => CellStyle::Literal,
        b'd' | b'v' => CellStyle::Default,
        _ => return (s, CellStyle::Default, false),
    };
    let before = &s[..s.len() - 1];
    let before_trimmed = before.trim();
    if before_trimmed.is_empty() || before_trimmed.ends_with('+') || before.ends_with(' ') {
        (&s[..s.len() - 1], style, true)
    } else {
        (s, CellStyle::Default, false)
    }
}

/// A cell specifier parsed in full: `[N*|C.R+][align][style]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExactCellSpec {
    pub duplication: u8,
    pub colspan: u8,
    pub rowspan: u8,
    pub halign: HAlign,
    pub valign: VAlign,
    pub style: CellStyle,
    /// See [`CellSpec::style_explicit`].
    pub style_explicit: bool,
    /// See [`CellSpec::halign_explicit`].
    pub halign_explicit: bool,
    /// See [`CellSpec::valign_explicit`].
    pub valign_explicit: bool,
}

/// Parse an entire string as a cell specifier (mirror of asciidoctor's
/// CellSpecRx): optional span factor (`2+`, `.3+`, `2.3+`) or duplication
/// factor (`3*`), then optional alignment (`<^>` for halign and/or `.<^>`
/// for valign), then optional style char. Returns None unless the whole
/// string is consumed — partial matches are not specs.
pub fn parse_cell_spec_exact(s: &str) -> Option<ExactCellSpec> {
    if s.is_empty() {
        return None;
    }
    let mut rest = s;
    let mut duplication: u8 = 1;
    let mut colspan: u8 = 1;
    let mut rowspan: u8 = 1;

    if let Some(op_idx) = rest.find(['+', '*']) {
        let factor = &rest[..op_idx];
        if !factor.is_empty() && factor.chars().all(|c| c.is_ascii_digit() || c == '.') {
            if rest.as_bytes()[op_idx] == b'*' {
                duplication = factor.parse::<u8>().ok()?.max(1);
            } else if let Some((c, r)) = factor.split_once('.') {
                colspan = if c.is_empty() { 1 } else { c.parse::<u8>().ok()?.max(1) };
                rowspan = r.parse::<u8>().ok()?.max(1);
            } else {
                colspan = factor.parse::<u8>().ok()?.max(1);
            }
            rest = &rest[op_idx + 1..];
        }
    }

    let (after_align, halign, valign, halign_explicit, valign_explicit) =
        parse_cell_align_prefix(rest);
    rest = after_align;

    let (style, style_explicit) = match rest {
        "" => (CellStyle::Default, false),
        // `d` (explicit default) and `v` (verse — rendered as a default
        // paragraph by the html converter) consume the spec char.
        "d" | "v" => (CellStyle::Default, true),
        "a" => (CellStyle::AsciiDoc, true),
        "h" => (CellStyle::Header, true),
        "e" => (CellStyle::Emphasis, true),
        "m" => (CellStyle::Monospace, true),
        "s" => (CellStyle::Strong, true),
        "l" => (CellStyle::Literal, true),
        _ => return None,
    };

    Some(ExactCellSpec {
        duplication,
        colspan,
        rowspan,
        halign,
        valign,
        style,
        style_explicit,
        halign_explicit,
        valign_explicit,
    })
}

/// Convenience wrapper over [`parse_table_cells_with_sep`] for the default `|`
/// separator. Only the separator-parametrized form is used by the scanner; this
/// keeps the single-argument call ergonomic for unit tests.
#[cfg(test)]
pub fn parse_table_cells(line: &str) -> Option<TableLineCells<'_>> {
    parse_table_cells_with_sep(line, b'|')
}

/// Parse one PSV table line, splitting cells at unescaped `sep`. `sep` is `|`
/// for ordinary tables and `!` for tables nested inside an AsciiDoc `a` cell.
pub fn parse_table_cells_with_sep(line: &str, sep: u8) -> Option<TableLineCells<'_>> {
    // Find the first unescaped separator — if none, not a table line
    let first_pipe = find_unescaped_sep(line, sep)?;

    // Before first separator: a valid align+style+span spec (for the first cell
    // on this line), or content continuing the previous line's last cell —
    // possibly with a spec at its end, attached to the separator that follows
    // (mirror of the non-last part parsing below). Continuation text keeps its
    // leading indentation (significant in literal and AsciiDoc cells).
    let prefix_raw = &line[..first_pipe];
    let prefix = prefix_raw.trim();
    let mut continuation: Option<Cow<'_, str>> = None;
    let mut pending = ExactCellSpec {
        duplication: 1,
        colspan: 1,
        rowspan: 1,
        style: CellStyle::Default,
        style_explicit: false,
        halign: HAlign::default(),
        valign: VAlign::default(),
        halign_explicit: false,
        valign_explicit: false,
    };

    if !prefix.is_empty() {
        if let Some(spec) = parse_cell_spec_exact(prefix) {
            pending = spec;
        } else {
            // Continuation text; a spec for the first cell of this line may
            // still sit at its end, whitespace-separated (`tail 2+|wide`).
            let mut text = prefix_raw.trim_end();
            if let Some((before, token)) = text.rsplit_once([' ', '\t'])
                && let Some(spec) = parse_cell_spec_exact(token)
            {
                pending = spec;
                text = before.trim_end();
            }
            if !text.trim().is_empty() {
                continuation = Some(unescape_cell_sep(text, sep));
            }
        }
    }

    let default_spec = ExactCellSpec {
        duplication: 1,
        colspan: 1,
        rowspan: 1,
        style: CellStyle::Default,
        style_explicit: false,
        halign: HAlign::default(),
        valign: VAlign::default(),
        halign_explicit: false,
        valign_explicit: false,
    };
    let mut cells = Vec::new();
    let parts: Vec<&str> = split_unescaped_sep(&line[first_pipe + 1..], sep);

    for (i, part) in parts.iter().enumerate() {
        // Parse next-cell specs from END of the part. A spec only ever
        // attaches to the `|` that follows it — for the last part of the line
        // no delimiter follows, so trailing characters are plain cell content
        // (`|a` is a cell "a", not an AsciiDoc style spec).
        let is_last = i == parts.len() - 1;
        let (content, next) = if is_last {
            (*part, default_spec)
        } else if let Some((before, spec)) = part
            .trim_end()
            .rsplit_once([' ', '\t'])
            .and_then(|(b, token)| parse_cell_spec_exact(token).map(|sp| (b, sp)))
        {
            // Whitespace-separated full spec token (handles chained specs
            // like `2*>m` that the legacy suffix parsers below can't)
            (before, spec)
        } else {
            let (after_style, style, style_explicit) = parse_cell_style_suffix(part);
            let (after_span, cs, rs) = parse_span_spec(after_style);
            let (content, halign, valign, halign_explicit, valign_explicit) =
                parse_cell_align_suffix(after_span);
            (
                content,
                ExactCellSpec {
                    duplication: 1,
                    colspan: cs,
                    rowspan: rs,
                    style,
                    style_explicit,
                    halign,
                    valign,
                    halign_explicit,
                    valign_explicit,
                },
            )
        };
        let content = content.trim();

        // An empty part is still a cell — every `|` opens one (asciidoctor
        // renders `|a |` as two cells, the second an empty <td>). A trailing
        // delimiter leaves the cell open: continuation lines fill it.
        cells.push(CellSpec {
            content: unescape_cell_sep(content, sep),
            duplication: pending.duplication.max(1),
            colspan: pending.colspan,
            rowspan: pending.rowspan,
            style: pending.style,
            style_explicit: pending.style_explicit,
            halign: pending.halign,
            valign: pending.valign,
            halign_explicit: pending.halign_explicit,
            valign_explicit: pending.valign_explicit,
        });

        pending = next;
    }

    Some(TableLineCells { continuation, cells })
}

pub fn strip_markdown_heading(line: &str) -> Option<(u8, &str)> {
    // Like section titles, Markdown-style ATX headings are recognised only at
    // column 0 (Asciidoctor treats an indented `## …` as a literal paragraph).
    let level = count_leading(line, '#');
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with(' ') {
        return None;
    }
    let title = rest[1..].trim();
    if title.is_empty() {
        return None;
    }
    Some((level as u8, title))
}

pub fn is_markdown_code_fence(line: &str) -> Option<(usize, Option<&str>)> {
    let trimmed = line.trim_end();
    let backtick_count = count_leading(trimmed, '`');
    if backtick_count < 3 {
        return None;
    }
    let info = trimmed[backtick_count..].trim();
    if info.is_empty() {
        return Some((backtick_count, None));
    }
    // Reject info strings containing backticks (CommonMark spec)
    if info.contains('`') {
        return None;
    }
    Some((backtick_count, Some(info)))
}

pub fn strip_any_section_marker(line: &str) -> Option<(u8, &str)> {
    strip_section_marker(line).or_else(|| strip_markdown_heading(line))
}

/// Asciidoctor default alt for an inline icon in text mode:
/// `File.basename(name, File.extname(name)).tr('_-', ' ')` — drop the directory
/// (everything up to the last `/`), drop the trailing `.ext`, then replace `_`/`-`
/// with spaces. A leading dot is not an extension (`File.extname(".hidden") == ""`).
pub fn icon_default_alt(name: &str) -> String {
    let base = match name.rfind('/') {
        Some(i) => &name[i + 1..],
        None => name,
    };
    let stem = match base.rfind('.') {
        Some(i) if i > 0 => &base[..i],
        _ => base,
    };
    stem.replace(['_', '-'], " ")
}

/// Strip URLs and inline `icon:` macros from title text for ID generation.
/// - `https://url[text]` / `http://url[text]` → `text`
/// - `link:url[text]` → `text`
/// - bare `https://url` / `http://url` (no bracket text) → removed
/// - `icon:name[attrs]` → its alt text (explicit `alt=` or [`icon_default_alt`]),
///   mirroring Asciidoctor generating the id from the macro-substituted title.
fn strip_urls_for_id(title: &str) -> String {
    let mut result = String::with_capacity(title.len());
    let mut rest = title;

    while !rest.is_empty() {
        // Check for link: macro or http(s):// URL
        let url_match = if rest.starts_with("link:") {
            Some(("link:", 5))
        } else if rest.starts_with("https://") {
            Some(("https://", 0))
        } else if rest.starts_with("http://") {
            Some(("http://", 0))
        } else {
            None
        };

        if let Some((_prefix, url_body_offset)) = url_match {
            let search_from = if url_body_offset > 0 { url_body_offset } else { 0 };
            // Find the bracket part [text]
            if let Some(bracket_pos) = rest[search_from..].find('[') {
                let abs_bracket = search_from + bracket_pos;
                if let Some(close_pos) = rest[abs_bracket..].find(']') {
                    let text = &rest[abs_bracket + 1..abs_bracket + close_pos];
                    result.push_str(text);
                    rest = &rest[abs_bracket + close_pos + 1..];
                    continue;
                }
            }
            // Bare URL (no brackets) — skip entirely
            let url_end = rest.find(' ').unwrap_or(rest.len());
            rest = &rest[url_end..];
            continue;
        }

        // `icon:name[attrs]` → its alt text. Quotes in `alt=` need no stripping:
        // the char filter in `generate_id` drops them. Malformed (no `[`/`]`) falls
        // through to per-char handling so `icon:noclose` degrades to literal text.
        if let Some(after) = rest.strip_prefix("icon:")
            && let Some(bopen) = after.find('[')
            && let Some(rel_close) = after[bopen..].find(']')
        {
            let bclose = bopen + rel_close;
            let name = &after[..bopen];
            let attrs = &after[bopen + 1..bclose];
            let alt = attrs
                .split(',')
                .filter_map(|p| p.trim().split_once('='))
                .find(|(k, _)| k.trim() == "alt")
                .map(|(_, v)| v.trim().to_string())
                .unwrap_or_else(|| icon_default_alt(name));
            result.push_str(&alt);
            rest = &after[bclose + 1..];
            continue;
        }

        // Regular character — advance by one char
        let mut chars = rest.chars();
        if let Some(ch) = chars.next() {
            result.push(ch);
            rest = chars.as_str();
        }
    }

    result
}

pub fn generate_id(title: &str, prefix: &str, separator: &str) -> String {
    let processed = strip_urls_for_id(title);
    let title = &processed;
    let sep_char = separator.chars().next().unwrap_or('_');
    let mut id = String::with_capacity(title.len() + prefix.len());
    id.push_str(prefix);
    let mut prev_was_separator = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                id.push(lc);
            }
            prev_was_separator = false;
        } else if (ch == ' ' || ch == '-' || ch == '_' || ch == '.')
            && !prev_was_separator {
                id.push(sep_char);
                prev_was_separator = true;
        }
    }
    if id.ends_with(sep_char) && id.len() > prefix.len() {
        id.pop();
    }
    id
}

pub struct RevisionInfo<'a> {
    /// `None` — the version group did not participate; `Some("")` — it matched
    /// empty (a comma with no digits before it), which still SETS `revnumber`
    /// (Asciidoctor renders `version ,` in the header details).
    pub version: Option<&'a str>,
    pub date: &'a str,
    /// `Some("")` when the line ends with a bare `:` — `revremark` is set
    /// empty (renders an empty span), distinct from no colon at all (`None`).
    pub remark: Option<&'a str>,
}

/// Mirror of Asciidoctor's RevisionInfoLineRx
/// (`^(?:[^\d{]*(.*?),)? *(?!:)(.*?)(?: *,?: *(.*))?$`) applied to the line
/// following the author line. The regex matches nearly every line; `None`
/// (= Asciidoctor's unshift, the line falls through to the body) only happens
/// for lines whose date component would start with a colon.
pub fn parse_revision_line(line: &str) -> Option<RevisionInfo<'_>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Version group `[^\d{]*(.*?),`: the greedy non-digit prefix puts the
    // capture start at the first digit/`{`; the lazy capture ends at the first
    // comma at-or-after it. With no such comma the engine backtracks to an
    // EMPTY capture ending at the last comma before the first digit (or the
    // last comma of a digit-free line) — `hello, world` → revnumber set "".
    let m0 = trimmed
        .find(|c: char| c.is_ascii_digit() || c == '{')
        .unwrap_or(trimmed.len());
    let (version, rest) = match trimmed[m0..].find(',') {
        Some(c) => (Some(trimmed[m0..m0 + c].trim_end()), &trimmed[m0 + c + 1..]),
        None => match trimmed[..m0].rfind(',') {
            Some(c) => (Some(""), &trimmed[c + 1..]),
            None => (None, trimmed),
        },
    };

    // ` *(?!:)` — the component after the version part must not start with a
    // colon; Asciidoctor throws such a line back to the reader.
    let rest = rest.trim_start();
    if rest.starts_with(':') {
        return None;
    }

    // `(?: *,?: *(.*))?$` — remark split at the first colon (one optional
    // trailing comma before it is consumed by the spec).
    let (component, remark) = match rest.find(':') {
        Some(colon) => {
            let head = rest[..colon].trim_end();
            let head = head.strip_suffix(',').map(str::trim_end).unwrap_or(head);
            (head, Some(rest[colon + 1..].trim()))
        }
        None => (rest.trim_end(), None),
    };

    // No version group: a component with a lowercase `v` head is the version
    // with that single char sliced off (`version 5` → `ersion 5` — faithful to
    // `component.slice 1`); anything else (incl. uppercase `V`) is the date.
    let (version, date) = match version {
        Some(v) => (Some(v), component),
        None => match component.strip_prefix('v') {
            Some(sliced) if !component.is_empty() => (Some(sliced), ""),
            _ => (None, component),
        },
    };

    Some(RevisionInfo { version, date, remark })
}

pub struct AuthorInfo<'a> {
    pub fullname: &'a str,
    pub firstname: &'a str,
    pub middlename: &'a str,
    pub lastname: &'a str,
    pub initials: String,
    pub address: &'a str,
}

pub fn parse_authors(line: &str) -> Vec<AuthorInfo<'_>> {
    let mut authors = Vec::new();
    for part in line.split("; ") {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name_part, address) = if let Some(open) = part.rfind('<') {
            if let Some(close) = part[open..].find('>') {
                let addr = &part[open + 1..open + close];
                let name = part[..open].trim();
                (name, addr)
            } else {
                (part, "")
            }
        } else {
            (part, "")
        };

        let words: Vec<&str> = name_part.split_whitespace().collect();
        let (firstname, middlename, lastname) = match words.len() {
            0 => ("", "", ""),
            1 => (words[0], "", words[0]),
            2 => (words[0], "", words[1]),
            _ => {
                let first = words[0];
                let last = words[words.len() - 1];
                // Find middle substring in name_part
                let first_end = name_part.find(first).unwrap() + first.len();
                let last_start = name_part.rfind(last).unwrap();
                let mid = name_part[first_end..last_start].trim();
                (first, mid, last)
            }
        };

        let mut initials = String::new();
        for word in &words {
            if let Some(ch) = word.chars().next() {
                initials.push(ch.to_ascii_uppercase());
            }
        }

        authors.push(AuthorInfo {
            fullname: name_part,
            firstname,
            middlename,
            lastname,
            initials,
            address,
        });
    }
    authors
}

/// Parse CSV fields from a line (RFC 4180-like).
/// Fields separated by commas; quoted fields support embedded commas and escaped quotes (`""`).
pub fn parse_csv_fields(line: &str) -> Vec<Cow<'_, str>> {
    let mut fields = Vec::new();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    loop {
        if i >= len {
            // Only add empty field if we haven't added anything yet (empty input)
            if fields.is_empty() {
                fields.push(Cow::Borrowed(""));
            }
            break;
        }

        // Skip leading whitespace
        while i < len && bytes[i] == b' ' {
            i += 1;
        }

        if i < len && bytes[i] == b'"' {
            // Quoted field
            i += 1; // skip opening quote
            let start = i;
            let mut has_escapes = false;
            while i < len {
                if bytes[i] == b'"' {
                    if i + 1 < len && bytes[i + 1] == b'"' {
                        has_escapes = true;
                        i += 2; // skip escaped quote
                    } else {
                        break; // closing quote
                    }
                } else {
                    i += 1;
                }
            }
            let raw = &line[start..i];
            if i < len {
                i += 1; // skip closing quote
            }
            // Skip to comma or end
            while i < len && bytes[i] != b',' {
                i += 1;
            }

            if has_escapes {
                fields.push(Cow::Owned(raw.replace("\"\"", "\"")));
            } else {
                fields.push(Cow::Borrowed(raw));
            }
        } else {
            // Unquoted field
            let start = i;
            while i < len && bytes[i] != b',' {
                i += 1;
            }
            fields.push(Cow::Borrowed(line[start..i].trim()));
        }

        // After field: expect comma or end
        if i < len && bytes[i] == b',' {
            i += 1; // skip comma
            // If comma was last char, add trailing empty field
            if i == len {
                fields.push(Cow::Borrowed(""));
            }
        } else {
            break;
        }
    }

    fields
}

/// Parse DSV fields from a line (colon-separated).
pub fn parse_dsv_fields(line: &str) -> Vec<Cow<'_, str>> {
    line.split(':').map(|f| Cow::Borrowed(f.trim())).collect()
}

/// Parse TSV fields from a line (tab-separated).
pub fn parse_tsv_fields(line: &str) -> Vec<Cow<'_, str>> {
    line.split('\t').map(|f| Cow::Borrowed(f.trim())).collect()
}

// Passthrough-span scanners (`pass:[…]`, `+…+`, `++…++`, `+++…+++`).
//
// Stateless text scanners shared by the inline parser — to skip over passthrough
// regions while matching quote delimiters — and the preprocessor — to leave
// counter macros inside a passthrough literal (Asciidoctor extracts passthroughs
// before the `attributes` substitution that resolves `{counter:…}`).

/// If byte `p` (just past `pass:`) starts an optional subs spec immediately
/// followed by `[`, return the spec's byte length (0 for the bare `pass:[…]`
/// form). `None` when no bracket follows — the macro form does not match and
/// the text stays literal in Asciidoctor (`pass:c` without brackets,
/// an uppercase spec, an empty comma token, …).
pub fn pass_spec_len(s: &str, p: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = p;
    while bytes
        .get(i)
        .is_some_and(|&b| b.is_ascii_lowercase() || b == b',' || b == b'_' || b == b'-')
    {
        i += 1;
    }
    if bytes.get(i) != Some(&b'[') {
        return None;
    }
    let spec = &s[p..i];
    if !spec.is_empty() && !spec.split(',').all(|t| !t.is_empty()) {
        return None;
    }
    Some(i - p)
}

/// If a `pass:[…]`/`pass:SPEC[…]` inline macro begins at byte offset `i` in
/// `s`, return its total byte length (so quote-delimiter scanning can skip
/// over it). Mirrors `try_pass_macro` (content runs to the first `]`).
/// `i` must point at `p`.
pub fn pass_macro_span_len(s: &str, i: usize) -> Option<usize> {
    let rest = &s[i..];
    if !rest.starts_with("pass:") {
        return None;
    }
    let spec_len = pass_spec_len(rest, 5)?;
    let content_start = 5 + spec_len + 1; // past '['
    let close = rest[content_start..].find(']')?;
    Some(content_start + close + 1)
}

/// If a closed `++…++` or `+++…+++` passthrough begins at byte offset `i` in `s`,
/// return its total byte length (so quote-delimiter scanning can skip over it).
/// Mirrors the matching in `try_double_plus_passthrough` / `try_triple_plus_passthrough`
/// (non-empty content, nearest closing delimiter). `i` must point at a `+`.
pub fn passthrough_span_len(s: &str, i: usize) -> Option<usize> {
    let rest = &s[i..];
    if let Some(after) = rest.strip_prefix("+++") {
        let close = after.find("+++")?;
        (close != 0).then_some(3 + close + 3)
    } else if let Some(after) = rest.strip_prefix("++") {
        let close = after.find("++")?;
        (close != 0).then_some(2 + close + 2)
    } else {
        None
    }
}

/// If a *constrained* single-plus passthrough (`+…+`) begins at byte offset
/// `i` in `s`, return its total byte length so quote-delimiter scanning can
/// skip over it. Mirrors the matching in `try_single_plus_passthrough`: the
/// opening `+` must not begin `++`/`+++`, must not follow a word char, the
/// content's first char must not be a space, and the closing `+` must obey
/// the constrained-close rule (not preceded by `+`/space, not followed by
/// `+`/word). A `pass:[…]` macro inside the span is extracted first, so a
/// `+` in its brackets cannot close. `i` must point at a `+`. AsciiDoc
/// extracts these passthroughs before quote substitution, so a quote marker
/// living inside one must not terminate the surrounding span — e.g. in
/// `` `<n>+`x`+y` `` the inner backticks are literal and the outer pair runs
/// from the first backtick to the last.
pub fn single_plus_span_len(s: &str, i: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    // Not a single '+': `++`/`+++` are handled by `passthrough_span_len`.
    if bytes.get(i + 1).copied() == Some(b'+') {
        return None;
    }
    // The opening '+' must not follow a word char (`C+a+` stays literal) nor a
    // backslash (`` `\+` `` is an escaped plus, not a passthrough — the main parse
    // loop consumes the escape before `try_single_plus_passthrough`, but this raw
    // delimiter scan must reject it explicitly). At i == 0 the preceding char is the
    // opening quote marker (`` ` ``/`_`/`#`/`*`), all non-word, so the open is allowed.
    if i > 0 {
        let prev = bytes[i - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'\\' {
            return None;
        }
    }
    // The content's first char must exist and not be a space.
    match bytes.get(i + 1) {
        None | Some(b' ') => return None,
        _ => {}
    }
    // Find the constrained closing '+'.
    let mut j = i + 1;
    while j < bytes.len() {
        let b = bytes[j];
        if b == b'p'
            && let Some(skip) = pass_macro_span_len(s, j)
        {
            j += skip;
            continue;
        }
        if b == b'+' && j > i + 1 {
            let prev = bytes[j - 1];
            let next = bytes.get(j + 1).copied();
            let followed_by_word = next.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_');
            if prev != b'+' && prev != b' ' && next != Some(b'+') && !followed_by_word {
                return Some(j - i + 1);
            }
        }
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_lines() {
        assert_eq!(split_lines("a\nb\nc"), vec!["a", "b", "c"]);
        assert_eq!(split_lines("a\r\nb\r\n"), vec!["a", "b", ""]);
        assert_eq!(split_lines(""), vec![""]);
    }

    #[test]
    fn test_strip_bom() {
        // Leading BOM is removed (zero-copy: result is a sub-slice of the input).
        assert_eq!(strip_bom("\u{feff}= Title"), "= Title");
        // No BOM: returned unchanged.
        assert_eq!(strip_bom("= Title"), "= Title");
        assert_eq!(strip_bom(""), "");
        // Only a single leading BOM; a BOM elsewhere is left intact.
        assert_eq!(strip_bom("a\u{feff}b"), "a\u{feff}b");
        assert_eq!(strip_bom("\u{feff}\u{feff}x"), "\u{feff}x");
    }

    #[test]
    fn test_is_blank() {
        assert!(is_blank(""));
        assert!(is_blank("   "));
        assert!(is_blank("\t"));
        assert!(!is_blank("hello"));
    }

    #[test]
    fn test_strip_section_marker() {
        assert_eq!(strip_section_marker("= Title"), Some((1, "Title")));
        assert_eq!(strip_section_marker("== Sub"), Some((2, "Sub")));
        assert_eq!(strip_section_marker("=NoSpace"), None);
        assert_eq!(strip_section_marker("= "), None);
        // Indented section marker is a literal paragraph, not a section (column 0 only).
        assert_eq!(strip_section_marker(" == Indented"), None);
        assert_eq!(strip_section_marker("  = Two spaces"), None);
        assert_eq!(strip_section_marker("\t== Tab"), None);
    }

    #[test]
    fn test_is_thematic_break() {
        // AsciiDoc apostrophe form and Markdown-style forms.
        for s in ["'''", "---", "***", "___", "- - -", "* * *", "_ _ _", "-  -  -"] {
            assert!(is_thematic_break(s), "expected thematic break: {s:?}");
        }
        // Up to 3 leading spaces allowed; trailing whitespace ignored.
        assert!(is_thematic_break("   ---"));
        assert!(is_thematic_break("---   "));
        // Four leading spaces, four markers, two markers, inconsistent spacing.
        assert!(!is_thematic_break("    ---"));
        assert!(!is_thematic_break("----"));
        assert!(!is_thematic_break("--"));
        assert!(!is_thematic_break("- - - -"));
        assert!(!is_thematic_break("- -  -"));
        assert!(!is_thematic_break("* **"));
        assert!(!is_thematic_break("abc"));
    }

    #[test]
    fn test_is_delimiter() {
        assert_eq!(is_delimiter("----"), Some((DelimiterType::Listing, 4)));
        assert_eq!(is_delimiter("...."), Some((DelimiterType::Literal, 4)));
        assert_eq!(is_delimiter("===="), Some((DelimiterType::Example, 4)));
        assert_eq!(is_delimiter("****"), Some((DelimiterType::Sidebar, 4)));
        assert_eq!(is_delimiter("____"), Some((DelimiterType::Quote, 4)));
        assert_eq!(is_delimiter("--"), Some((DelimiterType::Open, 2)));
        assert_eq!(is_delimiter("////"), Some((DelimiterType::Comment, 4)));
        assert_eq!(is_delimiter("++++"), Some((DelimiterType::Passthrough, 4)));
        assert_eq!(is_delimiter("---"), None);
        assert_eq!(is_delimiter("abc"), None);
    }

    #[test]
    fn test_is_list_marker_unordered() {
        assert_eq!(is_list_marker_unordered("* item"), Some((1, "item")));
        assert_eq!(is_list_marker_unordered("** nested"), Some((2, "nested")));
        assert_eq!(is_list_marker_unordered("*bold*"), None);
        // Hyphen is a distinct marker family → identity 0 (never collides with
        // the `*`-count identities, so `- x` nests under `* y`).
        assert_eq!(is_list_marker_unordered("- item"), Some((0, "item")));
        assert_eq!(is_list_marker_unordered("-no-space"), None);
    }

    #[test]
    fn test_is_list_marker_ordered() {
        assert_eq!(is_list_marker_ordered(". item"), Some((1, "item")));
        assert_eq!(is_list_marker_ordered(".. nested"), Some((2, "nested")));
    }

    #[test]
    fn test_is_admonition() {
        assert_eq!(is_admonition("NOTE: text"), Some(("NOTE", "text")));
        assert_eq!(is_admonition("TIP: text"), Some(("TIP", "text")));
        assert_eq!(is_admonition("NOTHING: text"), None);
    }

    #[test]
    fn test_is_block_image() {
        assert_eq!(
            is_block_image("image::path/to/img.png[Alt text]"),
            Some(("path/to/img.png", "Alt text"))
        );
        // Must end with `]` (BlockMediaMacroRx `\]$`): trailing content after
        // the closing bracket demotes the line to a paragraph.
        assert_eq!(is_block_image("image::sunset.jpg[] <.> <.>"), None);
        assert_eq!(is_block_image("image::sunset.jpg[Alt,200,100] <.>"), None);
        assert_eq!(is_block_image("image::sunset.jpg[]trailing"), None);
        // Trailing whitespace is rstripped before the anchor check.
        assert_eq!(is_block_image("image::x[]  "), Some(("x", "")));
        // Target may contain internal whitespace but not leading/trailing.
        assert_eq!(is_block_image("image::a b[Alt]"), Some(("a b", "Alt")));
        assert_eq!(is_block_image("image:: x[y]"), None);
        assert_eq!(is_block_image("image::x [Alt]"), None);
        // The closing bracket is the last `]`; inner brackets stay in attrs.
        assert_eq!(is_block_image("image::x[a]b]"), Some(("x", "a]b")));
        assert_eq!(is_block_image("image::[]"), None); // empty target
    }

    #[test]
    fn test_is_block_video() {
        assert_eq!(
            is_block_video("video::video.mp4[]"),
            Some(("video.mp4", ""))
        );
        assert_eq!(
            is_block_video("video::video.mp4[width=640,start=60,options=\"autoplay,loop\"]"),
            Some(("video.mp4", "width=640,start=60,options=\"autoplay,loop\""))
        );
        assert_eq!(
            is_block_video("video::path/to/video.mp4[poster=preview.jpg]"),
            Some(("path/to/video.mp4", "poster=preview.jpg"))
        );
        assert_eq!(is_block_video("video::[]"), None); // empty target
        assert_eq!(is_block_video("video::file.mp4"), None); // no brackets
        assert_eq!(is_block_video("not video::file.mp4[]"), None); // not at start
    }

    #[test]
    fn test_is_block_audio() {
        assert_eq!(
            is_block_audio("audio::audio.mp3[]"),
            Some(("audio.mp3", ""))
        );
        assert_eq!(
            is_block_audio("audio::audio.mp3[options=\"autoplay,loop\"]"),
            Some(("audio.mp3", "options=\"autoplay,loop\""))
        );
        assert_eq!(is_block_audio("audio::[]"), None); // empty target
        assert_eq!(is_block_audio("audio::file.mp3"), None); // no brackets
    }

    #[test]
    fn test_generate_id() {
        // Default prefix and separator
        assert_eq!(generate_id("My Title", "_", "_"), "_my_title");
        assert_eq!(generate_id("Hello World!", "_", "_"), "_hello_world");

        // Empty prefix
        assert_eq!(generate_id("My Title", "", "_"), "my_title");

        // Custom separator
        assert_eq!(generate_id("My Title", "_", "-"), "_my-title");

        // Empty prefix + custom separator
        assert_eq!(generate_id("My Title", "", "-"), "my-title");

        // Custom prefix
        assert_eq!(generate_id("My Title", "id-", "_"), "id-my_title");

        // Empty separator (no separator char — falls back to '_')
        assert_eq!(generate_id("My Title", "_", ""), "_my_title");

        // Unicode (Cyrillic) — should lowercase properly
        assert_eq!(generate_id("Базовые Принципы", "_", "_"), "_базовые_принципы");

        // URLs in title — should use link text, not URL
        assert_eq!(
            generate_id("Text https://example.com/api[Open API] more", "_", "_"),
            "_text_open_api_more"
        );

        // Bare URL — should be stripped
        assert_eq!(
            generate_id("Text https://example.com/api more", "_", "_"),
            "_text_more"
        );

        // link: macro
        assert_eq!(
            generate_id("See link:file.html[the docs]", "_", "_"),
            "_see_the_docs"
        );

        // Dots act as word separators (like spaces/hyphens), not dropped.
        // Asciidoctor: "0.3.0 Milestone Build" -> _0_3_0_milestone_build
        assert_eq!(
            generate_id("0.3.0 Milestone Build", "_", "_"),
            "_0_3_0_milestone_build"
        );
        // A run of separators (dot + space) collapses to a single separator.
        assert_eq!(generate_id("Foo. Bar", "_", "_"), "_foo_bar");

        // icon: macro in a heading → uses its alt text (default = basename.tr('_-',' ')),
        // not the literal `icon:name[]`. Mirrors Asciidoctor's macro-substituted title.
        assert_eq!(
            generate_id("icon:fast-forward[] Migration", "_", "_"),
            "_fast_forward_migration"
        );
        assert_eq!(
            generate_id("icon:ticket[] Resolved Issues", "_", "_"),
            "_ticket_resolved_issues"
        );
        // basename drops the extension and the path.
        assert_eq!(generate_id("icon:foo.bar[] X", "_", "_"), "_foo_x");
        assert_eq!(generate_id("icon:path/to/heart[] Y", "_", "_"), "_heart_y");
        // dots in the stem stay (become separators in the id).
        assert_eq!(generate_id("Pre icon:a.b.c[] Post", "_", "_"), "_pre_a_b_post");
        // positional arg is `size`, ignored for alt.
        assert_eq!(generate_id("icon:heart[Big] End", "_", "_"), "_heart_end");
        // explicit alt= wins (quotes are dropped by the char filter).
        assert_eq!(generate_id("icon:heart[alt=\"A B\"] Z", "_", "_"), "_a_b_z");
        // malformed icon macro degrades to literal text.
        assert_eq!(generate_id("icon:noclose Heading", "_", "_"), "_iconnoclose_heading");
    }

    #[test]
    fn test_icon_default_alt() {
        // tr('_-', ' ')
        assert_eq!(icon_default_alt("fast-forward"), "fast forward");
        assert_eq!(icon_default_alt("my_cool_icon"), "my cool icon");
        assert_eq!(icon_default_alt("foo_bar-baz"), "foo bar baz");
        // basename: drop directory and trailing extension.
        assert_eq!(icon_default_alt("foo.bar"), "foo");
        assert_eq!(icon_default_alt("path/to/heart"), "heart");
        // only the LAST extension is dropped.
        assert_eq!(icon_default_alt("a.b.c"), "a.b");
        // a leading dot is not an extension.
        assert_eq!(icon_default_alt(".hidden"), ".hidden");
        // plain name unchanged (identity for the gate's simple icons).
        assert_eq!(icon_default_alt("heart"), "heart");
    }

    #[test]
    fn test_is_attribute_entry() {
        assert_eq!(is_attribute_entry(":key: value"), Some(("key", "value")));
        assert_eq!(is_attribute_entry(":toc:"), Some(("toc", "")));
        assert_eq!(is_attribute_entry("not attr"), None);
        // A `::` after the name is NOT an attribute entry — it's a description-list
        // term (`:context::` → term `:context`). Mirror of AttributeEntryRx: the
        // separator colon must be followed by whitespace or end-of-line.
        assert_eq!(is_attribute_entry(":context:: A single block."), None);
        assert_eq!(is_attribute_entry(":foo:bar:: desc"), None);
        // No-space value (`:key:value`) is also not an attribute entry in Asciidoctor.
        assert_eq!(is_attribute_entry(":key:value"), None);
        // Value whose first char is `:` (after a space) stays a valid entry.
        assert_eq!(is_attribute_entry(":author: :smile:"), Some(("author", ":smile:")));
    }

    #[test]
    fn test_is_block_attribute() {
        assert_eq!(is_block_attribute("[source,rust]"), Some("source,rust"));
        assert_eq!(is_block_attribute("[]"), Some(""));
        assert_eq!(is_block_attribute("[ id=idname ]"), None);
        assert_eq!(is_block_attribute("not block attr"), None);
        // Shorthand/quoted/attr-ref first chars are allowed
        assert_eq!(is_block_attribute("[#id.role]"), Some("#id.role"));
        assert_eq!(is_block_attribute("[.role]"), Some(".role"));
        assert_eq!(is_block_attribute("[%opt]"), Some("%opt"));
        assert_eq!(is_block_attribute("[,ruby]"), Some(",ruby"));
        assert_eq!(is_block_attribute("[\"a\",\"b\"]"), Some("\"a\",\"b\""));
        assert_eq!(is_block_attribute("[{attr}]"), Some("{attr}"));
        // Block anchor: only when the whole line is the anchor
        assert_eq!(is_block_attribute("[[id]]"), Some("[id]"));
        assert_eq!(is_block_attribute("[[id,Some reftext]]"), Some("[id,Some reftext]"));
        // Trailing content after ]] → paragraph with inline anchor, not an attrlist
        assert_eq!(is_block_attribute("[[id]]image:tiger.png[Image of a tiger]"), None);
        assert_eq!(is_block_attribute("[[]]"), None);
    }

    #[test]
    fn test_is_line_comment() {
        assert!(is_line_comment("// this is a comment"));
        assert!(is_line_comment("//"));
        assert!(is_line_comment("///"));
        assert!(!is_line_comment("////"));
        assert!(!is_line_comment("/////"));
        assert!(!is_line_comment("not a comment"));
    }

    #[test]
    fn test_is_block_title() {
        assert_eq!(is_block_title(".My Title"), Some("My Title"));
        assert_eq!(is_block_title("..not"), None);
        assert_eq!(is_block_title(". space"), None);
    }

    #[test]
    fn test_is_list_continuation() {
        assert!(is_list_continuation("+"));
        assert!(is_list_continuation("  +  "));
        assert!(!is_list_continuation("++"));
        assert!(!is_list_continuation("+ text"));
        assert!(!is_list_continuation(""));
        assert!(!is_list_continuation("++++"));
    }

    #[test]
    fn test_is_description_list_marker() {
        assert_eq!(
            is_description_list_marker("CPU:: The brain"),
            Some((1, "CPU", "The brain"))
        );
        assert_eq!(
            is_description_list_marker("Speed::: Fast"),
            Some((2, "Speed", "Fast"))
        );
        assert_eq!(
            is_description_list_marker("Term::::"),
            Some((3, "Term", ""))
        );
        assert_eq!(is_description_list_marker(":: no term"), None);
        assert_eq!(is_description_list_marker("just text"), None);
        assert_eq!(is_description_list_marker("::::: too many"), None);
        assert_eq!(is_description_list_marker("a::b"), None); // no space after ::
        assert_eq!(
            is_description_list_marker("Term::"),
            Some((1, "Term", ""))
        );
        // A term may begin with a colon — the leading colon is part of the term
        // (matches Asciidoctor's `<dt>:context</dt>`).
        assert_eq!(
            is_description_list_marker(":context:: A single block."),
            Some((1, ":context", "A single block."))
        );
        assert_eq!(
            is_description_list_marker(":foo:bar:: desc"),
            Some((1, ":foo:bar", "desc"))
        );
    }

    #[test]
    fn test_is_toc_macro() {
        assert!(is_toc_macro("toc::[]"));
        assert!(is_toc_macro("  toc::[]  "));
        assert!(is_toc_macro("toc::[levels=3]"));
        assert!(!is_toc_macro("toc::"));
        assert!(!is_toc_macro("toc::["));
        assert!(!is_toc_macro(""));
        assert!(!is_toc_macro("something toc::[]"));
        assert!(!is_toc_macro("toc::[]extra"));
    }

    #[test]
    fn test_toc_macro_attrs() {
        assert_eq!(toc_macro_attrs("toc::[]"), Some(""));
        assert_eq!(toc_macro_attrs("  toc::[]  "), Some(""));
        assert_eq!(toc_macro_attrs("toc::[levels=1]"), Some("levels=1"));
        assert_eq!(toc_macro_attrs("toc::"), None);
        assert_eq!(toc_macro_attrs("toc::[levels=1"), None);
        assert_eq!(toc_macro_attrs("not a toc"), None);
    }

    #[test]
    fn test_is_include_directive() {
        assert_eq!(
            is_include_directive("include::file.adoc[]"),
            Some(("file.adoc", ""))
        );
        assert_eq!(
            is_include_directive("include::sub/path.adoc[leveloffset=+1]"),
            Some(("sub/path.adoc", "leveloffset=+1"))
        );
        assert_eq!(
            is_include_directive("include::chapter.adoc[leveloffset=+1,lines=1..10]"),
            Some(("chapter.adoc", "leveloffset=+1,lines=1..10"))
        );
        assert_eq!(is_include_directive("include::[]"), None); // empty path
        assert_eq!(is_include_directive("include::file.adoc"), None); // no brackets
        assert_eq!(is_include_directive("not include::file[]"), None); // not at start
        assert_eq!(is_include_directive("include::file.adoc]["), None); // malformed
        assert_eq!(is_include_directive(""), None);
        // IncludeDirectiveRx is anchored: `]` must end the line; trailing
        // remainder or leading indent makes the line plain text (probe-verified).
        assert_eq!(is_include_directive("include::core.rb[tag=parse] <.>"), None);
        assert_eq!(is_include_directive("  include::file.adoc[]"), None);
        assert_eq!(
            is_include_directive("include::file.adoc[]  "),
            Some(("file.adoc", ""))
        ); // reader lines are right-trimmed
        assert_eq!(
            is_include_directive("include::no pe.adoc[]"),
            Some(("no pe.adoc", ""))
        ); // interior whitespace in target is fine
        assert_eq!(is_include_directive("include:: x.adoc[]"), None); // leading ws in target
    }

    #[test]
    fn test_is_table_delimiter() {
        assert!(is_table_delimiter("|==="));
        assert!(is_table_delimiter("  |===  "));
        assert!(is_table_delimiter("|====")); // 4+ equals also valid (asciidoctor)
        assert!(is_table_delimiter("|=========="));
        assert!(!is_table_delimiter("===="));
        assert!(!is_table_delimiter("|==")); // need at least 3 equals
        assert!(!is_table_delimiter("|")); // pipe alone is not a delimiter
        assert!(!is_table_delimiter("|=== x")); // trailing content disqualifies
        assert!(!is_table_delimiter(""));
        // CSV (`,===`) and DSV (`:===`) shorthand delimiters
        assert!(is_table_delimiter(",==="));
        assert!(is_table_delimiter("  :====  "));
        assert!(!is_table_delimiter(",==")); // need at least 3 equals
        assert!(!is_table_delimiter(":")); // colon alone is not a delimiter
        assert!(!is_table_delimiter(":name: value")); // attribute entry, not a delimiter
        // Nested-table delimiter (`!===`, `!` cell separator)
        assert!(is_table_delimiter("!==="));
        assert!(is_table_delimiter("  !====  "));
        assert!(!is_table_delimiter("!==")); // need at least 3 equals
        assert!(!is_table_delimiter("!")); // bang alone is not a delimiter
    }

    fn cell(content: &str) -> CellSpec<'_> {
        CellSpec { content: Cow::Borrowed(content), duplication: 1, colspan: 1, rowspan: 1, style: CellStyle::Default, style_explicit: false, halign: HAlign::default(), valign: VAlign::default(), halign_explicit: false, valign_explicit: false }
    }

    fn spanned_cell(content: &str, colspan: u8, rowspan: u8) -> CellSpec<'_> {
        CellSpec { content: Cow::Borrowed(content), duplication: 1, colspan, rowspan, style: CellStyle::Default, style_explicit: false, halign: HAlign::default(), valign: VAlign::default(), halign_explicit: false, valign_explicit: false }
    }

    fn styled_cell(content: &str, style: CellStyle) -> CellSpec<'_> {
        CellSpec { content: Cow::Borrowed(content), duplication: 1, colspan: 1, rowspan: 1, style, style_explicit: true, halign: HAlign::default(), valign: VAlign::default(), halign_explicit: false, valign_explicit: false }
    }

    fn spanned_styled_cell(content: &str, colspan: u8, rowspan: u8, style: CellStyle) -> CellSpec<'_> {
        CellSpec { content: Cow::Borrowed(content), duplication: 1, colspan, rowspan, style, style_explicit: true, halign: HAlign::default(), valign: VAlign::default(), halign_explicit: false, valign_explicit: false }
    }

    // The aligned-cell assertions below only use non-default alignment
    // operators, so explicit-ness mirrors "value differs from default".
    fn aligned_cell(content: &str, halign: HAlign, valign: VAlign) -> CellSpec<'_> {
        CellSpec { content: Cow::Borrowed(content), duplication: 1, colspan: 1, rowspan: 1, style: CellStyle::Default, style_explicit: false, halign, valign, halign_explicit: halign != HAlign::default(), valign_explicit: valign != VAlign::default() }
    }

    /// Cells of a parsed table line (continuation ignored), for assertions.
    fn line_cells(line: &str) -> Option<Vec<CellSpec<'_>>> {
        parse_table_cells(line).map(|t| t.cells)
    }

    #[test]
    fn test_parse_table_cells() {
        assert_eq!(
            line_cells("| A | B | C"),
            Some(vec![cell("A"), cell("B"), cell("C")])
        );
        // A trailing `|` opens an (empty) cell — asciidoctor renders it as
        // a bare <td></td>
        assert_eq!(
            line_cells("| A | B |"),
            Some(vec![cell("A"), cell("B"), cell("")])
        );
        assert_eq!(line_cells("no pipe"), None);
        assert_eq!(
            line_cells("| single"),
            Some(vec![cell("single")])
        );
        // Text before the first `|` continues the previous line's last cell
        let t = parse_table_cells("mid |late").unwrap();
        assert_eq!(t.continuation.as_deref(), Some("mid"));
        assert_eq!(t.cells, vec![cell("late")]);
        // A span spec may sit between the continuation text and the `|`
        let t = parse_table_cells("tail 2+|wide").unwrap();
        assert_eq!(t.continuation.as_deref(), Some("tail"));
        assert_eq!(t.cells, vec![spanned_cell("wide", 2, 1)]);
        // A pure spec prefix is not a continuation
        let t = parse_table_cells("2+| x").unwrap();
        assert_eq!(t.continuation, None);
        assert_eq!(t.cells, vec![spanned_cell("x", 2, 1)]);
    }

    #[test]
    fn test_parse_table_cells_bang_separator() {
        // Nested tables (`!===`) split on `!`, not `|`. A literal `|` is then
        // ordinary cell content.
        assert_eq!(
            parse_table_cells_with_sep("! C11 ! C12", b'!').map(|t| t.cells),
            Some(vec![cell("C11"), cell("C12")])
        );
        assert_eq!(
            parse_table_cells_with_sep("! a | b ! c", b'!').map(|t| t.cells),
            Some(vec![cell("a | b"), cell("c")])
        );
        // No `!` → not a `!`-table line (continuation), even with a `|` present
        assert_eq!(parse_table_cells_with_sep("a | b", b'!'), None);
        // `\!` is an escaped separator under the `!` rule
        assert_eq!(
            parse_table_cells_with_sep("!a \\! b !c", b'!').map(|t| t.cells),
            Some(vec![cell("a ! b"), cell("c")])
        );
    }

    #[test]
    fn test_parse_table_cells_escaped_pipe() {
        // `\|` is an escaped separator: no split, one backslash consumed
        assert_eq!(
            line_cells("|a \\| b |c"),
            Some(vec![cell("a | b"), cell("c")])
        );
        // Cell consisting of an escaped table delimiter (delimited.adoc)
        assert_eq!(line_cells("|\\|==="), Some(vec![cell("|===")]));
        // `\\|`: the pipe is still escaped, exactly one backslash consumed
        assert_eq!(line_cells("|a \\\\| b"), Some(vec![cell("a \\| b")]));
        // A line with only escaped pipes is not a table line (pure continuation)
        assert_eq!(parse_table_cells("tail \\| more"), None);
        // Escaped pipe in continuation text before an unescaped separator
        let t = parse_table_cells("tail \\| more |next").unwrap();
        assert_eq!(t.continuation.as_deref(), Some("tail | more"));
        assert_eq!(t.cells, vec![cell("next")]);
    }

    #[test]
    fn test_parse_cell_spec_exact() {
        let sp = parse_cell_spec_exact("2*>m").unwrap();
        assert_eq!((sp.duplication, sp.halign, sp.style), (2, HAlign::Right, CellStyle::Monospace));
        let sp = parse_cell_spec_exact(".3+^.>s").unwrap();
        assert_eq!((sp.colspan, sp.rowspan, sp.halign, sp.valign, sp.style),
                   (1, 3, HAlign::Center, VAlign::Bottom, CellStyle::Strong));
        let sp = parse_cell_spec_exact("2.3+").unwrap();
        assert_eq!((sp.colspan, sp.rowspan), (2, 3));
        // Explicit-alignment flags: `<.>` sets an explicit Left (the default
        // value) and an explicit Bottom — both must be flagged so a cell can
        // override a non-default column alignment (cell.adoc `.3+<.>m`).
        let sp = parse_cell_spec_exact(".3+<.>m").unwrap();
        assert_eq!(
            (sp.halign, sp.valign, sp.halign_explicit, sp.valign_explicit),
            (HAlign::Left, VAlign::Bottom, true, true)
        );
        // A bare span spec carries no explicit alignment.
        let sp = parse_cell_spec_exact("2.3+").unwrap();
        assert_eq!((sp.halign_explicit, sp.valign_explicit), (false, false));
        // Partial matches are not specs
        assert!(parse_cell_spec_exact("mid").is_none());
        assert!(parse_cell_spec_exact("x2+").is_none());
        assert!(parse_cell_spec_exact("").is_none());
    }

    #[test]
    fn test_parse_table_cells_duplication() {
        // `2*|x` keeps the factor unexpanded (the cell may still grow via
        // continuation lines; the block scanner expands copies later)
        let t = parse_table_cells("2*>m|dup").unwrap();
        assert_eq!(t.cells.len(), 1);
        let c = &t.cells[0];
        assert_eq!((c.content.as_ref(), c.duplication, c.halign, c.style),
                   ("dup", 2, HAlign::Right, CellStyle::Monospace));
    }

    #[test]
    fn test_parse_span_spec() {
        // No spec
        assert_eq!(parse_span_spec("hello"), ("hello", 1, 1));
        // Colspan only
        assert_eq!(parse_span_spec("2+"), ("", 2, 1));
        // Rowspan only
        assert_eq!(parse_span_spec(".3+"), ("", 1, 3));
        // Both
        assert_eq!(parse_span_spec("2.3+"), ("", 2, 3));
        // With content before
        assert_eq!(parse_span_spec("text 2+"), ("text", 2, 1));
        // Plain `+` is not a span spec
        assert_eq!(parse_span_spec("+"), ("+", 1, 1));
    }

    #[test]
    fn test_parse_table_cells_with_colspan() {
        assert_eq!(
            line_cells("| A 2+| B spans"),
            Some(vec![cell("A"), spanned_cell("B spans", 2, 1)])
        );
    }

    #[test]
    fn test_parse_table_cells_with_rowspan() {
        assert_eq!(
            line_cells(".2+| C spans | D"),
            Some(vec![spanned_cell("C spans", 1, 2), cell("D")])
        );
    }

    #[test]
    fn test_parse_table_cells_with_both_spans() {
        assert_eq!(
            line_cells("2.3+| cell"),
            Some(vec![spanned_cell("cell", 2, 3)])
        );
    }

    #[test]
    fn test_strip_callout_markers() {
        use super::CalloutMarker::*;
        // Compare against the borrowed/owned text uniformly via `as_ref`.
        let check = |input: &str, text: &str, markers: Vec<CalloutMarker>| {
            let (t, m) = strip_callout_markers(input);
            assert_eq!((t.as_ref(), m), (text, markers), "input={input:?}");
        };
        // Standard numbered
        check("require 'sinatra' <1>", "require 'sinatra' ", vec![Standard(1)]);
        check("code <1> <2>", "code ", vec![Standard(1), Standard(2)]);
        check("no callouts", "no callouts", vec![]);
        check("<1>", "", vec![Standard(1)]);
        check("code <12>", "code ", vec![Standard(12)]);
        // Autonumbered
        check("code <.>", "code ", vec![Standard(0)]);
        check("code <.> <.>", "code ", vec![Standard(0), Standard(0)]);
        // XML comment callouts
        check("code <!--1-->", "code ", vec![XmlComment(1)]);
        check("code <!--.-->", "code ", vec![XmlComment(0)]);
        check(
            "  <title>Title</title> <!--1-->",
            "  <title>Title</title> ",
            vec![XmlComment(1)],
        );
        // Escaped markers (`\<N>` / `\<!--N-->`): NOT a conum — the backslash is
        // dropped and the marker stays literal (Asciidoctor `CalloutSourceRx`).
        check("x = 1 \\<1>", "x = 1 <1>", vec![]);
        check("// \\<1>", "// <1>", vec![]);
        check("\\<1>", "<1>", vec![]);
        check("  <title>T</title> \\<!--1-->", "  <title>T</title> <!--1-->", vec![]);
        // Escaped marker to the LEFT of a real one: the run stops at the escape;
        // the real (right) marker is still a conum, the escaped one stays literal.
        check("code \\<1> <2>", "code <1> ", vec![Standard(2)]);
    }

    #[test]
    fn test_callout_guard_offset() {
        // Comment token + single space → guard starts at the token, keeps the
        // trailing space in the slice; the space before the token stays in the
        // preceding text.
        let s = "require 'asciidoctor' # ";
        assert_eq!(&s[callout_guard_offset(s)..], "# ");
        let s = "get '/hi' do // ";
        assert_eq!(&s[callout_guard_offset(s)..], "// ");
        // No trailing space (marker was directly adjacent).
        let s = "foo ;;";
        assert_eq!(&s[callout_guard_offset(s)..], ";;");
        let s = "x--";
        assert_eq!(&s[callout_guard_offset(s)..], "--");
        let s = "code #";
        assert_eq!(&s[callout_guard_offset(s)..], "#");
        // Two spaces before the marker → not a guard (Asciidoctor allows one).
        let s = "x #  ";
        assert_eq!(callout_guard_offset(s), s.len());
        // No comment token.
        let s = "plain text ";
        assert_eq!(callout_guard_offset(s), s.len());
        assert_eq!(callout_guard_offset(""), 0);
    }

    #[test]
    fn test_is_callout_list_item() {
        assert_eq!(
            is_callout_list_item("<1> Library import"),
            Some((1, "Library import"))
        );
        assert_eq!(
            is_callout_list_item("<12> Two digits"),
            Some((12, "Two digits"))
        );
        assert_eq!(is_callout_list_item("not a callout"), None);
        assert_eq!(is_callout_list_item("<1>no space"), None);
        assert_eq!(
            is_callout_list_item("<3>"),
            Some((3, ""))
        );
    }

    #[test]
    fn test_parse_checklist_marker() {
        assert_eq!(parse_checklist_marker("[x] Task"), (Some(true), "Task"));
        assert_eq!(parse_checklist_marker("[ ] Task"), (Some(false), "Task"));
        assert_eq!(parse_checklist_marker("Regular"), (None, "Regular"));
        assert_eq!(parse_checklist_marker("[x]no space"), (None, "[x]no space"));
        assert_eq!(parse_checklist_marker("[ ]no space"), (None, "[ ]no space"));
        assert_eq!(parse_checklist_marker("[x] "), (Some(true), ""));
    }

    #[test]
    fn test_strip_line_continuation_soft() {
        assert_eq!(
            strip_line_continuation("value \\"),
            Some(("value", false))
        );
    }

    #[test]
    fn test_strip_line_continuation_hard() {
        assert_eq!(
            strip_line_continuation("value + \\"),
            Some(("value", true))
        );
    }

    #[test]
    fn test_strip_line_continuation_none() {
        assert_eq!(strip_line_continuation("value"), None);
    }

    #[test]
    fn test_strip_line_continuation_no_space() {
        assert_eq!(strip_line_continuation("value\\"), None);
    }

    #[test]
    fn test_strip_line_continuation_empty_prefix() {
        assert_eq!(
            strip_line_continuation(" \\"),
            Some(("", false))
        );
    }

    #[test]
    fn test_parse_cell_style_suffix() {
        // Standalone style letter
        assert_eq!(parse_cell_style_suffix("a"), ("", CellStyle::AsciiDoc, true));
        assert_eq!(parse_cell_style_suffix("h"), ("", CellStyle::Header, true));
        assert_eq!(parse_cell_style_suffix("e"), ("", CellStyle::Emphasis, true));
        assert_eq!(parse_cell_style_suffix("m"), ("", CellStyle::Monospace, true));
        assert_eq!(parse_cell_style_suffix("s"), ("", CellStyle::Strong, true));
        assert_eq!(parse_cell_style_suffix("l"), ("", CellStyle::Literal, true));
        // Explicit default (d) and verse (v) consume the spec char too
        assert_eq!(parse_cell_style_suffix("d"), ("", CellStyle::Default, true));
        assert_eq!(parse_cell_style_suffix("v"), ("", CellStyle::Default, true));
        // After span spec (ends with +)
        assert_eq!(parse_cell_style_suffix("2+a"), ("2+", CellStyle::AsciiDoc, true));
        assert_eq!(parse_cell_style_suffix("2.3+s"), ("2.3+", CellStyle::Strong, true));
        // Content ending with style letter — NOT a style
        assert_eq!(parse_cell_style_suffix("data"), ("data", CellStyle::Default, false));
        assert_eq!(parse_cell_style_suffix("date"), ("date", CellStyle::Default, false));
        // Empty
        assert_eq!(parse_cell_style_suffix(""), ("", CellStyle::Default, false));
        // Trailing space — last char is space, not a style letter
        assert_eq!(parse_cell_style_suffix(" a "), (" a ", CellStyle::Default, false));
        assert_eq!(parse_cell_style_suffix(" data "), (" data ", CellStyle::Default, false));
        // Style letter preceded by space (content + space + style)
        assert_eq!(parse_cell_style_suffix(" A e"), (" A ", CellStyle::Emphasis, true));
        assert_eq!(parse_cell_style_suffix(" text s"), (" text ", CellStyle::Strong, true));
    }

    #[test]
    fn test_parse_table_cells_with_style() {
        assert_eq!(
            line_cells("e| italic text"),
            Some(vec![styled_cell("italic text", CellStyle::Emphasis)])
        );
        assert_eq!(
            line_cells("s| bold text"),
            Some(vec![styled_cell("bold text", CellStyle::Strong)])
        );
        assert_eq!(
            line_cells("h| header text"),
            Some(vec![styled_cell("header text", CellStyle::Header)])
        );
        assert_eq!(
            line_cells("m| mono text"),
            Some(vec![styled_cell("mono text", CellStyle::Monospace)])
        );
        assert_eq!(
            line_cells("l| literal text"),
            Some(vec![styled_cell("literal text", CellStyle::Literal)])
        );
        assert_eq!(
            line_cells("a| asciidoc text"),
            Some(vec![styled_cell("asciidoc text", CellStyle::AsciiDoc)])
        );
    }

    #[test]
    fn test_parse_table_cells_span_plus_style() {
        assert_eq!(
            line_cells("2+e| wide italic"),
            Some(vec![spanned_styled_cell("wide italic", 2, 1, CellStyle::Emphasis)])
        );
        assert_eq!(
            line_cells("2.3+s| big bold"),
            Some(vec![spanned_styled_cell("big bold", 2, 3, CellStyle::Strong)])
        );
    }

    #[test]
    fn test_parse_table_cells_style_disambiguation() {
        // Content that ends with a style letter should NOT be treated as styled
        assert_eq!(
            line_cells("| data | more"),
            Some(vec![cell("data"), cell("more")])
        );
        // Inline style between cells
        assert_eq!(
            line_cells("| A e| B | C"),
            Some(vec![cell("A"), styled_cell("B", CellStyle::Emphasis), cell("C")])
        );
    }

    #[test]
    fn test_parse_table_cells_trailing_letter_is_content() {
        // A spec only attaches to a following `|`; at end of line a single
        // style letter (or span/align chars) is plain cell content.
        assert_eq!(line_cells("|a"), Some(vec![cell("a")]));
        assert_eq!(line_cells("|d |e"), Some(vec![cell("d"), cell("e")]));
        assert_eq!(line_cells("|text 2+"), Some(vec![cell("text 2+")]));
        assert_eq!(line_cells("|x ^"), Some(vec![cell("x ^")]));
        // ...while mid-line the trailing letter is the NEXT cell's style
        assert_eq!(
            line_cells("|one a|two"),
            Some(vec![cell("one"), styled_cell("two", CellStyle::AsciiDoc)])
        );
    }

    #[test]
    fn test_parse_cell_align_prefix_halign() {
        assert_eq!(parse_cell_align_prefix("^"), ("", HAlign::Center, VAlign::Top, true, false));
        assert_eq!(parse_cell_align_prefix("<"), ("", HAlign::Left, VAlign::Top, true, false));
        assert_eq!(parse_cell_align_prefix(">"), ("", HAlign::Right, VAlign::Top, true, false));
    }

    #[test]
    fn test_parse_cell_align_prefix_valign() {
        assert_eq!(parse_cell_align_prefix(".<"), ("", HAlign::Left, VAlign::Top, false, true));
        assert_eq!(parse_cell_align_prefix(".^"), ("", HAlign::Left, VAlign::Middle, false, true));
        assert_eq!(parse_cell_align_prefix(".>"), ("", HAlign::Left, VAlign::Bottom, false, true));
    }

    #[test]
    fn test_parse_cell_align_prefix_combined() {
        assert_eq!(parse_cell_align_prefix("^.>"), ("", HAlign::Center, VAlign::Bottom, true, true));
        assert_eq!(parse_cell_align_prefix(">.^rest"), ("rest", HAlign::Right, VAlign::Middle, true, true));
    }

    #[test]
    fn test_parse_cell_align_prefix_no_align() {
        assert_eq!(parse_cell_align_prefix(""), ("", HAlign::Left, VAlign::Top, false, false));
        assert_eq!(parse_cell_align_prefix("text"), ("text", HAlign::Left, VAlign::Top, false, false));
    }

    #[test]
    fn test_parse_table_cells_with_halign() {
        assert_eq!(
            line_cells("^| centered"),
            Some(vec![aligned_cell("centered", HAlign::Center, VAlign::Top)])
        );
        assert_eq!(
            line_cells(">| right"),
            Some(vec![aligned_cell("right", HAlign::Right, VAlign::Top)])
        );
    }

    #[test]
    fn test_parse_table_cells_with_valign() {
        assert_eq!(
            line_cells(".^| middle"),
            Some(vec![aligned_cell("middle", HAlign::Left, VAlign::Middle)])
        );
    }

    #[test]
    fn test_parse_table_cells_with_combined_align() {
        assert_eq!(
            line_cells(">.^| right-middle"),
            Some(vec![aligned_cell("right-middle", HAlign::Right, VAlign::Middle)])
        );
    }

    #[test]
    fn test_parse_table_cells_align_between_pipes() {
        assert_eq!(
            line_cells("| A ^| B | C"),
            Some(vec![
                cell("A"),
                aligned_cell("B", HAlign::Center, VAlign::Top),
                cell("C"),
            ])
        );
    }

    #[test]
    fn test_parse_authors_single() {
        let authors = parse_authors("John Doe <john@example.com>");
        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].fullname, "John Doe");
        assert_eq!(authors[0].firstname, "John");
        assert_eq!(authors[0].middlename, "");
        assert_eq!(authors[0].lastname, "Doe");
        assert_eq!(authors[0].initials, "JD");
        assert_eq!(authors[0].address, "john@example.com");
    }

    #[test]
    fn test_parse_authors_multiple() {
        let authors = parse_authors("John Doe; Jane Smith <jane@example.com>");
        assert_eq!(authors.len(), 2);
        assert_eq!(authors[0].fullname, "John Doe");
        assert_eq!(authors[0].firstname, "John");
        assert_eq!(authors[0].lastname, "Doe");
        assert_eq!(authors[0].address, "");
        assert_eq!(authors[1].fullname, "Jane Smith");
        assert_eq!(authors[1].firstname, "Jane");
        assert_eq!(authors[1].lastname, "Smith");
        assert_eq!(authors[1].initials, "JS");
        assert_eq!(authors[1].address, "jane@example.com");
    }

    #[test]
    fn test_parse_revision_line_all_fields() {
        let rev = parse_revision_line("v1.0, 2024-01-01: Initial release").unwrap();
        assert_eq!(rev.version, Some("1.0"));
        assert_eq!(rev.date, "2024-01-01");
        assert_eq!(rev.remark, Some("Initial release"));
    }

    #[test]
    fn test_parse_revision_line_version_and_date() {
        let rev = parse_revision_line("v1.0, 2024-01-01").unwrap();
        assert_eq!(rev.version, Some("1.0"));
        assert_eq!(rev.date, "2024-01-01");
        assert_eq!(rev.remark, None);
    }

    #[test]
    fn test_parse_revision_line_version_only() {
        let rev = parse_revision_line("v1.0").unwrap();
        assert_eq!(rev.version, Some("1.0"));
        assert_eq!(rev.date, "");
        assert_eq!(rev.remark, None);
    }

    #[test]
    fn test_parse_revision_line_date_and_remark() {
        let rev = parse_revision_line("2024-01-01: Initial release").unwrap();
        assert_eq!(rev.version, None);
        assert_eq!(rev.date, "2024-01-01");
        assert_eq!(rev.remark, Some("Initial release"));
    }

    #[test]
    fn test_parse_revision_line_date_only() {
        let rev = parse_revision_line("2024-01-01").unwrap();
        assert_eq!(rev.version, None);
        assert_eq!(rev.date, "2024-01-01");
        assert_eq!(rev.remark, None);
    }

    #[test]
    fn test_parse_revision_line_version_and_remark() {
        let rev = parse_revision_line("v1.0: Some remark").unwrap();
        assert_eq!(rev.version, Some("1.0"));
        assert_eq!(rev.date, "");
        assert_eq!(rev.remark, Some("Some remark"));
    }

    #[test]
    fn test_parse_revision_line_uppercase_v() {
        // With a comma the capture starts at the first digit, so `V2.5` still
        // yields `2.5`…
        let rev = parse_revision_line("V2.5, 2024-06-15: Release notes").unwrap();
        assert_eq!(rev.version, Some("2.5"));
        assert_eq!(rev.date, "2024-06-15");
        assert_eq!(rev.remark, Some("Release notes"));
        // …but without one only a lowercase `v` head marks a version
        // (`component.start_with? 'v'`); uppercase falls through to the date.
        let rev = parse_revision_line("V2.0").unwrap();
        assert_eq!(rev.version, None);
        assert_eq!(rev.date, "V2.0");
    }

    #[test]
    fn test_parse_revision_line_nondigit_prefix() {
        // Leading non-digit run is stripped from the revision number (`\D*`).
        let rev = parse_revision_line("LPR55, 2024: rem").unwrap();
        assert_eq!(rev.version, Some("55"));
        assert_eq!(rev.date, "2024");
        assert_eq!(rev.remark, Some("rem"));
        // Internal letters/spaces are kept; only the leading run is stripped.
        let rev = parse_revision_line("Version 2.5 RC1, 2024: x").unwrap();
        assert_eq!(rev.version, Some("2.5 RC1"));
        // A date with an internal comma survives (only the first comma splits).
        let rev = parse_revision_line("v8.3, July 29, 2025: Summertime!").unwrap();
        assert_eq!(rev.version, Some("8.3"));
        assert_eq!(rev.date, "July 29, 2025");
        assert_eq!(rev.remark, Some("Summertime!"));
    }

    #[test]
    fn test_parse_revision_line_freeform() {
        // Probe-verified against Asciidoctor: a freeform line is the revdate.
        let rev = parse_revision_line("hazards a team must vanquish \\").unwrap();
        assert_eq!(rev.version, None);
        assert_eq!(rev.date, "hazards a team must vanquish \\");
        assert_eq!(rev.remark, None);
        // A comma with no digits before it SETS an empty version (`version ,`).
        let rev = parse_revision_line("hello, world").unwrap();
        assert_eq!(rev.version, Some(""));
        assert_eq!(rev.date, "world");
        // `component.slice 1` is taken literally: `version 5` → `ersion 5`.
        let rev = parse_revision_line("version 5").unwrap();
        assert_eq!(rev.version, Some("ersion 5"));
        assert_eq!(rev.date, "");
        // A trailing bare colon sets an EMPTY remark (renders an empty span).
        let rev = parse_revision_line("2020-01-01:").unwrap();
        assert_eq!(rev.date, "2020-01-01");
        assert_eq!(rev.remark, Some(""));
        // A line whose component starts with a colon is thrown back.
        assert!(parse_revision_line(":weird").is_none());
    }

    #[test]
    fn test_parse_revision_line_empty() {
        assert!(parse_revision_line("").is_none());
        assert!(parse_revision_line("   ").is_none());
    }

    #[test]
    fn test_parse_csv_fields_simple() {
        let fields = parse_csv_fields("a,b,c");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_csv_fields_quoted() {
        let fields = parse_csv_fields("\"a,b\",c");
        assert_eq!(fields, vec!["a,b", "c"]);
    }

    #[test]
    fn test_parse_csv_fields_escaped_quotes() {
        let fields = parse_csv_fields("\"a\"\"b\",c");
        assert_eq!(fields, vec!["a\"b", "c"]);
    }

    #[test]
    fn test_parse_csv_fields_empty() {
        let fields = parse_csv_fields("a,,c");
        assert_eq!(fields, vec!["a", "", "c"]);
    }

    #[test]
    fn test_parse_csv_fields_single() {
        let fields = parse_csv_fields("hello");
        assert_eq!(fields, vec!["hello"]);
    }

    #[test]
    fn test_parse_csv_fields_trailing_comma() {
        let fields = parse_csv_fields("a,b,");
        assert_eq!(fields, vec!["a", "b", ""]);
    }

    #[test]
    fn test_parse_dsv_fields() {
        let fields = parse_dsv_fields("a:b:c");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_dsv_fields_with_spaces() {
        let fields = parse_dsv_fields("a : b : c");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_tsv_fields() {
        let fields = parse_tsv_fields("a\tb\tc");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_tsv_fields_with_spaces() {
        let fields = parse_tsv_fields("a \t b \t c");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_strip_markdown_heading() {
        assert_eq!(strip_markdown_heading("# Title"), Some((1, "Title")));
        assert_eq!(strip_markdown_heading("## Sub"), Some((2, "Sub")));
        assert_eq!(strip_markdown_heading("### Level 3"), Some((3, "Level 3")));
        assert_eq!(strip_markdown_heading("#### Level 4"), Some((4, "Level 4")));
        assert_eq!(strip_markdown_heading("##### Level 5"), Some((5, "Level 5")));
        assert_eq!(strip_markdown_heading("###### Level 6"), Some((6, "Level 6")));
        assert_eq!(strip_markdown_heading("####### Too deep"), None);
        assert_eq!(strip_markdown_heading("#NoSpace"), None);
        assert_eq!(strip_markdown_heading("# "), None); // empty title
        assert_eq!(strip_markdown_heading("not heading"), None);
        // Indented ATX heading is a literal paragraph, not a heading (column 0 only).
        assert_eq!(strip_markdown_heading("  ## Indented"), None);
        assert_eq!(strip_markdown_heading(" # One space"), None);
    }

    #[test]
    fn test_is_markdown_code_fence() {
        // Basic opening fence
        assert_eq!(is_markdown_code_fence("```"), Some((3, None)));
        // With language
        assert_eq!(is_markdown_code_fence("```rust"), Some((3, Some("rust"))));
        // 4+ backticks
        assert_eq!(is_markdown_code_fence("````"), Some((4, None)));
        assert_eq!(is_markdown_code_fence("````python"), Some((4, Some("python"))));
        // Trailing whitespace
        assert_eq!(is_markdown_code_fence("```  "), Some((3, None)));
        assert_eq!(is_markdown_code_fence("```rust  "), Some((3, Some("rust"))));
        // Not enough backticks
        assert_eq!(is_markdown_code_fence("``"), None);
        assert_eq!(is_markdown_code_fence("`"), None);
        // Backticks in info string (rejected per CommonMark)
        assert_eq!(is_markdown_code_fence("``` foo`bar"), None);
        // Empty string
        assert_eq!(is_markdown_code_fence(""), None);
    }

    #[test]
    fn test_strip_any_section_marker() {
        // AsciiDoc headings have priority
        assert_eq!(strip_any_section_marker("= Title"), Some((1, "Title")));
        assert_eq!(strip_any_section_marker("== Sub"), Some((2, "Sub")));
        // Markdown headings work too
        assert_eq!(strip_any_section_marker("# Title"), Some((1, "Title")));
        assert_eq!(strip_any_section_marker("## Sub"), Some((2, "Sub")));
        // Neither
        assert_eq!(strip_any_section_marker("just text"), None);
        // Indented markers fall through to a literal paragraph (column 0 only).
        assert_eq!(strip_any_section_marker(" == Indented"), None);
        assert_eq!(strip_any_section_marker("  ## Indented"), None);
    }
}
