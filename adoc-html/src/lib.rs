use std::collections::HashMap;
use adoc_parser::{CellStyle, Event, HAlign, Tag, TagEnd, AdmonitionKind, DelimitedBlockKind, VAlign};

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

struct BlockMeta {
    style: Option<String>,
    id: Option<String>,
    roles: Vec<String>,
    #[allow(dead_code)]
    options: Vec<String>,
}

struct HtmlRenderer {
    tag_stack: Vec<TagEnd>,
    in_source_block: bool,
    delimited_block_stack: Vec<DelimitedBlockKind>,
    footnotes: Vec<(usize, Option<String>, String)>, // (number, id, text)
    footnote_counter: usize,
    named_footnotes: HashMap<String, usize>, // id → number
    toc_entries: Vec<TocEntry>,
    toc_insert_position: Option<usize>,
    toc_levels: u8,
    in_section_title: bool,
    current_toc_entry: Option<TocEntry>,
    pending_block_meta: Option<BlockMeta>,
    kbd_mode: bool,
    menu_target: Option<String>,
    menu_items: Option<String>,
    icon_name: Option<String>,
    icon_attrs: Option<String>,
    stem_variant: Option<String>,
    stem_content: Option<String>,
    stem_block_variant: Option<String>,
    stem_block_content: Option<String>,
    cell_style_stack: Vec<CellStyle>,
}

