use std::borrow::Cow;
use std::collections::HashMap;

use crate::event::CowStr;

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
            if let Some(end) = rest.find(|c: char| c == '.' || c == '%' || c == ',') {
                attrs.id = Some(rest[..end].to_string());
            } else {
                attrs.id = Some(rest.to_string());
                return attrs;
            }
        }

        let parts: Vec<&str> = attr_str.split(',').collect();
        for part in parts.iter() {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((key, value)) = part.split_once('=') {
                attrs.named.insert(key.trim().to_string(), value.trim().to_string());
            } else if part.starts_with('.') {
                attrs.roles.push(part[1..].to_string());
            } else if part.starts_with('#') {
                attrs.id = Some(part[1..].to_string());
            } else if part.starts_with('%') {
                attrs.options.push(part[1..].to_string());
            } else {
                attrs.positional.push(part.to_string());
            }
        }

        attrs
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
}
