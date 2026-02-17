use adoc_parser::{
    AdmonitionKind, DelimitedBlockKind, Event, Tag, TagEnd,
};

use crate::asg::{AsgHeader, AsgNode};

/// Stack frame representing an open tag being built.
enum BuildFrame {
    Document {
        header: Option<AsgHeader>,
        blocks: Vec<AsgNode>,
    },
    Header {
        title: Vec<AsgNode>,
    },
    DocumentTitle {
        inlines: Vec<AsgNode>,
    },
    Section {
        level: u8,
        title: Vec<AsgNode>,
        blocks: Vec<AsgNode>,
    },
    SectionTitle {
        inlines: Vec<AsgNode>,
    },
    Paragraph {
        inlines: Vec<AsgNode>,
    },
    LiteralParagraph {
        inlines: Vec<AsgNode>,
    },
    UnorderedList {
        items: Vec<AsgNode>,
    },
    OrderedList {
        items: Vec<AsgNode>,
    },
    ListItem {
        children: Vec<AsgNode>,
    },
    DescriptionList {
        items: Vec<AsgNode>,
        current_terms: Vec<Vec<AsgNode>>,
    },
    DescriptionTerm {
        inlines: Vec<AsgNode>,
    },
    DescriptionDescription {
        children: Vec<AsgNode>,
    },
    DelimitedBlock {
        kind: DelimitedBlockKind,
        children: Vec<AsgNode>,
    },
    SourceBlock {
        children: Vec<AsgNode>,
    },
    Admonition {
        variant: String,
        blocks: Vec<AsgNode>,
    },
    BlockTitle {
        inlines: Vec<AsgNode>,
    },
    Strong {
        inlines: Vec<AsgNode>,
    },
    Emphasis {
        inlines: Vec<AsgNode>,
    },
    Monospace {
        inlines: Vec<AsgNode>,
    },
    Highlight {
        inlines: Vec<AsgNode>,
    },
    Superscript {
        inlines: Vec<AsgNode>,
    },
    Subscript {
        inlines: Vec<AsgNode>,
    },
    Link {
        url: String,
        inlines: Vec<AsgNode>,
    },
    Table,
    TableHead,
    TableBody,
    TableFoot,
    TableRow,
    TableCell {
        children: Vec<AsgNode>,
    },
    TableHeaderCell {
        children: Vec<AsgNode>,
    },
    BlockImage {
        target: String,
    },
    InlineImage {
        target: String,
    },
    CalloutList {
        items: Vec<AsgNode>,
    },
    CalloutListItem {
        children: Vec<AsgNode>,
    },
    Heading {
        level: u8,
        inlines: Vec<AsgNode>,
    },
    Anchor,
    CrossReference,
}

