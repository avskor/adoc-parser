use std::borrow::Cow;

use crate::attributes::BlockAttributes;
use crate::event::{
    AdmonitionKind, CowStr, DelimitedBlockKind, Event, Tag, TagEnd,
};
use crate::scanner;

#[derive(Debug, Clone)]
pub enum BlockContext {
    Section { level: u8 },
    DelimitedBlock { kind: scanner::DelimiterType, delimiter_len: usize, admonition_kind: Option<AdmonitionKind> },
    UnorderedList { depth: u8 },
    OrderedList { depth: u8 },
    ListItem { depth: u8 },
    DescriptionList { depth: u8 },
    DescriptionListEntry { depth: u8 },
    CalloutList,
}

pub struct BlockScanner<'a> {
    lines: Vec<&'a str>,
    pos: usize,
    context_stack: Vec<BlockContext>,
    event_buffer: Vec<Event<'a>>,
    pending_block_attrs: Option<BlockAttributes>,
    pending_block_title: Option<&'a str>,
    header_emitted: bool,
}

impl<'a> BlockScanner<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            lines: scanner::split_lines(input),
            pos: 0,
            context_stack: Vec::new(),
            event_buffer: Vec::new(),
            pending_block_attrs: None,
            pending_block_title: None,
            header_emitted: false,
        }
    }

    pub fn next_event(&mut self) -> Option<Event<'a>> {
        if let Some(ev) = self.event_buffer.pop() {
            return Some(ev);
        }

        self.scan_next_block()
    }

    fn push_event(&mut self, event: Event<'a>) {
        self.event_buffer.push(event);
    }

    fn current_line(&self) -> Option<&'a str> {
        self.lines.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn skip_blank_lines(&mut self) {
        while let Some(line) = self.current_line() {
            if scanner::is_blank(line) {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn close_sections_for_level(&mut self, level: u8) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        while let Some(ctx) = self.context_stack.last() {
            match ctx {
                BlockContext::Section { level: sec_level } if *sec_level >= level => {
                    events.push(Event::End(TagEnd::Section { level: *sec_level }));
                    self.context_stack.pop();
                }
                _ => break,
            }
        }
        events
    }

    fn close_all_open_contexts(&mut self) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        while let Some(ctx) = self.context_stack.pop() {
            match ctx {
                BlockContext::Section { level } => {
                    events.push(Event::End(TagEnd::Section { level }));
                }
                BlockContext::UnorderedList { .. } => {
                    events.push(Event::End(TagEnd::UnorderedList));
                }
                BlockContext::OrderedList { .. } => {
                    events.push(Event::End(TagEnd::OrderedList));
                }
                BlockContext::ListItem { .. } => {
                    events.push(Event::End(TagEnd::ListItem));
                }
                BlockContext::DelimitedBlock { admonition_kind, .. } => {
                    if admonition_kind.is_some() {
                        events.push(Event::End(TagEnd::Admonition));
                    } else {
                        events.push(Event::End(TagEnd::DelimitedBlock));
                    }
                }
                BlockContext::DescriptionList { .. } => {
                    events.push(Event::End(TagEnd::DescriptionList));
                }
                BlockContext::DescriptionListEntry { .. } => {
                    events.push(Event::End(TagEnd::DescriptionDescription));
                }
                BlockContext::CalloutList => {
                    events.push(Event::End(TagEnd::CalloutList));
                }
            }
        }
        events
    }

    fn take_pending_block_title(&mut self) -> Vec<Event<'a>> {
        if let Some(title) = self.pending_block_title.take() {
            vec![
                Event::Start(Tag::BlockTitle),
                Event::Text(Cow::Borrowed(title)),
                Event::End(TagEnd::BlockTitle),
            ]
        } else {
            Vec::new()
        }
    }

    fn push_title_then_events(&mut self, title_events: Vec<Event<'a>>) {
        // Title events should be emitted first (pushed on top of buffer)
        for ev in title_events.into_iter().rev() {
            self.push_event(ev);
        }
    }

    fn is_in_list_context(&self) -> bool {
        self.context_stack.iter().rev().any(|ctx| {
            matches!(ctx, BlockContext::ListItem { .. } | BlockContext::DescriptionListEntry { .. })
        })
    }

    fn scan_next_block(&mut self) -> Option<Event<'a>> {
        self.skip_blank_lines();

        let line = match self.current_line() {
            Some(l) => l,
            None => {
                let events = self.close_all_open_contexts();
                if !events.is_empty() {
                    for ev in events.iter().skip(1).rev() {
                        self.event_buffer.push(ev.clone());
                    }
                    return Some(events[0].clone());
                }
                return None;
            }
        };

        // Document header detection: first line is `= Title`
        if self.pos == 0 && !self.header_emitted
            && let Some((1, title)) = scanner::strip_section_marker(line) {
                return self.scan_document_header(title);
        }

        // Attribute entry `:name: value`
        if let Some((name, value)) = scanner::is_attribute_entry(line) {
            self.advance();
            return Some(Event::Attribute {
                name: Cow::Borrowed(name),
                value: Cow::Borrowed(value),
            });
        }

        // Block attribute `[...]`
        if let Some(attr_str) = scanner::is_block_attribute(line) {
            self.advance();
            self.pending_block_attrs = Some(BlockAttributes::parse(attr_str));
            return self.scan_next_block();
        }

        // Block title `.Title`
        if let Some(title) = scanner::is_block_title(line) {
            self.advance();
            self.pending_block_title = Some(title);
            return self.scan_next_block();
        }

        // Thematic break `'''`
        if scanner::is_thematic_break(line) {
            self.advance();
            return Some(Event::ThematicBreak);
        }

        // Page break `<<<`
        if scanner::is_page_break(line) {
            self.advance();
            return Some(Event::PageBreak);
        }

        // Section heading `== Title`
        if let Some((level, title)) = scanner::strip_section_marker(line) {
            return self.scan_section(level, title);
        }

        // TOC macro `toc::[]`
        if scanner::is_toc_macro(line) {
            self.advance();
            return Some(Event::Toc);
        }

        // Include directive `include::path[attrs]`
        if let Some((path, attrs)) = scanner::is_include_directive(line) {
            self.advance();
            return Some(Event::Include {
                path: Cow::Borrowed(path),
                attrs: Cow::Borrowed(attrs),
            });
        }

        // Block image `image::path[alt]`
        if let Some((target, alt)) = scanner::is_block_image(line) {
            self.advance();
            let title_events = self.take_pending_block_title();
            self.push_event(Event::End(TagEnd::BlockImage));
            self.push_event(Event::Start(Tag::BlockImage {
                target: Cow::Borrowed(target),
                alt: Cow::Borrowed(alt),
            }));
            self.push_title_then_events(title_events);
            return self.event_buffer.pop();
        }

        // Admonition `NOTE: text`
        if let Some((label, text)) = scanner::is_admonition(line) {
            return self.scan_admonition(label, text);
        }

        // Table `|===`
        if scanner::is_table_delimiter(line) {
            return self.scan_table();
        }

        // Delimited block
        if let Some((delim_type, delim_len)) = scanner::is_delimiter(line) {
            return self.scan_delimited_block(delim_type, delim_len);
        }

        // Single-line comment `// ...`
        if scanner::is_line_comment(line) {
            self.advance();
            return self.scan_next_block();
        }

        // Callout list
        if let Some((number, text)) = scanner::is_callout_list_item(line) {
            return self.scan_callout_list_item(number, text);
        }

        // Unordered list
        if let Some((depth, text)) = scanner::is_list_marker_unordered(line) {
            return self.scan_unordered_list_item(depth, text);
        }

        // Ordered list
        if let Some((depth, text)) = scanner::is_list_marker_ordered(line) {
            return self.scan_ordered_list_item(depth, text);
        }

        // Description list
        if let Some((depth, term, desc)) = scanner::is_description_list_marker(line) {
            return self.scan_description_list_item(depth, term, desc);
        }

        // List continuation `+`
        if scanner::is_list_continuation(line) {
            self.advance();
            if self.is_in_list_context() {
                return self.scan_next_block();
            }
            return self.scan_next_block();
        }

        // Literal paragraph (leading space)
        if line.starts_with(' ') || line.starts_with('\t') {
            return self.scan_literal_paragraph();
        }

        // Regular paragraph
        self.scan_paragraph()
    }

    fn scan_document_header(&mut self, title: &'a str) -> Option<Event<'a>> {
        self.header_emitted = true;
        self.advance();

        let id = scanner::generate_id(title);

        // Collect header content lines first
        let mut header_events: Vec<Event<'a>> = Vec::new();

        while let Some(line) = self.current_line() {
            if scanner::is_blank(line) {
                self.advance();
                break;
            }
            if let Some((name, value)) = scanner::is_attribute_entry(line) {
                header_events.push(Event::Attribute {
                    name: Cow::Borrowed(name),
                    value: Cow::Borrowed(value),
                });
                self.advance();
            } else {
                header_events.push(Event::Text(Cow::Borrowed(line)));
                self.advance();
            }
        }

        // Check if :toc: attribute is present in header events
        let has_toc = header_events.iter().any(|ev| {
            matches!(ev, Event::Attribute { name, .. } if name == "toc")
        });

        // Build buffer in reverse pop order:
        // Start(Header) -> Start(SectionTitle) -> Start(DocTitle) -> Text -> End(DocTitle) -> End(SectionTitle) -> [header content] -> End(Header) -> [Toc]
        // Toc is pushed first (bottom of stack), emitted last — right after End(Header)
        if has_toc {
            self.push_event(Event::Toc);
        }
        self.push_event(Event::End(TagEnd::Header));
        for ev in header_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_event(Event::End(TagEnd::SectionTitle));
        self.push_event(Event::End(TagEnd::DocumentTitle));
        self.push_event(Event::Text(Cow::Borrowed(title)));
        self.push_event(Event::Start(Tag::DocumentTitle));
        self.push_event(Event::Start(Tag::SectionTitle {
            level: 0,
            id: Cow::Owned(id),
        }));

        Some(Event::Start(Tag::Header))
    }

    fn scan_section(&mut self, level: u8, title: &'a str) -> Option<Event<'a>> {
        self.advance();
        let close_events = self.close_sections_for_level(level);

        let id = self.pending_block_attrs
            .as_ref()
            .and_then(|a| a.id.clone())
            .unwrap_or_else(|| scanner::generate_id(title));

        self.pending_block_attrs = None;
        let title_events = self.take_pending_block_title();

        self.context_stack.push(BlockContext::Section { level });

        // Buffer (bottom to top): section content, then close events, then title
        self.push_event(Event::End(TagEnd::SectionTitle));
        self.push_event(Event::Text(Cow::Borrowed(title)));
        self.push_event(Event::Start(Tag::SectionTitle {
            level,
            id: Cow::Owned(id),
        }));
        self.push_event(Event::Start(Tag::Section { level }));

        // Close events emitted before section opening
        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }

        // Title events emitted first
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    fn scan_table(&mut self) -> Option<Event<'a>> {
        self.advance(); // skip opening |===
        let title_events = self.take_pending_block_title();
        let block_attrs = self.pending_block_attrs.take().unwrap_or_default();

        // Collect lines until closing |=== or EOF
        let mut content_lines: Vec<&'a str> = Vec::new();
        while let Some(line) = self.current_line() {
            if scanner::is_table_delimiter(line) {
                self.advance();
                break;
            }
            content_lines.push(line);
            self.advance();
        }

        // Parse cells from content lines, tracking blank line positions
        let mut all_cells: Vec<&'a str> = Vec::new();
        let mut first_blank_after_first_row = false;
        let mut cells_before_blank: usize = 0;
        let mut found_first_data = false;

        for &line in &content_lines {
            if scanner::is_blank(line) {
                if found_first_data && !first_blank_after_first_row {
                    first_blank_after_first_row = true;
                    cells_before_blank = all_cells.len();
                }
                continue;
            }
            found_first_data = true;
            if let Some(cells) = scanner::parse_table_cells(line) {
                all_cells.extend(cells);
            }
        }

        if all_cells.is_empty() {
            self.push_title_then_events(title_events);
            return self.event_buffer.pop().or_else(|| self.scan_next_block());
        }

        // Determine number of columns: from cols attribute or first data line
        let num_cols = if let Some(n) = block_attrs.table_cols_count() {
            n
        } else {
            let mut cols = 0;
            for &line in &content_lines {
                if scanner::is_blank(line) {
                    continue;
                }
                if let Some(cells) = scanner::parse_table_cells(line) {
                    cols = cells.len();
                    break;
                }
            }
            if cols == 0 { 1 } else { cols }
        };

        // Determine header: %header option OR blank line after first row
        let has_header = block_attrs.has_option("header")
            || (first_blank_after_first_row && cells_before_blank == num_cols);

        // Determine footer: %footer option
        let has_footer = block_attrs.has_option("footer");

        // Split cells into header, body, footer
        let header_cells = if has_header {
            &all_cells[..num_cols.min(all_cells.len())]
        } else {
            &[][..]
        };
        let remaining = if has_header {
            &all_cells[num_cols.min(all_cells.len())..]
        } else {
            &all_cells[..]
        };
        // Footer is the last complete row of remaining cells
        let (body_cells, footer_cells) = if has_footer && remaining.len() >= num_cols {
            let footer_start = remaining.len() - (remaining.len() % num_cols).max(0);
            // Take the last full row
            let footer_start = if footer_start == remaining.len() {
                remaining.len() - num_cols
            } else {
                // Incomplete last row — no footer
                remaining.len()
            };
            (&remaining[..footer_start], &remaining[footer_start..])
        } else {
            (remaining, &[][..])
        };

        // Build events in reverse (buffer is a stack, pop from top)
        self.push_event(Event::End(TagEnd::Table));

        // TableFoot
        if !footer_cells.is_empty() {
            self.push_event(Event::End(TagEnd::TableFoot));
            for row in footer_cells.chunks(num_cols).rev() {
                self.push_event(Event::End(TagEnd::TableRow));
                for &cell in row.iter().rev() {
                    self.push_event(Event::End(TagEnd::TableCell));
                    self.push_event(Event::Text(Cow::Borrowed(cell)));
                    self.push_event(Event::Start(Tag::TableCell));
                }
                self.push_event(Event::Start(Tag::TableRow));
            }
            self.push_event(Event::Start(Tag::TableFoot));
        }

        // TableBody
        if !body_cells.is_empty() {
            self.push_event(Event::End(TagEnd::TableBody));
            for row in body_cells.chunks(num_cols).rev() {
                self.push_event(Event::End(TagEnd::TableRow));
                for &cell in row.iter().rev() {
                    self.push_event(Event::End(TagEnd::TableCell));
                    self.push_event(Event::Text(Cow::Borrowed(cell)));
                    self.push_event(Event::Start(Tag::TableCell));
                }
                self.push_event(Event::Start(Tag::TableRow));
            }
            self.push_event(Event::Start(Tag::TableBody));
        }

        // TableHead
        if has_header {
            self.push_event(Event::End(TagEnd::TableHead));
            self.push_event(Event::End(TagEnd::TableRow));
            for &cell in header_cells.iter().rev() {
                self.push_event(Event::End(TagEnd::TableHeaderCell));
                self.push_event(Event::Text(Cow::Borrowed(cell)));
                self.push_event(Event::Start(Tag::TableHeaderCell));
            }
            self.push_event(Event::Start(Tag::TableRow));
            self.push_event(Event::Start(Tag::TableHead));
        }

        self.push_event(Event::Start(Tag::Table));
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    fn scan_paragraph(&mut self) -> Option<Event<'a>> {
        let title_events = self.take_pending_block_title();
        let mut para_lines: Vec<&'a str> = Vec::new();

        while let Some(line) = self.current_line() {
            if scanner::is_blank(line)
                || scanner::strip_section_marker(line).is_some()
                || scanner::is_delimiter(line).is_some()
                || scanner::is_list_marker_unordered(line).is_some()
                || scanner::is_list_marker_ordered(line).is_some()
                || scanner::is_admonition(line).is_some()
                || scanner::is_block_image(line).is_some()
                || scanner::is_toc_macro(line)
                || scanner::is_include_directive(line).is_some()
                || scanner::is_thematic_break(line)
                || scanner::is_page_break(line)
                || scanner::is_attribute_entry(line).is_some()
                || scanner::is_block_attribute(line).is_some()
                || scanner::is_block_title(line).is_some()
                || scanner::is_line_comment(line)
                || scanner::is_description_list_marker(line).is_some()
                || scanner::is_callout_list_item(line).is_some()
                || scanner::is_list_continuation(line)
                || scanner::is_table_delimiter(line)
            {
                break;
            }
            para_lines.push(line);
            self.advance();
        }

        if para_lines.is_empty() {
            return self.scan_next_block();
        }

        // Check for admonition style from block attributes
        let admonition_kind = self.pending_block_attrs.as_ref()
            .and_then(|a| a.admonition_kind());

        if let Some(kind) = admonition_kind {
            self.push_event(Event::End(TagEnd::Admonition));
            self.push_event(Event::End(TagEnd::Paragraph));
            for (i, &pline) in para_lines.iter().enumerate().rev() {
                if i < para_lines.len() - 1 {
                    self.push_event(Event::SoftBreak);
                }
                self.push_event(Event::Text(Cow::Borrowed(pline)));
            }
            self.push_event(Event::Start(Tag::Paragraph));
            self.push_event(Event::Start(Tag::Admonition { kind }));
        } else {
            self.push_event(Event::End(TagEnd::Paragraph));
            for (i, &pline) in para_lines.iter().enumerate().rev() {
                if i < para_lines.len() - 1 {
                    self.push_event(Event::SoftBreak);
                }
                self.push_event(Event::Text(Cow::Borrowed(pline)));
            }
            self.push_event(Event::Start(Tag::Paragraph));
        }
        self.push_title_then_events(title_events);

        self.pending_block_attrs = None;
        self.event_buffer.pop()
    }

    fn scan_literal_paragraph(&mut self) -> Option<Event<'a>> {
        let title_events = self.take_pending_block_title();
        let mut lines: Vec<&'a str> = Vec::new();

        while let Some(line) = self.current_line() {
            if scanner::is_blank(line) || (!line.starts_with(' ') && !line.starts_with('\t')) {
                break;
            }
            lines.push(line);
            self.advance();
        }

        if lines.is_empty() {
            return self.scan_next_block();
        }

        // Compute minimum common indent
        let min_indent = lines.iter()
            .filter(|l| !l.is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);

        self.push_event(Event::End(TagEnd::LiteralParagraph));
        for (i, &pline) in lines.iter().enumerate().rev() {
            if i < lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            let stripped = if pline.len() >= min_indent { &pline[min_indent..] } else { pline };
            self.push_event(Event::Text(Cow::Borrowed(stripped)));
        }
        self.push_event(Event::Start(Tag::LiteralParagraph));
        self.push_title_then_events(title_events);

        self.pending_block_attrs = None;
        self.event_buffer.pop()
    }

    fn scan_admonition(&mut self, label: &'a str, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let kind = match label {
            "NOTE" => AdmonitionKind::Note,
            "TIP" => AdmonitionKind::Tip,
            "IMPORTANT" => AdmonitionKind::Important,
            "WARNING" => AdmonitionKind::Warning,
            "CAUTION" => AdmonitionKind::Caution,
            _ => AdmonitionKind::Note,
        };

        self.push_event(Event::End(TagEnd::Admonition));
        self.push_event(Event::End(TagEnd::Paragraph));
        self.push_event(Event::Text(Cow::Borrowed(text)));
        self.push_event(Event::Start(Tag::Paragraph));
        self.push_event(Event::Start(Tag::Admonition { kind }));
        self.push_title_then_events(title_events);

        self.pending_block_attrs = None;
        self.event_buffer.pop()
    }

    fn scan_delimited_block(
        &mut self,
        delim_type: scanner::DelimiterType,
        delim_len: usize,
    ) -> Option<Event<'a>> {
        self.advance(); // skip opening delimiter
        let title_events = self.take_pending_block_title();

        let block_attrs = self.pending_block_attrs.take().unwrap_or_default();

        // Check for source block
        if (delim_type == scanner::DelimiterType::Listing) && block_attrs.is_source_block() {
            let language = block_attrs.source_language().map(|l| Cow::Owned(l.to_string()));
            return self.scan_source_block(delim_type, delim_len, language, title_events);
        }

        // Comment block — skip content entirely
        if delim_type == scanner::DelimiterType::Comment {
            while let Some(line) = self.current_line() {
                self.advance();
                if let Some((dt, dl)) = scanner::is_delimiter(line)
                    && dt == delim_type && dl == delim_len {
                        break;
                }
            }
            return self.scan_next_block();
        }

        let kind = match delim_type {
            scanner::DelimiterType::Listing => DelimitedBlockKind::Listing,
            scanner::DelimiterType::Literal => DelimitedBlockKind::Literal,
            scanner::DelimiterType::Example => DelimitedBlockKind::Example,
            scanner::DelimiterType::Sidebar => DelimitedBlockKind::Sidebar,
            scanner::DelimiterType::Quote => DelimitedBlockKind::Quote,
            scanner::DelimiterType::Open => DelimitedBlockKind::Open,
            scanner::DelimiterType::Comment => DelimitedBlockKind::Comment,
            scanner::DelimiterType::Passthrough => DelimitedBlockKind::Passthrough,
        };

        let is_verbatim = matches!(
            kind,
            DelimitedBlockKind::Listing | DelimitedBlockKind::Literal | DelimitedBlockKind::Passthrough
        );

        if is_verbatim {
            let mut content_lines: Vec<&'a str> = Vec::new();
            let mut closed = false;
            while let Some(line) = self.current_line() {
                if let Some((dt, dl)) = scanner::is_delimiter(line)
                    && dt == delim_type && dl == delim_len {
                        self.advance();
                        closed = true;
                        break;
                }
                content_lines.push(line);
                self.advance();
            }

            // For unclosed blocks, trim trailing empty lines (artifacts of split_lines)
            if !closed {
                while content_lines.last().is_some_and(|l| l.is_empty()) {
                    content_lines.pop();
                }
            }

            // Handle single empty line in closed blocks: emit "\n" instead of ""
            if closed && content_lines.len() == 1 && content_lines[0].is_empty() {
                self.push_event(Event::End(TagEnd::DelimitedBlock));
                self.push_event(Event::Text(Cow::Borrowed("\n")));
                self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
                self.push_title_then_events(title_events);
                return self.event_buffer.pop();
            }

            // Push content (bottom of buffer)
            self.push_event(Event::End(TagEnd::DelimitedBlock));
            for (i, &cline) in content_lines.iter().enumerate().rev() {
                if i < content_lines.len() - 1 {
                    self.push_event(Event::SoftBreak);
                }
                self.push_event(Event::Text(Cow::Borrowed(cline)));
            }
            // Push Start on top of content
            self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
            // Push title events on very top (emitted first)
            self.push_title_then_events(title_events);

            return self.event_buffer.pop();
        }

        // Structural blocks (example, sidebar, quote, open): recursively parse content
        // Check for admonition style on Example blocks
        let adm_kind = if delim_type == scanner::DelimiterType::Example {
            block_attrs.admonition_kind()
        } else {
            None
        };

        self.context_stack.push(BlockContext::DelimitedBlock {
            kind: delim_type,
            delimiter_len: delim_len,
            admonition_kind: adm_kind.clone(),
        });
        if let Some(ak) = adm_kind {
            self.push_event(Event::Start(Tag::Admonition { kind: ak }));
        } else {
            self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
        }
        self.push_title_then_events(title_events);
        self.event_buffer.pop()
    }

    fn scan_source_block(
        &mut self,
        delim_type: scanner::DelimiterType,
        delim_len: usize,
        language: Option<CowStr<'a>>,
        title_events: Vec<Event<'a>>,
    ) -> Option<Event<'a>> {
        let mut content_lines: Vec<&'a str> = Vec::new();
        while let Some(line) = self.current_line() {
            if let Some((dt, dl)) = scanner::is_delimiter(line)
                && dt == delim_type && dl == delim_len {
                    self.advance();
                    break;
            }
            content_lines.push(line);
            self.advance();
        }

        self.push_event(Event::End(TagEnd::SourceBlock));
        for (i, &cline) in content_lines.iter().enumerate().rev() {
            if i < content_lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            let (stripped, callout_nums) = scanner::strip_callout_markers(cline);
            if callout_nums.is_empty() {
                self.push_event(Event::Text(Cow::Borrowed(cline)));
            } else {
                for &n in callout_nums.iter().rev() {
                    self.push_event(Event::CalloutRef(n));
                }
                self.push_event(Event::Text(Cow::Borrowed(stripped)));
            }
        }
        self.push_event(Event::Start(Tag::SourceBlock { language }));
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    fn close_list_items_for_depth(&mut self, target_depth: u8) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        loop {
            match self.context_stack.last() {
                Some(BlockContext::ListItem { depth }) if *depth >= target_depth => {
                    events.push(Event::End(TagEnd::ListItem));
                    self.context_stack.pop();
                }
                _ => break,
            }
        }
        events
    }

    fn is_in_list_at_depth(&self, depth: u8, unordered: bool) -> bool {
        for ctx in self.context_stack.iter().rev() {
            match ctx {
                BlockContext::UnorderedList { depth: d } if unordered && *d == depth => {
                    return true;
                }
                BlockContext::OrderedList { depth: d } if !unordered && *d == depth => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn close_description_entries_for_depth(&mut self, target_depth: u8) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        loop {
            match self.context_stack.last() {
                Some(BlockContext::DescriptionListEntry { depth }) if *depth >= target_depth => {
                    events.push(Event::End(TagEnd::DescriptionDescription));
                    self.context_stack.pop();
                }
                _ => break,
            }
        }
        events
    }

    fn is_in_description_list_at_depth(&self, depth: u8) -> bool {
        for ctx in self.context_stack.iter().rev() {
            match ctx {
                BlockContext::DescriptionList { depth: d } if *d == depth => return true,
                _ => {}
            }
        }
        false
    }

    fn scan_description_list_item(
        &mut self,
        depth: u8,
        term: &'a str,
        desc: &'a str,
    ) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let close_events = self.close_description_entries_for_depth(depth);

        let need_new_list = !self.is_in_description_list_at_depth(depth);

        if need_new_list {
            self.context_stack.push(BlockContext::DescriptionList { depth });
        }
        self.context_stack.push(BlockContext::DescriptionListEntry { depth });

        // Event buffer (bottom to top for FIFO via pop):
        if !desc.is_empty() {
            self.push_event(Event::Text(Cow::Borrowed(desc)));
        }
        self.push_event(Event::Start(Tag::DescriptionDescription));
        self.push_event(Event::End(TagEnd::DescriptionTerm));
        self.push_event(Event::Text(Cow::Borrowed(term)));
        self.push_event(Event::Start(Tag::DescriptionTerm));
        if need_new_list {
            self.push_event(Event::Start(Tag::DescriptionList));
        }

        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_title_then_events(title_events);

        self.pending_block_attrs = None;
        self.event_buffer.pop()
    }

    fn scan_unordered_list_item(&mut self, depth: u8, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let (checked, actual_text) = scanner::parse_checklist_marker(text);

        let close_events = self.close_list_items_for_depth(depth);

        let need_new_list = !self.is_in_list_at_depth(depth, true);

        if need_new_list {
            self.context_stack.push(BlockContext::UnorderedList { depth });
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(actual_text)));
            self.push_event(Event::Start(Tag::ListItem { depth, checked }));
            self.push_event(Event::Start(Tag::UnorderedList { has_checklist: checked.is_some() }));
        } else {
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(actual_text)));
            self.push_event(Event::Start(Tag::ListItem { depth, checked }));
        }

        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_title_then_events(title_events);

        self.pending_block_attrs = None;
        self.event_buffer.pop()
    }

    fn scan_ordered_list_item(&mut self, depth: u8, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let close_events = self.close_list_items_for_depth(depth);

        let need_new_list = !self.is_in_list_at_depth(depth, false);

        if need_new_list {
            self.context_stack.push(BlockContext::OrderedList { depth });
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(text)));
            self.push_event(Event::Start(Tag::ListItem { depth, checked: None }));
            self.push_event(Event::Start(Tag::OrderedList));
        } else {
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(text)));
            self.push_event(Event::Start(Tag::ListItem { depth, checked: None }));
        }

        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_title_then_events(title_events);

        self.pending_block_attrs = None;
        self.event_buffer.pop()
    }

    fn is_in_callout_list(&self) -> bool {
        self.context_stack.iter().rev().any(|ctx| matches!(ctx, BlockContext::CalloutList))
    }

    fn scan_callout_list_item(&mut self, number: u32, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let need_new_list = !self.is_in_callout_list();

        if need_new_list {
            self.context_stack.push(BlockContext::CalloutList);
        }

        // Buffer (bottom to top for FIFO via pop):
        self.push_event(Event::End(TagEnd::CalloutListItem));
        if !text.is_empty() {
            self.push_event(Event::Text(Cow::Borrowed(text)));
        }
        self.push_event(Event::Start(Tag::CalloutListItem { number }));
        if need_new_list {
            self.push_event(Event::Start(Tag::CalloutList));
        }

        self.push_title_then_events(title_events);

        self.pending_block_attrs = None;
        self.event_buffer.pop()
    }

    /// Check if we're inside a delimited block and if the current line closes it
    fn check_close_delimited_block(&mut self) -> bool {
        if let Some(line) = self.current_line()
            && let Some((delim_type, delim_len)) = scanner::is_delimiter(line) {
                // Check context stack for matching delimited block
                for (i, ctx) in self.context_stack.iter().enumerate().rev() {
                    if let BlockContext::DelimitedBlock { kind, delimiter_len, .. } = ctx
                        && *kind == delim_type && *delimiter_len == delim_len {
                            self.advance(); // consume delimiter
                            // Close everything up to and including this block
                            let mut events = Vec::new();
                            while self.context_stack.len() > i {
                                if let Some(ctx) = self.context_stack.pop() {
                                    match ctx {
                                        BlockContext::Section { level } => {
                                            events.push(Event::End(TagEnd::Section { level }));
                                        }
                                        BlockContext::DelimitedBlock { admonition_kind, .. } => {
                                            if admonition_kind.is_some() {
                                                events.push(Event::End(TagEnd::Admonition));
                                            } else {
                                                events.push(Event::End(TagEnd::DelimitedBlock));
                                            }
                                        }
                                        BlockContext::UnorderedList { .. } => {
                                            events.push(Event::End(TagEnd::UnorderedList));
                                        }
                                        BlockContext::OrderedList { .. } => {
                                            events.push(Event::End(TagEnd::OrderedList));
                                        }
                                        BlockContext::ListItem { .. } => {
                                            events.push(Event::End(TagEnd::ListItem));
                                        }
                                        BlockContext::DescriptionList { .. } => {
                                            events.push(Event::End(TagEnd::DescriptionList));
                                        }
                                        BlockContext::DescriptionListEntry { .. } => {
                                            events.push(Event::End(TagEnd::DescriptionDescription));
                                        }
                                        BlockContext::CalloutList => {
                                            events.push(Event::End(TagEnd::CalloutList));
                                        }
                                    }
                                }
                            }
                            for ev in events.into_iter().rev() {
                                self.event_buffer.push(ev);
                            }
                            return true;
                    }
                }
        }
        false
    }
}

