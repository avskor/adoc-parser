//! HTML renderer for the `adoc_parser` AsciiDoc event stream.
//!
//! The entry point is [`to_html`], which parses AsciiDoc source and renders a
//! standalone HTML document. Use [`push_html`] to render a parser event stream
//! into an existing buffer, or the `*_with_options` variants for embedded output.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use adoc_parser::{CellStyle, CowStr, Event, HAlign, Tag, TagEnd, AdmonitionKind, DelimitedBlockKind, SubstitutionSet, VAlign};
use adoc_render_core::{
    Author, AuthorRegistry, CaptionCounters, CaptionKind, CaptionPrefix, FootnoteRegistry,
    RefText, Revision, SectionNumberer, TocBuilder, TocEntry, TocStep, XrefResolver,
    DEFAULT_TOC_TITLE,
};

mod blocks;
mod escape;
mod events;
mod finish;
mod inline;
mod media;
#[cfg(test)]
mod tests;

use blocks::*;
use escape::*;
use media::*;

#[derive(Default, Clone)]
pub struct HtmlOptions {
    pub docinfo_head: Option<String>,
    pub docinfo_footer: Option<String>,
    pub standalone: bool,
    pub last_updated: Option<String>,
    pub attributes: HashMap<String, String>,
}

/// Render a parser event stream into the `String` buffer `s`.
///
/// ```
/// use adoc_parser::Parser;
/// let mut buf = String::new();
/// adoc_html::push_html(&mut buf, Parser::new("Hello"));
/// assert!(buf.contains("Hello"));
/// ```
pub fn push_html<'a>(s: &mut String, iter: impl Iterator<Item = Event<'a>>) {
    let mut renderer = HtmlRenderer::new();
    renderer.run(s, iter);
}

/// Parse AsciiDoc `input` and render it as a standalone HTML document.
///
/// ```
/// let html = adoc_html::to_html("Hello *world*");
/// assert!(html.contains("<strong>world</strong>"));
/// ```
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
    footnote_registry: FootnoteRegistry,
    toc_builder: TocBuilder,
    toc_insert_position: Option<usize>,
    toc_levels: u8,
    toc_position: String,
    toc_title: String,
    toc_auto_seen: bool,
    in_section_title: bool,
    current_toc_entry: Option<TocEntry>,
    pending_block_meta: Option<BlockMeta>,
    para_hardbreaks: bool,
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
    caption_counters: CaptionCounters,
    block_title_output_start: Option<usize>,
    block_title_inner_html: Option<String>,
    dlist_stack: Vec<DlistStyle>,
    dd_output_start: Option<usize>,
    hdlist_in_term_group: bool,
    has_document_title: bool,
    capturing_doctitle: bool,
    doctitle_buf: String,
    preamble_start: Option<usize>,
    pending_section_caption: Option<String>,
    sectnums: bool,
    sectanchors: bool,
    showtitle: bool,
    nofooter: bool,
    doctitle_h1_end: Option<usize>,
    section_numberer: SectionNumberer,
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
    /// Internal/inter-document xref link text, resolved in `finish()`. Each entry
    /// is `(placeholder, fallback, is_internal)`. `is_internal` selects the
    /// unresolved-fallback shape: bracketed `[id]` for internal anchor refs
    /// (Asciidoctor's default xreflabel), raw path for inter-document refs.
    xref_placeholders: Vec<(String, String, bool)>,
    /// Internal xref href ids, resolved in `finish()`: a placeholder paired with
    /// the raw target. A target matching a section title (natural cross
    /// reference) resolves to that section's id (`<<Substitutions>>` →
    /// `#_substitutions`); unmatched targets stay literal.
    xref_href_placeholders: Vec<(String, String)>,
    /// Block id -> rendered title HTML, for resolving empty `<<id>>` to a block title.
    block_ref_titles: Vec<(String, String)>,
    /// Bibliography anchor id -> rendered reftext (`[pp]` / `[gang]`), for
    /// resolving `<<id>>` to a bibliography entry to its bracketed label.
    bibliography_reftexts: Vec<(String, String)>,
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
    authors: AuthorRegistry,
    revision: Option<Revision>,
}

