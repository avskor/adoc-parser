use std::borrow::Cow;

use crate::attributes::parse_link_attrs;
use crate::event::{Event, SubstitutionSet, Tag, TagEnd};

pub(crate) fn apply_typographic_replacements<'a>(
    text: &'a str,
    left_is_boundary: bool,
    right_is_boundary: bool,
) -> Cow<'a, str> {
    // Quick check: if none of the trigger characters are present, return borrowed
    if !text.contains('-') && !text.contains('.') && !text.contains('(') && !text.contains('\'')
        && !text.contains('`')
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
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                // Spaced em-dash (Asciidoctor `(^|\n| |\\)--( |\n|$)`): `--` flanked by
                // space/newline/line-edge on BOTH sides → thin-space + em-dash +
                // thin-space; the flanking chars are consumed (adjacent lines merge).
                // A flanking char already consumed by a previous replacement does not
                // count (regex gsub semantics: `a -- -- b` replaces only the first).
                // At the run edges, the caller reports whether they are real line edges
                // (`^`/`$`) — span-internal content is bounded by `<tag>` chars, not `^`/`$`.
                let before_boundary = if i == 0 {
                    left_is_boundary
                } else {
                    i > copied_up_to && matches!(bytes[i - 1], b' ' | b'\n')
                };
                let after_boundary = if i + 2 >= len {
                    right_is_boundary
                } else {
                    matches!(bytes[i + 2], b' ' | b'\n')
                };
                if before_boundary && after_boundary {
                    let buf = result.get_or_insert_with(|| String::from(&text[..copied_up_to]));
                    let copy_end = if i == 0 { 0 } else { i - 1 };
                    buf.push_str(&text[copied_up_to..copy_end]);
                    buf.push_str("\u{2009}\u{2014}\u{2009}");
                    i += if i + 2 < len { 3 } else { 2 };
                    copied_up_to = i;
                    continue;
                }
                // word--word → em-dash + zero-width space (Asciidoctor `(\w)--(?=\w)`).
                // Anywhere else (` --flag`, trailing `S.S.T.--`, `a---b`, `----` runs)
                // Asciidoctor does not form an em-dash; leave this `-` literal and advance
                // one byte so the next `-` is reconsidered on its own (e.g. `-->` → `-→`).
                let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
                if i > 0 && is_word(bytes[i - 1]) && i + 2 < len && is_word(bytes[i + 2]) {
                    Some(("\u{2014}\u{200B}", 2)) // em-dash + ZWSP
                } else {
                    None
                }
            }
            // -> right arrow (but not -->)
            b'-' if i + 1 < len && bytes[i + 1] == b'>'
                && !(i + 2 < len && bytes[i + 2] == b'>') =>
            {
                Some(("\u{2192}", 2)) // →
            }
            b'.' if i + 2 < len && bytes[i + 1] == b'.' && bytes[i + 2] == b'.' => {
                Some(("\u{2026}\u{200B}", 3)) // ellipsis + zero-width space
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
            // `' → right single curly quote (closing smart quote)
            b'`' if i + 1 < len && bytes[i + 1] == b'\'' => {
                Some(("\u{2019}", 2))
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

/// Percent-encode `s` into `buf` the way Asciidoctor encodes mailto query
/// values (Ruby `ERB::Util.url_encode`): every byte outside `A-Za-z0-9_.~-`
/// becomes `%XX` with uppercase hex; a space is `%20`, not `+`.
fn url_encode_into(buf: &mut String, s: &str) {
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'~' | b'-' => {
                buf.push(b as char);
            }
            _ => {
                buf.push('%');
                buf.push(char::from_digit((b >> 4) as u32, 16).unwrap().to_ascii_uppercase());
                buf.push(char::from_digit((b & 0xF) as u32, 16).unwrap().to_ascii_uppercase());
            }
        }
    }
}

/// Inline-parsing options derived from document attributes.
///
/// This is the single channel through which document attributes influence the
/// inline parser. Consumers fill it in one of two ways:
/// - streaming (the pull [`crate::Parser`]): call [`InlineOptions::apply_attribute`]
///   on each `Event::Attribute`, so body text reflects the attribute state up
///   to that point (mid-document set/unset works like Asciidoctor);
/// - snapshot (renderers re-parsing attribute values): build from the current
///   document-attribute table via [`InlineOptions::from_attr_lookup`].
///
/// New attribute-gated inline behavior should add a field here plus an arm in
/// both constructors, rather than threading another ad-hoc flag.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InlineOptions {
    /// Whether the `:experimental:` document attribute is set, enabling the
    /// `kbd:`/`btn:`/`menu:` UI macros. When false they are left as literal
    /// text, matching Asciidoctor's default.
    pub experimental: bool,
}

impl InlineOptions {
    /// Update from a document-attribute entry, given the name exactly as it
    /// appears in `Event::Attribute` (unset spellings `!name`/`name!` included).
    /// Attributes that do not affect inline parsing are ignored.
    pub fn apply_attribute(&mut self, name: &str) {
        let (base, set) = if let Some(base) = name.strip_prefix('!') {
            (base, false)
        } else if let Some(base) = name.strip_suffix('!') {
            (base, false)
        } else {
            (name, true)
        };
        if base == "experimental" {
            self.experimental = set;
        }
    }

    /// Build from a snapshot of the document-attribute table; `is_set` reports
    /// whether the named attribute is currently set.
    pub fn from_attr_lookup(mut is_set: impl FnMut(&str) -> bool) -> Self {
        Self { experimental: is_set("experimental") }
    }
}

pub struct InlineParser;

impl InlineParser {
    #[cfg(test)]
    pub fn parse_str<'a>(text: &'a str) -> Vec<Event<'a>> {
        Self::parse_str_with_subs(text, SubstitutionSet::NORMAL)
    }

    pub fn parse_str_with_subs<'a>(text: &'a str, subs: SubstitutionSet) -> Vec<Event<'a>> {
        Self::parse_str_with_subs_options(text, subs, InlineOptions::default())
    }

    /// Like [`Self::parse_str_with_subs`], but with explicit [`InlineOptions`]
    /// (document-attribute-derived state such as `:experimental:`).
    pub fn parse_str_with_subs_options<'a>(
        text: &'a str,
        subs: SubstitutionSet,
        options: InlineOptions,
    ) -> Vec<Event<'a>> {
        if text.is_empty() {
            return vec![Event::Text(Cow::Borrowed(""))];
        }

        // Transitional: when the sequential-pass engine is enabled and can
        // handle this top-level text, use it; otherwise fall through to the
        // legacy recursive parser. Phase 0: the engine always declines, so this
        // is inert (see `crate::subst`).
        if crate::subst::enabled()
            && let Some(events) = crate::subst::try_parse(text, subs, options)
        {
            return events;
        }

        parse_legacy(text, subs, options)
    }
}

/// The legacy recursive inline parser, factored out of
/// [`InlineParser::parse_str_with_subs_options`] so the sequential-pass engine
/// ([`crate::subst`]) can run it for differential comparison without
/// re-entering the toggle check (which would recurse). Handles any `text`
/// including the empty string (yields a single empty `Text`).
pub(crate) fn parse_legacy<'a>(
    text: &'a str,
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'a>> {
    let mut events = Vec::new();
    let mut parser = InlineState::new(text, subs, options);
    // Top-level text: its start/end are the real paragraph/line edges.
    parser.edges_are_line_boundaries = true;
    parser.parse_inline(&mut events);

    if events.is_empty() {
        vec![Event::Text(Cow::Borrowed(text))]
    } else {
        events
    }
}

/// Map a pass-macro subs spec to a substitution set. Single-letter aliases
/// follow Asciidoctor's SUB_HINTS (`a`/`c`/`m`/`n`/`p`/`q`/`r`/`v`); full
/// names share `subs=` parsing. Unknown names are ignored (Asciidoctor
/// warns and skips them, still consuming the macro). Shared with the
/// sequential-pass passthrough extractor ([`crate::subst`]).
pub(crate) fn pass_spec_to_subs(spec: &str) -> SubstitutionSet {
    let mut set = SubstitutionSet::NONE;
    for token in spec.split(',') {
        let flags = match token {
            "a" => Some(SubstitutionSet::ATTRIBUTES),
            "c" => Some(SubstitutionSet::SPECIALCHARS),
            "m" => Some(SubstitutionSet::MACROS),
            "n" => crate::attributes::sub_name_to_flags("normal"),
            "p" => Some(SubstitutionSet::POST_REPLACEMENTS),
            "q" => Some(SubstitutionSet::QUOTES),
            "r" => Some(SubstitutionSet::REPLACEMENTS),
            "v" => crate::attributes::sub_name_to_flags("verbatim"),
            _ => crate::attributes::sub_name_to_flags(token),
        };
        if let Some(f) = flags {
            set.add(f);
        }
    }
    set
}

struct InlineState<'a> {
    input: &'a str,
    pos: usize,
    subs: SubstitutionSet,
    /// Document-attribute-derived options (e.g. `:experimental:` gating the
    /// `kbd:`/`btn:`/`menu:` UI macros).
    options: InlineOptions,
    /// Whether the start and end of `input` are real line/paragraph edges (`^`/`$`),
    /// as opposed to the content of an inline span reparsed in isolation. Asciidoctor
    /// runs replacements over the whole post-quotes string, so a span's inner content
    /// is bounded by its `<tag>`/`</tag>` (chars `>`/`<`), not by `^`/`$`. The boundary
    /// matters only for the spaced em-dash rule (`(^|\n| |\\)--( |\n|$)`): an edge `--`
    /// must stay literal inside a span (`` `--` `` → `<code>--</code>`) but becomes an
    /// em-dash at a true line edge. Top-level text sets this true; inner reparses leave
    /// it false (the default).
    edges_are_line_boundaries: bool,
    /// True when `input` is the reparsed inner content of a smart-quote span
    /// (`"`…`"` / `'`…`'`). In Asciidoctor's QUOTE_SUBS order the `:double`/`:single`
    /// substitutions run *after* constrained strong (`*`) but *before* constrained
    /// monospace (`` ` ``), emphasis (`_`) and mark (`#`). So at the span's leading
    /// edge those three see the `;` that ends the emitted `&#8220;`/`&#8216;` and their
    /// open assertion `(^|[^\w;:…])` fails — they stay literal — whereas strong has
    /// already matched against the original backtick (which its open class allows).
    /// This flag reproduces that asymmetry: a constrained `_`/`` ` ``/`#` open at
    /// position 0 of the inner span is suppressed; `*`, all unconstrained markers and
    /// super/subscript (no open assertion) are unaffected.
    smart_quote_leading_edge: bool,
    /// True when `input` is the reparsed inner content of an emphasis span
    /// (`_…_` / `__…__`). In Asciidoctor's QUOTE_SUBS order constrained strong (`*`)
    /// and monospace (`` ` ``) both run *before* emphasis, so at the span's leading
    /// edge they still see the literal `_` marker — a word character that their open
    /// assertion `(^|[^\w…])` rejects — and stay literal (`_`code`_` → `<em>`code`</em>`,
    /// `_*b*_` → `<em>*b*</em>`). Mark (`#`) runs *after* emphasis (it sees the emitted
    /// `<em>`'s `>`) and opens normally; unconstrained markers and super/subscript have
    /// no open assertion and are likewise unaffected. This flag reproduces that: a
    /// constrained `*`/`` ` `` open at position 0 of the inner span is suppressed.
    emphasis_leading_edge: bool,
}

impl<'a> InlineState<'a> {
    fn new(input: &'a str, subs: SubstitutionSet, options: InlineOptions) -> Self {
        Self {
            input,
            pos: 0,
            subs,
            options,
            edges_are_line_boundaries: false,
            smart_quote_leading_edge: false,
            emphasis_leading_edge: false,
        }
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
        // A valid char-ref survives only when specialchars AND replacements are both active:
        // specialchars escapes `&#167;` to `&amp;#167;`, then the replacements sub restores it.
        // Verbatim blocks have specialchars but not replacements, so they escape char-refs.
        let preserve_char_refs =
            self.subs.has(SubstitutionSet::SPECIALCHARS) && self.subs.has(SubstitutionSet::REPLACEMENTS);

        let mut text_start = self.pos;

        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];

            if self.handle_inline_escape(b, has_quotes, has_post_replacements, events, &mut text_start) {
                continue;
            }

            if self.handle_inline_passthrough(b, events, &mut text_start) {
                continue;
            }

            if self.handle_inline_formatting(b, has_quotes, events, &mut text_start) {
                continue;
            }

            if self.handle_inline_macro(b, has_quotes, has_macros, has_attributes, events, &mut text_start) {
                continue;
            }

            // Bare character reference: &#167; &copy; &amp; → preserve as a raw entity.
            // Asciidoctor keeps a valid char-ref intact in normal text (specialchars escapes it,
            // then replacements restores it); an invalid one (`&#1;`, bare `&`) stays escaped.
            // Verbatim blocks lack replacements, so they keep their char-refs escaped (matching
            // Asciidoctor). The reference is emitted as a passthrough so the renderer does not
            // escape its `&`.
            if b == b'&' && preserve_char_refs {
                let ref_len = self.char_ref_len_at(self.pos);
                if ref_len > 0 {
                    self.flush_text(text_start, self.pos, events);
                    let ref_start = self.pos;
                    self.advance_by(ref_len);
                    events.push(Event::InlinePassthrough(Cow::Borrowed(&self.input[ref_start..self.pos])));
                    text_start = self.pos;
                    continue;
                }
            }

