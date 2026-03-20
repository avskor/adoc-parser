use std::collections::BTreeMap;
use std::fmt::Write;

use scraper::node::Node;
use scraper::Html;

/// Tags whose content should be preserved verbatim (no whitespace normalization).
const VERBATIM_TAGS: &[&str] = &["pre", "code", "script", "style"];

/// A normalized representation of an HTML node for semantic comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedNode {
    Element {
        tag: String,
        attrs: BTreeMap<String, String>,
        children: Vec<NormalizedNode>,
    },
    Text(String),
}

impl NormalizedNode {
    /// Render this node as pretty-printed HTML for diff output.
    pub fn to_pretty_html(&self, indent: usize) -> String {
        let mut buf = String::new();
        self.write_pretty(&mut buf, indent);
        buf
    }

    fn write_pretty(&self, buf: &mut String, indent: usize) {
        let pad = " ".repeat(indent);
        match self {
            NormalizedNode::Text(text) => {
                let _ = write!(buf, "{pad}{text}");
            }
            NormalizedNode::Element {
                tag,
                attrs,
                children,
            } => {
                let _ = write!(buf, "{pad}<{tag}");
                for (key, value) in attrs {
                    let _ = write!(buf, " {key}=\"{value}\"");
                }
                if children.is_empty() {
                    let _ = write!(buf, "/>");
                } else {
                    let _ = write!(buf, ">");
                    let inline = children.len() == 1 && matches!(&children[0], NormalizedNode::Text(_));
                    if inline {
                        // Single text child — keep on same line
                        if let NormalizedNode::Text(t) = &children[0] {
                            let _ = write!(buf, "{t}</{tag}>");
                        }
                    } else {
                        for child in children {
                            buf.push('\n');
                            child.write_pretty(buf, indent + 2);
                        }
                        let _ = write!(buf, "\n{pad}</{tag}>");
                    }
                }
            }
        }
    }
}

/// Parse an HTML fragment and return a normalized tree.
pub fn parse_and_normalize(html: &str) -> Vec<NormalizedNode> {
    let document = Html::parse_fragment(html);
    let root = document.tree.root();
    // The fragment root is an artificial node; collect children of it.
    // scraper wraps fragment content in an <html> node, so we need to
    // go through the root's children.
    let mut nodes = Vec::new();
    collect_children(root, &mut nodes, false);
    nodes
}

fn collect_children(
    parent: ego_tree::NodeRef<'_, Node>,
    out: &mut Vec<NormalizedNode>,
    verbatim: bool,
) {
    for child in parent.children() {
        match child.value() {
            Node::Element(el) => {
                let tag = el.name.local.as_ref().to_string();
                // Skip the synthetic <html> wrapper added by parse_fragment
                if tag == "html" {
                    collect_children(child, out, verbatim);
                    continue;
                }

                let mut attrs = BTreeMap::new();
                for (name, value) in &el.attrs {
                    let key = name.local.as_ref();
                    let val: &str = value;
                    let normalized_value = if key == "class" {
                        let mut classes: Vec<&str> = val.split_whitespace().collect();
                        classes.sort();
                        classes.join(" ")
                    } else {
                        val.to_string()
                    };
                    attrs.insert(key.to_string(), normalized_value);
                }

                let is_verbatim = verbatim || VERBATIM_TAGS.contains(&tag.as_str());
                let mut children = Vec::new();
                collect_children(child, &mut children, is_verbatim);

                out.push(NormalizedNode::Element {
                    tag,
                    attrs,
                    children,
                });
            }
            Node::Text(text) => {
                let s: &str = text;
                if verbatim {
                    out.push(NormalizedNode::Text(s.to_string()));
                } else {
                    let collapsed = collapse_whitespace(s);
                    if !collapsed.is_empty() {
                        out.push(NormalizedNode::Text(collapsed));
                    }
                }
            }
            Node::Comment(_) => {}
            _ => {}
        }
    }
}

/// Collapse multiple whitespace characters into a single space and trim.
fn collapse_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut last_was_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !last_was_space && !result.is_empty() {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }
    // Trim trailing space
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

/// Compare two HTML strings semantically. Returns `Ok(())` on match, or
/// `Err(diff_string)` with a human-readable diff on mismatch.
pub fn assert_html_eq(expected_html: &str, actual_html: &str) -> Result<(), String> {
    let expected = parse_and_normalize(expected_html);
    let actual = parse_and_normalize(actual_html);

    if expected == actual {
        return Ok(());
    }

    // Build pretty-printed versions for diff
    let expected_pretty = nodes_to_pretty(&expected);
    let actual_pretty = nodes_to_pretty(&actual);

    let diff = similar::TextDiff::from_lines(&expected_pretty, &actual_pretty);
    let mut output = String::new();
    output.push_str("HTML mismatch (normalized):\n");
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        let _ = write!(output, "{sign}{change}");
    }

    Err(output)
}

fn nodes_to_pretty(nodes: &[NormalizedNode]) -> String {
    let mut buf = String::new();
    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            buf.push('\n');
        }
        node.write_pretty(&mut buf, 0);
    }
    buf.push('\n');
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitespace_normalization() {
        let html = "<p>  Hello   world  </p>";
        let nodes = parse_and_normalize(html);
        assert_eq!(
            nodes,
            vec![NormalizedNode::Element {
                tag: "p".to_string(),
                attrs: BTreeMap::new(),
                children: vec![NormalizedNode::Text("Hello world".to_string())],
            }]
        );
    }

    #[test]
    fn test_class_sorting() {
        let html = r#"<div class="sect1 bibliography">content</div>"#;
        let nodes = parse_and_normalize(html);
        if let NormalizedNode::Element { attrs, .. } = &nodes[0] {
            assert_eq!(attrs["class"], "bibliography sect1");
        } else {
            panic!("Expected element");
        }
    }

    #[test]
    fn test_verbatim_preserved() {
        let html = "<pre>  hello   world  </pre>";
        let nodes = parse_and_normalize(html);
        if let NormalizedNode::Element { children, .. } = &nodes[0] {
            assert_eq!(children, &[NormalizedNode::Text("  hello   world  ".to_string())]);
        } else {
            panic!("Expected element");
        }
    }

    #[test]
    fn test_comments_removed() {
        let html = "<p>hello<!-- comment -->world</p>";
        let nodes = parse_and_normalize(html);
        if let NormalizedNode::Element { children, .. } = &nodes[0] {
            assert_eq!(
                children,
                &[
                    NormalizedNode::Text("hello".to_string()),
                    NormalizedNode::Text("world".to_string()),
                ]
            );
        } else {
            panic!("Expected element");
        }
    }

    #[test]
    fn test_assert_html_eq_match() {
        let a = "<p>Hello  world</p>";
        let b = "<p>Hello world</p>";
        assert!(assert_html_eq(a, b).is_ok());
    }

    #[test]
    fn test_assert_html_eq_mismatch() {
        let a = "<p>Hello</p>";
        let b = "<p>World</p>";
        assert!(assert_html_eq(a, b).is_err());
    }
}
