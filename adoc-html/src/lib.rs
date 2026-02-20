use std::collections::HashMap;
use std::fmt::Write;
use adoc_parser::{CellStyle, Event, HAlign, Tag, TagEnd, AdmonitionKind, DelimitedBlockKind, SubstitutionSet, VAlign};

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

#[derive(Clone, Copy, PartialEq)]
enum DlistStyle {
    Normal,
    Horizontal,
    Qanda,
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
    options: Vec<String>,
    named: Vec<(String, String)>,
    subs: Option<SubstitutionSet>,
}

struct HtmlRenderer {
    tag_stack: Vec<TagEnd>,
    in_source_block: bool,
    subs_stack: Vec<SubstitutionSet>,
    pending_subs: Option<SubstitutionSet>,
    document_attrs: HashMap<String, String>,
    delimited_block_stack: Vec<(DelimitedBlockKind, bool)>,
    footnotes: Vec<(usize, Option<String>, String)>, // (number, id, text)
    footnote_counter: usize,
    named_footnotes: HashMap<String, usize>, // id → number
    toc_entries: Vec<TocEntry>,
    toc_insert_position: Option<usize>,
    toc_levels: u8,
    toc_position: String,
    toc_title: String,
    toc_auto_seen: bool,
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
    table_counter: usize,
    block_title_output_start: Option<usize>,
    block_title_inner_html: Option<String>,
    dlist_stack: Vec<DlistStyle>,
    hdlist_in_term_group: bool,
    has_document_title: bool,
    capturing_doctitle: bool,
    doctitle_buf: String,
    preamble_start: Option<usize>,
    appendix_counter: u8,
    pending_section_caption: Option<String>,
    sectnums: bool,
    section_counters: [u32; 6],
}

impl HtmlRenderer {
    fn new() -> Self {
        Self {
            tag_stack: Vec::new(),
            in_source_block: false,
            subs_stack: Vec::new(),
            pending_subs: None,
            document_attrs: HashMap::from([
                ("backend".to_string(), "html5".to_string()),
                ("doctype".to_string(), "article".to_string()),
            ]),
            delimited_block_stack: Vec::new(),
            footnotes: Vec::new(),
            footnote_counter: 0,
            named_footnotes: HashMap::new(),
            toc_entries: Vec::new(),
            toc_insert_position: None,
            toc_levels: 2,
            toc_position: String::new(),
            toc_title: String::from("Table of Contents"),
            toc_auto_seen: false,
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
            table_counter: 0,
            block_title_output_start: None,
            block_title_inner_html: None,
            dlist_stack: Vec::new(),
            hdlist_in_term_group: false,
            has_document_title: false,
            capturing_doctitle: false,
            doctitle_buf: String::new(),
            preamble_start: None,
            appendix_counter: 0,
            pending_section_caption: None,
            sectnums: false,
            section_counters: [0; 6],
        }
    }

    fn current_subs(&self) -> SubstitutionSet {
        self.subs_stack.last().copied().unwrap_or(SubstitutionSet::NORMAL)
    }

    fn default_subs_for_delimited(kind: DelimitedBlockKind) -> SubstitutionSet {
        match kind {
            DelimitedBlockKind::Listing | DelimitedBlockKind::Literal => SubstitutionSet::VERBATIM,
            DelimitedBlockKind::Passthrough | DelimitedBlockKind::Comment => SubstitutionSet::NONE,
            _ => SubstitutionSet::NORMAL,
        }
    }

