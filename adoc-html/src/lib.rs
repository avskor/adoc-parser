use adoc_parser::{Event, Tag, TagEnd, AdmonitionKind, DelimitedBlockKind};

pub fn push_html<'a>(s: &mut String, iter: impl Iterator<Item = Event<'a>>) {
    let mut renderer = HtmlRenderer::new();
    renderer.run(s, iter);
}

pub fn to_html(input: &str) -> String {
    let parser = adoc_parser::Parser::new(input);
    let mut output = String::new();
    push_html(&mut output, parser);
    output
}

struct HtmlRenderer {
    tag_stack: Vec<TagEnd>,
    in_source_block: bool,
}

impl HtmlRenderer {
    fn new() -> Self {
        Self {
            tag_stack: Vec::new(),
            in_source_block: false,
        }
    }

    fn run<'a>(&mut self, output: &mut String, iter: impl Iterator<Item = Event<'a>>) {
        for event in iter {
            match event {
                Event::Start(tag) => self.start_tag(output, &tag),
                Event::End(tag_end) => self.end_tag(output, &tag_end),
                Event::Text(text) => {
                    if self.in_source_block {
                        html_escape(output, &text);
                    } else {
                        html_escape(output, &text);
                    }
                }
                Event::Code(code) => {
                    output.push_str("<code>");
                    html_escape(output, &code);
                    output.push_str("</code>");
                }
                Event::SoftBreak => {
                    if self.in_source_block {
                        output.push('\n');
                    } else {
                        output.push('\n');
                    }
                }
                Event::HardBreak => {
                    output.push_str("<br>\n");
                }
                Event::ThematicBreak => {
                    output.push_str("<hr>\n");
                }
                Event::PageBreak => {
                    output.push_str("<div style=\"page-break-after: always;\"></div>\n");
                }
                Event::Attribute { .. } => {
                    // Attributes are metadata, not rendered
                }
                Event::AttributeReference(name) => {
                    output.push('{');
                    output.push_str(&name);
                    output.push('}');
                }
            }
        }
    }

    fn start_tag(&mut self, output: &mut String, tag: &Tag) {
        let tag_end = tag.to_end();
        self.tag_stack.push(tag_end);

        match tag {
            Tag::Header => {
                // Document header rendered as header div
                output.push_str("<div class=\"header\">\n");
            }
            Tag::DocumentTitle => {
                output.push_str("<h1>");
            }
            Tag::SectionTitle { level, id } => {
                let h = section_level_to_h(*level);
                output.push_str("<h");
                output.push_str(&h.to_string());
                output.push_str(" id=\"");
                html_escape(output, id);
                output.push_str("\">");
            }
            Tag::Section { .. } => {
                output.push_str("<div class=\"sect\">\n");
            }
            Tag::Paragraph => {
                output.push_str("<p>");
            }
            Tag::LiteralParagraph => {
                output.push_str("<pre>");
            }
            Tag::DelimitedBlock { kind } => {
                match kind {
                    DelimitedBlockKind::Listing => {
                        output.push_str("<div class=\"listingblock\">\n<div class=\"content\">\n<pre>");
                    }
                    DelimitedBlockKind::Literal => {
                        output.push_str("<div class=\"literalblock\">\n<div class=\"content\">\n<pre>");
                    }
                    DelimitedBlockKind::Example => {
                        output.push_str("<div class=\"exampleblock\">\n<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Sidebar => {
                        output.push_str("<div class=\"sidebarblock\">\n<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Quote => {
                        output.push_str("<div class=\"quoteblock\">\n<blockquote>\n");
                    }
                    DelimitedBlockKind::Open => {
                        output.push_str("<div class=\"openblock\">\n<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Comment => {
                        // Comment blocks are not rendered
                    }
                    DelimitedBlockKind::Passthrough => {
                        // Passthrough: content is rendered as-is
                    }
                }
            }
            Tag::SourceBlock { language } => {
                self.in_source_block = true;
                output.push_str("<div class=\"listingblock\">\n<div class=\"content\">\n<pre class=\"highlight\"><code");
                if let Some(lang) = language {
                    output.push_str(" class=\"language-");
                    html_escape(output, lang);
                    output.push('"');
                }
                output.push('>');
            }
            Tag::BlockTitle => {
                output.push_str("<div class=\"title\">");
            }
            Tag::UnorderedList => {
                output.push_str("<ul>\n");
            }
            Tag::OrderedList => {
                output.push_str("<ol>\n");
            }
            Tag::ListItem { .. } => {
                output.push_str("<li>");
            }
            Tag::Admonition { kind } => {
                let label = match kind {
                    AdmonitionKind::Note => "Note",
                    AdmonitionKind::Tip => "Tip",
                    AdmonitionKind::Important => "Important",
                    AdmonitionKind::Warning => "Warning",
                    AdmonitionKind::Caution => "Caution",
                };
                output.push_str("<div class=\"admonitionblock ");
                output.push_str(&label.to_lowercase());
                output.push_str("\">\n<table>\n<tr>\n<td class=\"icon\">\n<div class=\"title\">");
                output.push_str(label);
                output.push_str("</div>\n</td>\n<td class=\"content\">\n");
            }
            Tag::BlockImage { target, alt } => {
                output.push_str("<div class=\"imageblock\">\n<div class=\"content\">\n<img src=\"");
                html_escape(output, target);
                output.push_str("\" alt=\"");
                html_escape(output, alt);
                output.push_str("\">\n</div>\n");
            }
            Tag::InlineImage { target, alt } => {
                output.push_str("<span class=\"image\"><img src=\"");
                html_escape(output, target);
                output.push_str("\" alt=\"");
                html_escape(output, alt);
                output.push_str("\"></span>");
            }
            Tag::Strong => {
                output.push_str("<strong>");
            }
            Tag::Emphasis => {
                output.push_str("<em>");
            }
            Tag::Monospace => {
                output.push_str("<code>");
            }
            Tag::Highlight => {
                output.push_str("<mark>");
            }
            Tag::Superscript => {
                output.push_str("<sup>");
            }
            Tag::Subscript => {
                output.push_str("<sub>");
            }
            Tag::Link { url } => {
                output.push_str("<a href=\"");
                html_escape(output, url);
                output.push_str("\">");
            }
            Tag::CrossReference { target, .. } => {
                output.push_str("<a href=\"#");
                html_escape(output, target);
                output.push_str("\">");
            }
            Tag::Anchor { id } => {
                output.push_str("<a id=\"");
                html_escape(output, id);
                output.push_str("\"></a>");
            }
        }
    }

    fn end_tag(&mut self, output: &mut String, tag_end: &TagEnd) {
        self.tag_stack.pop();

        match tag_end {
            TagEnd::Header => {
                output.push_str("</div>\n");
            }
            TagEnd::DocumentTitle => {
                output.push_str("</h1>\n");
            }
            TagEnd::SectionTitle => {
                let level = self.find_section_level();
                let h = section_level_to_h(level);
                output.push_str("</h");
                output.push_str(&h.to_string());
                output.push_str(">\n");
            }
            TagEnd::Section { .. } => {
                output.push_str("</div>\n");
            }
            TagEnd::Paragraph => {
                output.push_str("</p>\n");
            }
            TagEnd::LiteralParagraph => {
                output.push_str("</pre>\n");
            }
            TagEnd::DelimitedBlock => {
                // Check the tag stack to determine what kind of block we're closing
                output.push_str("</pre>\n</div>\n</div>\n");
            }
            TagEnd::SourceBlock => {
                self.in_source_block = false;
                output.push_str("</code></pre>\n</div>\n</div>\n");
            }
            TagEnd::BlockTitle => {
                output.push_str("</div>\n");
            }
            TagEnd::UnorderedList => {
                output.push_str("</ul>\n");
            }
            TagEnd::OrderedList => {
                output.push_str("</ol>\n");
            }
            TagEnd::ListItem => {
                output.push_str("</li>\n");
            }
            TagEnd::Admonition => {
                output.push_str("</td>\n</tr>\n</table>\n</div>\n");
            }
            TagEnd::BlockImage => {
                output.push_str("</div>\n");
            }
            TagEnd::InlineImage => {
                // Already closed in start_tag
            }
            TagEnd::Strong => {
                output.push_str("</strong>");
            }
            TagEnd::Emphasis => {
                output.push_str("</em>");
            }
            TagEnd::Monospace => {
                output.push_str("</code>");
            }
            TagEnd::Highlight => {
                output.push_str("</mark>");
            }
            TagEnd::Superscript => {
                output.push_str("</sup>");
            }
            TagEnd::Subscript => {
                output.push_str("</sub>");
            }
            TagEnd::Link => {
                output.push_str("</a>");
            }
            TagEnd::CrossReference => {
                output.push_str("</a>");
            }
            TagEnd::Anchor => {
                // Already closed in start_tag
            }
        }
    }

    fn find_section_level(&self) -> u8 {
        for tag_end in self.tag_stack.iter().rev() {
            if let TagEnd::Section { level } = tag_end {
                return *level;
            }
        }
        1
    }
}

fn section_level_to_h(level: u8) -> u8 {
    // AsciiDoc: = (doc title/h1), == (h2), === (h3), etc.
    // level 0 = doc title = h1, level 2 = h2, level 3 = h3...
    if level == 0 {
        1
    } else {
        level
    }
}

fn html_escape(output: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_paragraph() {
        let html = to_html("Hello world.");
        assert_eq!(html, "<p>Hello world.</p>\n");
    }

    #[test]
    fn test_bold_text() {
        let html = to_html("Hello *bold* world.");
        assert_eq!(html, "<p>Hello <strong>bold</strong> world.</p>\n");
    }

    #[test]
    fn test_italic_text() {
        let html = to_html("Hello _italic_ world.");
        assert_eq!(html, "<p>Hello <em>italic</em> world.</p>\n");
    }

    #[test]
    fn test_section() {
        let html = to_html("== My Section\n\nContent.");
        assert!(html.contains("<h2 id=\"_my_section\">My Section</h2>"));
        assert!(html.contains("<p>Content.</p>"));
    }

    #[test]
    fn test_unordered_list() {
        let html = to_html("* item 1\n* item 2");
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>item 1</li>"));
        assert!(html.contains("<li>item 2</li>"));
        assert!(html.contains("</ul>"));
    }

    #[test]
    fn test_ordered_list() {
        let html = to_html(". first\n. second");
        assert!(html.contains("<ol>"));
        assert!(html.contains("<li>first</li>"));
        assert!(html.contains("<li>second</li>"));
        assert!(html.contains("</ol>"));
    }

    #[test]
    fn test_source_block() {
        let html = to_html("[source,rust]\n----\nfn main() {\n    println!(\"hello\");\n}\n----");
        assert!(html.contains("language-rust"));
        assert!(html.contains("fn main()"));
        assert!(html.contains("&quot;hello&quot;"));
    }

    #[test]
    fn test_admonition() {
        let html = to_html("NOTE: This is important.");
        assert!(html.contains("admonitionblock note"));
        assert!(html.contains("This is important."));
    }

    #[test]
    fn test_link() {
        let html = to_html("Visit link:https://example.com[Example].");
        assert!(html.contains("<a href=\"https://example.com\">Example</a>"));
    }

    #[test]
    fn test_thematic_break() {
        let html = to_html("Before.\n\n'''\n\nAfter.");
        assert!(html.contains("<hr>"));
    }

    #[test]
    fn test_html_escaping() {
        let html = to_html("Use <b> & \"quotes\".");
        assert!(html.contains("&lt;b&gt;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&quot;quotes&quot;"));
    }

    #[test]
    fn test_superscript() {
        let html = to_html("E=mc^2^");
        assert!(html.contains("<sup>2</sup>"));
    }

    #[test]
    fn test_subscript() {
        let html = to_html("H~2~O");
        assert!(html.contains("<sub>2</sub>"));
    }

    #[test]
    fn test_document_header() {
        let html = to_html("= My Document\n\nContent.");
        assert!(html.contains("<h1>My Document</h1>"));
    }
}
