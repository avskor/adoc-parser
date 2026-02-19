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
    let trimmed = line.trim_start();
    let level = count_leading(trimmed, '=');
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
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
    line.trim() == "'''"
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
        // Reject if content starts with space (e.g. `[ id=idname ]`)
        if !inner.is_empty() && inner.starts_with(' ') {
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
    let name = &rest[..end];
    let value = rest[end + 1..].trim_start();
    Some((name, value))
}

pub fn is_list_marker_unordered(line: &str) -> Option<(u8, &str)> {
    let trimmed = line.trim_start();
    // Hyphen marker: `- text` (depth 1)
    if let Some(rest) = trimmed.strip_prefix("- ") {
        let text = rest.trim_start();
        if text.is_empty() {
            return None;
        }
        return Some((1, text));
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

pub fn is_block_image(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("image::")?;
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.rfind(']')?;
    if bracket_end <= bracket_start {
        return None;
    }
    let target = &rest[..bracket_start];
    // Empty target: not a valid block image
    if target.is_empty() {
        return None;
    }
    let alt = &rest[bracket_start + 1..bracket_end];
    Some((target, alt))
}

pub fn is_block_video(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("video::")?;
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.rfind(']')?;
    if bracket_end < bracket_start {
        return None;
    }
    let target = &rest[..bracket_start];
    if target.is_empty() {
        return None;
    }
    let attrs = &rest[bracket_start + 1..bracket_end];
    Some((target, attrs))
}

pub fn is_block_audio(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("audio::")?;
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.rfind(']')?;
    if bracket_end < bracket_start {
        return None;
    }
    let target = &rest[..bracket_start];
    if target.is_empty() {
        return None;
    }
    let attrs = &rest[bracket_start + 1..bracket_end];
    Some((target, attrs))
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

pub fn is_toc_macro(line: &str) -> bool {
    line.trim() == "toc::[]"
}

pub fn is_include_directive(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("include::")?;
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.rfind(']')?;
    if bracket_end <= bracket_start {
        return None;
    }
    let path = &rest[..bracket_start];
    if path.is_empty() {
        return None;
    }
    let attrs = &rest[bracket_start + 1..bracket_end];
    Some((path, attrs))
}

pub fn strip_callout_markers(line: &str) -> (&str, Vec<u32>) {
    let mut numbers = Vec::new();
    let mut end = line.len();

    loop {
        let trimmed = line[..end].trim_end();
        if !trimmed.ends_with('>') {
            break;
        }
        let open = match trimmed[..trimmed.len() - 1].rfind('<') {
            Some(pos) => pos,
            None => break,
        };
        let digits = &trimmed[open + 1..trimmed.len() - 1];
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        match digits.parse::<u32>() {
            Ok(n) => {
                numbers.push(n);
                end = open;
            }
            Err(_) => break,
        }
    }

    numbers.reverse();
    (&line[..end], numbers)
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
    } else if let Some(rest) = text.strip_prefix("[ ] ") {
        (Some(false), rest)
    } else {
        (None, text)
    }
}

pub fn is_table_delimiter(line: &str) -> bool {
    line.trim() == "|==="
}

use crate::event::{CellStyle, HAlign, VAlign};

#[derive(Debug, Clone, PartialEq)]
pub struct CellSpec<'a> {
    pub content: &'a str,
    pub colspan: u8,
    pub rowspan: u8,
    pub style: CellStyle,
    pub halign: HAlign,
    pub valign: VAlign,
}

/// Parse alignment prefix from a cell specifier (prefix before first `|`).
/// Reads `[<^>]` for halign, then `.[<^>]` for valign from the beginning.
/// Returns `(remaining, halign, valign)`.
pub fn parse_cell_align_prefix(s: &str) -> (&str, HAlign, VAlign) {
    let mut rest = s;
    let mut halign = HAlign::default();
    let mut valign = VAlign::default();

    // Parse halign: <, ^, >
    if let Some(stripped) = rest.strip_prefix('<') {
        halign = HAlign::Left;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('^') {
        halign = HAlign::Center;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('>') {
        halign = HAlign::Right;
        rest = stripped;
    }

    // Parse valign: .<, .^, .>
    if let Some(stripped) = rest.strip_prefix(".<") {
        valign = VAlign::Top;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix(".^") {
        valign = VAlign::Middle;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix(".>") {
        valign = VAlign::Bottom;
        rest = stripped;
    }

    (rest, halign, valign)
}

/// Parse alignment suffix from the end of a segment (content between pipes).
/// The alignment spec for the NEXT cell sits at the END of the segment, after content.
/// Pattern at end: `[<^>]` for halign, then `.[<^>]` for valign.
/// Only valid if preceded by space or at start of string.
/// Returns `(remaining_content, halign, valign)`.
pub fn parse_cell_align_suffix(s: &str) -> (&str, HAlign, VAlign) {
    let trimmed = s.trim_end();
    if trimmed.is_empty() {
        return (s, HAlign::default(), VAlign::default());
    }

    let bytes = trimmed.as_bytes();
    let mut end = trimmed.len();
    let mut halign = HAlign::default();
    let mut valign = VAlign::default();
    let mut found = false;

    // Try to parse valign from end: .< .^ .>
    if end >= 2 && bytes[end - 2] == b'.' {
        match bytes[end - 1] {
            b'<' => { valign = VAlign::Top; end -= 2; found = true; }
            b'^' => { valign = VAlign::Middle; end -= 2; found = true; }
            b'>' => { valign = VAlign::Bottom; end -= 2; found = true; }
            _ => {}
        }
    }

    // Try to parse halign from end: < ^ >
    if end >= 1 {
        match bytes[end - 1] {
            b'<' => { halign = HAlign::Left; end -= 1; found = true; }
            b'^' => { halign = HAlign::Center; end -= 1; found = true; }
            b'>' => { halign = HAlign::Right; end -= 1; found = true; }
            _ => {}
        }
    }

    // Only valid if preceded by space or at start of string
    if found {
        let remaining = &trimmed[..end];
        if remaining.is_empty() || remaining.ends_with(' ') {
            return (&s[..s.len() - (trimmed.len() - end)], halign, valign);
        }
    }

    (s, HAlign::default(), VAlign::default())
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
/// `m` (Monospace), `s` (Strong), `l` (Literal).
/// The style char is valid only if it is the last char and before it
/// is either nothing or a `+` (part of span spec).
pub fn parse_cell_style_suffix(s: &str) -> (&str, CellStyle) {
    if s.is_empty() {
        return (s, CellStyle::Default);
    }
    let last_byte = s.as_bytes()[s.len() - 1];
    let style = match last_byte {
        b'a' => CellStyle::AsciiDoc,
        b'h' => CellStyle::Header,
        b'e' => CellStyle::Emphasis,
        b'm' => CellStyle::Monospace,
        b's' => CellStyle::Strong,
        b'l' => CellStyle::Literal,
        _ => return (s, CellStyle::Default),
    };
    let before = &s[..s.len() - 1];
    let before_trimmed = before.trim();
    if before_trimmed.is_empty() || before_trimmed.ends_with('+') || before.ends_with(' ') {
        (&s[..s.len() - 1], style)
    } else {
        (s, CellStyle::Default)
    }
}

pub fn parse_table_cells(line: &str) -> Option<Vec<CellSpec<'_>>> {
    let trimmed = line.trim_start();

    // Find the first pipe — if none, not a table line
    let first_pipe = trimmed.find('|')?;

    // Before first pipe: must be empty or a valid align+style+span spec (for the first cell)
    let prefix = trimmed[..first_pipe].trim();
    let mut pending_colspan: u8 = 1;
    let mut pending_rowspan: u8 = 1;
    let mut pending_style = CellStyle::Default;
    let mut pending_halign = HAlign::default();
    let mut pending_valign = VAlign::default();

    if !prefix.is_empty() {
        let (after_align, halign, valign) = parse_cell_align_prefix(prefix);
        let (after_style, style) = parse_cell_style_suffix(after_align);
        let (remaining, cs, rs) = parse_span_spec(after_style);
        if !remaining.trim().is_empty() {
            return None; // Not a table line — non-spec content before first pipe
        }
        pending_colspan = cs;
        pending_rowspan = rs;
        pending_style = style;
        pending_halign = halign;
        pending_valign = valign;
    }

    let mut cells = Vec::new();
    let parts: Vec<&str> = trimmed[first_pipe + 1..].split('|').collect();

    for (i, part) in parts.iter().enumerate() {
        // Parse next-cell specs from END: style, then span, then alignment
        let (after_style, next_style) = parse_cell_style_suffix(part);
        let (after_span, next_colspan, next_rowspan) = parse_span_spec(after_style);
        let (content, next_halign, next_valign) = parse_cell_align_suffix(after_span);
        let content = content.trim();

        if !content.is_empty() {
            cells.push(CellSpec {
                content,
                colspan: pending_colspan,
                rowspan: pending_rowspan,
                style: pending_style,
                halign: pending_halign,
                valign: pending_valign,
            });
        } else if i < parts.len() - 1 {
            // Empty cell between pipes — skip (preserving old behavior)
        }

        pending_colspan = next_colspan;
        pending_rowspan = next_rowspan;
        pending_style = next_style;
        pending_halign = next_halign;
        pending_valign = next_valign;
    }

    Some(cells)
}

pub fn generate_id(title: &str) -> String {
    let mut id = String::with_capacity(title.len() + 1);
    id.push('_');
    let mut prev_was_separator = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
            prev_was_separator = false;
        } else if (ch == ' ' || ch == '-' || ch == '_')
            && !prev_was_separator {
                id.push('_');
                prev_was_separator = true;
        }
    }
    if id.ends_with('_') && id.len() > 1 {
        id.pop();
    }
    id
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
        assert_eq!(generate_id("My Title"), "_my_title");
        assert_eq!(generate_id("Hello World!"), "_hello_world");
    }

    #[test]
    fn test_is_attribute_entry() {
        assert_eq!(is_attribute_entry(":key: value"), Some(("key", "value")));
        assert_eq!(is_attribute_entry(":toc:"), Some(("toc", "")));
        assert_eq!(is_attribute_entry("not attr"), None);
    }

    #[test]
    fn test_is_block_attribute() {
        assert_eq!(is_block_attribute("[source,rust]"), Some("source,rust"));
        assert_eq!(is_block_attribute("[]"), Some(""));
        assert_eq!(is_block_attribute("[ id=idname ]"), None);
        assert_eq!(is_block_attribute("not block attr"), None);
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
    }

    #[test]
    fn test_is_toc_macro() {
        assert!(is_toc_macro("toc::[]"));
        assert!(is_toc_macro("  toc::[]  "));
        assert!(!is_toc_macro("toc::"));
        assert!(!is_toc_macro("toc::[levels=3]"));
        assert!(!is_toc_macro(""));
        assert!(!is_toc_macro("something toc::[]"));
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
    }

    #[test]
    fn test_is_table_delimiter() {
        assert!(is_table_delimiter("|==="));
        assert!(is_table_delimiter("  |===  "));
        assert!(!is_table_delimiter("|===="));
        assert!(!is_table_delimiter("===="));
        assert!(!is_table_delimiter("|== "));
        assert!(!is_table_delimiter(""));
    }

    fn cell(content: &str) -> CellSpec<'_> {
        CellSpec { content, colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::default(), valign: VAlign::default() }
    }

    fn spanned_cell(content: &str, colspan: u8, rowspan: u8) -> CellSpec<'_> {
        CellSpec { content, colspan, rowspan, style: CellStyle::Default, halign: HAlign::default(), valign: VAlign::default() }
    }

    fn styled_cell(content: &str, style: CellStyle) -> CellSpec<'_> {
        CellSpec { content, colspan: 1, rowspan: 1, style, halign: HAlign::default(), valign: VAlign::default() }
    }

    fn spanned_styled_cell(content: &str, colspan: u8, rowspan: u8, style: CellStyle) -> CellSpec<'_> {
        CellSpec { content, colspan, rowspan, style, halign: HAlign::default(), valign: VAlign::default() }
    }

    fn aligned_cell(content: &str, halign: HAlign, valign: VAlign) -> CellSpec<'_> {
        CellSpec { content, colspan: 1, rowspan: 1, style: CellStyle::Default, halign, valign }
    }

    #[test]
    fn test_parse_table_cells() {
        assert_eq!(
            parse_table_cells("| A | B | C"),
            Some(vec![cell("A"), cell("B"), cell("C")])
        );
        assert_eq!(
            parse_table_cells("| A | B |"),
            Some(vec![cell("A"), cell("B")])
        );
        assert_eq!(parse_table_cells("no pipe"), None);
        assert_eq!(
            parse_table_cells("| single"),
            Some(vec![cell("single")])
        );
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
            parse_table_cells("| A 2+| B spans"),
            Some(vec![cell("A"), spanned_cell("B spans", 2, 1)])
        );
    }

    #[test]
    fn test_parse_table_cells_with_rowspan() {
        assert_eq!(
            parse_table_cells(".2+| C spans | D"),
            Some(vec![spanned_cell("C spans", 1, 2), cell("D")])
        );
    }

    #[test]
    fn test_parse_table_cells_with_both_spans() {
        assert_eq!(
            parse_table_cells("2.3+| cell"),
            Some(vec![spanned_cell("cell", 2, 3)])
        );
    }

    #[test]
    fn test_strip_callout_markers() {
        assert_eq!(
            strip_callout_markers("require 'sinatra' <1>"),
            ("require 'sinatra' ", vec![1])
        );
        assert_eq!(
            strip_callout_markers("code <1> <2>"),
            ("code ", vec![1, 2])
        );
        assert_eq!(
            strip_callout_markers("no callouts"),
            ("no callouts", vec![])
        );
        assert_eq!(
            strip_callout_markers("<1>"),
            ("", vec![1])
        );
        assert_eq!(
            strip_callout_markers("code <12>"),
            ("code ", vec![12])
        );
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
        assert_eq!(parse_cell_style_suffix("a"), ("", CellStyle::AsciiDoc));
        assert_eq!(parse_cell_style_suffix("h"), ("", CellStyle::Header));
        assert_eq!(parse_cell_style_suffix("e"), ("", CellStyle::Emphasis));
        assert_eq!(parse_cell_style_suffix("m"), ("", CellStyle::Monospace));
        assert_eq!(parse_cell_style_suffix("s"), ("", CellStyle::Strong));
        assert_eq!(parse_cell_style_suffix("l"), ("", CellStyle::Literal));
        // After span spec (ends with +)
        assert_eq!(parse_cell_style_suffix("2+a"), ("2+", CellStyle::AsciiDoc));
        assert_eq!(parse_cell_style_suffix("2.3+s"), ("2.3+", CellStyle::Strong));
        // Content ending with style letter — NOT a style
        assert_eq!(parse_cell_style_suffix("data"), ("data", CellStyle::Default));
        assert_eq!(parse_cell_style_suffix("date"), ("date", CellStyle::Default));
        // Empty
        assert_eq!(parse_cell_style_suffix(""), ("", CellStyle::Default));
        // Trailing space — last char is space, not a style letter
        assert_eq!(parse_cell_style_suffix(" a "), (" a ", CellStyle::Default));
        assert_eq!(parse_cell_style_suffix(" data "), (" data ", CellStyle::Default));
        // Style letter preceded by space (content + space + style)
        assert_eq!(parse_cell_style_suffix(" A e"), (" A ", CellStyle::Emphasis));
        assert_eq!(parse_cell_style_suffix(" text s"), (" text ", CellStyle::Strong));
    }

    #[test]
    fn test_parse_table_cells_with_style() {
        assert_eq!(
            parse_table_cells("e| italic text"),
            Some(vec![styled_cell("italic text", CellStyle::Emphasis)])
        );
        assert_eq!(
            parse_table_cells("s| bold text"),
            Some(vec![styled_cell("bold text", CellStyle::Strong)])
        );
        assert_eq!(
            parse_table_cells("h| header text"),
            Some(vec![styled_cell("header text", CellStyle::Header)])
        );
        assert_eq!(
            parse_table_cells("m| mono text"),
            Some(vec![styled_cell("mono text", CellStyle::Monospace)])
        );
        assert_eq!(
            parse_table_cells("l| literal text"),
            Some(vec![styled_cell("literal text", CellStyle::Literal)])
        );
        assert_eq!(
            parse_table_cells("a| asciidoc text"),
            Some(vec![styled_cell("asciidoc text", CellStyle::AsciiDoc)])
        );
    }

    #[test]
    fn test_parse_table_cells_span_plus_style() {
        assert_eq!(
            parse_table_cells("2+e| wide italic"),
            Some(vec![spanned_styled_cell("wide italic", 2, 1, CellStyle::Emphasis)])
        );
        assert_eq!(
            parse_table_cells("2.3+s| big bold"),
            Some(vec![spanned_styled_cell("big bold", 2, 3, CellStyle::Strong)])
        );
    }

    #[test]
    fn test_parse_table_cells_style_disambiguation() {
        // Content that ends with a style letter should NOT be treated as styled
        assert_eq!(
            parse_table_cells("| data | more"),
            Some(vec![cell("data"), cell("more")])
        );
        // Inline style between cells
        assert_eq!(
            parse_table_cells("| A e| B | C"),
            Some(vec![cell("A"), styled_cell("B", CellStyle::Emphasis), cell("C")])
        );
    }

    #[test]
    fn test_parse_cell_align_prefix_halign() {
        assert_eq!(parse_cell_align_prefix("^"), ("", HAlign::Center, VAlign::Top));
        assert_eq!(parse_cell_align_prefix("<"), ("", HAlign::Left, VAlign::Top));
        assert_eq!(parse_cell_align_prefix(">"), ("", HAlign::Right, VAlign::Top));
    }

    #[test]
    fn test_parse_cell_align_prefix_valign() {
        assert_eq!(parse_cell_align_prefix(".<"), ("", HAlign::Left, VAlign::Top));
        assert_eq!(parse_cell_align_prefix(".^"), ("", HAlign::Left, VAlign::Middle));
        assert_eq!(parse_cell_align_prefix(".>"), ("", HAlign::Left, VAlign::Bottom));
    }

    #[test]
    fn test_parse_cell_align_prefix_combined() {
        assert_eq!(parse_cell_align_prefix("^.>"), ("", HAlign::Center, VAlign::Bottom));
        assert_eq!(parse_cell_align_prefix(">.^rest"), ("rest", HAlign::Right, VAlign::Middle));
    }

    #[test]
    fn test_parse_cell_align_prefix_no_align() {
        assert_eq!(parse_cell_align_prefix(""), ("", HAlign::Left, VAlign::Top));
        assert_eq!(parse_cell_align_prefix("text"), ("text", HAlign::Left, VAlign::Top));
    }

    #[test]
    fn test_parse_table_cells_with_halign() {
        assert_eq!(
            parse_table_cells("^| centered"),
            Some(vec![aligned_cell("centered", HAlign::Center, VAlign::Top)])
        );
        assert_eq!(
            parse_table_cells(">| right"),
            Some(vec![aligned_cell("right", HAlign::Right, VAlign::Top)])
        );
    }

    #[test]
    fn test_parse_table_cells_with_valign() {
        assert_eq!(
            parse_table_cells(".^| middle"),
            Some(vec![aligned_cell("middle", HAlign::Left, VAlign::Middle)])
        );
    }

    #[test]
    fn test_parse_table_cells_with_combined_align() {
        assert_eq!(
            parse_table_cells(">.^| right-middle"),
            Some(vec![aligned_cell("right-middle", HAlign::Right, VAlign::Middle)])
        );
    }

    #[test]
    fn test_parse_table_cells_align_between_pipes() {
        assert_eq!(
            parse_table_cells("| A ^| B | C"),
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
}
