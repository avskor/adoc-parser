use std::borrow::Cow;

use crate::attributes::parse_link_attrs;
use crate::event::{Event, SubstitutionSet, Tag, TagEnd};

fn apply_typographic_replacements<'a>(text: &'a str) -> Cow<'a, str> {
    // Quick check: if none of the trigger characters are present, return borrowed
    if !text.contains('-') && !text.contains('.') && !text.contains('(') && !text.contains('\'')
        && !text.contains("->") && !text.contains("<-")
        && !text.contains("=>") && !text.contains("<=")
    {
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
            // -> right arrow (but not -->)
            b'-' if i + 1 < len && bytes[i + 1] == b'>'
                && !(i + 2 < len && bytes[i + 2] == b'>') =>
            {
                Some(("\u{2192}", 2)) // →
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
            b'\'' if i > 0
                && bytes[i - 1].is_ascii_alphanumeric()
                && i + 1 < len
                && bytes[i + 1].is_ascii_alphanumeric() =>
            {
                Some(("\u{2019}", 1))
            }
            // => double right arrow (but not ==>)
            b'=' if i + 1 < len && bytes[i + 1] == b'>'
                && !(i + 2 < len && bytes[i + 2] == b'>') =>
            {
                Some(("\u{21D2}", 2)) // ⇒
            }
            // <- left arrow (but not <--)
            b'<' if i + 1 < len && bytes[i + 1] == b'-'
                && !(i + 2 < len && bytes[i + 2] == b'-') =>
            {
                Some(("\u{2190}", 2)) // ←
            }
            // <= double left arrow (but not <==)
            b'<' if i + 1 < len && bytes[i + 1] == b'='
                && !(i + 2 < len && bytes[i + 2] == b'=') =>
            {
                Some(("\u{21D0}", 2)) // ⇐
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
    #[cfg(test)]
    pub fn parse_str<'a>(text: &'a str) -> Vec<Event<'a>> {
        Self::parse_str_with_subs(text, SubstitutionSet::NORMAL)
    }

    pub fn parse_str_with_subs<'a>(text: &'a str, subs: SubstitutionSet) -> Vec<Event<'a>> {
        if text.is_empty() {
            return vec![Event::Text(Cow::Borrowed(""))];
        }

        let mut events = Vec::new();
        let mut parser = InlineState::new(text, subs);
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
    subs: SubstitutionSet,
}

