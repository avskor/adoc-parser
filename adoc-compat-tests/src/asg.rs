use serde_json::Value;

/// Simplified ASG node for comparison with asciidoc-parsing-lab expected output.
/// Fields like `location`, `marker`, `form`, `delimiter` are intentionally omitted.
#[derive(Debug, Clone, PartialEq)]
pub enum AsgNode {
    Document {
        header: Option<AsgHeader>,
        blocks: Vec<AsgNode>,
    },
    Section {
        level: u64,
        title: Vec<AsgNode>,
        blocks: Vec<AsgNode>,
    },
    Heading {
        level: u64,
        title: Vec<AsgNode>,
    },
    Paragraph {
        inlines: Vec<AsgNode>,
    },
    List {
        variant: String,
        items: Vec<AsgNode>,
    },
    ListItem {
        principal: Vec<AsgNode>,
        blocks: Vec<AsgNode>,
    },
    Dlist {
        items: Vec<AsgNode>,
    },
    DlistItem {
        terms: Vec<Vec<AsgNode>>,
        principal: Vec<AsgNode>,
        blocks: Vec<AsgNode>,
    },
    Listing {
        inlines: Vec<AsgNode>,
    },
    Literal {
        inlines: Vec<AsgNode>,
    },
    Sidebar {
        blocks: Vec<AsgNode>,
    },
    Admonition {
        variant: String,
        blocks: Vec<AsgNode>,
    },
    Image {
        target: String,
    },
    Text {
        value: String,
    },
    Span {
        variant: String,
        inlines: Vec<AsgNode>,
    },
    ThematicBreak,
    PageBreak,
    Unknown {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorInfo {
    pub fullname: String,
    pub firstname: String,
    pub middlename: String,
    pub lastname: String,
    pub initials: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsgHeader {
    pub title: Vec<AsgNode>,
    pub authors: Vec<AuthorInfo>,
}

impl AsgNode {
    /// Deserialize an ASG node from a serde_json::Value.
    /// Ignores `location`, `marker`, `form`, `delimiter`, `metadata` fields.
    pub fn from_value(val: &Value) -> Self {
        let obj = match val.as_object() {
            Some(o) => o,
            None => return AsgNode::Unknown { name: format!("non-object: {val}") },
        };

        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");

        match name {
            "document" => {
                let header = obj.get("header").map(|h| {
                    let title = parse_inline_array(h.get("title"));
                    let authors = h.get("authors")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().map(|a| {
                                let obj = a.as_object();
                                let get_str = |key: &str| -> String {
                                    obj.and_then(|o| o.get(key))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string()
                                };
                                AuthorInfo {
                                    fullname: get_str("fullname"),
                                    firstname: get_str("firstname"),
                                    middlename: get_str("middlename"),
                                    lastname: get_str("lastname"),
                                    initials: get_str("initials"),
                                    address: get_str("address"),
                                }
                            }).collect()
                        })
                        .unwrap_or_default();
                    AsgHeader { title, authors }
                });
                let blocks = parse_block_array(obj.get("blocks"));
                AsgNode::Document { header, blocks }
            }
            "section" => {
                let level = obj.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
                let title = parse_inline_array(obj.get("title"));
                let blocks = parse_block_array(obj.get("blocks"));
                AsgNode::Section { level, title, blocks }
            }
            "heading" => {
                let level = obj.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
                let title = parse_inline_array(obj.get("title"));
                AsgNode::Heading { level, title }
            }
            "paragraph" => {
                let inlines = parse_inline_array(obj.get("inlines"));
                AsgNode::Paragraph { inlines }
            }
            "list" => {
                let variant = obj.get("variant")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unordered")
                    .to_string();
                let items = obj.get("items")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().map(AsgNode::from_value).collect())
                    .unwrap_or_default();
                AsgNode::List { variant, items }
            }
            "listItem" => {
                let principal = parse_inline_array(obj.get("principal"));
                let blocks = parse_block_array(obj.get("blocks"));
                AsgNode::ListItem { principal, blocks }
            }
            "dlist" => {
                let items = obj.get("items")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().map(AsgNode::from_value).collect())
                    .unwrap_or_default();
                AsgNode::Dlist { items }
            }
            "dlistItem" => {
                let terms = obj.get("terms")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|term_group| parse_inline_array(Some(term_group)))
                            .collect()
                    })
                    .unwrap_or_default();
                let principal = parse_inline_array(obj.get("principal"));
                let blocks = parse_block_array(obj.get("blocks"));
                AsgNode::DlistItem { terms, principal, blocks }
            }
            "listing" => {
                let inlines = parse_inline_array(obj.get("inlines"));
                AsgNode::Listing { inlines }
            }
            "literal" => {
                let inlines = parse_inline_array(obj.get("inlines"));
                AsgNode::Literal { inlines }
            }
            "sidebar" => {
                let blocks = parse_block_array(obj.get("blocks"));
                AsgNode::Sidebar { blocks }
            }
            "admonition" => {
                let variant = obj.get("variant")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let blocks = parse_block_array(obj.get("blocks"));
                AsgNode::Admonition { variant, blocks }
            }
            "image" => {
                let target = obj.get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                AsgNode::Image { target }
            }
            "text" => {
                let value = obj.get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                AsgNode::Text { value }
            }
            "span" => {
                let variant = obj.get("variant")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let inlines = parse_inline_array(obj.get("inlines"));
                AsgNode::Span { variant, inlines }
            }
            "thematicBreak" => AsgNode::ThematicBreak,
            "pageBreak" => AsgNode::PageBreak,
            _ => AsgNode::Unknown { name: name.to_string() },
        }
    }

    /// Pretty-print for diff output
    pub fn pretty_print(&self, indent: usize) -> String {
        let pad = " ".repeat(indent);
        match self {
            AsgNode::Document { header, blocks } => {
                let mut s = format!("{pad}Document");
                if let Some(h) = header {
                    s += &format!("\n{pad}  header:");
                    for node in &h.title {
                        s += &format!("\n{}", node.pretty_print(indent + 4));
                    }
                    if !h.authors.is_empty() {
                        s += &format!("\n{pad}  authors:");
                        for a in &h.authors {
                            s += &format!("\n{pad}    Author(fullname={:?}, initials={:?})", a.fullname, a.initials);
                        }
                    }
                }
                for b in blocks {
                    s += &format!("\n{}", b.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Section { level, title, blocks } => {
                let mut s = format!("{pad}Section(level={level})");
                s += &format!("\n{pad}  title:");
                for t in title {
                    s += &format!("\n{}", t.pretty_print(indent + 4));
                }
                for b in blocks {
                    s += &format!("\n{}", b.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Heading { level, title } => {
                let mut s = format!("{pad}Heading(level={level})");
                for t in title {
                    s += &format!("\n{}", t.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Paragraph { inlines } => {
                let mut s = format!("{pad}Paragraph");
                for i in inlines {
                    s += &format!("\n{}", i.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::List { variant, items } => {
                let mut s = format!("{pad}List({variant})");
                for item in items {
                    s += &format!("\n{}", item.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::ListItem { principal, blocks } => {
                let mut s = format!("{pad}ListItem");
                if !principal.is_empty() {
                    s += &format!("\n{pad}  principal:");
                    for p in principal {
                        s += &format!("\n{}", p.pretty_print(indent + 4));
                    }
                }
                for b in blocks {
                    s += &format!("\n{}", b.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Dlist { items } => {
                let mut s = format!("{pad}Dlist");
                for item in items {
                    s += &format!("\n{}", item.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::DlistItem { terms, principal, blocks } => {
                let mut s = format!("{pad}DlistItem");
                for (i, term) in terms.iter().enumerate() {
                    s += &format!("\n{pad}  term[{i}]:");
                    for t in term {
                        s += &format!("\n{}", t.pretty_print(indent + 4));
                    }
                }
                if !principal.is_empty() {
                    s += &format!("\n{pad}  principal:");
                    for p in principal {
                        s += &format!("\n{}", p.pretty_print(indent + 4));
                    }
                }
                for b in blocks {
                    s += &format!("\n{}", b.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Listing { inlines } => {
                let mut s = format!("{pad}Listing");
                for i in inlines {
                    s += &format!("\n{}", i.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Literal { inlines } => {
                let mut s = format!("{pad}Literal");
                for i in inlines {
                    s += &format!("\n{}", i.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Sidebar { blocks } => {
                let mut s = format!("{pad}Sidebar");
                for b in blocks {
                    s += &format!("\n{}", b.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Admonition { variant, blocks } => {
                let mut s = format!("{pad}Admonition({variant})");
                for b in blocks {
                    s += &format!("\n{}", b.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::Image { target } => {
                format!("{pad}Image({target})")
            }
            AsgNode::Text { value } => {
                format!("{pad}Text({value:?})")
            }
            AsgNode::Span { variant, inlines } => {
                let mut s = format!("{pad}Span({variant})");
                for i in inlines {
                    s += &format!("\n{}", i.pretty_print(indent + 2));
                }
                s
            }
            AsgNode::ThematicBreak => format!("{pad}ThematicBreak"),
            AsgNode::PageBreak => format!("{pad}PageBreak"),
            AsgNode::Unknown { name } => format!("{pad}Unknown({name})"),
        }
    }
}

fn parse_block_array(val: Option<&Value>) -> Vec<AsgNode> {
    val.and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(AsgNode::from_value).collect())
        .unwrap_or_default()
}

fn parse_inline_array(val: Option<&Value>) -> Vec<AsgNode> {
    val.and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(AsgNode::from_value).collect())
        .unwrap_or_default()
}
