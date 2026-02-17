use std::borrow::Cow;

use crate::event::{Event, Tag, TagEnd};

fn apply_typographic_replacements<'a>(text: &'a str) -> Cow<'a, str> {
    // Quick check: if none of the trigger characters are present, return borrowed
    if !text.contains('-') && !text.contains('.') && !text.contains('(') {
        return Cow::Borrowed(text);
    }

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut result: Option<String> = None;
    let mut i = 0;
    let mut copied_up_to = 0;

    while i < len {
        let replacement: Option<(&str, usize)> = match bytes[i] {
            b'-' if i + 2 < len && bytes[i + 1] == b'-' && bytes[i + 2] == b'-' => {
                Some(("\u{2014}", 3)) // em-dash
            }
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                Some(("\u{2013}", 2)) // en-dash
            }
            b'.' if i + 2 < len && bytes[i + 1] == b'.' && bytes[i + 2] == b'.' => {
                Some(("\u{2026}", 3)) // ellipsis
            }
            b'(' if i + 2 < len
                && bytes[i + 1] == b'C'
                && bytes[i + 2] == b')'
                && !matches!(bytes.get(i + 3), Some(b'A'..=b'Z' | b'a'..=b'z')) =>
            {
                Some(("\u{00A9}", 3)) // copyright
            }
            b'(' if i + 2 < len
                && bytes[i + 1] == b'R'
                && bytes[i + 2] == b')'
                && !matches!(bytes.get(i + 3), Some(b'A'..=b'Z' | b'a'..=b'z')) =>
            {
                Some(("\u{00AE}", 3)) // registered
            }
            b'(' if i + 4 <= len
                && bytes[i + 1] == b'T'
                && bytes[i + 2] == b'M'
                && bytes[i + 3] == b')' =>
            {
                Some(("\u{2122}", 4)) // trademark
            }
            _ => None,
        };

        if let Some((repl, skip)) = replacement {
            let buf = result.get_or_insert_with(|| String::with_capacity(len));
            buf.push_str(&text[copied_up_to..i]);
            buf.push_str(repl);
            i += skip;
            copied_up_to = i;
        } else {
            i += 1;
        }
    }

    match result {
        Some(mut buf) => {
            buf.push_str(&text[copied_up_to..]);
            Cow::Owned(buf)
        }
        None => Cow::Borrowed(text),
    }
}

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
                // Escape typographic patterns: \--- \-- \... \(C) \(R) \(TM)
                b'\\' if self.typographic_escape_len() > 0 => {
                    self.flush_text(text_start, self.pos, events);
                    let skip = self.typographic_escape_len();
                    self.advance_by(1); // skip backslash
                    let pattern_start = self.pos;
                    self.advance_by(skip);
                    // Emit pattern as plain text, bypassing typographic replacements
                    events.push(Event::Text(Cow::Borrowed(&self.input[pattern_start..self.pos])));
                    text_start = self.pos;
                }

                // Escape pass macro: \pass:[...] → literal "pass:[" + normal inline parsing of content
                b'\\' if self.input.get(self.pos + 1..).is_some_and(|s| s.starts_with("pass:[")) => {
                    self.flush_text(text_start, self.pos, events);
                    self.advance_by(1); // skip backslash
                    text_start = self.pos;
                    self.advance_by(6); // skip "pass:[" — included in next text flush as literal
                }

                // Escape plus sequences: \+, \++, \+++
                b'\\' if self.peek_at(1) == Some(b'+') => {
                    self.flush_text(text_start, self.pos, events);
                    self.advance_by(1); // skip backslash
                    text_start = self.pos;
                    while self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'+' {
                        self.advance_by(1);
                    }
                }

                // Backslash escape: \* \_ \` \# \^ \~ \{ \[ \< \\
                b'\\' if self.peek_at(1).is_some_and(|c| matches!(c, b'*' | b'_' | b'`' | b'#' | b'^' | b'~' | b'{' | b'[' | b'<' | b'\\')) => {
                    self.flush_text(text_start, self.pos, events);
                    self.advance_by(1); // skip backslash
                    text_start = self.pos;
                    self.advance_by(1); // skip escaped char (included in next text flush)
                }

                // Hard break: ` +` at end of string
                b' ' if self.check_hard_break() => {
                    self.flush_text(text_start, self.pos, events);
                    self.advance_by(2);
                    events.push(Event::HardBreak);
                    text_start = self.pos;
                }

                // Triple-plus passthrough: +++text+++
                b'+' if self.peek_at(1) == Some(b'+') && self.peek_at(2) == Some(b'+') => {
                    if self.try_triple_plus_passthrough(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Double-plus passthrough: ++text++
                b'+' if self.peek_at(1) == Some(b'+') => {
                    if self.try_double_plus_passthrough(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Single-plus passthrough: +text+
                b'+' => {
                    if self.try_single_plus_passthrough(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
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

                // Pass macro: pass:[text]
                b'p' if self.remaining().starts_with("pass:") => {
                    if self.try_pass_macro(events, &mut text_start) {
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

                // Footnote macro: footnote:[text] or footnote:id[text] or footnote:id[]
                b'f' if self.remaining().starts_with("footnote:") => {
                    if self.try_footnote_macro(events, &mut text_start) {
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
            let text = &self.input[start..end];
            events.push(Event::Text(apply_typographic_replacements(text)));
        }
    }

    /// Returns the length of a typographic pattern following a backslash, or 0 if none.
    fn typographic_escape_len(&self) -> usize {
        let bytes = self.input.as_bytes();
        let p = self.pos + 1; // position after backslash
        if p >= bytes.len() {
            return 0;
        }
        match bytes[p] {
            b'-' if p + 1 < bytes.len() && bytes[p + 1] == b'-' => {
                if p + 2 < bytes.len() && bytes[p + 2] == b'-' { 3 } else { 2 }
            }
            b'.' if p + 2 < bytes.len() && bytes[p + 1] == b'.' && bytes[p + 2] == b'.' => 3,
            b'(' if p + 2 < bytes.len() && bytes[p + 2] == b')' && (bytes[p + 1] == b'C' || bytes[p + 1] == b'R') => 3,
            b'(' if p + 3 < bytes.len() && bytes[p + 1] == b'T' && bytes[p + 2] == b'M' && bytes[p + 3] == b')' => 4,
            _ => 0,
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

    fn try_triple_plus_passthrough(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_open = start_pos + 3; // skip "+++"

        let rest = &self.input[after_open..];
        let close = match rest.find("+++") {
            Some(c) => c,
            None => return false,
        };

        if close == 0 {
            return false;
        }

        let inner = &rest[..close];

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::InlinePassthrough(Cow::Borrowed(inner)));

        self.pos = after_open + close + 3;
        *text_start = self.pos;
        true
    }

    fn try_double_plus_passthrough(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_open = start_pos + 2; // skip "++"

        let rest = &self.input[after_open..];
        let close = match rest.find("++") {
            Some(c) => c,
            None => return false,
        };

        if close == 0 {
            return false;
        }

        let inner = &rest[..close];

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::InlinePassthrough(Cow::Borrowed(inner)));

        self.pos = after_open + close + 2;
        *text_start = self.pos;
        true
    }

    fn try_single_plus_passthrough(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;

        // Must not be followed by another '+' (that would be ++ or +++)
        if self.peek_at(1) == Some(b'+') {
            return false;
        }

        let after_open = start_pos + 1; // skip "+"
        if after_open >= self.input.len() {
            return false;
        }

        // Find closing single '+' that is not part of '++' or '+++'
        let bytes = &self.input.as_bytes()[after_open..];
        let mut close = None;
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'+' && i > 0 {
                // Check it's not preceded by '+' or followed by '+'
                let preceded_by_plus = bytes.get(i.wrapping_sub(1)).copied() == Some(b'+');
                let followed_by_plus = bytes.get(i + 1).copied() == Some(b'+');
                if !preceded_by_plus && !followed_by_plus {
                    close = Some(i);
                    break;
                }
            }
        }

        let close = match close {
            Some(c) => c,
            None => return false,
        };

        let inner = &self.input[after_open..after_open + close];

        self.flush_text(*text_start, start_pos, events);
        // Single-plus: emit as plain Text (no inline parsing, no typographic replacements)
        events.push(Event::Text(Cow::Borrowed(inner)));

        self.pos = after_open + close + 1;
        *text_start = self.pos;
        true
    }

    fn try_pass_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 5..]; // skip "pass:"

        if !rest.starts_with('[') {
            return false;
        }

        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };

        let inner = &rest[1..bracket_end];

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::InlinePassthrough(Cow::Borrowed(inner)));

        self.pos = start_pos + 5 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_footnote_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_prefix = start_pos + 9; // skip "footnote:"
        let rest = &self.input[after_prefix..];

        if rest.is_empty() {
            return false;
        }

        // Determine if anonymous (starts with '[') or named (id before '[')
        let (id, bracket_rest) = if rest.starts_with('[') {
            (None, rest)
        } else {
            let bracket_pos = match rest.find('[') {
                Some(p) => p,
                None => return false,
            };
            let id = &rest[..bracket_pos];
            if id.is_empty() {
                return false;
            }
            (Some(id), &rest[bracket_pos..])
        };

        // bracket_rest starts with '['
        let bracket_end = match bracket_rest.find(']') {
            Some(p) => p,
            None => return false,
        };

        let content = &bracket_rest[1..bracket_end];

        self.flush_text(*text_start, start_pos, events);

        if let (Some(id_str), true) = (id, content.is_empty()) {
            // footnote:id[] — reference to existing footnote
            events.push(Event::FootnoteRef {
                id: Cow::Borrowed(id_str),
            });
        } else {
            // footnote:[text] or footnote:id[text]
            events.push(Event::Footnote {
                id: id.map(Cow::Borrowed),
                text: Cow::Borrowed(content),
            });
        }

        // Calculate full consumed length
        let id_len = id.map_or(0, |s| s.len());
        // "footnote:" + id + "[" + content + "]"
        self.pos = after_prefix + id_len + 1 + bracket_end;
        *text_start = self.pos;
        true
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
    fn test_escaped_bold() {
        let events = parse("hello \\*not bold* world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Text(Cow::Borrowed("*not bold* world")),
        ]);
    }

    #[test]
    fn test_escaped_italic() {
        let events = parse("hello \\_not italic_ world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Text(Cow::Borrowed("_not italic_ world")),
        ]);
    }

    #[test]
    fn test_escaped_monospace() {
        let events = parse("hello \\`not code` world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Text(Cow::Borrowed("`not code` world")),
        ]);
    }

    #[test]
    fn test_escaped_attribute_reference() {
        let events = parse("use \\{name} literally");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("use ")),
            Event::Text(Cow::Borrowed("{name} literally")),
        ]);
    }

    #[test]
    fn test_escaped_cross_reference() {
        let events = parse("not \\<<a ref>>");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("not ")),
            Event::Text(Cow::Borrowed("<<a ref>>")),
        ]);
    }

    #[test]
    fn test_escaped_backslash() {
        let events = parse("a \\\\ b");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("a ")),
            Event::Text(Cow::Borrowed("\\ b")),
        ]);
    }

    #[test]
    fn test_backslash_before_normal_char() {
        // Backslash before non-special char is kept as-is
        let events = parse("hello \\world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello \\world")),
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

    // Typographic replacement tests

    #[test]
    fn test_typographic_em_dash() {
        let events = parse("hello---world");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("hello\u{2014}world".to_string())),
        ]);
    }

    #[test]
    fn test_typographic_en_dash() {
        let events = parse("hello--world");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("hello\u{2013}world".to_string())),
        ]);
    }

    #[test]
    fn test_typographic_ellipsis() {
        let events = parse("wait...");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("wait\u{2026}".to_string())),
        ]);
    }

    #[test]
    fn test_typographic_copyright() {
        let events = parse("(C) 2024");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("\u{00A9} 2024".to_string())),
        ]);
    }

    #[test]
    fn test_typographic_registered() {
        let events = parse("Name(R)");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("Name\u{00AE}".to_string())),
        ]);
    }

    #[test]
    fn test_typographic_trademark() {
        let events = parse("Brand(TM)");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("Brand\u{2122}".to_string())),
        ]);
    }

    #[test]
    fn test_typographic_no_match() {
        let result = apply_typographic_replacements("hello world");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_typographic_mixed() {
        let events = parse("(C) 2024---all rights...");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("\u{00A9} 2024\u{2014}all rights\u{2026}".to_string())),
        ]);
    }

    #[test]
    fn test_escaped_en_dash_no_replacement() {
        let events = parse("hello \\-- world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Text(Cow::Borrowed("--")),
            Event::Text(Cow::Borrowed(" world")),
        ]);
    }

    #[test]
    fn test_escaped_em_dash_no_replacement() {
        let events = parse("hello \\--- world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Text(Cow::Borrowed("---")),
            Event::Text(Cow::Borrowed(" world")),
        ]);
    }

    #[test]
    fn test_escaped_ellipsis_no_replacement() {
        let events = parse("wait\\...");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("wait")),
            Event::Text(Cow::Borrowed("...")),
        ]);
    }

    #[test]
    fn test_escaped_copyright_no_replacement() {
        let events = parse("\\(C) 2024");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("(C)")),
            Event::Text(Cow::Borrowed(" 2024")),
        ]);
    }

    // Passthrough tests

    #[test]
    fn test_single_plus_passthrough() {
        let events = parse("hello +*not bold*+ world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Text(Cow::Borrowed("*not bold*")),
            Event::Text(Cow::Borrowed(" world")),
        ]);
    }

    #[test]
    fn test_triple_plus_passthrough() {
        let events = parse("hello +++<b>raw</b>+++ world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::InlinePassthrough(Cow::Borrowed("<b>raw</b>")),
            Event::Text(Cow::Borrowed(" world")),
        ]);
    }

    #[test]
    fn test_pass_macro() {
        let events = parse("hello pass:[<em>raw</em>] world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::InlinePassthrough(Cow::Borrowed("<em>raw</em>")),
            Event::Text(Cow::Borrowed(" world")),
        ]);
    }

    #[test]
    fn test_single_plus_no_typographic() {
        let events = parse("+(C) 2024+");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("(C) 2024")),
        ]);
    }

    #[test]
    fn test_passthrough_empty() {
        // Empty ++ should not match as passthrough
        let events = parse("hello ++ world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ++ world")),
        ]);
    }

    #[test]
    fn test_escaped_plus_no_passthrough() {
        let events = parse("hello \\+text+ world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Text(Cow::Borrowed("+text+ world")),
        ]);
    }

    // Footnote tests

    #[test]
    fn test_footnote_basic() {
        let events = parse("footnote:[text]");
        assert_eq!(events, vec![
            Event::Footnote { id: None, text: Cow::Borrowed("text") },
        ]);
    }

    #[test]
    fn test_footnote_named() {
        let events = parse("footnote:fn1[text]");
        assert_eq!(events, vec![
            Event::Footnote { id: Some(Cow::Borrowed("fn1")), text: Cow::Borrowed("text") },
        ]);
    }

    #[test]
    fn test_footnote_ref() {
        let events = parse("footnote:fn1[]");
        assert_eq!(events, vec![
            Event::FootnoteRef { id: Cow::Borrowed("fn1") },
        ]);
    }

    #[test]
    fn test_footnote_in_sentence() {
        let events = parse("Hello footnote:[note] world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("Hello ")),
            Event::Footnote { id: None, text: Cow::Borrowed("note") },
            Event::Text(Cow::Borrowed(" world")),
        ]);
    }
}