impl HtmlRenderer {
    pub(crate) fn new() -> Self {
        Self {
            tag_stack: Vec::new(),
            in_source_block: false,
            subs_stack: Vec::new(),
            pending_subs: None,
            document_attrs: HashMap::from([
                ("backend".to_string(), "html5".to_string()),
                ("doctype".to_string(), "article".to_string()),
                ("table-caption".to_string(), "Table".to_string()),
                ("figure-caption".to_string(), "Figure".to_string()),
            ]),
            delimited_block_stack: Vec::new(),
            footnote_registry: FootnoteRegistry::new(),
            toc_builder: TocBuilder::new(),
            toc_insert_position: None,
            toc_levels: 2,
            toc_position: String::new(),
            toc_title: String::from(DEFAULT_TOC_TITLE),
            toc_auto_seen: false,
            in_section_title: false,
            current_toc_entry: None,
            pending_block_meta: None,
            para_hardbreaks: false,
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
            caption_counters: CaptionCounters::new(),
            block_title_output_start: None,
            block_title_inner_html: None,
            dlist_stack: Vec::new(),
            dd_output_start: None,
            hdlist_in_term_group: false,
            has_document_title: false,
            capturing_doctitle: false,
            doctitle_buf: String::new(),
            preamble_start: None,
            pending_section_caption: None,
            sectnums: false,
            sectanchors: false,
            showtitle: false,
            nofooter: false,
            doctitle_h1_end: None,
            section_numberer: SectionNumberer::new(),
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
            xref_href_placeholders: Vec::new(),
            block_ref_titles: Vec::new(),
            bibliography_reftexts: Vec::new(),
            in_header: false,
            li_p_open: Vec::new(),
            li_para_count: Vec::new(),
            linenums_active: false,
            linenums_start: 1,
            source_code_buffer: None,
            header_suppress_start: None,
            quote_attribution: None,
            quote_citetitle: None,
            authors: AuthorRegistry::new(),
            revision: None,
        }
    }

    pub(crate) fn new_with_options(options: HtmlOptions) -> Self {
        let mut renderer = Self {
            docinfo_head: options.docinfo_head,
            docinfo_footer: options.docinfo_footer,
            standalone: options.standalone,
            last_updated: options.last_updated,
            ..Self::new()
        };
        for (name, value) in &options.attributes {
            renderer.apply_attribute(name, value);
        }
        renderer
    }

    pub(crate) fn apply_attribute(&mut self, name: &str, value: &str) {
        match name {
            "toclevels" => {
                if let Ok(n) = value.parse::<u8>() {
                    self.toc_levels = n;
                }
            }
            "toc-title" => self.toc_title = value.to_string(),
            "toc" => self.toc_position = value.to_string(),
            "sectnums" => self.sectnums = true,
            "!sectnums" | "sectnums!" => self.sectnums = false,
            "sectanchors" => self.sectanchors = true,
            "!sectanchors" | "sectanchors!" => self.sectanchors = false,
            "showtitle" => self.showtitle = true,
            "!showtitle" | "showtitle!" => self.showtitle = false,
            "nofooter" => self.nofooter = true,
            "!nofooter" | "nofooter!" => self.nofooter = false,
            _ => {}
        }
        if let Some(stripped) = name.strip_prefix('!') {
            self.document_attrs.remove(stripped);
        } else if let Some(stripped) = name.strip_suffix('!') {
            self.document_attrs.remove(stripped);
        } else {
            self.document_attrs.insert(name.to_string(), value.to_string());
        }
    }

    pub(crate) fn current_subs(&self) -> SubstitutionSet {
        self.subs_stack.last().copied().unwrap_or(SubstitutionSet::NORMAL)
    }

    pub(crate) fn default_subs_for_delimited(kind: DelimitedBlockKind) -> SubstitutionSet {
        match kind {
            DelimitedBlockKind::Listing | DelimitedBlockKind::Literal => SubstitutionSet::VERBATIM,
            DelimitedBlockKind::Passthrough | DelimitedBlockKind::Comment => SubstitutionSet::NONE,
            _ => SubstitutionSet::NORMAL,
        }
    }

    pub(crate) fn run<'a>(&mut self, output: &mut String, iter: impl Iterator<Item = Event<'a>>) {
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
    pub(crate) fn render_inline_value(&mut self, output: &mut String, value: &str) {
        // Resolved attribute values are substituted as part of the current block's
        // pipeline: in a verbatim block (`[subs="+attributes"]` listing/literal) only
        // specialchars run, so an apostrophe in the value stays straight rather than
        // being curled by replacements. At top level / in normal paragraphs this is
        // NORMAL, so behavior there is unchanged.
        let experimental = self.document_attrs.contains_key("experimental");
        let events = adoc_parser::InlineParser::parse_str_with_subs_experimental(
            value,
            self.current_subs(),
            experimental,
        );
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
}
