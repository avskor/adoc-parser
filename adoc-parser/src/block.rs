use std::borrow::Cow;

use crate::attributes::{BlockAttributes, TableFormat};
use crate::event::{
    AdmonitionKind, CellStyle, CowStr, DelimitedBlockKind, Event, HAlign, SubstitutionSet, Tag,
    TagEnd, VAlign,
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
    CalloutListItem,
}

pub struct BlockScanner<'a> {
    lines: Vec<&'a str>,
    pos: usize,
    context_stack: Vec<BlockContext>,
    event_buffer: Vec<Event<'a>>,
    pending_block_attrs: Option<BlockAttributes>,
    pending_block_title: Option<&'a str>,
    header_emitted: bool,
    body_started: bool,
    in_continuation: bool,
    had_blank_line: bool,
    leveloffset: i32,
    idprefix: String,
    idseparator: String,
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
            body_started: false,
            in_continuation: false,
            had_blank_line: false,
            leveloffset: 0,
            idprefix: "_".to_string(),
            idseparator: "_".to_string(),
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

    /// After advancing past the attribute entry line, consume any continuation
    /// lines (trailing ` \` for soft wrap or ` + \` for hard wrap) and return
    /// the fully joined attribute value.
    fn read_multiline_attribute_value(&mut self, initial_value: &'a str) -> CowStr<'a> {
        let Some((prefix, mut is_hard)) = scanner::strip_line_continuation(initial_value) else {
            return Cow::Borrowed(initial_value);
        };
        let mut result = String::from(prefix);
        loop {
            let Some(next_line) = self.current_line() else {
                break;
            };
            let trimmed = next_line.trim();
            if is_hard {
                result.push('\n');
            } else {
                result.push(' ');
            }
            match scanner::strip_line_continuation(trimmed) {
                Some((part, next_hard)) => {
                    result.push_str(part);
                    is_hard = next_hard;
                    self.advance();
                }
                None => {
                    result.push_str(trimmed);
                    self.advance();
                    break;
                }
            }
        }
        Cow::Owned(result)
    }

    fn skip_blank_lines(&mut self) {
        while let Some(line) = self.current_line() {
            if scanner::is_blank(line) {
                self.advance();
                self.had_blank_line = true;
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
                BlockContext::CalloutListItem => {
                    events.push(Event::End(TagEnd::CalloutListItem));
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

    fn emit_block_metadata(&mut self, attrs: &BlockAttributes, default_subs: SubstitutionSet) {
        // Only emit style for positional[0] values that are NOT consumed for block type detection
        let style = attrs.positional.first()
            .filter(|s| !matches!(s.as_str(),
                "source" | "verse" | "stem" | "latexmath" | "asciimath"
                | "NOTE" | "TIP" | "IMPORTANT" | "WARNING" | "CAUTION"
                | "discrete" | "normal"
                | "csv" | "dsv" | "tsv"
            ))
            .map(|s| Cow::Owned(s.clone()));
        let subs = attrs.substitution_set(default_subs);
        // Pass through named attributes that are not consumed by the parser
        let named: Vec<(Cow<'_, str>, Cow<'_, str>)> = attrs.named.iter()
            .filter(|(k, _)| !matches!(k.as_str(), "cols" | "format" | "start" | "subs"))
            .map(|(k, v)| (Cow::Owned(k.clone()), Cow::Owned(v.clone())))
            .collect();
        if style.is_some() || attrs.id.is_some() || !attrs.roles.is_empty()
            || !attrs.options.is_empty() || !named.is_empty() || subs.is_some()
        {
            self.push_event(Event::BlockMetadata {
                style,
                id: attrs.id.as_ref().map(|s| Cow::Owned(s.clone())),
                roles: attrs.roles.iter().map(|s| Cow::Owned(s.clone())).collect(),
                options: attrs.options.iter().map(|s| Cow::Owned(s.clone())).collect(),
                named,
                subs,
            });
        }
    }

    fn push_title_then_events(&mut self, title_events: Vec<Event<'a>>) {
        // Title events should be emitted first (pushed on top of buffer)
        for ev in title_events.into_iter().rev() {
            self.push_event(ev);
        }
    }

    fn update_leveloffset(&mut self, value: &str) {
        let trimmed = value.trim();
        if let Some(rest) = trimmed.strip_prefix('+') {
            if let Ok(n) = rest.parse::<i32>() {
                self.leveloffset += n;
            }
        } else if trimmed.starts_with('-') {
            if let Ok(n) = trimmed.parse::<i32>() {
                self.leveloffset += n;
            }
        } else if let Ok(n) = trimmed.parse::<i32>() {
            self.leveloffset = n;
        }
    }

    fn update_id_settings(&mut self, name: &str, value: &str) {
        // Handle normal set: :idprefix: value
        if name == "idprefix" {
            self.idprefix = value.to_string();
        } else if name == "idseparator" {
            self.idseparator = value.to_string();
        }
        // Handle unset prefix form: :!name:
        else if name == "!idprefix" {
            self.idprefix = String::new();
        } else if name == "!idseparator" {
            self.idseparator = "_".to_string();
        }
        // Handle unset suffix form: :name!:
        else if name == "idprefix!" {
            self.idprefix = String::new();
        } else if name == "idseparator!" {
            self.idseparator = "_".to_string();
        }
    }

    fn is_in_list_context(&self) -> bool {
        self.context_stack.iter().rev().any(|ctx| {
            matches!(ctx, BlockContext::ListItem { .. } | BlockContext::DescriptionListEntry { .. } | BlockContext::CalloutListItem)
        })
    }

    /// Close all list-related contexts from the top of the stack.
    /// Returns close events in emission order.
    fn close_list_contexts(&mut self) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        while let Some(ctx) = self.context_stack.last() {
            match ctx {
                BlockContext::ListItem { .. } => {
                    events.push(Event::End(TagEnd::ListItem));
                    self.context_stack.pop();
                }
                BlockContext::UnorderedList { .. } => {
                    events.push(Event::End(TagEnd::UnorderedList));
                    self.context_stack.pop();
                }
                BlockContext::OrderedList { .. } => {
                    events.push(Event::End(TagEnd::OrderedList));
                    self.context_stack.pop();
                }
                BlockContext::DescriptionListEntry { .. } => {
                    events.push(Event::End(TagEnd::DescriptionDescription));
                    self.context_stack.pop();
                }
                BlockContext::DescriptionList { .. } => {
                    events.push(Event::End(TagEnd::DescriptionList));
                    self.context_stack.pop();
                }
                BlockContext::CalloutList => {
                    events.push(Event::End(TagEnd::CalloutList));
                    self.context_stack.pop();
                }
                BlockContext::CalloutListItem => {
                    events.push(Event::End(TagEnd::CalloutListItem));
                    self.context_stack.pop();
                }
                _ => break,
            }
        }
        events
    }

    /// Close nested list items/lists above the outermost ListItem.
    /// Used when `+` appears after blank lines to attach continuation to ancestor.
    fn close_nested_list_items(&mut self) -> Vec<Event<'a>> {
        // Find the outermost (lowest-index) ListItem/DescriptionListEntry/CalloutListItem
        let outermost = self.context_stack.iter().position(|ctx| {
            matches!(ctx, BlockContext::ListItem { .. } | BlockContext::DescriptionListEntry { .. } | BlockContext::CalloutListItem)
        });
        let outermost = match outermost {
            Some(idx) => idx,
            None => return Vec::new(),
        };
        // Close everything above the outermost list item
        let mut events = Vec::new();
        while self.context_stack.len() > outermost + 1 {
            if let Some(ctx) = self.context_stack.pop() {
                match ctx {
                    BlockContext::ListItem { .. } => {
                        events.push(Event::End(TagEnd::ListItem));
                    }
                    BlockContext::UnorderedList { .. } => {
                        events.push(Event::End(TagEnd::UnorderedList));
                    }
                    BlockContext::OrderedList { .. } => {
                        events.push(Event::End(TagEnd::OrderedList));
                    }
                    BlockContext::DescriptionListEntry { .. } => {
                        events.push(Event::End(TagEnd::DescriptionDescription));
                    }
                    BlockContext::DescriptionList { .. } => {
                        events.push(Event::End(TagEnd::DescriptionList));
                    }
                    BlockContext::CalloutList => {
                        events.push(Event::End(TagEnd::CalloutList));
                    }
                    BlockContext::CalloutListItem => {
                        events.push(Event::End(TagEnd::CalloutListItem));
                    }
                    other => {
                        // Put it back — don't close non-list contexts
                        self.context_stack.push(other);
                        break;
                    }
                }
            }
        }
        events
    }

    /// Close all contexts above the matching parent list at the target depth.
    /// Used when a new list item at a given depth should return to an existing parent list,
    /// even when there are interleaved DL/other list contexts above it.
    fn close_to_parent_list(&mut self, target_depth: u8, unordered: bool) -> Vec<Event<'a>> {
        let target_pos = self.context_stack.iter().rposition(|ctx| match ctx {
            BlockContext::UnorderedList { depth } if unordered && *depth == target_depth => true,
            BlockContext::OrderedList { depth } if !unordered && *depth == target_depth => true,
            _ => false,
        });
        let target_pos = match target_pos {
            Some(pos) => pos,
            None => return Vec::new(),
        };

        let mut events = Vec::new();
        while self.context_stack.len() > target_pos + 1 {
            match self.context_stack.pop() {
                Some(BlockContext::ListItem { .. }) => events.push(Event::End(TagEnd::ListItem)),
                Some(BlockContext::UnorderedList { .. }) => events.push(Event::End(TagEnd::UnorderedList)),
                Some(BlockContext::OrderedList { .. }) => events.push(Event::End(TagEnd::OrderedList)),
                Some(BlockContext::DescriptionListEntry { .. }) => events.push(Event::End(TagEnd::DescriptionDescription)),
                Some(BlockContext::DescriptionList { .. }) => events.push(Event::End(TagEnd::DescriptionList)),
                Some(BlockContext::CalloutListItem) => events.push(Event::End(TagEnd::CalloutListItem)),
                Some(BlockContext::CalloutList) => events.push(Event::End(TagEnd::CalloutList)),
                Some(other) => {
                    self.context_stack.push(other);
                    break;
                }
                None => break,
            }
        }
        events
    }

    fn scan_next_block(&mut self) -> Option<Event<'a>> {
        self.skip_blank_lines();

        // Check if current line closes a delimited block
        if self.check_close_delimited_block() {
            // Discard pending block attributes — they have no target block
            self.pending_block_attrs = None;
            return self.event_buffer.pop();
        }

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

        // Document header detection: `= Title` before any body content
        // Skip if [discrete] attribute is pending — treat as discrete heading instead
        if !self.header_emitted && !self.body_started
            && let Some((1, title)) = scanner::strip_section_marker(line)
            && !self.pending_block_attrs.as_ref().is_some_and(|a| {
                a.positional.first().is_some_and(|s| s == "discrete")
            })
        {
                return self.scan_document_header(title);
        }

        // Attribute-only document header: starts with attribute entries but no `= Title` yet
        if !self.header_emitted && !self.body_started
            && scanner::is_attribute_entry(line).is_some()
        {
            return self.scan_attribute_only_header();
        }

        // Block attribute `[...]` — checked before body_started to allow metadata before header
        if let Some(attr_str) = scanner::is_block_attribute(line) {
            // If in list context with preceding blank line (not via continuation), close list
            if self.is_in_list_context() && !self.in_continuation && self.had_blank_line {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return self.event_buffer.pop();
            }
            self.advance();
            self.had_blank_line = false;
            self.pending_block_attrs = Some(BlockAttributes::parse(attr_str));
            return self.scan_next_block();
        }

        // Block title `.Title` — checked before body_started
        if let Some(title) = scanner::is_block_title(line) {
            self.advance();
            self.pending_block_title = Some(title);
            return self.scan_next_block();
        }

        // From here on, we're in the document body
        self.body_started = true;

        // Attribute entry `:name: value`
        if let Some((name, value)) = scanner::is_attribute_entry(line) {
            self.advance();
            let value = self.read_multiline_attribute_value(value);
            if name == "leveloffset" {
                self.update_leveloffset(&value);
                return Some(Event::Attribute {
                    name: Cow::Borrowed(name),
                    value: Cow::Owned(self.leveloffset.to_string()),
                });
            }
            self.update_id_settings(name, &value);
            return Some(Event::Attribute {
                name: Cow::Borrowed(name),
                value,
            });
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
            let block_attrs = self.pending_block_attrs.take();
            let img_attrs = crate::attributes::parse_image_attrs(alt);
            self.push_event(Event::End(TagEnd::BlockImage));
            self.push_event(Event::Start(Tag::BlockImage {
                target: Cow::Borrowed(target),
                alt: Cow::Borrowed(img_attrs.alt),
                width: img_attrs.width.map(Cow::Borrowed),
                height: img_attrs.height.map(Cow::Borrowed),
            }));
            if let Some(ref attrs) = block_attrs {
                self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
            }
            self.push_title_then_events(title_events);
            return self.event_buffer.pop();
        }

        // Block video `video::path[attrs]`
        if let Some((target, attrs)) = scanner::is_block_video(line) {
            self.advance();
            let title_events = self.take_pending_block_title();
            let block_attrs = self.pending_block_attrs.take();
            self.push_event(Event::End(TagEnd::BlockVideo));
            self.push_event(Event::Start(Tag::BlockVideo {
                target: Cow::Borrowed(target),
                attrs: Cow::Borrowed(attrs),
            }));
            if let Some(ref attrs) = block_attrs {
                self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
            }
            self.push_title_then_events(title_events);
            return self.event_buffer.pop();
        }

        // Block audio `audio::path[attrs]`
        if let Some((target, attrs)) = scanner::is_block_audio(line) {
            self.advance();
            let title_events = self.take_pending_block_title();
            let block_attrs = self.pending_block_attrs.take();
            self.push_event(Event::End(TagEnd::BlockAudio));
            self.push_event(Event::Start(Tag::BlockAudio {
                target: Cow::Borrowed(target),
                attrs: Cow::Borrowed(attrs),
            }));
            if let Some(ref attrs) = block_attrs {
                self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
            }
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
            // If in list context (and not via list continuation), close list first
            if self.is_in_list_context() && !self.in_continuation {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return self.event_buffer.pop();
            }
            self.in_continuation = false;
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
            if self.in_continuation {
                // Already in continuation mode — treat `+` as paragraph text
                self.advance();
                let mut para_lines: Vec<&'a str> = vec!["+"];
                while let Some(next_line) = self.current_line() {
                    if self.is_list_continuation_line(next_line) {
                        para_lines.push(next_line);
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.push_event(Event::End(TagEnd::Paragraph));
                for (i, &pline) in para_lines.iter().enumerate().rev() {
                    if i < para_lines.len() - 1 {
                        self.push_event(Event::SoftBreak);
                    }
                    self.push_event(Event::Text(Cow::Borrowed(pline)));
                }
                self.push_event(Event::Start(Tag::Paragraph));
                return self.event_buffer.pop();
            }
            // When `+` appears after blank lines, close nested lists so
            // continuation attaches to the outermost ancestor list item
            if self.had_blank_line && self.is_in_list_context() {
                let close_events = self.close_nested_list_items();
                if !close_events.is_empty() {
                    for ev in close_events.into_iter().rev() {
                        self.push_event(ev);
                    }
                    // Don't consume `+` yet — after nested lists are closed,
                    // next iteration will re-encounter `+` and handle it normally
                    return self.event_buffer.pop();
                }
            }
            self.advance();
            if self.is_in_list_context() {
                self.in_continuation = true;
                self.had_blank_line = false;
                let mut attr_events = Vec::new();
                loop {
                    match self.scan_next_block() {
                        Some(event) if matches!(&event, Event::Attribute { .. }) => {
                            attr_events.push(event);
                        }
                        other => {
                            self.in_continuation = false;
                            if attr_events.is_empty() {
                                return other;
                            }
                            // Buffer: block event + remaining attrs (reverse for FIFO)
                            if let Some(evt) = other {
                                self.event_buffer.push(evt);
                            }
                            for attr in attr_events.drain(1..).rev() {
                                self.event_buffer.push(attr);
                            }
                            return attr_events.into_iter().next();
                        }
                    }
                }
            }
            // Outside list context, emit as a single-line paragraph
            self.push_event(Event::End(TagEnd::Paragraph));
            self.push_event(Event::Text(Cow::Borrowed("+")));
            self.push_event(Event::Start(Tag::Paragraph));
            return self.event_buffer.pop();
        }

        // Literal paragraph (leading space), unless [normal] style overrides
        if line.starts_with(' ') || line.starts_with('\t') {
            let is_normal_style = self
                .pending_block_attrs
                .as_ref()
                .is_some_and(|a| a.positional.first().is_some_and(|s| s == "normal"));
            if is_normal_style {
                return self.scan_normal_indented_paragraph();
            }
            return self.scan_literal_paragraph();
        }

        // Regular paragraph
        // If in list context with preceding blank line or pending block attrs (not continuation),
        // close list first — a blank line followed by non-list content ends the list
        if self.is_in_list_context() && !self.in_continuation
            && (self.pending_block_attrs.is_some() || self.had_blank_line)
        {
            let close_events = self.close_list_contexts();
            for ev in close_events.into_iter().rev() {
                self.push_event(ev);
            }
            return self.event_buffer.pop();
        }
        self.scan_paragraph()
    }

    fn scan_document_header(&mut self, title: &'a str) -> Option<Event<'a>> {
        self.header_emitted = true;
        self.advance();

        let id = scanner::generate_id(title, &self.idprefix, &self.idseparator);

        // Collect header content lines first
        let mut header_events: Vec<Event<'a>> = Vec::new();

        // Check for author line: next line that is not blank, not attribute entry, not section marker
        if let Some(line) = self.current_line()
            && !scanner::is_blank(line)
            && scanner::is_attribute_entry(line).is_none()
            && scanner::strip_section_marker(line).is_none()
        {
            // Parse as author line
            let authors = scanner::parse_authors(line);
            for author in authors {
                header_events.push(Event::Author {
                    fullname: Cow::Borrowed(author.fullname),
                    firstname: Cow::Borrowed(author.firstname),
                    middlename: Cow::Borrowed(author.middlename),
                    lastname: Cow::Borrowed(author.lastname),
                    initials: Cow::Owned(author.initials),
                    address: Cow::Borrowed(author.address),
                });
            }
            self.advance();

            // Check for revision line after author line
            if let Some(rev_line) = self.current_line()
                && !scanner::is_blank(rev_line)
                && scanner::is_attribute_entry(rev_line).is_none()
                && scanner::strip_section_marker(rev_line).is_none()
                && let Some(rev_info) = scanner::parse_revision_line(rev_line)
            {
                header_events.push(Event::Revision {
                    version: Cow::Borrowed(rev_info.version),
                    date: Cow::Borrowed(rev_info.date),
                    remark: Cow::Borrowed(rev_info.remark),
                });
                if !rev_info.version.is_empty() {
                    header_events.push(Event::Attribute {
                        name: Cow::Borrowed("revnumber"),
                        value: Cow::Borrowed(rev_info.version),
                    });
                }
                if !rev_info.date.is_empty() {
                    header_events.push(Event::Attribute {
                        name: Cow::Borrowed("revdate"),
                        value: Cow::Borrowed(rev_info.date),
                    });
                }
                if !rev_info.remark.is_empty() {
                    header_events.push(Event::Attribute {
                        name: Cow::Borrowed("revremark"),
                        value: Cow::Borrowed(rev_info.remark),
                    });
                }
                self.advance();
            }
        }

        while let Some(line) = self.current_line() {
            if scanner::is_blank(line) {
                self.advance();
                break;
            }
            if let Some((name, value)) = scanner::is_attribute_entry(line) {
                self.advance();
                let value = self.read_multiline_attribute_value(value);
                if name == "leveloffset" {
                    self.update_leveloffset(&value);
                }
                self.update_id_settings(name, &value);
                header_events.push(Event::Attribute {
                    name: Cow::Borrowed(name),
                    value,
                });
            } else {
                // Non-attribute, non-blank line ends the header
                break;
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

    /// Scan an attribute-only document header (no `= Title`).
    /// Collects leading attribute entries and wraps them in Header events.
    /// If a `= Title` is encountered after attributes, transitions to a full document header.
    fn scan_attribute_only_header(&mut self) -> Option<Event<'a>> {
        self.header_emitted = true;
        let mut header_events: Vec<Event<'a>> = Vec::new();

        while let Some(line) = self.current_line() {
            if scanner::is_blank(line) {
                self.advance();
                // After blank, check if next non-blank line is `= Title`
                continue;
            }
            if let Some((name, value)) = scanner::is_attribute_entry(line) {
                self.advance();
                let value = self.read_multiline_attribute_value(value);
                if name == "leveloffset" {
                    self.update_leveloffset(&value);
                }
                self.update_id_settings(name, &value);
                header_events.push(Event::Attribute {
                    name: Cow::Borrowed(name),
                    value,
                });
            } else if let Some((1, title)) = scanner::strip_section_marker(line)
                && self.leveloffset == 0
            {
                // Found `= Title` after attributes — transition to full document header
                self.advance();
                return self.scan_document_header_with_pre_attrs(title, header_events);
            } else {
                break;
            }
        }

        self.push_event(Event::End(TagEnd::Header));
        for ev in header_events.into_iter().rev() {
            self.push_event(ev);
        }

        Some(Event::Start(Tag::Header))
    }

    /// Scan a document header that has attribute entries collected before the `= Title`.
    fn scan_document_header_with_pre_attrs(
        &mut self,
        title: &'a str,
        pre_attrs: Vec<Event<'a>>,
    ) -> Option<Event<'a>> {
        let id = scanner::generate_id(title, &self.idprefix, &self.idseparator);

        // Collect header content lines after the title
        let mut header_events: Vec<Event<'a>> = Vec::new();

        while let Some(line) = self.current_line() {
            if scanner::is_blank(line) {
                self.advance();
                break;
            }
            if let Some((name, value)) = scanner::is_attribute_entry(line) {
                self.advance();
                let value = self.read_multiline_attribute_value(value);
                header_events.push(Event::Attribute {
                    name: Cow::Borrowed(name),
                    value,
                });
            } else {
                header_events.push(Event::Text(Cow::Borrowed(line)));
                self.advance();
            }
        }

        // Check if :toc: is in any of the events
        let has_toc = pre_attrs.iter().chain(header_events.iter()).any(|ev| {
            matches!(ev, Event::Attribute { name, .. } if name == "toc")
        });

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
        // Pre-attributes go before the section title
        for ev in pre_attrs.into_iter().rev() {
            self.push_event(ev);
        }

        Some(Event::Start(Tag::Header))
    }

    fn is_inside_delimited_block(&self) -> bool {
        self.context_stack.iter().any(|ctx| matches!(ctx, BlockContext::DelimitedBlock { .. }))
    }

    fn scan_section(&mut self, level: u8, title: &'a str) -> Option<Event<'a>> {
        let is_discrete = self.pending_block_attrs.as_ref().is_some_and(|a| {
            a.positional.first().is_some_and(|s| s == "discrete")
        });
        let inside_delimited = self.is_inside_delimited_block();

        if is_discrete || inside_delimited {
            return self.scan_discrete_heading(level, title);
        }

        // Apply leveloffset to section level
        let effective_level = (level as i32 + self.leveloffset).max(1) as u8;

        self.advance();
        let list_close_events = self.close_list_contexts();
        let close_events = self.close_sections_for_level(effective_level);

        let id = self.pending_block_attrs
            .as_ref()
            .and_then(|a| a.id.clone())
            .unwrap_or_else(|| scanner::generate_id(title, &self.idprefix, &self.idseparator));

        let block_attrs = self.pending_block_attrs.take();
        let title_events = self.take_pending_block_title();

        self.context_stack.push(BlockContext::Section { level: effective_level });

        // Buffer (bottom to top): section content, then close events, then title
        self.push_event(Event::End(TagEnd::SectionTitle));
        self.push_event(Event::Text(Cow::Borrowed(title)));
        self.push_event(Event::Start(Tag::SectionTitle {
            level: effective_level,
            id: Cow::Owned(id),
        }));
        self.push_event(Event::Start(Tag::Section { level: effective_level }));
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }

        // Close events emitted before section opening
        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }

        // List close events emitted before section close events
        for ev in list_close_events.into_iter().rev() {
            self.push_event(ev);
        }

        // Title events emitted first
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    fn scan_discrete_heading(&mut self, level: u8, title: &'a str) -> Option<Event<'a>> {
        self.advance();
        let block_attrs = self.pending_block_attrs.take();
        let title_events = self.take_pending_block_title();

        // Apply leveloffset to heading level
        let effective_level = (level as i32 + self.leveloffset).max(1) as u8;

        // Emit: Start(Heading) Text(title) End(Heading) — no context push
        self.push_event(Event::End(TagEnd::Heading { level: effective_level }));
        self.push_event(Event::Text(Cow::Borrowed(title)));
        self.push_event(Event::Start(Tag::Heading { level: effective_level }));
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }
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

        // Check for CSV/DSV/TSV format
        let format = block_attrs.table_format();
        if format != TableFormat::Native {
            return self.scan_delimited_format_table(&content_lines, block_attrs, format, title_events);
        }

        // Parse cells from content lines, tracking blank line positions
        let mut all_cells: Vec<scanner::CellSpec<'a>> = Vec::new();
        let mut first_blank_after_first_row = false;
        let mut cells_before_blank_col_width: usize = 0;
        let mut found_first_data = false;

        for &line in &content_lines {
            if scanner::is_blank(line) {
                if found_first_data && !first_blank_after_first_row {
                    first_blank_after_first_row = true;
                    // Sum of colspan values for cells before blank
                    cells_before_blank_col_width = all_cells
                        .iter()
                        .map(|c| c.colspan as usize)
                        .sum();
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
            // Sum colspan of first row's cells
            let mut cols = 0;
            for &line in &content_lines {
                if scanner::is_blank(line) {
                    continue;
                }
                if let Some(cells) = scanner::parse_table_cells(line) {
                    cols = cells.iter().map(|c| c.colspan as usize).sum();
                    break;
                }
            }
            if cols == 0 { 1 } else { cols }
        };

        // Determine header: %header option OR blank line after first row
        let has_header = block_attrs.has_option("header")
            || (first_blank_after_first_row && cells_before_blank_col_width == num_cols);

        // Determine footer: %footer option
        let has_footer = block_attrs.has_option("footer");

        // Get column specs for alignment defaults
        let col_specs = block_attrs.table_col_specs();

        // Build rows using grid-aware placement (respects colspan/rowspan)
        let rows = Self::build_table_rows(&all_cells, num_cols);

        // Split rows into header, body, footer
        let header_rows = if has_header && !rows.is_empty() {
            &rows[..1]
        } else {
            &[][..]
        };
        let remaining_rows = if has_header && !rows.is_empty() {
            &rows[1..]
        } else {
            &rows[..]
        };
        let (body_rows, footer_rows) = if has_footer && !remaining_rows.is_empty() {
            let split = remaining_rows.len() - 1;
            (&remaining_rows[..split], &remaining_rows[split..])
        } else {
            (remaining_rows, &[][..])
        };

        // Resolve alignment for a cell: cell-level overrides column-level defaults.
        // A cell's alignment is "default" if not explicitly set by the cell spec.
        let resolve_align = |cell: &scanner::CellSpec<'_>, col_idx: usize| -> (HAlign, VAlign) {
            let mut halign = cell.halign;
            let mut valign = cell.valign;
            if let Some(ref specs) = col_specs
                && col_idx < specs.len()
            {
                if halign == HAlign::Left && cell.halign == HAlign::Left {
                    // Only use column default if cell didn't explicitly set alignment.
                    // We can't distinguish "explicitly set to Left" from "default Left",
                    // so we always apply column default when cell is Left (the default).
                    halign = specs[col_idx].halign;
                }
                if valign == VAlign::Top && cell.valign == VAlign::Top {
                    valign = specs[col_idx].valign;
                }
            }
            (halign, valign)
        };

        // Build events in reverse (buffer is a stack, pop from top)
        self.push_event(Event::End(TagEnd::Table));

        // Helper closure to emit cell events
        // Note: rows are iterated in reverse, so col_idx must be computed per-row
        macro_rules! emit_row_cells {
            ($row:expr, $is_header_section:expr) => {
                // First pass: compute column indices for each cell
                let mut col_indices = Vec::with_capacity($row.len());
                let mut col_idx: usize = 0;
                for cell in $row.iter() {
                    col_indices.push(col_idx);
                    col_idx += cell.colspan as usize;
                }
                // Emit in reverse order (buffer is a stack)
                for (ci, cell) in $row.iter().enumerate().rev() {
                    let (halign, valign) = resolve_align(cell, col_indices[ci]);
                    if cell.style == CellStyle::Header || $is_header_section {
                        self.push_event(Event::End(TagEnd::TableHeaderCell));
                        self.push_event(Event::Text(Cow::Borrowed(cell.content)));
                        self.push_event(Event::Start(Tag::TableHeaderCell {
                            colspan: cell.colspan,
                            rowspan: cell.rowspan,
                            style: cell.style,
                            halign,
                            valign,
                        }));
                    } else {
                        self.push_event(Event::End(TagEnd::TableCell));
                        self.push_event(Event::Text(Cow::Borrowed(cell.content)));
                        self.push_event(Event::Start(Tag::TableCell {
                            colspan: cell.colspan,
                            rowspan: cell.rowspan,
                            style: cell.style,
                            halign,
                            valign,
                        }));
                    }
                }
            };
        }

        // TableFoot
        if !footer_rows.is_empty() {
            self.push_event(Event::End(TagEnd::TableFoot));
            for row in footer_rows.iter().rev() {
                self.push_event(Event::End(TagEnd::TableRow));
                emit_row_cells!(row, false);
                self.push_event(Event::Start(Tag::TableRow));
            }
            self.push_event(Event::Start(Tag::TableFoot));
        }

        // TableBody
        if !body_rows.is_empty() {
            self.push_event(Event::End(TagEnd::TableBody));
            for row in body_rows.iter().rev() {
                self.push_event(Event::End(TagEnd::TableRow));
                emit_row_cells!(row, false);
                self.push_event(Event::Start(Tag::TableRow));
            }
            self.push_event(Event::Start(Tag::TableBody));
        }

        // TableHead
        if !header_rows.is_empty() {
            self.push_event(Event::End(TagEnd::TableHead));
            for row in header_rows.iter().rev() {
                self.push_event(Event::End(TagEnd::TableRow));
                emit_row_cells!(row, true);
                self.push_event(Event::Start(Tag::TableRow));
            }
            self.push_event(Event::Start(Tag::TableHead));
        }

        self.push_event(Event::Start(Tag::Table));
        self.emit_block_metadata(&block_attrs, SubstitutionSet::NORMAL);
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    /// Build table rows from a flat list of CellSpecs, respecting colspan/rowspan.
    fn build_table_rows(
        cells: &[scanner::CellSpec<'a>],
        num_cols: usize,
    ) -> Vec<Vec<scanner::CellSpec<'a>>> {
        let mut rows: Vec<Vec<scanner::CellSpec<'a>>> = Vec::new();
        // Track how many more rows each column is occupied by a rowspan
        let mut col_remaining: Vec<u8> = vec![0; num_cols];
        let mut current_row: Vec<scanner::CellSpec<'a>> = Vec::new();
        let mut col: usize = 0;

        for cell in cells {
            // Skip columns occupied by rowspan from previous rows
            while col < num_cols && col_remaining[col] > 0 {
                col_remaining[col] -= 1;
                col += 1;
            }

            // If we've filled the row, start a new one
            if col >= num_cols {
                rows.push(std::mem::take(&mut current_row));
                col = 0;
                // Decrement remaining rowspans for the new row
                for r in &mut col_remaining {
                    if *r > 0 {
                        *r -= 1;
                    }
                }
                // Skip columns occupied by rowspan
                while col < num_cols && col_remaining[col] > 0 {
                    col_remaining[col] -= 1;
                    col += 1;
                }
            }

            current_row.push(cell.clone());

            // Mark columns occupied by this cell's colspan and rowspan
            let span = (cell.colspan as usize).min(num_cols - col);
            if cell.rowspan > 1 {
                for r in col_remaining.iter_mut().skip(col).take(span) {
                    *r = cell.rowspan - 1;
                }
            }
            col += span;
        }

        // Push the last row if it has any cells
        if !current_row.is_empty() {
            rows.push(current_row);
        }

        rows
    }

    fn scan_delimited_format_table(
        &mut self,
        content_lines: &[&'a str],
        block_attrs: BlockAttributes,
        format: TableFormat,
        title_events: Vec<Event<'a>>,
    ) -> Option<Event<'a>> {
        // Parse each non-blank line into fields
        let mut rows: Vec<Vec<Cow<'a, str>>> = Vec::new();
        let mut first_blank_after_first_row = false;

        for &line in content_lines {
            if scanner::is_blank(line) {
                if !rows.is_empty() && !first_blank_after_first_row {
                    first_blank_after_first_row = true;
                }
                continue;
            }
            let fields = match format {
                TableFormat::Csv => scanner::parse_csv_fields(line),
                TableFormat::Dsv => scanner::parse_dsv_fields(line),
                TableFormat::Tsv => scanner::parse_tsv_fields(line),
                TableFormat::Native => unreachable!(),
            };
            rows.push(fields);
        }

        if rows.is_empty() {
            self.push_title_then_events(title_events);
            return self.event_buffer.pop().or_else(|| self.scan_next_block());
        }

        // Determine number of columns from cols attribute or first row
        let num_cols = if let Some(n) = block_attrs.table_cols_count() {
            n
        } else {
            rows[0].len()
        };

        // Determine header/footer
        let has_header = block_attrs.has_option("header")
            || (first_blank_after_first_row && rows[0].len() == num_cols);
        let has_footer = block_attrs.has_option("footer");

        // Get column specs for alignment defaults
        let col_specs = block_attrs.table_col_specs();

        // Split rows into header, body, footer
        let header_rows = if has_header && !rows.is_empty() {
            &rows[..1]
        } else {
            &[][..]
        };
        let remaining_rows = if has_header && !rows.is_empty() {
            &rows[1..]
        } else {
            &rows[..]
        };
        let (body_rows, footer_rows) = if has_footer && !remaining_rows.is_empty() {
            let split = remaining_rows.len() - 1;
            (&remaining_rows[..split], &remaining_rows[split..])
        } else {
            (remaining_rows, &[][..])
        };

        // Resolve alignment from column specs
        let resolve_align = |col_idx: usize| -> (HAlign, VAlign) {
            if let Some(ref specs) = col_specs
                && col_idx < specs.len()
            {
                return (specs[col_idx].halign, specs[col_idx].valign);
            }
            (HAlign::default(), VAlign::default())
        };

        // Build events in reverse (buffer is a stack)
        self.push_event(Event::End(TagEnd::Table));

        // Helper to emit cells for a row
        macro_rules! emit_format_row_cells {
            ($row:expr, $is_header_section:expr) => {
                for (ci, field) in $row.iter().enumerate().rev() {
                    if ci >= num_cols {
                        continue;
                    }
                    let (halign, valign) = resolve_align(ci);
                    let text: Cow<'a, str> = match field {
                        Cow::Borrowed(s) => Cow::Borrowed(*s),
                        Cow::Owned(s) => Cow::Owned(s.clone()),
                    };
                    if $is_header_section {
                        self.push_event(Event::End(TagEnd::TableHeaderCell));
                        self.push_event(Event::Text(text));
                        self.push_event(Event::Start(Tag::TableHeaderCell {
                            colspan: 1,
                            rowspan: 1,
                            style: CellStyle::Default,
                            halign,
                            valign,
                        }));
                    } else {
                        self.push_event(Event::End(TagEnd::TableCell));
                        self.push_event(Event::Text(text));
                        self.push_event(Event::Start(Tag::TableCell {
                            colspan: 1,
                            rowspan: 1,
                            style: CellStyle::Default,
                            halign,
                            valign,
                        }));
                    }
                }
            };
        }

        // TableFoot
        if !footer_rows.is_empty() {
            self.push_event(Event::End(TagEnd::TableFoot));
            for row in footer_rows.iter().rev() {
                self.push_event(Event::End(TagEnd::TableRow));
                emit_format_row_cells!(row, false);
                self.push_event(Event::Start(Tag::TableRow));
            }
            self.push_event(Event::Start(Tag::TableFoot));
        }

        // TableBody
        if !body_rows.is_empty() {
            self.push_event(Event::End(TagEnd::TableBody));
            for row in body_rows.iter().rev() {
                self.push_event(Event::End(TagEnd::TableRow));
                emit_format_row_cells!(row, false);
                self.push_event(Event::Start(Tag::TableRow));
            }
            self.push_event(Event::Start(Tag::TableBody));
        }

        // TableHead
        if !header_rows.is_empty() {
            self.push_event(Event::End(TagEnd::TableHead));
            for row in header_rows.iter().rev() {
                self.push_event(Event::End(TagEnd::TableRow));
                emit_format_row_cells!(row, true);
                self.push_event(Event::Start(Tag::TableRow));
            }
            self.push_event(Event::Start(Tag::TableHead));
        }

        self.push_event(Event::Start(Tag::Table));
        self.emit_block_metadata(&block_attrs, SubstitutionSet::NORMAL);
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
                || scanner::is_block_video(line).is_some()
                || scanner::is_block_audio(line).is_some()
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

        let block_attrs = self.pending_block_attrs.take();

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
            if let Some(ref attrs) = block_attrs {
                self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
            }
        } else {
            self.push_event(Event::End(TagEnd::Paragraph));
            for (i, &pline) in para_lines.iter().enumerate().rev() {
                if i < para_lines.len() - 1 {
                    self.push_event(Event::SoftBreak);
                }
                self.push_event(Event::Text(Cow::Borrowed(pline)));
            }
            self.push_event(Event::Start(Tag::Paragraph));
            if let Some(ref attrs) = block_attrs {
                self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
            }
        }
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    /// Scan indented text as a regular paragraph (when `[normal]` style is set).
    fn scan_normal_indented_paragraph(&mut self) -> Option<Event<'a>> {
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

        // Strip common indent and emit as regular paragraph
        let min_indent = lines
            .iter()
            .filter(|l| !l.is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);

        self.push_event(Event::End(TagEnd::Paragraph));
        for (i, &pline) in lines.iter().enumerate().rev() {
            if i < lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            let stripped = if pline.len() >= min_indent {
                &pline[min_indent..]
            } else {
                pline
            };
            self.push_event(Event::Text(Cow::Borrowed(stripped)));
        }
        self.push_event(Event::Start(Tag::Paragraph));
        let block_attrs = self.pending_block_attrs.take();
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }
        self.push_title_then_events(title_events);

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
        let block_attrs = self.pending_block_attrs.take();
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::VERBATIM);
        }
        self.push_title_then_events(title_events);

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
        let block_attrs = self.pending_block_attrs.take();
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }
        self.push_title_then_events(title_events);

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

        // Check for source block (applies to both listing `----` and literal `....` delimiters)
        if (delim_type == scanner::DelimiterType::Listing
            || delim_type == scanner::DelimiterType::Literal)
            && block_attrs.is_source_block()
        {
            let language = block_attrs.source_language().map(|l| Cow::Owned(l.to_string()));
            return self.scan_source_block(delim_type, delim_len, language, title_events, &block_attrs);
        }

        // Verse block: [verse] on quote delimiter
        if delim_type == scanner::DelimiterType::Quote && block_attrs.is_verse_style() {
            return self.scan_verse_block(delim_type, delim_len, title_events, &block_attrs);
        }

        // Stem block: [stem]/[latexmath]/[asciimath] on passthrough delimiter
        if delim_type == scanner::DelimiterType::Passthrough
            && let Some(variant) = block_attrs.stem_variant()
        {
            let variant = Cow::Owned(variant.to_string());
            return self.scan_stem_block(delim_type, delim_len, variant, title_events, &block_attrs);
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

            // For unclosed blocks, trim one trailing empty line (artifact of split_lines)
            if !closed
                && content_lines.last().is_some_and(|l| l.is_empty())
            {
                content_lines.pop();
            }

            // Handle single empty line: emit "\n" instead of ""
            if content_lines.len() == 1 && content_lines[0].is_empty() {
                self.push_event(Event::End(TagEnd::DelimitedBlock));
                self.push_event(Event::Text(Cow::Borrowed("\n")));
                self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
                self.emit_block_metadata(&block_attrs, SubstitutionSet::VERBATIM);
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
            self.emit_block_metadata(&block_attrs, SubstitutionSet::VERBATIM);
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
        self.emit_block_metadata(&block_attrs, SubstitutionSet::NORMAL);
        self.push_title_then_events(title_events);
        self.event_buffer.pop()
    }

    fn scan_source_block(
        &mut self,
        delim_type: scanner::DelimiterType,
        delim_len: usize,
        language: Option<CowStr<'a>>,
        title_events: Vec<Event<'a>>,
        block_attrs: &BlockAttributes,
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

        let resolved_subs = block_attrs
            .substitution_set(SubstitutionSet::VERBATIM)
            .unwrap_or(SubstitutionSet::VERBATIM);
        let process_callouts = resolved_subs.has(SubstitutionSet::CALLOUTS);

        self.push_event(Event::End(TagEnd::SourceBlock));
        for (i, &cline) in content_lines.iter().enumerate().rev() {
            if i < content_lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            if process_callouts {
                let (stripped, callout_nums) = scanner::strip_callout_markers(cline);
                if callout_nums.is_empty() {
                    self.push_event(Event::Text(Cow::Borrowed(cline)));
                } else {
                    for &n in callout_nums.iter().rev() {
                        self.push_event(Event::CalloutRef(n));
                    }
                    self.push_event(Event::Text(Cow::Borrowed(stripped)));
                }
            } else {
                self.push_event(Event::Text(Cow::Borrowed(cline)));
            }
        }
        self.push_event(Event::Start(Tag::SourceBlock { language }));
        self.emit_block_metadata(block_attrs, SubstitutionSet::VERBATIM);
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    fn scan_verse_block(
        &mut self,
        delim_type: scanner::DelimiterType,
        delim_len: usize,
        title_events: Vec<Event<'a>>,
        block_attrs: &BlockAttributes,
    ) -> Option<Event<'a>> {
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

        if !closed
            && content_lines.last().is_some_and(|l| l.is_empty())
        {
            content_lines.pop();
        }

        let kind = DelimitedBlockKind::Verse;

        self.push_event(Event::End(TagEnd::DelimitedBlock));
        for (i, &cline) in content_lines.iter().enumerate().rev() {
            if i < content_lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            self.push_event(Event::Text(Cow::Borrowed(cline)));
        }
        self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
        self.emit_block_metadata(block_attrs, SubstitutionSet::NORMAL);
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    fn scan_stem_block(
        &mut self,
        delim_type: scanner::DelimiterType,
        delim_len: usize,
        variant: CowStr<'a>,
        title_events: Vec<Event<'a>>,
        block_attrs: &BlockAttributes,
    ) -> Option<Event<'a>> {
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

        if !closed
            && content_lines.last().is_some_and(|l| l.is_empty())
        {
            content_lines.pop();
        }

        self.push_event(Event::End(TagEnd::StemBlock));
        for (i, &cline) in content_lines.iter().enumerate().rev() {
            if i < content_lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            self.push_event(Event::Text(Cow::Borrowed(cline)));
        }
        self.push_event(Event::Start(Tag::StemBlock { variant }));
        self.emit_block_metadata(block_attrs, SubstitutionSet::NONE);
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
                Some(BlockContext::UnorderedList { depth }) if *depth > target_depth => {
                    events.push(Event::End(TagEnd::UnorderedList));
                    self.context_stack.pop();
                }
                Some(BlockContext::OrderedList { depth }) if *depth > target_depth => {
                    events.push(Event::End(TagEnd::OrderedList));
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
        // Check if there's a parent DL at target depth — enables cross-type closing
        let has_parent_dl = self.is_in_description_list_at_depth(target_depth);

        let mut events = Vec::new();
        loop {
            match self.context_stack.last() {
                Some(BlockContext::DescriptionListEntry { depth }) if *depth >= target_depth => {
                    events.push(Event::End(TagEnd::DescriptionDescription));
                    self.context_stack.pop();
                }
                Some(BlockContext::DescriptionList { depth }) if *depth > target_depth => {
                    events.push(Event::End(TagEnd::DescriptionList));
                    self.context_stack.pop();
                }
                // When returning to parent DL, close interleaved list contexts
                Some(BlockContext::ListItem { .. })
                | Some(BlockContext::UnorderedList { .. })
                | Some(BlockContext::OrderedList { .. })
                | Some(BlockContext::CalloutListItem)
                | Some(BlockContext::CalloutList)
                    if has_parent_dl =>
                {
                    match self.context_stack.pop() {
                        Some(BlockContext::ListItem { .. }) => events.push(Event::End(TagEnd::ListItem)),
                        Some(BlockContext::UnorderedList { .. }) => events.push(Event::End(TagEnd::UnorderedList)),
                        Some(BlockContext::OrderedList { .. }) => events.push(Event::End(TagEnd::OrderedList)),
                        Some(BlockContext::CalloutListItem) => events.push(Event::End(TagEnd::CalloutListItem)),
                        Some(BlockContext::CalloutList) => events.push(Event::End(TagEnd::CalloutList)),
                        _ => unreachable!(),
                    }
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

    /// Check if a line is a continuation of a description list principal text.
    fn is_dlist_continuation_line(&self, line: &str) -> bool {
        !scanner::is_blank(line)
            && scanner::strip_section_marker(line).is_none()
            && scanner::is_delimiter(line).is_none()
            && scanner::is_list_marker_unordered(line).is_none()
            && scanner::is_list_marker_ordered(line).is_none()
            && scanner::is_admonition(line).is_none()
            && scanner::is_block_image(line).is_none()
            && scanner::is_block_video(line).is_none()
            && scanner::is_block_audio(line).is_none()
            && !scanner::is_toc_macro(line)
            && scanner::is_include_directive(line).is_none()
            && !scanner::is_thematic_break(line)
            && !scanner::is_page_break(line)
            && scanner::is_attribute_entry(line).is_none()
            && scanner::is_block_attribute(line).is_none()
            && scanner::is_block_title(line).is_none()
            && !scanner::is_line_comment(line)
            && scanner::is_description_list_marker(line).is_none()
            && scanner::is_callout_list_item(line).is_none()
            && !scanner::is_list_continuation(line)
            && !scanner::is_table_delimiter(line)
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

        // Collect additional terms (multiple terms per dlistItem)
        let mut extra_terms: Vec<&'a str> = Vec::new();

        // Collect principal text: desc on same line + wrapped continuation lines
        let mut principal_desc = desc;
        let mut continuation_lines: Vec<&'a str> = Vec::new();

        if desc.is_empty() {
            // Collect additional terms: consecutive dlist markers with empty desc at same depth
            loop {
                // Skip blank lines between terms
                let saved_pos = self.pos;
                while let Some(l) = self.current_line() {
                    if scanner::is_blank(l) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if let Some(line) = self.current_line()
                    && let Some((d, t, dd)) = scanner::is_description_list_marker(line)
                    && d == depth && dd.is_empty()
                {
                    extra_terms.push(t);
                    self.advance();
                    continue;
                }
                // Not another empty-desc term — restore position and look for principal
                self.pos = saved_pos;
                break;
            }

            // Empty desc: look for principal on next line(s),
            // possibly skipping blank lines ("ventilated" style)
            let saved_pos = self.pos;
            while let Some(l) = self.current_line() {
                if scanner::is_blank(l) {
                    self.advance();
                } else {
                    break;
                }
            }
            if let Some(line) = self.current_line() {
                let check = if line.starts_with(' ') || line.starts_with('\t') {
                    line.trim()
                } else {
                    line
                };
                if !check.is_empty() && self.is_dlist_continuation_line(check) {
                    principal_desc = check;
                    self.advance();
                } else {
                    self.pos = saved_pos;
                }
            } else {
                self.pos = saved_pos;
            }
        }

        // Collect wrapped continuation lines (non-indented, non-blank)
        if !principal_desc.is_empty() {
            while let Some(line) = self.current_line() {
                if self.is_dlist_continuation_line(line)
                    && !line.starts_with(' ') && !line.starts_with('\t')
                {
                    continuation_lines.push(line);
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Event buffer (bottom to top for FIFO via pop):
        for &cline in continuation_lines.iter().rev() {
            self.push_event(Event::Text(Cow::Borrowed(cline)));
            self.push_event(Event::SoftBreak);
        }
        if !principal_desc.is_empty() {
            self.push_event(Event::Text(Cow::Borrowed(principal_desc)));
        }
        self.push_event(Event::Start(Tag::DescriptionDescription));

        // Emit extra terms in reverse (they go on top of the first term)
        for &extra_term in extra_terms.iter().rev() {
            self.push_event(Event::End(TagEnd::DescriptionTerm));
            self.push_event(Event::Text(Cow::Borrowed(extra_term)));
            self.push_event(Event::Start(Tag::DescriptionTerm));
        }

        self.push_event(Event::End(TagEnd::DescriptionTerm));
        self.push_event(Event::Text(Cow::Borrowed(term)));
        self.push_event(Event::Start(Tag::DescriptionTerm));
        if need_new_list {
            self.push_event(Event::Start(Tag::DescriptionList));
        }
        let block_attrs = self.pending_block_attrs.take();
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }

        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    /// Check if a line is a continuation of a list item principal (not a block element or new list item).
    fn is_list_continuation_line(&self, line: &str) -> bool {
        !scanner::is_blank(line)
            && scanner::strip_section_marker(line).is_none()
            && scanner::is_delimiter(line).is_none()
            && scanner::is_list_marker_unordered(line).is_none()
            && scanner::is_list_marker_ordered(line).is_none()
            && scanner::is_admonition(line).is_none()
            && scanner::is_block_image(line).is_none()
            && scanner::is_block_video(line).is_none()
            && scanner::is_block_audio(line).is_none()
            && !scanner::is_toc_macro(line)
            && scanner::is_include_directive(line).is_none()
            && !scanner::is_thematic_break(line)
            && !scanner::is_page_break(line)
            && scanner::is_attribute_entry(line).is_none()
            && scanner::is_block_attribute(line).is_none()
            && scanner::is_block_title(line).is_none()
            && !scanner::is_line_comment(line)
            && scanner::is_description_list_marker(line).is_none()
            && scanner::is_callout_list_item(line).is_none()
            && !scanner::is_list_continuation(line)
            && !scanner::is_table_delimiter(line)
            && !line.starts_with(' ') && !line.starts_with('\t')
    }

    fn scan_unordered_list_item(&mut self, depth: u8, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let (checked, actual_text) = scanner::parse_checklist_marker(text);

        // Collect wrapped continuation lines
        let mut continuation_lines: Vec<&'a str> = Vec::new();
        while let Some(line) = self.current_line() {
            if self.is_list_continuation_line(line) {
                continuation_lines.push(line);
                self.advance();
            } else {
                break;
            }
        }

        // Use cross-type closing when there's an existing parent UL at the same depth
        let has_parent_list = self.is_in_list_at_depth(depth, true);
        let mut close_events = if has_parent_list {
            self.close_to_parent_list(depth, true)
        } else {
            self.close_list_items_for_depth(depth)
        };

        let mut need_new_list = !self.is_in_list_at_depth(depth, true);

        // If pending block attrs and same-depth list exists, force new list
        // (block attribute line between same-depth items starts a new list)
        if !need_new_list && self.pending_block_attrs.is_some()
            && let Some(BlockContext::UnorderedList { depth: d }) = self.context_stack.last()
            && *d == depth
        {
            close_events.push(Event::End(TagEnd::UnorderedList));
            self.context_stack.pop();
            need_new_list = true;
        }

        // Push text events in reverse (bottom of stack = last to emit)
        // Order: first text, then SoftBreak + continuation lines
        for &cline in continuation_lines.iter().rev() {
            self.push_event(Event::Text(Cow::Borrowed(cline)));
            self.push_event(Event::SoftBreak);
        }
        self.push_event(Event::Text(Cow::Borrowed(actual_text)));

        if need_new_list {
            self.context_stack.push(BlockContext::UnorderedList { depth });
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Start(Tag::ListItem { depth, checked }));
            self.push_event(Event::Start(Tag::UnorderedList { has_checklist: checked.is_some() }));
        } else {
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Start(Tag::ListItem { depth, checked }));
        }

        let block_attrs = self.pending_block_attrs.take();
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }

        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_title_then_events(title_events);

        self.had_blank_line = false;
        self.event_buffer.pop()
    }

    fn scan_ordered_list_item(&mut self, depth: u8, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let mut close_events = self.close_list_items_for_depth(depth);

        let mut need_new_list = !self.is_in_list_at_depth(depth, false);

        // If pending block attrs and same-depth list exists, force new list
        if !need_new_list && self.pending_block_attrs.is_some()
            && let Some(BlockContext::OrderedList { depth: d }) = self.context_stack.last()
            && *d == depth
        {
            close_events.push(Event::End(TagEnd::OrderedList));
            self.context_stack.pop();
            need_new_list = true;
        }

        let (list_start, list_reversed) = self.pending_block_attrs.as_ref()
            .map(|a| (a.list_start(), a.is_reversed()))
            .unwrap_or((None, false));

        if need_new_list {
            self.context_stack.push(BlockContext::OrderedList { depth });
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(text)));
            self.push_event(Event::Start(Tag::ListItem { depth, checked: None }));
            self.push_event(Event::Start(Tag::OrderedList { start: list_start, reversed: list_reversed }));
        } else {
            self.context_stack.push(BlockContext::ListItem { depth });

            self.push_event(Event::Text(Cow::Borrowed(text)));
            self.push_event(Event::Start(Tag::ListItem { depth, checked: None }));
        }

        let block_attrs = self.pending_block_attrs.take();
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }

        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    fn is_in_callout_list(&self) -> bool {
        self.context_stack.iter().rev().any(|ctx| matches!(ctx, BlockContext::CalloutList))
    }

    fn scan_callout_list_item(&mut self, number: u32, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let need_new_list = !self.is_in_callout_list();

        // Close previous CalloutListItem if one is open
        let mut close_events = Vec::new();
        if !need_new_list
            && let Some(BlockContext::CalloutListItem) = self.context_stack.last()
        {
            close_events.push(Event::End(TagEnd::CalloutListItem));
            self.context_stack.pop();
        }

        if need_new_list {
            self.context_stack.push(BlockContext::CalloutList);
        }
        self.context_stack.push(BlockContext::CalloutListItem);

        // Buffer (bottom to top for FIFO via pop):
        if !text.is_empty() {
            self.push_event(Event::Text(Cow::Borrowed(text)));
        }
        self.push_event(Event::Start(Tag::CalloutListItem { number }));
        if need_new_list {
            self.push_event(Event::Start(Tag::CalloutList));
        }

        let block_attrs = self.pending_block_attrs.take();
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }

        for ev in close_events.into_iter().rev() {
            self.push_event(ev);
        }
        self.push_title_then_events(title_events);

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
                                        BlockContext::CalloutListItem => {
                                            events.push(Event::End(TagEnd::CalloutListItem));
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
    fn test_multiline_attribute_soft_wrap_body() {
        let input = "== Section\n\n:description: This is a long \\\nvalue that spans two lines\n\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Section { level: 2 }),
            Event::Start(Tag::SectionTitle { level: 2, id: Cow::Owned("_section".into()) }),
            Event::Text(Cow::Borrowed("Section")),
            Event::End(TagEnd::SectionTitle),
            Event::Attribute {
                name: Cow::Borrowed("description"),
                value: Cow::Owned("This is a long value that spans two lines".into()),
            },
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
            Event::End(TagEnd::Section { level: 2 }),
        ]);
    }

    #[test]
    fn test_multiline_attribute_hard_wrap_body() {
        let input = "== Section\n\n:description: Line one + \\\nLine two\n\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Section { level: 2 }),
            Event::Start(Tag::SectionTitle { level: 2, id: Cow::Owned("_section".into()) }),
            Event::Text(Cow::Borrowed("Section")),
            Event::End(TagEnd::SectionTitle),
            Event::Attribute {
                name: Cow::Borrowed("description"),
                value: Cow::Owned("Line one\nLine two".into()),
            },
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
            Event::End(TagEnd::Section { level: 2 }),
        ]);
    }

    #[test]
    fn test_multiline_attribute_three_lines() {
        let input = "== S\n\n:desc: one \\\ntwo \\\nthree";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Section { level: 2 }),
            Event::Start(Tag::SectionTitle { level: 2, id: Cow::Owned("_s".into()) }),
            Event::Text(Cow::Borrowed("S")),
            Event::End(TagEnd::SectionTitle),
            Event::Attribute {
                name: Cow::Borrowed("desc"),
                value: Cow::Owned("one two three".into()),
            },
            Event::End(TagEnd::Section { level: 2 }),
        ]);
    }

    #[test]
    fn test_multiline_attribute_in_header() {
        let input = "= Title\n:description: A long \\\nvalue here\n\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Header),
            Event::Start(Tag::SectionTitle { level: 0, id: Cow::Owned("_title".into()) }),
            Event::Start(Tag::DocumentTitle),
            Event::Text(Cow::Borrowed("Title")),
            Event::End(TagEnd::DocumentTitle),
            Event::End(TagEnd::SectionTitle),
            Event::Attribute {
                name: Cow::Borrowed("description"),
                value: Cow::Owned("A long value here".into()),
            },
            Event::End(TagEnd::Header),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_multiline_attribute_mixed_wrap() {
        let input = "== S\n\n:val: soft \\\nmiddle + \\\nhard end";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Section { level: 2 }),
            Event::Start(Tag::SectionTitle { level: 2, id: Cow::Owned("_s".into()) }),
            Event::Text(Cow::Borrowed("S")),
            Event::End(TagEnd::SectionTitle),
            Event::Attribute {
                name: Cow::Borrowed("val"),
                value: Cow::Owned("soft middle\nhard end".into()),
            },
            Event::End(TagEnd::Section { level: 2 }),
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
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
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
            Event::Start(Tag::TableHeaderCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("Header 1")),
            Event::End(TagEnd::TableHeaderCell),
            Event::Start(Tag::TableHeaderCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("Header 2")),
            Event::End(TagEnd::TableHeaderCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableHead),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("Cell 1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("Cell 2")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("Cell 3")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
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
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
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
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![Cow::Owned("header".into())], named: vec![], subs: None },
            Event::Start(Tag::Table),
            Event::Start(Tag::TableHead),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableHeaderCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("H1")),
            Event::End(TagEnd::TableHeaderCell),
            Event::Start(Tag::TableHeaderCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("H2")),
            Event::End(TagEnd::TableHeaderCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableHead),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![Cow::Owned("footer".into())], named: vec![], subs: None },
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::Start(Tag::TableFoot),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("F1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![Cow::Owned("header".into()), Cow::Owned("footer".into())], named: vec![], subs: None },
            Event::Start(Tag::Table),
            Event::Start(Tag::TableHead),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableHeaderCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("H1")),
            Event::End(TagEnd::TableHeaderCell),
            Event::Start(Tag::TableHeaderCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("H2")),
            Event::End(TagEnd::TableHeaderCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableHead),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C2")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::Start(Tag::TableFoot),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("F1")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("F2")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableFoot),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_colspan() {
        let input = "|===\n| A 2+| B spans\n| C | D | E\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 2, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("B spans")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("D")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("E")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_rowspan() {
        let input = "|===\n.2+| A | B\n| C\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 2, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_cols_alignment() {
        let input = "[cols=\"<,^,>\"]\n|===\n| A | B | C\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("A")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Center, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("B")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Right, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("C")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_cell_level_alignment() {
        let input = "|===\n^| centered >.^| right-middle | normal\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Center, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("centered")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Right, valign: VAlign::Middle }),
            Event::Text(Cow::Borrowed("right-middle")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("normal")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_cell_overrides_col_alignment() {
        // cols says left, but cell says center — cell wins
        let input = "[cols=\"<,<\"]\n|===\n^| centered | normal\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Table),
            Event::Start(Tag::TableBody),
            Event::Start(Tag::TableRow),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Center, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("centered")),
            Event::End(TagEnd::TableCell),
            Event::Start(Tag::TableCell { colspan: 1, rowspan: 1, style: CellStyle::Default, halign: HAlign::Left, valign: VAlign::Top }),
            Event::Text(Cow::Borrowed("normal")),
            Event::End(TagEnd::TableCell),
            Event::End(TagEnd::TableRow),
            Event::End(TagEnd::TableBody),
            Event::End(TagEnd::Table),
        ]);
    }

    #[test]
    fn test_table_autowidth_option() {
        let input = "[%autowidth]\n|===\n| A | B\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events[0], Event::BlockMetadata {
            style: None, id: None, roles: vec![], options: vec![Cow::Owned("autowidth".into())], named: vec![], subs: None,
        });
        assert_eq!(events[1], Event::Start(Tag::Table));
    }

    #[test]
    fn test_table_stripes_named_attr() {
        let input = "[stripes=even]\n|===\n| A | B\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events[0], Event::BlockMetadata {
            style: None, id: None, roles: vec![], options: vec![],
            named: vec![(Cow::Owned("stripes".into()), Cow::Owned("even".into()))],
            subs: None,
        });
        assert_eq!(events[1], Event::Start(Tag::Table));
    }

    #[test]
    fn test_table_caption_named_attr() {
        let input = "[caption=\"Listing {counter:table-number}. \"]\n|===\n| A | B\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events[0], Event::BlockMetadata {
            style: None, id: None, roles: vec![], options: vec![],
            named: vec![(Cow::Owned("caption".into()), Cow::Owned("Listing {counter:table-number}. ".into()))],
            subs: None,
        });
    }

    #[test]
    fn test_table_autowidth_stripes_combined() {
        let input = "[%autowidth,stripes=odd]\n|===\n| A | B\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events[0], Event::BlockMetadata {
            style: None, id: None, roles: vec![], options: vec![Cow::Owned("autowidth".into())],
            named: vec![(Cow::Owned("stripes".into()), Cow::Owned("odd".into()))],
            subs: None,
        });
    }

    #[test]
    fn test_table_cols_not_in_named() {
        // cols is consumed by parser, should not appear in named
        let input = "[cols=\"2\",stripes=even]\n|===\n| A\n| B\n| C\n| D\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        if let Event::BlockMetadata { ref named, .. } = events[0] {
            assert!(named.iter().all(|(k, _)| k != "cols"), "cols should be filtered out");
            assert!(named.iter().any(|(k, _)| k == "stripes"), "stripes should be present");
        } else {
            panic!("Expected BlockMetadata");
        }
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

    #[test]
    fn test_verse_block() {
        let input = "[verse]\n____\nline one\nline two\n____";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Verse }),
            Event::Text(Cow::Borrowed("line one")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("line two")),
            Event::End(TagEnd::DelimitedBlock),
        ]);
    }

    #[test]
    fn test_block_video() {
        let input = "video::video.mp4[]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::BlockVideo {
                target: Cow::Borrowed("video.mp4"),
                attrs: Cow::Borrowed(""),
            }),
            Event::End(TagEnd::BlockVideo),
        ]);
    }

    #[test]
    fn test_block_video_with_attrs() {
        let input = "video::video.mp4[width=640,poster=preview.jpg]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::BlockVideo {
                target: Cow::Borrowed("video.mp4"),
                attrs: Cow::Borrowed("width=640,poster=preview.jpg"),
            }),
            Event::End(TagEnd::BlockVideo),
        ]);
    }

    #[test]
    fn test_block_audio() {
        let input = "audio::audio.mp3[]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::BlockAudio {
                target: Cow::Borrowed("audio.mp3"),
                attrs: Cow::Borrowed(""),
            }),
            Event::End(TagEnd::BlockAudio),
        ]);
    }

    #[test]
    fn test_block_audio_with_options() {
        let input = "audio::audio.mp3[options=\"autoplay,loop\"]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::BlockAudio {
                target: Cow::Borrowed("audio.mp3"),
                attrs: Cow::Borrowed("options=\"autoplay,loop\""),
            }),
            Event::End(TagEnd::BlockAudio),
        ]);
    }

    #[test]
    fn test_block_admonition() {
        let input = "[NOTE]\n====\nContent\n====";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Admonition { kind: crate::event::AdmonitionKind::Note }),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content")),
            Event::End(TagEnd::Paragraph),
            Event::End(TagEnd::Admonition),
        ]);
    }

    #[test]
    fn test_block_admonition_warning() {
        let input = "[WARNING]\n====\nDanger ahead!\n====";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Admonition { kind: crate::event::AdmonitionKind::Warning }),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Danger ahead!")),
            Event::End(TagEnd::Paragraph),
            Event::End(TagEnd::Admonition),
        ]);
    }

    #[test]
    fn test_multiple_authors() {
        let input = "= Title\nJohn Doe; Jane Smith";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Header),
            Event::Start(Tag::SectionTitle { level: 0, id: Cow::Owned("_title".into()) }),
            Event::Start(Tag::DocumentTitle),
            Event::Text(Cow::Borrowed("Title")),
            Event::End(TagEnd::DocumentTitle),
            Event::End(TagEnd::SectionTitle),
            Event::Author {
                fullname: Cow::Borrowed("John Doe"),
                firstname: Cow::Borrowed("John"),
                middlename: Cow::Borrowed(""),
                lastname: Cow::Borrowed("Doe"),
                initials: Cow::Owned("JD".into()),
                address: Cow::Borrowed(""),
            },
            Event::Author {
                fullname: Cow::Borrowed("Jane Smith"),
                firstname: Cow::Borrowed("Jane"),
                middlename: Cow::Borrowed(""),
                lastname: Cow::Borrowed("Smith"),
                initials: Cow::Owned("JS".into()),
                address: Cow::Borrowed(""),
            },
            Event::End(TagEnd::Header),
        ]);
    }

    #[test]
    fn test_document_header_with_full_revision() {
        let input = "= Title\nAuthor Name\nv1.0, 2024-01-01: Initial release\n\nContent";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Author {
            fullname: Cow::Borrowed("Author Name"),
            firstname: Cow::Borrowed("Author"),
            middlename: Cow::Borrowed(""),
            lastname: Cow::Borrowed("Name"),
            initials: Cow::Owned("AN".into()),
            address: Cow::Borrowed(""),
        }));
        assert!(events.contains(&Event::Revision {
            version: Cow::Borrowed("v1.0"),
            date: Cow::Borrowed("2024-01-01"),
            remark: Cow::Borrowed("Initial release"),
        }));
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("revnumber"),
            value: Cow::Borrowed("v1.0"),
        }));
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("revdate"),
            value: Cow::Borrowed("2024-01-01"),
        }));
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("revremark"),
            value: Cow::Borrowed("Initial release"),
        }));
    }

    #[test]
    fn test_document_header_with_version_only() {
        let input = "= Title\nAuthor Name\nv2.0\n\nContent";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Revision {
            version: Cow::Borrowed("v2.0"),
            date: Cow::Borrowed(""),
            remark: Cow::Borrowed(""),
        }));
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("revnumber"),
            value: Cow::Borrowed("v2.0"),
        }));
        // No revdate or revremark attributes
        assert!(!events.contains(&Event::Attribute {
            name: Cow::Borrowed("revdate"),
            value: Cow::Borrowed(""),
        }));
    }

    #[test]
    fn test_document_header_no_author_no_revision() {
        let input = "= Title\nv1.0, 2024-01-01: Initial release\n\nContent";
        let events: Vec<_> = BlockScanner::new(input).collect();
        // Without author line, the "v1.0..." line should NOT be parsed as revision
        assert!(!events.iter().any(|e| matches!(e, Event::Revision { .. })));
    }

    #[test]
    fn test_document_header_with_date_only() {
        let input = "= Title\nAuthor Name\n2024-01-01\n\nContent";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Revision {
            version: Cow::Borrowed(""),
            date: Cow::Borrowed("2024-01-01"),
            remark: Cow::Borrowed(""),
        }));
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("revdate"),
            value: Cow::Borrowed("2024-01-01"),
        }));
        // No revnumber attribute
        assert!(!events.contains(&Event::Attribute {
            name: Cow::Borrowed("revnumber"),
            value: Cow::Borrowed(""),
        }));
    }

    #[test]
    fn test_appendix_section_metadata() {
        let input = "[appendix]\n== Title";
        let events: Vec<_> = BlockScanner::new(input).collect();
        // BlockMetadata with style "appendix" should appear before Start(Section)
        let meta_pos = events.iter().position(|e| matches!(e,
            Event::BlockMetadata { style: Some(s), .. } if s == "appendix"
        ));
        let section_pos = events.iter().position(|e| matches!(e, Event::Start(Tag::Section { .. })));
        assert!(meta_pos.is_some(), "Expected BlockMetadata with style appendix");
        assert!(section_pos.is_some(), "Expected Start(Section)");
        assert!(meta_pos.unwrap() < section_pos.unwrap(),
            "BlockMetadata should appear before Start(Section)");
    }

    #[test]
    fn test_idprefix_idseparator_default() {
        let input = "== My Section";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SectionTitle {
            level: 2,
            id: Cow::Owned("_my_section".into()),
        })));
    }

    #[test]
    fn test_idprefix_empty() {
        let input = ":idprefix:\n\n== My Section";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SectionTitle {
            level: 2,
            id: Cow::Owned("my_section".into()),
        })));
    }

    #[test]
    fn test_idseparator_dash() {
        let input = ":idseparator: -\n\n== My Section";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SectionTitle {
            level: 2,
            id: Cow::Owned("_my-section".into()),
        })));
    }

    #[test]
    fn test_idprefix_empty_idseparator_dash() {
        let input = ":idprefix:\n:idseparator: -\n\n== My Section";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SectionTitle {
            level: 2,
            id: Cow::Owned("my-section".into()),
        })));
    }

    #[test]
    fn test_idprefix_unset() {
        let input = ":idprefix: sec-\n\n== First\n\n:!idprefix:\n\n== Second";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SectionTitle {
            level: 2,
            id: Cow::Owned("sec-first".into()),
        })));
        assert!(events.contains(&Event::Start(Tag::SectionTitle {
            level: 2,
            id: Cow::Owned("second".into()),
        })));
    }
}