/// Build an AsgNode tree from a stream of parser events.
pub fn build_asg<'a>(events: impl Iterator<Item = Event<'a>>) -> AsgNode {
    let mut stack: Vec<BuildFrame> = vec![BuildFrame::Document {
        header: None,
        blocks: vec![],
    }];

    for event in events {
        match event {
            Event::Start(tag) => {
                let frame = match tag {
                    Tag::Header => BuildFrame::Header { title: vec![] },
                    Tag::DocumentTitle => BuildFrame::DocumentTitle { inlines: vec![] },
                    Tag::Section { .. } => BuildFrame::Section {
                        level: 0, // will be set from SectionTitle
                        title: vec![],
                        blocks: vec![],
                    },
                    Tag::SectionTitle { level, .. } => {
                        // Update parent Section's level
                        if let Some(BuildFrame::Section { level: l, .. }) = stack.last_mut()
                        {
                            *l = level;
                        }
                        BuildFrame::SectionTitle { inlines: vec![] }
                    }
                    Tag::Heading { level } => BuildFrame::Heading {
                        level,
                        inlines: vec![],
                    },
                    Tag::Paragraph => BuildFrame::Paragraph { inlines: vec![] },
                    Tag::LiteralParagraph => BuildFrame::LiteralParagraph { inlines: vec![] },
                    Tag::UnorderedList { .. } => BuildFrame::UnorderedList { items: vec![] },
                    Tag::OrderedList => BuildFrame::OrderedList { items: vec![] },
                    Tag::ListItem { .. } => BuildFrame::ListItem {
                        children: vec![],
                    },
                    Tag::DescriptionList => BuildFrame::DescriptionList {
                        items: vec![],
                        current_terms: vec![],
                    },
                    Tag::DescriptionTerm => BuildFrame::DescriptionTerm { inlines: vec![] },
                    Tag::DescriptionDescription => {
                        BuildFrame::DescriptionDescription { children: vec![] }
                    }
                    Tag::DelimitedBlock { kind } => BuildFrame::DelimitedBlock {
                        kind,
                        children: vec![],
                    },
                    Tag::SourceBlock { .. } => BuildFrame::SourceBlock { children: vec![] },
                    Tag::Admonition { kind } => BuildFrame::Admonition {
                        variant: admonition_variant(&kind),
                        blocks: vec![],
                    },
                    Tag::BlockTitle => BuildFrame::BlockTitle { inlines: vec![] },
                    Tag::Strong => BuildFrame::Strong { inlines: vec![] },
                    Tag::Emphasis => BuildFrame::Emphasis { inlines: vec![] },
                    Tag::Monospace => BuildFrame::Monospace { inlines: vec![] },
                    Tag::Highlight => BuildFrame::Highlight { inlines: vec![] },
                    Tag::Superscript => BuildFrame::Superscript { inlines: vec![] },
                    Tag::Subscript => BuildFrame::Subscript { inlines: vec![] },
                    Tag::Link { url } => BuildFrame::Link {
                        url: url.to_string(),
                        inlines: vec![],
                    },
                    Tag::Table => BuildFrame::Table,
                    Tag::TableHead => BuildFrame::TableHead,
                    Tag::TableBody => BuildFrame::TableBody,
                    Tag::TableFoot => BuildFrame::TableFoot,
                    Tag::TableRow => BuildFrame::TableRow,
                    Tag::TableCell => BuildFrame::TableCell { children: vec![] },
                    Tag::TableHeaderCell => BuildFrame::TableHeaderCell { children: vec![] },
                    Tag::BlockImage { target, .. } => BuildFrame::BlockImage {
                        target: target.to_string(),
                    },
                    Tag::InlineImage { target, .. } => BuildFrame::InlineImage {
                        target: target.to_string(),
                    },
                    Tag::CalloutList => BuildFrame::CalloutList { items: vec![] },
                    Tag::CalloutListItem { .. } => BuildFrame::CalloutListItem {
                        children: vec![],
                    },
                    Tag::Anchor { .. } => BuildFrame::Anchor,
                    Tag::CrossReference { .. } => BuildFrame::CrossReference,
                };
                stack.push(frame);
            }

            Event::End(tag_end) => {
                let frame = stack.pop().expect("unbalanced End event");
                let node = finish_frame(frame, &tag_end);

                // Push the finished node into the parent frame
                if let Some(node) = node {
                    push_to_parent(&mut stack, node, &tag_end);
                }
            }

            Event::Text(text) => {
                let text_node = AsgNode::Text {
                    value: text.to_string(),
                };
                push_inline_to_current(&mut stack, text_node);
            }

            Event::Code(text) => {
                let span = AsgNode::Span {
                    variant: "code".to_string(),
                    inlines: vec![AsgNode::Text {
                        value: text.to_string(),
                    }],
                };
                push_inline_to_current(&mut stack, span);
            }

            Event::SoftBreak => {
                // Will be merged with adjacent Text nodes later
                push_inline_to_current(
                    &mut stack,
                    AsgNode::Text {
                        value: "\n".to_string(),
                    },
                );
            }

            Event::HardBreak => {
                push_inline_to_current(
                    &mut stack,
                    AsgNode::Text {
                        value: "\n".to_string(),
                    },
                );
            }

            Event::ThematicBreak => {
                push_block_to_current(&mut stack, AsgNode::ThematicBreak);
            }

            Event::PageBreak => {
                push_block_to_current(&mut stack, AsgNode::PageBreak);
            }

            Event::InlinePassthrough(text) => {
                push_inline_to_current(
                    &mut stack,
                    AsgNode::Text {
                        value: text.to_string(),
                    },
                );
            }

            // Attributes, footnotes, etc. — skip for now
            Event::Attribute { .. }
            | Event::AttributeReference(_)
            | Event::Footnote { .. }
            | Event::FootnoteRef { .. }
            | Event::CalloutRef(_)
            | Event::Toc
            | Event::Include { .. } => {}
        }
    }

    // Pop the Document frame
    match stack.pop().expect("missing Document frame") {
        BuildFrame::Document { header, blocks } => AsgNode::Document { header, blocks },
        _ => panic!("expected Document frame at bottom of stack"),
    }
}

