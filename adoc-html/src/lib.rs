use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use adoc_parser::{CellStyle, Event, HAlign, Tag, TagEnd, AdmonitionKind, DelimitedBlockKind, SubstitutionSet, VAlign};

const DEFAULT_STYLESHEET: &str = include_str!("asciidoctor.css");

const INTRINSIC_ATTRIBUTES: &[(&str, &str)] = &[
    ("amp", "&amp;"),
    ("asterisk", "*"),
    ("backslash", "\\"),
    ("backtick", "`"),
    ("blank", ""),
    ("brvbar", "&#166;"),
    ("caret", "^"),
    ("cpp", "C++"),
    ("deg", "&#176;"),
    ("empty", ""),
    ("endsb", "]"),
    ("gt", "&gt;"),
    ("ldquo", "&#8220;"),
    ("lsquo", "&#8216;"),
    ("lt", "&lt;"),
    ("nbsp", "&#160;"),
    ("plus", "&#43;"),
    ("rdquo", "&#8221;"),
    ("rsquo", "&#8217;"),
    ("sp", " "),
    ("startsb", "["),
    ("tilde", "~"),
    ("two-colons", "::"),
    ("two-semicolons", ";;"),
    ("vbar", "|"),
    ("wj", "&#8288;"),
    ("zwsp", "&#8203;"),
];

fn intrinsic_attribute(name: &str) -> Option<&'static str> {
    INTRINSIC_ATTRIBUTES
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
}

#[derive(Default, Clone)]
pub struct HtmlOptions {
    pub docinfo_head: Option<String>,
    pub docinfo_footer: Option<String>,
    pub standalone: bool,
    pub last_updated: Option<String>,
}

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

pub fn push_html_with_options<'a>(
    s: &mut String,
    iter: impl Iterator<Item = Event<'a>>,
    options: HtmlOptions,
) {
    let mut renderer = HtmlRenderer::new_with_options(options);
    renderer.run(s, iter);
}

