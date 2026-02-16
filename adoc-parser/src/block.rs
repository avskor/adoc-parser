use std::borrow::Cow;

use crate::attributes::BlockAttributes;
use crate::event::{
    AdmonitionKind, CowStr, DelimitedBlockKind, Event, Tag, TagEnd,
};
use crate::scanner;

#[derive(Debug, Clone)]
pub enum BlockContext {
    Section { level: u8 },
    DelimitedBlock { kind: scanner::DelimiterType, delimiter_len: usize },
    UnorderedList { depth: u8 },
    OrderedList { depth: u8 },
    ListItem { depth: u8 },
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
                BlockContext::DelimitedBlock { .. } => {
                    events.push(Event::End(TagEnd::DelimitedBlock));
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
        if self.pos == 0 && !self.header_emitted {
            if let Some((1, title)) = scanner::strip_section_marker(line) {
                return self.scan_document_header(title);
            }
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

        // Delimited block
        if let Some((delim_type, delim_len)) = scanner::is_delimiter(line) {
            return self.scan_delimited_block(delim_type, delim_len);
        }

        // Unordered list
        if let Some((depth, text)) = scanner::is_list_marker_unordered(line) {
            return self.scan_unordered_list_item(depth, text);
        }

        // Ordered list
        if let Some((depth, text)) = scanner::is_list_marker_ordered(line) {
            return self.scan_ordered_list_item(depth, text);
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

        // Build buffer in reverse pop order:
        // Start(Header) -> Start(SectionTitle) -> Start(DocTitle) -> Text -> End(DocTitle) -> End(SectionTitle) -> [header content] -> End(Header)
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
                || scanner::is_thematic_break(line)
                || scanner::is_page_break(line)
                || scanner::is_attribute_entry(line).is_some()
                || scanner::is_block_attribute(line).is_some()
                || scanner::is_block_title(line).is_some()
            {
                break;
            }
            para_lines.push(line);
            self.advance();
        }

        if para_lines.is_empty() {
            return self.scan_next_block();
        }

        self.push_event(Event::End(TagEnd::Paragraph));
        for (i, &pline) in para_lines.iter().enumerate().rev() {
            if i < para_lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            self.push_event(Event::Text(Cow::Borrowed(pline)));
        }
        self.push_event(Event::Start(Tag::Paragraph));
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

        self.push_event(Event::End(TagEnd::LiteralParagraph));
        for (i, &pline) in lines.iter().enumerate().rev() {
            if i < lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            self.push_event(Event::Text(Cow::Borrowed(pline)));
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
                if let Some((dt, dl)) = scanner::is_delimiter(line) {
                    if dt == delim_type && dl == delim_len {
                        break;
                    }
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
            while let Some(line) = self.current_line() {
                if let Some((dt, dl)) = scanner::is_delimiter(line) {
                    if dt == delim_type && dl == delim_len {
                        self.advance();
                        break;
                    }
                }
                content_lines.push(line);
                self.advance();
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
        self.context_stack.push(BlockContext::DelimitedBlock {
            kind: delim_type,
            delimiter_len: delim_len,
        });
        self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
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
            if let Some((dt, dl)) = scanner::is_delimiter(line) {
                if dt == delim_type && dl == delim_len {
                    self.advance();
                    break;
                }
            }
            content_lines.push(line);
            self.advance();
        }

        self.push_event(Event::End(TagEnd::SourceBlock));
        for (i, &cline) in content_lines.iter().enumerate().rev() {
            if i < content_lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            self.push_event(Event::Text(Cow::Borrowed(cline)));
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

    fn scan_unordered_list_item(&mut self, depth: u8, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let close_events = self.close_list_items_for_depth(depth);

        let need_new_list = !self.is_in_list_at_depth(depth, true);

        if need_new_list {
            self.context_stack.push(BlockContext::UnorderedList { depth });
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(text)));
            self.push_event(Event::Start(Tag::ListItem { depth }));
            self.push_event(Event::Start(Tag::UnorderedList));
        } else {
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(text)));
            self.push_event(Event::Start(Tag::ListItem { depth }));
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
            self.push_event(Event::Start(Tag::ListItem { depth }));
            self.push_event(Event::Start(Tag::OrderedList));
        } else {
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(text)));
            self.push_event(Event::Start(Tag::ListItem { depth }));
        }

        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_title_then_events(title_events);

        self.pending_block_attrs = None;
        self.event_buffer.pop()
    }

    /// Check if we're inside a delimited block and if the current line closes it
    fn check_close_delimited_block(&mut self) -> bool {
        if let Some(line) = self.current_line() {
            if let Some((delim_type, delim_len)) = scanner::is_delimiter(line) {
                // Check context stack for matching delimited block
                for (i, ctx) in self.context_stack.iter().enumerate().rev() {
                    if let BlockContext::DelimitedBlock { kind, delimiter_len } = ctx {
                        if *kind == delim_type && *delimiter_len == delim_len {
                            self.advance(); // consume delimiter
                            // Close everything up to and including this block
                            let mut events = Vec::new();
                            while self.context_stack.len() > i {
                                if let Some(ctx) = self.context_stack.pop() {
                                    match ctx {
                                        BlockContext::Section { level } => {
                                            events.push(Event::End(TagEnd::Section { level }));
                                        }
                                        BlockContext::DelimitedBlock { .. } => {
                                            events.push(Event::End(TagEnd::DelimitedBlock));
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
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }
}