/// Convert a finished frame into an AsgNode.
fn finish_frame(frame: BuildFrame, _tag_end: &TagEnd) -> Option<AsgNode> {
    match frame {
        BuildFrame::Document { .. } => {
            // Shouldn't happen — Document is handled in build_asg
            None
        }
        BuildFrame::Header { title } => {
            // Header is handled specially — store in parent Document
            Some(AsgNode::Document {
                header: Some(AsgHeader { title }),
                blocks: vec![],
            })
        }
        BuildFrame::DocumentTitle { inlines } => {
            // Will be captured by Header frame
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Paragraph {
                inlines: merged,
            })
        }
        BuildFrame::Heading { level, inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Heading {
                level: (level.saturating_sub(1)) as u64,
                title: merged,
            })
        }
        BuildFrame::Section { level, title, blocks } => {
            // ASG level = our level - 1
            Some(AsgNode::Section {
                level: (level.saturating_sub(1)) as u64,
                title,
                blocks,
            })
        }
        BuildFrame::SectionTitle { inlines } => {
            // Flatten any nested Paragraphs (from DocumentTitle inside SectionTitle in header)
            let flat: Vec<AsgNode> = inlines
                .into_iter()
                .flat_map(|n| {
                    if let AsgNode::Paragraph { inlines } = n {
                        inlines
                    } else {
                        vec![n]
                    }
                })
                .collect();
            let merged = merge_adjacent_text(flat);
            // Will be captured by Section or Header frame as title
            Some(AsgNode::Paragraph { inlines: merged })
        }
        BuildFrame::Paragraph { inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Paragraph { inlines: merged })
        }
        BuildFrame::LiteralParagraph { inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Literal { inlines: merged })
        }
        BuildFrame::UnorderedList { items } => Some(AsgNode::List {
            variant: "unordered".to_string(),
            items,
        }),
        BuildFrame::OrderedList { items } => Some(AsgNode::List {
            variant: "ordered".to_string(),
            items,
        }),
        BuildFrame::ListItem { children } => {
            // First Paragraph becomes principal, rest become blocks
            let (principal, blocks) = extract_principal(children);
            Some(AsgNode::ListItem { principal, blocks })
        }
        BuildFrame::DescriptionList {
            mut items,
            current_terms,
        } => {
            // If there are pending terms without a description, flush them
            if !current_terms.is_empty() {
                items.push(AsgNode::DlistItem {
                    terms: current_terms,
                    principal: vec![],
                    blocks: vec![],
                });
            }
            Some(AsgNode::Dlist { items })
        }
        BuildFrame::DescriptionTerm { inlines } => {
            let merged = merge_adjacent_text(inlines);
            // Wrapped in a temporary node; will be extracted by parent
            Some(AsgNode::Paragraph { inlines: merged })
        }
        BuildFrame::DescriptionDescription { children } => {
            // Extract principal from the first paragraph
            let (principal, blocks) = extract_principal(children);
            Some(AsgNode::ListItem { principal, blocks })
        }
        BuildFrame::DelimitedBlock { kind, children } => {
            Some(delimited_block_node(kind, children))
        }
        BuildFrame::SourceBlock { children } => {
            // SourceBlock → Listing with merged inlines
            let inlines = collect_all_inlines(children);
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Listing { inlines: merged })
        }
        BuildFrame::Admonition { variant, blocks } => {
            Some(AsgNode::Admonition { variant, blocks })
        }
        BuildFrame::BlockTitle { .. } => {
            // Block titles are metadata — skip for comparison
            None
        }
        BuildFrame::Strong { inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Span {
                variant: "strong".to_string(),
                inlines: merged,
            })
        }
        BuildFrame::Emphasis { inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Span {
                variant: "emphasis".to_string(),
                inlines: merged,
            })
        }
        BuildFrame::Monospace { inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Span {
                variant: "code".to_string(),
                inlines: merged,
            })
        }
        BuildFrame::Highlight { inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Span {
                variant: "highlight".to_string(),
                inlines: merged,
            })
        }
        BuildFrame::Superscript { inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Span {
                variant: "superscript".to_string(),
                inlines: merged,
            })
        }
        BuildFrame::Subscript { inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Span {
                variant: "subscript".to_string(),
                inlines: merged,
            })
        }
        BuildFrame::Link { url, inlines } => {
            let merged = merge_adjacent_text(inlines);
            Some(AsgNode::Span {
                variant: format!("link:{url}"),
                inlines: merged,
            })
        }
        BuildFrame::Table
        | BuildFrame::TableHead
        | BuildFrame::TableBody
        | BuildFrame::TableFoot
        | BuildFrame::TableRow => {
            // Tables not fully mapped to ASG yet
            Some(AsgNode::Unknown {
                name: "table".to_string(),
            })
        }
        BuildFrame::TableCell { children } | BuildFrame::TableHeaderCell { children } => {
            // Simplified: just pass children through
            if children.len() == 1 {
                Some(children.into_iter().next().unwrap())
            } else {
                Some(AsgNode::Paragraph {
                    inlines: merge_adjacent_text(children),
                })
            }
        }
        BuildFrame::BlockImage { target } => Some(AsgNode::Image { target }),
        BuildFrame::InlineImage { target } => Some(AsgNode::Image { target }),
        BuildFrame::CalloutList { items } => Some(AsgNode::List {
            variant: "callout".to_string(),
            items,
        }),
        BuildFrame::CalloutListItem { children } => {
            let (principal, blocks) = extract_principal(children);
            Some(AsgNode::ListItem { principal, blocks })
        }
        BuildFrame::Anchor | BuildFrame::CrossReference => None,
    }
}

