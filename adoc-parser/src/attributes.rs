use std::borrow::Cow;
use std::collections::HashMap;

use crate::event::CowStr;

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
        let val = self.named.get("cols")?;
        let trimmed = val.trim();
        if let Ok(n) = trimmed.parse::<usize>() {
            return Some(n);
        }
        Some(trimmed.split(',').count())
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
}
