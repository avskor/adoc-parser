use std::borrow::Cow;

use crate::block::BlockScanner;
use crate::event::Event;
use crate::inline::InlineParser;

pub struct Parser<'a> {
    block_scanner: BlockScanner<'a>,
    inline_buffer: Vec<Event<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            block_scanner: BlockScanner::new(input),
            inline_buffer: Vec::new(),
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

        match event {
            Event::Text(Cow::Borrowed(s)) => {
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