/// Push a finished node into the appropriate field of its parent frame.
fn push_to_parent(stack: &mut [BuildFrame], node: AsgNode, tag_end: &TagEnd) {
    let parent = stack.last_mut().expect("no parent frame");

    match tag_end {
        TagEnd::Header => {
            // Extract header from the pseudo-node and store in Document
            if let BuildFrame::Document { header, .. } = parent
                && let AsgNode::Document {
                    header: Some(h), ..
                } = node
            {
                *header = Some(h);
                return;
            }
        }
        TagEnd::DocumentTitle => {
            // DocumentTitle is inside SectionTitle — push inlines up
            // Parent here is SectionTitle, not Header directly
            if let BuildFrame::SectionTitle { inlines } = parent
                && let AsgNode::Paragraph { inlines: doc_inlines } = node
            {
                inlines.extend(doc_inlines);
                return;
            }
            // Fallback: direct parent is Header
            if let BuildFrame::Header { title } = parent
                && let AsgNode::Paragraph { inlines } = node
            {
                *title = inlines;
                return;
            }
        }
        TagEnd::SectionTitle => {
            // Store title inlines in the Section frame OR Header frame
            if let AsgNode::Paragraph { inlines } = node {
                match parent {
                    BuildFrame::Section { title, .. } | BuildFrame::Header { title } => {
                        *title = inlines;
                        return;
                    }
                    _ => {
                        // Re-wrap and fall through to push_node_to_frame
                        push_node_to_frame(parent, AsgNode::Paragraph { inlines });
                        return;
                    }
                }
            }
        }
        TagEnd::DescriptionTerm => {
            // Collect term into DescriptionList's current_terms
            if let BuildFrame::DescriptionList { current_terms, .. } = parent
                && let AsgNode::Paragraph { inlines } = node
            {
                current_terms.push(inlines);
                return;
            }
        }
        TagEnd::DescriptionDescription => {
            // Finalize a dlistItem with accumulated terms + this description
            if let BuildFrame::DescriptionList {
                items,
                current_terms,
            } = parent
                && let AsgNode::ListItem { principal, blocks } = node
            {
                let terms = std::mem::take(current_terms);
                items.push(AsgNode::DlistItem {
                    terms,
                    principal,
                    blocks,
                });
                return;
            }
        }
        _ => {}
    }

    // Default: push as block/inline child
    push_node_to_frame(parent, node);
}