    fn run<'a>(&mut self, output: &mut String, iter: impl Iterator<Item = Event<'a>>) {
        for event in iter {
            self.push_event(output, event);
        }
        self.finish(output);
    }

    fn push_event<'a>(&mut self, output: &mut String, event: Event<'a>) {
        // Wrap preamble content when a section starts
        if matches!(event, Event::Start(Tag::Section { .. }))
            && let Some(start) = self.preamble_start.take()
        {
            let preamble_content = output.split_off(start);
            if !preamble_content.is_empty() {
                output.push_str("<div id=\"preamble\">\n<div class=\"sectionbody\">\n");
                output.push_str(&preamble_content);
                output.push_str("</div>\n</div>\n");
            }
            if self.toc_position == "preamble" && self.toc_insert_position.is_none() {
                self.toc_insert_position = Some(output.len());
            }
        }

        match event {
            Event::Start(tag) => self.start_tag(output, &tag),
            Event::End(tag_end) => self.end_tag(output, &tag_end),
            Event::Text(text) => {
                if self.capturing_doctitle {
                    self.doctitle_buf.push_str(&text);
                }
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
                } else if self.current_subs().has(SubstitutionSet::SPECIALCHARS) {
                    html_escape(output, &text);
                } else {
                    output.push_str(&text);
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
                if name == "toc-title" {
                    self.toc_title = value.to_string();
                }
                if name == "toc" {
                    self.toc_position = value.to_string();
                }
                if name == "sectnums" {
                    self.sectnums = true;
                }
                if name == "!sectnums" || name == "sectnums!" {
                    self.sectnums = false;
                }
                // Store for attribute reference resolution
                if let Some(stripped) = name.strip_prefix('!') {
                    self.document_attrs.remove(stripped);
                } else if let Some(stripped) = name.strip_suffix('!') {
                    self.document_attrs.remove(stripped);
                } else {
                    self.document_attrs.insert(name.to_string(), value.to_string());
                }
            }
            Event::AttributeReference { name, fallback } => {
                if let Some(value) = self.document_attrs.get(name.as_ref()) {
                    html_escape(output, value);
                } else if let Some(fb) = fallback {
                    html_escape(output, &fb);
                } else {
                    output.push('{');
                    output.push_str(&name);
                    output.push('}');
                }
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
                if !self.toc_auto_seen {
                    self.toc_auto_seen = true;
                    // Auto-TOC: set position unless deferred to preamble or macro
                    match self.toc_position.as_str() {
                        "preamble" | "macro" => {}
                        _ => { self.toc_insert_position = Some(output.len()); }
                    }
                } else {
                    // Subsequent Toc = from toc::[] macro — always set position
                    self.toc_insert_position = Some(output.len());
                }
            }
            Event::Include { path, .. } => {
                output.push_str("<!-- include::");
                html_escape(output, &path);
                output.push_str("[] -->\n");
            }
            Event::Author { fullname, firstname, middlename, lastname, initials, address } => {
                self.document_attrs.insert("author".to_string(), fullname.to_string());
                self.document_attrs.insert("firstname".to_string(), firstname.to_string());
                if !middlename.is_empty() {
                    self.document_attrs.insert("middlename".to_string(), middlename.to_string());
                }
                self.document_attrs.insert("lastname".to_string(), lastname.to_string());
                self.document_attrs.insert("authorinitials".to_string(), initials.to_string());
                if !address.is_empty() {
                    self.document_attrs.insert("email".to_string(), address.to_string());
                }
            }
            Event::Revision { version, date, remark } => {
                if !version.is_empty() {
                    self.document_attrs.insert("revnumber".to_string(), version.to_string());
                }
                if !date.is_empty() {
                    self.document_attrs.insert("revdate".to_string(), date.to_string());
                }
                if !remark.is_empty() {
                    self.document_attrs.insert("revremark".to_string(), remark.to_string());
                }
            }
            Event::BlockMetadata { style, id, roles, options, named, subs } => {
                if let Some(s) = subs {
                    self.pending_subs = Some(s);
                }
                self.pending_block_meta = Some(BlockMeta {
                    style: style.map(|s| s.into_owned()),
                    id: id.map(|s| s.into_owned()),
                    roles: roles.into_iter().map(|s| s.into_owned()).collect(),
                    options: options.into_iter().map(|s| s.into_owned()).collect(),
                    named: named.into_iter().map(|(k, v)| (k.into_owned(), v.into_owned())).collect(),
                    subs,
                });
            }
        }
    }

    fn finish(&mut self, output: &mut String) {
        // If preamble_start is still set, no section followed — leave content as-is
        self.preamble_start = None;

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
        match self.toc_position.as_str() {
            "left" => toc.push_str("<div id=\"toc\" class=\"toc2 toc-left\">\n"),
            "right" => toc.push_str("<div id=\"toc\" class=\"toc2 toc-right\">\n"),
            _ => toc.push_str("<div id=\"toc\" class=\"toc\">\n"),
        }
        toc.push_str("<div id=\"toctitle\">");
        html_escape(&mut toc, &self.toc_title);
        toc.push_str("</div>\n");

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

        // Push subs stack for blocks that affect substitution context
        let meta_subs = meta.as_ref().and_then(|m| m.subs).or(self.pending_subs.take());
        match tag {
            Tag::SourceBlock { .. } => {
                self.subs_stack.push(meta_subs.unwrap_or(SubstitutionSet::VERBATIM));
            }
            Tag::DelimitedBlock { kind } => {
                let default = Self::default_subs_for_delimited(*kind);
                self.subs_stack.push(meta_subs.unwrap_or(default));
            }
            Tag::Paragraph => {
                // Inherit subs from parent block if no explicit override
                self.subs_stack.push(meta_subs.unwrap_or_else(|| self.current_subs()));
            }
            Tag::LiteralParagraph => {
                self.subs_stack.push(meta_subs.unwrap_or_else(|| self.current_subs()));
            }
            _ => {}
        }

        match tag {
            Tag::Header => {
                // Document header rendered as header div
                output.push_str("<div class=\"header\">\n");
            }
            Tag::DocumentTitle => {
                self.has_document_title = true;
                self.capturing_doctitle = true;
                self.doctitle_buf.clear();
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
                if self.sectnums && *level >= 2 && *level <= 5 && self.pending_section_caption.is_none() {
                    let lvl = *level as usize;
                    self.section_counters[lvl] += 1;
                    for l in (lvl + 1)..6 {
                        self.section_counters[l] = 0;
                    }
                    let mut prefix = String::new();
                    for l in 2..=lvl {
                        if !prefix.is_empty() {
                            prefix.push('.');
                        }
                        prefix.push_str(&self.section_counters[l].to_string());
                    }
                    prefix.push_str(". ");
                    output.push_str(&prefix);
                    if let Some(ref mut entry) = self.current_toc_entry {
                        entry.title.push_str(&prefix);
                    }
                }
                if let Some(caption) = self.pending_section_caption.take() {
                    output.push_str(&caption);
                    if let Some(ref mut entry) = self.current_toc_entry {
                        entry.title.push_str(&caption);
                    }
                }
            }
            Tag::Heading { level } => {
                let h = section_level_to_h(*level);
                output.push_str("<h");
                output.push_str(&h.to_string());
                Self::write_meta_attrs(output, &meta, "");
                output.push('>');
            }
            Tag::Section { .. } => {
                let style = meta.as_ref().and_then(|m| m.style.as_deref());
                let is_special = matches!(style, Some(
                    "appendix" | "glossary" | "bibliography" | "colophon"
                    | "abstract" | "preface" | "dedication" | "index"
                ));
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "sect");
                output.push_str(">\n");
                if style == Some("appendix") {
                    self.appendix_counter += 1;
                    let letter = (b'A' + self.appendix_counter - 1) as char;
                    self.pending_section_caption = Some(format!("Appendix {letter}: "));
                } else if is_special {
                    self.pending_section_caption = Some(String::new());
                }
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
                match kind {
                    DelimitedBlockKind::Listing => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_output_start = None;
                        self.block_title_inner_html = None;
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "listingblock");
                        output.push_str(">\n<div class=\"content\">\n<pre>");
                    }
                    DelimitedBlockKind::Literal => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_output_start = None;
                        self.block_title_inner_html = None;
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "literalblock");
                        output.push_str(">\n<div class=\"content\">\n<pre>");
                    }
                    DelimitedBlockKind::Example => {
                        let is_collapsible = meta.as_ref()
                            .is_some_and(|m| m.options.iter().any(|o| o == "collapsible"));

                        if is_collapsible {
                            let is_open = meta.as_ref()
                                .is_some_and(|m| m.options.iter().any(|o| o == "open"));

                            let summary = if let (Some(start), Some(inner)) =
                                (self.block_title_output_start.take(), self.block_title_inner_html.take())
                            {
                                output.truncate(start);
                                inner
                            } else {
                                "Details".to_string()
                            };

                            output.push_str("<details");
                            Self::write_meta_attrs(output, &meta, "exampleblock");
                            if is_open {
                                output.push_str(" open");
                            }
                            output.push_str(">\n<summary class=\"title\">");
                            output.push_str(&summary);
                            output.push_str("</summary>\n<div class=\"content\">\n");
                            self.delimited_block_stack.push((*kind, true));
                        } else {
                            self.delimited_block_stack.push((*kind, false));
                            self.block_title_output_start = None;
                            self.block_title_inner_html = None;
                            output.push_str("<div");
                            Self::write_meta_attrs(output, &meta, "exampleblock");
                            output.push_str(">\n<div class=\"content\">\n");
                        }
                    }
                    DelimitedBlockKind::Sidebar => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_output_start = None;
                        self.block_title_inner_html = None;
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "sidebarblock");
                        output.push_str(">\n<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Quote => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_output_start = None;
                        self.block_title_inner_html = None;
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "quoteblock");
                        output.push_str(">\n<blockquote>\n");
                    }
                    DelimitedBlockKind::Open => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_output_start = None;
                        self.block_title_inner_html = None;
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "openblock");
                        output.push_str(">\n<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Comment => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_output_start = None;
                        self.block_title_inner_html = None;
                        // Comment blocks are not rendered
                    }
                    DelimitedBlockKind::Passthrough => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_output_start = None;
                        self.block_title_inner_html = None;
                        // Passthrough: content is rendered as-is
                    }
                    DelimitedBlockKind::Verse => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_output_start = None;
                        self.block_title_inner_html = None;
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
                output.push_str(">\n<div class=\"content\">\n<pre");

                let highlighter = self.document_attrs.get("source-highlighter").cloned();
                let linenums = meta.as_ref().is_some_and(|m| {
                    m.options.iter().any(|o| o == "linenums")
                });

                // Build <pre> class
                let mut pre_classes = Vec::new();
                match highlighter.as_deref() {
                    Some("highlight.js" | "highlightjs") => pre_classes.push("highlightjs"),
                    Some("rouge") => pre_classes.push("rouge"),
                    Some("pygments") => pre_classes.push("pygments"),
                    Some("coderay") => pre_classes.push("CodeRay"),
                    _ => {}
                }
                if highlighter.is_some() {
                    pre_classes.push("highlight");
                }
                if linenums {
                    pre_classes.push("linenums");
                }
                if !pre_classes.is_empty() {
                    output.push_str(" class=\"");
                    output.push_str(&pre_classes.join(" "));
                    output.push('"');
                }

                output.push_str("><code");

                // Build <code> attrs
                if let Some(lang) = language {
                    if matches!(highlighter.as_deref(), Some("highlight.js" | "highlightjs") | None) {
                        output.push_str(" class=\"language-");
                        html_escape(output, lang);
                        output.push('"');
                    }
                    if highlighter.is_some() {
                        output.push_str(" data-lang=\"");
                        html_escape(output, lang);
                        output.push('"');
                    }
                }

                output.push('>');
            }
            Tag::BlockTitle => {
                self.block_title_output_start = Some(output.len());
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
                let style_str = meta.as_ref().and_then(|m| m.style.as_deref());
                let dlist_style = match style_str {
                    Some("horizontal") => DlistStyle::Horizontal,
                    Some("qanda") => DlistStyle::Qanda,
                    _ => DlistStyle::Normal,
                };
                self.dlist_stack.push(dlist_style);
                let mut adjusted_meta = meta;
                if let Some(ref mut m) = adjusted_meta {
                    m.style = None;
                }
                match dlist_style {
                    DlistStyle::Horizontal => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &adjusted_meta, "hdlist");
                        output.push_str(">\n<table>\n");
                    }
                    DlistStyle::Qanda => {
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &adjusted_meta, "qlist qanda");
                        output.push_str(">\n<ol>\n");
                    }
                    DlistStyle::Normal => {
                        output.push_str("<dl");
                        Self::write_meta_attrs(output, &adjusted_meta, "");
                        output.push_str(">\n");
                    }
                }
            }
            Tag::DescriptionTerm => {
                match self.current_dlist_style() {
                    DlistStyle::Horizontal => {
                        if self.hdlist_in_term_group {
                            output.push_str("<br>");
                        } else {
                            output.push_str("<tr>\n<td class=\"hdlist1\">");
                            self.hdlist_in_term_group = true;
                        }
                    }
                    DlistStyle::Qanda => {
                        output.push_str("<li>\n<p><em>");
                    }
                    DlistStyle::Normal => {
                        output.push_str("<dt>");
                    }
                }
            }
            Tag::DescriptionDescription => {
                match self.current_dlist_style() {
                    DlistStyle::Horizontal => {
                        output.push_str("</td>\n<td class=\"hdlist2\">");
                        self.hdlist_in_term_group = false;
                    }
                    DlistStyle::Qanda => {}
                    DlistStyle::Normal => {
                        output.push_str("<dd>");
                    }
                }
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
                output.push_str(">\n<table>\n<tr>\n<td class=\"icon\">\n");
                if self.document_attrs.get("icons").is_some_and(|v| v == "font") {
                    let icon_name = label.to_lowercase();
                    writeln!(output, "<i class=\"fa icon-{icon_name}\" title=\"{label}\"></i>")
                        .unwrap();
                } else {
                    output.push_str("<div class=\"title\">");
                    output.push_str(label);
                    output.push_str("</div>\n");
                }
                output.push_str("</td>\n<td class=\"content\">\n");
            }
            Tag::Table => {
                // Collect extra CSS classes from options/named attrs
                let has_autowidth = meta.as_ref().is_some_and(|m| m.options.iter().any(|o| o == "autowidth"));
                let stripes_value = meta.as_ref().and_then(|m| m.named.iter().find(|(k, _)| k == "stripes").map(|(_, v)| v.clone()));

                let mut classes = String::new();
                if has_autowidth {
                    classes.push_str("fit-content");
                }
                if let Some(ref sv) = stripes_value {
                    if !classes.is_empty() {
                        classes.push(' ');
                    }
                    classes.push_str("stripes-");
                    classes.push_str(sv);
                }

                output.push_str("<table");
                Self::write_meta_attrs(output, &meta, &classes);
                output.push_str(">\n");

                // Caption handling
                let title_html = self.block_title_inner_html.take();
                if let Some(start) = self.block_title_output_start.take() {
                    output.truncate(start);
                }
                if let Some(title) = title_html {
                    self.table_counter += 1;
                    let caption_attr = meta.as_ref().and_then(|m| m.named.iter().find(|(k, _)| k == "caption").map(|(_, v)| v.clone()));
                    output.push_str("<caption class=\"title\">");
                    match caption_attr.as_deref() {
                        Some("") => {
                            // Empty caption= means no prefix, just the title
                        }
                        Some(prefix) => {
                            html_escape(output, prefix);
                        }
                        None => {
                            // Default: "Table N. "
                            output.push_str("Table ");
                            output.push_str(&self.table_counter.to_string());
                            output.push_str(". ");
                        }
                    }
                    output.push_str(&title);
                    output.push_str("</caption>\n");
                }
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
            Tag::BlockImage { target, alt, width, height } => {
                // Build base class with align/float CSS classes from named attrs
                let base_class = Self::image_base_class("imageblock", &meta);
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, &base_class);
                output.push_str(">\n<div class=\"content\">\n<img src=\"");
                html_escape(output, target);
                output.push_str("\" alt=\"");
                html_escape(output, alt);
                output.push('"');
                if let Some(w) = width {
                    output.push_str(" width=\"");
                    html_escape(output, w);
                    output.push('"');
                }
                if let Some(h) = height {
                    output.push_str(" height=\"");
                    html_escape(output, h);
                    output.push('"');
                }
                output.push_str(">\n</div>\n");
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
            Tag::InlineImage { target, alt, width, height, align, float } => {
                let mut img_class = String::from("image");
                if let Some(f) = float {
                    img_class.push(' ');
                    img_class.push_str(f);
                }
                if let Some(a) = align {
                    let css = match a.as_ref() {
                        "left" => "text-left",
                        "center" => "text-center",
                        "right" => "text-right",
                        other => other,
                    };
                    img_class.push(' ');
                    img_class.push_str(css);
                }
                output.push_str("<span class=\"");
                output.push_str(&img_class);
                output.push_str("\"><img src=\"");
                html_escape(output, target);
                output.push_str("\" alt=\"");
                html_escape(output, alt);
                output.push('"');
                if let Some(w) = width {
                    output.push_str(" width=\"");
                    html_escape(output, w);
                    output.push('"');
                }
                if let Some(h) = height {
                    output.push_str(" height=\"");
                    html_escape(output, h);
                    output.push('"');
                }
                output.push_str("></span>");
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
            Tag::Link { url, window, nofollow } => {
                output.push_str("<a href=\"");
                html_escape(output, url);
                output.push('"');
                if let Some(w) = window {
                    output.push_str(" target=\"");
                    html_escape(output, w);
                    output.push('"');
                }
                let has_noopener = window.is_some();
                if has_noopener || *nofollow {
                    output.push_str(" rel=\"");
                    if has_noopener {
                        output.push_str("noopener");
                    }
                    if *nofollow {
                        if has_noopener {
                            output.push(' ');
                        }
                        output.push_str("nofollow");
                    }
                    output.push('"');
                }
                output.push('>');
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

        // Pop subs stack for blocks that pushed to it
        match tag_end {
            TagEnd::SourceBlock | TagEnd::DelimitedBlock | TagEnd::Paragraph | TagEnd::LiteralParagraph => {
                self.subs_stack.pop();
            }
            _ => {}
        }

        match tag_end {
            TagEnd::Header => {
                output.push_str("</div>\n");
                if self.has_document_title {
                    self.preamble_start = Some(output.len());
                }
            }
            TagEnd::DocumentTitle => {
                output.push_str("</h1>\n");
                self.capturing_doctitle = false;
                let title = std::mem::take(&mut self.doctitle_buf);
                self.document_attrs.insert("doctitle".to_string(), title);
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
                    Some((DelimitedBlockKind::Listing | DelimitedBlockKind::Literal, _)) => {
                        output.push_str("</pre>\n</div>\n</div>\n");
                    }
                    Some((DelimitedBlockKind::Quote, _)) => {
                        output.push_str("</blockquote>\n</div>\n");
                    }
                    Some((DelimitedBlockKind::Verse, _)) => {
                        output.push_str("</pre>\n</div>\n");
                    }
                    Some((DelimitedBlockKind::Example, true)) => {
                        output.push_str("</div>\n</details>\n");
                    }
                    Some((DelimitedBlockKind::Example | DelimitedBlockKind::Sidebar
                         | DelimitedBlockKind::Open, false)) => {
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
                if let Some(start) = self.block_title_output_start {
                    let title_tag = "<div class=\"title\">";
                    let inner_start = start + title_tag.len();
                    self.block_title_inner_html = Some(output[inner_start..].to_string());
                }
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
                let style = self.dlist_stack.pop().unwrap_or(DlistStyle::Normal);
                match style {
                    DlistStyle::Horizontal => output.push_str("</table>\n</div>\n"),
                    DlistStyle::Qanda => output.push_str("</ol>\n</div>\n"),
                    DlistStyle::Normal => output.push_str("</dl>\n"),
                }
            }
            TagEnd::DescriptionTerm => {
                match self.current_dlist_style() {
                    DlistStyle::Horizontal => {}
                    DlistStyle::Qanda => output.push_str("</em></p>\n"),
                    DlistStyle::Normal => output.push_str("</dt>\n"),
                }
            }
            TagEnd::DescriptionDescription => {
                match self.current_dlist_style() {
                    DlistStyle::Horizontal => output.push_str("</td>\n</tr>\n"),
                    DlistStyle::Qanda => output.push_str("</li>\n"),
                    DlistStyle::Normal => output.push_str("</dd>\n"),
                }
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

    /// Build a base CSS class for block images, appending align/float classes from named attrs.
    fn image_base_class(default: &str, meta: &Option<BlockMeta>) -> String {
        let mut class = String::from(default);
        if let Some(m) = meta {
            for (k, v) in &m.named {
                match k.as_str() {
                    "float" => {
                        class.push(' ');
                        class.push_str(v);
                    }
                    "align" => {
                        let css = match v.as_str() {
                            "left" => "text-left",
                            "center" => "text-center",
                            "right" => "text-right",
                            other => other,
                        };
                        class.push(' ');
                        class.push_str(css);
                    }
                    _ => {}
                }
            }
        }
        class
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

    fn current_dlist_style(&self) -> DlistStyle {
        self.dlist_stack.last().copied().unwrap_or(DlistStyle::Normal)
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
    fn test_link_with_window_html() {
        let html = to_html("link:https://example.com[Example,window=_blank]");
        assert!(html.contains("<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener\">Example</a>"));
    }

    #[test]
    fn test_link_with_nofollow_html() {
        let html = to_html("link:https://example.com[Example,opts=nofollow]");
        assert!(html.contains("<a href=\"https://example.com\" rel=\"nofollow\">Example</a>"));
    }

    #[test]
    fn test_link_with_window_and_nofollow_html() {
        let html = to_html("link:https://example.com[Example,window=_blank,opts=nofollow]");
        assert!(html.contains("<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener nofollow\">Example</a>"));
    }

    #[test]
    fn test_link_no_attrs_unchanged_html() {
        let html = to_html("link:https://example.com[Example]");
        assert!(html.contains("<a href=\"https://example.com\">Example</a>"));
        assert!(!html.contains("target="));
        assert!(!html.contains("rel="));
    }

    #[test]
    fn test_email_autolink_html() {
        let html = to_html("Contact user@example.com for info");
        assert!(html.contains("<a href=\"mailto:user@example.com\">user@example.com</a>"));
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
    fn test_toc_custom_title() {
        let input = "= Doc\n:toc:\n:toc-title: Содержание\n\n== S1\n\nText.";
        let html = to_html(input);
        assert!(html.contains("<div id=\"toctitle\">Содержание</div>"));
        assert!(!html.contains("Table of Contents"));
    }

    #[test]
    fn test_toc_left() {
        let input = "= Doc\n:toc: left\n\n== S1\n\nText.";
        let html = to_html(input);
        assert!(html.contains("class=\"toc2 toc-left\""));
    }

    #[test]
    fn test_toc_right() {
        let input = "= Doc\n:toc: right\n\n== S1\n\nText.";
        let html = to_html(input);
        assert!(html.contains("class=\"toc2 toc-right\""));
    }

    #[test]
    fn test_toc_preamble() {
        let input = "= Title\n:toc: preamble\n\nPreamble text.\n\n== Section One\n\nContent.";
        let html = to_html(input);
        assert!(html.contains("<div id=\"toc\""));
        // TOC should appear after preamble closing div, before section
        let preamble_end = html.find("</div>\n</div>\n").unwrap() + "</div>\n</div>\n".len();
        let toc_pos = html.find("<div id=\"toc\"").unwrap();
        let section_pos = html.find("<div class=\"sect\"").unwrap();
        assert!(toc_pos >= preamble_end, "TOC should be after preamble");
        assert!(toc_pos < section_pos, "TOC should be before first section");
    }

    #[test]
    fn test_toc_macro_only() {
        let input = "= Title\n:toc: macro\n\n== S1\n\ntoc::[]\n\n== S2";
        let html = to_html(input);
        assert!(html.contains("<div id=\"toc\""));
        // TOC should be placed at the macro position, after S1 heading
        let s1_pos = html.find("S1</h2>").unwrap();
        let toc_pos = html.find("<div id=\"toc\"").unwrap();
        assert!(toc_pos > s1_pos, "TOC should appear after S1 heading");
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
    fn test_table_autowidth_html() {
        let html = to_html("[%autowidth]\n|===\n| A | B\n|===");
        assert!(html.contains("<table class=\"fit-content\">"));
    }

    #[test]
    fn test_table_stripes_html() {
        let html = to_html("[stripes=even]\n|===\n| A | B\n|===");
        assert!(html.contains("<table class=\"stripes-even\">"));
    }

    #[test]
    fn test_table_stripes_odd_html() {
        let html = to_html("[stripes=odd]\n|===\n| A | B\n|===");
        assert!(html.contains("<table class=\"stripes-odd\">"));
    }

    #[test]
    fn test_table_autowidth_stripes_html() {
        let html = to_html("[%autowidth,stripes=even]\n|===\n| A | B\n|===");
        assert!(html.contains("fit-content"));
        assert!(html.contains("stripes-even"));
    }

    #[test]
    fn test_table_caption_default_html() {
        let html = to_html(".My Table\n|===\n| A | B\n|===");
        assert!(html.contains("<caption class=\"title\">Table 1. My Table</caption>"));
    }

    #[test]
    fn test_table_caption_auto_numbering_html() {
        let html = to_html(".First\n|===\n| A\n|===\n\n.Second\n|===\n| B\n|===");
        assert!(html.contains("<caption class=\"title\">Table 1. First</caption>"));
        assert!(html.contains("<caption class=\"title\">Table 2. Second</caption>"));
    }

    #[test]
    fn test_table_caption_custom_prefix_html() {
        let html = to_html("[caption=\"Data Set \"]\n.Results\n|===\n| A | B\n|===");
        assert!(html.contains("<caption class=\"title\">Data Set Results</caption>"));
    }

    #[test]
    fn test_table_caption_empty_prefix_html() {
        let html = to_html("[caption=]\n.Results\n|===\n| A | B\n|===");
        assert!(html.contains("<caption class=\"title\">Results</caption>"));
        assert!(!html.contains("Table 1"));
    }

    #[test]
    fn test_table_no_title_no_caption_html() {
        let html = to_html("|===\n| A | B\n|===");
        assert!(!html.contains("<caption"));
    }

    #[test]
    fn test_table_autowidth_with_id_and_role_html() {
        let html = to_html("[%autowidth#mytable.custom]\n|===\n| A | B\n|===");
        assert!(html.contains("id=\"mytable\""));
        assert!(html.contains("fit-content"));
        assert!(html.contains("custom"));
    }

    #[test]
    fn test_csv_table_html() {
        let html = to_html("[%header,format=csv]\n|===\nName,Age,City\nAlice,30,NYC\nBob,25,LA\n|===");
        assert!(html.contains("<table>"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th>Name</th>"));
        assert!(html.contains("<th>Age</th>"));
        assert!(html.contains("<th>City</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>Alice</td>"));
        assert!(html.contains("<td>30</td>"));
        assert!(html.contains("<td>NYC</td>"));
        assert!(html.contains("<td>Bob</td>"));
        assert!(html.contains("<td>25</td>"));
        assert!(html.contains("<td>LA</td>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("</table>"));
    }

    #[test]
    fn test_csv_table_shorthand_html() {
        let html = to_html("[%header,csv]\n|===\nName,Age\nAlice,30\n|===");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th>Name</th>"));
        assert!(html.contains("<th>Age</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>Alice</td>"));
        assert!(html.contains("<td>30</td>"));
        assert!(html.contains("</tbody>"));
    }

    #[test]
    fn test_dsv_table_html() {
        let html = to_html("[%header,format=dsv]\n|===\nName:Age:City\nAlice:30:NYC\n|===");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th>Name</th>"));
        assert!(html.contains("<th>Age</th>"));
        assert!(html.contains("<th>City</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>Alice</td>"));
        assert!(html.contains("<td>30</td>"));
        assert!(html.contains("<td>NYC</td>"));
        assert!(html.contains("</tbody>"));
    }

    #[test]
    fn test_tsv_table_html() {
        let html = to_html("[%header,format=tsv]\n|===\nName\tAge\tCity\nAlice\t30\tNYC\n|===");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th>Name</th>"));
        assert!(html.contains("<th>Age</th>"));
        assert!(html.contains("<th>City</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>Alice</td>"));
        assert!(html.contains("<td>30</td>"));
        assert!(html.contains("<td>NYC</td>"));
        assert!(html.contains("</tbody>"));
    }

    #[test]
    fn test_csv_table_no_header_html() {
        let html = to_html("[format=csv]\n|===\nAlice,30\nBob,25\n|===");
        assert!(!html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>Alice</td>"));
        assert!(html.contains("<td>30</td>"));
        assert!(html.contains("<td>Bob</td>"));
        assert!(html.contains("<td>25</td>"));
        assert!(html.contains("</tbody>"));
    }

    #[test]
    fn test_csv_table_quoted_fields_html() {
        let html = to_html("[%header,csv]\n|===\nName,Description\nAlice,\"Has a, comma\"\n|===");
        assert!(html.contains("<th>Name</th>"));
        assert!(html.contains("<th>Description</th>"));
        assert!(html.contains("<td>Alice</td>"));
        assert!(html.contains("<td>Has a, comma</td>"));
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

    #[test]
    fn test_block_admonition_html() {
        let html = to_html("[NOTE]\n====\nThis is a note.\n====");
        assert!(html.contains("<div class=\"admonitionblock note\">"));
        assert!(html.contains("<div class=\"title\">Note</div>"));
        assert!(html.contains("<td class=\"content\">"));
        assert!(html.contains("<p>This is a note.</p>"));
        assert!(html.contains("</td>\n</tr>\n</table>\n</div>"));
    }

    #[test]
    fn test_block_admonition_multi_para_html() {
        let html = to_html("[NOTE]\n====\nFirst paragraph.\n\nSecond paragraph.\n====");
        assert!(html.contains("<div class=\"admonitionblock note\">"));
        assert!(html.contains("<p>First paragraph.</p>"));
        assert!(html.contains("<p>Second paragraph.</p>"));
    }

    // Admonition icons tests

    #[test]
    fn test_admonition_icons_font() {
        let html = to_html(":icons: font\n\nNOTE: This is a note.");
        assert!(html.contains("<i class=\"fa icon-note\" title=\"Note\"></i>"));
        assert!(!html.contains("<div class=\"title\">Note</div>"));
    }

    #[test]
    fn test_admonition_icons_font_all_kinds() {
        for (marker, icon, label) in [
            ("NOTE", "note", "Note"),
            ("TIP", "tip", "Tip"),
            ("IMPORTANT", "important", "Important"),
            ("WARNING", "warning", "Warning"),
            ("CAUTION", "caution", "Caution"),
        ] {
            let input = format!(":icons: font\n\n{marker}: Some text.");
            let html = to_html(&input);
            let expected = format!("<i class=\"fa icon-{icon}\" title=\"{label}\"></i>");
            assert!(
                html.contains(&expected),
                "Expected {expected} in HTML for {marker}, got: {html}"
            );
        }
    }

    #[test]
    fn test_admonition_default_no_icons() {
        let html = to_html("NOTE: This is a note.");
        assert!(html.contains("<div class=\"title\">Note</div>"));
        assert!(!html.contains("<i class=\"fa"));
    }

    #[test]
    fn test_block_admonition_icons_font() {
        let html = to_html(":icons: font\n\n[NOTE]\n====\nBlock note content.\n====");
        assert!(html.contains("<i class=\"fa icon-note\" title=\"Note\"></i>"));
        assert!(!html.contains("<div class=\"title\">Note</div>"));
    }

    // Preamble tests

    #[test]
    fn test_preamble_html() {
        let html = to_html("= Title\n\nPreamble text.\n\n== Section");
        assert!(html.contains("<div id=\"preamble\">"));
        assert!(html.contains("<div class=\"sectionbody\">"));
        assert!(html.contains("<p>Preamble text.</p>"));
        assert!(html.contains("</div>\n</div>\n<div class=\"sect"));
    }

    #[test]
    fn test_preamble_multiple_blocks_html() {
        let html = to_html("= Title\n\nFirst para.\n\nSecond para.\n\n== Section");
        assert!(html.contains("<div id=\"preamble\">"));
        assert!(html.contains("<p>First para.</p>"));
        assert!(html.contains("<p>Second para.</p>"));
    }

    #[test]
    fn test_no_preamble_without_title_html() {
        let html = to_html("Content.\n\n== Section");
        assert!(!html.contains("preamble"));
    }

    #[test]
    fn test_no_preamble_without_section_html() {
        let html = to_html("= Title\n\nContent only.");
        assert!(!html.contains("preamble"));
        assert!(html.contains("<p>Content only.</p>"));
    }

    // Appendix section tests

    #[test]
    fn test_appendix_section_html() {
        let html = to_html("[appendix]\n== My Appendix\n\nContent.");
        assert!(html.contains("class=\"sect appendix\""));
        assert!(html.contains("Appendix A: My Appendix</h2>"));
    }

    #[test]
    fn test_appendix_multiple_html() {
        let html = to_html("[appendix]\n== First\n\nContent.\n\n[appendix]\n== Second\n\nMore.");
        assert!(html.contains("Appendix A: First</h2>"));
        assert!(html.contains("Appendix B: Second</h2>"));
    }

    #[test]
    fn test_appendix_no_caption_without_style_html() {
        let html = to_html("== Regular Section\n\nContent.");
        assert!(!html.contains("Appendix"));
    }

    #[test]
    fn test_glossary_section_html() {
        let html = to_html("[glossary]\n== Terms\n\nSome terms here.");
        assert!(html.contains("class=\"sect glossary\""));
    }

    #[test]
    fn test_bibliography_section_html() {
        let html = to_html("[bibliography]\n== References\n\nSome refs.");
        assert!(html.contains("class=\"sect bibliography\""));
    }

    #[test]
    fn test_colophon_section_html() {
        let html = to_html("[colophon]\n== Colophon\n\nPublishing info.");
        assert!(html.contains("class=\"sect colophon\""));
    }

    #[test]
    fn test_abstract_section_html() {
        let html = to_html("[abstract]\n== Summary\n\nBrief summary.");
        assert!(html.contains("class=\"sect abstract\""));
    }

    #[test]
    fn test_special_section_no_sectnums() {
        let html = to_html(":sectnums:\n\n== Numbered\n\n[glossary]\n== Terms\n\n[bibliography]\n== Refs\n\n== Also Numbered");
        // Regular sections should be numbered
        assert!(html.contains("1. Numbered"));
        assert!(html.contains("2. Also Numbered"));
        // Special sections should NOT be numbered
        assert!(html.contains(">Terms</h2>"));
        assert!(html.contains(">Refs</h2>"));
    }

    // Section numbering tests

    #[test]
    fn test_sectnums_basic() {
        let html = to_html("= Doc\n:sectnums:\n\n== First\n\n== Second");
        assert!(html.contains("1. First</h2>"));
        assert!(html.contains("2. Second</h2>"));
    }

    #[test]
    fn test_sectnums_nested() {
        let html = to_html("= Doc\n:sectnums:\n\n== Chapter\n\n=== Sub One\n\n=== Sub Two\n\n== Next");
        assert!(html.contains("1. Chapter</h2>"));
        assert!(html.contains("1.1. Sub One</h3>"));
        assert!(html.contains("1.2. Sub Two</h3>"));
        assert!(html.contains("2. Next</h2>"));
    }

    #[test]
    fn test_sectnums_disabled() {
        let html = to_html("= Doc\n\n== First\n\n== Second");
        assert!(html.contains(">First</h2>"));
        assert!(html.contains(">Second</h2>"));
        assert!(!html.contains("1. "));
    }

    #[test]
    fn test_sectnums_unset() {
        let html = to_html("= Doc\n:sectnums:\n\n== Numbered\n\n:!sectnums:\n\n== Not Numbered");
        assert!(html.contains("1. Numbered</h2>"));
        assert!(html.contains(">Not Numbered</h2>"));
    }

    #[test]
    fn test_sectnums_appendix_not_numbered() {
        let html = to_html("= Doc\n:sectnums:\n\n== Regular\n\n[appendix]\n== My Appendix");
        assert!(html.contains("1. Regular</h2>"));
        assert!(html.contains("Appendix A: My Appendix</h2>"));
        assert!(!html.contains("2. My Appendix"));
    }

    // Horizontal description list tests

    #[test]
    fn test_horizontal_description_list_html() {
        let html = to_html("[horizontal]\nCPU:: The brain\nRAM:: Memory");
        assert_eq!(
            html,
            "<div class=\"hdlist\">\n<table>\n\
             <tr>\n<td class=\"hdlist1\">CPU</td>\n<td class=\"hdlist2\">The brain</td>\n</tr>\n\
             <tr>\n<td class=\"hdlist1\">RAM</td>\n<td class=\"hdlist2\">Memory</td>\n</tr>\n\
             </table>\n</div>\n"
        );
    }

    #[test]
    fn test_horizontal_description_list_multiple_terms_html() {
        // Parser treats each term:: line as separate entry
        // This test verifies multiple entries render correctly
        let html = to_html("[horizontal]\nTerm1:: Desc1\nTerm2:: Desc2");
        assert!(html.contains("<td class=\"hdlist1\">Term1</td>"));
        assert!(html.contains("<td class=\"hdlist2\">Desc1</td>"));
        assert!(html.contains("<td class=\"hdlist1\">Term2</td>"));
        assert!(html.contains("<td class=\"hdlist2\">Desc2</td>"));
        assert_eq!(html.matches("<tr>").count(), 2);
    }

    #[test]
    fn test_horizontal_description_list_empty_desc_html() {
        let html = to_html("[horizontal]\nTerm:: ");
        assert!(html.contains("<div class=\"hdlist\">"));
        assert!(html.contains("<td class=\"hdlist1\">Term</td>"));
        assert!(html.contains("<td class=\"hdlist2\">"));
    }

    #[test]
    fn test_horizontal_description_list_with_id_html() {
        let html = to_html("[horizontal#mylist]\nA:: B");
        assert!(html.contains("id=\"mylist\""));
        assert!(html.contains("class=\"hdlist\""));
        assert!(html.contains("<table>"));
    }

    #[test]
    fn test_normal_description_list_unchanged_html() {
        let html = to_html("CPU:: The brain\nRAM:: Memory");
        assert_eq!(
            html,
            "<dl>\n<dt>CPU</dt>\n<dd>The brain</dd>\n<dt>RAM</dt>\n<dd>Memory</dd>\n</dl>\n"
        );
    }

    #[test]
    fn test_qanda_description_list_html() {
        let html = to_html("[qanda]\nWhat is Rust?:: A systems programming language.\nWhy use it?:: Memory safety.");
        assert_eq!(
            html,
            "<div class=\"qlist qanda\">\n<ol>\n\
             <li>\n<p><em>What is Rust?</em></p>\nA systems programming language.</li>\n\
             <li>\n<p><em>Why use it?</em></p>\nMemory safety.</li>\n\
             </ol>\n</div>\n"
        );
    }

    #[test]
    fn test_qanda_description_list_empty_answer_html() {
        let html = to_html("[qanda]\nQuestion?:: ");
        assert!(html.contains("<div class=\"qlist qanda\">"));
        assert!(html.contains("<li>\n<p><em>Question?</em></p>"));
        assert!(html.contains("</li>"));
    }

    #[test]
    fn test_qanda_description_list_with_id_html() {
        let html = to_html("[qanda#faq]\nQ:: A");
        assert!(html.contains("id=\"faq\""));
        assert!(html.contains("class=\"qlist qanda\""));
        assert!(html.contains("<ol>"));
    }

    #[test]
    fn test_block_image_dimensions_html() {
        let html = to_html("image::sunset.jpg[A beautiful sunset,600,400]");
        assert!(html.contains("src=\"sunset.jpg\""));
        assert!(html.contains("alt=\"A beautiful sunset\""));
        assert!(html.contains("width=\"600\""));
        assert!(html.contains("height=\"400\""));
    }

    #[test]
    fn test_block_image_named_dimensions_html() {
        let html = to_html("image::photo.jpg[alt=Photo,width=800,height=600]");
        assert!(html.contains("src=\"photo.jpg\""));
        assert!(html.contains("alt=\"Photo\""));
        assert!(html.contains("width=\"800\""));
        assert!(html.contains("height=\"600\""));
    }

    #[test]
    fn test_block_image_width_only_html() {
        let html = to_html("image::photo.jpg[Photo,300]");
        assert!(html.contains("src=\"photo.jpg\""));
        assert!(html.contains("alt=\"Photo\""));
        assert!(html.contains("width=\"300\""));
        assert!(!html.contains("height="));
    }

    #[test]
    fn test_block_image_no_dimensions_html() {
        let html = to_html("image::sunset.jpg[A beautiful sunset]");
        assert!(html.contains("src=\"sunset.jpg\""));
        assert!(html.contains("alt=\"A beautiful sunset\""));
        assert!(!html.contains("width="));
        assert!(!html.contains("height="));
    }

    #[test]
    fn test_inline_image_dimensions_html() {
        let html = to_html("see image:icon.png[Icon,32,32]");
        assert!(html.contains("src=\"icon.png\""));
        assert!(html.contains("alt=\"Icon\""));
        assert!(html.contains("width=\"32\""));
        assert!(html.contains("height=\"32\""));
    }

    #[test]
    fn test_block_image_align_center() {
        let html = to_html("image::photo.jpg[Alt,align=center]");
        assert!(html.contains("class=\"imageblock text-center\""));
    }

    #[test]
    fn test_block_image_float_left() {
        let html = to_html("image::photo.jpg[Alt,float=left]");
        assert!(html.contains("class=\"imageblock left\""));
    }

    #[test]
    fn test_block_image_align_from_block_attrs() {
        let html = to_html("[align=center]\nimage::photo.jpg[Alt]");
        assert!(html.contains("class=\"imageblock text-center\""));
    }

    #[test]
    fn test_block_image_float_right() {
        let html = to_html("image::photo.jpg[Alt,float=right]");
        assert!(html.contains("class=\"imageblock right\""));
    }

    #[test]
    fn test_block_image_align_right() {
        let html = to_html("image::photo.jpg[Alt,align=right]");
        assert!(html.contains("class=\"imageblock text-right\""));
    }

    #[test]
    fn test_inline_image_float_left() {
        let html = to_html("text image:icon.png[Icon,float=left] more");
        assert!(html.contains("class=\"image left\""));
    }

    #[test]
    fn test_inline_image_align_center() {
        let html = to_html("text image:icon.png[Icon,align=center] more");
        assert!(html.contains("class=\"image text-center\""));
    }

    #[test]
    fn test_collapsible_block_html() {
        let html = to_html("[%collapsible]\n====\nContent\n====");
        assert!(html.contains("<details"));
        assert!(html.contains("<summary class=\"title\">Details</summary>"));
        assert!(html.contains("<div class=\"content\">"));
        assert!(html.contains("<p>Content</p>"));
        assert!(html.contains("</div>\n</details>"));
        assert!(!html.contains("<div class=\"exampleblock\">"));
    }

    #[test]
    fn test_collapsible_block_with_title_html() {
        let html = to_html(".Click to expand\n[%collapsible]\n====\nContent\n====");
        assert!(html.contains("<details"));
        assert!(html.contains("<summary class=\"title\">Click to expand</summary>"));
        assert!(!html.contains("<div class=\"title\">Click to expand</div>"));
        assert!(html.contains("<p>Content</p>"));
        assert!(html.contains("</div>\n</details>"));
    }

    #[test]
    fn test_collapsible_block_open_html() {
        let html = to_html("[%collapsible%open]\n====\nContent\n====");
        assert!(html.contains("<details"));
        assert!(html.contains(" open>"));
        assert!(html.contains("<summary class=\"title\">Details</summary>"));
        assert!(html.contains("<p>Content</p>"));
    }

    #[test]
    fn test_collapsible_block_with_id_html() {
        let html = to_html("[%collapsible#myid]\n====\nContent\n====");
        assert!(html.contains("<details id=\"myid\""));
        assert!(html.contains("<summary class=\"title\">Details</summary>"));
    }

    #[test]
    fn test_example_block_unchanged_html() {
        let html = to_html("====\nContent\n====");
        assert!(html.contains("<div class=\"exampleblock\">"));
        assert!(html.contains("<div class=\"content\">"));
        assert!(html.contains("<p>Content</p>"));
        assert!(html.contains("</div>\n</div>"));
        assert!(!html.contains("<details"));
        assert!(!html.contains("<summary"));
    }

    // === Block substitution tests ===

    #[test]
    fn test_listing_block_subs_normal() {
        let html = to_html("[subs=normal]\n----\n*bold*\n----");
        assert!(html.contains("<strong>bold</strong>"), "subs=normal on listing block should enable inline parsing. Got: {html}");
    }

    #[test]
    fn test_paragraph_subs_none() {
        let html = to_html("[subs=none]\n*bold* & <tag>");
        assert!(!html.contains("<strong>"), "subs=none should disable inline parsing. Got: {html}");
        assert!(!html.contains("&amp;"), "subs=none should disable specialchars. Got: {html}");
        assert!(html.contains("*bold*"), "subs=none should preserve literal asterisks. Got: {html}");
        assert!(html.contains("<tag>"), "subs=none should pass through raw tags. Got: {html}");
    }

    #[test]
    fn test_listing_block_subs_plus_quotes() {
        let html = to_html("[subs=\"+quotes\"]\n----\n*bold*\n----");
        assert!(html.contains("<strong>bold</strong>"), "subs=+quotes on listing block should enable quote formatting. Got: {html}");
    }

    #[test]
    fn test_paragraph_subs_minus_replacements() {
        let html = to_html("[subs=\"-replacements\"]\nHello (C)");
        assert!(html.contains("(C)"), "subs=-replacements should not replace (C) with ©. Got: {html}");
        assert!(!html.contains("\u{00A9}"), "subs=-replacements should not produce ©. Got: {html}");
    }

    #[test]
    fn test_example_block_no_subs_unchanged() {
        let html = to_html("====\n*bold* text\n====");
        assert!(html.contains("<strong>bold</strong>"), "Example block without subs should process inline normally. Got: {html}");
    }

    #[test]
    fn test_listing_block_default_no_inline() {
        let html = to_html("----\n*bold*\n----");
        assert!(!html.contains("<strong>"), "Listing block default should NOT process inline formatting. Got: {html}");
        assert!(html.contains("*bold*"), "Listing block default should preserve raw markup. Got: {html}");
    }

    #[test]
    fn test_literal_paragraph_subs_normal() {
        let html = to_html("[subs=normal]\n  literal *bold*");
        assert!(html.contains("<strong>bold</strong>"), "subs=normal on literal paragraph should enable inline parsing. Got: {html}");
    }

    #[test]
    fn test_paragraph_subs_verbatim() {
        let html = to_html("[subs=verbatim]\n*bold* & <tag>");
        assert!(!html.contains("<strong>"), "subs=verbatim should disable inline parsing. Got: {html}");
        assert!(html.contains("&amp;"), "subs=verbatim should still escape specialchars. Got: {html}");
        assert!(html.contains("&lt;tag&gt;"), "subs=verbatim should escape angle brackets. Got: {html}");
    }

    #[test]
    fn test_source_block_subs_plus_quotes() {
        let html = to_html("[source,rust,subs=\"+quotes\"]\n----\nlet x = *bold*;\n----");
        assert!(html.contains("<strong>bold</strong>"), "subs=+quotes on source block should enable formatting. Got: {html}");
    }

    #[test]
    fn test_source_block_subs_minus_callouts() {
        // With -callouts, callout markers should be left as-is (not stripped)
        let html = to_html("[source,rust,subs=\"-callouts\"]\n----\nlet x = 1; // <1>\n----");
        assert!(!html.contains("<b class=\"conum\""), "subs=-callouts should not produce callout markers. Got: {html}");
    }

    #[test]
    fn test_listing_block_subs_plus_attributes() {
        let html = to_html(":myattr: hello\n\n[subs=\"+attributes\"]\n----\nValue is {myattr}\n----");
        assert!(html.contains("Value is hello"), "subs=+attributes on listing block should resolve attribute refs. Got: {html}");
    }

    #[test]
    fn test_source_block_subs_normal() {
        let html = to_html("[source,subs=normal]\n----\n*bold* & (C)\n----");
        assert!(html.contains("<strong>bold</strong>"), "subs=normal on source block should enable inline parsing. Got: {html}");
    }

    #[test]
    fn test_listing_block_subs_explicit_list() {
        // Only specialchars and quotes — no replacements
        let html = to_html("[subs=\"specialchars,quotes\"]\n----\n*bold* & (C)\n----");
        assert!(html.contains("<strong>bold</strong>"), "explicit subs should enable quotes. Got: {html}");
        assert!(html.contains("&amp;"), "explicit subs with specialchars should escape &. Got: {html}");
        assert!(html.contains("(C)"), "explicit subs without replacements should not replace (C). Got: {html}");
    }

    #[test]
    fn test_sidebar_block_subs_none() {
        let html = to_html("[subs=none]\n****\n*bold* & <tag>\n****");
        assert!(!html.contains("<strong>"), "subs=none on sidebar should disable inline. Got: {html}");
        assert!(html.contains("<tag>"), "subs=none on sidebar should pass raw tags. Got: {html}");
    }

    #[test]
    fn test_quote_block_subs_verbatim() {
        let html = to_html("[subs=verbatim]\n____\n*bold* & <tag>\n____");
        assert!(!html.contains("<strong>"), "subs=verbatim on quote block should disable inline. Got: {html}");
        assert!(html.contains("&amp;"), "subs=verbatim on quote block should escape &. Got: {html}");
    }

    #[test]
    fn test_source_block_no_highlighter() {
        let html = to_html("[source,rust]\n----\nfn main() {}\n----");
        assert!(html.contains("<pre><code class=\"language-rust\">"), "Without highlighter: bare <pre><code class=\"language-X\">. Got: {html}");
        assert!(!html.contains("data-lang"), "Without highlighter: no data-lang. Got: {html}");
        assert!(!html.contains("class=\"highlight\""), "Without highlighter: no highlight class. Got: {html}");
    }

    #[test]
    fn test_source_block_highlightjs() {
        let html = to_html(":source-highlighter: highlight.js\n\n[source,rust]\n----\nfn main() {}\n----");
        assert!(html.contains("<pre class=\"highlightjs highlight\">"), "highlight.js: pre class. Got: {html}");
        assert!(html.contains("class=\"language-rust\""), "highlight.js: language class on code. Got: {html}");
        assert!(html.contains("data-lang=\"rust\""), "highlight.js: data-lang on code. Got: {html}");
    }

    #[test]
    fn test_source_block_rouge() {
        let html = to_html(":source-highlighter: rouge\n\n[source,ruby]\n----\nputs 'hi'\n----");
        assert!(html.contains("<pre class=\"rouge highlight\">"), "rouge: pre class. Got: {html}");
        assert!(html.contains("data-lang=\"ruby\""), "rouge: data-lang on code. Got: {html}");
        assert!(!html.contains("class=\"language-ruby\""), "rouge: no language- class on code. Got: {html}");
    }

    #[test]
    fn test_source_block_linenums() {
        let html = to_html(":source-highlighter: highlight.js\n\n[source,rust,%linenums]\n----\nfn main() {}\n----");
        assert!(html.contains("linenums"), "linenums option should add linenums class. Got: {html}");
        assert!(html.contains("highlightjs highlight"), "highlightjs highlight classes should be present. Got: {html}");
    }

    #[test]
    fn test_source_block_linenums_no_highlighter() {
        let html = to_html("[source,rust,%linenums]\n----\nfn main() {}\n----");
        assert!(html.contains("linenums"), "linenums should work even without highlighter. Got: {html}");
    }

    #[test]
    fn test_source_block_no_language() {
        let html = to_html(":source-highlighter: highlight.js\n\n[source]\n----\nsome code\n----");
        assert!(html.contains("<pre class=\"highlightjs highlight\">"), "No language: pre class should still have highlighter. Got: {html}");
        assert!(!html.contains("data-lang"), "No language: no data-lang. Got: {html}");
        assert!(!html.contains("language-"), "No language: no language- class. Got: {html}");
    }

    #[test]
    fn test_source_block_pygments() {
        let html = to_html(":source-highlighter: pygments\n\n[source,python]\n----\nprint('hi')\n----");
        assert!(html.contains("<pre class=\"pygments highlight\">"), "pygments: pre class. Got: {html}");
        assert!(html.contains("data-lang=\"python\""), "pygments: data-lang. Got: {html}");
        assert!(!html.contains("class=\"language-python\""), "pygments: no language- class. Got: {html}");
    }

    #[test]
    fn test_source_block_coderay() {
        let html = to_html(":source-highlighter: coderay\n\n[source,java]\n----\nSystem.out.println();\n----");
        assert!(html.contains("<pre class=\"CodeRay highlight\">"), "coderay: pre class. Got: {html}");
        assert!(html.contains("data-lang=\"java\""), "coderay: data-lang. Got: {html}");
    }

    #[test]
    fn test_idprefix_idseparator() {
        // Default: prefix=_ separator=_
        let html = to_html("== My Section\n\nContent.");
        assert!(html.contains("id=\"_my_section\""), "default id. Got: {html}");

        // Empty prefix + dash separator
        let html = to_html(":idprefix:\n:idseparator: -\n\n== My Section\n\nContent.");
        assert!(html.contains("id=\"my-section\""), "custom id. Got: {html}");

        // Custom prefix
        let html = to_html(":idprefix: sec-\n\n== My Section\n\nContent.");
        assert!(html.contains("id=\"sec-my_section\""), "custom prefix. Got: {html}");
    }

    #[test]
    fn test_builtin_attr_backend() {
        let html = to_html("{backend}");
        assert!(html.contains("html5"), "backend should be html5. Got: {html}");
    }

    #[test]
    fn test_builtin_attr_doctype() {
        let html = to_html("{doctype}");
        assert!(html.contains("article"), "doctype should be article. Got: {html}");
    }

    #[test]
    fn test_builtin_attr_doctype_override() {
        let html = to_html(":doctype: book\n\n{doctype}");
        assert!(html.contains("book"), "doctype should be overridden to book. Got: {html}");
        assert!(!html.contains("article"), "should not contain default article. Got: {html}");
    }

    #[test]
    fn test_builtin_attr_author() {
        let html = to_html("= Title\nJohn Doe <john@example.com>\n\n{author} {firstname} {lastname} {authorinitials} {email}");
        assert!(html.contains("John Doe"), "author. Got: {html}");
        assert!(html.contains("John"), "firstname. Got: {html}");
        assert!(html.contains("Doe"), "lastname. Got: {html}");
        assert!(html.contains("JD"), "authorinitials. Got: {html}");
        assert!(html.contains("john@example.com"), "email. Got: {html}");
    }

    #[test]
    fn test_builtin_attr_revision() {
        let html = to_html("= Title\nAuthor Name\nv1.0, 2024-01-01: Initial\n\n{revnumber} {revdate} {revremark}");
        assert!(html.contains("v1.0"), "revnumber. Got: {html}");
        assert!(html.contains("2024-01-01"), "revdate. Got: {html}");
        assert!(html.contains("Initial"), "revremark. Got: {html}");
    }

    #[test]
    fn test_builtin_attr_doctitle() {
        let html = to_html("= My Title\n\n{doctitle}");
        // The doctitle attribute should resolve to "My Title" in the body
        assert_eq!(html.matches("My Title").count(), 2, "doctitle should appear twice (h1 + reference). Got: {html}");
    }

    #[test]
    fn test_attr_fallback() {
        let html = to_html("{undefined!fallback value}");
        assert!(html.contains("fallback value"), "fallback should be used when attr undefined. Got: {html}");
        assert!(!html.contains("{undefined}"), "should not show raw reference. Got: {html}");
    }

    #[test]
    fn test_attr_fallback_not_used() {
        let html = to_html(":name: real\n\n{name!fallback}");
        assert!(html.contains("real"), "defined attr should be used. Got: {html}");
        assert!(!html.contains("fallback"), "fallback should not be used when attr defined. Got: {html}");
    }

    #[test]
    fn test_attr_fallback_empty() {
        let html = to_html("{undefined!}");
        assert!(!html.contains("{undefined}"), "should not show raw reference. Got: {html}");
        // Empty fallback means nothing is rendered for the attribute
        assert!(!html.contains("undefined"), "empty fallback should render nothing for the attr. Got: {html}");
    }
}
