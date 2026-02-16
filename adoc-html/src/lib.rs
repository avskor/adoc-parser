use std::collections::HashMap;
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

struct TocEntry {
    level: u8,
    id: String,
    title: String,
}

struct HtmlRenderer {
    tag_stack: Vec<TagEnd>,
    in_source_block: bool,
    footnotes: Vec<(usize, Option<String>, String)>, // (number, id, text)
    footnote_counter: usize,
    named_footnotes: HashMap<String, usize>, // id → number
    toc_entries: Vec<TocEntry>,
    toc_insert_position: Option<usize>,
    toc_levels: u8,
    in_section_title: bool,
    current_toc_entry: Option<TocEntry>,
}

impl HtmlRenderer {
    fn new() -> Self {
        Self {
            tag_stack: Vec::new(),
            in_source_block: false,
            footnotes: Vec::new(),
            footnote_counter: 0,
            named_footnotes: HashMap::new(),
            toc_entries: Vec::new(),
            toc_insert_position: None,
            toc_levels: 2,
            in_section_title: false,
            current_toc_entry: None,
        }
    }

    fn run<'a>(&mut self, output: &mut String, iter: impl Iterator<Item = Event<'a>>) {
        for event in iter {
            match event {
                Event::Start(tag) => self.start_tag(output, &tag),
                Event::End(tag_end) => self.end_tag(output, &tag_end),
                Event::Text(text) => {
                    if self.in_section_title
                        && let Some(ref mut entry) = self.current_toc_entry {
                            entry.title.push_str(&text);
                    }
                    html_escape(output, &text);
                }
                Event::InlinePassthrough(text) => {
                    output.push_str(&text);
                }
                Event::Code(code) => {
                    if self.in_section_title
                        && let Some(ref mut entry) = self.current_toc_entry {
                            entry.title.push_str(&code);
                    }
                    output.push_str("<code>");
                    html_escape(output, &code);
                    output.push_str("</code>");
                }
                Event::SoftBreak => {
                    output.push('\n');
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
                Event::Attribute { ref name, ref value, .. } => {
                    if name == "toclevels"
                        && let Ok(n) = value.parse::<u8>() {
                            self.toc_levels = n;
                    }
                    // Attributes are metadata, not rendered
                }
                Event::AttributeReference(name) => {
                    output.push('{');
                    output.push_str(&name);
                    output.push('}');
                }
                Event::Footnote { id, text } => {
                    self.footnote_counter += 1;
                    let num = self.footnote_counter;
                    if let Some(ref id) = id {
                        self.named_footnotes.insert(id.to_string(), num);
                    }
                    self.footnotes.push((num, id.as_ref().map(|s| s.to_string()), text.to_string()));
                    output.push_str("<sup class=\"footnote\">[<a id=\"_footnoteref_");
                    output.push_str(&num.to_string());
                    output.push_str("\" href=\"#_footnotedef_");
                    output.push_str(&num.to_string());
                    output.push_str("\">");
                    output.push_str(&num.to_string());
                    output.push_str("</a>]</sup>");
                }
                Event::FootnoteRef { id } => {
                    if let Some(&num) = self.named_footnotes.get(id.as_ref()) {
                        output.push_str("<sup class=\"footnote\">[<a id=\"_footnoteref_");
                        output.push_str(&num.to_string());
                        output.push_str("\" href=\"#_footnotedef_");
                        output.push_str(&num.to_string());
                        output.push_str("\">");
                        output.push_str(&num.to_string());
                        output.push_str("</a>]</sup>");
                    }
                }
                Event::CalloutRef(num) => {
                    output.push_str("<b class=\"conum\">(");
                    output.push_str(&num.to_string());
                    output.push_str(")</b>");
                }
                Event::Toc => {
                    self.toc_insert_position = Some(output.len());
                }
                Event::Include { path, .. } => {
                    output.push_str("<!-- include::");
                    html_escape(output, &path);
                    output.push_str("[] -->\n");
                }
            }
        }

        if let Some(pos) = self.toc_insert_position {
            let toc_html = self.generate_toc();
            output.insert_str(pos, &toc_html);
        }

        if !self.footnotes.is_empty() {
            self.render_footnotes(output);
        }
    }

    fn render_footnotes(&self, output: &mut String) {
        output.push_str("<div id=\"footnotes\">\n<hr>\n");
        for (num, _id, text) in &self.footnotes {
            output.push_str("<div class=\"footnote\" id=\"_footnotedef_");
            output.push_str(&num.to_string());
            output.push_str("\">\n<a href=\"#_footnoteref_");
            output.push_str(&num.to_string());
            output.push_str("\">");
            output.push_str(&num.to_string());
            output.push_str("</a>. ");
            html_escape(output, text);
            output.push_str("\n</div>\n");
        }
        output.push_str("</div>\n");
    }

    fn generate_toc(&self) -> String {
        let min_level: u8 = 2;
        let max_level = min_level + self.toc_levels - 1;

        let entries: Vec<&TocEntry> = self.toc_entries.iter()
            .filter(|e| e.level >= min_level && e.level <= max_level)
            .collect();

        if entries.is_empty() {
            return String::new();
        }

        let mut toc = String::new();
        toc.push_str("<div id=\"toc\" class=\"toc\">\n");
        toc.push_str("<div id=\"toctitle\">Table of Contents</div>\n");

        let mut current_level = min_level;
        toc.push_str("<ul>\n");

        for entry in &entries {
            let level = entry.level;

            // Open nested lists
            while current_level < level {
                toc.push_str("<ul>\n");
                current_level += 1;
            }

            // Close nested lists
            while current_level > level {
                toc.push_str("</li>\n</ul>\n");
                current_level -= 1;
            }

            // Close previous item at same level (not for the very first entry)
            if current_level == level && toc.ends_with("</ul>\n") {
                // Just opened a list, no previous item to close
            } else if current_level == level {
                toc.push_str("</li>\n");
            }

            toc.push_str("<li><a href=\"#");
            html_escape(&mut toc, &entry.id);
            toc.push_str("\">");
            html_escape(&mut toc, &entry.title);
            toc.push_str("</a>\n");

            current_level = level;
        }

        // Close remaining open lists
        while current_level > min_level {
            toc.push_str("</li>\n</ul>\n");
            current_level -= 1;
        }
        toc.push_str("</li>\n</ul>\n");
        toc.push_str("</div>\n");

        toc
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
                if *level >= 2 {
                    self.in_section_title = true;
                    self.current_toc_entry = Some(TocEntry {
                        level: *level,
                        id: id.to_string(),
                        title: String::new(),
                    });
                }
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
            Tag::DescriptionList => {
                output.push_str("<dl>\n");
            }
            Tag::DescriptionTerm => {
                output.push_str("<dt>");
            }
            Tag::DescriptionDescription => {
                output.push_str("<dd>");
            }
            Tag::CalloutList => {
                output.push_str("<div class=\"colist arabic\">\n<ol>\n");
            }
            Tag::CalloutListItem { .. } => {
                output.push_str("<li><p>");
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
            Tag::Table => {
                output.push_str("<table>\n");
            }
            Tag::TableHead => {
                output.push_str("<thead>\n");
            }
            Tag::TableBody => {
                output.push_str("<tbody>\n");
            }
            Tag::TableFoot => {
                output.push_str("<tfoot>\n");
            }
            Tag::TableRow => {
                output.push_str("<tr>\n");
            }
            Tag::TableCell => {
                output.push_str("<td>");
            }
            Tag::TableHeaderCell => {
                output.push_str("<th>");
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
                if self.in_section_title {
                    if let Some(entry) = self.current_toc_entry.take() {
                        self.toc_entries.push(entry);
                    }
                    self.in_section_title = false;
                }
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
            TagEnd::DescriptionList => {
                output.push_str("</dl>\n");
            }
            TagEnd::DescriptionTerm => {
                output.push_str("</dt>\n");
            }
            TagEnd::DescriptionDescription => {
                output.push_str("</dd>\n");
            }
            TagEnd::CalloutList => {
                output.push_str("</ol>\n</div>\n");
            }
            TagEnd::CalloutListItem => {
                output.push_str("</p></li>\n");
            }
            TagEnd::Admonition => {
                output.push_str("</td>\n</tr>\n</table>\n</div>\n");
            }
            TagEnd::Table => {
                output.push_str("</table>\n");
            }
            TagEnd::TableHead => {
                output.push_str("</thead>\n");
            }
            TagEnd::TableBody => {
                output.push_str("</tbody>\n");
            }
            TagEnd::TableFoot => {
                output.push_str("</tfoot>\n");
            }
            TagEnd::TableRow => {
                output.push_str("</tr>\n");
            }
            TagEnd::TableCell => {
                output.push_str("</td>\n");
            }
            TagEnd::TableHeaderCell => {
                output.push_str("</th>\n");
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

    #[test]
    fn test_description_list_html() {
        let html = to_html("CPU:: The brain\nRAM:: Memory");
        assert_eq!(
            html,
            "<dl>\n<dt>CPU</dt>\n<dd>The brain</dd>\n<dt>RAM</dt>\n<dd>Memory</dd>\n</dl>\n"
        );
    }

    #[test]
    fn test_nested_description_list_html() {
        let html = to_html("CPU:: The brain\nSpeed::: Fast");
        assert_eq!(
            html,
            "<dl>\n<dt>CPU</dt>\n<dd>The brain<dl>\n<dt>Speed</dt>\n<dd>Fast</dd>\n</dl>\n</dd>\n</dl>\n"
        );
    }

    #[test]
    fn test_list_continuation_html() {
        let html = to_html("* item\n+\nContinued.");
        assert!(html.contains("<li>item<p>Continued.</p>"));
    }

    #[test]
    fn test_description_list_continuation_html() {
        let html = to_html("Term:: desc\n+\nMore.");
        assert!(html.contains("<dd>desc<p>More.</p>"));
    }

    #[test]
    fn test_inline_passthrough_html() {
        let html = to_html("hello +++<b>bold</b>+++ world");
        assert!(html.contains("hello <b>bold</b> world"));
    }

    #[test]
    fn test_table_html() {
        let html = to_html("|===\n| A | B\n| C | D\n|===");
        assert!(html.contains("<table>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<tr>"));
        assert!(html.contains("<td>A</td>"));
        assert!(html.contains("<td>B</td>"));
        assert!(html.contains("<td>C</td>"));
        assert!(html.contains("<td>D</td>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("</table>"));
        assert!(!html.contains("<thead>"));
    }

    #[test]
    fn test_table_with_header_html() {
        let html = to_html("|===\n| Header 1 | Header 2\n\n| Cell 1 | Cell 2\n| Cell 3 | Cell 4\n|===");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th>Header 1</th>"));
        assert!(html.contains("<th>Header 2</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>Cell 1</td>"));
        assert!(html.contains("<td>Cell 2</td>"));
        assert!(html.contains("<td>Cell 3</td>"));
        assert!(html.contains("<td>Cell 4</td>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("</table>"));
    }

    #[test]
    fn test_table_with_cols_html() {
        let html = to_html("[cols=\"2\"]\n|===\n| A\n| B\n| C\n| D\n|===");
        assert!(html.contains("<table>"));
        assert!(html.contains("<tbody>"));
        // Should have 2 rows of 2 cells
        let td_count = html.matches("<td>").count();
        assert_eq!(td_count, 4);
        let tr_count = html.matches("<tr>").count();
        assert_eq!(tr_count, 2);
        assert!(html.contains("</tbody>"));
        assert!(html.contains("</table>"));
    }

    #[test]
    fn test_table_footer_html() {
        let html = to_html("[%footer]\n|===\n| A | B\n| F1 | F2\n|===");
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>A</td>"));
        assert!(html.contains("<td>B</td>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("<tfoot>"));
        assert!(html.contains("<td>F1</td>"));
        assert!(html.contains("<td>F2</td>"));
        assert!(html.contains("</tfoot>"));
        assert!(!html.contains("<thead>"));
    }

    #[test]
    fn test_footnote_html() {
        let html = to_html("Hello footnote:[This is a note] world.");
        assert!(html.contains("<sup class=\"footnote\">[<a id=\"_footnoteref_1\" href=\"#_footnotedef_1\">1</a>]</sup>"));
        assert!(html.contains("<div id=\"footnotes\">"));
        assert!(html.contains("<hr>"));
        assert!(html.contains("<div class=\"footnote\" id=\"_footnotedef_1\">"));
        assert!(html.contains("<a href=\"#_footnoteref_1\">1</a>. This is a note"));
    }

    #[test]
    fn test_footnote_named_html() {
        let html = to_html("First footnote:fn1[Named note] and again footnote:fn1[].");
        // Definition
        assert!(html.contains("<sup class=\"footnote\">[<a id=\"_footnoteref_1\" href=\"#_footnotedef_1\">1</a>]</sup>"));
        // Reference should use the same number
        let refs: Vec<_> = html.match_indices("_footnoteref_1").collect();
        assert!(refs.len() >= 2, "Expected at least 2 references to footnote 1, got {}", refs.len());
    }

    #[test]
    fn test_footnote_multiple_html() {
        let html = to_html("A footnote:[First] B footnote:[Second] C footnote:[Third].");
        assert!(html.contains("_footnoteref_1"));
        assert!(html.contains("_footnoteref_2"));
        assert!(html.contains("_footnoteref_3"));
        assert!(html.contains("_footnotedef_1"));
        assert!(html.contains("_footnotedef_2"));
        assert!(html.contains("_footnotedef_3"));
        assert!(html.contains(">1</a>. First"));
        assert!(html.contains(">2</a>. Second"));
        assert!(html.contains(">3</a>. Third"));
    }

    #[test]
    fn test_toc_html() {
        let input = "= Document Title\n:toc:\n\n== Section One\n\nContent.\n\n== Section Two\n\nMore content.";
        let html = to_html(input);
        assert!(html.contains("<div id=\"toc\" class=\"toc\">"));
        assert!(html.contains("<div id=\"toctitle\">Table of Contents</div>"));
        assert!(html.contains("<a href=\"#_section_one\">Section One</a>"));
        assert!(html.contains("<a href=\"#_section_two\">Section Two</a>"));
        assert!(html.contains("</ul>"));
        assert!(html.contains("</div>"));
    }

    #[test]
    fn test_toc_levels() {
        let input = "= Document Title\n:toc:\n:toclevels: 3\n\n== Level 2\n\n=== Level 3\n\n==== Level 4\n\n===== Level 5";
        let html = to_html(input);
        assert!(html.contains("<a href=\"#_level_2\">Level 2</a>"));
        assert!(html.contains("<a href=\"#_level_3\">Level 3</a>"));
        assert!(html.contains("<a href=\"#_level_4\">Level 4</a>"));
        // Level 5 should NOT be in TOC (toclevels: 3 → levels 2..4)
        assert!(!html.contains("<a href=\"#_level_5\">Level 5</a>"));
    }

    #[test]
    fn test_toc_default_levels() {
        let input = "= Document Title\n:toc:\n\n== Level 2\n\n=== Level 3\n\n==== Level 4";
        let html = to_html(input);
        assert!(html.contains("<a href=\"#_level_2\">Level 2</a>"));
        assert!(html.contains("<a href=\"#_level_3\">Level 3</a>"));
        // Default toclevels: 2 → levels 2..3, so level 4 should NOT be in TOC
        assert!(!html.contains("<a href=\"#_level_4\">Level 4</a>"));
    }

    #[test]
    fn test_toc_macro_html() {
        let input = "= Document Title\n\n== Before\n\ntoc::[]\n\n== After";
        let html = to_html(input);
        assert!(html.contains("<div id=\"toc\" class=\"toc\">"));
        // TOC should be placed where toc::[] macro is (after "Before" section start)
        let toc_pos = html.find("<div id=\"toc\"").unwrap();
        let before_pos = html.find("Before</h2>").unwrap();
        assert!(toc_pos > before_pos, "TOC should appear after the Before heading");
    }

    #[test]
    fn test_unresolved_include_html() {
        let html = to_html("include::chapter.adoc[]");
        assert_eq!(html, "<!-- include::chapter.adoc[] -->\n");
    }

    #[test]
    fn test_unresolved_include_with_special_chars_html() {
        let html = to_html("include::path/to/<file>.adoc[]");
        assert_eq!(html, "<!-- include::path/to/&lt;file&gt;.adoc[] -->\n");
    }

    #[test]
    fn test_no_toc_without_attribute() {
        let input = "= Document Title\n\n== Section\n\nContent.";
        let html = to_html(input);
        assert!(!html.contains("<div id=\"toc\""));
    }

    #[test]
    fn test_source_block_callouts_html() {
        let input = "[source,ruby]\n----\nrequire 'sinatra' <1>\nget '/hi' do <2>\n  \"Hello World!\" <3>\nend\n----\n<1> Library import\n<2> URL mapping\n<3> Response";
        let html = to_html(input);
        assert!(html.contains("<b class=\"conum\">(1)</b>"));
        assert!(html.contains("<b class=\"conum\">(2)</b>"));
        assert!(html.contains("<b class=\"conum\">(3)</b>"));
        assert!(html.contains("<div class=\"colist arabic\">"));
        assert!(html.contains("<li><p>Library import</p></li>"));
        assert!(html.contains("<li><p>URL mapping</p></li>"));
        assert!(html.contains("<li><p>Response</p></li>"));
    }

    #[test]
    fn test_callout_multiple_per_line_html() {
        let input = "[source]\n----\ncode <1> <2>\n----\n<1> First\n<2> Second";
        let html = to_html(input);
        assert!(html.contains("<b class=\"conum\">(1)</b><b class=\"conum\">(2)</b>"));
        assert!(html.contains("<li><p>First</p></li>"));
        assert!(html.contains("<li><p>Second</p></li>"));
    }
}