fn push_node_to_frame(frame: &mut BuildFrame, node: AsgNode) {
    match frame {
        BuildFrame::Document { blocks, .. } => blocks.push(node),
        BuildFrame::Header { title } => title.push(node),
        BuildFrame::DocumentTitle { inlines } => inlines.push(node),
        BuildFrame::Section { blocks, .. } => blocks.push(node),
        BuildFrame::SectionTitle { inlines } => inlines.push(node),
        BuildFrame::Paragraph { inlines } => inlines.push(node),
        BuildFrame::LiteralParagraph { inlines } => inlines.push(node),
        BuildFrame::UnorderedList { items } => items.push(node),
        BuildFrame::OrderedList { items } => items.push(node),
        BuildFrame::ListItem { children } => children.push(node),
        BuildFrame::DescriptionList { items, .. } => items.push(node),
        BuildFrame::DescriptionTerm { inlines } => inlines.push(node),
        BuildFrame::DescriptionDescription { children } => children.push(node),
        BuildFrame::DelimitedBlock { children, .. } => children.push(node),
        BuildFrame::SourceBlock { children } => children.push(node),
        BuildFrame::Admonition { blocks, .. } => blocks.push(node),
        BuildFrame::BlockTitle { inlines } => inlines.push(node),
        BuildFrame::Strong { inlines } => inlines.push(node),
        BuildFrame::Emphasis { inlines } => inlines.push(node),
        BuildFrame::Monospace { inlines } => inlines.push(node),
        BuildFrame::Highlight { inlines } => inlines.push(node),
        BuildFrame::Superscript { inlines } => inlines.push(node),
        BuildFrame::Subscript { inlines } => inlines.push(node),
        BuildFrame::Link { inlines, .. } => inlines.push(node),
        BuildFrame::TableCell { children } | BuildFrame::TableHeaderCell { children } => {
            children.push(node);
        }
        BuildFrame::CalloutList { items } => items.push(node),
        BuildFrame::CalloutListItem { children } => children.push(node),
        BuildFrame::Heading { inlines, .. } => inlines.push(node),
        BuildFrame::Table
        | BuildFrame::TableHead
        | BuildFrame::TableBody
        | BuildFrame::TableFoot
        | BuildFrame::TableRow
        | BuildFrame::Anchor
        | BuildFrame::CrossReference => {
            // Ignored for now
        }
        BuildFrame::BlockImage { .. } | BuildFrame::InlineImage { .. } => {
            // Images don't have children in our model
        }
    }
}

fn push_inline_to_current(stack: &mut [BuildFrame], node: AsgNode) {
    if let Some(frame) = stack.last_mut() {
        push_node_to_frame(frame, node);
    }
}

fn push_block_to_current(stack: &mut [BuildFrame], node: AsgNode) {
    if let Some(frame) = stack.last_mut() {
        push_node_to_frame(frame, node);
    }
}

