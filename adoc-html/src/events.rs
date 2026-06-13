//! Event dispatch: the per-event entry point and the `Start`/`End` tag dispatchers.

use crate::*;

impl HtmlRenderer {
    pub(crate) fn push_event<'a>(&mut self, output: &mut String, event: Event<'a>) {
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
                // Raw content of an open AsciiDoc-style (`a`) table cell: collect
                // for the nested block parse on TagEnd::TableCell.
                if let Some(buf) = self.acell_capture.last_mut() {
                    buf.push_str(&text);
                    return;
                }
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
                    if let Some((placeholder, _, _)) = self.xref_placeholders.last() {
                        output.push_str(placeholder);
                    }
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
                    let stripped = rstrip_line_trailing_ws(&text);
                    if self.para_hardbreaks {
                        push_hardbreaks_text(target, &stripped, true);
                    } else {
                        html_escape_text(target, &stripped);
                    }
                } else {
                    let stripped = rstrip_line_trailing_ws(&text);
                    if self.para_hardbreaks {
                        push_hardbreaks_text(output, &stripped, false);
                    } else {
                        output.push_str(&stripped);
                    }
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
                    // Asciidoctor rstrips every source line: trailing spaces/tabs
                    // on the line just rendered are dropped before the line break.
                    // This path covers verbatim blocks (source/listing), where each
                    // line arrives as a separate Text + SoftBreak. Normal paragraphs
                    // are combined into one multi-line Text by the parser and handled
                    // in the Text arm via `rstrip_line_trailing_ws`.
                    let keep = target.trim_end_matches([' ', '\t']).len();
                    target.truncate(keep);
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
                self.apply_attribute(name, value);
                // Handle doctype=manpage retroactive title insertion
                if name.as_ref() == "doctype" && value.as_ref() == "manpage" {
                    if let Some(pos) = self.doctitle_close_pos {
                        output.insert_str(pos, " Manual Page");
                    }
                    if let Some(doctitle) = self.document_attrs.get("doctitle").cloned()
                        && let Some((mantitle, manvolnum)) = parse_manpage_title(&doctitle)
                    {
                        self.document_attrs.insert("mantitle".to_string(), mantitle);
                        self.document_attrs.insert("manvolnum".to_string(), manvolnum);
                    }
                }
            }
            Event::AttributeReference { name, fallback, trailing_brackets } => {
                let outcome = adoc_render_core::resolve_attribute_reference(
                    &name,
                    |n| self.document_attrs.get(n).map(|s| s.as_str()),
                    |env_name| std::env::var(env_name).ok(),
                    fallback.as_deref(),
                    self.document_attrs.get("attribute-missing").map(|s| s.as_str()),
                );
                use adoc_render_core::AttrRefOutcome;
                match outcome {
                    AttrRefOutcome::Document(value) => {
                        let lower_name = name.to_ascii_lowercase();
                        if self.attr_refs_in_progress.contains(&lower_name) {
                            // Re-entered while rendering this attribute's own
                            // value (`:x: {x}`) — emit the reference literally,
                            // like asciidoctor's linear substitution does.
                            output.push('{');
                            output.push_str(&name);
                            output.push('}');
                            if let Some(br) = trailing_brackets {
                                html_escape_text(output, &br);
                            }
                        } else {
                            let value = value.to_string();
                            self.attr_refs_in_progress.push(lower_name);
                            // Attributes substitute before macros: if a trailing `[...]`
                            // was captured, re-parse `value[...]` together so a URL-valued
                            // attribute forms a link macro. For non-URL values the bracket
                            // stays literal — same result as rendering them separately.
                            match trailing_brackets {
                                Some(br) => {
                                    let combined = format!("{value}{br}");
                                    self.render_inline_value(output, &combined);
                                }
                                None => self.render_inline_value(output, &value),
                            }
                            self.attr_refs_in_progress.pop();
                        }
                    }
                    AttrRefOutcome::Intrinsic(attr) => {
                        // The `html` column is pre-encoded — push raw
                        output.push_str(attr.html);
                        if let Some(br) = trailing_brackets {
                            html_escape_text(output, &br);
                        }
                    }
                    AttrRefOutcome::Env(value) => {
                        html_escape(output, &value);
                        if let Some(br) = trailing_brackets {
                            html_escape_text(output, &br);
                        }
                    }
                    AttrRefOutcome::Fallback(fb) => {
                        let fb = fb.to_string();
                        html_escape(output, &fb);
                        if let Some(br) = trailing_brackets {
                            html_escape_text(output, &br);
                        }
                    }
                    AttrRefOutcome::MissingSkip => {
                        output.push('{');
                        output.push_str(&name);
                        output.push('}');
                        if let Some(br) = trailing_brackets {
                            html_escape_text(output, &br);
                        }
                    }
                    AttrRefOutcome::MissingDrop => {}
                }
            }
            Event::Footnote { id, text } => {
                // A footnote whose id is already registered is a reference to
                // the existing definition — its text is ignored and the
                // counter is not bumped.
                if let Some(num) = id.as_deref().and_then(|i| self.footnote_registry.lookup(i)) {
                    push_footnote_ref(output, num);
                } else {
                    let num = self.footnote_registry.define(id.as_deref(), &text);
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
            }
            Event::FootnoteRef { id } => {
                if let Some(num) = self.footnote_registry.lookup(id.as_ref()) {
                    push_footnote_ref(output, num);
                } else {
                    output.push_str(
                        "<sup class=\"footnoteref red\" title=\"Unresolved footnote reference.\">[",
                    );
                    html_escape(output, &id);
                    output.push_str("]</sup>");
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
                output.push_str("\"></a>");
                // Reftext is the bracketed label (or id). Register it so a
                // `<<id>>` cross reference to this entry resolves to the same
                // `[label]` text in `finish()`.
                let mut reftext = String::from("[");
                html_escape(&mut reftext, label.as_ref().unwrap_or(&id));
                reftext.push(']');
                output.push_str(&reftext);
                self.bibliography_reftexts.push((id.to_string(), reftext));
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
            Event::XmlCalloutRef(num) => {
                let target = if self.in_source_block {
                    if let Some(ref mut buf) = self.source_code_buffer { buf } else { output }
                } else {
                    output
                };
                target.push_str("&lt;!--");
                target.push_str("<b class=\"conum\">(");
                target.push_str(&num.to_string());
                target.push_str(")</b>");
                target.push_str("--&gt;");
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
                let entries = self.authors.add(Author {
                    fullname: fullname.to_string(),
                    firstname: firstname.to_string(),
                    middlename: middlename.to_string(),
                    lastname: lastname.to_string(),
                    initials: initials.to_string(),
                    address: address.to_string(),
                });
                self.document_attrs.extend(entries);
                self.document_attrs.insert(
                    "authorcount".to_string(),
                    self.authors.authors().len().to_string(),
                );
            }
            Event::Revision { version, date, remark } => {
                let revision = Revision {
                    version: version.to_string(),
                    date: date.to_string(),
                    remark: remark.to_string(),
                };
                for (name, value) in revision.attr_entries() {
                    self.document_attrs.insert(name.to_string(), value.to_string());
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
            Event::TableCellParagraphBreak => {
                // Close the current <p class="tableblock"> paragraph (and its
                // style wrapper, if any) and open the next one. The cell style is
                // on top of the stack until TagEnd::TableCell pops it.
                let style = self.cell_style_stack.last().copied().unwrap_or_default();
                match style {
                    CellStyle::Emphasis => output.push_str("</em></p><p class=\"tableblock\"><em>"),
                    CellStyle::Strong => output.push_str("</strong></p><p class=\"tableblock\"><strong>"),
                    CellStyle::Monospace => output.push_str("</code></p><p class=\"tableblock\"><code>"),
                    _ => output.push_str("</p><p class=\"tableblock\">"),
                }
            }
        }
    }

    pub(crate) fn start_tag(&mut self, output: &mut String, tag: &Tag) {
        // Close <p> inside list item when a sub-block starts
        match tag {
            Tag::Paragraph | Tag::UnorderedList { .. } | Tag::OrderedList { .. }
            | Tag::DescriptionList | Tag::DelimitedBlock { .. } | Tag::SourceBlock { .. }
            | Tag::BlockImage { .. } | Tag::Table | Tag::Admonition { .. }
                if self.li_p_open.last() == Some(&true) =>
            {
                // A principal `<p>` with no text — e.g. a description-list item
                // whose first content is a block (`term::` + `+` + `--`, or a
                // term immediately followed by a nested list) — is never emitted
                // by Asciidoctor. If nothing was written after the opening `<p>`,
                // roll it back instead of closing an empty `<p></p>`.
                if output.ends_with("<p>") {
                    output.truncate(output.len() - 3);
                } else {
                    output.push_str("</p>\n");
                }
                *self.li_p_open.last_mut().unwrap() = false;
            }
            _ => {}
        }

        let tag_end = tag.to_end();
        self.tag_stack.push(tag_end);
        let meta = self.take_block_meta();

        // Record `id -> title` for a titled block so an empty cross-reference
        // (`<<id>>`) to it resolves to the block's title, like Asciidoctor.
        // Captured here because both the metadata id and the buffered title are
        // available regardless of the source order of `[#id]` and `.Title`.
        if let Some(m) = &meta
            && let Some(id) = &m.id
            && let Some(title) = &self.block_title_inner_html
        {
            self.block_ref_titles.push((id.clone(), title.clone()));
        }

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
            Tag::SectionTitle { level, id } => self.start_section_title(output, level, id),
            Tag::Heading { level } => {
                let h = section_level_to_h(*level);
                output.push_str("<h");
                output.push_str(&h.to_string());
                Self::write_meta_attrs(output, &meta, "");
                output.push('>');
            }
            Tag::Section { level } => self.start_section_div(output, level, &meta),
            Tag::Paragraph => self.start_paragraph(output, &meta),
            Tag::LiteralParagraph => {
                output.push_str("<div");
                Self::write_meta_attrs(output, &meta, "literalblock");
                output.push_str(">\n");
                self.emit_pending_block_title(output);
                output.push_str("<div class=\"content\">\n<pre>");
            }
            Tag::DelimitedBlock { kind } => self.start_delimited_block(output, kind, &meta),
            Tag::SourceBlock { language } => self.start_source_block(output, language, &meta),
            Tag::BlockTitle => {
                self.block_title_output_start = Some(output.len());
                output.push_str("<div class=\"title\">");
            }
            Tag::UnorderedList { has_checklist } => self.start_unordered_list(output, has_checklist, &meta),
            Tag::OrderedList { start, reversed, depth } => self.start_ordered_list(output, start, reversed, *depth, &meta),
            Tag::ListItem { checked, .. } => {
                let interactive = self.interactive_ulist_stack.last().copied().unwrap_or(false);
                output.push_str(match (checked, interactive) {
                    (Some(true), true) => "<li>\n<p><input type=\"checkbox\" data-item-complete=\"1\" checked> ",
                    (Some(false), true) => "<li>\n<p><input type=\"checkbox\" data-item-complete=\"0\"> ",
                    (Some(true), false) => "<li>\n<p>&#10003; ",
                    (Some(false), false) => "<li>\n<p>&#10063; ",
                    (None, _) => "<li>\n<p>",
                });
                self.open_li_paragraph();
            }
            Tag::DescriptionList => self.start_description_list(output, &meta),
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
                        // The first term of an item opens the <li>; adjacent
                        // terms sharing one answer (consecutive `term::` lines)
                        // only emit their own <p><em>…</em></p> inside it
                        // (Asciidoctor convert_dlist qanda). Reuses the shared
                        // in-term-group flag — qanda and horizontal never coexist
                        // in one list (a list has a single style).
                        if self.hdlist_in_term_group {
                            output.push_str("<p><em>");
                        } else {
                            output.push_str("<li>\n<p><em>");
                            self.hdlist_in_term_group = true;
                        }
                    }
                    DlistStyle::Normal => {
                        output.push_str("<dt class=\"hdlist1\">");
                    }
                    // A styled dlist (`[glossary]`, custom) drops the
                    // hdlist1 class from its terms.
                    DlistStyle::Styled => {
                        output.push_str("<dt>");
                    }
                }
                self.dt_term_start = Some(output.len());
            }
            Tag::DescriptionDescription => {
                self.li_para_count.push(1); // count the initial <p> in <dd>
                match self.current_dlist_style() {
                    DlistStyle::Horizontal => {
                        output.push_str("</td>\n<td class=\"hdlist2\">\n<p>");
                        self.hdlist_in_term_group = false;
                        self.li_p_open.push(true);
                    }
                    DlistStyle::Qanda => {
                        // The answer is wrapped in <p>…</p> (Asciidoctor emits
                        // `<p>#{dd.text}</p>` if the dd has text). An empty
                        // answer leaves a bare <p> that the end-arm rolls back.
                        self.hdlist_in_term_group = false;
                        self.dd_output_start = Some(output.len());
                        output.push_str("<p>");
                        self.li_p_open.push(true);
                    }
                    DlistStyle::Normal | DlistStyle::Styled => {
                        self.dd_output_start = Some(output.len());
                        output.push_str("<dd>\n<p>");
                        self.li_p_open.push(true);
                    }
                }
            }
            Tag::CalloutList => {
                output.push_str("<div class=\"colist arabic\">\n");
                self.emit_pending_block_title(output);
                output.push_str("<ol>\n");
            }
            Tag::CalloutListItem { .. } => {
                output.push_str("<li><p>");
                // Track the principal `<p>` like a regular list item so a
                // continuation block (e.g. a `+`-attached NOTE) closes it
                // before the block is emitted, instead of nesting the block
                // inside the still-open `<p>`.
                self.open_li_paragraph();
            }
            Tag::Admonition { kind, block } => self.start_admonition(output, kind, *block, &meta),
            Tag::Table => self.start_table(output, &meta),
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
            Tag::TableCell { colspan, rowspan, style, halign, valign } =>
                self.start_table_cell(output, false, colspan, rowspan, style, halign, valign),
            Tag::TableHeaderCell { colspan, rowspan, style, halign, valign } =>
                self.start_table_cell(output, true, colspan, rowspan, style, halign, valign),
            Tag::BlockImage { target, alt, width, height, link } =>
                self.start_block_image(output, target, alt, width, height, link, &meta),
            Tag::BlockVideo { target, attrs } => {
                self.open_block_with_title(output, &meta, "videoblock");
                render_video_tag(output, target, attrs);
            }
            Tag::BlockAudio { target, attrs } => {
                self.open_block_with_title(output, &meta, "audioblock");
                render_audio_tag(output, target, attrs);
            }
            Tag::InlineImage { target, alt, width, height, align: _, float, link, role, title } =>
                self.start_inline_image(output, target, alt, width, height, float, link, role, title),
            Tag::Strong { id, roles } => {
                output.push_str("<strong");
                Self::push_inline_id_class(output, id, roles);
                output.push('>');
            }
            Tag::Emphasis { id, roles } => {
                output.push_str("<em");
                Self::push_inline_id_class(output, id, roles);
                output.push('>');
            }
            Tag::Monospace { id, roles } => {
                output.push_str("<code");
                Self::push_inline_id_class(output, id, roles);
                output.push('>');
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
            Tag::Link { url, window, nofollow, is_bare, role } => {
                output.push_str("<a");
                write_attr(output, "href", url);
                // class comes right after href (asciidoctor order); bare
                // precedes the role: class="bare green".
                if *is_bare || role.is_some() {
                    output.push_str(" class=\"");
                    if *is_bare {
                        output.push_str("bare");
                        if role.is_some() {
                            output.push(' ');
                        }
                    }
                    if let Some(r) = role {
                        html_escape(output, r);
                    }
                    output.push('"');
                }
                if let Some(w) = window {
                    write_attr(output, "target", w);
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
            Tag::CrossReference { target, label } => self.start_cross_reference(output, target, label),
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
                self.open_block_with_title(output, &meta, "stemblock");
            }
            Tag::Anchor { id, label } => {
                let at_term_start = self.dt_term_start == Some(output.len());
                output.push_str("<a id=\"");
                html_escape(output, id);
                output.push_str("\"></a>");
                if let Some(label) = label {
                    // Explicit xreflabel: render it (normal subs apply at use)
                    // and register as this anchor's reference text.
                    let mut reftext = String::new();
                    self.render_inline_value(&mut reftext, label);
                    self.anchor_reftexts.push((id.to_string(), reftext));
                } else if at_term_start && self.pending_term_anchor.is_none() {
                    // Leading anchor in a dlist term: the rendered term that
                    // follows becomes the default reftext (captured at
                    // TagEnd::DescriptionTerm).
                    self.pending_term_anchor = Some((id.to_string(), output.len()));
                }
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

    pub(crate) fn end_tag(&mut self, output: &mut String, tag_end: &TagEnd) {
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
                // Asciidoctor locks the doctype when the header ends; a
                // `:doctype: book` entry in the body updates the attribute
                // table but not the structural behavior (part numbering).
                self.doctype_book =
                    self.document_attrs.get("doctype").is_some_and(|d| d == "book");
                self.finalize_header_authors();
                if self.standalone {
                    self.render_author_details(output);
                    // An auto-TOC (`:toc:` in the header) sits AFTER the
                    // author details: Asciidoctor's #header is h1, details,
                    // then the toc div.
                    if self.toc_auto_seen
                        && !matches!(self.toc_position.as_str(), "preamble" | "macro")
                    {
                        self.toc_insert_position = Some(output.len());
                    }
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
                        self.toc_builder.push(entry);
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
                let is_sect0 = self.sect0_stack.pop().unwrap_or(false);
                let needs_sectionbody_close = self.sectionbody_stack.pop().unwrap_or(false);
                self.section_style_stack.pop();
                if !is_sect0 {
                    if needs_sectionbody_close {
                        output.push_str("</div>\n");
                    }
                    output.push_str("</div>\n");
                }
            }
            TagEnd::Paragraph => {
                self.para_hardbreaks = false;
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
                        // Bare masqueraded-paragraph content has no trailing newline
                        if !output.ends_with('\n') {
                            output.push('\n');
                        }
                        output.push_str("</blockquote>\n");
                        self.render_quote_attribution(output);
                        output.push_str("</div>\n");
                    }
                    Some((DelimitedBlockKind::Verse, _)) => {
                        output.push_str("</pre>\n");
                        self.render_quote_attribution(output);
                        output.push_str("</div>\n");
                    }
                    Some((DelimitedBlockKind::Example, true)) => {
                        if !output.ends_with('\n') {
                            output.push('\n');
                        }
                        output.push_str("</div>\n</details>\n");
                    }
                    Some((DelimitedBlockKind::Example | DelimitedBlockKind::Sidebar
                         | DelimitedBlockKind::Open, false)) => {
                        if !output.ends_with('\n') {
                            output.push('\n');
                        }
                        output.push_str("</div>\n</div>\n");
                    }
                    Some((DelimitedBlockKind::Passthrough, _)) => {
                        // Passthrough content is emitted bare — no wrapper was opened.
                        if !output.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                    Some((DelimitedBlockKind::Comment, _)) => {}
                    _ => {
                        output.push_str("</div>\n");
                    }
                }
            }
            TagEnd::SourceBlock => {
                if self.linenums_active {
                    if self.source_line_highlighted {
                        if let Some(buf) = self.source_code_buffer.as_mut() {
                            buf.push_str("</span>");
                        }
                        self.source_line_highlighted = false;
                    }
                    if let Some(code) = self.source_code_buffer.take() {
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
                    }

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
                self.interactive_ulist_stack.pop();
                output.push_str("</ul>\n</div>\n");
            }
            TagEnd::OrderedList => {
                output.push_str("</ol>\n</div>\n");
            }
            TagEnd::ListItem => {
                let p_open = self.close_li_paragraph();
                output.push_str(if p_open { "</p>\n</li>\n" } else { "</li>\n" });
            }
            TagEnd::DescriptionList => {
                let style = self.dlist_stack.pop().unwrap_or(DlistStyle::Normal);
                match style {
                    DlistStyle::Horizontal => output.push_str("</table>\n</div>\n"),
                    DlistStyle::Qanda => output.push_str("</ol>\n</div>\n"),
                    DlistStyle::Normal | DlistStyle::Styled => output.push_str("</dl>\n</div>\n"),
                }
            }
            TagEnd::DescriptionTerm => {
                if let Some((id, pos)) = self.pending_term_anchor.take()
                    && output.len() > pos
                {
                    let reftext = output[pos..].to_string();
                    self.anchor_reftexts.push((id, reftext));
                }
                self.dt_term_start = None;
                match self.current_dlist_style() {
                    DlistStyle::Horizontal => {}
                    DlistStyle::Qanda => output.push_str("</em></p>\n"),
                    DlistStyle::Normal | DlistStyle::Styled => output.push_str("</dt>\n"),
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
                    DlistStyle::Qanda => {
                        // Roll back an empty answer's bare <p> (a sub-block start
                        // already cleared li_p_open via the principal-<p> guard),
                        // otherwise close the answer paragraph; then close <li>.
                        if let Some(start) = self.dd_output_start.take()
                            && &output[start..] == "<p>"
                        {
                            output.truncate(start);
                            self.li_p_open.pop();
                        } else if self.li_p_open.pop() == Some(true) {
                            output.push_str("</p>\n");
                        }
                        output.push_str("</li>\n");
                    }
                    DlistStyle::Normal | DlistStyle::Styled => {
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
                // A closed principal `<p>` means a continuation block (e.g. a
                // NOTE attached with `+`) already emitted `</p>`.
                let p_open = self.close_li_paragraph();
                output.push_str(if p_open { "</p></li>\n" } else { "</li>\n" });
            }
            TagEnd::Admonition => {
                self.admonition_block_stack.pop();
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
                let p_start = self.cell_p_start_stack.pop().unwrap_or(None);
                // For the styled (e/s/m) and default cells, an empty cell
                // (nothing written after the opening wrapper, so p_start still
                // equals the current length) rolls the whole wrapper back to a
                // bare <td></td>, matching asciidoctor. A non-empty multi-
                // paragraph cell never trips this: p_start points after the
                // first wrapper, far below the final length.
                let is_empty = p_start == Some(output.len());
                match style {
                    CellStyle::Emphasis => {
                        if is_empty {
                            output.truncate(output.len() - "<p class=\"tableblock\"><em>".len());
                        } else {
                            output.push_str("</em></p>");
                        }
                    }
                    CellStyle::Strong => {
                        if is_empty {
                            output.truncate(output.len() - "<p class=\"tableblock\"><strong>".len());
                        } else {
                            output.push_str("</strong></p>");
                        }
                    }
                    CellStyle::Monospace => {
                        if is_empty {
                            output.truncate(output.len() - "<p class=\"tableblock\"><code>".len());
                        } else {
                            output.push_str("</code></p>");
                        }
                    }
                    CellStyle::Literal => output.push_str("</pre></div>"),
                    CellStyle::AsciiDoc => {
                        // Nested block parse of the captured raw text, rendered
                        // through this same renderer so footnotes, xrefs and
                        // document attributes share the outer document's state.
                        // Pop BEFORE rendering: nested Text must not be captured.
                        let raw = self.acell_capture.pop().unwrap_or_default();
                        if !raw.is_empty() {
                            for ev in adoc_parser::Parser::new(&raw) {
                                self.push_event(output, ev);
                            }
                            // Blocks end with '\n'; asciidoctor puts none before
                            // the closing </div></td>.
                            if output.ends_with('\n') {
                                output.pop();
                            }
                        }
                        output.push_str("</div>");
                    }
                    _ => {
                        if is_empty {
                            // Empty cell: asciidoctor renders a bare <td></td>
                            // without the tableblock paragraph wrapper.
                            output.truncate(output.len() - "<p class=\"tableblock\">".len());
                        } else {
                            output.push_str("</p>");
                        }
                    }
                }
                // A body cell in a header (`h`) column closes with </th>.
                output.push_str(if matches!(style, CellStyle::Header) { "</th>\n" } else { "</td>\n" });
            }
            TagEnd::TableHeaderCell => {
                let style = self.cell_style_stack.pop().unwrap_or_default();
                self.cell_p_start_stack.pop();
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
}

/// Reference to an already-defined footnote: no anchor ids (those live on the
/// definition), `footnoteref` class on the sup.
fn push_footnote_ref(output: &mut String, num: usize) {
    output.push_str("<sup class=\"footnoteref\">[<a class=\"footnote\" href=\"#_footnotedef_");
    output.push_str(&num.to_string());
    output.push_str("\" title=\"View footnote.\">");
    output.push_str(&num.to_string());
    output.push_str("</a>]</sup>");
}