impl<'a> Iterator for BlockScanner<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // Check if current line closes a delimited block
        if !self.event_buffer.is_empty() {
            return self.event_buffer.pop();
        }

        // Skip blank lines and check for delimited block closing
        self.skip_blank_lines();
        if self.check_close_delimited_block() {
            return self.event_buffer.pop();
        }

        self.next_event()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_paragraph() {
        let input = "Hello world.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Hello world.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_two_paragraphs() {
        let input = "First.\n\nSecond.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("First.")),
            Event::End(TagEnd::Paragraph),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Second.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_multiline_paragraph() {
        let input = "Line one\nLine two";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Line one")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("Line two")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_section() {
        let input = "== My Section\n\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Section { level: 2 }),
            Event::Start(Tag::SectionTitle { level: 2, id: Cow::Owned("_my_section".into()) }),
            Event::Text(Cow::Borrowed("My Section")),
            Event::End(TagEnd::SectionTitle),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
            Event::End(TagEnd::Section { level: 2 }),
        ]);
    }

    #[test]
    fn test_nested_sections() {
        let input = "== Level 2\n\n=== Level 3\n\n== Another Level 2";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Section { level: 2 }),
            Event::Start(Tag::SectionTitle { level: 2, id: Cow::Owned("_level_2".into()) }),
            Event::Text(Cow::Borrowed("Level 2")),
            Event::End(TagEnd::SectionTitle),
            Event::Start(Tag::Section { level: 3 }),
            Event::Start(Tag::SectionTitle { level: 3, id: Cow::Owned("_level_3".into()) }),
            Event::Text(Cow::Borrowed("Level 3")),
            Event::End(TagEnd::SectionTitle),
            Event::End(TagEnd::Section { level: 3 }),
            Event::End(TagEnd::Section { level: 2 }),
            Event::Start(Tag::Section { level: 2 }),
            Event::Start(Tag::SectionTitle { level: 2, id: Cow::Owned("_another_level_2".into()) }),
            Event::Text(Cow::Borrowed("Another Level 2")),
            Event::End(TagEnd::SectionTitle),
            Event::End(TagEnd::Section { level: 2 }),
        ]);
    }

    #[test]
    fn test_line_comment_skipped() {
        let input = "First.\n// this is a comment\nSecond.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("First.")),
            Event::End(TagEnd::Paragraph),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Second.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_line_comment_at_start() {
        let input = "// comment\nHello.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Hello.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_simple_description_list() {
        let input = "CPU:: The brain\nRAM:: Memory";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::DescriptionList),
            Event::Start(Tag::DescriptionTerm),
            Event::Text(Cow::Borrowed("CPU")),
            Event::End(TagEnd::DescriptionTerm),
            Event::Start(Tag::DescriptionDescription),
            Event::Text(Cow::Borrowed("The brain")),
            Event::End(TagEnd::DescriptionDescription),
            Event::Start(Tag::DescriptionTerm),
            Event::Text(Cow::Borrowed("RAM")),
            Event::End(TagEnd::DescriptionTerm),
            Event::Start(Tag::DescriptionDescription),
            Event::Text(Cow::Borrowed("Memory")),
            Event::End(TagEnd::DescriptionDescription),
            Event::End(TagEnd::DescriptionList),
        ]);
    }

    #[test]
    fn test_description_list_empty_desc() {
        let input = "Term::";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::DescriptionList),
            Event::Start(Tag::DescriptionTerm),
            Event::Text(Cow::Borrowed("Term")),
            Event::End(TagEnd::DescriptionTerm),
            Event::Start(Tag::DescriptionDescription),
            Event::End(TagEnd::DescriptionDescription),
            Event::End(TagEnd::DescriptionList),
        ]);
    }

    #[test]
    fn test_list_continuation_paragraph() {
        let input = "* item\n+\nContinued.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("item")),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Continued.")),
            Event::End(TagEnd::Paragraph),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);
    }

    #[test]
    fn test_list_continuation_multiple() {
        let input = "* item\n+\nPara one.\n+\nPara two.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("item")),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Para one.")),
            Event::End(TagEnd::Paragraph),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Para two.")),
            Event::End(TagEnd::Paragraph),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);
    }

    #[test]
    fn test_list_continuation_with_delimited_block() {
        let input = "* item\n+\n----\ncode\n----";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("item")),
            Event::Start(Tag::DelimitedBlock { kind: crate::event::DelimitedBlockKind::Listing }),
            Event::Text(Cow::Borrowed("code")),
            Event::End(TagEnd::DelimitedBlock),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);
    }

    #[test]
    fn test_line_comment_between_sections() {
        let input = "== Section\n// comment\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Section { level: 2 }),
            Event::Start(Tag::SectionTitle { level: 2, id: Cow::Owned("_section".into()) }),
            Event::Text(Cow::Borrowed("Section")),
            Event::End(TagEnd::SectionTitle),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
            Event::End(TagEnd::Section { level: 2 }),
        ]);
    }

    #[test]
    fn test_document_header() {
        let input = "= My Document\n:toc: left\n\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Header),
            Event::Start(Tag::SectionTitle { level: 0, id: Cow::Owned("_my_document".into()) }),
            Event::Start(Tag::DocumentTitle),
            Event::Text(Cow::Borrowed("My Document")),
            Event::End(TagEnd::DocumentTitle),
            Event::End(TagEnd::SectionTitle),
            Event::Attribute { name: Cow::Borrowed("toc"), value: Cow::Borrowed("left") },
            Event::End(TagEnd::Header),
            Event::Toc,
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_simple_table() {
        let input = "|===\n| A | B\n| C | D\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("D")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_with_header() {
        let input = "|===\n| Header 1 | Header 2\n\n| Cell 1 | Cell 2\n| Cell 3 | Cell 4\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableHead),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableHeaderCell),
            Event::Text(Cow::Borrowed("Header 1")),
            Event::End(TagEnd::TableHeaderCell),
            Event::Start(Tag::TableHeaderCell),
            Event::Text(Cow::Borrowed("Header 2")),
            Event::End(TagEnd::TableHeaderCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableHead),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("Cell 1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("Cell 2")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("Cell 3")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("Cell 4")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_single_column_per_line() {
        let input = "|===\n| A\n| B\n| C\n| D\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        // num_cols = 1 (first line has 1 cell), so 4 rows of 1 cell each
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("D")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_cols_attribute() {
        let input = "[cols=\"2\"]\n|===\n| A\n| B\n| C\n| D\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("D")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_header_option() {
        let input = "[%header]\n|===\n| H1 | H2\n| C1 | C2\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableHead),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableHeaderCell),
            Event::Text(Cow::Borrowed("H1")),
            Event::End(TagEnd::TableHeaderCell),
            Event::Start(Tag::TableHeaderCell),
            Event::Text(Cow::Borrowed("H2")),
            Event::End(TagEnd::TableHeaderCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableHead),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("C1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("C2")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_footer_option() {
        let input = "[%footer]\n|===\n| A | B\n| F1 | F2\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::Start(Tag::TableFoot),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("F1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("F2")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableFoot),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_header_footer_combined() {
        let input = "[%header,%footer]\n|===\n| H1 | H2\n| C1 | C2\n| F1 | F2\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableHead),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableHeaderCell),
            Event::Text(Cow::Borrowed("H1")),
            Event::End(TagEnd::TableHeaderCell),
            Event::Start(Tag::TableHeaderCell),
            Event::Text(Cow::Borrowed("H2")),
            Event::End(TagEnd::TableHeaderCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableHead),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("C1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("C2")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::Start(Tag::TableFoot),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("F1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell),
            Event::Text(Cow::Borrowed("F2")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableFoot),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_toc_event_after_header() {
        let input = "= My Document\n:toc:\n\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Header),
            Event::Start(Tag::SectionTitle { level: 0, id: Cow::Owned("_my_document".into()) }),
            Event::Start(Tag::DocumentTitle),
            Event::Text(Cow::Borrowed("My Document")),
            Event::End(TagEnd::DocumentTitle),
            Event::End(TagEnd::SectionTitle),
            Event::Attribute { name: Cow::Borrowed("toc"), value: Cow::Borrowed("") },
            Event::End(TagEnd::Header),
            Event::Toc,
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_include_directive() {
        let input = "include::chapter.adoc[]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Include {
                path: Cow::Borrowed("chapter.adoc"),
                attrs: Cow::Borrowed(""),
            },
        ]);
    }

    #[test]
    fn test_include_directive_with_attrs() {
        let input = "include::sub/file.adoc[leveloffset=+1]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Include {
                path: Cow::Borrowed("sub/file.adoc"),
                attrs: Cow::Borrowed("leveloffset=+1"),
            },
        ]);
    }

    #[test]
    fn test_include_breaks_paragraph() {
        let input = "Some text.\ninclude::file.adoc[]\nMore text.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Some text.")),
            Event::End(TagEnd::Paragraph),
            Event::Include {
                path: Cow::Borrowed("file.adoc"),
                attrs: Cow::Borrowed(""),
            },
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("More text.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_toc_macro() {
        let input = "toc::[]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![Event::Toc]);
    }

    #[test]
    fn test_no_toc_without_attribute() {
        let input = "= My Document\n\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(!events.contains(&Event::Toc));
    }

    #[test]
    fn test_source_block_with_callouts() {
        let input = "[source,ruby]\n----\nrequire 'sinatra' <1>\nget '/hi' do <2>\n  \"Hello World!\" <3>\nend\n----";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::SourceBlock { language: Some(Cow::Owned("ruby".into())) }),
            Event::Text(Cow::Borrowed("require 'sinatra' ")),
            Event::CalloutRef(1),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("get '/hi' do ")),
            Event::CalloutRef(2),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("  \"Hello World!\" ")),
            Event::CalloutRef(3),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("end")),
            Event::End(TagEnd::SourceBlock),
        ]);
    }

    #[test]
    fn test_source_block_multiple_callouts() {
        let input = "[source]\n----\ncode <1> <2>\n----";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::SourceBlock { language: None }),
            Event::Text(Cow::Borrowed("code ")),
            Event::CalloutRef(1),
            Event::CalloutRef(2),
            Event::End(TagEnd::SourceBlock),
        ]);
    }

    #[test]
    fn test_callout_list() {
        let input = "<1> Library import\n<2> URL mapping";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::CalloutList),
            Event::Start(Tag::CalloutListItem { number: 1 }),
            Event::Text(Cow::Borrowed("Library import")),
            Event::End(TagEnd::CalloutListItem),
            Event::Start(Tag::CalloutListItem { number: 2 }),
            Event::Text(Cow::Borrowed("URL mapping")),
            Event::End(TagEnd::CalloutListItem),
            Event::End(TagEnd::CalloutList),
        ]);
    }

    #[test]
    fn test_checklist_basic() {
        let input = "* [x] done\n* [ ] todo";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: true }),
            Event::Start(Tag::ListItem { depth: 1, checked: Some(true) }),
            Event::Text(Cow::Borrowed("done")),
            Event::End(TagEnd::ListItem),
            Event::Start(Tag::ListItem { depth: 1, checked: Some(false) }),
            Event::Text(Cow::Borrowed("todo")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);
    }

    #[test]
    fn test_checklist_mixed() {
        let input = "* [x] checked\n* regular\n* [ ] unchecked";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: true }),
            Event::Start(Tag::ListItem { depth: 1, checked: Some(true) }),
            Event::Text(Cow::Borrowed("checked")),
            Event::End(TagEnd::ListItem),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("regular")),
            Event::End(TagEnd::ListItem),
            Event::Start(Tag::ListItem { depth: 1, checked: Some(false) }),
            Event::Text(Cow::Borrowed("unchecked")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);
    }

    #[test]
    fn test_checklist_regular_list_no_checklist() {
        let input = "* item 1\n* item 2";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events[0], Event::Start(Tag::UnorderedList { has_checklist: false }));
    }
}