/// Merge adjacent Text nodes (including \n separators from SoftBreak).
fn merge_adjacent_text(nodes: Vec<AsgNode>) -> Vec<AsgNode> {
    let mut result: Vec<AsgNode> = Vec::new();
    let mut pending_text = String::new();

    for node in nodes {
        if let AsgNode::Text { value } = &node {
            pending_text.push_str(value);
        } else {
            if !pending_text.is_empty() {
                result.push(AsgNode::Text {
                    value: std::mem::take(&mut pending_text),
                });
            }
            result.push(node);
        }
    }

    if !pending_text.is_empty() {
        result.push(AsgNode::Text {
            value: pending_text,
        });
    }

    result
}

/// Extract leading inline content as `principal`, remaining children as `blocks`.
///
/// Our parser emits list item content in two forms:
/// 1. Simple: `Start(ListItem) Text("item") End(ListItem)` — raw Text, no Paragraph wrapper
/// 2. With continuation: `Start(ListItem) Text("item") Start(Paragraph) ... End(Paragraph) End(ListItem)`
///
/// ASG expects: `{ principal: [Text("item")], blocks: [...] }`
fn extract_principal(children: Vec<AsgNode>) -> (Vec<AsgNode>, Vec<AsgNode>) {
    // Collect leading inline nodes (Text, Span, etc.) as principal
    let mut principal_nodes = Vec::new();
    let mut blocks = Vec::new();
    let mut in_principal = true;

    for child in children {
        if in_principal {
            match &child {
                AsgNode::Text { .. } | AsgNode::Span { .. } => {
                    principal_nodes.push(child);
                }
                _ => {
                    in_principal = false;
                    blocks.push(child);
                }
            }
        } else {
            blocks.push(child);
        }
    }

    let principal = merge_adjacent_text(principal_nodes);
    (principal, blocks)
}

/// Convert a DelimitedBlock by kind into the appropriate AsgNode.
fn delimited_block_node(kind: DelimitedBlockKind, children: Vec<AsgNode>) -> AsgNode {
    match kind {
        DelimitedBlockKind::Listing => {
            let inlines = collect_all_inlines(children);
            let merged = merge_adjacent_text(inlines);
            AsgNode::Listing { inlines: merged }
        }
        DelimitedBlockKind::Literal => {
            let inlines = collect_all_inlines(children);
            let merged = merge_adjacent_text(inlines);
            AsgNode::Literal { inlines: merged }
        }
        DelimitedBlockKind::Sidebar => AsgNode::Sidebar { blocks: children },
        DelimitedBlockKind::Example => {
            // Example blocks act as generic containers
            AsgNode::Sidebar { blocks: children }
        }
        DelimitedBlockKind::Quote => AsgNode::Unknown {
            name: "quote".to_string(),
        },
        DelimitedBlockKind::Open => {
            // Open blocks act as generic containers
            if children.len() == 1 {
                children.into_iter().next().unwrap()
            } else {
                AsgNode::Sidebar { blocks: children }
            }
        }
        DelimitedBlockKind::Comment => AsgNode::Unknown {
            name: "comment".to_string(),
        },
        DelimitedBlockKind::Passthrough => {
            let _inlines = collect_all_inlines(children);
            AsgNode::Unknown {
                name: "passthrough".to_string(),
            }
        }
    }
}

/// For verbatim blocks (listing, literal), collect all inline/text content.
fn collect_all_inlines(children: Vec<AsgNode>) -> Vec<AsgNode> {
    let mut inlines = Vec::new();
    for child in children {
        match child {
            AsgNode::Text { .. } => inlines.push(child),
            AsgNode::Paragraph { inlines: inner } => inlines.extend(inner),
            _ => inlines.push(child),
        }
    }
    inlines
}

fn admonition_variant(kind: &AdmonitionKind) -> String {
    match kind {
        AdmonitionKind::Note => "note",
        AdmonitionKind::Tip => "tip",
        AdmonitionKind::Important => "important",
        AdmonitionKind::Warning => "warning",
        AdmonitionKind::Caution => "caution",
    }
    .to_string()
}
