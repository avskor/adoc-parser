use std::borrow::Cow;

use crate::block::BlockScanner;
use crate::event::{DelimitedBlockKind, Event, SubstitutionSet, Tag, TagEnd};
use crate::inline::InlineParser;

pub struct Parser<'a> {
    block_scanner: BlockScanner<'a>,
    inline_buffer: Vec<Event<'a>>,
    pending_event: Option<Event<'a>>,
    subs_stack: Vec<SubstitutionSet>,
    pending_subs: Option<SubstitutionSet>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            block_scanner: BlockScanner::new(input),
            inline_buffer: Vec::new(),
            pending_event: None,
            subs_stack: Vec::new(),
            pending_subs: None,
        }
    }

    fn next_block_event(&mut self) -> Option<Event<'a>> {
        self.pending_event.take().or_else(|| self.block_scanner.next())
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
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ev) = self.inline_buffer.pop() {
            return Some(ev);
        }

        let event = self.next_block_event()?;

        match &event {
            Event::BlockMetadata { subs, .. } => {
                self.pending_subs = *subs;
            }
            Event::Start(Tag::SourceBlock { .. }) => {
                let subs = self.pending_subs.take().unwrap_or(SubstitutionSet::VERBATIM);
                self.subs_stack.push(subs);
            }
            Event::End(TagEnd::SourceBlock) => {
                self.subs_stack.pop();
            }
            Event::Start(Tag::DelimitedBlock { kind }) => {
                let default = Self::default_subs_for_delimited(*kind);
                let subs = self.pending_subs.take().unwrap_or(default);
                self.subs_stack.push(subs);
            }
            Event::End(TagEnd::DelimitedBlock) => {
                self.subs_stack.pop();
            }
            Event::Start(Tag::Paragraph) => {
                // Inherit subs from parent block if no explicit override
                let subs = self.pending_subs.take().unwrap_or_else(|| self.current_subs());
                self.subs_stack.push(subs);
            }
            Event::End(TagEnd::Paragraph) => {
                self.subs_stack.pop();
            }
            Event::Start(Tag::LiteralParagraph) => {
                let subs = self.pending_subs.take().unwrap_or_else(|| self.current_subs());
                self.subs_stack.push(subs);
            }
            Event::End(TagEnd::LiteralParagraph) => {
                self.subs_stack.pop();
            }
            _ => {}
        }

        match event {
            Event::Author { .. } => Some(event),
            Event::Text(Cow::Borrowed(s)) if self.current_subs().needs_inline_parsing() => {
                let subs = self.current_subs();
                // Peek at the next block event to see if this is a multiline paragraph
                let next = self.next_block_event();
                if matches!(next, Some(Event::SoftBreak)) {
                    // Multiline mode: combine Text + SoftBreak + Text + ... into one string
                    let mut combined = String::from(s);
                    combined.push('\n');

                    let mut next = self.next_block_event();
                    loop {
                        match next {
                            Some(Event::Text(Cow::Borrowed(t))) => {
                                combined.push_str(t);
                                next = self.next_block_event();
                                match next {
                                    Some(Event::SoftBreak) => {
                                        combined.push('\n');
                                        next = self.next_block_event();
                                    }
                                    other => {
                                        self.pending_event = other;
                                        break;
                                    }
                                }
                            }
                            other => {
                                self.pending_event = other;
                                break;
                            }
                        }
                    }

                    let events = InlineParser::parse_str_with_subs(&combined, subs);
                    if events.len() == 1 {
                        Some(events.into_iter().next().unwrap().into_static())
                    } else {
                        for ev in events.into_iter().rev() {
                            self.inline_buffer.push(ev.into_static());
                        }
                        self.inline_buffer.pop()
                    }
                } else {
                    // Single-line: zero-copy path
                    self.pending_event = next;
                    let events = InlineParser::parse_str_with_subs(s, subs);
                    if events.len() == 1 {
                        Some(events.into_iter().next().unwrap())
                    } else {
                        for ev in events.into_iter().rev() {
                            self.inline_buffer.push(ev);
                        }
                        self.inline_buffer.pop()
                    }
                }
            }
            other => Some(other),
        }
    }
}