impl HtmlRenderer {
    fn new() -> Self {
        Self {
            tag_stack: Vec::new(),
            in_source_block: false,
            delimited_block_stack: Vec::new(),
            footnotes: Vec::new(),
            footnote_counter: 0,
            named_footnotes: HashMap::new(),
            toc_entries: Vec::new(),
            toc_insert_position: None,
            toc_levels: 2,
            in_section_title: false,
            current_toc_entry: None,
            pending_block_meta: None,
            kbd_mode: false,
            menu_target: None,
            menu_items: None,
            icon_name: None,
            icon_attrs: None,
            stem_variant: None,
            stem_content: None,
            stem_block_variant: None,
            stem_block_content: None,
            cell_style_stack: Vec::new(),
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
                    if self.kbd_mode {
                        self.render_kbd_keys(output, &text);
                    } else if self.menu_target.is_some() {
                        self.menu_items = Some(text.to_string());
                    } else if self.icon_name.is_some() {
                        self.icon_attrs = Some(text.to_string());
                    } else if self.stem_variant.is_some() {
                        self.stem_content = Some(text.to_string());
                    } else if self.stem_block_variant.is_some() {
                        self.stem_block_content.get_or_insert_with(String::new).push_str(&text);
                    } else {
                        html_escape(output, &text);
                    }
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
                    if self.stem_block_variant.is_some() {
                        self.stem_block_content.get_or_insert_with(String::new).push('\n');
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
                Event::IndexTerm { text } => {
                    html_escape(output, &text);
                }
                Event::ConcealedIndexTerm { .. } => {
                    // Concealed index terms produce no visible output
                }
                Event::BibliographyAnchor { id, label } => {
                    output.push_str("<a id=\"");
                    html_escape(output, &id);
                    output.push_str("\"></a>[");
                    html_escape(output, label.as_ref().unwrap_or(&id));
                    output.push(']');
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
                Event::Author { .. } => {
                    // Author metadata — not rendered to HTML body
                }
                Event::BlockMetadata { style, id, roles, options } => {
                    self.pending_block_meta = Some(BlockMeta {
                        style: style.map(|s| s.into_owned()),
                        id: id.map(|s| s.into_owned()),
                        roles: roles.into_iter().map(|s| s.into_owned()).collect(),
                        options: options.into_iter().map(|s| s.into_owned()).collect(),
                    });
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
        let meta = self.take_block_meta();

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
            Tag::Heading { level } => {
                let h = section_level_to_h(*level);
                output.push_str("<h");
                output.push_str(&h.to_string());
                Self::write_meta_attrs(output, &meta, "");
                output.push('>');
            }
            Tag::Section { .. } => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "sect");
                output.push_str(">\n");
            }
            Tag::Paragraph => {
                output.push_str("<p");
                Self::write_meta_attrs(output, &meta, "");
                output.push('>');
            }
            Tag::LiteralParagraph => {
                output.push_str("<pre");
                Self::write_meta_attrs(output, &meta, "");
                output.push('>');
            }
            Tag::DelimitedBlock { kind } => {
                self.delimited_block_stack.push(*kind);
                match kind {
                    DelimitedBlockKind::Listing => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "listingblock");
                        output.push_str(">\n<div class=\"content\">\n<pre>");
                    }
                    DelimitedBlockKind::Literal => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "literalblock");
                        output.push_str(">\n<div class=\"content\">\n<pre>");
                    }
                    DelimitedBlockKind::Example => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "exampleblock");
                        output.push_str(">\n<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Sidebar => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "sidebarblock");
                        output.push_str(">\n<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Quote => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "quoteblock");
                        output.push_str(">\n<blockquote>\n");
                    }
                    DelimitedBlockKind::Open => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "openblock");
                        output.push_str(">\n<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Comment => {
                        // Comment blocks are not rendered
                    }
                    DelimitedBlockKind::Passthrough => {
                        // Passthrough: content is rendered as-is
                    }
                    DelimitedBlockKind::Verse => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "verseblock");
                        output.push_str(">\n<pre class=\"content\">");
                    }
                }
            }
            Tag::SourceBlock { language } => {
                self.in_source_block = true;
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "listingblock");
                output.push_str(">\n<div class=\"content\">\n<pre class=\"highlight\"><code");
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
            Tag::UnorderedList { has_checklist: true } => {
                output.push_str("<ul");
                Self::write_meta_attrs(output, &meta, "checklist");
                output.push_str(">\n");
            }
            Tag::UnorderedList { has_checklist: false } => {
                output.push_str("<ul");
                Self::write_meta_attrs(output, &meta, "");
                output.push_str(">\n");
            }
            Tag::OrderedList { start, reversed } => {
                output.push_str("<ol");
                if let Some(ref m) = meta
                    && let Some(ref style) = m.style
                {
                    let type_attr = match style.as_str() {
                        "loweralpha" => Some("a"),
                        "upperalpha" => Some("A"),
                        "lowerroman" => Some("i"),
                        "upperroman" => Some("I"),
                        _ => None,
                    };
                    if let Some(t) = type_attr {
                        output.push_str(" type=\"");
                        output.push_str(t);
                        output.push('"');
                    }
                }
                if let Some(s) = start {
                    use std::fmt::Write;
                    let _ = write!(output, " start=\"{}\"", s);
                }
                if *reversed {
                    output.push_str(" reversed");
                }
                Self::write_meta_attrs(output, &meta, "");
                output.push_str(">\n");
            }
            Tag::ListItem { checked: Some(true), .. } => {
                output.push_str("<li><input type=\"checkbox\" disabled checked> ");
            }
            Tag::ListItem { checked: Some(false), .. } => {
                output.push_str("<li><input type=\"checkbox\" disabled> ");
            }
            Tag::ListItem { checked: None, .. } => {
                output.push_str("<li>");
            }
            Tag::DescriptionList => {
                output.push_str("<dl");
                Self::write_meta_attrs(output, &meta, "");
                output.push_str(">\n");
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
                let adm_class = format!("admonitionblock {}", label.to_lowercase());
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, &adm_class);
                output.push_str(">\n<table>\n<tr>\n<td class=\"icon\">\n<div class=\"title\">");
                output.push_str(label);
                output.push_str("</div>\n</td>\n<td class=\"content\">\n");
            }
            Tag::Table => {
                output.push_str("<table");
                Self::write_meta_attrs(output, &meta, "");
                output.push_str(">\n");
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
            Tag::TableCell { colspan, rowspan, style, halign, valign } => {
                self.cell_style_stack.push(*style);
                output.push_str("<td");
                if *colspan > 1 {
                    output.push_str(&format!(" colspan=\"{}\"", colspan));
                }
                if *rowspan > 1 {
                    output.push_str(&format!(" rowspan=\"{}\"", rowspan));
                }
                Self::write_align_style(output, halign, valign);
                output.push('>');
                match style {
                    CellStyle::Emphasis => output.push_str("<em>"),
                    CellStyle::Strong => output.push_str("<strong>"),
                    CellStyle::Monospace | CellStyle::Literal => output.push_str("<code>"),
                    _ => {}
                }
            }
            Tag::TableHeaderCell { colspan, rowspan, style, halign, valign } => {
                self.cell_style_stack.push(*style);
                output.push_str("<th");
                if *colspan > 1 {
                    output.push_str(&format!(" colspan=\"{}\"", colspan));
                }
                if *rowspan > 1 {
                    output.push_str(&format!(" rowspan=\"{}\"", rowspan));
                }
                Self::write_align_style(output, halign, valign);
                output.push('>');
                match style {
                    CellStyle::Emphasis => output.push_str("<em>"),
                    CellStyle::Strong => output.push_str("<strong>"),
                    CellStyle::Monospace | CellStyle::Literal => output.push_str("<code>"),
                    _ => {}
                }
            }
            Tag::BlockImage { target, alt } => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "imageblock");
                output.push_str(">\n<div class=\"content\">\n<img src=\"");
                html_escape(output, target);
                output.push_str("\" alt=\"");
                html_escape(output, alt);
                output.push_str("\">\n</div>\n");
            }
            Tag::BlockVideo { target, attrs } => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "videoblock");
                output.push_str(">\n<div class=\"content\">\n");
                render_video_tag(output, target, attrs);
            }
            Tag::BlockAudio { target, attrs } => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "audioblock");
                output.push_str(">\n<div class=\"content\">\n");
                render_audio_tag(output, target, attrs);
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
            Tag::InlineSpan { id, roles } => {
                output.push_str("<span");
                if let Some(id) = id {
                    output.push_str(" id=\"");
                    html_escape(output, id);
                    output.push('"');
                }
                if !roles.is_empty() {
                    output.push_str(" class=\"");
                    for (i, role) in roles.iter().enumerate() {
                        if i > 0 {
                            output.push(' ');
                        }
                        html_escape(output, role);
                    }
                    output.push('"');
                }
                output.push('>');
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
            Tag::Keyboard => {
                self.kbd_mode = true;
            }
            Tag::Button => {
                output.push_str("<b class=\"button\">");
            }
            Tag::Menu { target } => {
                self.menu_target = Some(target.to_string());
            }
            Tag::Icon { name } => {
                self.icon_name = Some(name.to_string());
                self.icon_attrs = None;
            }
            Tag::Stem { variant } => {
                self.stem_variant = Some(variant.to_string());
                self.stem_content = None;
            }
            Tag::StemBlock { variant } => {
                self.stem_block_variant = Some(variant.to_string());
                self.stem_block_content = None;
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "stemblock");
                output.push_str(">\n<div class=\"content\">\n");
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
            TagEnd::Heading { level } => {
                let h = section_level_to_h(*level);
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
                match self.delimited_block_stack.pop() {
                    Some(DelimitedBlockKind::Listing | DelimitedBlockKind::Literal) => {
                        output.push_str("</pre>\n</div>\n</div>\n");
                    }
                    Some(DelimitedBlockKind::Quote) => {
                        output.push_str("</blockquote>\n</div>\n");
                    }
                    Some(DelimitedBlockKind::Verse) => {
                        output.push_str("</pre>\n</div>\n");
                    }
                    Some(DelimitedBlockKind::Example | DelimitedBlockKind::Sidebar
                         | DelimitedBlockKind::Open) => {
                        output.push_str("</div>\n</div>\n");
                    }
                    _ => {
                        output.push_str("</div>\n");
                    }
                }
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
                let style = self.cell_style_stack.pop().unwrap_or_default();
                match style {
                    CellStyle::Emphasis => output.push_str("</em>"),
                    CellStyle::Strong => output.push_str("</strong>"),
                    CellStyle::Monospace | CellStyle::Literal => output.push_str("</code>"),
                    _ => {}
                }
                output.push_str("</td>\n");
            }
            TagEnd::TableHeaderCell => {
                let style = self.cell_style_stack.pop().unwrap_or_default();
                match style {
                    CellStyle::Emphasis => output.push_str("</em>"),
                    CellStyle::Strong => output.push_str("</strong>"),
                    CellStyle::Monospace | CellStyle::Literal => output.push_str("</code>"),
                    _ => {}
                }
                output.push_str("</th>\n");
            }
            TagEnd::BlockImage => {
                output.push_str("</div>\n");
            }
            TagEnd::BlockVideo => {
                output.push_str("</div>\n");
            }
            TagEnd::BlockAudio => {
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
            TagEnd::InlineSpan => {
                output.push_str("</span>");
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
            TagEnd::Keyboard => {
                self.kbd_mode = false;
            }
            TagEnd::Button => {
                output.push_str("</b>");
            }
            TagEnd::Menu => {
                self.render_menu(output);
            }
            TagEnd::Icon => {
                self.render_icon(output);
            }
            TagEnd::Stem => {
                self.render_inline_stem(output);
            }
            TagEnd::StemBlock => {
                self.render_stem_block(output);
            }
            TagEnd::Anchor => {
                // Already closed in start_tag
            }
        }
    }

    fn take_block_meta(&mut self) -> Option<BlockMeta> {
        self.pending_block_meta.take()
    }

    /// Write `style="text-align:...; vertical-align:..."` attribute if alignment is non-default.
    fn write_align_style(output: &mut String, halign: &HAlign, valign: &VAlign) {
        let ha = match halign {
            HAlign::Center => Some("text-align: center"),
            HAlign::Right => Some("text-align: right"),
            HAlign::Left => None,
        };
        let va = match valign {
            VAlign::Middle => Some("vertical-align: middle"),
            VAlign::Bottom => Some("vertical-align: bottom"),
            VAlign::Top => None,
        };
        if ha.is_some() || va.is_some() {
            output.push_str(" style=\"");
            if let Some(h) = ha {
                output.push_str(h);
                output.push(';');
                if va.is_some() {
                    output.push(' ');
                }
            }
            if let Some(v) = va {
                output.push_str(v);
                output.push(';');
            }
            output.push('"');
        }
    }

    /// Write HTML id and class attributes from block metadata into an already-started tag.
    /// `default_class` is the base class (e.g. "sect", "listingblock").
    /// If no metadata and default_class is non-empty, writes ` class="default_class"`.
    /// Roles from metadata are appended to the class list.
    fn write_meta_attrs(output: &mut String, meta: &Option<BlockMeta>, default_class: &str) {
        if let Some(m) = meta
            && let Some(ref id) = m.id
        {
            output.push_str(" id=\"");
            html_escape(output, id);
            output.push('"');
        }
        let style = meta.as_ref().and_then(|m| m.style.as_deref());
        let roles = meta.as_ref().map(|m| &m.roles[..]).unwrap_or(&[]);
        if !default_class.is_empty() || style.is_some() || !roles.is_empty() {
            output.push_str(" class=\"");
            let mut first = true;
            if !default_class.is_empty() {
                output.push_str(default_class);
                first = false;
            }
            if let Some(s) = style {
                if !first {
                    output.push(' ');
                }
                html_escape(output, s);
                first = false;
            }
            for role in roles {
                if !first {
                    output.push(' ');
                }
                html_escape(output, role);
                first = false;
            }
            output.push('"');
        }
    }

    fn render_kbd_keys(&self, output: &mut String, text: &str) {
        let keys: Vec<&str> = text.split('+').map(|k| k.trim()).collect();
        if keys.len() == 1 {
            output.push_str("<kbd>");
            html_escape(output, keys[0]);
            output.push_str("</kbd>");
        } else {
            output.push_str("<span class=\"keyseq\">");
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    output.push('+');
                }
                output.push_str("<kbd>");
                html_escape(output, key);
                output.push_str("</kbd>");
            }
            output.push_str("</span>");
        }
    }

    fn render_menu(&mut self, output: &mut String) {
        let target = match self.menu_target.take() {
            Some(t) => t,
            None => return,
        };
        let items = self.menu_items.take();

        let items_str = items.unwrap_or_default();
        if items_str.is_empty() {
            // menu:File[] — just the menu name
            output.push_str("<span class=\"menu\">");
            html_escape(output, &target);
            output.push_str("</span>");
        } else {
            let parts: Vec<&str> = items_str.split('>').map(|s| s.trim()).collect();
            output.push_str("<span class=\"menuseq\"><span class=\"menu\">");
            html_escape(output, &target);
            output.push_str("</span>");
            for (i, part) in parts.iter().enumerate() {
                output.push_str("\u{00A0}\u{25B8} ");
                if i < parts.len() - 1 {
                    output.push_str("<span class=\"submenu\">");
                    html_escape(output, part);
                    output.push_str("</span>");
                } else {
                    output.push_str("<span class=\"menuitem\">");
                    html_escape(output, part);
                    output.push_str("</span>");
                }
            }
            output.push_str("</span>");
        }
    }

    fn render_icon(&mut self, output: &mut String) {
        let name = match self.icon_name.take() {
            Some(n) => n,
            None => return,
        };
        let attrs_str = self.icon_attrs.take().unwrap_or_default();

        let mut size = None;
        let mut rotate = None;
        let mut flip = None;
        let mut role = None;
        let mut link = None;
        let mut title = None;

        if !attrs_str.is_empty() {
            for (i, part) in attrs_str.split(',').enumerate() {
                let part = part.trim();
                if let Some((key, val)) = part.split_once('=') {
                    match key.trim() {
                        "role" => role = Some(val.trim().to_string()),
                        "link" => link = Some(val.trim().to_string()),
                        "title" => title = Some(val.trim().to_string()),
                        "rotate" => rotate = Some(val.trim().to_string()),
                        "flip" => flip = Some(val.trim().to_string()),
                        _ => {}
                    }
                } else if i == 0 {
                    // First positional = size
                    size = Some(part.to_string());
                }
            }
        }

        // Build class list
        let mut classes = format!("fa fa-{name}");
        if let Some(ref s) = size {
            classes.push_str(&format!(" fa-{s}"));
        }
        if let Some(ref r) = rotate {
            classes.push_str(&format!(" fa-rotate-{r}"));
        }
        if let Some(ref f) = flip {
            classes.push_str(&format!(" fa-flip-{f}"));
        }
        if let Some(ref r) = role {
            classes.push(' ');
            classes.push_str(r);
        }

        if let Some(ref href) = link {
            output.push_str("<a class=\"icon\" href=\"");
            html_escape(output, href);
            output.push_str("\">");
        }

        output.push_str("<i class=\"");
        html_escape(output, &classes);
        output.push('"');
        if let Some(ref t) = title {
            output.push_str(" title=\"");
            html_escape(output, t);
            output.push('"');
        }
        output.push_str("></i>");

        if link.is_some() {
            output.push_str("</a>");
        }
    }

    fn render_inline_stem(&mut self, output: &mut String) {
        let variant = match self.stem_variant.take() {
            Some(v) => v,
            None => return,
        };
        let content = self.stem_content.take().unwrap_or_default();

        if variant == "latexmath" {
            output.push_str("\\(");
            output.push_str(&content);
            output.push_str("\\)");
        } else {
            // stem and asciimath
            output.push_str("\\$");
            output.push_str(&content);
            output.push_str("\\$");
        }
    }

    fn render_stem_block(&mut self, output: &mut String) {
        let variant = match self.stem_block_variant.take() {
            Some(v) => v,
            None => return,
        };
        let content = self.stem_block_content.take().unwrap_or_default();

        if variant == "latexmath" {
            output.push_str("\\[");
            output.push_str(&content);
            output.push_str("\\]");
        } else {
            output.push_str("\\$");
            output.push_str(&content);
            output.push_str("\\$");
        }
        output.push_str("\n</div>\n</div>\n");
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

struct MediaAttrs<'a> {
    width: Option<&'a str>,
    height: Option<&'a str>,
    poster: Option<&'a str>,
    start: Option<&'a str>,
    end: Option<&'a str>,
    autoplay: bool,
    loop_: bool,
    nocontrols: bool,
}