impl<'a> InlineState<'a> {
    fn new(input: &'a str, subs: SubstitutionSet) -> Self {
        Self { input, pos: 0, subs }
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
        let has_quotes = self.subs.has(SubstitutionSet::QUOTES);
        let has_macros = self.subs.has(SubstitutionSet::MACROS);
        let has_attributes = self.subs.has(SubstitutionSet::ATTRIBUTES);
        let has_post_replacements = self.subs.has(SubstitutionSet::POST_REPLACEMENTS);

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

                // Escape smart quote openers: \"` or \'`
                b'\\' if has_quotes
                    && self.peek_at(1).is_some_and(|c| c == b'"' || c == b'\'')
                    && self.peek_at(2) == Some(b'`') =>
                {
                    self.flush_text(text_start, self.pos, events);
                    self.advance_by(1); // skip backslash
                    text_start = self.pos;
                    self.advance_by(2); // skip quote + backtick (literal text in next flush)
                }

                // Backslash escape: \* \_ \` \# \^ \~ \{ \[ \< \\
                b'\\' if self.peek_at(1).is_some_and(|c| matches!(c, b'*' | b'_' | b'`' | b'#' | b'^' | b'~' | b'{' | b'[' | b'<' | b'\\' | b'\'')) => {
                    self.flush_text(text_start, self.pos, events);
                    self.advance_by(1); // skip backslash
                    text_start = self.pos;
                    self.advance_by(1); // skip escaped char (included in next text flush)
                }

                // Hard break: ` +` at end of string or before `\n`
                b' ' if has_post_replacements && self.check_hard_break() => {
                    self.flush_text(text_start, self.pos, events);
                    self.advance_by(2); // skip ` +`
                    if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'\n' {
                        self.advance_by(1); // skip `\n`
                    }
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

                // Unconstrained formatting: double markers (QUOTES)
                b'*' if has_quotes && self.peek_at(1) == Some(b'*') => {
                    if self.try_unconstrained(b'*', Tag::Strong, TagEnd::Strong, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'_' if has_quotes && self.peek_at(1) == Some(b'_') => {
                    if self.try_unconstrained(b'_', Tag::Emphasis, TagEnd::Emphasis, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Smart double quotes: "`text`" (QUOTES)
                b'"' if has_quotes && self.peek_at(1) == Some(b'`') => {
                    if self.try_smart_quotes(b'"', events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Smart single quotes: '`text`' (QUOTES)
                b'\'' if has_quotes && self.peek_at(1) == Some(b'`') => {
                    if self.try_smart_quotes(b'\'', events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                b'`' if has_quotes && self.peek_at(1) == Some(b'`') => {
                    if self.try_unconstrained(b'`', Tag::Monospace, TagEnd::Monospace, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'#' if has_quotes && self.peek_at(1) == Some(b'#') => {
                    if self.try_unconstrained(b'#', Tag::Highlight, TagEnd::Highlight, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Constrained formatting: single markers (QUOTES)
                b'*' if has_quotes => {
                    if self.try_constrained(b'*', Tag::Strong, TagEnd::Strong, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'_' if has_quotes => {
                    if self.try_constrained(b'_', Tag::Emphasis, TagEnd::Emphasis, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'`' if has_quotes => {
                    if self.try_constrained(b'`', Tag::Monospace, TagEnd::Monospace, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'#' if has_quotes => {
                    if self.try_constrained(b'#', Tag::Highlight, TagEnd::Highlight, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Superscript ^text^ (QUOTES)
                b'^' if has_quotes => {
                    if self.try_simple_pair(b'^', Tag::Superscript, TagEnd::Superscript, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Subscript ~text~ (QUOTES)
                b'~' if has_quotes => {
                    if self.try_simple_pair(b'~', Tag::Subscript, TagEnd::Subscript, events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Cross-reference <<id>> or <<id,label>> (MACROS)
                b'<' if has_macros && self.peek_at(1) == Some(b'<') => {
                    if self.try_cross_reference(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Inline stem macro: stem:[content] (MACROS)
                b's' if has_macros && self.remaining().starts_with("stem:[") => {
                    if self.try_stem_macro(5, "stem", events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Pass macro: pass:[text] (always active)
                b'p' if self.remaining().starts_with("pass:") => {
                    if self.try_pass_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Inline latexmath macro: latexmath:[content] (MACROS)
                b'l' if has_macros && self.remaining().starts_with("latexmath:[") => {
                    if self.try_stem_macro(10, "latexmath", events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Link macro: link:url[text] (MACROS)
                b'l' if has_macros && self.remaining().starts_with("link:") => {
                    if self.try_link_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Index term macros (MACROS)
                b'i' if has_macros && self.remaining().starts_with("indexterm2:") => {
                    if self.try_indexterm2_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }
                b'i' if has_macros && self.remaining().starts_with("indexterm:") => {
                    if self.try_indexterm_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Icon macro: icon:name[attrs] (MACROS)
                b'i' if has_macros && self.remaining().starts_with("icon:") => {
                    if self.try_icon_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Inline image: image:path[alt] (MACROS)
                b'i' if has_macros && self.remaining().starts_with("image:") && !self.remaining().starts_with("image::") => {
                    if self.try_inline_image(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Keyboard macro: kbd:[keys] (MACROS)
                b'k' if has_macros && self.remaining().starts_with("kbd:") => {
                    if self.try_kbd_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Button macro: btn:[label] (MACROS)
                b'b' if has_macros && self.remaining().starts_with("btn:") => {
                    if self.try_btn_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Mailto macro (MACROS)
                b'm' if has_macros && self.remaining().starts_with("mailto:") => {
                    if self.try_mailto_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Menu macro (MACROS)
                b'm' if has_macros && self.remaining().starts_with("menu:") => {
                    if self.try_menu_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Footnote macro (MACROS)
                b'f' if has_macros && self.remaining().starts_with("footnote:") => {
                    if self.try_footnote_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Attribute reference {name} (ATTRIBUTES)
                b'{' if has_attributes => {
                    if self.try_attribute_reference(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Inline asciimath macro (MACROS)
                b'a' if has_macros && self.remaining().starts_with("asciimath:[") => {
                    if self.try_stem_macro(10, "asciimath", events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Xref macro (MACROS)
                b'x' if has_macros && self.remaining().starts_with("xref:") => {
                    if self.try_xref_macro(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Autolink: http:// or https:// (MACROS)
                b'h' if has_macros && (self.remaining().starts_with("http://") || self.remaining().starts_with("https://")) => {
                    if self.try_autolink(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Email autolink: user@example.com (MACROS)
                b'@' if has_macros => {
                    if self.try_email_autolink(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Inline attr span: [.class]#text# or [#id.class]#text# (QUOTES)
                b'[' if has_quotes
                    && self.peek_at(1) != Some(b'[')
                    && self.peek_at(1).is_some_and(|c| c == b'.' || c == b'#') =>
                {
                    if self.try_inline_attr_span(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Bibliography anchor [[[id]]] or [[[id, label]]] (MACROS)
                b'[' if has_macros && self.peek_at(1) == Some(b'[') && self.peek_at(2) == Some(b'[') => {
                    if self.try_bibliography_anchor(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Anchor [[id]] (MACROS)
                b'[' if has_macros && self.peek_at(1) == Some(b'[') => {
                    if self.try_anchor(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Concealed index term (((primary, secondary, tertiary))) or flow index term ((term)) (MACROS)
                b'(' if has_macros && self.peek_at(1) == Some(b'(') && self.peek_at(2) == Some(b'(') => {
                    if self.try_concealed_index_term(events, &mut text_start) {
                        continue;
                    }
                    if self.try_flow_index_term(events, &mut text_start) {
                        continue;
                    }
                    self.pos += 1;
                }

                // Flow index term ((term)) (MACROS)
                b'(' if has_macros && self.peek_at(1) == Some(b'(') => {
                    if self.try_flow_index_term(events, &mut text_start) {
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
            if self.subs.has(SubstitutionSet::REPLACEMENTS) {
                events.push(Event::Text(apply_typographic_replacements(text)));
            } else {
                events.push(Event::Text(Cow::Borrowed(text)));
            }
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
            b'-' if p + 1 < bytes.len() && bytes[p + 1] == b'>' => 2, // \->
            b'=' if p + 1 < bytes.len() && bytes[p + 1] == b'>' => 2, // \=>
            b'<' if p + 1 < bytes.len() && (bytes[p + 1] == b'-' || bytes[p + 1] == b'=') => 2, // \<- or \<=
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
            && (self.pos + 2 == self.input.len()
                || self.input.as_bytes()[self.pos + 2] == b'\n')
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

            let mut inner_parser = InlineState::new(inner, self.subs);
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

            let mut inner_parser = InlineState::new(inner, self.subs);
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

    fn try_kbd_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 4..]; // skip "kbd:"

        if !rest.starts_with('[') {
            return false;
        }

        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };

        let content = &rest[1..bracket_end];
        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Keyboard));
        events.push(Event::Text(Cow::Borrowed(content)));
        events.push(Event::End(TagEnd::Keyboard));

        self.pos = start_pos + 4 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_btn_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 4..]; // skip "btn:"

        if !rest.starts_with('[') {
            return false;
        }

        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };

        let content = &rest[1..bracket_end];
        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Button));
        events.push(Event::Text(Cow::Borrowed(content)));
        events.push(Event::End(TagEnd::Button));

        self.pos = start_pos + 4 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_menu_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 5..]; // skip "menu:"

        let bracket_start = match rest.find('[') {
            Some(p) => p,
            None => return false,
        };

        let target = &rest[..bracket_start];
        if target.is_empty() {
            return false;
        }

        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };
        if bracket_end <= bracket_start {
            return false;
        }

        let items = &rest[bracket_start + 1..bracket_end];

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Menu {
            target: Cow::Borrowed(target),
        }));
        if !items.is_empty() {
            events.push(Event::Text(Cow::Borrowed(items)));
        }
        events.push(Event::End(TagEnd::Menu));

        self.pos = start_pos + 5 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_icon_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 5..]; // skip "icon:"

        let bracket_start = match rest.find('[') {
            Some(p) => p,
            None => return false,
        };

        let name = &rest[..bracket_start];
        if name.is_empty() {
            return false;
        }

        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };
        if bracket_end <= bracket_start {
            return false;
        }

        let attrs = &rest[bracket_start + 1..bracket_end];

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Icon {
            name: Cow::Borrowed(name),
        }));
        if !attrs.is_empty() {
            events.push(Event::Text(Cow::Borrowed(attrs)));
        }
        events.push(Event::End(TagEnd::Icon));

        self.pos = start_pos + 5 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_stem_macro(
        &mut self,
        prefix_len: usize,
        variant: &'a str,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + prefix_len..]; // skip "stem:" / "latexmath:" / "asciimath:"

        if !rest.starts_with('[') {
            return false;
        }

        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };

        let content = &rest[1..bracket_end];

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Stem {
            variant: Cow::Borrowed(variant),
        }));
        if !content.is_empty() {
            events.push(Event::Text(Cow::Borrowed(content)));
        }
        events.push(Event::End(TagEnd::Stem));

        self.pos = start_pos + prefix_len + bracket_end + 1;
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

        // Handle ++url with spaces++ passthrough in URL
        if let Some(after_open) = rest.strip_prefix("++") {
            let close_pp = match after_open.find("++") {
                Some(p) => p,
                None => return false,
            };
            let url = &after_open[..close_pp];
            let after_close = &rest[2 + close_pp + 2..]; // after closing ++
            if !after_close.starts_with('[') {
                return false;
            }
            let bracket_end = match after_close.find(']') {
                Some(p) => p,
                None => return false,
            };
            let bracket_content = &after_close[1..bracket_end];

            if url.is_empty() {
                return false;
            }

            self.flush_text(*text_start, start_pos, events);
            let link_attrs = parse_link_attrs(bracket_content);
            events.push(Event::Start(Tag::Link {
                url: Cow::Borrowed(url),
                window: link_attrs.window.map(Cow::Borrowed),
                nofollow: link_attrs.nofollow,
            }));
            let display = if link_attrs.text.is_empty() { url } else { link_attrs.text };
            events.push(Event::Text(Cow::Borrowed(display)));
            events.push(Event::End(TagEnd::Link));

            self.pos = start_pos + 5 + 2 + close_pp + 2 + bracket_end + 1;
            *text_start = self.pos;
            return true;
        }

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
        let bracket_content = &rest[bracket_start + 1..bracket_end];

        if url.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        let link_attrs = parse_link_attrs(bracket_content);
        events.push(Event::Start(Tag::Link {
            url: Cow::Borrowed(url),
            window: link_attrs.window.map(Cow::Borrowed),
            nofollow: link_attrs.nofollow,
        }));
        let display = if link_attrs.text.is_empty() { url } else { link_attrs.text };
        events.push(Event::Text(Cow::Borrowed(display)));
        events.push(Event::End(TagEnd::Link));

        self.pos = start_pos + 5 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_mailto_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 7..]; // skip "mailto:"

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

        let email = &rest[..bracket_start];
        let bracket_content = &rest[bracket_start + 1..bracket_end];

        if email.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        // Build mailto: URL
        let url = &self.input[start_pos..start_pos + 7 + bracket_start]; // "mailto:email"
        let link_attrs = parse_link_attrs(bracket_content);
        events.push(Event::Start(Tag::Link {
            url: Cow::Borrowed(url),
            window: link_attrs.window.map(Cow::Borrowed),
            nofollow: link_attrs.nofollow,
        }));
        let display = if link_attrs.text.is_empty() { email } else { link_attrs.text };
        events.push(Event::Text(Cow::Borrowed(display)));
        events.push(Event::End(TagEnd::Link));

        self.pos = start_pos + 7 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_xref_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 5..]; // skip "xref:"

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
        let label_text = &rest[bracket_start + 1..bracket_end];

        if target.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        let label = if label_text.is_empty() {
            None
        } else {
            Some(Cow::Borrowed(label_text))
        };

        events.push(Event::Start(Tag::CrossReference {
            target: Cow::Borrowed(target),
            label: label.clone(),
        }));
        let display = label.unwrap_or(Cow::Borrowed(target));
        events.push(Event::Text(display));
        events.push(Event::End(TagEnd::CrossReference));

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
        let bracket_content = &rest[bracket_start + 1..bracket_end];
        let img_attrs = crate::attributes::parse_image_attrs(bracket_content);

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::Start(Tag::InlineImage {
            target: Cow::Borrowed(target),
            alt: Cow::Borrowed(img_attrs.alt),
            width: img_attrs.width.map(Cow::Borrowed),
            height: img_attrs.height.map(Cow::Borrowed),
            align: img_attrs.align.map(Cow::Borrowed),
            float: img_attrs.float.map(Cow::Borrowed),
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
        let content = &rest[..close];

        let (attr_name, fallback) = if let Some(bang) = content.find('!') {
            (&content[..bang], Some(&content[bang + 1..]))
        } else {
            (content, None)
        };

        if attr_name.is_empty()
            || !attr_name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::AttributeReference {
            name: Cow::Borrowed(attr_name),
            fallback: fallback.map(Cow::Borrowed),
        });

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
            window: None,
            nofollow: false,
        }));
        events.push(Event::Text(Cow::Borrowed(url)));
        events.push(Event::End(TagEnd::Link));

        self.pos = start_pos + url_end;
        *text_start = self.pos;
        true
    }

    fn try_email_autolink(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let at_pos = self.pos;
        let bytes = self.input.as_bytes();

        // Scan backwards for local part (a-zA-Z0-9._+-)
        let mut local_start = at_pos;
        while local_start > *text_start {
            let b = bytes[local_start - 1];
            if b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'+' || b == b'-' {
                local_start -= 1;
            } else {
                break;
            }
        }

        // Local part must be non-empty
        if local_start == at_pos {
            return false;
        }

        // Scan forward for domain (a-zA-Z0-9.-)
        let mut domain_end = at_pos + 1;
        let mut has_dot = false;
        while domain_end < bytes.len() {
            let b = bytes[domain_end];
            if b.is_ascii_alphanumeric() || b == b'-' {
                domain_end += 1;
            } else if b == b'.' {
                has_dot = true;
                domain_end += 1;
            } else {
                break;
            }
        }

        // Domain must contain at least one dot
        if !has_dot {
            return false;
        }

        // Domain must not start or end with '.' or '-'
        let domain = &self.input[at_pos + 1..domain_end];
        if domain.starts_with('.') || domain.starts_with('-')
            || domain.ends_with('.') || domain.ends_with('-')
        {
            return false;
        }

        let email = &self.input[local_start..domain_end];

        self.flush_text(*text_start, local_start, events);

        events.push(Event::Start(Tag::Link {
            url: Cow::Owned(format!("mailto:{email}")),
            window: None,
            nofollow: false,
        }));
        events.push(Event::Text(Cow::Borrowed(email)));
        events.push(Event::End(TagEnd::Link));

        self.pos = domain_end;
        *text_start = self.pos;
        true
    }

    fn try_bibliography_anchor(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_open = start_pos + 3; // skip "[[["

        let rest = &self.input[after_open..];
        let close = match rest.find("]]]") {
            Some(c) => c,
            None => return false,
        };
        let content = &rest[..close];

        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        let (id, label) = if let Some((i, l)) = content.split_once(',') {
            let id = i.trim();
            let label = l.trim();
            if id.is_empty() {
                return false;
            }
            (id, Some(Cow::Borrowed(label)))
        } else {
            (content, None)
        };

        events.push(Event::BibliographyAnchor {
            id: Cow::Borrowed(id),
            label,
        });

        self.pos = after_open + close + 3;
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

    fn try_concealed_index_term(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_open = start_pos + 3; // skip "((("

        let rest = &self.input[after_open..];
        let close = match rest.find(")))") {
            Some(c) => c,
            None => return false,
        };

        let content = &rest[..close];
        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        let mut parts = content.splitn(3, ',');
        let primary = parts.next().unwrap().trim();
        let secondary = parts.next().map(|s| s.trim());
        let tertiary = parts.next().map(|s| s.trim());

        events.push(Event::ConcealedIndexTerm {
            primary: Cow::Borrowed(primary),
            secondary: secondary.map(Cow::Borrowed),
            tertiary: tertiary.map(Cow::Borrowed),
        });

        self.pos = after_open + close + 3;
        *text_start = self.pos;
        true
    }

    fn try_flow_index_term(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_open = start_pos + 2; // skip "(("

        let rest = &self.input[after_open..];
        let close = match rest.find("))") {
            Some(c) => c,
            None => return false,
        };

        let content = &rest[..close];
        if content.is_empty() {
            return false;
        }

        // Make sure closing )) is not followed by another ) — that would be )))
        let after_close = after_open + close + 2;
        if after_close < self.input.len() && self.input.as_bytes()[after_close] == b')' {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::IndexTerm {
            text: Cow::Borrowed(content),
        });

        self.pos = after_close;
        *text_start = self.pos;
        true
    }

    fn try_indexterm_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 10..]; // skip "indexterm:"

        if !rest.starts_with('[') {
            return false;
        }

        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };

        let content = &rest[1..bracket_end];
        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        let mut parts = content.splitn(3, ',');
        let primary = parts.next().unwrap().trim();
        let secondary = parts.next().map(|s| s.trim());
        let tertiary = parts.next().map(|s| s.trim());

        events.push(Event::ConcealedIndexTerm {
            primary: Cow::Borrowed(primary),
            secondary: secondary.map(Cow::Borrowed),
            tertiary: tertiary.map(Cow::Borrowed),
        });

        self.pos = start_pos + 10 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    fn try_indexterm2_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 11..]; // skip "indexterm2:"

        if !rest.starts_with('[') {
            return false;
        }

        let bracket_end = match rest.find(']') {
            Some(p) => p,
            None => return false,
        };

        let content = &rest[1..bracket_end];
        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::IndexTerm {
            text: Cow::Borrowed(content),
        });

        self.pos = start_pos + 11 + bracket_end + 1;
        *text_start = self.pos;
        true
    }

    /// Parse inline shorthand notation from bracket content like `#id.class1.class2`.
    /// Returns (id, roles).
    fn parse_inline_shorthand(s: &str) -> (Option<&str>, Vec<&str>) {
        let mut id = None;
        let mut roles = Vec::new();

        let mut rest = s;
        // Parse #id if present
        if let Some(stripped) = rest.strip_prefix('#') {
            rest = stripped;
            let end = rest.find('.').unwrap_or(rest.len());
            let id_str = &rest[..end];
            if !id_str.is_empty() {
                id = Some(id_str);
            }
            rest = &rest[end..];
        }

        // Parse .class1.class2...
        for part in rest.split('.') {
            if !part.is_empty() {
                roles.push(part);
            }
        }

        (id, roles)
    }

    fn try_inline_attr_span(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_bracket = start_pos + 1; // skip '['

        // Find closing ']'
        let bytes = self.input.as_bytes();
        let mut bracket_close = None;
        for (i, &b) in bytes.iter().enumerate().skip(after_bracket) {
            if b == b']' {
                bracket_close = Some(i);
                break;
            }
            // Don't cross newlines
            if b == b'\n' {
                return false;
            }
        }
        let bracket_close = match bracket_close {
            Some(p) => p,
            None => return false,
        };

        let attr_content = &self.input[after_bracket..bracket_close];
        let (id, roles) = Self::parse_inline_shorthand(attr_content);

        // Must have at least an id or a role
        if id.is_none() && roles.is_empty() {
            return false;
        }

        let after_close_bracket = bracket_close + 1;
        if after_close_bracket >= self.input.len() {
            return false;
        }

        // Check for ## (unconstrained) or # (constrained)
        let is_unconstrained = after_close_bracket + 1 < self.input.len()
            && bytes[after_close_bracket] == b'#'
            && bytes[after_close_bracket + 1] == b'#';

        if bytes[after_close_bracket] != b'#' {
            return false;
        }

        if is_unconstrained {
            // Unconstrained: [.class]##text##
            let content_start = after_close_bracket + 2;
            if content_start >= self.input.len() {
                return false;
            }
            let close_pos = match self.find_closing_unconstrained(b'#', content_start) {
                Some(p) => p,
                None => return false,
            };
            let inner = &self.input[content_start..close_pos];
            if inner.is_empty() {
                return false;
            }

            self.flush_text(*text_start, start_pos, events);
            events.push(Event::Start(Tag::InlineSpan {
                id: id.map(Cow::Borrowed),
                roles: roles.iter().copied().map(Cow::Borrowed).collect(),
            }));

            let mut inner_parser = InlineState::new(inner, self.subs);
            inner_parser.parse_inline(events);

            events.push(Event::End(TagEnd::InlineSpan));

            self.pos = close_pos + 2;
            *text_start = self.pos;
            return true;
        }

        // Constrained: [.class]#text#
        let content_start = after_close_bracket + 1;
        if content_start >= self.input.len() {
            return false;
        }
        if bytes[content_start] == b' ' {
            return false;
        }

        if let Some(close_offset) = self.find_closing_constrained(b'#', content_start) {
            let close_pos = content_start + close_offset;
            let inner = &self.input[content_start..close_pos];

            if inner.ends_with(' ') {
                return false;
            }

            let after_close = close_pos + 1;
            if self.is_word_char_after(after_close) {
                return false;
            }

            self.flush_text(*text_start, start_pos, events);
            events.push(Event::Start(Tag::InlineSpan {
                id: id.map(Cow::Borrowed),
                roles: roles.iter().copied().map(Cow::Borrowed).collect(),
            }));

            let mut inner_parser = InlineState::new(inner, self.subs);
            inner_parser.parse_inline(events);

            events.push(Event::End(TagEnd::InlineSpan));

            self.pos = after_close;
            *text_start = self.pos;
            return true;
        }

        false
    }

    fn try_smart_quotes(
        &mut self,
        quote_char: u8,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let after_open = start_pos + 2; // skip quote + backtick

        if after_open >= self.input.len() {
            return false;
        }

        let close_pos = match self.find_smart_quote_close(quote_char, after_open) {
            Some(pos) => pos,
            None => return false,
        };

        let inner = &self.input[after_open..close_pos];
        if inner.is_empty() {
            return false;
        }

        let (open_q, close_q) = if quote_char == b'"' {
            ("\u{201C}", "\u{201D}")
        } else {
            ("\u{2018}", "\u{2019}")
        };

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Text(Cow::Borrowed(open_q)));

        let mut inner_parser = InlineState::new(inner, self.subs);
        inner_parser.parse_inline(events);

        events.push(Event::Text(Cow::Borrowed(close_q)));

        self.pos = close_pos + 2; // skip closing backtick + quote
        *text_start = self.pos;
        true
    }

    fn find_smart_quote_close(&self, quote_char: u8, search_start: usize) -> Option<usize> {
        let bytes = self.input.as_bytes();
        let mut i = search_start;
        while i + 1 < bytes.len() {
            if bytes[i] == b'`' && bytes[i + 1] == quote_char {
                return Some(i);
            }
            i += 1;
        }
        None
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
                window: None,
                nofollow: false,
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
                width: None,
                height: None,
                align: None,
                float: None,
            }),
            Event::End(TagEnd::InlineImage),
        ]);
    }

    #[test]
    fn test_attribute_reference() {
        let events = parse("version {version}");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("version ")),
            Event::AttributeReference {
                name: Cow::Borrowed("version"),
                fallback: None,
            },
        ]);
    }

    #[test]
    fn test_attribute_reference_with_fallback() {
        let events = parse("{name!default value}");
        assert_eq!(events, vec![
            Event::AttributeReference {
                name: Cow::Borrowed("name"),
                fallback: Some(Cow::Borrowed("default value")),
            },
        ]);
    }

    #[test]
    fn test_attribute_reference_with_empty_fallback() {
        let events = parse("{name!}");
        assert_eq!(events, vec![
            Event::AttributeReference {
                name: Cow::Borrowed("name"),
                fallback: Some(Cow::Borrowed("")),
            },
        ]);
    }

    #[test]
    fn test_autolink() {
        let events = parse("visit https://example.com for info");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("visit ")),
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://example.com"),
                window: None,
                nofollow: false,
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

    // Smart quotes tests

    #[test]
    fn test_smart_double_quotes() {
        let events = parse("\"`text`\"");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\u{201C}")),
            Event::Text(Cow::Borrowed("text")),
            Event::Text(Cow::Borrowed("\u{201D}")),
        ]);
    }

    #[test]
    fn test_smart_single_quotes() {
        let events = parse("'`text`'");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\u{2018}")),
            Event::Text(Cow::Borrowed("text")),
            Event::Text(Cow::Borrowed("\u{2019}")),
        ]);
    }

    #[test]
    fn test_smart_quotes_in_sentence() {
        let events = parse("He said \"`hello`\" to her");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("He said ")),
            Event::Text(Cow::Borrowed("\u{201C}")),
            Event::Text(Cow::Borrowed("hello")),
            Event::Text(Cow::Borrowed("\u{201D}")),
            Event::Text(Cow::Borrowed(" to her")),
        ]);
    }

    #[test]
    fn test_smart_quotes_with_formatting() {
        let events = parse("\"`*bold* text`\"");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\u{201C}")),
            Event::Start(Tag::Strong),
            Event::Text(Cow::Borrowed("bold")),
            Event::End(TagEnd::Strong),
            Event::Text(Cow::Borrowed(" text")),
            Event::Text(Cow::Borrowed("\u{201D}")),
        ]);
    }

    #[test]
    fn test_smart_quotes_nested_types() {
        let events = parse("'`outer \"`inner`\" end`'");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\u{2018}")),
            Event::Text(Cow::Borrowed("outer ")),
            Event::Text(Cow::Borrowed("\u{201C}")),
            Event::Text(Cow::Borrowed("inner")),
            Event::Text(Cow::Borrowed("\u{201D}")),
            Event::Text(Cow::Borrowed(" end")),
            Event::Text(Cow::Borrowed("\u{2019}")),
        ]);
    }

    #[test]
    fn test_smart_quotes_unclosed() {
        let events = parse("\"`unclosed");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\"`unclosed")),
        ]);
    }

    #[test]
    fn test_smart_quotes_escaped() {
        let events = parse("\\\"`text`\"");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\"`text`\"")),
        ]);
    }

    // UI macro tests

    #[test]
    fn test_kbd_macro() {
        let events = parse("kbd:[Ctrl+C]");
        assert_eq!(events, vec![
            Event::Start(Tag::Keyboard),
            Event::Text(Cow::Borrowed("Ctrl+C")),
            Event::End(TagEnd::Keyboard),
        ]);
    }

    #[test]
    fn test_kbd_single_key() {
        let events = parse("kbd:[F11]");
        assert_eq!(events, vec![
            Event::Start(Tag::Keyboard),
            Event::Text(Cow::Borrowed("F11")),
            Event::End(TagEnd::Keyboard),
        ]);
    }

    #[test]
    fn test_kbd_in_sentence() {
        let events = parse("Press kbd:[Ctrl+C] to copy");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("Press ")),
            Event::Start(Tag::Keyboard),
            Event::Text(Cow::Borrowed("Ctrl+C")),
            Event::End(TagEnd::Keyboard),
            Event::Text(Cow::Borrowed(" to copy")),
        ]);
    }

    #[test]
    fn test_btn_macro() {
        let events = parse("btn:[OK]");
        assert_eq!(events, vec![
            Event::Start(Tag::Button),
            Event::Text(Cow::Borrowed("OK")),
            Event::End(TagEnd::Button),
        ]);
    }

    #[test]
    fn test_menu_macro() {
        let events = parse("menu:File[Save As]");
        assert_eq!(events, vec![
            Event::Start(Tag::Menu { target: Cow::Borrowed("File") }),
            Event::Text(Cow::Borrowed("Save As")),
            Event::End(TagEnd::Menu),
        ]);
    }

    #[test]
    fn test_menu_no_items() {
        let events = parse("menu:File[]");
        assert_eq!(events, vec![
            Event::Start(Tag::Menu { target: Cow::Borrowed("File") }),
            Event::End(TagEnd::Menu),
        ]);
    }

    #[test]
    fn test_menu_with_submenus() {
        let events = parse("menu:File[New > Document]");
        assert_eq!(events, vec![
            Event::Start(Tag::Menu { target: Cow::Borrowed("File") }),
            Event::Text(Cow::Borrowed("New > Document")),
            Event::End(TagEnd::Menu),
        ]);
    }

    // Icon macro tests

    #[test]
    fn test_icon_macro() {
        let events = parse("icon:heart[]");
        assert_eq!(events, vec![
            Event::Start(Tag::Icon { name: Cow::Borrowed("heart") }),
            Event::End(TagEnd::Icon),
        ]);
    }

    #[test]
    fn test_icon_with_attrs() {
        let events = parse("icon:heart[2x]");
        assert_eq!(events, vec![
            Event::Start(Tag::Icon { name: Cow::Borrowed("heart") }),
            Event::Text(Cow::Borrowed("2x")),
            Event::End(TagEnd::Icon),
        ]);
    }

    #[test]
    fn test_icon_in_sentence() {
        let events = parse("Press icon:save[] to save");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("Press ")),
            Event::Start(Tag::Icon { name: Cow::Borrowed("save") }),
            Event::End(TagEnd::Icon),
            Event::Text(Cow::Borrowed(" to save")),
        ]);
    }

    // Stem macro tests

    #[test]
    fn test_stem_macro() {
        let events = parse("stem:[x^2]");
        assert_eq!(events, vec![
            Event::Start(Tag::Stem { variant: Cow::Borrowed("stem") }),
            Event::Text(Cow::Borrowed("x^2")),
            Event::End(TagEnd::Stem),
        ]);
    }

    #[test]
    fn test_stem_empty() {
        let events = parse("stem:[]");
        assert_eq!(events, vec![
            Event::Start(Tag::Stem { variant: Cow::Borrowed("stem") }),
            Event::End(TagEnd::Stem),
        ]);
    }

    #[test]
    fn test_latexmath_macro() {
        let events = parse("latexmath:[C = \\alpha]");
        assert_eq!(events, vec![
            Event::Start(Tag::Stem { variant: Cow::Borrowed("latexmath") }),
            Event::Text(Cow::Borrowed("C = \\alpha")),
            Event::End(TagEnd::Stem),
        ]);
    }

    #[test]
    fn test_asciimath_macro() {
        let events = parse("asciimath:[sqrt(4)]");
        assert_eq!(events, vec![
            Event::Start(Tag::Stem { variant: Cow::Borrowed("asciimath") }),
            Event::Text(Cow::Borrowed("sqrt(4)")),
            Event::End(TagEnd::Stem),
        ]);
    }

    #[test]
    fn test_stem_in_sentence() {
        let events = parse("The formula stem:[x^2] is simple");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("The formula ")),
            Event::Start(Tag::Stem { variant: Cow::Borrowed("stem") }),
            Event::Text(Cow::Borrowed("x^2")),
            Event::End(TagEnd::Stem),
            Event::Text(Cow::Borrowed(" is simple")),
        ]);
    }

    #[test]
    fn test_smart_quotes_empty() {
        let events = parse("\"``\"");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\"``\"")),
        ]);
    }

    #[test]
    fn test_plain_quotes_no_backtick() {
        let events = parse("\"text\"");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\"text\"")),
        ]);
    }

    // Index term tests

    #[test]
    fn test_flow_index_term() {
        let events = parse("((tigers))");
        assert_eq!(events, vec![
            Event::IndexTerm { text: Cow::Borrowed("tigers") },
        ]);
    }

    #[test]
    fn test_flow_index_term_in_sentence() {
        let events = parse("I love ((tigers)) very much");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("I love ")),
            Event::IndexTerm { text: Cow::Borrowed("tigers") },
            Event::Text(Cow::Borrowed(" very much")),
        ]);
    }

    #[test]
    fn test_concealed_index_term_single() {
        let events = parse("(((animals)))");
        assert_eq!(events, vec![
            Event::ConcealedIndexTerm {
                primary: Cow::Borrowed("animals"),
                secondary: None,
                tertiary: None,
            },
        ]);
    }

    #[test]
    fn test_concealed_index_term_two_levels() {
        let events = parse("(((animals, cats)))");
        assert_eq!(events, vec![
            Event::ConcealedIndexTerm {
                primary: Cow::Borrowed("animals"),
                secondary: Some(Cow::Borrowed("cats")),
                tertiary: None,
            },
        ]);
    }

    #[test]
    fn test_concealed_index_term_three_levels() {
        let events = parse("(((animals, cats, tigers)))");
        assert_eq!(events, vec![
            Event::ConcealedIndexTerm {
                primary: Cow::Borrowed("animals"),
                secondary: Some(Cow::Borrowed("cats")),
                tertiary: Some(Cow::Borrowed("tigers")),
            },
        ]);
    }

    #[test]
    fn test_indexterm_macro() {
        let events = parse("indexterm:[animals, cats]");
        assert_eq!(events, vec![
            Event::ConcealedIndexTerm {
                primary: Cow::Borrowed("animals"),
                secondary: Some(Cow::Borrowed("cats")),
                tertiary: None,
            },
        ]);
    }

    #[test]
    fn test_indexterm2_macro() {
        let events = parse("indexterm2:[tigers]");
        assert_eq!(events, vec![
            Event::IndexTerm { text: Cow::Borrowed("tigers") },
        ]);
    }

    #[test]
    fn test_unclosed_double_parens() {
        let events = parse("((unclosed");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("((unclosed")),
        ]);
    }

    #[test]
    fn test_empty_flow_index_term() {
        let events = parse("(())");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("(())")),
        ]);
    }

    #[test]
    fn test_no_conflict_with_copyright() {
        // (C) should still produce copyright symbol, not index term
        let events = parse("(C) 2024");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("\u{00A9} 2024".to_string())),
        ]);
    }

    #[test]
    fn test_indexterm_macro_single_level() {
        let events = parse("indexterm:[animals]");
        assert_eq!(events, vec![
            Event::ConcealedIndexTerm {
                primary: Cow::Borrowed("animals"),
                secondary: None,
                tertiary: None,
            },
        ]);
    }

    #[test]
    fn test_indexterm_macro_three_levels() {
        let events = parse("indexterm:[animals, cats, tigers]");
        assert_eq!(events, vec![
            Event::ConcealedIndexTerm {
                primary: Cow::Borrowed("animals"),
                secondary: Some(Cow::Borrowed("cats")),
                tertiary: Some(Cow::Borrowed("tigers")),
            },
        ]);
    }

    #[test]
    fn test_indexterm2_in_sentence() {
        let events = parse("I love indexterm2:[tigers] very much");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("I love ")),
            Event::IndexTerm { text: Cow::Borrowed("tigers") },
            Event::Text(Cow::Borrowed(" very much")),
        ]);
    }

    // Bibliography anchor tests

    #[test]
    fn test_bibliography_anchor() {
        let events = parse("[[[pp]]]");
        assert_eq!(events, vec![
            Event::BibliographyAnchor { id: Cow::Borrowed("pp"), label: None },
        ]);
    }

    #[test]
    fn test_bibliography_anchor_with_label() {
        let events = parse("[[[gof, 2]]]");
        assert_eq!(events, vec![
            Event::BibliographyAnchor { id: Cow::Borrowed("gof"), label: Some(Cow::Borrowed("2")) },
        ]);
    }

    #[test]
    fn test_bibliography_anchor_empty_id() {
        let events = parse("[[[]]]");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("[[[]]]")),
        ]);
    }

    #[test]
    fn test_bibliography_anchor_unclosed() {
        // [[[ref]] — no closing ]]], first [ consumed as text, remaining [[ref]] parsed as anchor
        let events = parse("[[[ref]]");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("[")),
            Event::Start(Tag::Anchor { id: Cow::Borrowed("ref") }),
            Event::End(TagEnd::Anchor),
        ]);
    }

    #[test]
    fn test_anchor_still_works() {
        let events = parse("[[id]]");
        assert_eq!(events, vec![
            Event::Start(Tag::Anchor { id: Cow::Borrowed("id") }),
            Event::End(TagEnd::Anchor),
        ]);
    }

    #[test]
    fn test_anchor_with_reftext_still_works() {
        let events = parse("[[id,reftext]]");
        assert_eq!(events, vec![
            Event::Start(Tag::Anchor { id: Cow::Borrowed("id,reftext") }),
            Event::End(TagEnd::Anchor),
        ]);
    }

    #[test]
    fn test_bibliography_anchor_adjacent_to_text() {
        let events = parse("text[[[ref]]]more");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("text")),
            Event::BibliographyAnchor { id: Cow::Borrowed("ref"), label: None },
            Event::Text(Cow::Borrowed("more")),
        ]);
    }

    #[test]
    fn test_mixed_anchors_and_bibliography() {
        let events = parse("[[id]][[[ref]]]");
        assert_eq!(events, vec![
            Event::Start(Tag::Anchor { id: Cow::Borrowed("id") }),
            Event::End(TagEnd::Anchor),
            Event::BibliographyAnchor { id: Cow::Borrowed("ref"), label: None },
        ]);
    }

    // Apostrophe conversion tests

    #[test]
    fn test_apostrophe_basic_contraction() {
        let events = parse("it's");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("it\u{2019}s".to_string())),
        ]);
    }

    #[test]
    fn test_apostrophe_multiple_contractions() {
        let events = parse("don't won't can't");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("don\u{2019}t won\u{2019}t can\u{2019}t".to_string())),
        ]);
    }

    #[test]
    fn test_apostrophe_in_sentence() {
        let events = parse("I don't think it's right");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("I don\u{2019}t think it\u{2019}s right".to_string())),
        ]);
    }

    #[test]
    fn test_apostrophe_not_at_start() {
        // 'twas — apostrophe at start of word, not between alphanums
        let events = parse("'twas the night");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("'twas the night")),
        ]);
    }

    #[test]
    fn test_apostrophe_not_at_end() {
        // cats' — apostrophe at end, not between alphanums
        let events = parse("the cats' toys");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("the cats' toys")),
        ]);
    }

    #[test]
    fn test_apostrophe_rock_n_roll() {
        let events = parse("rock'n'roll");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("rock\u{2019}n\u{2019}roll".to_string())),
        ]);
    }

    #[test]
    fn test_apostrophe_with_dash_combo() {
        let events = parse("it's---done");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("it\u{2019}s\u{2014}done".to_string())),
        ]);
    }

    #[test]
    fn test_apostrophe_escaped() {
        let events = parse("it\\'s fine");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("it")),
            Event::Text(Cow::Borrowed("'s fine")),
        ]);
    }

    // Inline span tests: [.class]#text#

    #[test]
    fn test_inline_span_single_role() {
        let events = parse("[.lead]#text#");
        assert_eq!(events, vec![
            Event::Start(Tag::InlineSpan {
                id: None,
                roles: vec![Cow::Borrowed("lead")],
            }),
            Event::Text(Cow::Borrowed("text")),
            Event::End(TagEnd::InlineSpan),
        ]);
    }

    #[test]
    fn test_inline_span_multiple_roles() {
        let events = parse("[.big.red]#text#");
        assert_eq!(events, vec![
            Event::Start(Tag::InlineSpan {
                id: None,
                roles: vec![Cow::Borrowed("big"), Cow::Borrowed("red")],
            }),
            Event::Text(Cow::Borrowed("text")),
            Event::End(TagEnd::InlineSpan),
        ]);
    }

    #[test]
    fn test_inline_span_id_and_role() {
        let events = parse("[#myid.lead]#text#");
        assert_eq!(events, vec![
            Event::Start(Tag::InlineSpan {
                id: Some(Cow::Borrowed("myid")),
                roles: vec![Cow::Borrowed("lead")],
            }),
            Event::Text(Cow::Borrowed("text")),
            Event::End(TagEnd::InlineSpan),
        ]);
    }

    #[test]
    fn test_inline_span_unconstrained() {
        let events = parse("[.lead]##text##");
        assert_eq!(events, vec![
            Event::Start(Tag::InlineSpan {
                id: None,
                roles: vec![Cow::Borrowed("lead")],
            }),
            Event::Text(Cow::Borrowed("text")),
            Event::End(TagEnd::InlineSpan),
        ]);
    }

    #[test]
    fn test_inline_span_in_sentence() {
        let events = parse("hello [.lead]#world# end");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello ")),
            Event::Start(Tag::InlineSpan {
                id: None,
                roles: vec![Cow::Borrowed("lead")],
            }),
            Event::Text(Cow::Borrowed("world")),
            Event::End(TagEnd::InlineSpan),
            Event::Text(Cow::Borrowed(" end")),
        ]);
    }

    #[test]
    fn test_inline_span_unconstrained_mid_word() {
        let events = parse("hel[.x]##lo wo##rld");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hel")),
            Event::Start(Tag::InlineSpan {
                id: None,
                roles: vec![Cow::Borrowed("x")],
            }),
            Event::Text(Cow::Borrowed("lo wo")),
            Event::End(TagEnd::InlineSpan),
            Event::Text(Cow::Borrowed("rld")),
        ]);
    }

    #[test]
    fn test_inline_span_nested_formatting() {
        let events = parse("[.lead]#*bold* text#");
        assert_eq!(events, vec![
            Event::Start(Tag::InlineSpan {
                id: None,
                roles: vec![Cow::Borrowed("lead")],
            }),
            Event::Start(Tag::Strong),
            Event::Text(Cow::Borrowed("bold")),
            Event::End(TagEnd::Strong),
            Event::Text(Cow::Borrowed(" text")),
            Event::End(TagEnd::InlineSpan),
        ]);
    }

    #[test]
    fn test_bare_highlight_unchanged() {
        // Bare #highlight# should still work as highlight (<mark>)
        let events = parse("#highlight#");
        assert_eq!(events, vec![
            Event::Start(Tag::Highlight),
            Event::Text(Cow::Borrowed("highlight")),
            Event::End(TagEnd::Highlight),
        ]);
    }

    #[test]
    fn test_non_shorthand_bracket_not_span() {
        // [text]#foo# — content doesn't start with . or # → not a span
        let events = parse("[text]#foo#");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("[text]")),
            Event::Start(Tag::Highlight),
            Event::Text(Cow::Borrowed("foo")),
            Event::End(TagEnd::Highlight),
        ]);
    }

    #[test]
    fn test_inline_span_escaped() {
        // \[.lead]#text# — escaped bracket
        let events = parse("\\[.lead]#text#");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("[.lead]")),
            Event::Start(Tag::Highlight),
            Event::Text(Cow::Borrowed("text")),
            Event::End(TagEnd::Highlight),
        ]);
    }

    // Mailto macro tests

    #[test]
    fn test_mailto_basic() {
        let events = parse("mailto:user@example.com[]");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("mailto:user@example.com"),
                window: None,
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("user@example.com")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_mailto_with_display_text() {
        let events = parse("mailto:user@example.com[Email Me]");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("mailto:user@example.com"),
                window: None,
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("Email Me")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_mailto_in_sentence() {
        let events = parse("Contact mailto:user@example.com[us] for help");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("Contact ")),
            Event::Start(Tag::Link {
                url: Cow::Borrowed("mailto:user@example.com"),
                window: None,
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("us")),
            Event::End(TagEnd::Link),
            Event::Text(Cow::Borrowed(" for help")),
        ]);
    }

    #[test]
    fn test_mailto_no_brackets() {
        // Without [] — not recognized as macro, but email part is auto-linked
        let events = parse("mailto:user@example.com");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("mailto:")),
            Event::Start(Tag::Link {
                url: Cow::Owned("mailto:user@example.com".to_string()),
                window: None,
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("user@example.com")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_link_macro_with_window() {
        let events = parse("link:https://example.com[Example,window=_blank]");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://example.com"),
                window: Some(Cow::Borrowed("_blank")),
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("Example")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_link_macro_with_nofollow() {
        let events = parse("link:https://example.com[Example,opts=nofollow]");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://example.com"),
                window: None,
                nofollow: true,
            }),
            Event::Text(Cow::Borrowed("Example")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_link_macro_with_window_and_nofollow() {
        let events = parse("link:https://example.com[Example,window=_blank,opts=nofollow]");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://example.com"),
                window: Some(Cow::Borrowed("_blank")),
                nofollow: true,
            }),
            Event::Text(Cow::Borrowed("Example")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_mailto_with_window() {
        let events = parse("mailto:user@example.com[Email,window=_blank]");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("mailto:user@example.com"),
                window: Some(Cow::Borrowed("_blank")),
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("Email")),
            Event::End(TagEnd::Link),
        ]);
    }

    // Xref macro tests

    #[test]
    fn test_xref_basic() {
        let events = parse("xref:chapter1[]");
        assert_eq!(events, vec![
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("chapter1"),
                label: None,
            }),
            Event::Text(Cow::Borrowed("chapter1")),
            Event::End(TagEnd::CrossReference),
        ]);
    }

    #[test]
    fn test_xref_with_label() {
        let events = parse("xref:file.adoc#anchor[Link Text]");
        assert_eq!(events, vec![
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("file.adoc#anchor"),
                label: Some(Cow::Borrowed("Link Text")),
            }),
            Event::Text(Cow::Borrowed("Link Text")),
            Event::End(TagEnd::CrossReference),
        ]);
    }

    #[test]
    fn test_xref_in_sentence() {
        let events = parse("See xref:intro[Introduction] for details");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("See ")),
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("intro"),
                label: Some(Cow::Borrowed("Introduction")),
            }),
            Event::Text(Cow::Borrowed("Introduction")),
            Event::End(TagEnd::CrossReference),
            Event::Text(Cow::Borrowed(" for details")),
        ]);
    }

    #[test]
    fn test_xref_no_brackets() {
        // Without [] — not recognized as macro
        let events = parse("xref:target");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("xref:target")),
        ]);
    }

    // Arrow replacement tests

    #[test]
    fn test_arrow_right() {
        let events = parse("A -> B");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("A \u{2192} B".to_string())),
        ]);
    }

    #[test]
    fn test_arrow_left() {
        let events = parse("A <- B");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("A \u{2190} B".to_string())),
        ]);
    }

    #[test]
    fn test_arrow_double_right() {
        let events = parse("A => B");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("A \u{21D2} B".to_string())),
        ]);
    }

    #[test]
    fn test_arrow_double_left() {
        let events = parse("A <= B");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("A \u{21D0} B".to_string())),
        ]);
    }

    #[test]
    fn test_arrow_triple_not_replaced() {
        // --> should NOT be replaced (triple sequence)
        let events = parse("A --> B");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("A \u{2013}> B".to_string())), // -- becomes en-dash, > stays
        ]);
    }

    #[test]
    fn test_arrow_escaped_right() {
        let events = parse("A \\-> B");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("A ")),
            Event::Text(Cow::Borrowed("->")),
            Event::Text(Cow::Borrowed(" B")),
        ]);
    }

    #[test]
    fn test_arrow_escaped_double_right() {
        let events = parse("A \\=> B");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("A ")),
            Event::Text(Cow::Borrowed("=>")),
            Event::Text(Cow::Borrowed(" B")),
        ]);
    }

    #[test]
    fn test_arrow_em_dash_still_works() {
        let events = parse("hello---world");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("hello\u{2014}world".to_string())),
        ]);
    }

    #[test]
    fn test_arrow_en_dash_still_works() {
        let events = parse("hello--world");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("hello\u{2013}world".to_string())),
        ]);
    }

    #[test]
    fn test_email_autolink() {
        let events = parse("user@example.com");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Owned("mailto:user@example.com".to_string()),
                window: None,
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("user@example.com")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_email_autolink_in_sentence() {
        let events = parse("Contact user@example.com for info");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("Contact ")),
            Event::Start(Tag::Link {
                url: Cow::Owned("mailto:user@example.com".to_string()),
                window: None,
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("user@example.com")),
            Event::End(TagEnd::Link),
            Event::Text(Cow::Borrowed(" for info")),
        ]);
    }

    #[test]
    fn test_email_autolink_with_subdomain() {
        let events = parse("user@mail.example.com");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Owned("mailto:user@mail.example.com".to_string()),
                window: None,
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("user@mail.example.com")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_email_autolink_with_plus() {
        let events = parse("user+tag@example.com");
        assert_eq!(events, vec![
            Event::Start(Tag::Link {
                url: Cow::Owned("mailto:user+tag@example.com".to_string()),
                window: None,
                nofollow: false,
            }),
            Event::Text(Cow::Borrowed("user+tag@example.com")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_email_autolink_no_domain_dot() {
        let events = parse("user@localhost");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("user@localhost")),
        ]);
    }
}
