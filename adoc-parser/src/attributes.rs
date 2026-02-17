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

        if let Some(rest) = attr_str.strip_prefix('#') {
            if let Some(end) = rest.find(['.', '%', ',']) {
                attrs.id = Some(rest[..end].to_string());
            } else {
                attrs.id = Some(rest.to_string());
                return attrs;
            }
        }

        let parts = split_respecting_quotes(attr_str);
        for part in &parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((key, value)) = part.split_once('=') {
                let value = value.trim();
                // Strip surrounding quotes from value
                let value = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value);
                attrs.named.insert(key.trim().to_string(), value.to_string());
            } else if let Some(stripped) = part.strip_prefix('.') {
                attrs.roles.push(stripped.to_string());
            } else if let Some(stripped) = part.strip_prefix('#') {
                attrs.id = Some(stripped.to_string());
            } else if let Some(stripped) = part.strip_prefix('%') {
                attrs.options.push(stripped.to_string());
            } else {
                attrs.positional.push(part.to_string());
            }
        }

        attrs
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
}
