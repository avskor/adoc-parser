use std::borrow::Cow;
use std::collections::HashMap;

use crate::event::{CowStr, CellStyle, HAlign, VAlign};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableFormat {
    Native,
    Csv,
    Dsv,
    Tsv,
}

fn split_respecting_quotes(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;

    for (i, ch) in s.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct AttributeStore {
    attrs: HashMap<String, String>,
}

#[allow(dead_code)]
impl AttributeStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: &str, value: &str) {
        self.attrs.insert(name.to_string(), value.to_string());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(|s| s.as_str())
    }

    pub fn resolve<'a>(&self, name: &str) -> Option<CowStr<'a>> {
        self.attrs.get(name).map(|v| Cow::Owned(v.clone()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockAttributes {
    pub id: Option<String>,
    pub roles: Vec<String>,
    pub options: Vec<String>,
    pub positional: Vec<String>,
    pub named: HashMap<String, String>,
    #[allow(dead_code)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ColSpec {
    pub halign: HAlign,
    pub valign: VAlign,
    pub width: u8,
    pub style: CellStyle,
}

/// Parse a single column spec: `[multiplier*][halign][.valign][width][style]`
/// Returns `(count, ColSpec)` where count is the multiplier (default 1).
fn parse_col_spec(s: &str) -> (usize, ColSpec) {
    let mut rest = s;
    let mut count: usize = 1;

    // Check for multiplier: `N*`
    if let Some(star_pos) = rest.find('*') {
        let before = &rest[..star_pos];
        if !before.is_empty() && before.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(n) = before.parse::<usize>() {
                count = n.max(1);
            }
            rest = &rest[star_pos + 1..];
        }
    }

    let mut spec = ColSpec::default();

    // Parse halign: <, ^, >
    if let Some(stripped) = rest.strip_prefix('<') {
        spec.halign = HAlign::Left;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('^') {
        spec.halign = HAlign::Center;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('>') {
        spec.halign = HAlign::Right;
        rest = stripped;
    }

    // Parse valign: .<, .^, .>
    if let Some(stripped) = rest.strip_prefix(".<") {
        spec.valign = VAlign::Top;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix(".^") {
        spec.valign = VAlign::Middle;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix(".>") {
        spec.valign = VAlign::Bottom;
        rest = stripped;
    }

    // Parse width (digits)
    let digit_count = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count > 0 {
        if let Ok(w) = rest[..digit_count].parse::<u8>() {
            spec.width = w;
        }
        rest = &rest[digit_count..];
    }

    // Parse style letter
    if rest.len() == 1 {
        match rest.as_bytes()[0] {
            b'a' => spec.style = CellStyle::AsciiDoc,
            b'h' => spec.style = CellStyle::Header,
            b'e' => spec.style = CellStyle::Emphasis,
            b'm' => spec.style = CellStyle::Monospace,
            b's' => spec.style = CellStyle::Strong,
            b'l' => spec.style = CellStyle::Literal,
            _ => {}
        }
    }

    (count, spec)
}

impl BlockAttributes {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.id.is_none()
            && self.roles.is_empty()
            && self.options.is_empty()
            && self.positional.is_empty()
            && self.named.is_empty()
            && self.title.is_none()
    }

    pub fn parse(attr_str: &str) -> Self {
        let mut attrs = BlockAttributes::new();
        if attr_str.is_empty() {
            return attrs;
        }

        let parts = split_respecting_quotes(attr_str);
        for part in &parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((key, value)) = part.split_once('=') {
                // named: key=value
                let value = value.trim();
                let value = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .unwrap_or(value);
                attrs.named.insert(key.trim().to_string(), value.to_string());
            } else if part.starts_with('#') || part.starts_with('.') || part.starts_with('%') {
                // Pure shorthand: #id.role1.role2%opt1
                Self::parse_shorthand(part, &mut attrs);
            } else if let Some(pos) = part.find(['#', '.', '%']) {
                // Mixed: "discrete#myid.role" → positional + shorthand
                attrs.positional.push(part[..pos].to_string());
                Self::parse_shorthand(&part[pos..], &mut attrs);
            } else {
                attrs.positional.push(part.to_string());
            }
        }

        attrs
    }

    fn parse_shorthand(s: &str, attrs: &mut Self) {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let marker = bytes[i];
            i += 1;
            let start = i;
            while i < bytes.len() && !matches!(bytes[i], b'#' | b'.' | b'%') {
                i += 1;
            }
            let value = &s[start..i];
            if value.is_empty() {
                continue;
            }
            match marker {
                b'#' => attrs.id = Some(value.to_string()),
                b'.' => attrs.roles.push(value.to_string()),
                b'%' => attrs.options.push(value.to_string()),
                _ => {}
            }
        }
    }

    pub fn admonition_kind(&self) -> Option<crate::event::AdmonitionKind> {
        match self.positional.first().map(|s| s.as_str())? {
            "NOTE" => Some(crate::event::AdmonitionKind::Note),
            "TIP" => Some(crate::event::AdmonitionKind::Tip),
            "IMPORTANT" => Some(crate::event::AdmonitionKind::Important),
            "WARNING" => Some(crate::event::AdmonitionKind::Warning),
            "CAUTION" => Some(crate::event::AdmonitionKind::Caution),
            _ => None,
        }
    }

    pub fn source_language(&self) -> Option<&str> {
        if self.positional.first().map(|s| s.as_str()) == Some("source") {
            self.positional.get(1).map(|s| s.as_str())
        } else {
            None
        }
    }

    pub fn is_source_block(&self) -> bool {
        self.positional.first().map(|s| s.as_str()) == Some("source")
    }

    pub fn is_verse_style(&self) -> bool {
        self.positional.first().map(|s| s.as_str()) == Some("verse")
    }

    pub fn table_cols_count(&self) -> Option<usize> {
        if let Some(specs) = self.table_col_specs() {
            return Some(specs.len());
        }
        None
    }

    pub fn table_col_specs(&self) -> Option<Vec<ColSpec>> {
        let val = self.named.get("cols")?;
        let trimmed = val.trim();

        // Simple numeric: cols="3" → 3 columns with defaults
        if let Ok(n) = trimmed.parse::<usize>() {
            return Some(vec![ColSpec::default(); n]);
        }

        // Comma-separated specs: cols="<,^,>" or cols="^.>2,<1" or cols="3*^"
        let mut specs = Vec::new();
        for part in trimmed.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (count, spec) = parse_col_spec(part);
            for _ in 0..count {
                specs.push(spec.clone());
            }
        }

        if specs.is_empty() {
            None
        } else {
            Some(specs)
        }
    }

    pub fn stem_variant(&self) -> Option<&str> {
        match self.positional.first().map(|s| s.as_str()) {
            Some("stem") | Some("latexmath") | Some("asciimath") => {
                self.positional.first().map(|s| s.as_str())
            }
            _ => None,
        }
    }

    pub fn has_option(&self, name: &str) -> bool {
        self.options.iter().any(|o| o == name)
    }

    pub fn list_start(&self) -> Option<u32> {
        self.named.get("start")?.parse().ok()
    }

    pub fn is_reversed(&self) -> bool {
        self.has_option("reversed")
    }

    pub fn table_format(&self) -> TableFormat {
        // Check named attribute: format=csv / format=dsv / format=tsv
        if let Some(fmt) = self.named.get("format") {
            match fmt.as_str() {
                "csv" => return TableFormat::Csv,
                "dsv" => return TableFormat::Dsv,
                "tsv" => return TableFormat::Tsv,
                _ => {}
            }
        }
        // Check positional shorthand: [csv], [dsv], [tsv]
        if let Some(first) = self.positional.first() {
            match first.as_str() {
                "csv" => return TableFormat::Csv,
                "dsv" => return TableFormat::Dsv,
                "tsv" => return TableFormat::Tsv,
                _ => {}
            }
        }
        TableFormat::Native
    }
}

pub struct ImageAttrs<'a> {
    pub alt: &'a str,
    pub width: Option<&'a str>,
    pub height: Option<&'a str>,
}

pub fn parse_image_attrs(bracket_content: &str) -> ImageAttrs<'_> {
    if bracket_content.is_empty() {
        return ImageAttrs {
            alt: "",
            width: None,
            height: None,
        };
    }

    let mut alt: Option<&str> = None;
    let mut width: Option<&str> = None;
    let mut height: Option<&str> = None;
    let mut positional = Vec::new();

    for part in split_respecting_quotes(bracket_content) {
        let part = part.trim();
        if part.is_empty() {
            positional.push(part);
            continue;
        }
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            match key {
                "alt" => alt = Some(value),
                "width" => width = Some(value),
                "height" => height = Some(value),
                _ => {}
            }
        } else {
            positional.push(part);
        }
    }

    // alt: named "alt" or positional[0] or entire bracket_content
    let alt = alt.unwrap_or_else(|| positional.first().copied().unwrap_or(bracket_content));
    // width: named "width" or positional[1]
    if width.is_none()
        && let Some(&w) = positional.get(1)
        && !w.is_empty()
    {
        width = Some(w);
    }
    // height: named "height" or positional[2]
    if height.is_none()
        && let Some(&h) = positional.get(2)
        && !h.is_empty()
    {
        height = Some(h);
    }

    ImageAttrs { alt, width, height }
}

