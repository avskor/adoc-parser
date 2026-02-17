use std::borrow::Cow;

use crate::block::BlockScanner;
use crate::event::Event;
use crate::inline::InlineParser;

pub struct Parser<'a> {
    block_scanner: BlockScanner<'a>,
    inline_buffer: Vec<Event<'a>>,
    in_source_block: bool,
    in_verbatim_block: bool,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            block_scanner: BlockScanner::new(input),
            inline_buffer: Vec::new(),
            in_source_block: false,
            in_verbatim_block: false,
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ev) = self.inline_buffer.pop() {
            return Some(ev);
        }

        let event = self.block_scanner.next()?;

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
            other => Some(other),
        }
    }
}
