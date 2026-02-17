use std::borrow::Cow;

use crate::block::BlockScanner;
use crate::event::Event;
use crate::inline::InlineParser;

pub struct Parser<'a> {
    block_scanner: BlockScanner<'a>,
    inline_buffer: Vec<Event<'a>>,
    pending_event: Option<Event<'a>>,
    in_source_block: bool,
    in_verbatim_block: bool,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            block_scanner: BlockScanner::new(input),
            inline_buffer: Vec::new(),
            pending_event: None,
            in_source_block: false,
            in_verbatim_block: false,
        }
    }

    fn next_block_event(&mut self) -> Option<Event<'a>> {
        self.pending_event.take().or_else(|| self.block_scanner.next())
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
            Event::Start(crate::event::Tag::SourceBlock { .. }) => {
                self.in_source_block = true;
            }
            Event::End(crate::event::TagEnd::SourceBlock) => {
                self.in_source_block = false;
            }
            Event::Start(crate::event::Tag::DelimitedBlock { kind }) => {
                if matches!(
                    kind,
                    crate::event::DelimitedBlockKind::Listing
                        | crate::event::DelimitedBlockKind::Literal
                        | crate::event::DelimitedBlockKind::Passthrough
                ) {
                    self.in_verbatim_block = true;
                }
            }
            Event::End(crate::event::TagEnd::DelimitedBlock) => {
                self.in_verbatim_block = false;
            }
            Event::Start(crate::event::Tag::LiteralParagraph) => {
                self.in_verbatim_block = true;
            }
            Event::End(crate::event::TagEnd::LiteralParagraph) => {
                self.in_verbatim_block = false;
            }
            _ => {}
        }

        match event {
            Event::Author { .. } => Some(event),
            Event::Text(Cow::Borrowed(s)) if !self.in_source_block && !self.in_verbatim_block => {
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

                    let events = InlineParser::parse_str(&combined);
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
                    let events = InlineParser::parse_str(s);
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