pub struct LinkAttrs<'a> {
    pub text: &'a str,
    pub window: Option<&'a str>,
    pub nofollow: bool,
}

pub fn parse_link_attrs(bracket_content: &str) -> LinkAttrs<'_> {
    if bracket_content.is_empty() {
        return LinkAttrs {
            text: "",
            window: None,
            nofollow: false,
        };
    }

    let mut window: Option<&str> = None;
    let mut nofollow = false;
    let mut positional = Vec::new();

    for part in split_respecting_quotes(bracket_content) {
        let part = part.trim();
        if part.is_empty() {
            positional.push(part);
            continue;
        }
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            match key {
                "window" => window = Some(value),
                "opts" if value == "nofollow" => nofollow = true,
                _ => {}
            }
        } else {
            positional.push(part);
        }
    }

    let text = positional.first().copied().unwrap_or(bracket_content);

    LinkAttrs { text, window, nofollow }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_store() {
        let mut store = AttributeStore::new();
        store.set("toc", "left");
        assert_eq!(store.get("toc"), Some("left"));
        assert_eq!(store.get("missing"), None);
    }

    #[test]
    fn test_block_attributes_parse_source() {
        let attrs = BlockAttributes::parse("source,rust");
        assert_eq!(attrs.positional, vec!["source", "rust"]);
        assert!(attrs.is_source_block());
        assert_eq!(attrs.source_language(), Some("rust"));
    }

    #[test]
    fn test_block_attributes_parse_id() {
        let attrs = BlockAttributes::parse("#my-id");
        assert_eq!(attrs.id.as_deref(), Some("my-id"));
    }

    #[test]
    fn test_block_attributes_parse_role() {
        let attrs = BlockAttributes::parse(".role1,.role2");
        assert_eq!(attrs.roles, vec!["role1", "role2"]);
    }

    #[test]
    fn test_block_attributes_empty() {
        let attrs = BlockAttributes::new();
        assert!(attrs.is_empty());
    }

    #[test]
    fn test_table_cols_count() {
        let attrs = BlockAttributes::parse("cols=\"3\"");
        assert_eq!(attrs.table_cols_count(), Some(3));

        let attrs = BlockAttributes::parse("cols=\"1,1,1\"");
        assert_eq!(attrs.table_cols_count(), Some(3));

        let attrs = BlockAttributes::new();
        assert_eq!(attrs.table_cols_count(), None);
    }

    #[test]
    fn test_is_verse_style() {
        let attrs = BlockAttributes::parse("verse");
        assert!(attrs.is_verse_style());

        let attrs = BlockAttributes::parse("source,rust");
        assert!(!attrs.is_verse_style());

        let attrs = BlockAttributes::new();
        assert!(!attrs.is_verse_style());
    }

    #[test]
    fn test_has_option() {
        let attrs = BlockAttributes::parse("%header");
        assert!(attrs.has_option("header"));
        assert!(!attrs.has_option("footer"));

        let attrs = BlockAttributes::parse("%header,%footer");
        assert!(attrs.has_option("header"));
        assert!(attrs.has_option("footer"));
    }

    #[test]
    fn test_shorthand_id_and_roles() {
        let attrs = BlockAttributes::parse("#notice.important");
        assert_eq!(attrs.id.as_deref(), Some("notice"));
        assert_eq!(attrs.roles, vec!["important"]);
    }

    #[test]
    fn test_shorthand_multiple_roles() {
        let attrs = BlockAttributes::parse(".role1.role2.role3");
        assert_eq!(attrs.roles, vec!["role1", "role2", "role3"]);
        assert!(attrs.id.is_none());
    }

    #[test]
    fn test_shorthand_id_and_multiple_roles() {
        let attrs = BlockAttributes::parse("#myid.role1.role2");
        assert_eq!(attrs.id.as_deref(), Some("myid"));
        assert_eq!(attrs.roles, vec!["role1", "role2"]);
    }

    #[test]
    fn test_shorthand_options() {
        let attrs = BlockAttributes::parse("%opt1%opt2");
        assert_eq!(attrs.options, vec!["opt1", "opt2"]);
    }

    #[test]
    fn test_shorthand_id_and_options() {
        let attrs = BlockAttributes::parse("#myid%opt1%opt2");
        assert_eq!(attrs.id.as_deref(), Some("myid"));
        assert_eq!(attrs.options, vec!["opt1", "opt2"]);
    }

    #[test]
    fn test_mixed_positional_and_shorthand() {
        let attrs = BlockAttributes::parse("discrete#myid.role");
        assert_eq!(attrs.positional, vec!["discrete"]);
        assert_eq!(attrs.id.as_deref(), Some("myid"));
        assert_eq!(attrs.roles, vec!["role"]);
    }

    #[test]
    fn test_source_with_shorthand_id() {
        let attrs = BlockAttributes::parse("source,rust,#code1");
        assert_eq!(attrs.positional, vec!["source", "rust"]);
        assert_eq!(attrs.id.as_deref(), Some("code1"));
    }

    #[test]
    fn test_list_start() {
        let attrs = BlockAttributes::parse("start=5");
        assert_eq!(attrs.list_start(), Some(5));

        let attrs = BlockAttributes::parse("start=1");
        assert_eq!(attrs.list_start(), Some(1));

        let attrs = BlockAttributes::new();
        assert_eq!(attrs.list_start(), None);
    }

    #[test]
    fn test_is_reversed() {
        let attrs = BlockAttributes::parse("%reversed");
        assert!(attrs.is_reversed());

        let attrs = BlockAttributes::new();
        assert!(!attrs.is_reversed());
    }

    #[test]
    fn test_table_format_named() {
        let attrs = BlockAttributes::parse("format=csv");
        assert_eq!(attrs.table_format(), TableFormat::Csv);

        let attrs = BlockAttributes::parse("format=dsv");
        assert_eq!(attrs.table_format(), TableFormat::Dsv);

        let attrs = BlockAttributes::parse("format=tsv");
        assert_eq!(attrs.table_format(), TableFormat::Tsv);
    }

    #[test]
    fn test_table_format_shorthand() {
        let attrs = BlockAttributes::parse("csv");
        assert_eq!(attrs.table_format(), TableFormat::Csv);

        let attrs = BlockAttributes::parse("dsv");
        assert_eq!(attrs.table_format(), TableFormat::Dsv);

        let attrs = BlockAttributes::parse("tsv");
        assert_eq!(attrs.table_format(), TableFormat::Tsv);
    }

    #[test]
    fn test_table_format_default() {
        let attrs = BlockAttributes::new();
        assert_eq!(attrs.table_format(), TableFormat::Native);

        let attrs = BlockAttributes::parse("source,rust");
        assert_eq!(attrs.table_format(), TableFormat::Native);
    }

    #[test]
    fn test_parse_col_spec_simple_number() {
        let (count, spec) = parse_col_spec("1");
        assert_eq!(count, 1);
        assert_eq!(spec.width, 1);
        assert_eq!(spec.halign, HAlign::Left);
        assert_eq!(spec.valign, VAlign::Top);
    }

    #[test]
    fn test_parse_col_spec_halign() {
        let (_, spec) = parse_col_spec("<");
        assert_eq!(spec.halign, HAlign::Left);

        let (_, spec) = parse_col_spec("^");
        assert_eq!(spec.halign, HAlign::Center);

        let (_, spec) = parse_col_spec(">");
        assert_eq!(spec.halign, HAlign::Right);
    }

    #[test]
    fn test_parse_col_spec_valign() {
        let (_, spec) = parse_col_spec(".<");
        assert_eq!(spec.valign, VAlign::Top);

        let (_, spec) = parse_col_spec(".^");
        assert_eq!(spec.valign, VAlign::Middle);

        let (_, spec) = parse_col_spec(".>");
        assert_eq!(spec.valign, VAlign::Bottom);
    }

    #[test]
    fn test_parse_col_spec_combined_align_and_width() {
        let (count, spec) = parse_col_spec("^.>2");
        assert_eq!(count, 1);
        assert_eq!(spec.halign, HAlign::Center);
        assert_eq!(spec.valign, VAlign::Bottom);
        assert_eq!(spec.width, 2);
    }

    #[test]
    fn test_parse_col_spec_multiplier() {
        let (count, spec) = parse_col_spec("3*^");
        assert_eq!(count, 3);
        assert_eq!(spec.halign, HAlign::Center);
    }

    #[test]
    fn test_parse_col_spec_with_style() {
        let (_, spec) = parse_col_spec("2a");
        assert_eq!(spec.width, 2);
        assert_eq!(spec.style, CellStyle::AsciiDoc);

        let (_, spec) = parse_col_spec(">1e");
        assert_eq!(spec.halign, HAlign::Right);
        assert_eq!(spec.width, 1);
        assert_eq!(spec.style, CellStyle::Emphasis);
    }

    #[test]
    fn test_table_col_specs_numeric() {
        let attrs = BlockAttributes::parse("cols=\"3\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        for s in &specs {
            assert_eq!(s.halign, HAlign::Left);
            assert_eq!(s.valign, VAlign::Top);
        }
    }

    #[test]
    fn test_table_col_specs_align_list() {
        let attrs = BlockAttributes::parse("cols=\"<,^,>\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0].halign, HAlign::Left);
        assert_eq!(specs[1].halign, HAlign::Center);
        assert_eq!(specs[2].halign, HAlign::Right);
    }

    #[test]
    fn test_table_col_specs_multiplier() {
        let attrs = BlockAttributes::parse("cols=\"3*^\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        for s in &specs {
            assert_eq!(s.halign, HAlign::Center);
        }
    }

    #[test]
    fn test_table_col_specs_mixed() {
        let attrs = BlockAttributes::parse("cols=\"^.>2,<1\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].halign, HAlign::Center);
        assert_eq!(specs[0].valign, VAlign::Bottom);
        assert_eq!(specs[0].width, 2);
        assert_eq!(specs[1].halign, HAlign::Left);
        assert_eq!(specs[1].width, 1);
    }

    #[test]
    fn test_table_col_specs_with_style() {
        let attrs = BlockAttributes::parse("cols=\"1,2a,3\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0].style, CellStyle::Default);
        assert_eq!(specs[1].style, CellStyle::AsciiDoc);
        assert_eq!(specs[1].width, 2);
        assert_eq!(specs[2].style, CellStyle::Default);
        assert_eq!(specs[2].width, 3);
    }

    #[test]
    fn test_parse_image_attrs_all() {
        let attrs = parse_image_attrs("Alt text,600,400");
        assert_eq!(attrs.alt, "Alt text");
        assert_eq!(attrs.width, Some("600"));
        assert_eq!(attrs.height, Some("400"));
    }

    #[test]
    fn test_parse_image_attrs_alt_only() {
        let attrs = parse_image_attrs("A beautiful sunset");
        assert_eq!(attrs.alt, "A beautiful sunset");
        assert_eq!(attrs.width, None);
        assert_eq!(attrs.height, None);
    }

    #[test]
    fn test_parse_image_attrs_named() {
        let attrs = parse_image_attrs("alt=Photo,width=800");
        assert_eq!(attrs.alt, "Photo");
        assert_eq!(attrs.width, Some("800"));
        assert_eq!(attrs.height, None);
    }

    #[test]
    fn test_parse_image_attrs_named_all() {
        let attrs = parse_image_attrs("alt=Photo,width=800,height=600");
        assert_eq!(attrs.alt, "Photo");
        assert_eq!(attrs.width, Some("800"));
        assert_eq!(attrs.height, Some("600"));
    }

    #[test]
    fn test_parse_image_attrs_empty() {
        let attrs = parse_image_attrs("");
        assert_eq!(attrs.alt, "");
        assert_eq!(attrs.width, None);
        assert_eq!(attrs.height, None);
    }

    #[test]
    fn test_parse_image_attrs_width_only() {
        let attrs = parse_image_attrs("Alt,300");
        assert_eq!(attrs.alt, "Alt");
        assert_eq!(attrs.width, Some("300"));
        assert_eq!(attrs.height, None);
    }

    #[test]
    fn test_parse_link_attrs_text_only() {
        let attrs = parse_link_attrs("Example Site");
        assert_eq!(attrs.text, "Example Site");
        assert_eq!(attrs.window, None);
        assert!(!attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_with_window() {
        let attrs = parse_link_attrs("Example,window=_blank");
        assert_eq!(attrs.text, "Example");
        assert_eq!(attrs.window, Some("_blank"));
        assert!(!attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_with_nofollow() {
        let attrs = parse_link_attrs("Example,opts=nofollow");
        assert_eq!(attrs.text, "Example");
        assert_eq!(attrs.window, None);
        assert!(attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_with_all() {
        let attrs = parse_link_attrs("Example,window=_blank,opts=nofollow");
        assert_eq!(attrs.text, "Example");
        assert_eq!(attrs.window, Some("_blank"));
        assert!(attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_empty() {
        let attrs = parse_link_attrs("");
        assert_eq!(attrs.text, "");
        assert_eq!(attrs.window, None);
        assert!(!attrs.nofollow);
    }
}
