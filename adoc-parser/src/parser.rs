use std::borrow::Cow;

use crate::block::BlockScanner;
use crate::event::{CellStyle, DelimitedBlockKind, Event, SubstitutionSet, Tag, TagEnd};
use crate::inline::{InlineOptions, InlineParser};

/// A pull parser that turns AsciiDoc source into a stream of [`Event`]s.
///
/// `Parser` implements [`Iterator`], yielding [`Event::Start`]/[`Event::End`]
/// pairs for blocks and inline formatting (in the style of `pulldown-cmark`).
///
/// ```
/// use adoc_parser::{Parser, Event, Tag};
/// let mut parser = Parser::new("Hello *world*");
/// assert!(parser.any(|ev| matches!(ev, Event::Start(Tag::Strong { .. }))));
/// ```
pub struct Parser<'a> {
    block_scanner: BlockScanner<'a>,
    inline_buffer: Vec<Event<'a>>,
    pending_event: Option<Event<'a>>,
    subs_stack: Vec<SubstitutionSet>,
    pending_subs: Option<SubstitutionSet>,
    /// Document-attribute-derived inline-parsing options (e.g. `:experimental:`
    /// gating the `kbd:`/`btn:`/`menu:` UI macros). Updated from
    /// `Event::Attribute` so body text reflects the attribute state up to
    /// that point.
    inline_options: InlineOptions,
    /// One entry per open table body cell: true if the cell pushed onto
    /// `subs_stack`. AsciiDoc (`a`) cells get NONE — content reaches the
    /// consumer raw for a nested block parse; literal (`l`) cells get
    /// VERBATIM — raw text, special chars escaped by the consumer.
    cell_subs_pushed: Vec<bool>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            block_scanner: BlockScanner::new(crate::scanner::strip_bom(input)),
            inline_buffer: Vec::new(),
            pending_event: None,
            subs_stack: Vec::new(),
            pending_subs: None,
            inline_options: InlineOptions::default(),
            cell_subs_pushed: Vec::new(),
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
            Event::Start(Tag::TableCell { style, .. }) => {
                let cell_subs = match style {
                    CellStyle::AsciiDoc => Some(SubstitutionSet::NONE),
                    CellStyle::Literal => Some(SubstitutionSet::VERBATIM),
                    _ => None,
                };
                self.cell_subs_pushed.push(cell_subs.is_some());
                if let Some(subs) = cell_subs {
                    self.subs_stack.push(subs);
                }
            }
            Event::End(TagEnd::TableCell) => {
                if self.cell_subs_pushed.pop().unwrap_or(false) {
                    self.subs_stack.pop();
                }
            }
            Event::Start(Tag::LiteralParagraph) => {
                // A literal paragraph is verbatim by definition; without an explicit `[subs=…]`
                // (carried via pending_subs) it must not inherit the parent's richer subs.
                let subs = self.pending_subs.take().unwrap_or(SubstitutionSet::VERBATIM);
                self.subs_stack.push(subs);
            }
            Event::End(TagEnd::LiteralParagraph) => {
                self.subs_stack.pop();
            }
            Event::Attribute { name, .. } => {
                // Feed document-attribute entries into the inline-parsing
                // options channel (e.g. :experimental: gates kbd:/btn:/menu:;
                // Asciidoctor leaves them literal otherwise).
                self.inline_options.apply_attribute(name.as_ref());
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

                    let events = InlineParser::parse_str_with_subs_options(&combined, subs, self.inline_options);
                    if events.len() == 1 {
                        Some(events.into_iter().next().unwrap().into_static())
                    } else {
                        for ev in events.into_iter().rev() {
                            self.inline_buffer.push(ev.into_static());
                        }
                        // If inline parsing produced no events, continue to the
                        // next event rather than ending iteration prematurely (D6).
                        self.inline_buffer.pop().or_else(|| self.next())
                    }
                } else {
                    // Single-line: zero-copy path
                    self.pending_event = next;
                    let events = InlineParser::parse_str_with_subs_options(s, subs, self.inline_options);
                    if events.len() == 1 {
                        Some(events.into_iter().next().unwrap())
                    } else {
                        for ev in events.into_iter().rev() {
                            self.inline_buffer.push(ev);
                        }
                        // See above (D6): don't end iteration on an empty result.
                        self.inline_buffer.pop().or_else(|| self.next())
                    }
                }
            }
            Event::Text(Cow::Owned(s)) if self.current_subs().needs_inline_parsing() => {
                // Owned text (e.g. merged multi-line table cells) goes through
                // inline parsing too; results borrow from the local string, so
                // they are detached via into_static.
                let subs = self.current_subs();
                let events = InlineParser::parse_str_with_subs_options(&s, subs, self.inline_options);
                for ev in events.into_iter().rev() {
                    self.inline_buffer.push(ev.into_static());
                }
                // See above (D6): don't end iteration on an empty result.
                self.inline_buffer.pop().or_else(|| self.next())
            }
            other => Some(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A leading UTF-8 BOM is stripped, so `= Title` is recognized as the
    /// document title exactly as without the BOM (F-I, mirrors Asciidoctor).
    #[test]
    fn strips_leading_bom() {
        let with_bom: Vec<_> = Parser::new("\u{feff}= Title\n\nbody").collect();
        let without: Vec<_> = Parser::new("= Title\n\nbody").collect();
        assert_eq!(with_bom, without);
        // Sanity: the title is actually parsed as a heading, not a paragraph.
        assert!(
            with_bom
                .iter()
                .any(|e| matches!(e, Event::Text(t) if t.as_ref() == "Title")),
            "expected a Title text event, got {with_bom:?}"
        );
    }

    /// Only the leading BOM is removed; a BOM in the middle of text is kept.
    #[test]
    fn keeps_non_leading_bom() {
        let events: Vec<_> = Parser::new("a\u{feff}b").collect();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Text(t) if t.contains('\u{feff}'))),
            "interior BOM must be preserved, got {events:?}"
        );
    }
}