pub fn to_html_with_options(input: &str, options: HtmlOptions) -> String {
    let parser = adoc_parser::Parser::new(input);
    let mut output = String::new();
    push_html_with_options(&mut output, parser, options);
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

#[derive(Clone)]
struct BlockMeta {
    style: Option<String>,
    id: Option<String>,
    roles: Vec<String>,
    options: Vec<String>,
    named: Vec<(String, String)>,
    subs: Option<SubstitutionSet>,
}

fn parse_highlight_spec(spec: &str) -> HashSet<usize> {
    let mut result = HashSet::new();
    for part in spec.split([',', ';']) {
        let part = part.trim();
        if let Some((start_s, end_s)) = part.split_once("..") {
            if let (Ok(start), Ok(end)) = (start_s.trim().parse::<usize>(), end_s.trim().parse::<usize>()) {
                for n in start..=end {
                    result.insert(n);
                }
            }
        } else if let Ok(n) = part.parse::<usize>() {
            result.insert(n);
        }
    }
    result
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
    example_counter: usize,
    block_title_output_start: Option<usize>,
    block_title_inner_html: Option<String>,
    dlist_stack: Vec<DlistStyle>,
    dd_output_start: Option<usize>,
    hdlist_in_term_group: bool,
    has_document_title: bool,
    capturing_doctitle: bool,
    doctitle_buf: String,
    preamble_start: Option<usize>,
    appendix_counter: u8,
    pending_section_caption: Option<String>,
    sectnums: bool,
    sectanchors: bool,
    showtitle: bool,
    nofooter: bool,
    doctitle_h1_end: Option<usize>,
    section_counters: [u32; 6],
    highlight_lines: HashSet<usize>,
    source_line_num: usize,
    source_line_highlighted: bool,
    docinfo_head: Option<String>,
    docinfo_footer: Option<String>,
    doctitle_close_pos: Option<usize>,
    manpage_name_capture: bool,
    manpage_name_buf: String,
    book_part_stack: Vec<bool>,
    sectionbody_stack: Vec<bool>,
    section_style_stack: Vec<Option<String>>,
    standalone: bool,
    last_updated: Option<String>,
    content_start: Option<usize>,
    in_unlabeled_xref: bool,
    xref_placeholder_counter: usize,
    xref_placeholders: Vec<(String, String)>,
    in_header: bool,
    /// Stack of booleans tracking whether a `<p>` is currently open inside a list item/dd.
    li_p_open: Vec<bool>,
    li_para_count: Vec<u32>,
    linenums_active: bool,
    linenums_start: usize,
    source_code_buffer: Option<String>,
    header_suppress_start: Option<usize>,
    quote_attribution: Option<String>,
    quote_citetitle: Option<String>,
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
            example_counter: 0,
            block_title_output_start: None,
            block_title_inner_html: None,
            dlist_stack: Vec::new(),
            dd_output_start: None,
            hdlist_in_term_group: false,
            has_document_title: false,
            capturing_doctitle: false,
            doctitle_buf: String::new(),
            preamble_start: None,
            appendix_counter: 0,
            pending_section_caption: None,
            sectnums: false,
            sectanchors: false,
            showtitle: false,
            nofooter: false,
            doctitle_h1_end: None,
            section_counters: [0; 6],
            highlight_lines: HashSet::new(),
            source_line_num: 0,
            source_line_highlighted: false,
            docinfo_head: None,
            docinfo_footer: None,
            doctitle_close_pos: None,
            manpage_name_capture: false,
            manpage_name_buf: String::new(),
            book_part_stack: Vec::new(),
            sectionbody_stack: Vec::new(),
            section_style_stack: Vec::new(),
            standalone: false,
            last_updated: None,
            content_start: None,
            in_unlabeled_xref: false,
            xref_placeholder_counter: 0,
            xref_placeholders: Vec::new(),
            in_header: false,
            li_p_open: Vec::new(),
            li_para_count: Vec::new(),
            linenums_active: false,
            linenums_start: 1,
            source_code_buffer: None,
            header_suppress_start: None,
            quote_attribution: None,
            quote_citetitle: None,
        }
    }

    fn new_with_options(options: HtmlOptions) -> Self {
        Self {
            docinfo_head: options.docinfo_head,
            docinfo_footer: options.docinfo_footer,
            standalone: options.standalone,
            last_updated: options.last_updated,
            ..Self::new()
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
        if self.standalone {
            // Render body content into a temporary buffer
            let mut body = String::new();
            for event in iter {
                self.push_event(&mut body, event);
            }
            self.finish(&mut body);

            // Write HTML head (<!DOCTYPE>, <head>, <body>)
            self.write_document_head(output);

            if let Some(split) = self.content_start {
                // Has a document header — emit it, then wrap the rest in <div id="content">
                output.push_str(&body[..split]);
                output.push_str("<div id=\"content\">\n");
                output.push_str(&body[split..]);
                output.push_str("</div>\n");
            } else {
                // No document header — insert empty header, wrap everything in <div id="content">
                output.push_str("<div id=\"header\">\n</div>\n");
                output.push_str("<div id=\"content\">\n");
                output.push_str(&body);
                output.push_str("</div>\n");
            }

            // Footer div
            if !self.nofooter {
                output.push_str("<div id=\"footer\">\n");
                if let Some(ref ts) = self.last_updated {
                    output.push_str("<div id=\"footer-text\">\n");
                    writeln!(output, "Last updated {ts}").unwrap();
                    output.push_str("</div>\n");
                }
                output.push_str("</div>\n");
            }

            self.write_document_tail(output);
        } else {
            for event in iter {
                self.push_event(output, event);
            }
            self.finish(output);
        }
    }

    /// Render an attribute value through inline parsing, so that URLs, formatting,
    /// etc. inside attribute values are properly converted to HTML.
    fn render_inline_value(&mut self, output: &mut String, value: &str) {
        let events = adoc_parser::InlineParser::parse_str_with_subs(value, SubstitutionSet::NORMAL);
        // If inline parsing produced only a single Text event identical to the input,
        // there is no inline markup — just escape and output directly.
        if events.len() == 1
            && let Event::Text(ref t) = events[0]
            && t.as_ref() == value
        {
            html_escape(output, value);
            return;
        }
        for event in events {
            self.push_event(output, event);
        }
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
                if self.manpage_name_capture {
                    self.manpage_name_buf.push_str(&text);
                }
                if self.in_unlabeled_xref {
                    let (ref placeholder, _) = *self.xref_placeholders.last().unwrap();
                    output.push_str(placeholder);
                    self.in_unlabeled_xref = false;
                } else if self.kbd_mode {
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
                    let target = if self.in_source_block {
                        if let Some(ref mut buf) = self.source_code_buffer { buf } else { output }
                    } else {
                        output
                    };
                    if self.in_source_block && self.source_line_num > 0
                        && self.highlight_lines.contains(&self.source_line_num)
                        && !self.source_line_highlighted
                    {
                        target.push_str("<span class=\"hll\">");
                        self.source_line_highlighted = true;
                    }
                    html_escape_text(target, &text);
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
                html_escape_text(output, &code);
                output.push_str("</code>");
            }
            Event::SoftBreak => {
                if self.stem_block_variant.is_some() {
                    self.stem_block_content.get_or_insert_with(String::new).push('\n');
                } else {
                    let target = if self.in_source_block {
                        if let Some(ref mut buf) = self.source_code_buffer { buf } else { output }
                    } else {
                        output
                    };
                    if self.source_line_highlighted {
                        target.push_str("</span>");
                        self.source_line_highlighted = false;
                    }
                    if self.in_source_block && self.source_line_num > 0 {
                        self.source_line_num += 1;
                    }
                    target.push('\n');
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
                if name == "sectanchors" {
                    self.sectanchors = true;
                }
                if name == "!sectanchors" || name == "sectanchors!" {
                    self.sectanchors = false;
                }
                if name == "showtitle" {
                    self.showtitle = true;
                }
                if name == "!showtitle" || name == "showtitle!" {
                    self.showtitle = false;
                }
                if name == "nofooter" {
                    self.nofooter = true;
                }
                if name == "!nofooter" || name == "nofooter!" {
                    self.nofooter = false;
                }
                // Store for attribute reference resolution
                if let Some(stripped) = name.strip_prefix('!') {
                    self.document_attrs.remove(stripped);
                } else if let Some(stripped) = name.strip_suffix('!') {
                    self.document_attrs.remove(stripped);
                } else {
                    self.document_attrs.insert(name.to_string(), value.to_string());
                    if name.as_ref() == "doctype" && value.as_ref() == "manpage" {
                        // Retroactively insert " Manual Page" into the document title
                        if let Some(pos) = self.doctitle_close_pos {
                            output.insert_str(pos, " Manual Page");
                        }
                        // Extract mantitle/manvolnum from doctitle "command(N)"
                        if let Some(doctitle) = self.document_attrs.get("doctitle").cloned()
                            && let Some((mantitle, manvolnum)) = parse_manpage_title(&doctitle)
                        {
                            self.document_attrs.insert("mantitle".to_string(), mantitle);
                            self.document_attrs.insert("manvolnum".to_string(), manvolnum);
                        }
                    }
                }
            }
            Event::AttributeReference { name, fallback } => {
                let lower_name = name.to_ascii_lowercase();
                if let Some(value) = self.document_attrs.get(lower_name.as_str()) {
                    let value = value.clone();
                    self.render_inline_value(output, &value);
                } else if let Some(value) = intrinsic_attribute(&lower_name) {
                    // Intrinsic values are pre-encoded HTML — push raw
                    output.push_str(value);
                } else if let Some(env_name) = name.strip_prefix("env-") {
                    if let Ok(value) = std::env::var(env_name) {
                        html_escape(output, &value);
                    } else if let Some(fb) = fallback {
                        html_escape(output, &fb);
                    } else {
                        output.push('{');
                        output.push_str(&name);
                        output.push('}');
                    }
                } else if let Some(fb) = fallback {
                    html_escape(output, &fb);
                } else {
                    let mode = self.document_attrs.get("attribute-missing").map(|s| s.as_str());
                    match mode {
                        Some("drop") | Some("drop-line") => {
                            // Output nothing
                        }
                        _ => {
                            // "skip" (default) / "warn"
                            output.push('{');
                            output.push_str(&name);
                            output.push('}');
                        }
                    }
                }
            }
            Event::Footnote { id, text } => {
                self.footnote_counter += 1;
                let num = self.footnote_counter;
                if let Some(ref id) = id {
                    self.named_footnotes.insert(id.to_string(), num);
                }
                self.footnotes.push((num, id.as_ref().map(|s| s.to_string()), text.to_string()));
                output.push_str("<sup class=\"footnote\"");
                if let Some(ref id) = id {
                    output.push_str(" id=\"_footnote_");
                    html_escape(output, id);
                    output.push('"');
                }
                output.push_str(">[<a class=\"footnote\" id=\"_footnoteref_");
                output.push_str(&num.to_string());
                output.push_str("\" href=\"#_footnotedef_");
                output.push_str(&num.to_string());
                output.push_str("\" title=\"View footnote.\">");
                output.push_str(&num.to_string());
                output.push_str("</a>]</sup>");
            }
            Event::FootnoteRef { id } => {
                if let Some(&num) = self.named_footnotes.get(id.as_ref()) {
                    output.push_str("<sup class=\"footnote\">[<a class=\"footnote\" id=\"_footnoteref_");
                    output.push_str(&num.to_string());
                    output.push_str("\" href=\"#_footnotedef_");
                    output.push_str(&num.to_string());
                    output.push_str("\" title=\"View footnote.\">");
                    output.push_str(&num.to_string());
                    output.push_str("</a>]</sup>");
                }
            }
            Event::IndexTerm { text } => {
                html_escape_text(output, &text);
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
                let target = if self.in_source_block {
                    if let Some(ref mut buf) = self.source_code_buffer { buf } else { output }
                } else {
                    output
                };
                target.push_str("<b class=\"conum\">(");
                target.push_str(&num.to_string());
                target.push_str(")</b>");
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
            if !toc_html.is_empty() {
                output.insert_str(pos, &toc_html);
                // Shift content_start if TOC was inserted before it
                if let Some(ref mut cs) = self.content_start
                    && pos <= *cs
                {
                    *cs += toc_html.len();
                }
            }
        }

        if !self.xref_placeholders.is_empty() {
            let mut id_to_title: HashMap<String, String> = HashMap::new();
            for entry in &self.toc_entries {
                id_to_title.insert(entry.id.clone(), entry.title.clone());
            }
            for (placeholder, target_id) in &self.xref_placeholders {
                let replacement = if let Some(title) = id_to_title.get(target_id) {
                    let mut escaped = String::new();
                    html_escape(&mut escaped, title);
                    escaped
                } else {
                    let mut escaped = String::new();
                    html_escape(&mut escaped, target_id);
                    escaped
                };
                *output = output.replace(placeholder, &replacement);
            }
        }

        if !self.footnotes.is_empty() {
            self.render_footnotes(output);
        }

        // In standalone mode, docinfo is handled by write_document_head/tail
        if !self.standalone {
            if let Some(ref footer) = self.docinfo_footer
                && !footer.is_empty()
            {
                output.push('\n');
                output.push_str(footer);
            }

            if let Some(ref head) = self.docinfo_head
                && !head.is_empty()
            {
                let mut prefix = head.clone();
                prefix.push('\n');
                output.insert_str(0, &prefix);
            }
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
            html_escape_text(output, text);
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

        let mut current_level = min_level - 1;

        for entry in &entries {
            let level = entry.level;

            if level > current_level {
                // Going deeper — open new ul(s)
                while current_level < level {
                    if !toc.ends_with('\n') {
                        toc.push('\n');
                    }
                    current_level += 1;
                    let sl = current_level - 1;
                    writeln!(toc, "<ul class=\"sectlevel{sl}\">").unwrap();
                }
            } else if level < current_level {
                // Going shallower — close nested lists
                while current_level > level {
                    toc.push_str("</li>\n</ul>\n");
                    current_level -= 1;
                }
                // Close previous item at this level
                toc.push_str("</li>\n");
            } else {
                // Same level — close previous item
                toc.push_str("</li>\n");
            }

            toc.push_str("<li><a href=\"#");
            html_escape(&mut toc, &entry.id);
            toc.push_str("\">");
            html_escape(&mut toc, &entry.title);
            toc.push_str("</a>");
        }

        // Close all remaining open levels
        while current_level >= min_level {
            toc.push_str("</li>\n</ul>\n");
            current_level -= 1;
        }
        toc.push_str("</div>\n");

        toc
    }

    fn start_tag(&mut self, output: &mut String, tag: &Tag) {
        // Close <p> inside list item when a sub-block starts
        match tag {
            Tag::Paragraph | Tag::UnorderedList { .. } | Tag::OrderedList { .. }
            | Tag::DescriptionList | Tag::DelimitedBlock { .. } | Tag::SourceBlock { .. }
            | Tag::BlockImage { .. } | Tag::Table => {
                if self.li_p_open.last() == Some(&true) {
                    output.push_str("</p>\n");
                    *self.li_p_open.last_mut().unwrap() = false;
                }
            }
            _ => {}
        }

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
                self.in_header = true;
                if self.standalone {
                    output.push_str("<div id=\"header\">\n");
                } else {
                    // In embedded mode, suppress header output
                    self.header_suppress_start = Some(output.len());
                }
            }
            Tag::DocumentTitle => {
                self.has_document_title = true;
                self.capturing_doctitle = true;
                self.doctitle_buf.clear();
                // No <h1> here — the enclosing SectionTitle already emits <h1 id="...">.
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
                let is_part = self.is_book() && *level == 1
                    && self.book_part_stack.last() == Some(&true);
                output.push_str("<h");
                output.push_str(&h.to_string());
                if !self.in_header {
                    output.push_str(" id=\"");
                    html_escape(output, id);
                    output.push('"');
                }
                if is_part {
                    output.push_str(" class=\"sect0\"");
                }
                output.push('>');
                if self.sectanchors && !self.in_header {
                    output.push_str("<a class=\"anchor\" href=\"#");
                    html_escape(output, id);
                    output.push_str("\"></a>");
                }
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
            Tag::Section { level } => {
                let style = meta.as_ref().and_then(|m| m.style.as_deref());
                let is_special = matches!(style, Some(
                    "appendix" | "glossary" | "bibliography" | "colophon"
                    | "abstract" | "preface" | "dedication" | "index"
                ));
                let is_part = self.is_book() && *level == 1 && !is_special;
                self.book_part_stack.push(is_part);
                self.sectionbody_stack.push(*level == 2 && !is_part);
                self.section_style_stack.push(
                    if is_special { style.map(|s| s.to_string()) } else { None }
                );
                if !is_part {
                    output.push_str("<div");
                    let sect_class = format!("sect{}", level - 1);
                    // ID goes on the heading, not on the section div
                    let mut div_meta = meta.clone();
                    if let Some(ref mut m) = div_meta {
                        m.id = None;
                        if style == Some("bibliography") {
                            m.style = None;
                        }
                    }
                    Self::write_meta_attrs(output, &div_meta, &sect_class);
                    output.push_str(">\n");
                }
                if style == Some("appendix") {
                    self.appendix_counter += 1;
                    let letter = (b'A' + self.appendix_counter - 1) as char;
                    self.pending_section_caption = Some(format!("Appendix {letter}: "));
                } else if is_special {
                    self.pending_section_caption = Some(String::new());
                }
            }
            Tag::Paragraph => {
                // Track paragraph count in list items
                if let Some(count) = self.li_para_count.last_mut() {
                    *count += 1;
                }
                let is_continuation_para = self.li_para_count.last().is_some_and(|&c| c > 1);
                if self.is_direct_child_of_admonition() {
                    // Inline admonitions: no <p> wrapper
                } else if !self.is_inside_compact_context() || is_continuation_para {
                    output.push_str("<div");
                    Self::write_meta_attrs(output, &meta, "paragraph");
                    output.push_str(">\n");
                    self.emit_pending_block_title(output);
                    output.push_str("<p>");
                } else {
                    output.push_str("<p>");
                }
            }
            Tag::LiteralParagraph => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "literalblock");
                output.push_str(">\n<div class=\"content\">\n<pre>");
            }
            Tag::DelimitedBlock { kind } => {
                match kind {
                    DelimitedBlockKind::Listing => {
                        self.delimited_block_stack.push((*kind, false));
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "listingblock");
                        output.push_str(">\n");
                        self.emit_pending_block_title(output);
                        output.push_str("<div class=\"content\">\n<pre>");
                    }
                    DelimitedBlockKind::Literal => {
                        self.delimited_block_stack.push((*kind, false));
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "literalblock");
                        output.push_str(">\n");
                        self.emit_pending_block_title(output);
                        output.push_str("<div class=\"content\">\n<pre>");
                    }
                    DelimitedBlockKind::Example => {
                        let is_collapsible = meta.as_ref()
                            .is_some_and(|m| m.options.iter().any(|o| o == "collapsible"));

                        if is_collapsible {
                            let is_open = meta.as_ref()
                                .is_some_and(|m| m.options.iter().any(|o| o == "open"));

                            let summary = self.block_title_inner_html.take()
                                .unwrap_or_else(|| "Details".to_string());

                            output.push_str("<details");
                            Self::write_meta_attrs(output, &meta, "");
                            if is_open {
                                output.push_str(" open");
                            }
                            output.push_str(">\n<summary class=\"title\">");
                            output.push_str(&summary);
                            output.push_str("</summary>\n<div class=\"content\">\n");
                            self.delimited_block_stack.push((*kind, true));
                        } else {
                            self.delimited_block_stack.push((*kind, false));
                            output.push_str("<div");
                            Self::write_meta_attrs(output, &meta, "exampleblock");
                            output.push_str(">\n");
                            if let Some(title) = self.block_title_inner_html.take() {
                                self.example_counter += 1;
                                let caption_attr = meta.as_ref().and_then(|m| {
                                    m.named.iter().find(|(k, _)| k == "caption").map(|(_, v)| v.clone())
                                });
                                output.push_str("<div class=\"title\">");
                                match caption_attr.as_deref() {
                                    Some("") => {}
                                    Some(prefix) => {
                                        html_escape(output, prefix);
                                    }
                                    None => {
                                        output.push_str("Example ");
                                        output.push_str(&self.example_counter.to_string());
                                        output.push_str(". ");
                                    }
                                }
                                output.push_str(&title);
                                output.push_str("</div>\n");
                            }
                            output.push_str("<div class=\"content\">\n");
                        }
                    }
                    DelimitedBlockKind::Sidebar => {
                        self.delimited_block_stack.push((*kind, false));
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "sidebarblock");
                        output.push_str(">\n<div class=\"content\">\n");
                        self.emit_pending_block_title(output);
                    }
                    DelimitedBlockKind::Quote => {
                        self.delimited_block_stack.push((*kind, false));
                        // Capture attribution and citetitle from metadata
                        if let Some(ref m) = meta {
                            self.quote_attribution = m.named.iter()
                                .find(|(k, _)| k == "attribution")
                                .map(|(_, v)| v.clone());
                            self.quote_citetitle = m.named.iter()
                                .find(|(k, _)| k == "citetitle")
                                .map(|(_, v)| v.clone());
                        }
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "quoteblock");
                        output.push_str(">\n");
                        self.emit_pending_block_title(output);
                        output.push_str("<blockquote>\n");
                    }
                    DelimitedBlockKind::Open => {
                        self.delimited_block_stack.push((*kind, false));
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "openblock");
                        output.push_str(">\n");
                        self.emit_pending_block_title(output);
                        output.push_str("<div class=\"content\">\n");
                    }
                    DelimitedBlockKind::Comment => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_inner_html = None;
                        // Comment blocks are not rendered
                    }
                    DelimitedBlockKind::Passthrough => {
                        self.delimited_block_stack.push((*kind, false));
                        self.block_title_inner_html = None;
                        // Passthrough: content is rendered as-is
                    }
                    DelimitedBlockKind::Verse => {
                        self.delimited_block_stack.push((*kind, false));
                        // Capture attribution and citetitle from metadata
                        if let Some(ref m) = meta {
                            self.quote_attribution = m.named.iter()
                                .find(|(k, _)| k == "attribution")
                                .map(|(_, v)| v.clone());
                            self.quote_citetitle = m.named.iter()
                                .find(|(k, _)| k == "citetitle")
                                .map(|(_, v)| v.clone());
                        }
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &meta, "verseblock");
                        output.push_str(">\n");
                        self.emit_pending_block_title(output);
                        output.push_str("<pre class=\"content\">");
                    }
                }
            }
            Tag::SourceBlock { language } => {
                self.in_source_block = true;
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "listingblock");
                output.push_str(">\n");
                self.emit_pending_block_title(output);
                output.push_str("<div class=\"content\">\n<pre");

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
                // Source blocks always get "highlight" class (matches Asciidoctor behavior)
                pre_classes.push("highlight");
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
                    if matches!(highlighter.as_deref(), Some("highlight.js" | "highlightjs")) {
                        output.push_str(" class=\"hljs language-");
                        html_escape(output, lang);
                        output.push('"');
                    } else if highlighter.is_none() {
                        output.push_str(" class=\"language-");
                        html_escape(output, lang);
                        output.push('"');
                    }
                    output.push_str(" data-lang=\"");
                    html_escape(output, lang);
                    output.push('"');
                }

                if let Some(hl_spec) = meta.as_ref()
                    .and_then(|m| m.named.iter().find(|(k, _)| k == "highlight").map(|(_, v)| v.clone()))
                {
                    self.highlight_lines = parse_highlight_spec(&hl_spec);
                    self.source_line_num = 1;
                } else {
                    self.source_line_num = 0;
                }
                self.source_line_highlighted = false;

                if linenums {
                    self.linenums_active = true;
                    self.linenums_start = meta.as_ref()
                        .and_then(|m| m.named.iter().find(|(k, _)| k == "start"))
                        .and_then(|(_, v)| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    self.source_code_buffer = Some(String::new());
                    if self.source_line_num == 0 {
                        self.source_line_num = 1;
                    }
                }

                output.push('>');
            }
            Tag::BlockTitle => {
                self.block_title_output_start = Some(output.len());
                output.push_str("<div class=\"title\">");
            }
            Tag::UnorderedList { has_checklist } => {
                let is_bibliography = self.section_style_stack.last()
                    .and_then(|s| s.as_deref()) == Some("bibliography");
                if !self.is_inside_list_item() {
                    let wrapper_class = if *has_checklist {
                        "ulist checklist"
                    } else if is_bibliography {
                        "ulist bibliography"
                    } else {
                        "ulist"
                    };
                    output.push_str("<div");
                    Self::write_meta_attrs(output, &meta, wrapper_class);
                    output.push_str(">\n<ul");
                    if *has_checklist {
                        output.push_str(" class=\"checklist\"");
                    } else if is_bibliography {
                        output.push_str(" class=\"bibliography\"");
                    }
                    output.push_str(">\n");
                } else {
                    let wrapper_class = if *has_checklist { "ulist checklist" } else { "ulist" };
                    output.push_str("<div class=\"");
                    output.push_str(wrapper_class);
                    output.push_str("\">\n<ul");
                    if *has_checklist {
                        output.push_str(" class=\"checklist\"");
                    }
                    output.push_str(">\n");
                }
            }
            Tag::OrderedList { start, reversed } => {
                let style_name = meta.as_ref()
                    .and_then(|m| m.style.as_deref())
                    .unwrap_or_else(|| {
                        // Auto-assign style based on nesting depth (like Asciidoctor).
                        // tag_stack already contains the current OrderedList, so subtract 1.
                        let depth = self.tag_stack.iter()
                            .filter(|t| matches!(t, TagEnd::OrderedList))
                            .count()
                            .saturating_sub(1);
                        match depth {
                            0 => "arabic",
                            1 => "loweralpha",
                            2 => "lowerroman",
                            3 => "upperalpha",
                            _ => "upperroman",
                        }
                    });
                let wrapper_class = format!("olist {style_name}");
                if !self.is_inside_list_item() {
                    output.push_str("<div");
                    // Write id/roles from meta onto the wrapper div
                    let mut wrapper_meta = meta.clone();
                    if let Some(ref mut m) = wrapper_meta {
                        m.style = None; // style goes into wrapper class
                    }
                    Self::write_meta_attrs(output, &wrapper_meta, &wrapper_class);
                    output.push_str(">\n");
                } else {
                    output.push_str("<div class=\"");
                    output.push_str(&wrapper_class);
                    output.push_str("\">\n");
                }
                output.push_str("<ol class=\"");
                output.push_str(style_name);
                output.push('"');
                let type_attr = match style_name {
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
                if let Some(s) = start {
                    use std::fmt::Write;
                    let _ = write!(output, " start=\"{}\"", s);
                }
                if *reversed {
                    output.push_str(" reversed");
                }
                output.push_str(">\n");
            }
            Tag::ListItem { checked: Some(true), .. } => {
                output.push_str("<li>\n<p>&#10003; ");
                self.li_p_open.push(true);
                self.li_para_count.push(1); // count the initial <p>
            }
            Tag::ListItem { checked: Some(false), .. } => {
                output.push_str("<li>\n<p>&#10063; ");
                self.li_p_open.push(true);
                self.li_para_count.push(1); // count the initial <p>
            }
            Tag::ListItem { checked: None, .. } => {
                output.push_str("<li>\n<p>");
                self.li_p_open.push(true);
                self.li_para_count.push(1); // count the initial <p>
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
                        output.push_str("<div");
                        Self::write_meta_attrs(output, &adjusted_meta, "dlist");
                        output.push_str(">\n<dl>\n");
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
                        output.push_str("<dt class=\"hdlist1\">");
                    }
                }
            }
            Tag::DescriptionDescription => {
                self.li_para_count.push(1); // count the initial <p> in <dd>
                match self.current_dlist_style() {
                    DlistStyle::Horizontal => {
                        output.push_str("</td>\n<td class=\"hdlist2\">\n<p>");
                        self.hdlist_in_term_group = false;
                        self.li_p_open.push(true);
                    }
                    DlistStyle::Qanda => {}
                    DlistStyle::Normal => {
                        self.dd_output_start = Some(output.len());
                        output.push_str("<dd>\n<p>");
                        self.li_p_open.push(true);
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
                self.emit_pending_block_title(output);
            }
            Tag::Table => {
                // Collect extra CSS classes from options/named attrs
                let has_autowidth = meta.as_ref().is_some_and(|m| m.options.iter().any(|o| o == "autowidth"));
                let stripes_value = meta.as_ref().and_then(|m| m.named.iter().find(|(k, _)| k == "stripes").map(|(_, v)| v.clone()));

                // Base Asciidoctor table classes
                let mut classes = String::from("tableblock frame-all grid-all");
                if !has_autowidth {
                    classes.push_str(" stretch");
                } else {
                    classes.push_str(" fit-content");
                }
                if let Some(ref sv) = stripes_value {
                    classes.push_str(" stripes-");
                    classes.push_str(sv);
                }

                // Extract cols spec for colgroup generation
                let cols_value = meta.as_ref().and_then(|m| m.named.iter()
                    .find(|(k, _)| k == "cols").map(|(_, v)| v.clone()));

                output.push_str("<table");
                Self::write_meta_attrs(output, &meta, &classes);
                output.push_str(">\n");

                // Emit <colgroup> based on cols spec
                if let Some(ref cols_str) = cols_value {
                    let col_widths = Self::parse_col_widths(cols_str);
                    if !col_widths.is_empty() {
                        output.push_str("<colgroup>\n");
                        for w in &col_widths {
                            output.push_str("<col style=\"width: ");
                            output.push_str(&w.to_string());
                            output.push_str("%;\">\n");
                        }
                        output.push_str("</colgroup>\n");
                    }
                }

                // Caption handling
                let title_html = self.block_title_inner_html.take();
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
                let cell_class = Self::tableblock_cell_class(halign, valign);
                output.push_str("<td class=\"");
                output.push_str(&cell_class);
                output.push('"');
                if *colspan > 1 {
                    output.push_str(&format!(" colspan=\"{}\"", colspan));
                }
                if *rowspan > 1 {
                    output.push_str(&format!(" rowspan=\"{}\"", rowspan));
                }
                output.push('>');
                match style {
                    CellStyle::Emphasis => output.push_str("<p class=\"tableblock\"><em>"),
                    CellStyle::Strong => output.push_str("<p class=\"tableblock\"><strong>"),
                    CellStyle::Monospace | CellStyle::Literal => output.push_str("<p class=\"tableblock\"><code>"),
                    CellStyle::AsciiDoc => {}
                    _ => output.push_str("<p class=\"tableblock\">"),
                }
            }
            Tag::TableHeaderCell { colspan, rowspan, style, halign, valign } => {
                self.cell_style_stack.push(*style);
                let cell_class = Self::tableblock_cell_class(halign, valign);
                output.push_str("<th class=\"");
                output.push_str(&cell_class);
                output.push('"');
                if *colspan > 1 {
                    output.push_str(&format!(" colspan=\"{}\"", colspan));
                }
                if *rowspan > 1 {
                    output.push_str(&format!(" rowspan=\"{}\"", rowspan));
                }
                output.push('>');
                match style {
                    CellStyle::Emphasis => output.push_str("<em>"),
                    CellStyle::Strong => output.push_str("<strong>"),
                    CellStyle::Monospace | CellStyle::Literal => output.push_str("<code>"),
                    _ => {}
                }
            }
            Tag::BlockImage { target, alt, width, height, link } => {
                // Build base class with align/float CSS classes from named attrs
                let base_class = Self::image_base_class("imageblock", &meta);
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, &base_class);
                output.push_str(">\n<div class=\"content\">\n");
                let has_link = link.is_some();
                if let Some(href) = link {
                    output.push_str("<a class=\"image\" href=\"");
                    html_escape(output, href);
                    output.push_str("\">");
                }
                output.push_str("<img src=\"");
                html_escape(output, &target.as_ref().replace(' ', "%20"));
                // Auto-generate alt from filename if empty
                let effective_alt = if alt.as_ref().is_empty() {
                    auto_alt_from_target(target)
                } else {
                    alt.to_string()
                };
                output.push_str("\" alt=\"");
                html_escape(output, &effective_alt);
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
                output.push('>');
                if has_link {
                    output.push_str("</a>");
                }
                output.push_str("\n</div>\n");
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
            Tag::InlineImage { target, alt, width, height, align, float, link } => {
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
                output.push_str("\">");
                let has_link = link.is_some();
                if let Some(href) = link {
                    output.push_str("<a class=\"image\" href=\"");
                    html_escape(output, href);
                    output.push_str("\">");
                }
                output.push_str("<img src=\"");
                html_escape(output, &target.as_ref().replace(' ', "%20"));
                let effective_alt = if alt.as_ref().is_empty() {
                    auto_alt_from_target(target)
                } else {
                    alt.to_string()
                };
                output.push_str("\" alt=\"");
                html_escape(output, &effective_alt);
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
                output.push('>');
                if has_link {
                    output.push_str("</a>");
                }
                output.push_str("</span>");
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
            Tag::Link { url, window, nofollow, is_bare } => {
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
                if *is_bare {
                    output.push_str(" class=\"bare\"");
                }
                output.push('>');
            }
            Tag::CrossReference { target, label } => {
                if target.contains('.') && !target.starts_with('#') {
                    // Inter-document xref: rewrite .adoc to .html
                    output.push_str("<a href=\"");
                    let rewritten = if let Some(base) = target.strip_suffix(".adoc") {
                        format!("{base}.html")
                    } else if let Some((file_part, anchor)) = target.split_once('#')
                        && file_part.ends_with(".adoc")
                    {
                        format!("{}.html#{anchor}", &file_part[..file_part.len()-5])
                    } else {
                        target.to_string()
                    };
                    html_escape(output, &rewritten);
                } else {
                    // Internal xref (anchor reference)
                    output.push_str("<a href=\"#");
                    html_escape(output, target);
                }
                output.push_str("\">");
                if label.is_none() {
                    self.in_unlabeled_xref = true;
                    self.xref_placeholder_counter += 1;
                    let placeholder = format!("\x00XREF_{}\x00", self.xref_placeholder_counter);
                    self.xref_placeholders.push((placeholder, target.to_string()));
                }
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
            Tag::CustomInlineMacro { name, .. } => {
                output.push_str("<span class=\"custom-macro macro-");
                html_escape(output, name);
                output.push_str("\">");
            }
            Tag::CustomBlockMacro { name, .. } => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, &format!("custom-macro macro-{name}"));
                output.push_str(">\n");
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
                self.in_header = false;
                if self.standalone {
                    output.push_str("</div>\n");
                    self.content_start = Some(output.len());
                    if self.has_document_title {
                        self.preamble_start = Some(output.len());
                    }
                } else if let Some(pos) = self.header_suppress_start.take() {
                    if self.showtitle {
                        if let Some(h1_end) = self.doctitle_h1_end {
                            output.truncate(h1_end);
                        } else {
                            output.truncate(pos);
                        }
                    } else {
                        output.truncate(pos);
                    }
                    // Reset TOC insert position to after truncation point
                    if self.toc_auto_seen {
                        self.toc_insert_position = Some(output.len());
                    }
                    if self.has_document_title {
                        self.preamble_start = Some(output.len());
                    }
                }
            }
            TagEnd::DocumentTitle => {
                self.doctitle_close_pos = Some(output.len());
                // No </h1> here — the enclosing SectionTitle's End emits </h1>.
                self.capturing_doctitle = false;
                let title = std::mem::take(&mut self.doctitle_buf);
                self.document_attrs.insert("doctitle".to_string(), title);
            }
            TagEnd::SectionTitle => {
                if self.in_section_title {
                    if let Some(ref entry) = self.current_toc_entry
                        && self.document_attrs.get("doctype").map(|s| s.as_str()) == Some("manpage")
                        && entry.title.eq_ignore_ascii_case("NAME")
                    {
                        self.manpage_name_capture = true;
                    }
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
                if self.in_header && self.has_document_title {
                    self.doctitle_h1_end = Some(output.len());
                }
                if level == 2
                    && let Some(&true) = self.sectionbody_stack.last()
                {
                    output.push_str("<div class=\"sectionbody\">\n");
                }
            }
            TagEnd::Heading { level } => {
                let h = section_level_to_h(*level);
                output.push_str("</h");
                output.push_str(&h.to_string());
                output.push_str(">\n");
            }
            TagEnd::Section { .. } => {
                if self.manpage_name_capture {
                    self.manpage_name_capture = false;
                    let buf = std::mem::take(&mut self.manpage_name_buf);
                    let trimmed = buf.trim();
                    if let Some(dash_pos) = trimmed.find(" - ") {
                        self.document_attrs.insert("manname".to_string(), trimmed[..dash_pos].trim().to_string());
                        self.document_attrs.insert("manpurpose".to_string(), trimmed[dash_pos + 3..].trim().to_string());
                    }
                }
                let is_part = self.book_part_stack.pop().unwrap_or(false);
                let needs_sectionbody_close = self.sectionbody_stack.pop().unwrap_or(false);
                self.section_style_stack.pop();
                if !is_part {
                    if needs_sectionbody_close {
                        output.push_str("</div>\n");
                    }
                    output.push_str("</div>\n");
                }
            }
            TagEnd::Paragraph => {
                // Trim trailing whitespace before closing <p>
                let trimmed = output.trim_end_matches([' ', '\t']);
                output.truncate(trimmed.len());

                let is_continuation_para = self.li_para_count.last().is_some_and(|&c| c > 1);
                if self.is_direct_child_of_admonition() {
                    // Inline admonitions: no </p>
                    output.push('\n');
                } else if !self.is_inside_compact_context() || is_continuation_para {
                    output.push_str("</p>\n</div>\n");
                } else {
                    output.push_str("</p>\n");
                }
            }
            TagEnd::LiteralParagraph => {
                output.push_str("</pre>\n</div>\n</div>\n");
            }
            TagEnd::DelimitedBlock => {
                match self.delimited_block_stack.pop() {
                    Some((DelimitedBlockKind::Listing | DelimitedBlockKind::Literal, _)) => {
                        // Trim leading/trailing blank lines in verbatim content (matches Asciidoctor)
                        Self::trim_verbatim_content(output);
                        output.push_str("</pre>\n</div>\n</div>\n");
                    }
                    Some((DelimitedBlockKind::Quote, _)) => {
                        output.push_str("</blockquote>\n");
                        let attribution = self.quote_attribution.take();
                        let citetitle = self.quote_citetitle.take();
                        if attribution.is_some() || citetitle.is_some() {
                            output.push_str("<div class=\"attribution\">\n");
                            if let Some(ref attr) = attribution {
                                output.push_str("&#8212; ");
                                html_escape(output, attr);
                            }
                            if let Some(ref cite) = citetitle {
                                if attribution.is_some() {
                                    output.push_str("<br>\n");
                                }
                                output.push_str("<cite>");
                                html_escape(output, cite);
                                output.push_str("</cite>");
                            }
                            output.push('\n');
                            output.push_str("</div>\n");
                        }
                        output.push_str("</div>\n");
                    }
                    Some((DelimitedBlockKind::Verse, _)) => {
                        output.push_str("</pre>\n");
                        let attribution = self.quote_attribution.take();
                        let citetitle = self.quote_citetitle.take();
                        if attribution.is_some() || citetitle.is_some() {
                            output.push_str("<div class=\"attribution\">\n");
                            if let Some(ref attr) = attribution {
                                output.push_str("&#8212; ");
                                html_escape(output, attr);
                            }
                            if let Some(ref cite) = citetitle {
                                if attribution.is_some() {
                                    output.push_str("<br>\n");
                                }
                                output.push_str("<cite>");
                                html_escape(output, cite);
                                output.push_str("</cite>");
                            }
                            output.push('\n');
                            output.push_str("</div>\n");
                        }
                        output.push_str("</div>\n");
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
                if self.linenums_active {
                    if self.source_line_highlighted {
                        self.source_code_buffer.as_mut().unwrap().push_str("</span>");
                        self.source_line_highlighted = false;
                    }
                    let code = self.source_code_buffer.take().unwrap();
                    let code_trimmed = code.strip_suffix('\n').unwrap_or(&code);
                    let line_count = code_trimmed.split('\n').count();
                    let start = self.linenums_start;

                    output.push_str("<table class=\"linenotable\"><tbody><tr>\n<td class=\"linenos\"><pre class=\"linenos\">");
                    for i in 0..line_count {
                        if i > 0 {
                            output.push('\n');
                        }
                        let _ = write!(output, "{}", start + i);
                    }
                    output.push_str("</pre></td>\n<td class=\"code\"><pre>");
                    output.push_str(code_trimmed);
                    output.push_str("</pre></td>\n</tr></tbody></table>");

                    self.linenums_active = false;
                    self.linenums_start = 1;
                } else {
                    if self.source_line_highlighted {
                        output.push_str("</span>");
                        self.source_line_highlighted = false;
                    }
                }
                self.highlight_lines.clear();
                self.source_line_num = 0;
                self.in_source_block = false;
                output.push_str("</code></pre>\n</div>\n</div>\n");
            }
            TagEnd::BlockTitle => {
                if let Some(start) = self.block_title_output_start.take() {
                    let title_tag = "<div class=\"title\">";
                    let inner_start = start + title_tag.len();
                    self.block_title_inner_html = Some(output[inner_start..].to_string());
                    output.truncate(start);
                }
            }
            TagEnd::UnorderedList => {
                output.push_str("</ul>\n</div>\n");
            }
            TagEnd::OrderedList => {
                output.push_str("</ol>\n</div>\n");
            }
            TagEnd::ListItem => {
                self.li_para_count.pop();
                if self.li_p_open.pop() == Some(true) {
                    output.push_str("</p>\n</li>\n");
                } else {
                    output.push_str("</li>\n");
                }
            }
            TagEnd::DescriptionList => {
                let style = self.dlist_stack.pop().unwrap_or(DlistStyle::Normal);
                match style {
                    DlistStyle::Horizontal => output.push_str("</table>\n</div>\n"),
                    DlistStyle::Qanda => output.push_str("</ol>\n</div>\n"),
                    DlistStyle::Normal => output.push_str("</dl>\n</div>\n"),
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
                self.li_para_count.pop();
                match self.current_dlist_style() {
                    DlistStyle::Horizontal => {
                        if self.li_p_open.pop() == Some(true) {
                            output.push_str("</p>\n");
                        }
                        output.push_str("</td>\n</tr>\n");
                    }
                    DlistStyle::Qanda => output.push_str("</li>\n"),
                    DlistStyle::Normal => {
                        // Check if dd is empty (term-only) — if so, rollback
                        if let Some(start) = self.dd_output_start.take() {
                            let dd_content = &output[start..];
                            // "<dd>\n<p>" is 8 chars; if nothing was added after, it's empty
                            if dd_content == "<dd>\n<p>" {
                                output.truncate(start);
                                self.li_p_open.pop();
                                // skip emitting </dd>
                            } else if self.li_p_open.pop() == Some(true) {
                                output.push_str("</p>\n</dd>\n");
                            } else {
                                output.push_str("</dd>\n");
                            }
                        } else if self.li_p_open.pop() == Some(true) {
                            output.push_str("</p>\n</dd>\n");
                        } else {
                            output.push_str("</dd>\n");
                        }
                    }
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
                    CellStyle::Emphasis => output.push_str("</em></p>"),
                    CellStyle::Strong => output.push_str("</strong></p>"),
                    CellStyle::Monospace | CellStyle::Literal => output.push_str("</code></p>"),
                    CellStyle::AsciiDoc => {}
                    _ => output.push_str("</p>"),
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
                self.in_unlabeled_xref = false;
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
            TagEnd::CustomInlineMacro => {
                output.push_str("</span>");
            }
            TagEnd::CustomBlockMacro => {
                output.push_str("</div>\n");
            }
        }
    }

    fn emit_pending_block_title(&mut self, output: &mut String) {
        if let Some(title) = self.block_title_inner_html.take() {
            output.push_str("<div class=\"title\">");
            output.push_str(&title);
            output.push_str("</div>\n");
        }
    }

    fn take_block_meta(&mut self) -> Option<BlockMeta> {
        self.pending_block_meta.take()
    }

    /// Trim leading and trailing blank lines from verbatim (pre) content in the output buffer.
    /// Finds the last `<pre>` or `<pre ...>` tag and trims blank lines after it and before end.
    fn trim_verbatim_content(output: &mut String) {
        // Find the position right after the last <pre> or <pre ...> tag
        if let Some(pre_start) = output.rfind("<pre") {
            let after_pre = if let Some(gt) = output[pre_start..].find('>') {
                pre_start + gt + 1
            } else {
                return;
            };
            let content = &output[after_pre..];
            // Trim leading blank lines
            let leading_trimmed = content.trim_start_matches('\n');
            let leading_removed = content.len() - leading_trimmed.len();
            // Trim trailing blank lines
            let trailing_trimmed = leading_trimmed.trim_end_matches('\n');
            if leading_removed > 0 || trailing_trimmed.len() != leading_trimmed.len() {
                let new_content = trailing_trimmed.to_string();
                output.truncate(after_pre);
                output.push_str(&new_content);
            }
        }
    }

    /// Parse a cols spec string (e.g. "1,1" or "3" or "<,^,>") and return percentage widths.
    fn parse_col_widths(cols_str: &str) -> Vec<u32> {
        let trimmed = cols_str.trim();

        // Simple numeric: "3" → 3 equal columns
        if let Ok(n) = trimmed.parse::<usize>() {
            if n == 0 { return Vec::new(); }
            let pct = 100 / n as u32;
            return vec![pct; n];
        }

        // Comma-separated: parse each part for weight
        let parts: Vec<&str> = trimmed.split(',').collect();
        let mut weights: Vec<u32> = Vec::new();
        for part in &parts {
            let part = part.trim();
            if part.is_empty() { continue; }
            // Extract numeric weight from the spec (e.g., "1", "<2", "^.>1")
            // The weight is the trailing number, default is 1
            let weight = part.chars().rev()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .chars().rev().collect::<String>()
                .parse::<u32>()
                .unwrap_or(1);
            weights.push(weight);
        }

        if weights.is_empty() { return Vec::new(); }

        let total: u32 = weights.iter().sum();
        if total == 0 { return Vec::new(); }

        weights.iter().map(|w| {
            (w * 100 + total / 2) / total
        }).collect()
    }

    fn tableblock_cell_class(halign: &HAlign, valign: &VAlign) -> String {
        let ha = match halign {
            HAlign::Left => "halign-left",
            HAlign::Center => "halign-center",
            HAlign::Right => "halign-right",
        };
        let va = match valign {
            VAlign::Top => "valign-top",
            VAlign::Middle => "valign-middle",
            VAlign::Bottom => "valign-bottom",
        };
        format!("tableblock {ha} {va}")
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
            output.push_str("<span class=\"menuseq\"><b class=\"menu\">");
            html_escape(output, &target);
            output.push_str("</b>");
            for (i, part) in parts.iter().enumerate() {
                output.push_str("&#160;<b class=\"caret\">&#8250;</b> ");
                if i < parts.len() - 1 {
                    output.push_str("<b class=\"submenu\">");
                    html_escape(output, part);
                    output.push_str("</b>");
                } else {
                    output.push_str("<b class=\"menuitem\">");
                    html_escape(output, part);
                    output.push_str("</b>");
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

        if let Some(href) = &link {
            output.push_str("<a class=\"icon\" href=\"");
            html_escape(output, href);
            output.push_str("\">");
        } else {
            output.push_str("<span class=\"icon\">");
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
        } else {
            output.push_str("</span>");
        }
    }

    fn render_inline_stem(&mut self, output: &mut String) {
        let variant = match self.stem_variant.take() {
            Some(v) => v,
            None => return,
        };
        let content = self.stem_content.take().unwrap_or_default();

        // Resolve "stem" to the document attribute :stem: value
        let resolved = if variant == "stem" {
            self.document_attrs.get("stem")
                .map(|s| s.as_str())
                .unwrap_or("asciimath")
        } else {
            &variant
        };

        if resolved == "latexmath" {
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

        // Resolve "stem" to the document attribute :stem: value
        let resolved = if variant == "stem" {
            self.document_attrs.get("stem")
                .map(|s| s.as_str())
                .unwrap_or("asciimath")
        } else {
            &variant
        };

        if resolved == "latexmath" {
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

    fn write_document_head(&self, output: &mut String) {
        let doctitle = self.document_attrs.get("doctitle")
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .unwrap_or("Untitled");
        let doctype = self.document_attrs.get("doctype").map(|s| s.as_str()).unwrap_or("article");

        output.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        output.push_str("<meta charset=\"UTF-8\">\n");
        output.push_str("<meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\">\n");
        output.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        output.push_str("<meta name=\"generator\" content=\"adoc-parser\">\n");
        output.push_str("<title>");
        html_escape(output, doctitle);
        output.push_str("</title>\n");
        output.push_str("<link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css?family=Open+Sans:300,300italic,400,400italic,600,600italic%7CNoto+Serif:400,400italic,700,700italic%7CDroid+Sans+Mono:400,700\">\n");
        output.push_str("<style>\n");
        output.push_str(DEFAULT_STYLESHEET);
        output.push_str("\n</style>\n");
        if let Some(ref head) = self.docinfo_head
            && !head.is_empty()
        {
            output.push_str(head);
            output.push('\n');
        }
        output.push_str("</head>\n");

        // Build body classes
        let mut body_classes = String::from(doctype);
        if !self.toc_position.is_empty() {
            body_classes.push_str(" toc2");
            if self.toc_position == "right" {
                body_classes.push_str(" toc-right");
            }
        }
        output.push_str("<body class=\"");
        output.push_str(&body_classes);
        output.push_str("\">\n");
    }

    fn write_document_tail(&self, output: &mut String) {
        if let Some(ref footer) = self.docinfo_footer
            && !footer.is_empty()
        {
            output.push_str(footer);
            output.push('\n');
        }
        output.push_str("</body>\n</html>");
    }

    fn is_book(&self) -> bool {
        self.document_attrs.get("doctype").map(|s| s.as_str()) == Some("book")
    }

    fn find_section_level(&self) -> u8 {
        for tag_end in self.tag_stack.iter().rev() {
            if let TagEnd::Section { level } = tag_end {
                return *level;
            }
        }
        1
    }

    /// Returns true when the immediate parent on the tag stack is an Admonition.
    /// Used to suppress <p> tags for inline admonitions.
    fn is_direct_child_of_admonition(&self) -> bool {
        // In start_tag: stack has [..., Admonition, Paragraph], so check second-to-last
        // In end_tag: stack has [..., Admonition] (Paragraph already popped), so check last
        // Both cases: look for Admonition as the nearest non-Paragraph ancestor
        for tag_end in self.tag_stack.iter().rev() {
            match tag_end {
                TagEnd::Paragraph => continue, // skip self during start_tag
                TagEnd::Admonition => return true,
                _ => return false,
            }
        }
        false
    }

    /// Returns true when inside a list item (for skipping list wrapper divs on nested lists).
    fn is_inside_list_item(&self) -> bool {
        for tag_end in self.tag_stack.iter().rev() {
            match tag_end {
                TagEnd::ListItem | TagEnd::DescriptionDescription | TagEnd::CalloutListItem => return true,
                TagEnd::Section { .. } | TagEnd::DelimitedBlock => return false,
                _ => {}
            }
        }
        false
    }

    /// Returns true when inside a context that should NOT get paragraph/list wrapper divs.
    /// These are: ListItem, DescriptionDescription, Admonition, CalloutListItem, TableCell, TableHeaderCell.
    fn is_inside_compact_context(&self) -> bool {
        for tag_end in self.tag_stack.iter().rev() {
            match tag_end {
                TagEnd::ListItem
                | TagEnd::DescriptionDescription
                | TagEnd::Admonition
                | TagEnd::CalloutListItem
                | TagEnd::TableCell
                | TagEnd::TableHeaderCell => return true,
                TagEnd::Section { .. }
                | TagEnd::DelimitedBlock
                | TagEnd::SourceBlock => return false,
                _ => {}
            }
        }
        false
    }
}

fn parse_manpage_title(title: &str) -> Option<(String, String)> {
    let title = title.trim();
    let paren_start = title.rfind('(')?;
    if !title.ends_with(')') {
        return None;
    }
    let name = title[..paren_start].trim();
    let volnum = &title[paren_start + 1..title.len() - 1];
    if name.is_empty() || volnum.is_empty() {
        return None;
    }
    Some((name.to_string(), volnum.to_string()))
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

/// Detect video provider from the first positional attribute.
fn detect_video_provider(attrs: &str) -> Option<&str> {
    let first = attrs.split(',').next().unwrap_or("").trim();
    match first {
        "youtube" | "vimeo" => Some(first),
        _ => None,
    }
}

fn render_video_tag(output: &mut String, target: &str, attrs: &str) {
    let media = parse_media_attrs(attrs);

    match detect_video_provider(attrs) {
        Some("youtube") => {
            output.push_str("<iframe");
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
            output.push_str(" src=\"https://www.youtube.com/embed/");
            html_escape(output, target);
            output.push_str("?rel=0");
            if media.autoplay {
                output.push_str("&amp;autoplay=1");
            }
            if media.loop_ {
                output.push_str("&amp;loop=1");
            }
            if media.nocontrols {
                output.push_str("&amp;controls=0");
            }
            if let Some(s) = media.start {
                output.push_str("&amp;start=");
                output.push_str(s);
            }
            if let Some(e) = media.end {
                output.push_str("&amp;end=");
                output.push_str(e);
            }
            output.push_str("\" frameborder=\"0\" allowfullscreen></iframe>\n</div>\n");
        }
        Some("vimeo") => {
            output.push_str("<iframe");
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
            output.push_str(" src=\"https://player.vimeo.com/video/");
            html_escape(output, target);
            output.push_str("\" frameborder=\"0\" allowfullscreen></iframe>\n</div>\n");
        }
        _ => {
            // Regular HTML5 video
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
    }
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

/// Generate alt text from image target: strip path, strip extension, replace `-_` with spaces.
fn auto_alt_from_target(target: &str) -> String {
    // Get filename (last path component)
    let filename = target.rsplit('/').next().unwrap_or(target);
    // Strip extension
    let stem = match filename.rfind('.') {
        Some(pos) if pos > 0 => &filename[..pos],
        _ => filename,
    };
    // Replace hyphens and underscores with spaces
    stem.replace(['-', '_'], " ")
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

/// Like `html_escape` but does NOT escape `"` — for use in text content (not attributes).
fn html_escape_text(output: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
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
        assert_eq!(html, "<div class=\"paragraph\">\n<p>Hello world.</p>\n</div>\n");
    }

    #[test]
    fn test_bold_text() {
        let html = to_html("Hello *bold* world.");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>Hello <strong>bold</strong> world.</p>\n</div>\n");
    }

    #[test]
    fn test_italic_text() {
        let html = to_html("Hello _italic_ world.");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>Hello <em>italic</em> world.</p>\n</div>\n");
    }

    #[test]
    fn test_document_title_no_duplicate_h1() {
        let html = to_html_with_options("= Document Title\n\nContent.", HtmlOptions { standalone: true, ..Default::default() });
        // Must produce exactly one <h1> opening tag, not <h1 id="..."><h1>
        let h1_count = html.matches("<h1").count();
        assert_eq!(h1_count, 1, "expected exactly one <h1> tag, got {h1_count}. HTML:\n{html}");
        assert!(
            html.contains("<h1>Document Title</h1>"),
            "expected <h1>Document Title</h1> (no id in header), got:\n{html}"
        );
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
        assert!(html.contains("<div class=\"ulist\">\n<ul>"));
        assert!(html.contains("<li>\n<p>item 1</p>\n</li>"));
        assert!(html.contains("<li>\n<p>item 2</p>\n</li>"));
        assert!(html.contains("</ul>\n</div>"));
    }

    #[test]
    fn test_ordered_list() {
        let html = to_html(". first\n. second");
        assert!(html.contains("<div class=\"olist arabic\">\n<ol"));
        assert!(!html.contains("type="));
        assert!(!html.contains("start="));
        assert!(!html.contains("reversed"));
        assert!(html.contains("<li>\n<p>first</p>\n</li>"));
        assert!(html.contains("<li>\n<p>second</p>\n</li>"));
        assert!(html.contains("</ol>\n</div>"));
    }

    #[test]
    fn test_ordered_list_loweralpha() {
        let html = to_html("[loweralpha]\n. a\n. b");
        assert!(html.contains("<ol class=\"loweralpha\" type=\"a\""), "expected ol with class and type. Got:\n{html}");
        assert!(html.contains("class=\"olist loweralpha\""));
    }

    #[test]
    fn test_ordered_list_upperroman() {
        let html = to_html("[upperroman]\n. x\n. y");
        assert!(html.contains("<ol class=\"upperroman\" type=\"I\""), "expected ol with class and type. Got:\n{html}");
        assert!(html.contains("class=\"olist upperroman\""));
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
        assert!(html.contains("class=\"olist loweralpha\""));
    }

    #[test]
    fn test_source_block() {
        let html = to_html("[source,rust]\n----\nfn main() {\n    println!(\"hello\");\n}\n----");
        assert!(html.contains("language-rust"));
        assert!(html.contains("fn main()"));
        assert!(html.contains("\"hello\""));
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
    fn test_link_passthrough_url_with_spaces() {
        let html = to_html("link:++https://example.com/my page++[Click]");
        assert!(html.contains("<a href=\"https://example.com/my page\">Click</a>"));
    }

    #[test]
    fn test_link_passthrough_url_with_brackets() {
        let html = to_html("link:++https://example.com/path[1]++[Click]");
        assert!(html.contains("<a href=\"https://example.com/path[1]\">Click</a>"));
    }

    #[test]
    fn test_link_passthrough_url_empty_text() {
        let html = to_html("link:++https://example.com/my page++[]");
        assert!(html.contains("<a href=\"https://example.com/my page\">https://example.com/my page</a>"));
    }

    #[test]
    fn test_link_passthrough_url_with_attrs() {
        let html = to_html("link:++https://example.com/my page++[Click,window=_blank]");
        assert!(html.contains("<a href=\"https://example.com/my page\""));
        assert!(html.contains("target=\"_blank\""));
        assert!(html.contains(">Click</a>"));
    }

    #[test]
    fn test_email_autolink_html() {
        let html = to_html("Contact user@example.com for info");
        assert!(html.contains("<a href=\"mailto:user@example.com\" class=\"bare\">user@example.com</a>"));
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
        assert!(html.contains("\"quotes\""));
        assert!(!html.contains("&quot;"));
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
        // In embedded mode, document header (h1) is not rendered
        let html = to_html("= My Document\n\nContent.");
        assert!(!html.contains("<h1>"), "embedded mode should not render document header h1. Got:\n{html}");
        // In standalone mode, the header is rendered
        let html = to_html_with_options("= My Document\n\nContent.", HtmlOptions { standalone: true, ..Default::default() });
        assert!(html.contains("<h1>My Document</h1>"),
            "expected <h1>My Document</h1> in standalone mode, got:\n{html}");
    }

    #[test]
    fn test_description_list_html() {
        let html = to_html("CPU:: The brain\nRAM:: Memory");
        assert_eq!(
            html,
            "<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">CPU</dt>\n<dd>\n<p>The brain</p>\n</dd>\n<dt class=\"hdlist1\">RAM</dt>\n<dd>\n<p>Memory</p>\n</dd>\n</dl>\n</div>\n"
        );
    }

    #[test]
    fn test_nested_description_list_html() {
        let html = to_html("CPU:: The brain\nSpeed::: Fast");
        assert_eq!(
            html,
            "<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">CPU</dt>\n<dd>\n<p>The brain</p>\n<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">Speed</dt>\n<dd>\n<p>Fast</p>\n</dd>\n</dl>\n</div>\n</dd>\n</dl>\n</div>\n"
        );
    }

    #[test]
    fn test_list_continuation_html() {
        let html = to_html("* item\n+\nContinued.");
        assert!(html.contains("<p>item</p>\n<div class=\"paragraph\">\n<p>Continued.</p>\n</div>"), "continuation should be wrapped in div.paragraph:\n{html}");
    }

    #[test]
    fn test_description_list_continuation_html() {
        let html = to_html("Term:: desc\n+\nMore.");
        assert!(html.contains("<p>desc</p>\n<div class=\"paragraph\">\n<p>More.</p>\n</div>"), "dlist continuation should be wrapped in div.paragraph:\n{html}");
    }

    #[test]
    fn test_inline_passthrough_html() {
        let html = to_html("hello +++<b>bold</b>+++ world");
        assert!(html.contains("hello <b>bold</b> world"));
    }

    #[test]
    fn test_table_html() {
        let html = to_html("|===\n| A | B\n| C | D\n|===");
        assert!(html.contains("<table class=\"tableblock frame-all grid-all stretch\">"), "expected table classes. Got:\n{html}");
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<tr>"));
        assert!(html.contains("<p class=\"tableblock\">A</p>"));
        assert!(html.contains("<p class=\"tableblock\">B</p>"));
        assert!(html.contains("<p class=\"tableblock\">C</p>"));
        assert!(html.contains("<p class=\"tableblock\">D</p>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("</table>"));
        assert!(!html.contains("<thead>"));
    }

    #[test]
    fn test_table_with_header_html() {
        let html = to_html("|===\n| Header 1 | Header 2\n\n| Cell 1 | Cell 2\n| Cell 3 | Cell 4\n|===");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Header 1</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Header 2</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<p class=\"tableblock\">Cell 1</p>"));
        assert!(html.contains("<p class=\"tableblock\">Cell 2</p>"));
        assert!(html.contains("<p class=\"tableblock\">Cell 3</p>"));
        assert!(html.contains("<p class=\"tableblock\">Cell 4</p>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("</table>"));
    }

    #[test]
    fn test_table_with_cols_html() {
        let html = to_html("[cols=\"2\"]\n|===\n| A\n| B\n| C\n| D\n|===");
        assert!(html.contains("<table class=\"tableblock frame-all grid-all stretch\">"));
        assert!(html.contains("<tbody>"));
        // Should have 2 rows of 2 cells
        let td_count = html.matches("<td class=\"tableblock").count();
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
        assert!(html.contains("<p class=\"tableblock\">A</p>"));
        assert!(html.contains("<p class=\"tableblock\">B</p>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("<tfoot>"));
        assert!(html.contains("<p class=\"tableblock\">F1</p>"));
        assert!(html.contains("<p class=\"tableblock\">F2</p>"));
        assert!(html.contains("</tfoot>"));
        assert!(!html.contains("<thead>"));
    }

    #[test]
    fn test_footnote_html() {
        let html = to_html("Hello footnote:[This is a note] world.");
        assert!(html.contains("<sup class=\"footnote\">[<a class=\"footnote\" id=\"_footnoteref_1\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>"));
        assert!(html.contains("<div id=\"footnotes\">"));
        assert!(html.contains("<hr>"));
        assert!(html.contains("<div class=\"footnote\" id=\"_footnotedef_1\">"));
        assert!(html.contains("<a href=\"#_footnoteref_1\">1</a>. This is a note"));
    }

    #[test]
    fn test_footnote_named_html() {
        let html = to_html("First footnote:fn1[Named note] and again footnote:fn1[].");
        // Definition
        assert!(html.contains("<sup class=\"footnote\" id=\"_footnote_fn1\">[<a class=\"footnote\" id=\"_footnoteref_1\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>"));
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
        assert!(html.contains("<div id=\"toc\""), "should contain TOC. Got:\n{html}");
        let toc_pos = html.find("<div id=\"toc\"").unwrap();
        let section_pos = html.find("<div class=\"sect1\"").unwrap();
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
        assert!(html.contains("<div class=\"ulist checklist\">\n<ul class=\"checklist\">"));
        assert!(html.contains("<li>\n<p>&#10003; Done</p>\n</li>"));
        assert!(html.contains("<li>\n<p>&#10063; Todo</p>\n</li>"));
        assert!(html.contains("</ul>\n</div>"));
    }

    #[test]
    fn test_checklist_mixed_html() {
        let html = to_html("* [x] Checked\n* Regular\n* [ ] Unchecked");
        assert!(html.contains("<div class=\"ulist checklist\">\n<ul class=\"checklist\">"));
        assert!(html.contains("<li>\n<p>&#10003; Checked</p>\n</li>"));
        assert!(html.contains("<li>\n<p>Regular</p>\n</li>"));
        assert!(html.contains("<li>\n<p>&#10063; Unchecked</p>\n</li>"));
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
        assert!(html.contains("<p class=\"tableblock\">A</p>"));
        assert!(html.contains("colspan=\"2\"><p class=\"tableblock\">B spans</p>"));
        assert!(html.contains("<p class=\"tableblock\">C</p>"));
        assert!(html.contains("<p class=\"tableblock\">D</p>"));
        assert!(html.contains("<p class=\"tableblock\">E</p>"));
    }

    #[test]
    fn test_table_rowspan_html() {
        let html = to_html("|===\n.2+| A | B\n| C\n|===");
        assert!(html.contains("rowspan=\"2\"><p class=\"tableblock\">A</p>"));
        assert!(html.contains("<p class=\"tableblock\">B</p>"));
        assert!(html.contains("<p class=\"tableblock\">C</p>"));
        // Should have 2 rows
        assert_eq!(html.matches("<tr>").count(), 2);
    }

    #[test]
    fn test_table_colspan_rowspan_html() {
        let html = to_html("|===\n2.3+| cell | B\n| C\n| D\n|===");
        assert!(html.contains("colspan=\"2\" rowspan=\"3\"><p class=\"tableblock\">cell</p>"));
    }

    #[test]
    fn test_table_cell_style_emphasis_html() {
        let html = to_html("|===\ne| italic\n|===");
        assert!(html.contains("<p class=\"tableblock\"><em>italic</em></p>"), "expected emphasis in tableblock p. Got:\n{html}");
    }

    #[test]
    fn test_table_cell_style_strong_html() {
        let html = to_html("|===\ns| bold\n|===");
        assert!(html.contains("<p class=\"tableblock\"><strong>bold</strong></p>"), "expected strong in tableblock p. Got:\n{html}");
    }

    #[test]
    fn test_table_cell_style_monospace_html() {
        let html = to_html("|===\nm| code\n|===");
        assert!(html.contains("<p class=\"tableblock\"><code>code</code></p>"), "expected code in tableblock p. Got:\n{html}");
    }

    #[test]
    fn test_table_cell_style_literal_html() {
        let html = to_html("|===\nl| literal\n|===");
        assert!(html.contains("<p class=\"tableblock\"><code>literal</code></p>"), "expected code in tableblock p. Got:\n{html}");
    }

    #[test]
    fn test_table_cell_style_header_in_body_html() {
        let html = to_html("|===\nh| header cell\n|===");
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">header cell</th>"), "expected th with tableblock class. Got:\n{html}");
    }

    #[test]
    fn test_table_cell_style_with_colspan_html() {
        let html = to_html("|===\n2+e| wide italic | B\n| C | D\n|===");
        assert!(html.contains("colspan=\"2\"><p class=\"tableblock\"><em>wide italic</em></p>"), "expected colspan with emphasis in tableblock p. Got:\n{html}");
    }

    #[test]
    fn test_table_cell_style_no_false_positive_html() {
        // "data" ends with 'a' but should NOT be treated as AsciiDoc style
        let html = to_html("|===\n| data | more\n|===");
        assert!(html.contains("<p class=\"tableblock\">data</p>"));
        assert!(html.contains("<p class=\"tableblock\">more</p>"));
    }

    #[test]
    fn test_table_cols_alignment_html() {
        let html = to_html("[cols=\"<,^,>\"]\n|===\n| A | B | C\n|===");
        assert!(html.contains("halign-left"), "Left-aligned should have halign-left class");
        assert!(html.contains("halign-center"), "Center should have halign-center class");
        assert!(html.contains("halign-right"), "Right should have halign-right class");
        assert!(html.contains("<p class=\"tableblock\">A</p>"));
        assert!(html.contains("<p class=\"tableblock\">B</p>"));
        assert!(html.contains("<p class=\"tableblock\">C</p>"));
    }

    #[test]
    fn test_table_cell_align_html() {
        let html = to_html("|===\n^| centered\n|===");
        assert!(html.contains("halign-center"), "expected halign-center class. Got:\n{html}");
        assert!(html.contains("<p class=\"tableblock\">centered</p>"));
    }

    #[test]
    fn test_table_cell_combined_align_html() {
        let html = to_html("|===\n>.^| text\n|===");
        assert!(html.contains("halign-right valign-middle"), "expected halign-right valign-middle. Got:\n{html}");
        assert!(html.contains("<p class=\"tableblock\">text</p>"));
    }

    #[test]
    fn test_table_cell_override_cols_align_html() {
        // cols says left, cell overrides to center
        let html = to_html("[cols=\"<,<\"]\n|===\n^| centered | normal\n|===");
        assert!(html.contains("halign-center"), "cell should override to center. Got:\n{html}");
        assert!(html.contains("<p class=\"tableblock\">centered</p>"));
        assert!(html.contains("<p class=\"tableblock\">normal</p>"));
    }

    #[test]
    fn test_table_valign_only_html() {
        let html = to_html("|===\n.>| bottom\n|===");
        assert!(html.contains("valign-bottom"), "expected valign-bottom class. Got:\n{html}");
        assert!(html.contains("<p class=\"tableblock\">bottom</p>"));
    }

    #[test]
    fn test_table_cols_valign_html() {
        let html = to_html("[cols=\".^,1\"]\n|===\n| A | B\n|===");
        assert!(html.contains("valign-middle"), "expected valign-middle class. Got:\n{html}");
        assert!(html.contains("<p class=\"tableblock\">A</p>"));
        assert!(html.contains("<p class=\"tableblock\">B</p>"));
    }

    #[test]
    fn test_kbd_single_key_html() {
        let html = to_html("kbd:[F11]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><kbd>F11</kbd></p>\n</div>\n");
    }

    #[test]
    fn test_kbd_combo_html() {
        let html = to_html("kbd:[Ctrl+C]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"keyseq\"><kbd>Ctrl</kbd>+<kbd>C</kbd></span></p>\n</div>\n");
    }

    #[test]
    fn test_btn_html() {
        let html = to_html("btn:[OK]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><b class=\"button\">OK</b></p>\n</div>\n");
    }

    #[test]
    fn test_menu_html() {
        let html = to_html("menu:File[Save As]");
        assert_eq!(
            html,
            "<div class=\"paragraph\">\n<p><span class=\"menuseq\"><b class=\"menu\">File</b>&#160;<b class=\"caret\">&#8250;</b> <b class=\"menuitem\">Save As</b></span></p>\n</div>\n"
        );
    }

    #[test]
    fn test_menu_no_items_html() {
        let html = to_html("menu:File[]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"menu\">File</span></p>\n</div>\n");
    }

    #[test]
    fn test_icon_basic_html() {
        let html = to_html("icon:heart[]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-heart\"></i></span></p>\n</div>\n");
    }

    #[test]
    fn test_icon_size_html() {
        let html = to_html("icon:heart[2x]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-heart fa-2x\"></i></span></p>\n</div>\n");
    }

    #[test]
    fn test_icon_role_html() {
        let html = to_html("icon:tags[role=blue]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-tags blue\"></i></span></p>\n</div>\n");
    }

    #[test]
    fn test_icon_title_html() {
        let html = to_html("icon:info[title=Info]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-info\" title=\"Info\"></i></span></p>\n</div>\n");
    }

    #[test]
    fn test_icon_rotate_html() {
        let html = to_html("icon:shield[rotate=90]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-shield fa-rotate-90\"></i></span></p>\n</div>\n");
    }

    #[test]
    fn test_icon_flip_html() {
        let html = to_html("icon:shield[flip=vertical]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-shield fa-flip-vertical\"></i></span></p>\n</div>\n");
    }

    #[test]
    fn test_icon_link_html() {
        let html = to_html("icon:download[link=https://example.com]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><a class=\"icon\" href=\"https://example.com\"><i class=\"fa fa-download\"></i></a></p>\n</div>\n");
    }

    #[test]
    fn test_icon_combined_html() {
        let html = to_html("icon:heart[2x,role=red]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-heart fa-2x red\"></i></span></p>\n</div>\n");
    }

    #[test]
    fn test_menu_submenus_html() {
        let html = to_html("menu:File[New > Doc]");
        assert_eq!(
            html,
            "<div class=\"paragraph\">\n<p><span class=\"menuseq\"><b class=\"menu\">File</b>&#160;<b class=\"caret\">&#8250;</b> <b class=\"submenu\">New</b>&#160;<b class=\"caret\">&#8250;</b> <b class=\"menuitem\">Doc</b></span></p>\n</div>\n"
        );
    }

    // Stem macro tests

    #[test]
    fn test_stem_inline_html() {
        let html = to_html("stem:[x^2]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>\\$x^2\\$</p>\n</div>\n");
    }

    #[test]
    fn test_latexmath_inline_html() {
        let html = to_html("latexmath:[C = \\alpha]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>\\(C = \\alpha\\)</p>\n</div>\n");
    }

    #[test]
    fn test_asciimath_inline_html() {
        let html = to_html("asciimath:[sqrt(4)]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>\\$sqrt(4)\\$</p>\n</div>\n");
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
        assert_eq!(html, "<div class=\"paragraph\">\n<p>I love tigers very much</p>\n</div>\n");
    }

    #[test]
    fn test_concealed_index_term_html() {
        let html = to_html("(((animals, cats)))Visible text");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>Visible text</p>\n</div>\n");
    }

    #[test]
    fn test_indexterm2_macro_html() {
        let html = to_html("indexterm2:[tigers]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>tigers</p>\n</div>\n");
    }

    #[test]
    fn test_indexterm_macro_html() {
        let html = to_html("indexterm:[animals, cats]");
        assert_eq!(html, "<div class=\"paragraph\">\n<p></p>\n</div>\n");
    }

    #[test]
    fn test_flow_index_term_escaping_html() {
        let html = to_html("((a <b> & c))");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>a &lt;b&gt; &amp; c</p>\n</div>\n");
    }

    // Block metadata: custom id/class tests

    #[test]
    fn test_paragraph_with_id_and_role() {
        let html = to_html("[#notice.important]\nText");
        assert!(html.contains("id=\"notice\""), "should have id on div. Got: {html}");
        assert!(html.contains("class=\"paragraph important\""), "should have class on div. Got: {html}");
        assert!(html.contains("<p>Text</p>"), "p should be plain. Got: {html}");
    }

    #[test]
    fn test_paragraph_with_id_only() {
        let html = to_html("[#myid]\nHello");
        assert!(html.contains("<div id=\"myid\" class=\"paragraph\">"), "id on div. Got: {html}");
        assert!(html.contains("<p>Hello</p>"), "p should be plain. Got: {html}");
    }

    #[test]
    fn test_paragraph_with_role_only() {
        let html = to_html("[.lead]\nText");
        assert!(html.contains("class=\"paragraph lead\""), "role on div. Got: {html}");
        assert!(html.contains("<p>Text</p>"), "p should be plain. Got: {html}");
    }

    #[test]
    fn test_paragraph_with_multiple_roles() {
        let html = to_html("[.r1.r2.r3]\nText");
        assert!(html.contains("class=\"paragraph r1 r2 r3\""), "roles on div. Got: {html}");
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
        assert!(html.contains("<div id=\"mylist\" class=\"ulist\">"));
    }

    #[test]
    fn test_table_with_id_and_role() {
        let html = to_html("[#data.striped]\n|===\n| A | B\n|===");
        assert!(html.contains("id=\"data\""), "expected id=\"data\". Got:\n{html}");
        assert!(html.contains("striped"), "expected striped in class. Got:\n{html}");
    }

    #[test]
    fn test_table_autowidth_html() {
        let html = to_html("[%autowidth]\n|===\n| A | B\n|===");
        assert!(html.contains("fit-content"), "expected fit-content class. Got:\n{html}");
        assert!(html.contains("tableblock frame-all grid-all"), "expected tableblock classes. Got:\n{html}");
    }

    #[test]
    fn test_table_stripes_html() {
        let html = to_html("[stripes=even]\n|===\n| A | B\n|===");
        assert!(html.contains("stripes-even"), "expected stripes-even class. Got:\n{html}");
        assert!(html.contains("tableblock frame-all grid-all"), "expected tableblock classes. Got:\n{html}");
    }

    #[test]
    fn test_table_stripes_odd_html() {
        let html = to_html("[stripes=odd]\n|===\n| A | B\n|===");
        assert!(html.contains("stripes-odd"), "expected stripes-odd class. Got:\n{html}");
        assert!(html.contains("tableblock frame-all grid-all"), "expected tableblock classes. Got:\n{html}");
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
        assert!(html.contains("<table class=\"tableblock frame-all grid-all stretch\">"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Age</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">City</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
        assert!(html.contains("<p class=\"tableblock\">30</p>"));
        assert!(html.contains("<p class=\"tableblock\">NYC</p>"));
        assert!(html.contains("<p class=\"tableblock\">Bob</p>"));
        assert!(html.contains("<p class=\"tableblock\">25</p>"));
        assert!(html.contains("<p class=\"tableblock\">LA</p>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("</table>"));
    }

    #[test]
    fn test_csv_table_shorthand_html() {
        let html = to_html("[%header,csv]\n|===\nName,Age\nAlice,30\n|===");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Age</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
        assert!(html.contains("<p class=\"tableblock\">30</p>"));
        assert!(html.contains("</tbody>"));
    }

    #[test]
    fn test_dsv_table_html() {
        let html = to_html("[%header,format=dsv]\n|===\nName:Age:City\nAlice:30:NYC\n|===");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Age</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">City</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
        assert!(html.contains("<p class=\"tableblock\">30</p>"));
        assert!(html.contains("<p class=\"tableblock\">NYC</p>"));
        assert!(html.contains("</tbody>"));
    }

    #[test]
    fn test_tsv_table_html() {
        let html = to_html("[%header,format=tsv]\n|===\nName\tAge\tCity\nAlice\t30\tNYC\n|===");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Age</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">City</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
        assert!(html.contains("<p class=\"tableblock\">30</p>"));
        assert!(html.contains("<p class=\"tableblock\">NYC</p>"));
        assert!(html.contains("</tbody>"));
    }

    #[test]
    fn test_csv_table_no_header_html() {
        let html = to_html("[format=csv]\n|===\nAlice,30\nBob,25\n|===");
        assert!(!html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
        assert!(html.contains("<p class=\"tableblock\">30</p>"));
        assert!(html.contains("<p class=\"tableblock\">Bob</p>"));
        assert!(html.contains("<p class=\"tableblock\">25</p>"));
        assert!(html.contains("</tbody>"));
    }

    #[test]
    fn test_csv_table_quoted_fields_html() {
        let html = to_html("[%header,csv]\n|===\nName,Description\nAlice,\"Has a, comma\"\n|===");
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
        assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Description</th>"));
        assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
        assert!(html.contains("<p class=\"tableblock\">Has a, comma</p>"));
    }

    #[test]
    fn test_discrete_heading_with_id_and_role() {
        let html = to_html("[discrete#myh.special]\n== Heading");
        assert!(html.contains("id=\"myh\""), "should have explicit id. Got: {html}");
        assert!(html.contains("class=\"discrete special\""), "should have discrete + role class. Got: {html}");
    }

    // Inline span tests

    #[test]
    fn test_inline_span_single_role_html() {
        let html = to_html("[.lead]#text#");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"lead\">text</span></p>\n</div>\n");
    }

    #[test]
    fn test_inline_span_multiple_roles_html() {
        let html = to_html("[.big.red]#text#");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"big red\">text</span></p>\n</div>\n");
    }

    #[test]
    fn test_inline_span_id_and_role_html() {
        let html = to_html("[#myid.lead]#text#");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><span id=\"myid\" class=\"lead\">text</span></p>\n</div>\n");
    }

    #[test]
    fn test_inline_span_unconstrained_html() {
        let html = to_html("hel[.x]##lo##rld");
        assert_eq!(html, "<div class=\"paragraph\">\n<p>hel<span class=\"x\">lo</span>rld</p>\n</div>\n");
    }

    #[test]
    fn test_bare_highlight_no_regression_html() {
        let html = to_html("#highlight#");
        assert_eq!(html, "<div class=\"paragraph\">\n<p><mark>highlight</mark></p>\n</div>\n");
    }

    #[test]
    fn test_block_admonition_html() {
        let html = to_html("[NOTE]\n====\nThis is a note.\n====");
        assert!(html.contains("<div class=\"admonitionblock note\">"), "no admonitionblock note in:\n{html}");
        assert!(html.contains("<div class=\"title\">Note</div>"), "no title in:\n{html}");
        assert!(html.contains("<td class=\"content\">\nThis is a note.\n</td>"), "no td content in:\n{html}");
        assert!(html.contains("</td>\n</tr>\n</table>\n</div>"), "no closing tags in:\n{html}");
    }

    #[test]
    fn test_block_admonition_multi_para_html() {
        let html = to_html("[NOTE]\n====\nFirst paragraph.\n\nSecond paragraph.\n====");
        assert!(html.contains("<div class=\"admonitionblock note\">"), "no admonition class in:\n{html}");
        assert!(html.contains("First paragraph."), "no first para in:\n{html}");
        assert!(html.contains("Second paragraph."), "no second para in:\n{html}");
        assert!(html.contains("<td class=\"content\">"), "no td content in:\n{html}");
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
        assert!(html.contains("class=\"sect1 appendix\""));
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
        assert!(html.contains("class=\"sect1 glossary\""));
    }

    #[test]
    fn test_bibliography_section_html() {
        let html = to_html("[bibliography]\n== References\n\n* [[[ref1]]] First ref.");
        // bibliography style is not added to section div, but propagated to child list
        assert!(html.contains("class=\"sect1\""));
        assert!(html.contains("class=\"ulist bibliography\""));
        assert!(html.contains("class=\"bibliography\""));
    }

    #[test]
    fn test_colophon_section_html() {
        let html = to_html("[colophon]\n== Colophon\n\nPublishing info.");
        assert!(html.contains("class=\"sect1 colophon\""));
    }

    #[test]
    fn test_abstract_section_html() {
        let html = to_html("[abstract]\n== Summary\n\nBrief summary.");
        assert!(html.contains("class=\"sect1 abstract\""));
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
             <tr>\n<td class=\"hdlist1\">CPU</td>\n<td class=\"hdlist2\">\n<p>The brain</p>\n</td>\n</tr>\n\
             <tr>\n<td class=\"hdlist1\">RAM</td>\n<td class=\"hdlist2\">\n<p>Memory</p>\n</td>\n</tr>\n\
             </table>\n</div>\n"
        );
    }

    #[test]
    fn test_horizontal_description_list_multiple_terms_html() {
        // Parser treats each term:: line as separate entry
        // This test verifies multiple entries render correctly
        let html = to_html("[horizontal]\nTerm1:: Desc1\nTerm2:: Desc2");
        assert!(html.contains("<td class=\"hdlist1\">Term1</td>"));
        assert!(html.contains("<td class=\"hdlist2\">\n<p>Desc1</p>\n</td>"));
        assert!(html.contains("<td class=\"hdlist1\">Term2</td>"));
        assert!(html.contains("<td class=\"hdlist2\">\n<p>Desc2</p>\n</td>"));
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
            "<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">CPU</dt>\n<dd>\n<p>The brain</p>\n</dd>\n<dt class=\"hdlist1\">RAM</dt>\n<dd>\n<p>Memory</p>\n</dd>\n</dl>\n</div>\n"
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
    fn test_block_image_with_link() {
        let html = to_html("image::thumb.jpg[Alt,link=fullsize.jpg]");
        assert!(html.contains("<a class=\"image\" href=\"fullsize.jpg\"><img src=\"thumb.jpg\" alt=\"Alt\"></a>"));
    }

    #[test]
    fn test_inline_image_with_link() {
        let html = to_html("text image:icon.png[Icon,link=https://example.com] more");
        assert!(html.contains("<a class=\"image\" href=\"https://example.com\"><img src=\"icon.png\" alt=\"Icon\"></a>"));
    }

    #[test]
    fn test_block_image_with_link_and_dimensions() {
        let html = to_html("image::photo.jpg[Alt,300,200,link=big.jpg]");
        assert!(html.contains("<a class=\"image\" href=\"big.jpg\"><img src=\"photo.jpg\" alt=\"Alt\" width=\"300\" height=\"200\"></a>"));
    }

    #[test]
    fn test_block_image_without_link_no_anchor() {
        let html = to_html("image::photo.jpg[Alt]");
        assert!(!html.contains("<a "));
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
        assert!(html.contains("<pre class=\"highlight\"><code class=\"language-rust\" data-lang=\"rust\">"), "Without highlighter: <pre class=\"highlight\"><code class=\"language-X\" data-lang=\"X\">. Got: {html}");
    }

    #[test]
    fn test_source_block_highlightjs() {
        let html = to_html(":source-highlighter: highlight.js\n\n[source,rust]\n----\nfn main() {}\n----");
        assert!(html.contains("<pre class=\"highlightjs highlight\">"), "highlight.js: pre class. Got: {html}");
        assert!(html.contains("class=\"hljs language-rust\""), "highlight.js: hljs + language class on code. Got: {html}");
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
        assert!(html.contains("<table class=\"linenotable\">"), "linenums should produce linenotable. Got: {html}");
    }

    #[test]
    fn test_source_block_linenums_no_highlighter() {
        let html = to_html("[source,rust,%linenums]\n----\nfn main() {}\n----");
        assert!(html.contains("linenums"), "linenums should work even without highlighter. Got: {html}");
        assert!(html.contains("<table class=\"linenotable\">"), "linenums should produce linenotable. Got: {html}");
    }

    #[test]
    fn test_source_block_linenums_basic() {
        let html = to_html("[source,ruby,%linenums]\n----\nputs \"Hello\"\nx = 42\nputs x\n----");
        assert!(html.contains("<td class=\"linenos\"><pre class=\"linenos\">1\n2\n3</pre></td>"), "should have line numbers 1-3. Got: {html}");
        assert!(html.contains("<td class=\"code\"><pre>puts \"Hello\"\nx = 42\nputs x</pre></td>"), "should have code in td. Got: {html}");
    }

    #[test]
    fn test_source_block_linenums_start() {
        let html = to_html("[source,ruby,%linenums,start=10]\n----\nputs \"Hello\"\nx = 42\nputs x\n----");
        assert!(html.contains("<td class=\"linenos\"><pre class=\"linenos\">10\n11\n12</pre></td>"), "should have line numbers 10-12. Got: {html}");
    }

    #[test]
    fn test_source_block_linenums_with_highlight() {
        let html = to_html("[source,rust,%linenums,highlight=2]\n----\nlet a = 1;\nlet b = 2;\nlet c = 3;\n----");
        assert!(html.contains("<table class=\"linenotable\">"), "should have linenotable. Got: {html}");
        assert!(html.contains("<span class=\"hll\">let b = 2;</span>"), "should have highlight span in code. Got: {html}");
        assert!(html.contains("<td class=\"code\">"), "should have code td. Got: {html}");
    }

    #[test]
    fn test_source_block_linenums_single_line() {
        let html = to_html("[source,ruby,%linenums]\n----\nputs \"hi\"\n----");
        assert!(html.contains("<pre class=\"linenos\">1</pre>"), "single line should have just 1. Got: {html}");
    }

    #[test]
    fn test_source_block_linenums_with_callouts() {
        let html = to_html("[source,ruby,%linenums]\n----\nputs \"Hello\" <1>\nx = 42 <2>\n----");
        assert!(html.contains("<td class=\"code\">"), "should have code td. Got: {html}");
        assert!(html.contains("<b class=\"conum\">(1)</b>"), "should have callout. Got: {html}");
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
    fn test_source_highlight_single_line() {
        let html = to_html("[source,rust,highlight=2]\n----\nlet a = 1;\nlet b = 2;\nlet c = 3;\n----");
        assert!(html.contains("let a = 1;\n<span class=\"hll\">let b = 2;</span>\nlet c = 3;"), "single line highlight. Got: {html}");
    }

    #[test]
    fn test_source_highlight_multiple_lines() {
        let html = to_html("[source,rust,highlight=1;3]\n----\nlet a = 1;\nlet b = 2;\nlet c = 3;\n----");
        assert!(html.contains("<span class=\"hll\">let a = 1;</span>\nlet b = 2;\n<span class=\"hll\">let c = 3;</span>"), "multiple lines highlight. Got: {html}");
    }

    #[test]
    fn test_source_highlight_range() {
        let html = to_html("[source,rust,highlight=2..4]\n----\nline 1\nline 2\nline 3\nline 4\nline 5\n----");
        assert!(html.contains("line 1\n<span class=\"hll\">line 2</span>\n<span class=\"hll\">line 3</span>\n<span class=\"hll\">line 4</span>\nline 5"), "range highlight. Got: {html}");
    }

    #[test]
    fn test_source_no_highlight_no_change() {
        let html = to_html("[source,rust]\n----\nlet a = 1;\nlet b = 2;\n----");
        assert!(!html.contains("hll"), "no highlight attr should produce no hll. Got: {html}");
    }

    #[test]
    fn test_source_highlight_last_line() {
        let html = to_html("[source,rust,highlight=3]\n----\nline 1\nline 2\nline 3\n----");
        assert!(html.contains("<span class=\"hll\">line 3</span></code>"), "last line highlight should close span before </code>. Got: {html}");
    }

    #[test]
    fn test_source_highlight_comma_separator() {
        let html = to_html("[source,rust,highlight=\"1,3\"]\n----\nline 1\nline 2\nline 3\n----");
        assert!(html.contains("<span class=\"hll\">line 1</span>"), "comma-separated highlight. Got: {html}");
        assert!(html.contains("<span class=\"hll\">line 3</span>"), "comma-separated highlight. Got: {html}");
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
        // In embedded mode, document header is not rendered, so doctitle only appears in the body reference
        let html = to_html("= My Title\n\n{doctitle}");
        assert!(html.contains("My Title"), "doctitle should resolve in body. Got: {html}");
        // In standalone mode, it appears in both header and body
        let html = to_html_with_options("= My Title\n\n{doctitle}", HtmlOptions { standalone: true, ..Default::default() });
        assert!(html.contains("<h1>My Title</h1>"), "standalone should have h1. Got: {html}");
        assert!(html.contains("<p>My Title</p>"), "doctitle should resolve in body. Got: {html}");
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

    // --- Markdown compatibility tests ---

    #[test]
    fn test_markdown_heading_h2() {
        let html = to_html("## Title\n\nContent.");
        assert!(html.contains("id=\"_title\""), "should generate id. Got: {html}");
        assert!(html.contains("<h2"), "should render h2. Got: {html}");
        assert!(html.contains("Title"), "should contain title text. Got: {html}");
    }

    #[test]
    fn test_markdown_heading_h3() {
        let html = to_html("### Level Three\n\nContent.");
        assert!(html.contains("id=\"_level_three\""), "should generate id. Got: {html}");
        assert!(html.contains("<h3"), "should render h3. Got: {html}");
    }

    #[test]
    fn test_markdown_heading_document_title() {
        // In embedded mode, document header (h1) is suppressed
        let html = to_html("# Doc Title\n\nBody text.");
        assert!(html.contains("Body text"), "should contain body. Got: {html}");
        // In standalone mode, h1 is rendered
        let html = to_html_with_options("# Doc Title\n\nBody text.", HtmlOptions { standalone: true, ..Default::default() });
        assert!(html.contains("Doc Title"), "should contain title. Got: {html}");
        assert!(html.contains("<h1"), "document title should render h1. Got: {html}");
    }

    #[test]
    fn test_markdown_heading_mixed_with_asciidoc() {
        let html = to_html("= Doc Title\n\n== AsciiDoc Section\n\nPara 1.\n\n### Markdown Section\n\nPara 2.");
        assert!(html.contains("<h2"), "should have h2 for asciidoc section. Got: {html}");
        assert!(html.contains("<h3"), "should have h3 for markdown section. Got: {html}");
        assert!(html.contains("AsciiDoc Section"), "asciidoc heading. Got: {html}");
        assert!(html.contains("Markdown Section"), "markdown heading. Got: {html}");
    }

    #[test]
    fn test_markdown_code_fence_with_language() {
        let html = to_html("```rust\nfn main() {}\n```");
        assert!(html.contains("class=\"language-rust\""), "should have language class. Got: {html}");
        assert!(html.contains("fn main() {}"), "should contain code. Got: {html}");
        assert!(html.contains("<code"), "should have <code> tag. Got: {html}");
        assert!(html.contains("listingblock"), "should have listingblock class. Got: {html}");
    }

    #[test]
    fn test_markdown_code_fence_without_language() {
        let html = to_html("```\nsome code\n```");
        assert!(html.contains("some code"), "should contain code. Got: {html}");
        assert!(html.contains("listingblock"), "should have listingblock class. Got: {html}");
        assert!(html.contains("<pre>"), "should have <pre> tag. Got: {html}");
        assert!(!html.contains("<code"), "listing block should not have <code> tag. Got: {html}");
    }

    #[test]
    fn test_markdown_code_fence_4_backticks() {
        let html = to_html("````python\nprint('hi')\n````");
        assert!(html.contains("class=\"language-python\""), "should have python language. Got: {html}");
        assert!(html.contains("print('hi')"), "should contain code. Got: {html}");
    }

    #[test]
    fn test_markdown_code_fence_nested() {
        // 4-backtick fence can contain 3-backtick fences
        let html = to_html("````\n```\ninner\n```\n````");
        assert!(html.contains("```"), "inner backticks should be verbatim. Got: {html}");
        assert!(html.contains("inner"), "should contain inner text. Got: {html}");
    }

    #[test]
    fn test_markdown_code_fence_unclosed() {
        let html = to_html("```rust\nunclosed code");
        assert!(html.contains("unclosed code"), "should contain code even if unclosed. Got: {html}");
        assert!(html.contains("class=\"language-rust\""), "should still have language. Got: {html}");
    }

    #[test]
    fn test_markdown_code_fence_with_highlighter() {
        let html = to_html(":source-highlighter: highlight.js\n\n```rust\nfn main() {}\n```");
        assert!(html.contains("highlightjs highlight"), "should use highlighter. Got: {html}");
        assert!(html.contains("data-lang=\"rust\""), "should have data-lang. Got: {html}");
        assert!(html.contains("class=\"hljs language-rust\""), "should have hljs + language class. Got: {html}");
    }

    #[test]
    fn test_markdown_code_fence_with_title() {
        let html = to_html(".My Code\n```rust\nfn main() {}\n```");
        assert!(html.contains("My Code"), "should contain block title. Got: {html}");
        assert!(html.contains("class=\"language-rust\""), "should have language class. Got: {html}");
    }

    // === Block style attributes (4.13) ===

    #[test]
    fn test_listing_style_on_paragraph() {
        let html = to_html("[listing]\nsome code here");
        assert!(html.contains("listingblock"), "should have listingblock class. Got: {html}");
        assert!(html.contains("<pre>"), "should have <pre>. Got: {html}");
        assert!(html.contains("some code here"), "should contain text. Got: {html}");
        assert!(!html.contains("<p>"), "should NOT have <p>. Got: {html}");
    }

    #[test]
    fn test_source_style_on_paragraph() {
        let html = to_html("[source,rust]\nfn main() {}");
        assert!(html.contains("language-rust"), "should have language-rust. Got: {html}");
        assert!(html.contains("fn main()"), "should contain code. Got: {html}");
        assert!(!html.contains("<p>"), "should NOT have <p>. Got: {html}");
    }

    #[test]
    fn test_verse_style_on_paragraph() {
        let html = to_html("[verse]\nline one\nline two");
        assert!(html.contains("verseblock"), "should have verseblock class. Got: {html}");
        assert!(html.contains("<pre class=\"content\">"), "should have verse pre. Got: {html}");
        assert!(html.contains("line one"), "should contain text. Got: {html}");
    }

    #[test]
    fn test_quote_style_on_paragraph() {
        let html = to_html("[quote]\nThis is a quote.");
        assert!(html.contains("quoteblock"), "should have quoteblock class. Got: {html}");
        assert!(html.contains("<blockquote>"), "should have blockquote. Got: {html}");
        assert!(html.contains("This is a quote."), "should contain text. Got: {html}");
    }

    #[test]
    fn test_sidebar_style_on_paragraph() {
        let html = to_html("[sidebar]\nSidebar content.");
        assert!(html.contains("sidebarblock"), "should have sidebarblock class. Got: {html}");
        assert!(html.contains("Sidebar content."), "should contain text. Got: {html}");
    }

    #[test]
    fn test_example_style_on_paragraph() {
        let html = to_html("[example]\nExample content.");
        assert!(html.contains("exampleblock"), "should have exampleblock class. Got: {html}");
        assert!(html.contains("Example content."), "should contain text. Got: {html}");
    }

    #[test]
    fn test_listing_style_on_open_block() {
        let html = to_html("[listing]\n--\ncode inside open\n--");
        assert!(html.contains("listingblock"), "should have listingblock class. Got: {html}");
        assert!(html.contains("<pre>"), "should have <pre>. Got: {html}");
        assert!(html.contains("code inside open"), "should contain text. Got: {html}");
    }

    #[test]
    fn test_source_style_on_open_block() {
        let html = to_html("[source,py]\n--\nprint('hello')\n--");
        assert!(html.contains("language-py"), "should have language-py. Got: {html}");
        assert!(html.contains("print("), "should contain code. Got: {html}");
    }

    #[test]
    fn test_quote_style_on_open_block() {
        let html = to_html("[quote]\n--\nQuoted text.\n--");
        assert!(html.contains("quoteblock"), "should have quoteblock class. Got: {html}");
        assert!(html.contains("<blockquote>"), "should have blockquote. Got: {html}");
        assert!(html.contains("Quoted text."), "should contain text. Got: {html}");
    }

    #[test]
    fn test_note_style_on_open_block() {
        let html = to_html("[NOTE]\n--\nNote content.\n--");
        assert!(html.contains("admonitionblock note"), "should have admonition. Got: {html}");
        assert!(html.contains("Note content."), "should contain text. Got: {html}");
    }

    // --- Universal style remapping on non-native delimiters ---

    #[test]
    fn test_source_style_on_example_delimiter() {
        let html = to_html("[source,rust]\n====\nfn main() {}\n====");
        assert!(html.contains("language-rust"), "should have language-rust. Got: {html}");
        assert!(html.contains("fn main()"), "should contain code. Got: {html}");
        assert!(!html.contains("exampleblock"), "should NOT have exampleblock. Got: {html}");
    }

    #[test]
    fn test_listing_style_on_example_delimiter() {
        let html = to_html("[listing]\n====\ncode here\n====");
        assert!(html.contains("listingblock"), "should have listingblock. Got: {html}");
        assert!(html.contains("<pre>"), "should have <pre>. Got: {html}");
        assert!(html.contains("code here"), "should contain text. Got: {html}");
        assert!(!html.contains("exampleblock"), "should NOT have exampleblock. Got: {html}");
    }

    #[test]
    fn test_quote_style_on_listing_delimiter() {
        let html = to_html("[quote]\n----\nQuoted text.\n----");
        assert!(html.contains("quoteblock"), "should have quoteblock. Got: {html}");
        assert!(html.contains("<blockquote>"), "should have blockquote. Got: {html}");
        assert!(html.contains("Quoted text."), "should contain text. Got: {html}");
        assert!(!html.contains("listingblock"), "should NOT have listingblock. Got: {html}");
    }

    #[test]
    fn test_verse_style_on_listing_delimiter() {
        let html = to_html("[verse]\n----\nVerse line one\nVerse line two\n----");
        assert!(html.contains("verseblock"), "should have verseblock. Got: {html}");
        assert!(html.contains("Verse line one"), "should contain text. Got: {html}");
        assert!(!html.contains("listingblock"), "should NOT have listingblock. Got: {html}");
    }

    #[test]
    fn test_note_style_on_listing_delimiter() {
        let html = to_html("[NOTE]\n----\nNote content.\n----");
        assert!(html.contains("admonitionblock note"), "should have admonition. Got: {html}");
        assert!(html.contains("Note content."), "should contain text. Got: {html}");
        assert!(!html.contains("listingblock"), "should NOT have listingblock. Got: {html}");
    }

    #[test]
    fn test_sidebar_style_on_example_delimiter() {
        let html = to_html("[sidebar]\n====\nSidebar content.\n====");
        assert!(html.contains("sidebarblock"), "should have sidebarblock. Got: {html}");
        assert!(html.contains("Sidebar content."), "should contain text. Got: {html}");
        assert!(!html.contains("exampleblock"), "should NOT have exampleblock. Got: {html}");
    }

    // === Nested delimited blocks (4.12) ===

    #[test]
    fn test_nested_example_blocks_different_lengths() {
        let html = to_html("======\nOuter\n====\nInner\n====\nAfter inner\n======");
        // Should have two exampleblocks
        assert_eq!(html.matches("<div class=\"exampleblock\">").count(), 2,
            "should have two example blocks. Got: {html}");
        assert!(html.contains("Outer"), "should contain outer text. Got: {html}");
        assert!(html.contains("Inner"), "should contain inner text. Got: {html}");
        assert!(html.contains("After inner"), "should contain text after inner. Got: {html}");
    }

    #[test]
    fn test_nested_quote_inside_example() {
        let html = to_html("====\nBefore\n____\nQuote text\n____\nAfter\n====");
        assert!(html.contains("<div class=\"exampleblock\">"),
            "should have example block. Got: {html}");
        assert!(html.contains("<div class=\"quoteblock\">"),
            "should have quote block. Got: {html}");
        assert!(html.contains("Quote text"), "should contain quote text. Got: {html}");
    }

    #[test]
    fn test_listing_inside_sidebar() {
        let html = to_html("****\nBefore\n----\ncode here\n----\nAfter\n****");
        assert!(html.contains("<div class=\"sidebarblock\">"),
            "should have sidebar block. Got: {html}");
        assert!(html.contains("<div class=\"listingblock\">"),
            "should have listing block. Got: {html}");
        assert!(html.contains("code here"), "should contain code. Got: {html}");
        assert!(html.contains("After"), "should contain text after listing. Got: {html}");
    }

    #[test]
    fn test_open_block_inside_example() {
        let html = to_html("====\nBefore\n--\nOpen content\n--\nAfter\n====");
        assert!(html.contains("<div class=\"exampleblock\">"),
            "should have example block. Got: {html}");
        assert!(html.contains("<div class=\"openblock\">"),
            "should have open block. Got: {html}");
        assert!(html.contains("Open content"), "should contain open block text. Got: {html}");
    }

    #[test]
    fn test_unclosed_listing_inside_example_parent_delimiter_wins() {
        // Listing block is not closed, but parent example delimiter should take priority
        let html = to_html("====\nBefore\n----\ncode here\n====");
        assert!(html.contains("<div class=\"exampleblock\">"),
            "should have example block. Got: {html}");
        assert!(html.contains("code here"), "should contain code. Got: {html}");
        // The example block should be properly closed
        assert!(html.contains("Before"), "should contain text before listing. Got: {html}");
    }

    #[test]
    fn test_three_level_nesting() {
        let html = to_html("======\nL1\n=====\nL2\n====\nL3\n====\nL2 after\n=====\nL1 after\n======");
        assert_eq!(html.matches("<div class=\"exampleblock\">").count(), 3,
            "should have three example blocks. Got: {html}");
        assert!(html.contains("L1"), "should contain L1 text. Got: {html}");
        assert!(html.contains("L2"), "should contain L2 text. Got: {html}");
        assert!(html.contains("L3"), "should contain L3 text. Got: {html}");
    }

    #[test]
    fn test_source_block_inside_sidebar() {
        let html = to_html("****\n[source,rust]\n----\nfn main() {}\n----\n****");
        assert!(html.contains("<div class=\"sidebarblock\">"),
            "should have sidebar block. Got: {html}");
        assert!(html.contains("<code"), "should have code element. Got: {html}");
        assert!(html.contains("fn main() {}"), "should contain source code. Got: {html}");
    }

    #[test]
    fn test_env_attribute_existing_var() {
        // PATH is set on all platforms
        let html = to_html("Value: {env-PATH}");
        assert!(!html.contains("{env-PATH}"), "env-PATH should be resolved, not literal. Got: {html}");
        assert!(html.contains("Value: "), "should contain prefix. Got: {html}");
    }

    #[test]
    fn test_env_attribute_missing_var() {
        let html = to_html("Value: {env-ADOC_PARSER_TEST_VAR_12345}");
        assert!(html.contains("{env-ADOC_PARSER_TEST_VAR_12345}"),
            "missing env var should render as literal. Got: {html}");
    }

    #[test]
    fn test_env_attribute_missing_var_with_fallback() {
        let html = to_html("Value: {env-ADOC_PARSER_TEST_VAR_12345!fallback}");
        assert!(html.contains("Value: fallback"),
            "missing env var with fallback should use fallback. Got: {html}");
    }

    #[test]
    fn test_custom_inline_macro_with_attrs() {
        let html = to_html("chart:sales[Q1,Q2]");
        assert!(html.contains("<span class=\"custom-macro macro-chart\">Q1,Q2</span>"),
            "custom inline macro should render. Got: {html}");
    }

    #[test]
    fn test_custom_block_macro_with_attrs() {
        let html = to_html("chart::sales-data[type=bar]");
        assert!(html.contains("<div class=\"custom-macro macro-chart\">"),
            "custom block macro should render div. Got: {html}");
        assert!(html.contains("type=bar"),
            "custom block macro should contain attrs text. Got: {html}");
        assert!(html.contains("</div>"),
            "custom block macro should close div. Got: {html}");
    }

    #[test]
    fn test_custom_inline_macro_empty_attrs() {
        let html = to_html("widget:component[]");
        assert!(html.contains("<span class=\"custom-macro macro-widget\"></span>"),
            "custom inline macro with empty attrs should render empty span. Got: {html}");
    }

    #[test]
    fn test_kbd_not_captured_as_custom() {
        let html = to_html("kbd:[Ctrl+S]");
        assert!(html.contains("<kbd>"),
            "kbd should remain a built-in macro, not custom. Got: {html}");
        assert!(!html.contains("custom-macro"),
            "kbd should not be treated as custom macro. Got: {html}");
    }

    #[test]
    fn test_block_image_not_captured_as_custom() {
        let html = to_html("image::photo.jpg[alt]");
        assert!(html.contains("<img"),
            "image:: should remain a built-in block image. Got: {html}");
        assert!(!html.contains("custom-macro"),
            "image:: should not be treated as custom macro. Got: {html}");
    }

    #[test]
    fn test_custom_macro_with_hyphen_underscore_name() {
        let html = to_html("my-custom_macro:target[attrs]");
        assert!(html.contains("<span class=\"custom-macro macro-my-custom_macro\">attrs</span>"),
            "macro names with hyphen/underscore should work. Got: {html}");
    }

    #[test]
    fn test_docinfo_head() {
        let html = to_html_with_options("Hello world", HtmlOptions {
            docinfo_head: Some("<meta name=\"test\" content=\"value\">".to_string()),
            ..Default::default()
        });
        assert!(html.starts_with("<meta name=\"test\" content=\"value\">\n"),
            "docinfo head should be prepended. Got: {html}");
        assert!(html.contains("<p>Hello world</p>"),
            "content should follow head. Got: {html}");
    }

    #[test]
    fn test_docinfo_footer() {
        let html = to_html_with_options("Hello world", HtmlOptions {
            docinfo_footer: Some("<script src=\"app.js\"></script>".to_string()),
            ..Default::default()
        });
        assert!(html.ends_with("\n<script src=\"app.js\"></script>"),
            "docinfo footer should be appended. Got: {html}");
        assert!(html.contains("<p>Hello world</p>"),
            "content should precede footer. Got: {html}");
    }

    #[test]
    fn test_docinfo_head_and_footer() {
        let html = to_html_with_options("Hello world", HtmlOptions {
            docinfo_head: Some("<meta name=\"x\">".to_string()),
            docinfo_footer: Some("<script></script>".to_string()),
            ..Default::default()
        });
        assert!(html.starts_with("<meta name=\"x\">\n"),
            "head should be first. Got: {html}");
        assert!(html.ends_with("\n<script></script>"),
            "footer should be last. Got: {html}");
    }

    #[test]
    fn test_docinfo_default_options_same_as_to_html() {
        let input = "= Title\n\nHello world";
        let html_default = to_html(input);
        let html_options = to_html_with_options(input, HtmlOptions::default());
        assert_eq!(html_default, html_options,
            "default options should produce identical output");
    }

    #[test]
    fn test_docinfo_head_before_toc() {
        let input = "= Title\n:toc:\n\n== Section 1\n\nContent";
        let html = to_html_with_options(input, HtmlOptions {
            docinfo_head: Some("<meta name=\"toc-test\">".to_string()),
            ..Default::default()
        });
        let head_pos = html.find("<meta name=\"toc-test\">").unwrap();
        let toc_pos = html.find("<div id=\"toc\"").unwrap();
        assert!(head_pos < toc_pos,
            "head should appear before TOC. Got: {html}");
    }

    #[test]
    fn test_docinfo_empty_content_no_extra_newlines() {
        let input = "Hello world";
        let html_empty = to_html_with_options(input, HtmlOptions {
            docinfo_head: Some(String::new()),
            docinfo_footer: Some(String::new()),
            ..Default::default()
        });
        let html_none = to_html(input);
        assert_eq!(html_empty, html_none,
            "empty docinfo should not add extra content");
    }

    #[test]
    fn test_manpage_title_suffix() {
        let input = "= command(1)\n:doctype: manpage\n\n== SYNOPSIS\n\ntext";
        // In standalone mode, h1 is rendered with manpage suffix
        let html = to_html_with_options(input, HtmlOptions { standalone: true, ..Default::default() });
        assert!(html.contains("command(1) Manual Page</h1>"),
            "manpage title should have ' Manual Page' suffix. Got: {html}");
    }

    #[test]
    fn test_manpage_auto_attrs() {
        let input = "= command(1)\n:doctype: manpage\n\nmanvol={manvolnum} mantitle={mantitle}";
        let html = to_html(input);
        assert!(html.contains("manvol=1"), "manvolnum should be '1'. Got: {html}");
        assert!(html.contains("mantitle=command"), "mantitle should be 'command'. Got: {html}");
    }

    #[test]
    fn test_manpage_name_extraction() {
        let input = "= command(1)\n:doctype: manpage\n\n== NAME\n\nmycmd - manage things\n\n== SYNOPSIS\n\nname={manname} purpose={manpurpose}";
        let html = to_html(input);
        assert!(html.contains("name=mycmd"), "manname should be 'mycmd'. Got: {html}");
        assert!(html.contains("purpose=manage things"), "manpurpose should be 'manage things'. Got: {html}");
    }

    #[test]
    fn test_no_manpage_suffix_for_article() {
        let input = "= Title\n\ntext";
        // In standalone mode, verify article title doesn't get manpage suffix
        let html = to_html_with_options(input, HtmlOptions { standalone: true, ..Default::default() });
        assert!(html.contains("<h1>Title</h1>"),
            "article title should not have ' Manual Page'. Got: {html}");
        assert!(!html.contains("Manual Page"),
            "article should not contain 'Manual Page'. Got: {html}");
    }

    #[test]
    fn test_manpage_doctype_attr_ref() {
        let input = "= command(1)\n:doctype: manpage\n\ntype={doctype}";
        let html = to_html(input);
        assert!(html.contains("type=manpage"), "doctype should be 'manpage'. Got: {html}");
    }

    #[test]
    fn test_book_part_rendering() {
        let input = "= Book Title\n:doctype: book\n\n= Part One\n\npart intro\n\n== Chapter 1\n\ntext";
        let html = to_html(input);
        assert!(html.contains("class=\"sect0\""), "part title should have class=\"sect0\". Got: {html}");
        assert!(html.contains("<h1 id=\"_part_one\" class=\"sect0\">Part One</h1>"),
            "part should render as <h1> with sect0 class. Got: {html}");
        // Part should NOT be wrapped in <div class="sect1">
        assert!(!html.contains("<div class=\"sect1\">\n<h1 id=\"_part_one\""),
            "part should not have div wrapper. Got: {html}");
    }

    #[test]
    fn test_book_chapter_rendering() {
        let input = "= Book Title\n:doctype: book\n\n= Part One\n\n== Chapter 1\n\ntext";
        let html = to_html(input);
        assert!(html.contains("<div class=\"sect1\">"), "chapter should have div wrapper. Got: {html}");
        assert!(html.contains("<h2 id=\"_chapter_1\">Chapter 1</h2>"),
            "chapter should render as <h2>. Got: {html}");
    }

    #[test]
    fn test_article_no_part_behavior() {
        let input = "= Title\n\n== Section\n\ntext";
        let html = to_html(input);
        assert!(html.contains("<div class=\"sect1\">"), "article sections should have div wrapper. Got: {html}");
    }

    #[test]
    fn test_book_special_section_not_part() {
        let input = "= Book Title\n:doctype: book\n\n[appendix]\n= Appendix A\n\ntext";
        let html = to_html(input);
        // Appendix at level 1 in book should NOT be treated as a part
        assert!(!html.contains("class=\"sect0\""),
            "appendix should not have sect0 class. Got: {html}");
        assert!(html.contains("Appendix A:"),
            "appendix should have caption. Got: {html}");
    }

    #[test]
    fn test_book_multiple_parts() {
        let input = "= Book Title\n:doctype: book\n\n= Part 1\n\n== Ch1\n\ntext1\n\n= Part 2\n\n== Ch2\n\ntext2";
        let html = to_html(input);
        assert!(html.contains("<h1 id=\"_part_1\" class=\"sect0\">Part 1</h1>"),
            "first part should have sect0. Got: {html}");
        assert!(html.contains("<h1 id=\"_part_2\" class=\"sect0\">Part 2</h1>"),
            "second part should have sect0. Got: {html}");
    }

    #[test]
    fn test_book_doctype_attr_ref() {
        let input = "= Book Title\n:doctype: book\n\ntype={doctype}";
        let html = to_html(input);
        assert!(html.contains("type=book"), "doctype should be 'book'. Got: {html}");
    }

}