            self.pos += 1;
        }

        self.flush_text(text_start, self.pos, events);
    }

    fn handle_inline_macro(&mut self, b: u8, has_quotes: bool, has_macros: bool, has_attributes: bool, events: &mut Vec<Event<'a>>, text_start: &mut usize) -> bool {
        match b {
            // Cross-reference <<id>> or <<id,label>> (MACROS)
            b'<' if has_macros && self.peek_at(1) == Some(b'<') => {
                if self.try_cross_reference(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Inline stem macro: stem:[content] (MACROS)
            b's' if has_macros && self.remaining().starts_with("stem:[") => {
                if self.try_stem_macro(5, "stem", events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Pass macro: pass:[text] (always active)
            b'p' if self.remaining().starts_with("pass:") => {
                if self.try_pass_macro(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Inline latexmath macro: latexmath:[content] (MACROS)
            b'l' if has_macros && self.remaining().starts_with("latexmath:[") => {
                if self.try_stem_macro(10, "latexmath", events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Link macro: link:url[text] (MACROS)
            b'l' if has_macros && self.remaining().starts_with("link:") => {
                if self.try_link_macro(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Index term macros (MACROS)
            b'i' if has_macros && self.remaining().starts_with("indexterm2:") => {
                if self.try_indexterm2_macro(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            b'i' if has_macros && self.remaining().starts_with("indexterm:") => {
                if self.try_indexterm_macro(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Icon macro: icon:name[attrs] (MACROS)
            b'i' if has_macros && self.remaining().starts_with("icon:") => {
                if self.try_icon_macro(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Inline image: image:path[alt] (MACROS)
            b'i' if has_macros && self.remaining().starts_with("image:") && !self.remaining().starts_with("image::") => {
                if self.try_inline_image(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Keyboard macro: kbd:[keys] — experimental only; otherwise literal text.
            b'k' if has_macros && self.remaining().starts_with("kbd:") => {
                if self.options.experimental {
                    if self.try_kbd_macro(events, text_start) {
                        return true;
                    }
                    self.pos += 1;
                } else {
                    self.skip_disabled_ui_macro(4);
                }
                true
            }

            // Button macro: btn:[label] — experimental only; otherwise literal text.
            b'b' if has_macros && self.remaining().starts_with("btn:") => {
                if self.options.experimental {
                    if self.try_btn_macro(events, text_start) {
                        return true;
                    }
                    self.pos += 1;
                } else {
                    self.skip_disabled_ui_macro(4);
                }
                true
            }

            // Mailto macro (MACROS)
            b'm' if has_macros && self.remaining().starts_with("mailto:") => {
                if self.try_mailto_macro(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Menu macro — experimental only; otherwise literal text.
            b'm' if has_macros && self.remaining().starts_with("menu:") => {
                if self.options.experimental {
                    if self.try_menu_macro(events, text_start) {
                        return true;
                    }
                    self.pos += 1;
                } else {
                    self.skip_disabled_ui_macro(5);
                }
                true
            }

            // Footnote macro (MACROS)
            b'f' if has_macros && self.remaining().starts_with("footnote:") => {
                if self.try_footnote_macro(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Attribute reference {name} (ATTRIBUTES)
            b'{' if has_attributes => {
                if self.try_attribute_reference(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Anchor macro anchor:id[] (MACROS)
            b'a' if has_macros && self.remaining().starts_with("anchor:") => {
                if self.try_anchor_macro(events, text_start) {
                    return true;
                }
                // An invalid anchor target is literal text in Asciidoctor —
                // skip the whole prefix so its interior isn't rescanned.
                self.pos += 7;
                true
            }

            // Inline asciimath macro (MACROS)
            b'a' if has_macros && self.remaining().starts_with("asciimath:[") => {
                if self.try_stem_macro(10, "asciimath", events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Xref macro (MACROS)
            b'x' if has_macros && self.remaining().starts_with("xref:") => {
                if self.try_xref_macro(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Autolink: http:// or https:// (MACROS)
            b'h' if has_macros && (self.remaining().starts_with("http://") || self.remaining().starts_with("https://")) => {
                if self.try_autolink(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Autolink: ftp:// (MACROS)
            b'f' if has_macros && self.remaining().starts_with("ftp://") => {
                if self.try_autolink(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Autolink: irc:// (MACROS)
            b'i' if has_macros && self.remaining().starts_with("irc://") => {
                if self.try_autolink(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Email autolink: user@example.com (MACROS)
            b'@' if has_macros => {
                if self.try_email_autolink(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Inline attr span: [.class]#text#, [#id.class]#text#, or a bare-word
            // role list [role]#text# (QUOTES). A bare word is taken verbatim as the
            // role by Asciidoctor (`[big]##O##` → <span class="big">O</span>); the
            // `[[` form is reserved for the bibliography/anchor macros.
            b'[' if has_quotes
                && self.peek_at(1) != Some(b'[')
                && self
                    .peek_at(1)
                    .is_some_and(|c| c == b'.' || c == b'#' || c.is_ascii_alphanumeric() || c == b'_') =>
            {
                if self.try_inline_attr_span(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Bibliography anchor [[[id]]] or [[[id, label]]] (MACROS)
            b'[' if has_macros && self.peek_at(1) == Some(b'[') && self.peek_at(2) == Some(b'[') => {
                if self.try_bibliography_anchor(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Anchor [[id]] (MACROS)
            b'[' if has_macros && self.peek_at(1) == Some(b'[') => {
                if self.try_anchor(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Index term: concealed (((primary, secondary))) or flow ((term)) (MACROS).
            // One pattern, mirroring Asciidoctor's `\(\((.+?)\)\)(?!\))`: the
            // matched content decides the form by its own enclosing parens.
            b'(' if has_macros && self.peek_at(1) == Some(b'(') => {
                if self.try_index_term(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // No catch-all for unknown `name:target[attrs]` forms: Asciidoctor
            // matches only registered macro names, so unknown macros stay
            // literal text and their bracket interior flows through the normal
            // substitutions (`foo:bar[*b*]` → `foo:bar[<strong>b</strong>]`).
            _ => false,
        }
    }

    fn handle_inline_formatting(&mut self, b: u8, has_quotes: bool, events: &mut Vec<Event<'a>>, text_start: &mut usize) -> bool {
        match b {
            // Unconstrained formatting: double markers (QUOTES)
            b'*' if has_quotes && self.peek_at(1) == Some(b'*') => {
                if self.try_unconstrained(b'*', Tag::Strong { id: None, roles: Vec::new() }, TagEnd::Strong, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            b'_' if has_quotes && self.peek_at(1) == Some(b'_') => {
                if self.try_unconstrained(b'_', Tag::Emphasis { id: None, roles: Vec::new() }, TagEnd::Emphasis, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Smart double quotes: "`text`" (QUOTES)
            b'"' if has_quotes && self.peek_at(1) == Some(b'`') => {
                if self.try_smart_quotes(b'"', events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Smart single quotes: '`text`' (QUOTES)
            b'\'' if has_quotes && self.peek_at(1) == Some(b'`') => {
                if self.try_smart_quotes(b'\'', events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            b'`' if has_quotes && self.peek_at(1) == Some(b'`') => {
                if self.try_unconstrained(b'`', Tag::Monospace { id: None, roles: Vec::new() }, TagEnd::Monospace, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            b'#' if has_quotes && self.peek_at(1) == Some(b'#') => {
                if self.try_unconstrained(b'#', Tag::Highlight, TagEnd::Highlight, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Constrained formatting: single markers (QUOTES)
            b'*' if has_quotes => {
                if self.try_constrained(b'*', Tag::Strong { id: None, roles: Vec::new() }, TagEnd::Strong, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            b'_' if has_quotes => {
                if self.try_constrained(b'_', Tag::Emphasis { id: None, roles: Vec::new() }, TagEnd::Emphasis, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            b'`' if has_quotes => {
                if self.try_constrained(b'`', Tag::Monospace { id: None, roles: Vec::new() }, TagEnd::Monospace, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            b'#' if has_quotes => {
                if self.try_constrained(b'#', Tag::Highlight, TagEnd::Highlight, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Superscript ^text^ (QUOTES)
            b'^' if has_quotes => {
                if self.try_simple_pair(b'^', Tag::Superscript, TagEnd::Superscript, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            // Subscript ~text~ (QUOTES)
            b'~' if has_quotes => {
                if self.try_simple_pair(b'~', Tag::Subscript, TagEnd::Subscript, events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }

            _ => false,
        }
    }

    fn handle_inline_passthrough(&mut self, b: u8, events: &mut Vec<Event<'a>>, text_start: &mut usize) -> bool {
        match b {
            // Triple-plus passthrough: +++text+++
            b'+' if self.peek_at(1) == Some(b'+') && self.peek_at(2) == Some(b'+') => {
                if self.try_triple_plus_passthrough(events, text_start) {
                    return true;
                }
                // `++++` is an empty double-plus passthrough (`++` + `++`),
                // so a failed `+++` open retries as `++` from the same spot
                // (Asciidoctor's `(\+\+\+?)(.*?)\1` backtracks the same way).
                if self.try_double_plus_passthrough(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            // Double-plus passthrough: ++text++
            b'+' if self.peek_at(1) == Some(b'+') => {
                if self.try_double_plus_passthrough(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            // Single-plus passthrough: +text+
            b'+' => {
                if self.try_single_plus_passthrough(events, text_start) {
                    return true;
                }
                self.pos += 1;
                true
            }
            _ => false,
        }
    }

    fn handle_inline_escape(&mut self, b: u8, has_quotes: bool, has_post_replacements: bool, events: &mut Vec<Event<'a>>, text_start: &mut usize) -> bool {
        match b {
            // Escape typographic patterns: \--- \-- \... \(C) \(R) \(TM)
            b'\\' if self.typographic_escape_len() > 0 => {
                self.flush_text(*text_start, self.pos, events);
                let skip = self.typographic_escape_len();
                self.advance_by(1); // skip backslash
                let pattern_start = self.pos;
                self.advance_by(skip);
                // Emit pattern as plain text, bypassing typographic replacements
                events.push(Event::Text(Cow::Borrowed(&self.input[pattern_start..self.pos])));
                *text_start = self.pos;
                true
            }

            // Escape pass macro: \pass:[...] or \pass:SPEC[...] → literal "pass:SPEC[" +
            // normal inline parsing of the rest (Asciidoctor drops the backslash and skips
            // extraction; the bracketed content and the trailing `]` flow through the
            // remaining substitutions: `\pass:c[*b*]` → `pass:c[<strong>b</strong>]`).
            b'\\' if self.pass_escape_prefix_len(self.pos + 1) > 0 => {
                let skip = self.pass_escape_prefix_len(self.pos + 1);
                self.flush_text(*text_start, self.pos, events);
                self.advance_by(1); // skip backslash
                *text_start = self.pos;
                self.advance_by(skip); // skip "pass:SPEC[" — included in next text flush as literal
                true
            }

            // `\\pass:SPEC[…]`: only one backslash takes part in the escape
            // (Asciidoctor's pass regex captures a single `\`); the first one
            // stays literal text: `\\pass:c[abc]` → `\pass:c[abc]`.
            b'\\' if self.peek_at(1) == Some(b'\\') && self.pass_escape_prefix_len(self.pos + 2) > 0 => {
                let skip = self.pass_escape_prefix_len(self.pos + 2);
                self.flush_text(*text_start, self.pos + 1, events); // first backslash stays
                self.advance_by(2); // past both backslashes
                *text_start = self.pos;
                self.advance_by(skip); // skip "pass:SPEC[" — included in next text flush as literal
                true
            }

            // Escape a recognized inline macro: \footnote:[x], \indexterm2:[term], \image:p[alt],
            // \link:u[t], etc. → drop the backslash and emit the whole macro form as literal text.
            // Asciidoctor strips the leading backslash and renders the macro form verbatim without
            // running macro processing; the renderer still escapes special characters in the text.
            b'\\' if self.inline_macro_escape_len(self.pos + 1) > 0 => {
                self.flush_text(*text_start, self.pos, events);
                let macro_len = self.inline_macro_escape_len(self.pos + 1);
                self.advance_by(1); // skip backslash
                let macro_start = self.pos;
                self.advance_by(macro_len);
                events.push(Event::Text(Cow::Borrowed(&self.input[macro_start..self.pos])));
                *text_start = self.pos;
                true
            }

            // Escape an index term: \((…)) → the backslash drops and the match
            // stays literal text; \(((…))) (escaped concealed) → literal parens
            // around a VISIBLE flow term of the inner text (Asciidoctor
            // substitutors.rb: "escape concealed index term, but process
            // nested flow index term"). Without a would-be match (no closing
            // `))` ahead) the backslash stays literal.
            b'\\' if self.subs.has(SubstitutionSet::MACROS)
                && self.peek_at(1) == Some(b'(')
                && self.peek_at(2) == Some(b'(')
                && Self::index_term_close(&self.input[self.pos + 3..]).is_some() =>
            {
                self.flush_text(*text_start, self.pos, events);
                let content_start = self.pos + 3;
                let close = Self::index_term_close(&self.input[content_start..])
                    .expect("checked in guard");
                let content = &self.input[content_start..content_start + close];
                if content.starts_with('(') && content.ends_with(')') {
                    events.push(Event::Text(Cow::Borrowed("(")));
                    events.push(Event::IndexTerm {
                        text: Cow::Borrowed(&content[1..content.len() - 1]),
                    });
                    events.push(Event::Text(Cow::Borrowed(")")));
                } else {
                    // whole match minus the backslash, literal
                    events.push(Event::Text(Cow::Borrowed(
                        &self.input[self.pos + 1..content_start + close + 2],
                    )));
                }
                self.pos = content_start + close + 2;
                *text_start = self.pos;
                true
            }

            // `\\` before an unconstrained pair (`**`/`__`/`##`/<double
            // backtick>): both backslashes are consumed and the marks stay
            // literal, while the content between them still receives normal
            // substitutions. Mirrors Asciidoctor's cascading gsub passes: the
            // unconstrained pass matches `\MM…MM` and strips one backslash,
            // then the constrained pass matches with the remaining `\` lead
            // and strips the second (`\\__func__` → `__func__`,
            // `\\__a*b*c__` → `__a<strong>b</strong>c__`).
            b'\\' if has_quotes
                && self.peek_at(1) == Some(b'\\')
                && self
                    .peek_at(2)
                    .is_some_and(|c| matches!(c, b'*' | b'_' | b'#' | b'`'))
                && self.peek_at(3) == self.peek_at(2)
                && self
                    .find_closing_unconstrained(self.input.as_bytes()[self.pos + 2], self.pos + 4)
                    .is_some_and(|c| c > self.pos + 4) =>
            {
                self.flush_text(*text_start, self.pos, events);
                let marker = self.input.as_bytes()[self.pos + 2];
                let open_marks = self.pos + 2;
                let content_start = self.pos + 4;
                let close_pos = self
                    .find_closing_unconstrained(marker, content_start)
                    .expect("checked in guard");
                events.push(Event::Text(Cow::Borrowed(
                    &self.input[open_marks..content_start],
                )));
                let inner = &self.input[content_start..close_pos];
                let mut inner_parser = InlineState::new(inner, self.subs, self.options);
                inner_parser.parse_inline(events);
                events.push(Event::Text(Cow::Borrowed(
                    &self.input[close_pos..close_pos + 2],
                )));
                self.pos = close_pos + 2;
                *text_start = self.pos;
                true
            }

            // Escape plus sequences: \+, \++, \+++
            b'\\' if self.peek_at(1) == Some(b'+') => {
                self.flush_text(*text_start, self.pos, events);
                self.advance_by(1); // skip backslash
                *text_start = self.pos;
                while self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'+' {
                    self.advance_by(1);
                }
                true
            }

            // Escape smart quote openers: \"` or \'`
            b'\\' if has_quotes
                && self.peek_at(1).is_some_and(|c| c == b'"' || c == b'\'')
                && self.peek_at(2) == Some(b'`') =>
            {
                self.flush_text(*text_start, self.pos, events);
                self.advance_by(1); // skip backslash
                *text_start = self.pos;
                self.advance_by(2); // skip quote + backtick (literal text in next flush)
                true
            }

            // Escape a character reference: \&#174; \&#xA0; \&copy; → drop the backslash and
            // emit the reference as literal text (the renderer escapes its `&` to `&amp;`),
            // matching Asciidoctor's CharRefRx escaping. The whole reference is emitted in one
            // span so an inner `#` (e.g. \&#174;) is never taken for mark/highlight syntax.
            b'\\' if self.char_ref_len_at(self.pos + 1) > 0 => {
                self.flush_text(*text_start, self.pos, events);
                let ref_len = self.char_ref_len_at(self.pos + 1);
                self.advance_by(1); // skip backslash
                let ref_start = self.pos;
                self.advance_by(ref_len);
                events.push(Event::Text(Cow::Borrowed(&self.input[ref_start..self.pos])));
                *text_start = self.pos;
                true
            }

            // `\https://…` — escaped bare autolink: the backslash drops and the
            // URL stays literal text. LinkRx matches the backslash as part of
            // the URL pattern, so this only applies where an unescaped autolink
            // would match (valid boundary before the backslash); elsewhere —
            // `word-\https://…`, `\\https://…` — the backslash stays literal.
            b'\\' if self.subs.has(SubstitutionSet::MACROS)
                && self.autolink_scheme_at(self.pos + 1)
                && self.at_autolink_boundary(self.pos) =>
            {
                self.flush_text(*text_start, self.pos, events);
                self.advance_by(1); // skip backslash; the URL itself is blocked
                *text_start = self.pos; // from autolinking by the `\` before it
                true
            }

            // `\*`/`\_`/`` \` `` with no closing marker of the same kind ahead: the
            // constrained span cannot form, so Asciidoctor keeps the backslash literal.
            // Its quote regexps capture an optional leading `\` (`\\?`) and only strip it
            // when the construct actually matches (`\*bold*` → `*bold*`, escaped span); a
            // marker that never closes (`\* is an asterisk`, `` `\* literal` ``) is left
            // untouched, backslash and all. The blanket arm below still drops the backslash
            // for the would-be-span case — adding only the keep-it case here is
            // regression-safe: `find_closing_constrained` returning None means no marker can
            // close the span, so Asciidoctor cannot have matched it either.
            b'\\' if has_quotes
                && self.peek_at(1).is_some_and(|c| matches!(c, b'*' | b'_' | b'`'))
                && self
                    .find_closing_constrained(self.input.as_bytes()[self.pos + 1], self.pos + 2)
                    .is_none() =>
            {
                self.flush_text(*text_start, self.pos, events);
                let lit_start = self.pos;
                self.advance_by(2); // backslash + marker both stay literal
                events.push(Event::Text(Cow::Borrowed(&self.input[lit_start..self.pos])));
                *text_start = self.pos;
                true
            }

            // Backslash escape: \* \_ \` \# \^ \~ \{ \[ \< \\
            b'\\' if self.peek_at(1).is_some_and(|c| matches!(c, b'*' | b'_' | b'`' | b'#' | b'^' | b'~' | b'{' | b'[' | b'<' | b'\\' | b'\'')) => {
                self.flush_text(*text_start, self.pos, events);
                self.advance_by(1); // skip backslash
                *text_start = self.pos;
                self.advance_by(1); // skip escaped char (included in next text flush)
                true
            }

            // Hard break: ` +` before `\n`, or at a true line edge (end of top-level input)
            b' ' if has_post_replacements && self.check_hard_break() => {
                self.flush_text(*text_start, self.pos, events);
                self.advance_by(2); // skip ` +`
                if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'\n' {
                    self.advance_by(1); // skip `\n`
                }
                events.push(Event::HardBreak);
                *text_start = self.pos;
                true
            }

            _ => false,
        }
    }

    fn flush_text(&self, start: usize, end: usize, events: &mut Vec<Event<'a>>) {
        if start < end {
            let text = &self.input[start..end];
            if self.subs.has(SubstitutionSet::REPLACEMENTS) {
                // An edge-anchored replacement (the spaced em-dash) treats a run edge as a
                // boundary unless that edge is also a true input edge that is NOT a line
                // boundary — i.e. the start/end of inline-span content reparsed in isolation
                // (`` `--` `` → `<code>--</code>`, never an em-dash). Mid-input run edges keep
                // the legacy "boundary" treatment so an attribute-ref that expands to nothing
                // stays transparent (`{empty}--{empty}` → em-dash at the cell's line edges).
                let left_is_boundary = start != 0 || self.edges_are_line_boundaries;
                let right_is_boundary = end < self.input.len() || self.edges_are_line_boundaries;
                events.push(Event::Text(apply_typographic_replacements(
                    text,
                    left_is_boundary,
                    right_is_boundary,
                )));
            } else {
                events.push(Event::Text(Cow::Borrowed(text)));
            }
        }
    }

    /// Emit the explicit display text of an inline macro (link/xref/mailto label).
    /// Asciidoctor runs the quotes, attributes and replacements substitutions
    /// before the macros substitution, so formatting spans (`` `x` `` → `<code>`),
    /// `{attr}` references, apostrophes, dashes, arrows, etc. inside an explicit
    /// `[label]` are already transformed by the time the macro is rendered.
    /// Re-parsing the label with MACROS disabled reproduces that ordering
    /// (a macro consumed into the label is not scanned again by Asciidoctor).
    /// Targets/URLs used as fallback display are emitted raw.
    fn push_macro_label(&self, text: &'a str, events: &mut Vec<Event<'a>>) {
        let label_subs = self.subs.without(SubstitutionSet::MACROS);
        let mut inner_parser = InlineState::new(text, label_subs, self.options);
        inner_parser.parse_inline(events);
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
                // `\--` is an escape only where an unescaped `--` would be replaced
                // (Asciidoctor `(\w)\\?--(?=\w)` / `(^|\n| |\\)--( |\n|$)`): there is
                // no `---` rule, so in `\---` nothing matches and the backslash stays
                // literal.
                let after = bytes.get(p + 2).copied();
                let spaced_ok = matches!(after, None | Some(b' ') | Some(b'\n'));
                let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
                let word_ok = self.pos > 0
                    && is_word(bytes[self.pos - 1])
                    && matches!(after, Some(b) if is_word(b));
                if spaced_ok || word_ok { 2 } else { 0 }
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

    /// If a valid HTML character reference begins at byte `start` (where `input[start]` is `&`),
    /// returns its total byte length (including the leading `&` and trailing `;`); otherwise 0.
    /// Mirrors Asciidoctor's `CharRefRx`: named `[A-Za-z][A-Za-z]+\d{0,2}`, decimal `#\d\d\d{0,4}`
    /// (2–6 digits), or hex `#x[0-9A-Fa-f][0-9A-Fa-f]+`, each terminated by `;`. ASCII-only, so
    /// byte indexing is safe.
    fn char_ref_len_at(&self, start: usize) -> usize {
        let bytes = self.input.as_bytes();
        if bytes.get(start) != Some(&b'&') {
            return 0;
        }
        let mut i = start + 1;
        if bytes.get(i) == Some(&b'#') {
            i += 1;
            if matches!(bytes.get(i), Some(b'x' | b'X')) {
                // hex: at least 2 hex digits
                i += 1;
                let hex_start = i;
                while bytes.get(i).is_some_and(u8::is_ascii_hexdigit) {
                    i += 1;
                }
                if i - hex_start < 2 {
                    return 0;
                }
            } else {
                // decimal: 2..=6 digits
                let dec_start = i;
                while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
                if !(2..=6).contains(&(i - dec_start)) {
                    return 0;
                }
            }
        } else {
            // named: a letter, then at least one more letter, then 0..=2 trailing digits
            let name_start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_alphabetic) {
                i += 1;
            }
            if i - name_start < 2 {
                return 0;
            }
            let mut digits = 0;
            while digits < 2 && bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
                digits += 1;
            }
        }
        if bytes.get(i) == Some(&b';') { i + 1 - start } else { 0 }
    }

    /// If a recognized inline macro form (`name:target[attrs]`) begins at byte `p`, returns its
    /// total byte length; otherwise 0. Used to strip a leading backslash escape and emit the macro
    /// text literally, matching Asciidoctor (which drops the backslash and skips macro processing).
    /// Only macros enabled by default are recognized; the experimental `kbd:`/`btn:`/`menu:` macros
    /// and unknown macro names are excluded (Asciidoctor leaves their backslash intact), as is the
    /// block form `image::` (double colon). Gated on the MACROS substitution being active.
    fn inline_macro_escape_len(&self, p: usize) -> usize {
        if !self.subs.has(SubstitutionSet::MACROS) {
            return 0;
        }
        let Some(rest) = self.input.get(p..) else {
            return 0;
        };
        // Longest-first is not required (each name is uniquely delimited by its colon), but keep
        // indexterm2 before indexterm for clarity.
        const NAMES: [&str; 12] = [
            "stem:", "latexmath:", "asciimath:", "link:", "xref:", "mailto:", "icon:",
            "indexterm2:", "indexterm:", "footnote:", "image:", "anchor:",
        ];
        let Some(name_len) = NAMES.iter().find_map(|n| rest.starts_with(n).then_some(n.len())) else {
            return 0;
        };
        let bytes = rest.as_bytes();
        // Reject the block-macro form `name::` (e.g. image::target[]).
        if bytes.get(name_len) == Some(&b':') {
            return 0;
        }
        // Target: a run of non-whitespace characters up to the opening bracket.
        let mut i = name_len;
        while let Some(&c) = bytes.get(i) {
            if matches!(c, b'[' | b' ' | b'\t' | b'\n') {
                break;
            }
            i += 1;
        }
        // Require an opening bracket immediately, then a closing bracket somewhere after it.
        if bytes.get(i) != Some(&b'[') {
            return 0;
        }
        i += 1; // past '['
        while let Some(&c) = bytes.get(i) {
            if c == b']' {
                return i + 1; // length from p to the closing ']' inclusive
            }
            i += 1;
        }
        0
    }

    fn check_hard_break(&self) -> bool {
        let bytes = self.input.as_bytes();
        if self.pos + 2 > self.input.len() || bytes[self.pos] != b' ' || bytes[self.pos + 1] != b'+'
        {
            return false;
        }
        if self.pos + 2 == self.input.len() {
            // ` +` at end of input is a hard break only at a true line/paragraph edge.
            // Asciidoctor applies the line-break replacement after spans are rendered, so a
            // trailing ` +` inside a reparsed inline span is bounded by its closing tag, not
            // by `$` (`` `x +` `` → `<code>x +</code>`, never `<code>x<br></code>`). Inner
            // reparses leave `edges_are_line_boundaries` false; top-level text sets it true.
            self.edges_are_line_boundaries
        } else {
            bytes[self.pos + 2] == b'\n'
        }
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

        // At the leading edge of a smart-quote span, constrained monospace/emphasis/mark
        // cannot open — they run after the `:double`/`:single` substitution, so they see
        // the trailing `;` of the emitted `&#8220;`/`&#8216;`, which their open assertion
        // forbids. Constrained strong (`*`) ran *before* that substitution and is exempt;
        // it matched against the original backtick. See `smart_quote_leading_edge`.
        if self.smart_quote_leading_edge
            && start_pos == 0
            && matches!(marker, b'`' | b'_' | b'#')
        {
            return false;
        }

        // At the leading edge of an emphasis span (`_…_` / `__…__`), constrained strong
        // (`*`) and monospace (`` ` ``) cannot open: both run before emphasis in
        // QUOTE_SUBS, so they still see the literal `_` (a word character) their open
        // assertion forbids. Mark (`#`) runs after emphasis and is exempt. See
        // `emphasis_leading_edge`.
        if self.emphasis_leading_edge && start_pos == 0 && matches!(marker, b'*' | b'`') {
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
            // Constrained monospace has a stricter close assertion than the other
            // quotes: Asciidoctor's `(?![\w"'`])` also forbids `"`, `'` and a
            // backtick immediately after the closing tick. Without this, a backtick
            // that is really the start of a typographic right single quote (`` `' ``)
            // is mistaken for a monospace close, e.g. `` the `'00s ... werewolves`' ``
            // would wrongly fold into <code> instead of rendering two `’` apostrophes.
            if marker == b'`'
                && matches!(self.input.as_bytes().get(after_close), Some(b'"' | b'\'' | b'`'))
            {
                return false;
            }

            self.flush_text(*text_start, start_pos, events);
            events.push(Event::Start(tag));

            // Constrained monospace `` `text` `` undergoes the full normal substitution
            // group, replacements included: Asciidoctor applies `(C)`/`--`/`...` and
            // restores valid char-refs (`&#167;`) inside `<code>` exactly as in prose.
            // Literal monospace (`+...+`, `pass:[]`) is intercepted as a passthrough
            // before this point, so it stays verbatim regardless of these subs.
            let mut inner_parser = InlineState::new(inner, self.subs, self.options);
            inner_parser.emphasis_leading_edge = marker == b'_';
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
                // Superscript/subscript content undergoes the full normal
                // substitution group in Asciidoctor (attributes, quotes,
                // replacements, macros): `^a{sp}b^` → `<sup>a b</sup>`,
                // `^*x*^` → `<sup><strong>x</strong></sup>`, `^a--b^` em-dash,
                // `^url[t]^` a link. Reparse the inner span rather than emitting
                // it verbatim.
                let mut inner_parser = InlineState::new(inner, self.subs, self.options);
                inner_parser.parse_inline(events);
                events.push(Event::End(tag_end));

                self.pos = close_pos + 1;
                *text_start = self.pos;
                return true;
            }
        }

        false
    }

    fn find_closing_constrained(&self, marker: u8, search_start: usize) -> Option<usize> {
        let s = &self.input[search_start..];
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            // Passthroughs are extracted before quote substitution in AsciiDoc, so
            // a quote marker that lives inside a `++…++` / `+++…+++` / `+…+` passthrough
            // must not terminate the surrounding span. Skip the whole passthrough region;
            // its inner content (incl. any markers) is handled when the span is reparsed.
            if b == b'+' {
                if let Some(skip) = crate::scanner::passthrough_span_len(s, i) {
                    i += skip;
                    continue;
                }
                if let Some(skip) = crate::scanner::single_plus_span_len(s, i) {
                    i += skip;
                    continue;
                }
            }
            // The `pass:[…]` inline macro is also extracted before quote
            // substitution, so a quote marker living inside its bracket must not
            // terminate the surrounding span (`` `pass:[`']` `` → `<code>`'</code>`).
            if b == b'p'
                && let Some(skip) = crate::scanner::pass_macro_span_len(s, i)
            {
                i += skip;
                continue;
            }
            if b == marker && i > 0 && bytes.get(i + 1).copied() != Some(marker) {
                return Some(i);
            }
            i += 1;
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

            // Unconstrained monospace ``` ``text`` ``` undergoes the full normal
            // substitution group, replacements included — same rule as constrained
            // monospace (see try_constrained): `(C)`/`--`/`...` are replaced and valid
            // char-refs restored inside `<code>`. Passthroughs stay verbatim regardless.
            let mut inner_parser = InlineState::new(inner, self.subs, self.options);
            inner_parser.emphasis_leading_edge = marker == b'_';
            inner_parser.parse_inline(events);

            events.push(Event::End(tag_end));

            self.pos = close_pos + 2;
            *text_start = self.pos;
            return true;
        }

        false
    }

    fn find_closing_unconstrained(&self, marker: u8, search_start: usize) -> Option<usize> {
        let s = &self.input[search_start..];
        let bytes = s.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            // Passthroughs (`++…++`/`+++…+++`/`+…+`/`pass:[…]`) are extracted before
            // quote substitution, so a marker pair inside them must not close
            // the surrounding unconstrained span (mirror of
            // find_closing_constrained; `**a+++**+++b**` → strong over a**b).
            if bytes[i] == b'+' {
                if let Some(skip) = crate::scanner::passthrough_span_len(s, i) {
                    i += skip;
                    continue;
                }
                if let Some(skip) = crate::scanner::single_plus_span_len(s, i) {
                    i += skip;
                    continue;
                }
            }
            if bytes[i] == b'p'
                && let Some(skip) = crate::scanner::pass_macro_span_len(s, i)
            {
                i += skip;
                continue;
            }
            if bytes[i] == marker && bytes[i + 1] == marker {
                return Some(search_start + i);
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

        let inner = &rest[..close];

        self.flush_text(*text_start, start_pos, events);
        // Double-plus passthrough applies ONLY the `specialcharacters` sub (escapes
        // `<`/`>`/`&`) — no quotes, replacements, attributes, or inline re-parsing
        // (probe-verified: `++*x*++`→`*x*`, `++a -- b++`→`a -- b`, `++a & b++`→`a &amp; b`).
        // Emitting `Event::Text` (not `InlinePassthrough`) gives exactly that: the
        // renderer html-escapes Text but does not re-run subs. Triple-plus stays raw
        // (`InlinePassthrough`). `++++` → empty passthrough: renders as nothing.
        if !inner.is_empty() {
            events.push(Event::Text(Cow::Borrowed(inner)));
        }

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

        // Single-plus passthrough is a *constrained* pair (like `*`/`_`/`` ` ``):
        // the opening '+' must not follow a word character (`C+a+` stays literal)
        // and the content's first char must not be a space (`+ a+` stays literal).
        if self.is_word_char_before(start_pos) {
            return false;
        }

        let after_open = start_pos + 1; // skip "+"
        if after_open >= self.input.len() {
            return false;
        }
        if self.input.as_bytes()[after_open] == b' ' {
            return false;
        }

        // Find a closing single '+' obeying the constrained-close rule: it must not
        // be part of '++'/'+++', must not be immediately followed by a word char,
        // and the content must not end with a space. A '+' that fails these (e.g. the
        // inner '+' in `+a+b+`, followed by a word char) cannot close, so the scan
        // continues to the next candidate (`+a+b+` → content `a+b`).
        // `pass:[…]` macros are extracted before the `+…+` span in AsciiDoc, so a
        // '+' inside their brackets cannot close — skip the whole macro region.
        let s = &self.input[after_open..];
        let bytes = s.as_bytes();
        let mut close = None;
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'p'
                && let Some(skip) = crate::scanner::pass_macro_span_len(s, i)
            {
                i += skip;
                continue;
            }
            if b == b'+' && i > 0 {
                let preceded_by_plus = bytes[i - 1] == b'+';
                let preceded_by_space = bytes[i - 1] == b' ';
                let next = bytes.get(i + 1).copied();
                let followed_by_plus = next == Some(b'+');
                let followed_by_word =
                    next.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_');
                if !preceded_by_plus && !preceded_by_space && !followed_by_plus && !followed_by_word
                {
                    close = Some(i);
                    break;
                }
            }
            i += 1;
        }

        let close = match close {
            Some(c) => c,
            None => return false,
        };

        let inner = &self.input[after_open..after_open + close];

        self.flush_text(*text_start, start_pos, events);
        // Single-plus: literal content (no inline parsing, no typographic
        // replacements) — except `pass:[…]` macros, which AsciiDoc extracts
        // before the `+…+` span is matched (`+pass:[x]+` → `x`). Inside
        // `++…++`/`+++…+++` the macro is NOT extracted (the double/triple-plus
        // span wins positionally in the same extraction pass) — those emit
        // their content verbatim and stay untouched.
        Self::push_single_plus_content(inner, events);

        self.pos = after_open + close + 1;
        *text_start = self.pos;
        true
    }

    /// Emit the content of a single-plus passthrough: literal `Text`, except
    /// embedded `pass:[…]` macros, which become `InlinePassthrough` (mirrors
    /// `try_pass_macro` — raw content to the first `]`). A spec'd macro whose
    /// set keeps specialchars (`pass:c[…]`) emits `Text` instead, so the
    /// renderer escapes it; formatting subs from a spec are not re-run here
    /// (no parser state in this static helper — membership-only edge).
    fn push_single_plus_content(inner: &'a str, events: &mut Vec<Event<'a>>) {
        let bytes = inner.as_bytes();
        let mut text_start = 0;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'p'
                && let Some(skip) = crate::scanner::pass_macro_span_len(inner, i)
            {
                if i > text_start {
                    events.push(Event::Text(Cow::Borrowed(&inner[text_start..i])));
                }
                let spec_len = crate::scanner::pass_spec_len(&inner[i..], 5).unwrap_or(0);
                let spec = &inner[i + 5..i + 5 + spec_len];
                // content sits between "pass:SPEC[" and the trailing "]"
                let content = &inner[i + 5 + spec_len + 1..i + skip - 1];
                if !spec.is_empty()
                    && pass_spec_to_subs(spec).has(SubstitutionSet::SPECIALCHARS)
                {
                    events.push(Event::Text(Cow::Borrowed(content)));
                } else {
                    events.push(Event::InlinePassthrough(Cow::Borrowed(content)));
                }
                i += skip;
                text_start = i;
                continue;
            }
            i += 1;
        }
        if text_start < inner.len() {
            events.push(Event::Text(Cow::Borrowed(&inner[text_start..])));
        }
    }

    /// Parse a `<prefix>[content]` bracket macro from the current position.
    /// Returns `(content, new_pos)` (position just past `]`), or `None` if there
    /// is no `[...]`. Does not emit events or move `self.pos` — the caller owns
    /// flush/emit/empty-policy.
    fn parse_bracket_macro(&self, prefix_len: usize) -> Option<(&'a str, usize)> {
        let rest = &self.input[self.pos + prefix_len..];
        if !rest.starts_with('[') {
            return None;
        }
        let bracket_end = rest.find(']')?;
        Some((&rest[1..bracket_end], self.pos + prefix_len + bracket_end + 1))
    }

    /// Like `parse_bracket_macro`, but a `]` directly preceded by `\` does not
    /// close the bracket, and every `\]` in the content is unescaped to `]`
    /// (Asciidoctor's stem-macro content rule: `(.*?[^\\])?\]`).
    fn parse_bracket_macro_escaped(&self, prefix_len: usize) -> Option<(Cow<'a, str>, usize)> {
        let rest = &self.input[self.pos + prefix_len..];
        if !rest.starts_with('[') {
            return None;
        }
        let bytes = rest.as_bytes();
        let mut i = 1;
        let bracket_end = loop {
            match bytes[i..].iter().position(|&b| b == b']') {
                Some(off) => {
                    let at = i + off;
                    if bytes[at - 1] == b'\\' {
                        i = at + 1;
                    } else {
                        break at;
                    }
                }
                None => return None,
            }
        };
        let inner = &rest[1..bracket_end];
        let content = if inner.contains("\\]") {
            Cow::Owned(inner.replace("\\]", "]"))
        } else {
            Cow::Borrowed(inner)
        };
        Some((content, self.pos + prefix_len + bracket_end + 1))
    }

    fn try_kbd_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let Some((content, new_pos)) = self.parse_bracket_macro(4) else {
            return false;
        };
        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Keyboard));
        events.push(Event::Text(Cow::Borrowed(content)));
        events.push(Event::End(TagEnd::Keyboard));

        self.pos = new_pos;
        *text_start = self.pos;
        true
    }

    fn try_btn_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let Some((content, new_pos)) = self.parse_bracket_macro(4) else {
            return false;
        };
        if content.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Button));
        events.push(Event::Text(Cow::Borrowed(content)));
        events.push(Event::End(TagEnd::Button));

        self.pos = new_pos;
        *text_start = self.pos;
        true
    }

    /// With the experimental UI macros disabled, advance past a `kbd:`/`btn:`/
    /// `menu:` token so it stays in the surrounding text run as literal text
    /// (matching Asciidoctor's default). Skipping past the `[...]` keeps the
    /// token's interior from being rescanned and misread. `prefix_len` is
    /// the byte length of `kbd:`/`btn:`/`menu:` (colon included).
    fn skip_disabled_ui_macro(&mut self, prefix_len: usize) {
        let rest = &self.input[self.pos + prefix_len..];
        if let Some(open) = rest.find('[')
            && let Some(close) = rest[open + 1..].find(']')
        {
            self.pos += prefix_len + open + 1 + close + 1;
        } else {
            self.pos += prefix_len;
        }
    }

    /// Parse a `<prefix>target[items]` bracket macro from the current position.
    /// Returns `(target, items, new_pos)` (position just past `]`), or `None` if
    /// the target is empty or the `[...]` is missing/malformed. Caller owns
    /// flush/emit.
    fn parse_target_bracket_macro(&self, prefix_len: usize) -> Option<(&'a str, &'a str, usize)> {
        let rest = &self.input[self.pos + prefix_len..];
        let bracket_start = rest.find('[')?;
        let target = &rest[..bracket_start];
        if target.is_empty() {
            return None;
        }
        let bracket_end = rest.find(']')?;
        if bracket_end <= bracket_start {
            return None;
        }
        Some((
            target,
            &rest[bracket_start + 1..bracket_end],
            self.pos + prefix_len + bracket_end + 1,
        ))
    }

    fn try_menu_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let Some((target, items, new_pos)) = self.parse_target_bracket_macro(5) else {
            return false;
        };

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Menu {
            target: Cow::Borrowed(target),
        }));
        if !items.is_empty() {
            events.push(Event::Text(Cow::Borrowed(items)));
        }
        events.push(Event::End(TagEnd::Menu));

        self.pos = new_pos;
        *text_start = self.pos;
        true
    }

    fn try_icon_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let Some((name, attrs, new_pos)) = self.parse_target_bracket_macro(5) else {
            return false;
        };

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Icon {
            name: Cow::Borrowed(name),
        }));
        if !attrs.is_empty() {
            events.push(Event::Text(Cow::Borrowed(attrs)));
        }
        events.push(Event::End(TagEnd::Icon));

        self.pos = new_pos;
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
        // Escaped `\]` does not close the macro and is unescaped in the content
        // (`stem:[[[a,b\],[c,d\]\]]` → `[[a,b],[c,d]]`, probe-verified).
        let Some((content, new_pos)) = self.parse_bracket_macro_escaped(prefix_len) else {
            return false;
        };

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Start(Tag::Stem {
            variant: Cow::Borrowed(variant),
        }));
        if !content.is_empty() {
            events.push(Event::Text(content));
        }
        events.push(Event::End(TagEnd::Stem));

        self.pos = new_pos;
        *text_start = self.pos;
        true
    }

    fn try_pass_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        // Optional subs spec between the colon and the bracket: `pass:c[…]`,
        // `pass:q,a[…]`, full names too (`pass:quotes[…]`). Without brackets
        // the macro form does not match at all and the text stays literal.
        let Some(spec_len) = crate::scanner::pass_spec_len(self.input, start_pos + 5) else {
            return false;
        };
        let Some((inner, new_pos)) = self.parse_bracket_macro(5 + spec_len) else {
            return false;
        };

        self.flush_text(*text_start, start_pos, events);

        if spec_len == 0 {
            // Bare `pass:[…]` — raw verbatim insertion.
            events.push(Event::InlinePassthrough(Cow::Borrowed(inner)));
        } else {
            let spec = &self.input[start_pos + 5..start_pos + 5 + spec_len];
            self.push_pass_spec_content(inner, pass_spec_to_subs(spec), events);
        }

        self.pos = new_pos;
        *text_start = self.pos;
        true
    }

    /// Emit `pass:SPEC[content]`: the content is reparsed with exactly the
    /// spec'd substitutions. When specialchars is absent the plain-text runs
    /// must reach the output unescaped, so they are downgraded to
    /// `InlinePassthrough` (the renderer escapes `Text` unconditionally).
    /// Order-of-application nuances (`pass:c,q[…]` — Asciidoctor runs quotes
    /// over the already-escaped text, where `;` blocks a constrained open)
    /// are not representable in the bitflag model; membership only.
    fn push_pass_spec_content(
        &self,
        content: &'a str,
        set: SubstitutionSet,
        events: &mut Vec<Event<'a>>,
    ) {
        let mut sub_events = Vec::new();
        let mut inner = InlineState::new(content, set, self.options);
        inner.parse_inline(&mut sub_events);
        let escape = set.has(SubstitutionSet::SPECIALCHARS);
        for ev in sub_events {
            match ev {
                Event::Text(t) if !escape => events.push(Event::InlinePassthrough(t)),
                ev => events.push(ev),
            }
        }
    }

    /// Length of a literal `pass:SPEC[` prefix beginning at byte `p`, for the
    /// backslash-escape arm; 0 when the escaped form does not match (then the
    /// backslash itself stays literal, as in Asciidoctor).
    fn pass_escape_prefix_len(&self, p: usize) -> usize {
        let Some(rest) = self.input.get(p..) else {
            return 0;
        };
        if !rest.starts_with("pass:") {
            return 0;
        }
        match crate::scanner::pass_spec_len(rest, 5) {
            Some(spec_len) => 5 + spec_len + 1, // "pass:" + spec + "["
            None => 0,
        }
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

        // Strip leading '#' — it's just an explicit anchor marker, not part of the ID
        let target = target.strip_prefix('#').unwrap_or(target);

        events.push(Event::Start(Tag::CrossReference {
            target: Cow::Borrowed(target),
            label: label.clone(),
        }));
        match label {
            Some(Cow::Borrowed(l)) => self.push_macro_label(l, events),
            Some(l) => events.push(Event::Text(l)),
            None => events.push(Event::Text(Cow::Borrowed(target))),
        }
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
            // Asciidoctor marks a link macro with no explicit text as "bare"
            // (the visible text defaults to the target) → class="bare".
            let is_bare = link_attrs.text.is_empty();
            events.push(Event::Start(Tag::Link {
                url: Cow::Borrowed(url),
                window: link_attrs.window.map(Cow::Borrowed),
                nofollow: link_attrs.nofollow,
                is_bare,
                role: link_attrs.role.map(Cow::Borrowed),
            }));
            if is_bare {
                events.push(Event::Text(Cow::Borrowed(url)));
            } else {
                self.push_macro_label(link_attrs.text, events);
            }
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
        let is_bare = link_attrs.text.is_empty();
        events.push(Event::Start(Tag::Link {
            url: Cow::Borrowed(url),
            window: link_attrs.window.map(Cow::Borrowed),
            nofollow: link_attrs.nofollow,
            is_bare,
            role: link_attrs.role.map(Cow::Borrowed),
        }));
        if is_bare {
            events.push(Event::Text(Cow::Borrowed(url)));
        } else {
            self.push_macro_label(link_attrs.text, events);
        }
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

        // Build mailto: URL; positional attrs 2/3 become ?subject=&body= query
        // params, percent-encoded (asciidoctor keeps only A-Za-z0-9_.~- literal).
        let url = &self.input[start_pos..start_pos + 7 + bracket_start]; // "mailto:email"
        let link_attrs = parse_link_attrs(bracket_content);
        let url: Cow<'a, str> = match (link_attrs.subject, link_attrs.body) {
            (None, None) => Cow::Borrowed(url),
            (subject, body) => {
                let mut u = String::from(url);
                let mut sep = '?';
                if let Some(s) = subject {
                    u.push(sep);
                    u.push_str("subject=");
                    url_encode_into(&mut u, s);
                    sep = '&';
                }
                if let Some(b) = body {
                    u.push(sep);
                    u.push_str("body=");
                    url_encode_into(&mut u, b);
                }
                Cow::Owned(u)
            }
        };
        events.push(Event::Start(Tag::Link {
            url,
            window: link_attrs.window.map(Cow::Borrowed),
            nofollow: link_attrs.nofollow,
            is_bare: false,
            role: link_attrs.role.map(Cow::Borrowed),
        }));
        if link_attrs.text.is_empty() {
            events.push(Event::Text(Cow::Borrowed(email)));
        } else {
            self.push_macro_label(link_attrs.text, events);
        }
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
        match label {
            Some(Cow::Borrowed(l)) => self.push_macro_label(l, events),
            Some(l) => events.push(Event::Text(l)),
            None => events.push(Event::Text(Cow::Borrowed(target))),
        }
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
            link: img_attrs.link.map(Cow::Borrowed),
            role: img_attrs.role.map(Cow::Borrowed),
            title: img_attrs.title.map(Cow::Borrowed),
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

        // Handle {set:name:value} / {set:name!} / {set:name} inline macro
        if let Some(set_rest) = content.strip_prefix("set:") {
            return self.try_inline_set(set_rest, events, text_start, start_pos, close);
        }

        // Asciidoctor's reference name is `\w[\w-]*` — `{n!}` or any other
        // content with `!` is not a reference and stays literal (probe-verified).
        let attr_name = content;

        if attr_name.is_empty() {
            return false;
        }
        let first = attr_name.as_bytes()[0];
        if !(first.is_ascii_alphanumeric() || first == b'_') {
            return false;
        }
        if !attr_name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return false;
        }

        // Capture a `[...]` following the reference (no space) so the renderer can
        // re-parse `value<path>[...]` together: an attribute holding a URL then
        // forms a link macro, matching Asciidoctor's attributes-before-macros
        // order. An optional path segment between `}` and `[` is captured too
        // (`{url}/issues[text]` → `value/issues[text]` → URL macro). Skip `[[`
        // (inline anchor) and a bracket with no closing `]`.
        let input = self.input; // &'a str (Copy) — decouple slice lifetime from &self
        let after_brace = start_pos + 1 + close + 1;
        let trailing_brackets = {
            let tail = &input[after_brace..];
            // Path = run of non-space, non-bracket bytes (all stop chars are ASCII,
            // so the count is a valid char boundary even with UTF-8 in between).
            let path_len = tail
                .bytes()
                .take_while(|&b| b != b'[' && b != b']' && !b.is_ascii_whitespace())
                .count();
            let after_path = &tail[path_len..];
            if after_path.starts_with('[') && !after_path.starts_with("[[") {
                after_path
                    .find(']')
                    .map(|rb| Cow::Borrowed(&tail[..path_len + rb + 1]))
            } else {
                None
            }
        };
        let consumed = trailing_brackets.as_ref().map_or(0, |b| b.len());

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::AttributeReference {
            name: Cow::Borrowed(attr_name),
            fallback: None,
            trailing_brackets,
        });

        self.pos = after_brace + consumed;
        *text_start = self.pos;
        true
    }

    fn try_inline_set(
        &mut self,
        set_rest: &'a str,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
        start_pos: usize,
        close: usize,
    ) -> bool {
        // {set:name!} — unset attribute
        if let Some(name) = set_rest.strip_suffix('!') {
            if name.is_empty() || !Self::is_valid_attr_name(name) {
                return false;
            }
            self.flush_text(*text_start, start_pos, events);
            let unset_name = format!("!{name}");
            events.push(Event::Attribute {
                name: Cow::Owned(unset_name),
                value: Cow::Borrowed(""),
            });
            self.pos = start_pos + 1 + close + 1;
            *text_start = self.pos;
            return true;
        }

        // {set:name:value} or {set:name} (empty value)
        let (name, value) = if let Some(colon_pos) = set_rest.find(':') {
            (&set_rest[..colon_pos], &set_rest[colon_pos + 1..])
        } else {
            (set_rest, "")
        };

        if name.is_empty() || !Self::is_valid_attr_name(name) {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);
        events.push(Event::Attribute {
            name: Cow::Borrowed(name),
            value: Cow::Borrowed(value),
        });

        self.pos = start_pos + 1 + close + 1;
        *text_start = self.pos;
        true
    }

    fn is_valid_attr_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let first = name.as_bytes()[0];
        if !(first.is_ascii_alphanumeric() || first == b'_') {
            return false;
        }
        name.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    }

    /// Asciidoctor's InlineLinkRx matches a bare URL only after start-of-text,
    /// whitespace, or one of `<>()[];` — any other preceding character blocks
    /// the autolink (probe-verified: `see:https://…`, `word-https://…`, `a=…`,
    /// `a,…`, and straight `"`/`'` stay literal, e.g. an escaped
    /// `include::https://…[]` line).
    fn at_autolink_boundary(&self, p: usize) -> bool {
        match self.input[..p].chars().next_back() {
            None => true,
            Some(prev) => {
                prev.is_whitespace()
                    || matches!(prev, '<' | '>' | '(' | ')' | '[' | ']' | ';')
            }
        }
    }

    /// True when an autolink scheme (`http://`, `https://`, `ftp://`, `irc://`)
    /// starts at byte offset `p`.
    fn autolink_scheme_at(&self, p: usize) -> bool {
        self.input.get(p..).is_some_and(|rest| {
            rest.starts_with("http://")
                || rest.starts_with("https://")
                || rest.starts_with("ftp://")
                || rest.starts_with("irc://")
        })
    }

    fn try_autolink(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;

        if !self.at_autolink_boundary(start_pos) {
            return false;
        }

        let rest = &self.input[start_pos..];

        let url_end = rest
            .find(|c: char| c.is_whitespace() || c == '[' || c == ']' || c == '<' || c == '>')
            .unwrap_or(rest.len());

        let mut url = &rest[..url_end];
        if url.len() <= 8 {
            return false;
        }

        // Trailing punctuation (incl. `)`) is stripped only from BARE urls —
        // the `URL[text]` macro form keeps it (separate InlineLinkRx alternate),
        // so check for an attrlist on the unstripped url first.
        let bracket_follows = rest[url_end..].starts_with('[') && rest[url_end..].contains(']');
        if !bracket_follows {
            while url.len() > 8 && matches!(url.as_bytes()[url.len() - 1], b'.' | b',' | b';' | b':' | b'!' | b'?' | b')') {
                url = &url[..url.len() - 1];
            }
        }
        let url_end = url.len();

        self.flush_text(*text_start, start_pos, events);

        // Check for [link text] immediately after the URL
        let after_url = &rest[url_end..];
        if after_url.starts_with('[')
            && let Some(close) = after_url.find(']')
        {
            let bracket_content = &after_url[1..close];
            let link_attrs = parse_link_attrs(bracket_content);
            let is_bare = link_attrs.text.is_empty();
            events.push(Event::Start(Tag::Link {
                url: Cow::Borrowed(url),
                window: link_attrs.window.map(Cow::Borrowed),
                nofollow: link_attrs.nofollow,
                is_bare,
                role: link_attrs.role.map(Cow::Borrowed),
            }));
            if is_bare {
                events.push(Event::Text(Cow::Borrowed(url)));
            } else {
                self.push_macro_label(link_attrs.text, events);
            }
            events.push(Event::End(TagEnd::Link));
            self.pos = start_pos + url_end + close + 1;
            *text_start = self.pos;
            return true;
        }

        events.push(Event::Start(Tag::Link {
            url: Cow::Borrowed(url),
            window: None,
            nofollow: false,
            is_bare: true,
            role: None,
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
            // Asciidoctor does not add class="bare" to email autolinks (only to
            // bare URL autolinks and link:/URL macros with empty text).
            is_bare: false,
            role: None,
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
        let content = &rest[..close];

        if content.is_empty() {
            return false;
        }

        // `[[id,xreflabel]]` — the label is reference text for xrefs, never
        // part of the id.
        let (id, label) = match content.split_once(',') {
            Some((i, l)) => {
                let l = l.trim_start();
                (i.trim_end(), (!l.is_empty()).then_some(Cow::Borrowed(l)))
            }
            None => (content, None),
        };
        if id.is_empty() {
            return false;
        }

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::Start(Tag::Anchor {
            id: Cow::Borrowed(id),
            label,
        }));
        events.push(Event::End(TagEnd::Anchor));

        self.pos = start_pos + 2 + close + 2;
        *text_start = self.pos;
        true
    }

    /// Inline anchor macro `anchor:id[]` / `anchor:id[xreflabel]` — equivalent
    /// to `[[id]]`. The bracket content (xreflabel) is reference text for
    /// xrefs, never rendered in place.
    fn try_anchor_macro(
        &mut self,
        events: &mut Vec<Event<'a>>,
        text_start: &mut usize,
    ) -> bool {
        let start_pos = self.pos;
        let rest = &self.input[start_pos + 7..]; // skip "anchor:"

        let bracket = match rest.find('[') {
            Some(b) => b,
            None => return false,
        };
        let id = &rest[..bracket];
        // Target is a run of non-whitespace characters (Asciidoctor: \S+).
        if id.is_empty() || id.contains(char::is_whitespace) {
            return false;
        }
        let close = match rest[bracket + 1..].find(']') {
            Some(c) => c,
            None => return false,
        };
        let label_text = &rest[bracket + 1..bracket + 1 + close];

        self.flush_text(*text_start, start_pos, events);

        events.push(Event::Start(Tag::Anchor {
            id: Cow::Borrowed(id),
            label: (!label_text.is_empty()).then_some(Cow::Borrowed(label_text)),
        }));
        events.push(Event::End(TagEnd::Anchor));

        self.pos = start_pos + 7 + bracket + 1 + close + 1;
        *text_start = self.pos;
        true
    }

    /// Closing `))` position for an index term, relative to the content start
    /// (the value is the content length). Mirrors Asciidoctor's non-greedy
    /// `(.+?)\)\)(?!\))`: the first `))` whose follower is yet another `)`
    /// slides forward by one, extending the content (`a)))` → content `a)`).
    fn index_term_close(rest: &str) -> Option<usize> {
        let bytes = rest.as_bytes();
        let mut close = rest.find("))")?;
        while bytes.get(close + 2) == Some(&b')') {
            close += 1;
        }
        if close == 0 { None } else { Some(close) }
    }

    /// Index term at `((`: the matched content decides the form
    /// (Asciidoctor substitutors.rb, InlineIndextermMacroRx else-branch):
    /// parens on both ends → concealed term (invisible, comma-split);
    /// a paren on one end only stays literal text around a flow term;
    /// otherwise a plain flow term rendering its text.
    fn try_index_term(&mut self, events: &mut Vec<Event<'a>>, text_start: &mut usize) -> bool {
        let start_pos = self.pos;
        let after_open = start_pos + 2; // skip "(("

        let rest = &self.input[after_open..];
        let close = match Self::index_term_close(rest) {
            Some(c) => c,
            None => return false,
        };
        let content = &rest[..close];

        self.flush_text(*text_start, start_pos, events);

        let starts = content.starts_with('(');
        let ends = content.ends_with(')');
        if starts && ends {
            let inner = &content[1..content.len() - 1];
            let mut parts = inner.splitn(3, ',');
            let primary = parts.next().unwrap().trim();
            let secondary = parts.next().map(|s| s.trim());
            let tertiary = parts.next().map(|s| s.trim());
            events.push(Event::ConcealedIndexTerm {
                primary: Cow::Borrowed(primary),
                secondary: secondary.map(Cow::Borrowed),
                tertiary: tertiary.map(Cow::Borrowed),
            });
        } else if starts {
            events.push(Event::Text(Cow::Borrowed("(")));
            events.push(Event::IndexTerm {
                text: Cow::Borrowed(&content[1..]),
            });
        } else if ends {
            events.push(Event::IndexTerm {
                text: Cow::Borrowed(&content[..content.len() - 1]),
            });
            events.push(Event::Text(Cow::Borrowed(")")));
        } else {
            events.push(Event::IndexTerm {
                text: Cow::Borrowed(content),
            });
        }

        self.pos = after_open + close + 2;
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

        // Mirror Asciidoctor `parse_quoted_text_attributes`: only the first
        // positional attribute is considered, and an attrlist that does NOT use the
        // `.`/`#` shorthand is taken verbatim as a single role — dots are NOT split
        // (`[a.b]##x##` → role "a.b", whereas shorthand `[.a.b]` → roles "a b").
        let attr_content = &self.input[after_bracket..bracket_close];
        let first_positional = attr_content
            .split(',')
            .next()
            .unwrap_or(attr_content)
            .trim();
        let (id, roles) = if first_positional.is_empty() {
            (None, Vec::new())
        } else if first_positional.starts_with('.') || first_positional.starts_with('#') {
            Self::parse_inline_shorthand(first_positional)
        } else {
            (None, vec![first_positional])
        };

        // Must have at least an id or a role
        if id.is_none() && roles.is_empty() {
            return false;
        }

        let after_close_bracket = bracket_close + 1;
        if after_close_bracket >= self.input.len() {
            return false;
        }

        // The attribute list may be followed by `#` (span), or by a formatting
        // marker `_`/`*`/backtick — in the latter case the id and roles are applied
        // directly to the emphasis/strong/monospace element (e.g. [.path]_x_ → <em class="path">).
        let marker = bytes[after_close_bracket];
        if !matches!(marker, b'#' | b'_' | b'*' | b'`') {
            return false;
        }

        let id_cow = id.map(Cow::Borrowed);
        let roles_cow = roles.iter().copied().map(Cow::Borrowed).collect::<Vec<_>>();
        let start_tag = match marker {
            b'_' => Tag::Emphasis { id: id_cow, roles: roles_cow },
            b'*' => Tag::Strong { id: id_cow, roles: roles_cow },
            b'`' => Tag::Monospace { id: id_cow, roles: roles_cow },
            _ => Tag::InlineSpan { id: id_cow, roles: roles_cow },
        };
        let end_tag = match marker {
            b'_' => TagEnd::Emphasis,
            b'*' => TagEnd::Strong,
            b'`' => TagEnd::Monospace,
            _ => TagEnd::InlineSpan,
        };
        // Monospace content is literal — no typographic replacements (mirrors try_constrained).
        let inner_subs = if marker == b'`' {
            self.subs.without(SubstitutionSet::REPLACEMENTS)
        } else {
            self.subs
        };

        // Doubled marker → unconstrained: [.class]##text## / [.class]__text__ etc.
        let is_unconstrained = after_close_bracket + 1 < self.input.len()
            && bytes[after_close_bracket + 1] == marker;

        if is_unconstrained {
            let content_start = after_close_bracket + 2;
            if content_start >= self.input.len() {
                return false;
            }
            let close_pos = match self.find_closing_unconstrained(marker, content_start) {
                Some(p) => p,
                None => return false,
            };
            let inner = &self.input[content_start..close_pos];
            if inner.is_empty() {
                return false;
            }

            self.flush_text(*text_start, start_pos, events);
            events.push(Event::Start(start_tag));

            let mut inner_parser = InlineState::new(inner, inner_subs, self.options);
            inner_parser.parse_inline(events);

            events.push(Event::End(end_tag));

            self.pos = close_pos + 2;
            *text_start = self.pos;
            return true;
        }

        // Constrained: [.class]#text# / [.class]_text_ etc. Unlike the unconstrained
        // form (which may appear mid-word), the constrained `[attrlist]` prefix is
        // bound by the same opening boundary as the bare quote: the character before
        // `[` must not be a word character (`word[role]#x#` stays literal — verified
        // vs Asciidoctor; `hel[x]##lo##` mid-word DOES match as it is unconstrained).
        if self.is_word_char_before(start_pos) {
            return false;
        }

        let content_start = after_close_bracket + 1;
        if content_start >= self.input.len() {
            return false;
        }
        if bytes[content_start] == b' ' {
            return false;
        }

        if let Some(close_offset) = self.find_closing_constrained(marker, content_start) {
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
            events.push(Event::Start(start_tag));

            let mut inner_parser = InlineState::new(inner, inner_subs, self.options);
            inner_parser.parse_inline(events);

            events.push(Event::End(end_tag));

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

        let mut inner_parser = InlineState::new(inner, self.subs, self.options);
        inner_parser.smart_quote_leading_edge = true;
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

    /// Parse with the experimental UI macros (`kbd:`/`btn:`/`menu:`) enabled,
    /// as if `:experimental:` were set.
    fn parse_experimental(text: &str) -> Vec<Event<'_>> {
        InlineParser::parse_str_with_subs_options(
            text,
            SubstitutionSet::NORMAL,
            InlineOptions { experimental: true },
        )
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
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
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
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
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
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
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
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
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
    fn test_cross_reference_with_hash_prefix() {
        let events = parse("see <<#my-section>>");
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
    fn test_cross_reference_with_hash_prefix_and_label() {
        let events = parse("see <<#my-section, My Section>>");
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
                is_bare: false,
                role: None,
            }),
            Event::Text(Cow::Borrowed("here")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_link_role_mailto_query_irc_scheme() {
        // role= named attr is carried on Tag::Link.
        assert_eq!(parse("https://x.org[text,role=green]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://x.org"),
                window: None,
                nofollow: false,
                is_bare: false,
                role: Some(Cow::Borrowed("green")),
            }),
            Event::Text(Cow::Borrowed("text")),
            Event::End(TagEnd::Link),
        ]);
        // Named-only attrlist → empty text → bare (text falls back to the URL).
        assert_eq!(parse("https://x.org[role=green]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://x.org"),
                window: None,
                nofollow: false,
                is_bare: true,
                role: Some(Cow::Borrowed("green")),
            }),
            Event::Text(Cow::Borrowed("https://x.org")),
            Event::End(TagEnd::Link),
        ]);
        // mailto positional 2/3 → ?subject=&body=, percent-encoded (%20 for
        // space, %21 for '!'); quoted values lose their quotes.
        assert_eq!(parse("mailto:join@x.org[Subscribe,Subscribe me,I want to join!]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("mailto:join@x.org?subject=Subscribe%20me&body=I%20want%20to%20join%21"),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text(Cow::Borrowed("Subscribe")),
            Event::End(TagEnd::Link),
        ]);
        assert_eq!(parse("mailto:a@b.c[T,\"comma, inside\"]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("mailto:a@b.c?subject=comma%2C%20inside"),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text(Cow::Borrowed("T")),
            Event::End(TagEnd::Link),
        ]);
        // irc:// is an autolink scheme like http(s)/ftp.
        assert_eq!(parse("see irc://irc.x.org/#chan now"), vec![
            Event::Text(Cow::Borrowed("see ")),
            Event::Start(Tag::Link {
                url: Cow::Borrowed("irc://irc.x.org/#chan"),
                window: None,
                nofollow: false,
                is_bare: true,
                role: None,
            }),
            Event::Text(Cow::Borrowed("irc://irc.x.org/#chan")),
            Event::End(TagEnd::Link),
            Event::Text(Cow::Borrowed(" now")),
        ]);
    }

    #[test]
    fn test_macro_label_replacements() {
        // Asciidoctor runs REPLACEMENTS before macros, so an apostrophe inside an
        // explicit link/xref/mailto label is curled in the visible text.
        // link: macro
        assert_eq!(parse("link:u.html[cell's separator]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("u.html"),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text(Cow::Borrowed("cell\u{2019}s separator")),
            Event::End(TagEnd::Link),
        ]);
        // xref:target[label]
        assert_eq!(parse("xref:t.adoc[attribute's value]"), vec![
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("t.adoc"),
                label: Some(Cow::Borrowed("attribute's value")),
            }),
            Event::Text(Cow::Borrowed("attribute\u{2019}s value")),
            Event::End(TagEnd::CrossReference),
        ]);
        // <<id,label>> form
        assert_eq!(parse("<<sec,group's charter>>"), vec![
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("sec"),
                label: Some(Cow::Borrowed("group's charter")),
            }),
            Event::Text(Cow::Borrowed("group\u{2019}s charter")),
            Event::End(TagEnd::CrossReference),
        ]);
        // Bare URL display (no explicit text) is emitted raw — apostrophes in the
        // URL itself are not curled. Empty text → the link is "bare".
        assert_eq!(parse("link:a'b.html[]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("a'b.html"),
                window: None,
                nofollow: false,
                is_bare: true,
                role: None,
            }),
            Event::Text(Cow::Borrowed("a'b.html")),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn test_macro_label_quotes_formatting() {
        // Asciidoctor runs QUOTES before macros, so formatting spans inside an
        // explicit link/xref/mailto label are already converted when the macro
        // is rendered: xref:t.adoc[see `code`] → <a …>see <code>code</code></a>.
        assert_eq!(parse("xref:t.adoc[see `partnums`]"), vec![
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("t.adoc"),
                label: Some(Cow::Borrowed("see `partnums`")),
            }),
            Event::Text(Cow::Borrowed("see ")),
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("partnums")),
            Event::End(TagEnd::Monospace),
            Event::End(TagEnd::CrossReference),
        ]);
        // link: macro with bold + italic in the label.
        assert_eq!(parse("link:u.html[*b* and _i_]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("u.html"),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("b")),
            Event::End(TagEnd::Strong),
            Event::Text(Cow::Borrowed(" and ")),
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("i")),
            Event::End(TagEnd::Emphasis),
            Event::End(TagEnd::Link),
        ]);
        // <<id,label>> shorthand form.
        assert_eq!(parse("<<sec,`mono` label>>"), vec![
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("sec"),
                label: Some(Cow::Borrowed("`mono` label")),
            }),
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("mono")),
            Event::End(TagEnd::Monospace),
            Event::Text(Cow::Borrowed(" label")),
            Event::End(TagEnd::CrossReference),
        ]);
        // ATTRIBUTES also precede macros: `{attr}` in a label is a reference.
        assert_eq!(parse("xref:t.adoc[with {myattr} ref]"), vec![
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("t.adoc"),
                label: Some(Cow::Borrowed("with {myattr} ref")),
            }),
            Event::Text(Cow::Borrowed("with ")),
            Event::AttributeReference {
                name: Cow::Borrowed("myattr"),
                fallback: None,
                trailing_brackets: None,
            },
            Event::Text(Cow::Borrowed(" ref")),
            Event::End(TagEnd::CrossReference),
        ]);
        // A macro consumed into the label is NOT scanned again (MACROS disabled
        // in the label re-parse) — the inner macro text stays literal.
        assert_eq!(parse("xref:t.adoc[see <<other>>]"), vec![
            Event::Start(Tag::CrossReference {
                target: Cow::Borrowed("t.adoc"),
                label: Some(Cow::Borrowed("see <<other>>")),
            }),
            Event::Text(Cow::Borrowed("see <<other>>")),
            Event::End(TagEnd::CrossReference),
        ]);
    }

    #[test]
    fn test_link_macro_empty_text_is_bare() {
        // link: macro with no explicit text → bare (class="bare"), text = target.
        assert_eq!(parse("link:LICENSE[]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("LICENSE"),
                window: None,
                nofollow: false,
                is_bare: true,
                role: None,
            }),
            Event::Text(Cow::Borrowed("LICENSE")),
            Event::End(TagEnd::Link),
        ]);
        // Explicit text → not bare, even if it equals the target.
        assert_eq!(parse("link:LICENSE[LICENSE]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("LICENSE"),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text(Cow::Borrowed("LICENSE")),
            Event::End(TagEnd::Link),
        ]);
        // Bare URL with empty bracket text → bare.
        assert_eq!(parse("https://example.org[]"), vec![
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://example.org"),
                window: None,
                nofollow: false,
                is_bare: true,
                role: None,
            }),
            Event::Text(Cow::Borrowed("https://example.org")),
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
                link: None,
                role: None,
                title: None,
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
                trailing_brackets: None,
            },
        ]);
    }

    #[test]
    fn test_attribute_reference_bang_is_literal() {
        // Asciidoctor's reference name is `\w[\w-]*`: any `!` inside the braces
        // means "not a reference" — the text stays literal (probe-verified),
        // there is no fallback syntax.
        let events = parse("{name!default value}");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("{name!default value}"))]);

        let events = parse("{name!}");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("{name!}"))]);
    }

    #[test]
    fn test_attribute_reference_captures_trailing_brackets() {
        // `{attr}[text]` — the trailing `[...]` is captured so the renderer can
        // re-parse `value[text]` as a link macro (attributes substitute before
        // macros). Brackets must be immediately adjacent (no space).
        let events = parse("see {url}[the page^] end");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("see ")),
            Event::AttributeReference {
                name: Cow::Borrowed("url"),
                fallback: None,
                trailing_brackets: Some(Cow::Borrowed("[the page^]")),
            },
            Event::Text(Cow::Borrowed(" end")),
        ]);

        // A double bracket (inline anchor) is NOT captured as link text.
        let events = parse("{url}[[id]]");
        assert!(matches!(
            events[0],
            Event::AttributeReference { trailing_brackets: None, .. }
        ), "{events:?}");

        // A space before the bracket means it is not adjacent → not captured.
        let events = parse("{url} [text]");
        assert!(matches!(
            events[0],
            Event::AttributeReference { trailing_brackets: None, .. }
        ), "{events:?}");

        // No closing bracket → not captured.
        let events = parse("{url}[no close");
        assert!(matches!(
            events[0],
            Event::AttributeReference { trailing_brackets: None, .. }
        ), "{events:?}");
    }

    #[test]
    fn test_attribute_reference_captures_path_before_brackets() {
        // `{url}/issues[text]` — Asciidoctor expands the attribute, then re-parses
        // `value/issues[text]` as a URL macro. The path between `}` and `[` is
        // captured together with the brackets.
        let events = parse("see {url}/issues[the tracker] end");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("see ")),
            Event::AttributeReference {
                name: Cow::Borrowed("url"),
                fallback: None,
                trailing_brackets: Some(Cow::Borrowed("/issues[the tracker]")),
            },
            Event::Text(Cow::Borrowed(" end")),
        ]);

        // A space inside the path stops capture before the bracket → not captured.
        let events = parse("{url}/a b[text]");
        assert!(matches!(
            events[0],
            Event::AttributeReference { trailing_brackets: None, .. }
        ), "{events:?}");

        // A path with no following bracket → not captured (path stays literal).
        let events = parse("{url}/issues done");
        assert!(matches!(
            events[0],
            Event::AttributeReference { trailing_brackets: None, .. }
        ), "{events:?}");
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
                is_bare: true,
                role: None,
            }),
            Event::Text(Cow::Borrowed("https://example.com")),
            Event::End(TagEnd::Link),
            Event::Text(Cow::Borrowed(" for info")),
        ]);
    }

    #[test]
    fn test_autolink_with_link_text() {
        let events = parse(
            "Обратитесь к https://tools.ietf.org/html/rfc7231#section-6[HTTP response code spec]",
        );
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("Обратитесь к ")),
            Event::Start(Tag::Link {
                url: Cow::Borrowed("https://tools.ietf.org/html/rfc7231#section-6"),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text(Cow::Borrowed("HTTP response code spec")),
            Event::End(TagEnd::Link),
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
            Event::Start(Tag::Anchor { id: Cow::Borrowed("my-anchor"), label: None }),
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
    fn test_escaped_marker_no_span_keeps_backslash() {
        // No closing marker of the same kind ahead → the constrained span can't form,
        // so Asciidoctor keeps the leading backslash literal (its quote regexps strip
        // `\` only when the construct matches). Contrast test_escaped_bold/italic/
        // monospace above, where a closing marker exists and the backslash is dropped.
        assert_eq!(parse("\\* is an asterisk"), vec![
            Event::Text(Cow::Borrowed("\\*")),
            Event::Text(Cow::Borrowed(" is an asterisk")),
        ]);
        assert_eq!(parse("an \\_lone underscore"), vec![
            Event::Text(Cow::Borrowed("an ")),
            Event::Text(Cow::Borrowed("\\_")),
            Event::Text(Cow::Borrowed("lone underscore")),
        ]);
        assert_eq!(parse("a \\`lone tick"), vec![
            Event::Text(Cow::Borrowed("a ")),
            Event::Text(Cow::Borrowed("\\`")),
            Event::Text(Cow::Borrowed("lone tick")),
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
    fn test_escaped_char_reference() {
        // \&#NNN; / \&#xHH; / \&name; → backslash dropped, reference emitted as literal text
        // (the `&` is later escaped to &amp; by the renderer). The whole reference is one span,
        // so the inner `#` is never taken as mark syntax.
        for (input, want) in [
            ("a \\&#174; b", "&#174;"),
            ("a \\&#x1F600; b", "&#x1F600;"),
            ("a \\&copy; b", "&copy;"),
        ] {
            let events = parse(input);
            assert_eq!(events, vec![
                Event::Text(Cow::Borrowed("a ")),
                Event::Text(Cow::Borrowed(want)),
                Event::Text(Cow::Borrowed(" b")),
            ], "input: {input:?}");
        }
    }

    #[test]
    fn test_escaped_inline_macro() {
        // A backslash before a recognized inline macro drops the backslash and emits the macro
        // form as a single literal Text span (no macro processing), matching Asciidoctor.
        for (input, want) in [
            ("a \\indexterm2:[primary] b", "indexterm2:[primary]"),
            ("a \\indexterm:[x, y] b", "indexterm:[x, y]"),
            ("a \\footnote:[note] b", "footnote:[note]"),
            ("a \\image:p.png[alt] b", "image:p.png[alt]"),
            ("a \\link:u.html[t] b", "link:u.html[t]"),
            ("a \\xref:tgt[lbl] b", "xref:tgt[lbl]"),
        ] {
            let events = parse(input);
            assert_eq!(events, vec![
                Event::Text(Cow::Borrowed("a ")),
                Event::Text(Cow::Borrowed(want)),
                Event::Text(Cow::Borrowed(" b")),
            ], "input: {input:?}");
        }
    }

    #[test]
    fn test_backslash_before_unrecognized_macro_kept() {
        // The backslash is preserved for the experimental kbd/btn/menu macros and the block form
        // image:: — Asciidoctor does not treat those as escapable inline macros, so the escape
        // must not fire (the backslash survives in the text run).
        for input in ["a \\kbd:[Ctrl] b", "a \\btn:[OK] b", "a \\menu:File[New] b", "a \\image::t[] b"] {
            let events = parse(input);
            assert!(
                events.iter().any(|e| matches!(e, Event::Text(t) if t.contains('\\'))),
                "backslash should be preserved for {input:?}, got {events:?}"
            );
        }
    }

    #[test]
    fn test_backslash_before_invalid_char_reference_kept() {
        // Backslash is kept when what follows is NOT a valid character reference, matching
        // Asciidoctor: bare `&`, missing `;`, too-few/too-many digits, single-letter name.
        for input in ["a \\& b", "a \\&# b", "a \\&#9; b", "a \\&a; b", "a \\&notanentity b"] {
            let events = parse(input);
            assert!(
                events.iter().any(|e| matches!(e, Event::Text(t) if t.contains('\\'))),
                "backslash should be preserved for {input:?}, got {events:?}"
            );
        }
    }

    #[test]
    fn test_bare_char_reference_preserved() {
        // A valid bare char-ref (no backslash) is preserved as a raw entity (InlinePassthrough),
        // so the renderer does not escape its `&` — matching Asciidoctor's specialchars sub.
        for (input, want) in [
            ("a &#167; b", "&#167;"),
            ("a &copy; b", "&copy;"),
            ("a &amp; b", "&amp;"),
            ("a &#x1F600; b", "&#x1F600;"),
        ] {
            let events = parse(input);
            assert_eq!(events, vec![
                Event::Text(Cow::Borrowed("a ")),
                Event::InlinePassthrough(Cow::Borrowed(want)),
                Event::Text(Cow::Borrowed(" b")),
            ], "input: {input:?}");
        }
    }

    #[test]
    fn test_bare_invalid_char_reference_not_preserved() {
        // An invalid char-ref (too-few digits, single-letter name, missing `;`, bare `&`) is left
        // as plain text — the renderer escapes its `&` to `&amp;`. No InlinePassthrough is emitted.
        for input in ["a &#1; b", "a &x; b", "a &#167 b", "a & b", "a &#xZ; b"] {
            let events = parse(input);
            assert!(
                !events.iter().any(|e| matches!(e, Event::InlinePassthrough(_))),
                "no passthrough should be emitted for {input:?}, got {events:?}"
            );
        }
    }

    #[test]
    fn test_nested_formatting() {
        let events = parse("*bold _and italic_*");
        assert_eq!(events, vec![
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("bold ")),
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("and italic")),
            Event::End(TagEnd::Emphasis),
            Event::End(TagEnd::Strong),
        ]);
    }

    // Typographic replacement tests

    #[test]
    fn test_typographic_em_dash() {
        // Asciidoctor has no `---` rule: neither `(\w)--(?=\w)` nor the spaced
        // form can match inside a longer dash run — the text stays literal.
        let events = parse("hello---world");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("hello---world"))]);
        let events = parse("g --- h");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("g --- h"))]);
        let events = parse("e----f");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("e----f"))]);
        let events = parse("---- <.>\nplain");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("---- <.>\nplain"))]);
    }

    #[test]
    fn test_typographic_bare_em_dash() {
        // Asciidoctor `(\w)--(?=\w)` → em-dash followed by a zero-width space.
        let events = parse("hello--world");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("hello\u{2014}\u{200B}world".to_string())),
        ]);
    }

    #[test]
    fn test_typographic_dash_not_between_words() {
        // ` --flag` (space before, letter after) is not `\w--\w`: keep `--`.
        let events = parse("run --dir=x");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("run --dir=x"))]);
    }

    #[test]
    fn test_typographic_trailing_dashes() {
        // Trailing `--` (no following word char) stays literal.
        let events = parse("For S.S.T.--");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("For S.S.T.--"))]);
    }

    #[test]
    fn test_typographic_ellipsis() {
        let events = parse("wait...");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("wait\u{2026}\u{200B}".to_string())),
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
        let result = apply_typographic_replacements("hello world", true, true);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_typographic_mixed() {
        // `---` stays literal (no Asciidoctor rule); (C) and ellipsis are replaced.
        let events = parse("(C) 2024---all rights...");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("\u{00A9} 2024---all rights\u{2026}\u{200B}".to_string())),
        ]);
    }

    #[test]
    fn test_typographic_spaced_em_dash() {
        let events = parse("hello -- world");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("hello\u{2009}\u{2014}\u{2009}world".to_string())),
        ]);
        // Line boundaries count as flanks and are consumed — adjacent lines merge
        // (Asciidoctor `(^|\n| |\\)--( |\n|$)` replaces the `\n` with thin space).
        let events = parse("a\n-- b");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("a\u{2009}\u{2014}\u{2009}b".to_string())),
        ]);
        let events = parse("c --\nd");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("c\u{2009}\u{2014}\u{2009}d".to_string())),
        ]);
        // Text start/end are boundaries too.
        let events = parse("-- lead");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("\u{2009}\u{2014}\u{2009}lead".to_string())),
        ]);
        let events = parse("tail --");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("tail\u{2009}\u{2014}\u{2009}".to_string())),
        ]);
        // gsub semantics: a flank consumed by the previous match doesn't count.
        let events = parse("a -- -- b");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("a\u{2009}\u{2014}\u{2009}-- b".to_string())),
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
        // `\---` is not an escape: no replacement rule matches `---`, so the
        // backslash itself stays literal (Asciidoctor keeps it).
        let events = parse("hello \\--- world");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("hello \\--- world")),
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
    fn test_pass_macro_subs_spec_events() {
        // pass:c[…] — specialchars kept: plain Text (the renderer escapes it).
        let events = parse("pass:c[<b>]");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("<b>"))]);

        // pass:q[*b*] — no specialchars: text runs are raw passthroughs.
        let events = parse("pass:q[*b*]");
        assert_eq!(events, vec![
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
            Event::InlinePassthrough(Cow::Borrowed("b")),
            Event::End(TagEnd::Strong),
        ]);

        // Bare pass:[…] unchanged — raw verbatim insertion.
        let events = parse("pass:[<b>]");
        assert_eq!(events, vec![Event::InlinePassthrough(Cow::Borrowed("<b>"))]);

        // No bracket after the spec — the macro form does not match at all.
        let events = parse("pass:c here");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("pass:c here"))]);
    }

    #[test]
    fn test_single_plus_no_typographic() {
        let events = parse("+(C) 2024+");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("(C) 2024")),
        ]);
    }

    #[test]
    fn test_monospace_applies_replacements() {
        // Constrained monospace `` `text` `` undergoes the full normal substitution
        // group like prose: `(C)` becomes ©, a word-flanked `--` an em-dash, and a
        // valid char-ref is restored (Asciidoctor: <code>&#169; x&#8212;&#8203;y &#167;</code>).
        let events = parse("`(C) x--y &#167;`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("\u{00A9} x\u{2014}\u{200B}y ")),
            Event::InlinePassthrough(Cow::Borrowed("&#167;")),
            Event::End(TagEnd::Monospace),
        ]);
    }

    #[test]
    fn test_monospace_edge_em_dash_stays_literal() {
        // A spaced em-dash needs a `^`/`$`/space boundary on each side. Asciidoctor runs
        // replacements after wrapping in `<code>`, so `--` at the span edge is bounded by
        // the tag chars, not a line edge → it stays literal (`` `--` `` → <code>--</code>).
        for input in ["`--`", "`--x`", "`x--`"] {
            let events = parse(input);
            let text: String = events.iter().filter_map(|e| match e {
                Event::Text(t) => Some(t.as_ref()),
                _ => None,
            }).collect();
            assert!(!text.contains('\u{2014}'), "{input:?} should keep `--` literal, got {events:?}");
        }
        // But a true word-flanked `--` inside the span is still replaced.
        let events = parse("`x--y`");
        let text: String = events.iter().filter_map(|e| match e {
            Event::Text(t) => Some(t.to_string()),
            _ => None,
        }).collect();
        assert!(text.contains('\u{2014}'), "x--y inside monospace should form an em-dash, got {events:?}");
    }

    #[test]
    fn test_monospace_edge_trailing_space_plus_stays_literal() {
        // A hard break is ` +` at a true line edge. Asciidoctor applies the line-break
        // replacement after spans are rendered, so a trailing ` +` inside a span is bounded
        // by the closing `</code>`, not by `$` → it stays literal, never a `<br>`
        // (`` `x +` `` → <code>x +</code>, `` ` + +` `` → <code> + +</code>).
        for input in ["`x +`", "` + +`", "`+ +`"] {
            let events = parse(input);
            assert!(
                !events.iter().any(|e| matches!(e, Event::HardBreak)),
                "{input:?} should not produce a hard break, got {events:?}"
            );
        }
        // But ` +` at a true line/paragraph edge (top-level input) is still a hard break.
        assert!(
            parse("line one +").iter().any(|e| matches!(e, Event::HardBreak)),
            "trailing ` +` at a line edge should be a hard break"
        );
        // And ` +\n` mid-content (a real newline) stays a hard break even inside a span.
        assert!(
            parse("`a +\nb`").iter().any(|e| matches!(e, Event::HardBreak)),
            "` +` before a newline should be a hard break even inside monospace"
        );
    }

    #[test]
    fn test_single_plus_passthrough_constrained() {
        // Constrained close: an inner '+' followed by a word char cannot close, so
        // the scan continues to the trailing '+'. `+a+b+` → content "a+b".
        let events = parse("+a+b+");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("a+b"))]);

        // Inner '+' surrounded by spaces stays literal; close is the trailing '+'.
        let events = parse("+a + b+");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("a + b"))]);

        // Opening '+' after a word char does not start a passthrough (whole literal).
        let events = parse("C+a+b+");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("C+a+b+"))]);

        // No valid (non-word-followed) close → the leading '+' stays literal.
        let events = parse("+a+b");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("+a+b"))]);

        // Content may not start with a space → literal.
        let events = parse("+ a+");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("+ a+"))]);
    }

    #[test]
    fn test_monospace_passthrough_inner_plus() {
        // `+...+` literal-monospace: the inner '+' in kbd:[key(+key)*] must survive
        // (Asciidoctor renders <code>kbd:[key(+key)*]</code>).
        let events = parse("`+kbd:[key(+key)*]+`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("kbd:[key(+key)*]")),
            Event::End(TagEnd::Monospace),
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

    #[test]
    fn test_passthrough_inside_monospace() {
        // `++`++` — the inner backtick lives in a ++…++ passthrough and must not
        // close the monospace span early. Asciidoctor renders <code>`</code>.
        // Double-plus applies specialchars only and emits `Text` (backtick is not a
        // special char, so it survives verbatim once html-escaped).
        let events = parse("(`++`++`)");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("(")),
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("`")),
            Event::End(TagEnd::Monospace),
            Event::Text(Cow::Borrowed(")")),
        ]);

        // ++b++ inside monospace → passthrough yields literal "b" (as `Text`).
        let events = parse("`++b++`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("b")),
            Event::End(TagEnd::Monospace),
        ]);

        // A lone, unclosed ++ stays literal; the monospace span matches normally.
        let events = parse("`x ++ y`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("x ++ y")),
            Event::End(TagEnd::Monospace),
        ]);
    }

    #[test]
    fn test_pass_macro_inside_monospace() {
        // `pass:[`']` — the backtick inside the pass macro bracket must not close
        // the monospace span; the macro yields its raw content. Asciidoctor:
        // <code>`'</code>.
        let events = parse("`pass:[`']`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::InlinePassthrough(Cow::Borrowed("`'")),
            Event::End(TagEnd::Monospace),
        ]);

        // pass:[…] with no inner marker is unaffected: the closing backtick is
        // found at the end as before.
        let events = parse("`pass:[++++]`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::InlinePassthrough(Cow::Borrowed("++++")),
            Event::End(TagEnd::Monospace),
        ]);
    }

    #[test]
    fn test_single_plus_passthrough_spans_backtick() {
        // A single-plus passthrough is extracted before monospace and may swallow
        // backticks: in `` `a +`b`+ c` `` the inner `+`b`+` is a passthrough whose
        // backticks are literal, so the outer monospace runs from the first backtick
        // to the last (Asciidoctor: <code>a `b` c</code>), not two separate spans.
        let events = parse("`a +`b`+ c`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("a ")),
            Event::Text(Cow::Borrowed("`b`")),
            Event::Text(Cow::Borrowed(" c")),
            Event::End(TagEnd::Monospace),
        ]);
    }

    #[test]
    fn test_escaped_plus_does_not_span_backtick() {
        // An escaped `\+` is NOT a passthrough open, so it must not swallow the
        // backticks of a following monospace span. `` `\+` and `n+` `` stays two
        // separate <code> spans (Asciidoctor: <code>+</code> and <code>n+</code>).
        let events = parse("`\\+` and `n+`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("+")),
            Event::End(TagEnd::Monospace),
            Event::Text(Cow::Borrowed(" and ")),
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("n+")),
            Event::End(TagEnd::Monospace),
        ]);
    }

    #[test]
    fn test_pass_macro_inside_single_plus() {
        // pass:[…] is extracted BEFORE the single-plus span (asciidoctor
        // substitution order), so `+pass:[x]+` yields the raw macro content.
        let events = parse("+pass:[x]+");
        assert_eq!(events, vec![Event::InlinePassthrough(Cow::Borrowed("x"))]);

        // Empty macro: `+pass:[]+` (typically inside monospace: `` `+pass:[]+` ``)
        // yields empty content — asciidoctor renders <code></code>.
        let events = parse("`+pass:[]+`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::InlinePassthrough(Cow::Borrowed("")),
            Event::End(TagEnd::Monospace),
        ]);

        // Discriminator: `+pass:[]+more+` — the '+' after the macro cannot close
        // (followed by a word char), the span runs to the last '+'; the macro is
        // resolved within. Asciidoctor: <code>+more</code>.
        let events = parse("`+pass:[]+more+`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::InlinePassthrough(Cow::Borrowed("")),
            Event::Text(Cow::Borrowed("+more")),
            Event::End(TagEnd::Monospace),
        ]);

        // Mixed literal + macro content.
        let events = parse("+a pass:[b] c+");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("a ")),
            Event::InlinePassthrough(Cow::Borrowed("b")),
            Event::Text(Cow::Borrowed(" c")),
        ]);

        // Inside `++…++` the macro is NOT extracted (the double-plus span wins
        // positionally in the same extraction pass) — content stays verbatim, emitted
        // as `Text` (double-plus applies specialchars only; `pass:[y]` has none).
        let events = parse("`++pass:[y]++`");
        assert_eq!(events, vec![
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("pass:[y]")),
            Event::End(TagEnd::Monospace),
        ]);
    }

    #[test]
    fn test_double_plus_passthrough_applies_specialchars() {
        // Double-plus `++…++` applies ONLY the specialcharacters sub: it emits `Text`
        // (which the renderer html-escapes `<`/`>`/`&`), NOT `InlinePassthrough` (raw).
        // Asciidoctor: `++[<LABEL>]++` → `[&lt;LABEL&gt;]`, `++a & b++` → `a &amp; b`.
        assert_eq!(parse("++[<LABEL>]++"), vec![Event::Text(Cow::Borrowed("[<LABEL>]"))]);
        assert_eq!(parse("++a & b++"), vec![Event::Text(Cow::Borrowed("a & b"))]);

        // No quotes/replacements/attributes subs run inside double-plus.
        assert_eq!(parse("++*x*++"), vec![Event::Text(Cow::Borrowed("*x*"))]);
        assert_eq!(parse("++{foo}++"), vec![Event::Text(Cow::Borrowed("{foo}"))]);

        // Triple-plus stays raw (`InlinePassthrough` — no specialchars escaping).
        assert_eq!(
            parse("+++[<LABEL>]+++"),
            vec![Event::InlinePassthrough(Cow::Borrowed("[<LABEL>]"))]
        );
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
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("bold")),
            Event::End(TagEnd::Strong),
            Event::Text(Cow::Borrowed(" text")),
            Event::Text(Cow::Borrowed("\u{201D}")),
        ]);
    }

    #[test]
    fn test_smart_quotes_double_backtick_inner_literal() {
        // `"``end points``"` → curved quotes with the inner single backticks left
        // *literal*: constrained monospace cannot open at the smart-quote leading edge
        // (it runs after the `:double` sub, seeing the `;` of `&#8220;`). Asciidoctor:
        // `&#8220;`end points`&#8221;`. A *triple* pair would leave `` ``end points`` ``,
        // which is unconstrained monospace and DOES become `<code>`.
        let events = parse("\"``end points``\"");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\u{201C}")),
            Event::Text(Cow::Borrowed("`end points`")),
            Event::Text(Cow::Borrowed("\u{201D}")),
        ]);
    }

    #[test]
    fn test_smart_quotes_edge_emphasis_mark_literal() {
        // Constrained emphasis (`_`) and mark (`#`) are also suppressed at the leading
        // edge, for the same substitution-order reason. Strong (`*`) is NOT — it runs
        // before `:double` (covered by `test_smart_quotes_with_formatting`).
        assert_eq!(parse("\"`_em_ x`\""), vec![
            Event::Text(Cow::Borrowed("\u{201C}")),
            Event::Text(Cow::Borrowed("_em_ x")),
            Event::Text(Cow::Borrowed("\u{201D}")),
        ]);
        assert_eq!(parse("\"`#mk# x`\""), vec![
            Event::Text(Cow::Borrowed("\u{201C}")),
            Event::Text(Cow::Borrowed("#mk# x")),
            Event::Text(Cow::Borrowed("\u{201D}")),
        ]);
    }

    #[test]
    fn test_smart_quotes_edge_suppression_is_leading_only() {
        // The suppression is positional (leading edge only): a constrained monospace
        // span later in the inner content, preceded by a real space, still opens.
        let events = parse("\"`a `c` b`\"");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\u{201C}")),
            Event::Text(Cow::Borrowed("a ")),
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("c")),
            Event::End(TagEnd::Monospace),
            Event::Text(Cow::Borrowed(" b")),
            Event::Text(Cow::Borrowed("\u{201D}")),
        ]);
    }

    #[test]
    fn test_emphasis_leading_edge_suppresses_strong_and_mono() {
        // At an emphasis span's leading edge, constrained strong (`*`) and monospace
        // (`` ` ``) stay literal: both run before emphasis in QUOTE_SUBS and still see
        // the literal `_` (a word char) their open assertion forbids.
        // `_`inline` text_` → `<em>`inline` text</em>` (asciidoc-lang
        // document-attributes-ref.adoc line 1216).
        assert_eq!(parse("_`inline` text_"), vec![
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("`inline` text")),
            Event::End(TagEnd::Emphasis),
        ]);
        assert_eq!(parse("_*bold* x_"), vec![
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("*bold* x")),
            Event::End(TagEnd::Emphasis),
        ]);
        // Unconstrained emphasis `__…__` suppresses the same way.
        assert_eq!(parse("__`inline` x__"), vec![
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("`inline` x")),
            Event::End(TagEnd::Emphasis),
        ]);
    }

    #[test]
    fn test_emphasis_leading_edge_does_not_suppress_mark_or_unconstrained() {
        // Mark (`#`) runs *after* emphasis (it sees the emitted `<em>`'s `>`), so it
        // opens at the leading edge; unconstrained monospace (``) has no open assertion.
        assert_eq!(parse("_#mark# x_"), vec![
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
            Event::Start(Tag::Highlight),
            Event::Text(Cow::Borrowed("mark")),
            Event::End(TagEnd::Highlight),
            Event::Text(Cow::Borrowed(" x")),
            Event::End(TagEnd::Emphasis),
        ]);
        assert_eq!(parse("_``code`` x_"), vec![
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("code")),
            Event::End(TagEnd::Monospace),
            Event::Text(Cow::Borrowed(" x")),
            Event::End(TagEnd::Emphasis),
        ]);
    }

    #[test]
    fn test_emphasis_leading_edge_suppression_is_leading_only() {
        // Positional: a constrained monospace later in the emphasis content, preceded by
        // a real space, still opens. Strong/mono after `*`/`#` outer markers also open
        // (those markers are not word characters), proving the gate is emphasis-specific.
        assert_eq!(parse("_x `c` y_"), vec![
            Event::Start(Tag::Emphasis { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("x ")),
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("c")),
            Event::End(TagEnd::Monospace),
            Event::Text(Cow::Borrowed(" y")),
            Event::End(TagEnd::Emphasis),
        ]);
        assert_eq!(parse("*`code` x*"), vec![
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
            Event::Start(Tag::Monospace { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("code")),
            Event::End(TagEnd::Monospace),
            Event::Text(Cow::Borrowed(" x")),
            Event::End(TagEnd::Strong),
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
        let events = parse_experimental("kbd:[Ctrl+C]");
        assert_eq!(events, vec![
            Event::Start(Tag::Keyboard),
            Event::Text(Cow::Borrowed("Ctrl+C")),
            Event::End(TagEnd::Keyboard),
        ]);
    }

    #[test]
    fn test_kbd_single_key() {
        let events = parse_experimental("kbd:[F11]");
        assert_eq!(events, vec![
            Event::Start(Tag::Keyboard),
            Event::Text(Cow::Borrowed("F11")),
            Event::End(TagEnd::Keyboard),
        ]);
    }

    #[test]
    fn test_kbd_in_sentence() {
        let events = parse_experimental("Press kbd:[Ctrl+C] to copy");
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
        let events = parse_experimental("btn:[OK]");
        assert_eq!(events, vec![
            Event::Start(Tag::Button),
            Event::Text(Cow::Borrowed("OK")),
            Event::End(TagEnd::Button),
        ]);
    }

    #[test]
    fn test_menu_macro() {
        let events = parse_experimental("menu:File[Save As]");
        assert_eq!(events, vec![
            Event::Start(Tag::Menu { target: Cow::Borrowed("File") }),
            Event::Text(Cow::Borrowed("Save As")),
            Event::End(TagEnd::Menu),
        ]);
    }

    #[test]
    fn test_menu_no_items() {
        let events = parse_experimental("menu:File[]");
        assert_eq!(events, vec![
            Event::Start(Tag::Menu { target: Cow::Borrowed("File") }),
            Event::End(TagEnd::Menu),
        ]);
    }

    #[test]
    fn test_menu_with_submenus() {
        let events = parse_experimental("menu:File[New > Document]");
        assert_eq!(events, vec![
            Event::Start(Tag::Menu { target: Cow::Borrowed("File") }),
            Event::Text(Cow::Borrowed("New > Document")),
            Event::End(TagEnd::Menu),
        ]);
    }

    #[test]
    fn test_experimental_macros_literal_without_experimental() {
        // Without :experimental:, kbd:/btn:/menu: are left as literal text
        // (Asciidoctor's default). The whole token must survive verbatim.
        assert_eq!(
            parse("Press kbd:[Ctrl+C] now"),
            vec![Event::Text(Cow::Borrowed("Press kbd:[Ctrl+C] now"))]
        );
        assert_eq!(
            parse("Click btn:[OK]."),
            vec![Event::Text(Cow::Borrowed("Click btn:[OK]."))]
        );
        assert_eq!(
            parse("Select menu:File[Save]!"),
            vec![Event::Text(Cow::Borrowed("Select menu:File[Save]!"))]
        );
        // Lowercase target/content must not be rescanned as another macro form.
        assert_eq!(
            parse("menu:file[save]"),
            vec![Event::Text(Cow::Borrowed("menu:file[save]"))]
        );
    }

    #[test]
    fn test_inline_options_channel() {
        // Streaming path: set / both unset spellings, unrelated attrs ignored.
        let mut opts = InlineOptions::default();
        assert!(!opts.experimental);
        opts.apply_attribute("experimental");
        assert!(opts.experimental);
        opts.apply_attribute("!experimental");
        assert!(!opts.experimental);
        opts.apply_attribute("experimental");
        opts.apply_attribute("experimental!");
        assert!(!opts.experimental);
        opts.apply_attribute("toc");
        opts.apply_attribute("!sectnums");
        assert_eq!(opts, InlineOptions::default());

        // Snapshot path mirrors the streaming result.
        assert_eq!(
            InlineOptions::from_attr_lookup(|name| name == "experimental"),
            InlineOptions { experimental: true }
        );
        assert_eq!(InlineOptions::from_attr_lookup(|_| false), InlineOptions::default());
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
    fn test_stem_macro_escaped_brackets() {
        // `\]` does not close the macro and is unescaped in the content
        // (probe-verified: stem:[[[a,b\],[c,d\]\]((n),(k))] → [[a,b],[c,d]]((n),(k))).
        let events = parse(r"stem:[[[a,b\],[c,d\]\]((n),(k))]");
        assert_eq!(events, vec![
            Event::Start(Tag::Stem { variant: Cow::Borrowed("stem") }),
            Event::Text(Cow::Owned("[[a,b],[c,d]]((n),(k))".into())),
            Event::End(TagEnd::Stem),
        ]);

        let events = parse(r"stem:[a\]b]");
        assert_eq!(events, vec![
            Event::Start(Tag::Stem { variant: Cow::Borrowed("stem") }),
            Event::Text(Cow::Owned("a]b".into())),
            Event::End(TagEnd::Stem),
        ]);
    }

    #[test]
    fn test_empty_double_plus_passthrough() {
        // `++++` inline = empty unconstrained passthrough → renders as nothing
        // (probe-verified); `stem::[…]` block form falls to a paragraph whose
        // text must stay literal (no inline stem match on `stem::[`).
        let events = parse("a ++++ b");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("a ")),
            Event::Text(Cow::Borrowed(" b")),
        ]);

        let events = parse("stem::[x_0(1 + r)^2]");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("stem::[x_0(1 + r)^2]"))]);
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
    fn test_index_term_sliding_close_and_partial_parens() {
        // Probe-verified vs asciidoctor (/tmp/p_subs/p3): the non-greedy close
        // slides past a `))` followed by another `)`.
        // ((a))) → flow term "a" + literal ")"
        let events = parse("((a))) here");
        assert_eq!(events, vec![
            Event::IndexTerm { text: Cow::Borrowed("a") },
            Event::Text(Cow::Borrowed(")")),
            Event::Text(Cow::Borrowed(" here")),
        ]);
        // ((((b)))) → concealed term "(b)" (inner parens are content)
        let events = parse("((((b))))");
        assert_eq!(events, vec![
            Event::ConcealedIndexTerm {
                primary: Cow::Borrowed("(b)"),
                secondary: None,
                tertiary: None,
            },
        ]);
        // (((a)) → literal "(" + flow term "a"
        let events = parse("(((a)) x");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("(")),
            Event::IndexTerm { text: Cow::Borrowed("a") },
            Event::Text(Cow::Borrowed(" x")),
        ]);
    }

    #[test]
    fn test_escaped_index_term() {
        // Probe-verified vs asciidoctor (/tmp/p_subs/p1, p3).
        // \((a)) → the whole match stays literal, backslash drops
        let events = parse("\\((simple)) escaped");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("((simple))")),
            Event::Text(Cow::Borrowed(" escaped")),
        ]);
        // \(((a))) → escaped concealed: literal parens around a VISIBLE flow term
        let events = parse("\\(((concealed)))");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("(")),
            Event::IndexTerm { text: Cow::Borrowed("concealed") },
            Event::Text(Cow::Borrowed(")")),
        ]);
        // \((a))) → content "a)" has no enclosing parens → whole match literal
        let events = parse("\\((a))) here");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("((a)))")),
            Event::Text(Cow::Borrowed(" here")),
        ]);
        // no would-be match (no closing) → backslash stays literal
        let events = parse("\\((open");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\\((open")),
        ]);
    }

    #[test]
    fn test_double_backslash_escapes_unconstrained() {
        // Probe-verified vs asciidoctor (/tmp/p_subs/p2): `\\` before an
        // unconstrained pair eats both backslashes, marks stay literal,
        // content still gets normal substitutions.
        let events = parse("The text \\\\__func__ stays");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("The text ")),
            Event::Text(Cow::Borrowed("__")),
            Event::Text(Cow::Borrowed("func")),
            Event::Text(Cow::Borrowed("__")),
            Event::Text(Cow::Borrowed(" stays")),
        ]);
        // inner content keeps formatting (probe /tmp/p_subs/p5):
        // \\__a *b* c__ → __a <strong>b</strong> c__
        let events = parse("\\\\__a *b* c__");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("__")),
            Event::Text(Cow::Borrowed("a ")),
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
            Event::Text(Cow::Borrowed("b")),
            Event::End(TagEnd::Strong),
            Event::Text(Cow::Borrowed(" c")),
            Event::Text(Cow::Borrowed("__")),
        ]);
        // mid-word position works too: mid\\__word__ → mid__word__
        let events = parse("mid\\\\__word__ inside");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("mid")),
            Event::Text(Cow::Borrowed("__")),
            Event::Text(Cow::Borrowed("word")),
            Event::Text(Cow::Borrowed("__")),
            Event::Text(Cow::Borrowed(" inside")),
        ]);
        // without a closing pair the generic `\\` escape applies as before
        // (one backslash dropped, the rest literal)
        let events = parse("\\\\__open");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("\\__open")),
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
            Event::Start(Tag::Anchor { id: Cow::Borrowed("ref"), label: None }),
            Event::End(TagEnd::Anchor),
        ]);
    }

    #[test]
    fn test_anchor_still_works() {
        let events = parse("[[id]]");
        assert_eq!(events, vec![
            Event::Start(Tag::Anchor { id: Cow::Borrowed("id"), label: None }),
            Event::End(TagEnd::Anchor),
        ]);
    }

    #[test]
    fn test_anchor_with_reftext_still_works() {
        // The xreflabel is reference text for xrefs, never part of the id.
        let events = parse("[[id,reftext]]");
        assert_eq!(events, vec![
            Event::Start(Tag::Anchor { id: Cow::Borrowed("id"), label: Some(Cow::Borrowed("reftext")) }),
            Event::End(TagEnd::Anchor),
        ]);
    }

    #[test]
    fn test_anchor_macro() {
        // anchor:id[] is equivalent to [[id]]
        let events = parse("anchor:bookmark-c[]text");
        assert_eq!(events, vec![
            Event::Start(Tag::Anchor { id: Cow::Borrowed("bookmark-c"), label: None }),
            Event::End(TagEnd::Anchor),
            Event::Text(Cow::Borrowed("text")),
        ]);

        // The xreflabel content is not rendered in place.
        let events = parse("anchor:ok[Custom Label]rest");
        assert_eq!(events, vec![
            Event::Start(Tag::Anchor { id: Cow::Borrowed("ok"), label: Some(Cow::Borrowed("Custom Label")) }),
            Event::End(TagEnd::Anchor),
            Event::Text(Cow::Borrowed("rest")),
        ]);

        // A target with whitespace is not a macro — literal text.
        let events = parse("anchor:b ad[]text");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("anchor:b ad[]text"))]);

        // Escaped macro: backslash dropped, macro skipped.
        let events = parse("\\anchor:foo[]text");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("anchor:foo[]")),
            Event::Text(Cow::Borrowed("text")),
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
            Event::Start(Tag::Anchor { id: Cow::Borrowed("id"), label: None }),
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
        // Apostrophe is replaced; `---` has no rule and stays literal.
        let events = parse("it's---done");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("it\u{2019}s---done".to_string())),
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
            Event::Start(Tag::Strong { id: None, roles: Vec::new() }),
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
    fn test_non_shorthand_bracket_is_role_span() {
        // [text]#foo# — a bare-word attrlist is taken verbatim as the role
        // (Asciidoctor: <span class="text">foo</span>).
        let events = parse("[text]#foo#");
        assert_eq!(events, vec![
            Event::Start(Tag::InlineSpan { id: None, roles: vec![Cow::Borrowed("text")] }),
            Event::Text(Cow::Borrowed("foo")),
            Event::End(TagEnd::InlineSpan),
        ]);
    }

    #[test]
    fn test_bareword_role_not_split_on_dot() {
        // [a.b]##x## — bare word (no leading shorthand) → one role "a.b", dots NOT
        // split (contrast shorthand [.a.b] → roles "a b").
        let events = parse("[a.b]##x##");
        assert_eq!(events, vec![
            Event::Start(Tag::InlineSpan { id: None, roles: vec![Cow::Borrowed("a.b")] }),
            Event::Text(Cow::Borrowed("x")),
            Event::End(TagEnd::InlineSpan),
        ]);
    }

    #[test]
    fn test_bareword_role_rejected_after_word_char() {
        // word[role]#foo# — word char before `[` blocks the span (stays literal).
        let events = parse("word[role]#foo#");
        assert_eq!(events, vec![
            Event::Text(Cow::Borrowed("word[role]")),
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
        // `-->` is not an em-dash: the leading `-` stays literal and the trailing
        // `->` becomes a right arrow, matching Asciidoctor (`A --> B` → `A -→ B`).
        let events = parse("A --> B");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("A -\u{2192} B".to_string())),
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
        // `---` has no replacement rule in Asciidoctor — stays literal.
        let events = parse("hello---world");
        assert_eq!(events, vec![Event::Text(Cow::Borrowed("hello---world"))]);
    }

    #[test]
    fn test_arrow_em_dash_bare_still_works() {
        // word--word → em-dash + zero-width space (Asciidoctor `(\w)--(?=\w)`).
        let events = parse("hello--world");
        assert_eq!(events, vec![
            Event::Text(Cow::Owned("hello\u{2014}\u{200B}world".to_string())),
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
                is_bare: false,
                role: None,
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
