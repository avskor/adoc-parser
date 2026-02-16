use std::borrow::Cow;

use crate::event::{Event, Tag, TagEnd};

pub struct InlineParser;

impl InlineParser {
    pub fn parse_str<'a>(text: &'a str) -> Vec<Event<'a>> {
        if text.is_empty() {
            return vec![Event::Text(Cow::Borrowed(""))];
        }

        let mut events = Vec::new();
        let mut parser = InlineState::new(text);
        parser.parse_inline(&mut events);

        if events.is_empty() {
            vec![Event::Text(Cow::Borrowed(text))]
        } else {
            events
        }
    }
}

struct InlineState<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> InlineState<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.input.as_bytes().get(self.pos + offset).copied()
    }

    fn advance_by(&mut self, n: usize) {
        self.pos += n;
    }

    fn parse_inline(&mut self, events: &mut Vec<Event<'a>>) {
        let mut text_start = self.pos;

        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];

            match b {
                // Hard break: ` +` at end of string
                b' ' if self.check_hard_break() => {
                    self.flush_text(text_start, self.pos, events);
                    self.advance_by(2);
                    events.push(Event::HardBreak);
                    text_start = self.pos;
                }

                // Unconstrained formatting: double markers
                b'*' if self.peek_at(1) == Some(b'*') => {
                    if self.try_unconstrained(b'*', Tag::Strong, TagEnd::Strong, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'_' if self.peek_at(1) == Some(b'_') => {
                    if self.try_unconstrained(b'_', Tag::Emphasis, TagEnd::Emphasis, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'`' if self.peek_at(1) == Some(b'`') => {
                    if self.try_unconstrained(b'`', Tag::Monospace, TagEnd::Monospace, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'#' if self.peek_at(1) == Some(b'#') => {
                    if self.try_unconstrained(b'#', Tag::Highlight, TagEnd::Highlight, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Constrained formatting: single markers
                b'*' => {
                    if self.try_constrained(b'*', Tag::Strong, TagEnd::Strong, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'_' => {
                    if self.try_constrained(b'_', Tag::Emphasis, TagEnd::Emphasis, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'`' => {
                    if self.try_constrained(b'`', Tag::Monospace, TagEnd::Monospace, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'#' => {
                    if self.try_constrained(b'#', Tag::Highlight, TagEnd::Highlight, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Superscript ^text^
                b'^' => {
                    if self.try_simple_pair(b'^', Tag::Superscript, TagEnd::Superscript, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Subscript ~text~
                b'~' => {
                    if self.try_simple_pair(b'~', Tag::Subscript, TagEnd::Subscript, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Cross-reference <<id>> or <<id,label>>
                b'<' if self.peek_at(1) == Some(b'<') => {
                    if self.try_cross_reference(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Link macro: link:url[text]
                b'l' if self.remaining().starts_with("link:") => {
                    if self.try_link_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Inline image: image:path[alt] (not image::path[alt])
                b'i' if self.remaining().starts_with("image:") && !self.remaining().starts_with("image::") => {
                    if self.try_inline_image(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Attribute reference {name}
                b'{' => {
                    if self.try_attribute_reference(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Autolink: http:// or https://
                b'h' if self.remaining().starts_with("http://") || self.remaining().starts_with("https://") => {
                    if self.try_autolink(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Anchor [[id]]
                b'[' if self.peek_at(1) == Some(b'[') => {
                    if self.try_anchor(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                _ => {
                    self.pos += 1;
                }
            }
        }

        self.flush_text(text_start, self.pos, events);
    }

    fn flush_text(&self, start: usize, end: usize, events: &mut Vec<Event<'a>>) {
        if start < end {
            events.push(Event::Text(Cow::Borrowed(&self.input[start..end])));
        }
    }

    fn check_hard_break(&self) -> bool {
        self.pos + 2 <= self.input.len()
            && self.input.as_bytes()[self.pos] == b' '
            && self.input.as_bytes()[self.pos + 1] == b'+'
            && self.pos + 2 == self.input.len()
    }

    fn is_word_char_before(&self, pos: usize) -> bool {
        if pos == 0 {
            return false;
        }
        let b = self.input.as_bytes()[pos - 1];
        b.is_ascii_alphanumeric() || b == b'_'
    }

    fn is_word_char_after(&self, pos: usize) -> bool {
        if pos >= self.input.len() {
            return false;
        }
        let b = self.input.as_bytes()[pos];
        b.is_ascii_alphanumeric() || b == b'_'
    }

    fn try_constrained(
        &mut self,
        marker: u8,
        tag: Tag<'a>,
        tag_end: TagEnd,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;

        if self.is_word_char_before(start_pos) {
            return false;
        }

        let after_marker = start_pos + 1;
        if after_marker >= self.input.len() {
            return false;
        }
        if self.input.as_bytes()[after_marker] == b' ' {
            return false;
        }

        if let Some(close_offset) = self.find_closing_constrained(marker, after_marker) {
            let close_pos = after_marker + close_offset;
            let inner = &self.input[after_marker..close_pos];

            if inner.ends_with(' ') {
                return false;
            }

            let after_close = close_pos + 1;
            if self.is_word_char_after(after_close) {
                return false;
            }

            self.flush_text(*text_start, start_pos, events);
            events.push(Event::Start(tag));

            let mut inner_parser = InlineState::new(inner);
            inner_parser.parse_inline(events);

            events.push(Event::End(tag_end));

            self.pos = after_close;
            *text_start = self.pos;
            return true;
        }

        false
    }

    fn try_simple_pair(
        &mut self,
        marker: u8,
        tag: Tag<'a>,
        tag_end: TagEnd,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_marker = start_pos + 1;

        if after_marker >= self.input.len() {
            return false;
        }

        // Find matching closing marker
        let bytes = &self.input.as_bytes()[after_marker..];
        for (i, &b) in bytes.iter().enumerate() {
            if b == marker && i > 0 {
                let close_pos = after_marker + i;
                let inner = &self.input[after_marker..close_pos];

                self.flush_text(*text_start, start_pos, events);
                events.push(Event::Start(tag));
                events.push(Event::Text(Cow::Borrowed(inner)));
                events.push(Event::End(tag_end));

                self.pos = close_pos + 1;
                *text_start = self.pos;
                return true;
            }
        }

        false
    }

    fn find_closing_constrained(&self, marker: u8, search_start: usize) -> Option<usize> {
        let bytes = &self.input.as_bytes()[search_start..];
        for (i, &b) in bytes.iter().enumerate() {
            if b == marker && i > 0 {
                let next = bytes.get(i + 1).copied();
                if next != Some(marker) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn try_unconstrained(
        &mut self,
        marker: u8,
        tag: Tag<'a>,
        tag_end: TagEnd,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_markers = start_pos + 2;

        if after_markers >= self.input.len() {
            return false;
        }

        if let Some(close_pos) = self.find_closing_unconstrained(marker, after_markers) {
            let inner = &self.input[after_markers..close_pos];
            if inner.is_empty() {
                return false;
            }

            self.flush_text(*text_start, start_pos, events);
            events.push(Event::Start(tag));

            let mut inner_parser = InlineState::new(inner);
            inner_parser.parse_inline(events);

            events.push(Event::End(tag_end));

            self.pos = close_pos + 2;
            *text_start = self.pos;
            return true;
        }

        false
    }

    fn find_closing_unconstrained(&self, marker: u8, search_start: usize) -> Option<usize> {
        let bytes = self.input.as_bytes();
        let mut i = search_start;
        while i + 1 < bytes.len() {
            if bytes[i] == marker && bytes[i + 1] == marker {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    fn try_cross_reference(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_open = start_pos + 2;

        let rest = &self.input[after_open..];
        let close = match rest.find(">>") {
            Some(c) => c,
            None => return false,
        };
        let content = &rest[..close];

        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        let (target, label) = if let Some((t, l)) = content.split_once(',') {
            (t.trim(), Some(Cow::Borrowed(l.trim())))
        } else {
            (content, None)
        };

        events.push(Event::Start(Tag::CrossReference {
            target: Cow::Borrowed(target),
            label: label.clone(),
        }));
        let display = label.unwrap_or(Cow::Borrowed(target));
        events.push(Event::Text(display));
        events.push(Event::End(TagEnd::CrossReference));

        self.pos = after_open + close + 2;
        *text_start = self.pos;
        true
    }

    fn try_link_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 5..]; // skip "link:"

        let bracket_start = match rest.find('[') {
            Some(p) => p,
            None => return false,
        };
        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };
        if bracket_end <= bracket_start {
            return false;
        }

        let url = &rest[..bracket_start];
        let link_text = &rest[bracket_start + 1..bracket_end];

        if url.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::Start(Tag::Link {
            url: Cow::Borrowed(url),
        }));
        let display = if link_text.is_empty() { url } else { link_text };
        events.push(Event::Text(Cow::Borrowed(display)));
        events.push(Event::End(TagEnd::Link));

        self.pos = start_pos + 5 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_inline_image(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 6..]; // skip "image:"

        let bracket_start = match rest.find('[') {
            Some(p) => p,
            None => return false,
        };
        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };
        if bracket_end <= bracket_start {
            return false;
        }

        let target = &rest[..bracket_start];
        let alt = &rest[bracket_start + 1..bracket_end];

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::Start(Tag::InlineImage {
            target: Cow::Borrowed(target),
            alt: Cow::Borrowed(alt),
        }));
        events.push(Event::End(TagEnd::InlineImage));

        self.pos = start_pos + 6 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_attribute_reference(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 1..]; // skip '{'

        let close = match rest.find('}') {
            Some(c) => c,
            None => return false,
        };
        let name = &rest[..close];

        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::AttributeReference(Cow::Borrowed(name)));

        self.pos = start_pos + 1 + close + 1;
        *text_start = self.pos;
        true
    }

    fn try_autolink(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos..];

        let url_end = rest
            .find(|c: char| c.is_whitespace() || c == '[' || c == ']' || c == '<' || c == '>')
            .unwrap_or(rest.len());

        let url = &rest[..url_end];
        if url.len() <= 8 {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::Start(Tag::Link {
            url: Cow::Borrowed(url),
        }));
        events.push(Event::Text(Cow::Borrowed(url)));
        events.push(Event::End(TagEnd::Link));

        self.pos = start_pos + url_end;
        *text_start = self.pos;
        true
    }

    fn try_anchor(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 2..]; // skip "[["

        let close = match rest.find("]]") {
            Some(c) => c,
            None => return false,
        };
        let id = &rest[..close];

        if id.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::Start(Tag::Anchor {
            id: Cow::Borrowed(id),
        }));
        events.push(Event::End(TagEnd::Anchor));

        self.pos = start_pos + 2 + close + 2;
        *text_start = self.pos;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<Event<'_>> {
        InlineParser::parse_str(text)
    }

    #[test]
    fn test_plain_text() {
        let events = parse("hello world");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("hello world"))]);
    }

    #[test]
    fn test_bold() {
        let events = parse("hello *bold* world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Start(Tag::Strong),
            Event::Text(Cow::Borrowed("bold")),
            Event::End(TagEnd::Strong),
            Event::Text(Cow::Borrowed(" world")),
        ]);
    }

    #[test]
    fn test_italic() {
        let events = parse("hello _italic_ world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Start(Tag::Emphasis),
            Event::Text(Cow::Borrowed("italic")),
            Event::End(TagEnd::Emphasis),
            Event::Text(Cow::Borrowed(" world")),
        ]);
    }

    #[test]
    fn test_monospace() {
        let events = parse("use `code` here");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("use ")),
            Event::Start(Tag::Monospace),
            Event::Text(Cow::Borrowed("code")),
            Event::End(TagEnd::Monospace),
            Event::Text(Cow::Borrowed(" here")),
        ]);
    }

    #[test]
    fn test_highlight() {
        let events = parse("the #highlight# text");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("the ")),
            Event::Start(Tag::Highlight),
            Event::Text(Cow::Borrowed("highlight")),
            Event::End(TagEnd::Highlight),
            Event::Text(Cow::Borrowed(" text")),
        ]);
    }

    #[test]
    fn test_superscript() {
        let events = parse("E=mc^2^");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("E=mc")),
            Event::Start(Tag::Superscript),
            Event::Text(Cow::Borrowed("2")),
            Event::End(TagEnd::Superscript),
        ]);
    }

    #[test]
    fn test_subscript() {
        let events = parse("H~2~O");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("H")),
            Event::Start(Tag::Subscript),
            Event::Text(Cow::Borrowed("2")),
            Event::End(TagEnd::Subscript),
            Event::Text(Cow::Borrowed("O")),
        ]);
    }

    #[test]
    fn test_unconstrained_bold() {
        let events = parse("hel**lo wo**rld");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hel")),
            Event::Start(Tag::Strong),
            Event::Text(Cow::Borrowed("lo wo")),
            Event::End(TagEnd::Strong),
            Event::Text(Cow::Borrowed("rld")),
        ]);
    }

    #[test]
    fn test_cross_reference() {
        let events = parse("see <<my-section>>");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("see ")),
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("my-section"),
                label: None,
            }),
            Event::Text(Cow::Borrowed("my-section")),
            Event::End(TagEnd::CrossReference),
        ]);
    }

    #[test]
    fn test_cross_reference_with_label() {
        let events = parse("see <<my-section, My Section>>");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("see ")),
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("my-section"),
                label: Some(Cow::Borrowed("My Section")),
            }),
            Event::Text(Cow::Borrowed("My Section")),
            Event::End(TagEnd::CrossReference),
        ]);
    }

    #[test]
    fn test_link_macro() {
        let events = parse("click link:https://example.com[here]");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("click ")),
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://example.com"),
            }),
            Event::Text(Cow::Borrowed("here")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_inline_image() {
        let events = parse("see image:icon.png[icon]");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("see ")),
            Event::Start(Tag::InlineImage {
                target: Cow::Borrowed("icon.png"),
                alt: Cow::Borrowed("icon"),
            }),
            Event::End(TagEnd::InlineImage),
        ]);
    }

    #[test]
    fn test_attribute_reference() {
        let events = parse("version {version}");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("version ")),
            Event::AttributeReference(Cow::Borrowed("version")),
        ]);
    }

    #[test]
    fn test_autolink() {
        let events = parse("visit https://example.com for info");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("visit ")),
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://example.com"),
            }),
            Event::Text(Cow::Borrowed("https://example.com")),
            Event::End(TagEnd::Link),
            Event::Text(Cow::Borrowed(" for info")),
        ]);
    }

    #[test]
    fn test_hard_break() {
        let events = parse("line one +");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("line one")),
            Event::HardBreak,
        ]);
    }

    #[test]
    fn test_anchor() {
        let events = parse("[[my-anchor]]text");
        assert_eq!(events, vec![
            Event::Start(Tag::Anchor { id: Cow::Borrowed("my-anchor") }),
            Event::End(TagEnd::Anchor),
            Event::Text(Cow::Borrowed("text")),
        ]);
    }

    #[test]
    fn test_nested_formatting() {
        let events = parse("*bold _and italic_*");
        assert_eq!(events, vec![
            Event::Start(Tag::Strong),
            Event::Text(Cow::Borrowed("bold ")),
            Event::Start(Tag::Emphasis),
            Event::Text(Cow::Borrowed("and italic")),
            Event::End(TagEnd::Emphasis),
            Event::End(TagEnd::Strong),
        ]);
    }
}
