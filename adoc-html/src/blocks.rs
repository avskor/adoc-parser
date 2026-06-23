//! Block-level rendering: sections, paragraphs, lists, tables, delimited
//! blocks, block titles/captions and block-metadata helpers.

use crate::*;

pub(crate) fn parse_manpage_title(title: &str) -> Option<(String, String)> {
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

pub(crate) fn section_level_to_h(level: u8) -> u8 {
    // AsciiDoc: = (doc title/h1), == (h2), === (h3), etc.
    // level 0 = doc title = h1, level 2 = h2, level 3 = h3...
    if level == 0 {
        1
    } else {
        level
    }
}

impl HtmlRenderer {
    pub(crate) fn start_admonition(&mut self, output: &mut String, kind: &AdmonitionKind, block: bool, meta: &Option<BlockMeta>) {
        self.admonition_block_stack.push(block);
        let label = match kind {
            AdmonitionKind::Note => "Note",
            AdmonitionKind::Tip => "Tip",
            AdmonitionKind::Important => "Important",
            AdmonitionKind::Warning => "Warning",
            AdmonitionKind::Caution => "Caution",
        };
        let adm_class = format!("admonitionblock {}", label.to_lowercase());
        // A block-level [caption="…"] overrides the displayed label text (but not the
        // admonitionblock class or icon kind, which always track the admonition type).
        let caption = meta
            .as_ref()
            .and_then(|m| m.named.iter().find(|(k, _)| k == "caption").map(|(_, v)| v.as_ref()));
        output.push_str("<div");
        Self::write_meta_attrs(output, meta, &adm_class);
        output.push_str(">\n<table>\n<tr>\n<td class=\"icon\">\n");
        match self.document_attrs.get("icons").map(|v| v.as_str()) {
            Some("font") => {
                let icon_name = label.to_lowercase();
                output.push_str("<i class=\"fa icon-");
                output.push_str(&icon_name);
                output.push_str("\" title=\"");
                match caption {
                    Some(c) => html_escape(output, c),
                    None => output.push_str(label),
                }
                output.push_str("\"></i>\n");
            }
            // Any other set value (including empty) selects image-based icons:
            // {iconsdir}/{name}.{icontype}. Asciidoctor derives iconsdir at init
            // time, before the header is parsed, so a header imagesdir does not
            // affect the ./images/icons default.
            Some(_) => {
                let iconsdir = self
                    .document_attrs
                    .get("iconsdir")
                    .map(|s| s.as_str())
                    .unwrap_or("./images/icons");
                let icontype = self
                    .document_attrs
                    .get("icontype")
                    .map(|s| s.as_str())
                    .unwrap_or("png");
                output.push_str("<img src=\"");
                html_escape(output, iconsdir);
                output.push('/');
                output.push_str(&label.to_lowercase());
                output.push('.');
                html_escape(output, icontype);
                output.push_str("\" alt=\"");
                match caption {
                    Some(c) => html_escape(output, c),
                    None => output.push_str(label),
                }
                output.push_str("\">\n");
            }
            None => {
                output.push_str("<div class=\"title\">");
                match caption {
                    Some(c) => html_escape(output, c),
                    None => output.push_str(label),
                }
                output.push_str("</div>\n");
            }
        }
        output.push_str("</td>\n<td class=\"content\">\n");
        self.emit_pending_block_title(output);
    }

    pub(crate) fn start_table(&mut self, output: &mut String, meta: &Option<BlockMeta>) {
        // Collect extra CSS classes from options/named attrs
        let has_autowidth = meta.as_ref().is_some_and(|m| m.options.iter().any(|o| o == "autowidth"));
        let stripes_value = meta.as_ref().and_then(|m| m.named.iter().find(|(k, _)| k == "stripes").map(|(_, v)| v.clone()));
        let width_value = meta.as_ref().and_then(|m| m.named.iter().find(|(k, _)| k == "width").map(|(_, v)| v.clone()));

        // Percentage table width (asciidoctor `tablepcwidth`): Ruby to_i on the
        // raw value (leading integer, 0 if none); out-of-range values fall back
        // to 100 except a literal "0"/"0%" (probe-verified: width=50% → inline
        // style, width=100%/width=150/width=abc → stretch, width=0 → width: 0%).
        let tablepcwidth = match width_value {
            Some(ref raw) => {
                let t = raw.trim_start();
                let (sign, body) = match t.strip_prefix('-') {
                    Some(rest) => (-1i64, rest),
                    None => (1i64, t.strip_prefix('+').unwrap_or(t)),
                };
                let digits: &str = &body[..body
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(body.len())];
                let intval = sign * digits.parse::<i64>().unwrap_or(0);
                if !(1..=100).contains(&intval) && !(intval == 0 && (raw == "0" || raw == "0%")) {
                    100
                } else {
                    intval
                }
            }
            None => 100,
        };

        // Base Asciidoctor table classes (html5.rb convert_table:859-860):
        // `frame-{frame} grid-{grid}` where frame defaults to "all" with
        // "topbot" aliased to "ends", grid defaults to "all". Both read the
        // table's named attribute first, then fall back to the document
        // attribute (table-frame/table-grid), then the default. The value is
        // emitted verbatim — asciidoctor performs no validation here.
        let frame_value = meta.as_ref()
            .and_then(|m| m.named.iter().find(|(k, _)| k == "frame").map(|(_, v)| v.clone()))
            .or_else(|| self.document_attrs.get("table-frame").cloned());
        let grid_value = meta.as_ref()
            .and_then(|m| m.named.iter().find(|(k, _)| k == "grid").map(|(_, v)| v.clone()))
            .or_else(|| self.document_attrs.get("table-grid").cloned());
        let frame = match frame_value.as_deref().unwrap_or("all") {
            "topbot" => "ends",
            other => other,
        };
        let grid = grid_value.as_deref().unwrap_or("all");
        let mut classes = format!("tableblock frame-{frame} grid-{grid}");
        // Class order mirrors asciidoctor html5.rb convert_table: `stripes-{stripes}`
        // is appended right after `grid-{grid}`, BEFORE the width class
        // (`stretch`/`fit-content`); roles (via write_meta_attrs) come last.
        if let Some(ref sv) = stripes_value {
            classes.push_str(" stripes-");
            classes.push_str(sv);
        }
        let mut width_style = None;
        if tablepcwidth == 100 {
            // An explicit width suppresses fit-content even with %autowidth.
            if has_autowidth && width_value.is_none() {
                classes.push_str(" fit-content");
            } else {
                classes.push_str(" stretch");
            }
        } else {
            width_style = Some(format!("width: {}%;", tablepcwidth));
        }

        // Extract cols spec for colgroup generation
        let cols_value = meta.as_ref().and_then(|m| m.named.iter()
            .find(|(k, _)| k == "cols").map(|(_, v)| v.clone()));

        output.push_str("<table");
        Self::write_meta_attrs(output, meta, &classes);
        if let Some(ref ws) = width_style {
            output.push_str(" style=\"");
            output.push_str(ws);
            output.push('"');
        }
        output.push_str(">\n");

        // Caption must come before colgroup per HTML5 spec
        let title_html = self.block_title_inner_html.take();
        if let Some(title) = title_html {
            output.push_str("<caption class=\"title\">");
            let label = self.document_attrs.get("table-caption").cloned();
            let prefix = self.render_caption_prefix(meta, label.as_deref(), CaptionKind::Table);
            output.push_str(&prefix);
            output.push_str(&title);
            output.push_str("</caption>\n");
            self.register_block_ref(meta, prefix, title);
        }

        // Emit <colgroup> based on cols spec
        if let Some(ref cols_str) = cols_value {
            let col_widths = Self::parse_col_widths(cols_str);
            if !col_widths.is_empty() {
                output.push_str("<colgroup>\n");
                if has_autowidth {
                    for _ in &col_widths {
                        output.push_str("<col>\n");
                    }
                } else {
                    for w in &col_widths {
                        output.push_str("<col style=\"width: ");
                        output.push_str(&Self::format_col_width(*w));
                        output.push_str(";\">\n");
                    }
                }
                output.push_str("</colgroup>\n");
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_table_cell(&mut self, output: &mut String, is_header: bool, colspan: &u8, rowspan: &u8, style: &CellStyle, halign: &HAlign, valign: &VAlign) {
        self.cell_style_stack.push(*style);
        let cell_class = Self::tableblock_cell_class(halign, valign);
        // A header-row cell, or a body cell in a header (`h`) column, uses <th>.
        // The body header cell still gets the <p class="tableblock"> wrapper below.
        let use_th = is_header || matches!(style, CellStyle::Header);
        output.push_str(if use_th { "<th class=\"" } else { "<td class=\"" });
        output.push_str(&cell_class);
        output.push('"');
        if *colspan > 1 {
            output.push_str(&format!(" colspan=\"{}\"", colspan));
        }
        if *rowspan > 1 {
            output.push_str(&format!(" rowspan=\"{}\"", rowspan));
        }
        output.push('>');
        let mut p_start = None;
        if is_header {
            match style {
                CellStyle::Emphasis => output.push_str("<em>"),
                CellStyle::Strong => output.push_str("<strong>"),
                CellStyle::Monospace | CellStyle::Literal => output.push_str("<code>"),
                _ => {}
            }
        } else {
            // Record the position right after the paragraph wrapper for the
            // styled (e/s/m) and default cells so TagEnd::TableCell can detect
            // an empty cell (nothing written after the wrapper) and roll the
            // wrapper back to a bare <td></td>, matching asciidoctor's
            // `Cell#content` (empty text => [] => no paragraph). Literal and
            // AsciiDoc cells keep their wrapper even when empty (asciidoctor
            // emits <pre></pre>/<div class="content"></div>), so no marker.
            match style {
                CellStyle::Emphasis => {
                    output.push_str("<p class=\"tableblock\"><em>");
                    p_start = Some(output.len());
                }
                CellStyle::Strong => {
                    output.push_str("<p class=\"tableblock\"><strong>");
                    p_start = Some(output.len());
                }
                CellStyle::Monospace => {
                    output.push_str("<p class=\"tableblock\"><code>");
                    p_start = Some(output.len());
                }
                CellStyle::Literal => output.push_str("<div class=\"literal\"><pre>"),
                CellStyle::AsciiDoc => {
                    // Nested-document cell: content is captured raw and block-
                    // parsed on TagEnd::TableCell, inside a content wrapper.
                    output.push_str("<div class=\"content\">");
                    self.acell_capture.push(String::new());
                }
                _ => {
                    output.push_str("<p class=\"tableblock\">");
                    p_start = Some(output.len());
                }
            }
        }
        self.cell_p_start_stack.push(p_start);
    }

    pub(crate) fn start_unordered_list(&mut self, output: &mut String, has_checklist: &bool, meta: &Option<BlockMeta>) {
        let interactive = meta.as_ref()
            .is_some_and(|m| m.options.iter().any(|o| o == "interactive"));
        self.interactive_ulist_stack.push(interactive);
        // Bibliography is derived from the enclosing section style and only
        // applies to the top-level list of that section, never a nested one.
        let is_bibliography = !self.is_inside_list_item()
            && self.section_style_stack.last().and_then(|s| s.as_deref()) == Some("bibliography");

        // The explicit block style (`[square]`/`[circle]`/`[disc]`/`[none]`/
        // `[no-bullet]`, or any keyword) is the marker class. Asciidoctor puts
        // it — and id/roles — on the wrapper div (`ulist {style} {roles}`, via
        // write_meta_attrs), but ONLY the style on the `<ul>` (roles/id stay on
        // the div). checklist/bibliography are derived classes filling the same
        // `<ul>` slot when no explicit style is set. Both top-level and nested
        // styled lists carry the class (probe /tmp/p_ov marker-override).
        let style = meta.as_ref().and_then(|m| m.style.as_deref());
        let (base_class, ul_class) = if *has_checklist {
            ("ulist checklist", Some("checklist"))
        } else if is_bibliography {
            ("ulist bibliography", Some("bibliography"))
        } else {
            ("ulist", style)
        };

        output.push_str("<div");
        Self::write_meta_attrs(output, meta, base_class);
        output.push_str(">\n");
        self.emit_pending_block_title(output);
        output.push_str("<ul");
        if let Some(class) = ul_class {
            write_attr(output, "class", class);
        }
        output.push_str(">\n");
    }

    pub(crate) fn start_ordered_list(&mut self, output: &mut String, start: &Option<u32>, reversed: &bool, depth: u8, meta: &Option<BlockMeta>) {
        // Implicit style comes from the marker's dot count (Asciidoctor):
        // `.` arabic, `..` loweralpha, `...` lowerroman, … — even when
        // the list is nested inside another list type.
        let style_name = meta.as_ref()
            .and_then(|m| m.style.as_deref())
            .unwrap_or(match depth {
                0 | 1 => "arabic",
                2 => "loweralpha",
                3 => "lowerroman",
                4 => "upperalpha",
                _ => "upperroman",
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
            output.push_str("<div");
            write_attr(output, "class", &wrapper_class);
            output.push_str(">\n");
        }
        self.emit_pending_block_title(output);
        output.push_str("<ol");
        write_attr(output, "class", style_name);
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

    pub(crate) fn start_description_list(&mut self, output: &mut String, meta: &Option<BlockMeta>) {
        let style_str = meta.as_ref().and_then(|m| m.style.as_deref());
        let dlist_style = match style_str {
            Some("horizontal") => DlistStyle::Horizontal,
            Some("qanda") => DlistStyle::Qanda,
            Some(_) => DlistStyle::Styled,
            None => DlistStyle::Normal,
        };
        self.dlist_stack.push(dlist_style);
        let mut adjusted_meta = meta.clone();
        if let Some(ref mut m) = adjusted_meta
            && dlist_style != DlistStyle::Styled
        {
            // A custom style joins the wrapper class (`dlist glossary`);
            // the dedicated layouts consume the style instead.
            m.style = None;
        }
        match dlist_style {
            DlistStyle::Horizontal => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &adjusted_meta, "hdlist");
                output.push_str(">\n");
                self.emit_pending_block_title(output);
                output.push_str("<table>\n");
                // labelwidth/itemwidth → <colgroup> with two <col> elements
                // (Asciidoctor html5 convert_dlist horizontal). The colgroup is
                // emitted iff either attribute is present; each <col> gets a
                // width style only when its own attribute is set, otherwise it is
                // bare. A trailing `%` in the value is dropped (`.chomp '%'`).
                let named_attr = |key: &str| {
                    meta.as_ref()
                        .and_then(|m| m.named.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()))
                };
                let labelwidth = named_attr("labelwidth");
                let itemwidth = named_attr("itemwidth");
                if labelwidth.is_some() || itemwidth.is_some() {
                    output.push_str("<colgroup>\n");
                    for w in [labelwidth, itemwidth] {
                        match w {
                            Some(v) => {
                                output.push_str("<col style=\"width: ");
                                output.push_str(v.strip_suffix('%').unwrap_or(v));
                                output.push_str("%;\">\n");
                            }
                            None => output.push_str("<col>\n"),
                        }
                    }
                    output.push_str("</colgroup>\n");
                }
            }
            DlistStyle::Qanda => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &adjusted_meta, "qlist qanda");
                output.push_str(">\n");
                self.emit_pending_block_title(output);
                output.push_str("<ol>\n");
            }
            DlistStyle::Normal | DlistStyle::Styled => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &adjusted_meta, "dlist");
                output.push_str(">\n");
                self.emit_pending_block_title(output);
                output.push_str("<dl>\n");
            }
        }
    }

    pub(crate) fn start_section_title(&mut self, output: &mut String, level: &u8, id: &CowStr<'_>) {
        // Every body section enters the TOC registry — including level 1
        // (book parts / body sect0), which Asciidoctor shows at TOC depth 1.
        if !self.in_header {
            self.in_section_title = true;
            self.current_toc_entry = Some(TocEntry {
                level: *level,
                // The enclosing Section is already on the stack (pushed by
                // start_section_div), so the open-section count IS the tree
                // depth of this section.
                depth: self.sect0_stack.len() as u8,
                id: id.to_string(),
                title: String::new(),
            });
        }
        let h = section_level_to_h(*level);
        let is_sect0 = *level == 1 && self.sect0_stack.last() == Some(&true);
        output.push_str("<h");
        output.push_str(&h.to_string());
        if !self.in_header {
            output.push_str(" id=\"");
            html_escape(output, id);
            output.push('"');
        }
        if is_sect0 {
            output.push_str(" class=\"sect0\"");
        }
        output.push('>');
        if self.sectanchors && !self.in_header {
            output.push_str("<a class=\"anchor\" href=\"#");
            html_escape(output, id);
            output.push_str("\"></a>");
        }
        if self.sectnums
            && self.pending_section_caption.is_none()
            // A descendant of a non-numbered special section (preface, colophon,
            // …) inherits its unnumbered status and emits no number — and must
            // not bump the counter, so the test short-circuits before
            // `number_prefix`.
            && self.section_unnumbered_stack.last() != Some(&true)
            // `sectnumlevels` caps numbering depth (Asciidoctor level =
            // display level − 1 ≤ sectnumlevels). Deeper sections show no
            // number and don't bump the counter.
            && (*level as u16) <= self.sectnumlevels as u16 + 1
            && let Some(prefix) = self.section_numberer.number_prefix(*level)
        {
            // Book chapters (display level 2 = Asciidoctor level 1) get a
            // `{chapter-signifier} ` prefix before the section number when
            // `:chapter-signifier:` is set (unset by default), mirroring
            // Asciidoctor's html5 convert_section. Escaped — it goes raw into
            // heading and TOC HTML (like part-signifier/appendix-caption) and
            // precedes `pending_section_title_html_start`, so it stays out of
            // the title slice used for xref reference text.
            if self.doctype_book
                && *level == 2
                && let Some(sig) = self.document_attrs.get("chapter-signifier")
            {
                let mut esc = String::new();
                html_escape(&mut esc, sig);
                esc.push(' ');
                output.push_str(&esc);
                if let Some(ref mut entry) = self.current_toc_entry {
                    entry.title.push_str(&esc);
                }
            }
            output.push_str(&prefix);
            // The bare dotted number ("1.1") for this section's xref text.
            self.pending_section_number = self.section_numberer.last_number().map(str::to_string);
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
        // The rendered inline title begins here (after the number/caption
        // prefix); `TagEnd::SectionTitle` slices `output[start..]` as the raw
        // title HTML for xref reference text.
        self.pending_section_title_html_start = output.len();
    }

    pub(crate) fn start_section_div(&mut self, output: &mut String, level: &u8, meta: &Option<BlockMeta>) {
        let style = meta.as_ref().and_then(|m| m.style.as_deref());
        // `[abstract]` in a book doctype is reclassified by Asciidoctor's parser
        // as a numbered level-1 chapter (parser.rb `initialize_section`:
        // `sect_name='chapter', sect_level=1`), shedding its special status: it
        // renders as an ordinary numbered `sect1` chapter, not an unnumbered
        // abstract. (In an article it stays special.)
        let book_abstract = self.doctype_book && style == Some("abstract");
        let is_special = !book_abstract && matches!(style, Some(
            "appendix" | "glossary" | "bibliography" | "colophon"
            | "abstract" | "preface" | "dedication" | "index"
        ));
        // A level-0 section in the body (book part or article sect0) renders as a
        // standalone <h1 class="sect0"> with no wrapper div and no sectionbody.
        let is_sect0 = *level == 1 && !is_special;
        // Per-parent ordinals: an article body sect0 restarts its children's
        // numbering. Book parts don't — chapters number sequentially across
        // parts (document-global chapter-number counter).
        if is_sect0 && !self.doctype_book {
            self.section_numberer.reset_descendant_ordinals();
        }
        self.sect0_stack.push(is_sect0);
        self.sectionbody_stack.push(*level == 2 && !is_sect0);
        self.section_style_stack.push(
            if is_special { style.map(|s| s.to_string()) } else { None }
        );
        // Asciidoctor inherits `special` from the parent section (section.rb:
        // `@special = parent.special`) and numbers a special section only when
        // it's an appendix (or under `sectnums=all`, which we don't track). So a
        // non-appendix special section and its entire descendant subtree are
        // unnumbered: a `=== Subsection` under `[preface]`/`[colophon]`/etc. gets
        // no number even though it carries no special style of its own.
        let parent_unnumbered = self.section_unnumbered_stack.last().copied().unwrap_or(false);
        let unnumbered_subtree = parent_unnumbered || (is_special && style != Some("appendix"));
        self.section_unnumbered_stack.push(unnumbered_subtree);
        // Placeholder for the content-slot start; the real position is recorded at
        // `TagEnd::SectionTitle` once the heading (and, for a sect1, the
        // `sectionbody` open) has been emitted. `usize::MAX` reads as non-empty if
        // a title close never fires (no such case for a real section).
        self.section_content_start.push(usize::MAX);
        // Section kind for `xrefstyle` reference text (Asciidoctor `sectname`):
        // `[appendix]` → Appendix; a level-0 body section (book part / article
        // sect0) → Part; a book chapter (level 2, not special) → Chapter; every
        // other section (incl. special prefaces, deeper sections) → Section.
        self.pending_section_sectname = if style == Some("appendix") {
            SectName::Appendix
        } else if is_sect0 {
            SectName::Part
        } else if self.doctype_book && *level == 2 && !is_special {
            SectName::Chapter
        } else {
            SectName::Section
        };
        // Explicit reference text on the section block (`[reftext=…]` /
        // `[[id,reftext]]`, stashed by the parser as the `reftext` attribute):
        // it outranks the title in `Section#xreftext`. Rendered at section-title
        // close (where `render_inline_value` is reachable).
        self.pending_section_reftext = meta
            .as_ref()
            .and_then(|m| m.named.iter().find(|(k, _)| k == "reftext"))
            .map(|(_, v)| v.clone());
        // Bare section number for xref text. Reset here; set by the appendix
        // branch below, the `:partnums:` part branch below, or by
        // `start_section_title`'s `number_prefix` branch.
        self.pending_section_number = None;
        if !is_sect0 {
            output.push_str("<div");
            let sect_class = format!("sect{}", level - 1);
            // ID goes on the heading, not on the section div
            let mut div_meta = meta.clone();
            if let Some(ref mut m) = div_meta {
                m.id = None;
                if is_special || book_abstract {
                    m.style = None;
                }
            }
            Self::write_meta_attrs(output, &div_meta, &sect_class);
            output.push_str(">\n");
        }
        if style == Some("appendix") {
            // `:appendix-caption:` customizes the label; unset (`!`) drops it,
            // leaving the bare letter numeral ("A. "). The attribute value is
            // escaped here — the prefix goes raw into heading and TOC HTML.
            let caption = self.document_attrs.get("appendix-caption").map(|v| {
                let mut esc = String::new();
                html_escape(&mut esc, v);
                esc
            });
            self.pending_section_caption =
                Some(self.section_numberer.appendix_prefix(*level, caption.as_deref()));
            // The appendix letter ("A") is the bare number for xref text.
            self.pending_section_number = self.section_numberer.last_number().map(str::to_string);
        } else if is_sect0 && self.doctype_book && self.document_attrs.contains_key("partnums") {
            // Book part under `:partnums:` → "Part I: " / "I: " prefix
            // (roman numeral, document-global). `part-signifier` set (even
            // empty) contributes "{signifier} "; unset drops it — mirroring
            // Asciidoctor's html5 convert_section. The signifier is escaped
            // here (it goes raw into heading and TOC HTML, like appendix);
            // the roman numeral is plain ASCII.
            let signifier = self.document_attrs.get("part-signifier").map(|v| {
                let mut esc = String::new();
                html_escape(&mut esc, v);
                esc
            });
            self.pending_section_caption =
                Some(self.section_numberer.part_prefix(signifier.as_deref()));
            // A `:partnums:` part is `@numbered`: its bare roman numeral ("I")
            // feeds `Section#xreftext` so a full/short xref to the part renders
            // "{part-refsig} I, …" instead of the bare title.
            self.pending_section_number = self.section_numberer.last_number().map(str::to_string);
        } else if is_special {
            self.pending_section_caption = Some(String::new());
        }
    }

    pub(crate) fn start_paragraph(&mut self, output: &mut String, meta: &Option<BlockMeta>) {
        // The `hardbreaks` option (`[%hardbreaks]`) — or the document-wide
        // `hardbreaks-option` attribute — turns every soft line break in the
        // paragraph into a hard break (`<br>`).
        self.para_hardbreaks = self.document_attrs.contains_key("hardbreaks-option")
            || meta.as_ref().is_some_and(|m| m.options.iter().any(|o| o == "hardbreaks"));
        // Track paragraph count in list items
        if let Some(count) = self.li_para_count.last_mut() {
            *count += 1;
        }
        // An `[abstract]` paragraph renders as a quoteblock (Asciidoctor turns
        // the abstract paragraph style into an open block with simple content;
        // our parser keeps it a paragraph, so the renderer reshapes it here).
        if meta.as_ref().and_then(|m| m.style.as_deref()) == Some("abstract") {
            self.abstract_para = true;
            self.start_abstract_block(output, meta);
            return;
        }
        let is_continuation_para = self.li_para_count.last().is_some_and(|&c| c > 1);
        if self.is_direct_child_of_admonition() {
            // Inline admonitions: no <p> wrapper
        } else if !self.is_inside_compact_context() || is_continuation_para {
            output.push_str("<div");
            Self::write_meta_attrs(output, meta, "paragraph");
            output.push_str(">\n");
            self.emit_pending_block_title(output);
            output.push_str("<p>");
        } else {
            output.push_str("<p>");
        }
    }

    /// Asciidoctor adds the `nowrap` class to a verbatim block's `<pre>` when the
    /// block carries the `nowrap` option (`[%nowrap]`) or the document-wide
    /// `prewrap` attribute has been unset (`:prewrap!:`). `prewrap` is seeded as a
    /// default attribute in `HtmlRenderer::new`, so the second arm fires only when
    /// the user explicitly removed it. Mirrors `convert_listing`/`convert_literal`.
    fn nowrap_active(&self, meta: &Option<BlockMeta>) -> bool {
        meta.as_ref().is_some_and(|m| m.options.iter().any(|o| o == "nowrap"))
            || !self.document_attrs.contains_key("prewrap")
    }

    pub(crate) fn start_delimited_block(&mut self, output: &mut String, kind: &DelimitedBlockKind, meta: &Option<BlockMeta>) {
        let pre_open = if self.nowrap_active(meta) {
            "<pre class=\"nowrap\">"
        } else {
            "<pre>"
        };
        match kind {
            DelimitedBlockKind::Listing => {
                self.delimited_block_stack.push((*kind, false));
                output.push_str("<div");
                Self::write_meta_attrs(output, &Self::strip_block_style(meta), "listingblock");
                output.push_str(">\n");
                self.emit_listing_title(output, meta);
                output.push_str("<div class=\"content\">\n");
                output.push_str(pre_open);
            }
            DelimitedBlockKind::Literal => {
                self.delimited_block_stack.push((*kind, false));
                output.push_str("<div");
                Self::write_meta_attrs(output, &Self::strip_block_style(meta), "literalblock");
                output.push_str(">\n");
                self.emit_pending_block_title(output);
                output.push_str("<div class=\"content\">\n");
                output.push_str(pre_open);
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
                    Self::write_meta_attrs(output, meta, "");
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
                    Self::write_meta_attrs(output, meta, "exampleblock");
                    output.push_str(">\n");
                    if let Some(title) = self.block_title_inner_html.take() {
                        output.push_str("<div class=\"title\">");
                        let label = self.document_attrs.get("example-caption").cloned();
                        let prefix = self.render_caption_prefix(meta, label.as_deref(), CaptionKind::Example);
                        output.push_str(&prefix);
                        output.push_str(&title);
                        output.push_str("</div>\n");
                        self.register_block_ref(meta, prefix, title);
                    }
                    output.push_str("<div class=\"content\">\n");
                }
            }
            DelimitedBlockKind::Sidebar => {
                self.delimited_block_stack.push((*kind, false));
                output.push_str("<div");
                Self::write_meta_attrs(output, meta, "sidebarblock");
                output.push_str(">\n<div class=\"content\">\n");
                self.emit_pending_block_title(output);
            }
            DelimitedBlockKind::Quote => {
                self.delimited_block_stack.push((*kind, false));
                // Capture attribution and citetitle from metadata
                if let Some(m) = meta {
                    let has = |key: &str| m.named.iter().any(|(k, _)| k == key);
                    self.quote_attribution = m.named.iter()
                        .find(|(k, _)| k == "attribution")
                        .map(|(_, v)| (v.clone(), has("attribution-subs")));
                    self.quote_citetitle = m.named.iter()
                        .find(|(k, _)| k == "citetitle")
                        .map(|(_, v)| (v.clone(), has("citetitle-subs")));
                }
                output.push_str("<div");
                Self::write_meta_attrs(output, meta, "quoteblock");
                output.push_str(">\n");
                self.emit_pending_block_title(output);
                output.push_str("<blockquote>\n");
            }
            DelimitedBlockKind::Open => {
                // The `abstract` style turns an open block into a quoteblock
                // (Asciidoctor's `convert_open`). The stack flag records it so
                // the matching close emits `</blockquote>` instead of the open
                // block's content div.
                let style = meta.as_ref().and_then(|m| m.style.as_deref());
                let is_abstract = style == Some("abstract");
                // Asciidoctor's `convert_open` EXCLUDES a `[partintro]` block
                // (returns '') unless it is a direct child of a book part —
                // i.e. unless the doctype is book. (The valid book-part case is
                // produced by the parser's implicit wrap / explicit-style pass
                // and rendered below.) Outside a book the whole block, title and
                // all, contributes nothing; record the pre-block output position
                // and emit nothing — `TagEnd::DelimitedBlock` truncates back.
                let suppress_partintro = style == Some("partintro") && !self.doctype_book;
                if suppress_partintro {
                    self.partintro_suppress
                        .push((self.delimited_block_stack.len(), output.len()));
                    // The block's `.title` is excluded too — drop it so it can't
                    // leak onto the next block.
                    self.block_title_inner_html = None;
                }
                self.delimited_block_stack.push((*kind, is_abstract));
                if suppress_partintro {
                    // Content renders into `output` but is truncated at the close.
                } else if is_abstract {
                    self.start_abstract_block(output, meta);
                } else {
                    self.open_block_with_title(output, meta, "openblock");
                }
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
                if let Some(m) = meta {
                    let has = |key: &str| m.named.iter().any(|(k, _)| k == key);
                    self.quote_attribution = m.named.iter()
                        .find(|(k, _)| k == "attribution")
                        .map(|(_, v)| (v.clone(), has("attribution-subs")));
                    self.quote_citetitle = m.named.iter()
                        .find(|(k, _)| k == "citetitle")
                        .map(|(_, v)| (v.clone(), has("citetitle-subs")));
                }
                output.push_str("<div");
                Self::write_meta_attrs(output, meta, "verseblock");
                output.push_str(">\n");
                self.emit_pending_block_title(output);
                output.push_str("<pre class=\"content\">");
            }
        }
    }

    pub(crate) fn start_source_block(&mut self, output: &mut String, language: &Option<CowStr<'_>>, meta: &Option<BlockMeta>) {
        self.in_source_block = true;
        output.push_str("<div");
        Self::write_meta_attrs(output, meta, "listingblock");
        output.push_str(">\n");
        self.emit_listing_title(output, meta);
        output.push_str("<div class=\"content\">\n<pre");

        let highlighter = self.document_attrs.get("source-highlighter").cloned();
        // Line numbering only renders under a build-time highlighter; with
        // highlight.js or no highlighter Asciidoctor ignores the option
        // entirely (no class, no table).
        let linenums = matches!(highlighter.as_deref(), Some("rouge" | "pygments" | "coderay"))
            && meta.as_ref().is_some_and(|m| {
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
        // `nowrap` is appended last (`[%nowrap]` option or `:prewrap!:`), matching
        // Asciidoctor's `<pre class="… highlight … nowrap">` ordering.
        if self.nowrap_active(meta) {
            pre_classes.push("nowrap");
        }
        if !pre_classes.is_empty() {
            output.push_str(" class=\"");
            output.push_str(&pre_classes.join(" "));
            output.push('"');
        }

        output.push_str("><code");

        // Build <code> attrs
        let is_hljs = matches!(highlighter.as_deref(), Some("highlight.js" | "highlightjs"));
        if let Some(lang) = language {
            if is_hljs {
                // Asciidoctor order: `language-X hljs` (language class first).
                output.push_str(" class=\"language-");
                html_escape(output, lang);
                output.push_str(" hljs\"");
            } else if highlighter.is_none() {
                output.push_str(" class=\"language-");
                html_escape(output, lang);
                output.push('"');
            }
            write_attr(output, "data-lang", lang);
        } else if is_hljs {
            // highlight.js without a language: Asciidoctor emits `language-none hljs`
            // and no data-lang attribute.
            output.push_str(" class=\"language-none hljs\"");
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

    pub(crate) fn emit_pending_block_title(&mut self, output: &mut String) {
        if let Some(title) = self.block_title_inner_html.take() {
            output.push_str("<div class=\"title\">");
            output.push_str(&title);
            output.push_str("</div>\n");
        }
    }

    /// Emit a listing/source block's `.title` caption. `listing-caption` is unset
    /// by default, so this normally renders the bare title (matching the plain
    /// `.title` path); when the attribute is set it prefixes "Listing N. " and
    /// shares one counter across listing and source blocks. Always registers the
    /// block for `full`/`short` cross references.
    pub(crate) fn emit_listing_title(&mut self, output: &mut String, meta: &Option<BlockMeta>) {
        if let Some(title) = self.block_title_inner_html.take() {
            output.push_str("<div class=\"title\">");
            let label = self.document_attrs.get("listing-caption").cloned();
            let prefix = self.render_caption_prefix(meta, label.as_deref(), CaptionKind::Listing);
            output.push_str(&prefix);
            output.push_str(&title);
            output.push_str("</div>\n");
            self.register_block_ref(meta, prefix, title);
        }
    }

    /// Open the principal `<p>` of a list item, tracking it so a continuation
    /// block can close it before being emitted (see the guard in `start_tag`).
    pub(crate) fn open_li_paragraph(&mut self) {
        self.li_p_open.push(LiPara::OpenItem);
        self.li_para_count.push(1); // count the initial <p>
    }

    /// Pop a list-item scope; returns whether the principal `<p>` is still
    /// open and needs its closing tag.
    pub(crate) fn close_li_paragraph(&mut self) -> bool {
        self.li_para_count.pop();
        self.li_p_open.pop().is_some_and(LiPara::is_open)
    }

    /// Open a block wrapper div with meta attrs, emit a pending `.Title`,
    /// then open the content div — the shared shape of the audio/video/stem
    /// and open-block arms. Forgetting the title emission in a hand-rolled
    /// wrapper has caused a string of leak bugs; new block arms should use
    /// this instead.
    pub(crate) fn open_block_with_title(&mut self, output: &mut String, meta: &Option<BlockMeta>, default_class: &str) {
        output.push_str("<div");
        Self::write_meta_attrs(output, meta, default_class);
        output.push_str(">\n");
        self.emit_pending_block_title(output);
        output.push_str("<div class=\"content\">\n");
    }

    /// Open an `[abstract]` block. Asciidoctor's `convert_open` renders the
    /// `abstract` style as a `quoteblock` wrapping a `<blockquote>`, regardless
    /// of whether the source was a paragraph (simple content model → bare text
    /// in the blockquote) or an open block (compound → child blocks). Only the
    /// inner content differs; the event stream supplies it. The class/id/roles
    /// come from `write_meta_attrs` — style "abstract" is already in `meta`, so
    /// the default class "quoteblock" yields `class="quoteblock abstract …"`.
    pub(crate) fn start_abstract_block(&mut self, output: &mut String, meta: &Option<BlockMeta>) {
        output.push_str("<div");
        Self::write_meta_attrs(output, meta, "quoteblock");
        output.push_str(">\n");
        self.emit_pending_block_title(output);
        output.push_str("<blockquote>\n");
    }

    /// Close an `[abstract]` block opened by [`start_abstract_block`]. The
    /// newline guard handles both forms: a paragraph's bare text leaves no
    /// trailing newline, an open block's last child already ends in one.
    pub(crate) fn close_abstract_block(output: &mut String) {
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("</blockquote>\n</div>\n");
    }

    /// Emit the numbered caption prefix for a titled block (table/figure).
    /// A block-level `caption=` overrides verbatim with no counter bump
    /// (empty value → no prefix); otherwise a set `*-caption` document
    /// attribute gives "{label} {n}. " and bumps the counter, while an unset
    /// one (`:table-caption!:` / `:figure-caption!:`) gives no prefix.
    /// Build the rendered caption prefix string for a titled block, bumping the
    /// per-kind counter exactly once. The result is the escaped prefix as it
    /// appears on the block (`"Figure 1. "`, a custom `caption=` value, or empty
    /// when suppressed/unset) — used both for the block's own `.title` markup and
    /// as the `caption` input to `block_xreftext`.
    pub(crate) fn render_caption_prefix(
        &mut self,
        meta: &Option<BlockMeta>,
        doc_label: Option<&str>,
        kind: CaptionKind,
    ) -> String {
        let caption_attr = meta
            .as_ref()
            .and_then(|m| m.named.iter().find(|(k, _)| k == "caption").map(|(_, v)| v.clone()));
        let mut prefix = String::new();
        match self.caption_counters.caption_prefix(kind, caption_attr.as_deref(), doc_label) {
            CaptionPrefix::None => {}
            CaptionPrefix::Custom(custom) => html_escape(&mut prefix, custom),
            CaptionPrefix::Numbered { label, number } => {
                html_escape(&mut prefix, label);
                prefix.push(' ');
                prefix.push_str(&number.to_string());
                prefix.push_str(". ");
            }
        }
        prefix
    }

    /// Record a titled block's caption + title under its id so a `full`/`short`
    /// cross reference to it can build `AbstractBlock#xreftext`. No-op without an
    /// id (an unreferenceable block needs no entry).
    pub(crate) fn register_block_ref(&mut self, meta: &Option<BlockMeta>, caption: String, title_html: String) {
        if let Some(id) = meta.as_ref().and_then(|m| m.id.clone()) {
            self.block_refs.push((id, BlockRefMeta { caption, title_html }));
        }
    }

    pub(crate) fn take_block_meta(&mut self) -> Option<BlockMeta> {
        self.pending_block_meta.take()
    }

    /// Trim leading and trailing blank lines from verbatim (pre) content in the output buffer.
    /// Finds the last `<pre>` or `<pre ...>` tag and trims blank lines after it and before end.
    pub(crate) fn trim_verbatim_content(output: &mut String) {
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
    /// Uses f64 with 4-decimal precision to match Asciidoctor output.
    /// The last column gets the remainder to ensure percentages sum to exactly 100%.
    pub(crate) fn parse_col_widths(cols_str: &str) -> Vec<f64> {
        let trimmed = cols_str.trim();

        // Simple numeric: "3" → 3 equal columns
        if let Ok(n) = trimmed.parse::<usize>() {
            if n == 0 { return Vec::new(); }
            return Self::distribute_widths(&vec![1.0; n]);
        }

        // Comma- or semicolon-separated: parse each part for weight. Match the
        // parser's separator rule (attributes.rs table_col_specs): a comma
        // forces comma-splitting, otherwise split on semicolon — so unquoted
        // `[cols=1;m;m]` yields three columns here too.
        let sep = if trimmed.contains(',') { ',' } else { ';' };
        let parts: Vec<&str> = trimmed.split(sep).collect();
        let mut weights: Vec<f64> = Vec::new();
        for part in &parts {
            let mut part = part.trim();
            if part.is_empty() { continue; }
            // Repetition multiplier: `3*`, `2*1`, `2*<.^2` repeat the spec
            let mut count = 1usize;
            if let Some(star) = part.find('*') {
                let before = &part[..star];
                if !before.is_empty() && before.chars().all(|c| c.is_ascii_digit()) {
                    count = before.parse::<usize>().map_or(1, |n: usize| n.max(1));
                    part = &part[star + 1..];
                }
            }
            // Extract numeric weight from the spec (e.g., "1", "<2", "^.>1").
            // A trailing style letter (`1m`, `3e`) is not part of the weight —
            // strip it first (cols="1m,3m" → 25%/75%, probe-verified).
            let part = part.strip_suffix(['a', 'd', 'e', 'h', 'l', 'm', 's', 'v']).unwrap_or(part);
            let weight = part.chars().rev()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .chars().rev().collect::<String>()
                .parse::<f64>()
                .unwrap_or(1.0);
            for _ in 0..count {
                weights.push(weight);
            }
        }

        if weights.is_empty() { return Vec::new(); }

        Self::distribute_widths(&weights)
    }

    /// Distribute column widths as percentages with 4-decimal precision.
    /// Last column gets the remainder so the total is exactly 100%.
    pub(crate) fn distribute_widths(weights: &[f64]) -> Vec<f64> {
        let total: f64 = weights.iter().sum();
        if total == 0.0 { return Vec::new(); }

        let n = weights.len();
        let mut widths: Vec<f64> = Vec::with_capacity(n);
        let mut sum: f64 = 0.0;
        for (i, w) in weights.iter().enumerate() {
            if i == n - 1 {
                // Last column gets remainder to ensure sum = 100%
                // Truncate to 4 decimal places
                let raw = 100.0_f64 - sum;
                let last = (raw * 10000.0).round() / 10000.0;
                widths.push(last);
            } else {
                // Truncate to 4 decimal places (floor)
                let raw = w * 100.0 / total;
                let pct = (raw * 10000.0).floor() / 10000.0;
                sum += pct;
                widths.push(pct);
            }
        }
        widths
    }

    /// Format a column width percentage to match Asciidoctor output.
    /// Integer values: "50%" (no decimals). Fractional: "33.3333%" (4 decimals).
    pub(crate) fn format_col_width(w: f64) -> String {
        if (w - w.round()).abs() < 0.00005 {
            format!("{}%", w.round() as u32)
        } else {
            format!("{:.4}%", w)
        }
    }

    pub(crate) fn tableblock_cell_class(halign: &HAlign, valign: &VAlign) -> String {
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
    /// Verbatim blocks (literal/listing) derive their CSS class from the block
    /// context alone. An unrecognized block style (e.g. `[plantuml]`, `[ditaa]`)
    /// is dropped from the class, matching Asciidoctor — only id and roles
    /// survive. Styles that carry meaning (`source` → SourceBlock path,
    /// `listing`/`literal` → context) are resolved before reaching this arm.
    pub(crate) fn strip_block_style(meta: &Option<BlockMeta>) -> Option<BlockMeta> {
        meta.clone().map(|mut m| {
            m.style = None;
            m
        })
    }

    pub(crate) fn write_meta_attrs(output: &mut String, meta: &Option<BlockMeta>, default_class: &str) {
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
                // Escape at the emission boundary: every value entering an HTML
                // attribute is escaped exactly once here (D1/D7 systemic rule).
                // No-op for fixed class literals; protects user-derived default_class
                // such as the ordered-list `olist {style}` wrapper class.
                html_escape(output, default_class);
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

    pub(crate) fn current_dlist_style(&self) -> DlistStyle {
        self.dlist_stack.last().copied().unwrap_or(DlistStyle::Normal)
    }

    pub(crate) fn find_section_level(&self) -> u8 {
        for tag_end in self.tag_stack.iter().rev() {
            if let TagEnd::Section { level } = tag_end {
                return *level;
            }
        }
        1
    }

    /// Returns true when the immediate parent on the tag stack is a paragraph-form
    /// Admonition (`NOTE: text`). Used to suppress <p> tags: paragraph admonitions
    /// render their text bare in the content td; block admonitions (`[NOTE]` + `====`)
    /// keep normal paragraph wrappers.
    pub(crate) fn is_direct_child_of_admonition(&self) -> bool {
        // In start_tag: stack has [..., Admonition, Paragraph], so check second-to-last
        // In end_tag: stack has [..., Admonition] (Paragraph already popped), so check last
        // Both cases: look for Admonition as the nearest non-Paragraph ancestor
        for tag_end in self.tag_stack.iter().rev() {
            match tag_end {
                TagEnd::Paragraph => continue, // skip self during start_tag
                TagEnd::Admonition => {
                    // The innermost open admonition is the top of the parallel stack.
                    return self.admonition_block_stack.last().is_some_and(|&block| !block);
                }
                _ => return false,
            }
        }
        false
    }

    /// Returns true when inside a list item (for skipping list wrapper divs on nested lists).
    pub(crate) fn is_inside_list_item(&self) -> bool {
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
    pub(crate) fn is_inside_compact_context(&self) -> bool {
        for tag_end in self.tag_stack.iter().rev() {
            match tag_end {
                TagEnd::Admonition => {
                    // Block-form admonitions wrap child paragraphs normally;
                    // paragraph-form content is compact (suppressed upstream anyway).
                    return self.admonition_block_stack.last().is_some_and(|&block| !block);
                }
                TagEnd::ListItem
                | TagEnd::DescriptionDescription
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

    /// Emit the `<div class="attribution">` trailer for a quote/verse block
    /// from the captured attribution/citetitle. Values flagged for
    /// substitution (single-quoted attrlist values, quoted-paragraph and
    /// markdown-quote credit lines) are rendered with normal subs; others
    /// are escaped verbatim.
    pub(crate) fn render_quote_attribution(&mut self, output: &mut String) {
        let attribution = self.quote_attribution.take();
        let citetitle = self.quote_citetitle.take();
        if attribution.is_none() && citetitle.is_none() {
            return;
        }
        output.push_str("<div class=\"attribution\">\n");
        if let Some((ref attr, subs)) = attribution {
            output.push_str("&#8212; ");
            if subs {
                self.render_inline_value(output, attr);
            } else {
                html_escape(output, attr);
            }
        }
        if let Some((ref cite, subs)) = citetitle {
            if attribution.is_some() {
                output.push_str("<br>\n");
            }
            output.push_str("<cite>");
            if subs {
                self.render_inline_value(output, cite);
            } else {
                html_escape(output, cite);
            }
            output.push_str("</cite>");
        }
        output.push('\n');
        output.push_str("</div>\n");
    }
}