fn parse_media_attrs(attrs: &str) -> MediaAttrs<'_> {
    let mut result = MediaAttrs {
        width: None,
        height: None,
        poster: None,
        start: None,
        end: None,
        autoplay: false,
        loop_: false,
        nocontrols: false,
    };
    if attrs.is_empty() {
        return result;
    }

    // Split on commas, but respect quoted strings
    let mut parts: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    for (i, ch) in attrs.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                parts.push(&attrs[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&attrs[start..]);

    for part in parts {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "width" => result.width = Some(value),
                "height" => result.height = Some(value),
                "poster" => result.poster = Some(value),
                "start" => result.start = Some(value),
                "end" => result.end = Some(value),
                "options" => {
                    for opt in value.split(',') {
                        let opt = opt.trim();
                        match opt {
                            "autoplay" => result.autoplay = true,
                            "loop" => result.loop_ = true,
                            "nocontrols" => result.nocontrols = true,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
    result
}

fn render_video_tag(output: &mut String, target: &str, attrs: &str) {
    let media = parse_media_attrs(attrs);

    output.push_str("<video src=\"");
    html_escape(output, target);
    // Append time fragment if start/end present
    match (media.start, media.end) {
        (Some(s), Some(e)) => {
            output.push_str("#t=");
            output.push_str(s);
            output.push(',');
            output.push_str(e);
        }
        (Some(s), None) => {
            output.push_str("#t=");
            output.push_str(s);
        }
        (None, Some(e)) => {
            output.push_str("#t=,");
            output.push_str(e);
        }
        (None, None) => {}
    }
    output.push('"');

    if let Some(w) = media.width {
        output.push_str(" width=\"");
        output.push_str(w);
        output.push('"');
    }
    if let Some(h) = media.height {
        output.push_str(" height=\"");
        output.push_str(h);
        output.push('"');
    }
    if let Some(p) = media.poster {
        output.push_str(" poster=\"");
        html_escape(output, p);
        output.push('"');
    }
    if !media.nocontrols {
        output.push_str(" controls");
    }
    if media.autoplay {
        output.push_str(" autoplay");
    }
    if media.loop_ {
        output.push_str(" loop");
    }
    output.push_str(">\nYour browser does not support the video tag.\n</video>\n</div>\n");
}

fn render_audio_tag(output: &mut String, target: &str, attrs: &str) {
    let media = parse_media_attrs(attrs);

    output.push_str("<audio src=\"");
    html_escape(output, target);
    output.push('"');

    if !media.nocontrols {
        output.push_str(" controls");
    }
    if media.autoplay {
        output.push_str(" autoplay");
    }
    if media.loop_ {
        output.push_str(" loop");
    }
    output.push_str(">\nYour browser does not support the audio tag.\n</audio>\n</div>\n");
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
        assert!(!html.contains("type="));
        assert!(!html.contains("start="));
        assert!(!html.contains("reversed"));
        assert!(html.contains("<li>first</li>"));
        assert!(html.contains("<li>second</li>"));
        assert!(html.contains("</ol>"));
    }

    #[test]
    fn test_ordered_list_loweralpha() {
        let html = to_html("[loweralpha]\n. a\n. b");
        assert!(html.contains("<ol type=\"a\""));
        assert!(html.contains("class=\"loweralpha\""));
    }

    #[test]
    fn test_ordered_list_upperroman() {
        let html = to_html("[upperroman]\n. x\n. y");
        assert!(html.contains("<ol type=\"I\""));
        assert!(html.contains("class=\"upperroman\""));
    }

    #[test]
    fn test_ordered_list_start() {
        let html = to_html("[start=3]\n. x\n. y");
        assert!(html.contains("start=\"3\""));
    }

    #[test]
    fn test_ordered_list_reversed() {
        let html = to_html("[%reversed]\n. z\n. y");
        assert!(html.contains(" reversed"));
    }

    #[test]
    fn test_ordered_list_combined() {
        let html = to_html("[loweralpha,start=2]\n. x");
        assert!(html.contains("type=\"a\""));
        assert!(html.contains("start=\"2\""));
        assert!(html.contains("class=\"loweralpha\""));
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

    #[test]
    fn test_checklist_html() {
        let html = to_html("* [x] Done\n* [ ] Todo");
        assert!(html.contains("<ul class=\"checklist\">"));
        assert!(html.contains("<li><input type=\"checkbox\" disabled checked> Done</li>"));
        assert!(html.contains("<li><input type=\"checkbox\" disabled> Todo</li>"));
        assert!(html.contains("</ul>"));
    }

    #[test]
    fn test_checklist_mixed_html() {
        let html = to_html("* [x] Checked\n* Regular\n* [ ] Unchecked");
        assert!(html.contains("<ul class=\"checklist\">"));
        assert!(html.contains("<li><input type=\"checkbox\" disabled checked> Checked</li>"));
        assert!(html.contains("<li>Regular</li>"));
        assert!(html.contains("<li><input type=\"checkbox\" disabled> Unchecked</li>"));
    }

    #[test]
    fn test_regular_list_no_checklist_class() {
        let html = to_html("* item 1\n* item 2");
        assert!(html.contains("<ul>"));
        assert!(!html.contains("checklist"));
    }

    #[test]
    fn test_verse_block_html() {
        let html = to_html("[verse]\n____\nline one\nline two\n____");
        assert_eq!(
            html,
            "<div class=\"verseblock\">\n<pre class=\"content\">line one\nline two</pre>\n</div>\n"
        );
    }

    #[test]
    fn test_verse_block_with_formatting_html() {
        let html = to_html("[verse]\n____\nhello *bold* world\nand _italic_ too\n____");
        assert!(html.contains("<div class=\"verseblock\">"));
        assert!(html.contains("<pre class=\"content\">"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("</pre>\n</div>\n"));
    }

    #[test]
    fn test_table_colspan_html() {
        let html = to_html("|===\n| A 2+| B spans\n| C | D | E\n|===");
        assert!(html.contains("<td>A</td>"));
        assert!(html.contains("<td colspan=\"2\">B spans</td>"));
        assert!(html.contains("<td>C</td>"));
        assert!(html.contains("<td>D</td>"));
        assert!(html.contains("<td>E</td>"));
    }

    #[test]
    fn test_table_rowspan_html() {
        let html = to_html("|===\n.2+| A | B\n| C\n|===");
        assert!(html.contains("<td rowspan=\"2\">A</td>"));
        assert!(html.contains("<td>B</td>"));
        assert!(html.contains("<td>C</td>"));
        // Should have 2 rows
        assert_eq!(html.matches("<tr>").count(), 2);
    }

    #[test]
    fn test_table_colspan_rowspan_html() {
        let html = to_html("|===\n2.3+| cell | B\n| C\n| D\n|===");
        assert!(html.contains("<td colspan=\"2\" rowspan=\"3\">cell</td>"));
    }

    #[test]
    fn test_table_cell_style_emphasis_html() {
        let html = to_html("|===\ne| italic\n|===");
        assert!(html.contains("<td><em>italic</em></td>"));
    }

    #[test]
    fn test_table_cell_style_strong_html() {
        let html = to_html("|===\ns| bold\n|===");
        assert!(html.contains("<td><strong>bold</strong></td>"));
    }

    #[test]
    fn test_table_cell_style_monospace_html() {
        let html = to_html("|===\nm| code\n|===");
        assert!(html.contains("<td><code>code</code></td>"));
    }

    #[test]
    fn test_table_cell_style_literal_html() {
        let html = to_html("|===\nl| literal\n|===");
        assert!(html.contains("<td><code>literal</code></td>"));
    }

    #[test]
    fn test_table_cell_style_header_in_body_html() {
        let html = to_html("|===\nh| header cell\n|===");
        assert!(html.contains("<th>header cell</th>"));
    }

    #[test]
    fn test_table_cell_style_with_colspan_html() {
        let html = to_html("|===\n2+e| wide italic | B\n| C | D\n|===");
        assert!(html.contains("<td colspan=\"2\"><em>wide italic</em></td>"));
    }

    #[test]
    fn test_table_cell_style_no_false_positive_html() {
        // "data" ends with 'a' but should NOT be treated as AsciiDoc style
        let html = to_html("|===\n| data | more\n|===");
        assert!(html.contains("<td>data</td>"));
        assert!(html.contains("<td>more</td>"));
    }

    #[test]
    fn test_table_cols_alignment_html() {
        let html = to_html("[cols=\"<,^,>\"]\n|===\n| A | B | C\n|===");
        assert!(html.contains("<td>A</td>"), "Left-aligned should have no style");
        assert!(html.contains("<td style=\"text-align: center;\">B</td>"), "Center should have text-align: center");
        assert!(html.contains("<td style=\"text-align: right;\">C</td>"), "Right should have text-align: right");
    }

    #[test]
    fn test_table_cell_align_html() {
        let html = to_html("|===\n^| centered\n|===");
        assert!(html.contains("<td style=\"text-align: center;\">centered</td>"));
    }

    #[test]
    fn test_table_cell_combined_align_html() {
        let html = to_html("|===\n>.^| text\n|===");
        assert!(html.contains("<td style=\"text-align: right; vertical-align: middle;\">text</td>"));
    }

    #[test]
    fn test_table_cell_override_cols_align_html() {
        // cols says left, cell overrides to center
        let html = to_html("[cols=\"<,<\"]\n|===\n^| centered | normal\n|===");
        assert!(html.contains("<td style=\"text-align: center;\">centered</td>"));
        assert!(html.contains("<td>normal</td>"));
    }

    #[test]
    fn test_table_valign_only_html() {
        let html = to_html("|===\n.>| bottom\n|===");
        assert!(html.contains("<td style=\"vertical-align: bottom;\">bottom</td>"));
    }

    #[test]
    fn test_table_cols_valign_html() {
        let html = to_html("[cols=\".^,1\"]\n|===\n| A | B\n|===");
        assert!(html.contains("<td style=\"vertical-align: middle;\">A</td>"));
        assert!(html.contains("<td>B</td>"));
    }

    #[test]
    fn test_kbd_single_key_html() {
        let html = to_html("kbd:[F11]");
        assert_eq!(html, "<p><kbd>F11</kbd></p>\n");
    }

    #[test]
    fn test_kbd_combo_html() {
        let html = to_html("kbd:[Ctrl+C]");
        assert_eq!(html, "<p><span class=\"keyseq\"><kbd>Ctrl</kbd>+<kbd>C</kbd></span></p>\n");
    }

    #[test]
    fn test_btn_html() {
        let html = to_html("btn:[OK]");
        assert_eq!(html, "<p><b class=\"button\">OK</b></p>\n");
    }

    #[test]
    fn test_menu_html() {
        let html = to_html("menu:File[Save As]");
        assert_eq!(
            html,
            "<p><span class=\"menuseq\"><span class=\"menu\">File</span>\u{00A0}\u{25B8} <span class=\"menuitem\">Save As</span></span></p>\n"
        );
    }

    #[test]
    fn test_menu_no_items_html() {
        let html = to_html("menu:File[]");
        assert_eq!(html, "<p><span class=\"menu\">File</span></p>\n");
    }

    #[test]
    fn test_icon_basic_html() {
        let html = to_html("icon:heart[]");
        assert_eq!(html, "<p><i class=\"fa fa-heart\"></i></p>\n");
    }

    #[test]
    fn test_icon_size_html() {
        let html = to_html("icon:heart[2x]");
        assert_eq!(html, "<p><i class=\"fa fa-heart fa-2x\"></i></p>\n");
    }

    #[test]
    fn test_icon_role_html() {
        let html = to_html("icon:tags[role=blue]");
        assert_eq!(html, "<p><i class=\"fa fa-tags blue\"></i></p>\n");
    }

    #[test]
    fn test_icon_title_html() {
        let html = to_html("icon:info[title=Info]");
        assert_eq!(html, "<p><i class=\"fa fa-info\" title=\"Info\"></i></p>\n");
    }

    #[test]
    fn test_icon_rotate_html() {
        let html = to_html("icon:shield[rotate=90]");
        assert_eq!(html, "<p><i class=\"fa fa-shield fa-rotate-90\"></i></p>\n");
    }

    #[test]
    fn test_icon_flip_html() {
        let html = to_html("icon:shield[flip=vertical]");
        assert_eq!(html, "<p><i class=\"fa fa-shield fa-flip-vertical\"></i></p>\n");
    }

    #[test]
    fn test_icon_link_html() {
        let html = to_html("icon:download[link=https://example.com]");
        assert_eq!(html, "<p><a class=\"icon\" href=\"https://example.com\"><i class=\"fa fa-download\"></i></a></p>\n");
    }

    #[test]
    fn test_icon_combined_html() {
        let html = to_html("icon:heart[2x,role=red]");
        assert_eq!(html, "<p><i class=\"fa fa-heart fa-2x red\"></i></p>\n");
    }

    #[test]
    fn test_menu_submenus_html() {
        let html = to_html("menu:File[New > Doc]");
        assert_eq!(
            html,
            "<p><span class=\"menuseq\"><span class=\"menu\">File</span>\u{00A0}\u{25B8} <span class=\"submenu\">New</span>\u{00A0}\u{25B8} <span class=\"menuitem\">Doc</span></span></p>\n"
        );
    }

    // Stem macro tests

    #[test]
    fn test_stem_inline_html() {
        let html = to_html("stem:[x^2]");
        assert_eq!(html, "<p>\\$x^2\\$</p>\n");
    }

    #[test]
    fn test_latexmath_inline_html() {
        let html = to_html("latexmath:[C = \\alpha]");
        assert_eq!(html, "<p>\\(C = \\alpha\\)</p>\n");
    }

    #[test]
    fn test_asciimath_inline_html() {
        let html = to_html("asciimath:[sqrt(4)]");
        assert_eq!(html, "<p>\\$sqrt(4)\\$</p>\n");
    }

    #[test]
    fn test_stem_no_escape_html() {
        let html = to_html("stem:[a < b]");
        assert!(html.contains("a < b"), "stem content should not be HTML-escaped");
        assert!(!html.contains("&lt;"), "stem content must not contain &lt;");
    }

    #[test]
    fn test_stem_block_html() {
        let html = to_html("[stem]\n++++\nx^2\n++++");
        assert!(html.contains("<div class=\"stemblock\">"));
        assert!(html.contains("<div class=\"content\">"));
        assert!(html.contains("\\$x^2\\$"));
        assert!(html.contains("</div>\n</div>\n"));
    }

    #[test]
    fn test_latexmath_block_html() {
        let html = to_html("[latexmath]\n++++\nx^2\n++++");
        assert!(html.contains("<div class=\"stemblock\">"));
        assert!(html.contains("\\[x^2\\]"));
    }

    #[test]
    fn test_video_basic_html() {
        let html = to_html("video::video.mp4[]");
        assert_eq!(
            html,
            "<div class=\"videoblock\">\n<div class=\"content\">\n<video src=\"video.mp4\" controls>\nYour browser does not support the video tag.\n</video>\n</div>\n</div>\n"
        );
    }

    #[test]
    fn test_video_attrs_html() {
        let html = to_html("video::video.mp4[width=640,height=480,poster=preview.jpg]");
        assert!(html.contains("<video src=\"video.mp4\" width=\"640\" height=\"480\" poster=\"preview.jpg\" controls>"));
    }

    #[test]
    fn test_video_options_html() {
        let html = to_html("video::video.mp4[options=\"autoplay,loop,nocontrols\"]");
        assert!(html.contains("<video src=\"video.mp4\" autoplay loop>"));
        assert!(!html.contains("controls"));
    }

    #[test]
    fn test_video_start_end_html() {
        let html = to_html("video::video.mp4[start=60,end=120]");
        assert!(html.contains("src=\"video.mp4#t=60,120\""));
    }

    #[test]
    fn test_video_start_only_html() {
        let html = to_html("video::video.mp4[start=30]");
        assert!(html.contains("src=\"video.mp4#t=30\""));
    }

    #[test]
    fn test_audio_basic_html() {
        let html = to_html("audio::audio.mp3[]");
        assert_eq!(
            html,
            "<div class=\"audioblock\">\n<div class=\"content\">\n<audio src=\"audio.mp3\" controls>\nYour browser does not support the audio tag.\n</audio>\n</div>\n</div>\n"
        );
    }

    #[test]
    fn test_audio_options_html() {
        let html = to_html("audio::audio.mp3[options=\"autoplay,loop\"]");
        assert!(html.contains("<audio src=\"audio.mp3\" controls autoplay loop>"));
    }

    #[test]
    fn test_audio_nocontrols_html() {
        let html = to_html("audio::audio.mp3[options=\"nocontrols\"]");
        assert!(html.contains("<audio src=\"audio.mp3\">"));
        assert!(!html.contains("controls"));
    }

    // Index term tests

    #[test]
    fn test_flow_index_term_html() {
        let html = to_html("I love ((tigers)) very much");
        assert_eq!(html, "<p>I love tigers very much</p>\n");
    }

    #[test]
    fn test_concealed_index_term_html() {
        let html = to_html("(((animals, cats)))Visible text");
        assert_eq!(html, "<p>Visible text</p>\n");
    }

    #[test]
    fn test_indexterm2_macro_html() {
        let html = to_html("indexterm2:[tigers]");
        assert_eq!(html, "<p>tigers</p>\n");
    }

    #[test]
    fn test_indexterm_macro_html() {
        let html = to_html("indexterm:[animals, cats]");
        assert_eq!(html, "<p></p>\n");
    }

    #[test]
    fn test_flow_index_term_escaping_html() {
        let html = to_html("((a <b> & c))");
        assert_eq!(html, "<p>a &lt;b&gt; &amp; c</p>\n");
    }

    // Block metadata: custom id/class tests

    #[test]
    fn test_paragraph_with_id_and_role() {
        let html = to_html("[#notice.important]\nText");
        assert!(html.contains("id=\"notice\""));
        assert!(html.contains("class=\"important\""));
        assert!(html.contains("Text"));
    }

    #[test]
    fn test_paragraph_with_id_only() {
        let html = to_html("[#myid]\nHello");
        assert!(html.contains("id=\"myid\""));
        assert!(html.contains("Hello"));
    }

    #[test]
    fn test_paragraph_with_role_only() {
        let html = to_html("[.lead]\nText");
        assert!(html.contains("class=\"lead\""));
        assert!(html.contains("Text"));
    }

    #[test]
    fn test_paragraph_with_multiple_roles() {
        let html = to_html("[.r1.r2.r3]\nText");
        assert!(html.contains("class=\"r1 r2 r3\""));
    }

    #[test]
    fn test_sidebar_with_id_and_role() {
        let html = to_html("[#tips.custom]\n****\nContent\n****");
        assert!(html.contains("id=\"tips\""));
        assert!(html.contains("class=\"sidebarblock custom\""));
    }

    #[test]
    fn test_source_block_with_id() {
        let html = to_html("[source,rust,#code1]\n----\nfn main() {}\n----");
        assert!(html.contains("id=\"code1\""));
    }

    #[test]
    fn test_admonition_with_id_and_role() {
        let html = to_html("[#w1.special]\nWARNING: Danger!");
        assert!(html.contains("id=\"w1\""));
        assert!(html.contains("admonitionblock warning special"));
    }

    #[test]
    fn test_list_with_id() {
        let html = to_html("[#mylist]\n* item 1\n* item 2");
        assert!(html.contains("<ul id=\"mylist\">"));
    }

    #[test]
    fn test_table_with_id_and_role() {
        let html = to_html("[#data.striped]\n|===\n| A | B\n|===");
        assert!(html.contains("id=\"data\""));
        assert!(html.contains("class=\"striped\""));
    }

    #[test]
    fn test_discrete_heading_with_id_and_role() {
        let html = to_html("[discrete#myh.special]\n== Heading");
        assert!(html.contains("id=\"myh\""));
        assert!(html.contains("class=\"special\""));
    }

    // Inline span tests

    #[test]
    fn test_inline_span_single_role_html() {
        let html = to_html("[.lead]#text#");
        assert_eq!(html, "<p><span class=\"lead\">text</span></p>\n");
    }

    #[test]
    fn test_inline_span_multiple_roles_html() {
        let html = to_html("[.big.red]#text#");
        assert_eq!(html, "<p><span class=\"big red\">text</span></p>\n");
    }

    #[test]
    fn test_inline_span_id_and_role_html() {
        let html = to_html("[#myid.lead]#text#");
        assert_eq!(html, "<p><span id=\"myid\" class=\"lead\">text</span></p>\n");
    }

    #[test]
    fn test_inline_span_unconstrained_html() {
        let html = to_html("hel[.x]##lo##rld");
        assert_eq!(html, "<p>hel<span class=\"x\">lo</span>rld</p>\n");
    }

    #[test]
    fn test_bare_highlight_no_regression_html() {
        let html = to_html("#highlight#");
        assert_eq!(html, "<p><mark>highlight</mark></p>\n");
    }
}
