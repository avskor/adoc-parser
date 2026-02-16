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
    let trimmed = line.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2 {
        Some(&trimmed[1..trimmed.len() - 1])
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
    let stars = count_leading(trimmed, '*');
    if stars == 0 {
        return None;
    }
    let rest = &trimmed[stars..];
    if !rest.starts_with(' ') {
        return None;
    }
    Some((stars as u8, rest[1..].trim_start()))
}

pub fn is_list_marker_ordered(line: &str) -> Option<(u8, &str)> {
    let trimmed = line.trim_start();
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
    let alt = &rest[bracket_start + 1..bracket_end];
    Some((target, alt))
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

pub fn is_list_continuation(line: &str) -> bool {
    line.trim() == "+"
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
        assert_eq!(is_block_attribute("[]"), None);
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
}
