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
    /// Implicit `[partintro]` open block wrapped around the leading body
    /// blocks of a book part; closes at the first child section heading.
    PartIntro,
    UnorderedList { depth: u8 },
    OrderedList { depth: u8 },
    ListItem,
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
    /// Set by `scan_next_block_once` when it consumed a no-event prefix line
    /// (block attribute / title / comment) and must be re-invoked. The
    /// `scan_next_block` wrapper loops on this flag instead of recursing, so a
    /// long run of metadata lines runs in O(1) stack.
    rescan_requested: bool,
    /// Markdown-blockquote nesting level of THIS scanner's content; nested
    /// `> >` content is parsed by a child scanner with the level incremented.
    /// Capped to keep pathological `> > > …` chains from recursing unboundedly.
    md_quote_depth: u8,
    /// A book part (`= Title` in the body, doctype book, no style) was just
    /// opened and its first child block hasn't been scanned yet — that block
    /// gets wrapped in (or restyled as) a `[partintro]` open block.
    part_awaiting_intro: bool,
    leveloffset: i32,
    idprefix: String,
    idseparator: String,
    /// Registry of section/discrete-heading ids assigned so far (in document
    /// order). Auto-generated ids that collide with a registered id get a
    /// numeric suffix (`_2`, `_3`, …), matching Asciidoctor's de-duplication.
    /// Explicit ids are registered as-is and never renamed.
    used_ids: std::collections::HashSet<String>,
    /// Attribute-entry values seen so far (names lowercased, values with
    /// `{refs}` resolved at definition time), plus the author/revision
    /// attributes implied by the header lines. Used to substitute attribute
    /// references in section titles before auto-id generation, mirroring
    /// Asciidoctor (`== About {author}` → `_about_kismet_r_lee`).
    doc_attrs: std::collections::HashMap<String, String>,
}

/// A section marker carrying this style (positional slot 1) is a standalone
/// (non-section) heading, not a section. Asciidoctor treats `float` as an
/// alias of `discrete` (the modern name); the emitted class is the literal
/// style name, so `[float]` → `class="float"` and `[discrete]` →
/// `class="discrete"`.
fn is_discrete_style(style: &str) -> bool {
    matches!(style, "discrete" | "float")
}

/// Reindent verbatim block content per the `indent` attribute, mirroring
/// Asciidoctor's `adjust_indentation!`: with `indent == 0` the common leading
/// indentation is stripped; with `indent > 0` it is replaced by `indent`
/// spaces; a negative value preserves the content. The common block indent is
/// the minimum leading whitespace over non-empty lines, or nothing if any
/// non-empty line is flush left. Empty lines pass through untouched. Stripping
/// is a zero-copy suffix slice (`indent == 0`); only `indent > 0` allocates.
/// Tabs are not expanded (rare; `tabsize` unsupported).
fn reindent_verbatim_lines<'a>(lines: Vec<&'a str>, indent: i32) -> Vec<Cow<'a, str>> {
    if indent < 0 {
        return lines.into_iter().map(Cow::Borrowed).collect();
    }
    // Common block indent = min leading whitespace over non-empty lines; a
    // flush-left non-empty line cancels stripping entirely (block_indent None).
    let mut block_indent: Option<usize> = None;
    for &line in &lines {
        if line.is_empty() {
            continue;
        }
        let line_indent = line.len() - line.trim_start().len();
        if line_indent == 0 {
            block_indent = None;
            break;
        }
        block_indent = Some(block_indent.map_or(line_indent, |b| b.min(line_indent)));
    }
    if indent == 0 {
        match block_indent {
            Some(bi) => lines
                .into_iter()
                .map(|l| if l.is_empty() { Cow::Borrowed(l) } else { Cow::Borrowed(&l[bi..]) })
                .collect(),
            None => lines.into_iter().map(Cow::Borrowed).collect(),
        }
    } else {
        let pad = " ".repeat(indent as usize);
        let strip = block_indent.unwrap_or(0);
        lines
            .into_iter()
            .map(|l| {
                if l.is_empty() {
                    Cow::Borrowed(l)
                } else {
                    Cow::Owned(format!("{pad}{}", &l[strip..]))
                }
            })
            .collect()
    }
}

/// Strip and resolve callout markers across verbatim content lines, returning
/// each line's stripped text paired with its resolved markers. Autonumber
/// markers (`<>`) are assigned sequential numbers in document order. Borrowed
/// lines stay zero-copy; owned lines (reindented) stay owned.
fn resolve_callouts_in_lines<'a>(
    content: Vec<Cow<'a, str>>,
    process_callouts: bool,
) -> Vec<(Cow<'a, str>, Vec<scanner::CalloutMarker>)> {
    if !process_callouts {
        return content.into_iter().map(|c| (c, Vec::new())).collect();
    }
    let mut auto_num: u32 = 0;
    content
        .into_iter()
        .map(|cow| {
            let (text, markers) = match cow {
                Cow::Borrowed(s) => {
                    let (stripped, markers) = scanner::strip_callout_markers(s);
                    (Cow::Borrowed(stripped), markers)
                }
                Cow::Owned(s) => {
                    let (stripped, markers) = scanner::strip_callout_markers(&s);
                    if markers.is_empty() {
                        (Cow::Owned(s), markers)
                    } else {
                        (Cow::Owned(stripped.to_string()), markers)
                    }
                }
            };
            if markers.is_empty() {
                (text, Vec::new())
            } else {
                let resolved = markers
                    .into_iter()
                    .map(|m| match m {
                        scanner::CalloutMarker::Standard(0) => {
                            auto_num += 1;
                            scanner::CalloutMarker::Standard(auto_num)
                        }
                        scanner::CalloutMarker::XmlComment(0) => {
                            auto_num += 1;
                            scanner::CalloutMarker::XmlComment(auto_num)
                        }
                        other => other,
                    })
                    .collect();
                (text, resolved)
            }
        })
        .collect()
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
            rescan_requested: false,
            md_quote_depth: 0,
            part_awaiting_intro: false,
            leveloffset: 0,
            idprefix: "_".to_string(),
            idseparator: "_".to_string(),
            used_ids: std::collections::HashSet::new(),
            doc_attrs: std::collections::HashMap::new(),
        }
    }

    /// Scanner over pre-stripped lines in BODY context (no document-header
    /// detection) — used for the compound content of markdown blockquotes.
    fn new_nested(lines: Vec<&'a str>, md_quote_depth: u8) -> Self {
        let mut scanner = Self::new("");
        scanner.lines = lines;
        scanner.header_emitted = true;
        scanner.body_started = true;
        scanner.md_quote_depth = md_quote_depth;
        scanner
    }

    /// Register an explicit id (from `[#id]`/`[[id]]`) so later auto-generated
    /// ids de-duplicate against it. Explicit ids are kept verbatim even on
    /// collision (Asciidoctor only warns).
    fn register_explicit_id(&mut self, id: &str) {
        self.used_ids.insert(id.to_string());
    }

    /// Return a unique id for an auto-generated section/heading id, appending
    /// `<sep>2`, `<sep>3`, … on collision with an already-registered id, then
    /// register and return it.
    fn unique_auto_id(&mut self, base: String) -> String {
        if self.used_ids.insert(base.clone()) {
            return base;
        }
        let mut n = 2u32;
        loop {
            let candidate = format!("{}{}{}", base, self.idseparator, n);
            if self.used_ids.insert(candidate.clone()) {
                return candidate;
            }
            n += 1;
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

    /// Push pre-resolved callout marker events (reversed for FIFO pop) with spacing.
    /// Autonumbers must already be resolved before calling this.
    fn push_callout_events_resolved(
        &mut self,
        markers: &[scanner::CalloutMarker],
        stripped: Cow<'a, str>,
    ) {
        // Push in reverse order (last marker first) for FIFO via pop.
        // Space separators are pushed AFTER the callout ref so they appear
        // BEFORE it in the pop (FIFO) output stream.
        for (idx, marker) in markers.iter().enumerate().rev() {
            match *marker {
                scanner::CalloutMarker::Standard(n) => {
                    self.push_event(Event::CalloutRef(n));
                }
                scanner::CalloutMarker::XmlComment(n) => {
                    self.push_event(Event::XmlCalloutRef(n));
                }
            }
            // Space between consecutive callout refs
            if idx > 0 {
                self.push_event(Event::Text(Cow::Borrowed(" ")));
            }
        }
        self.push_event(Event::Text(stripped));
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
        while let Some(next_line) = self.current_line() {
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
                BlockContext::PartIntro => {
                    events.push(Event::End(TagEnd::DelimitedBlock));
                }
                BlockContext::UnorderedList { .. } => {
                    events.push(Event::End(TagEnd::UnorderedList));
                }
                BlockContext::OrderedList { .. } => {
                    events.push(Event::End(TagEnd::OrderedList));
                }
                BlockContext::ListItem => {
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
                | "normal"
                | "listing" | "literal" | "quote" | "example" | "sidebar" | "pass"
                | "open"
                | "csv" | "dsv" | "tsv"
            ))
            // For implied source shorthand (`[,ruby]`) positional[0] is the
            // language, not a block style — don't leak it onto the wrapper class.
            .filter(|_| attrs.implied_source_lang.is_none())
            // A block style only ever sits in slot 1. When slot 1 was consumed
            // by a named/shorthand attribute, the remaining positionals belong
            // to later slots (`[%header,%footer]` → "%footer" is slot 2, not
            // a style).
            .filter(|_| attrs.first_positional_is_style)
            .map(|s| Cow::Owned(s.clone()));
        let subs = attrs.substitution_set(default_subs);
        // Pass through named attributes that are not consumed by the parser
        let mut named: Vec<(Cow<'_, str>, Cow<'_, str>)> = attrs.named.iter()
            .filter(|(k, _)| !matches!(k.as_str(), "format" | "subs"))
            .map(|(k, v)| (Cow::Owned(k.clone()), Cow::Owned(v.clone())))
            // Pass positional[1] as "attribution" and positional[2] as "citetitle" only for quote/verse blocks
            .chain(attrs.positional.get(1).filter(|s| !s.is_empty())
                .filter(|_| matches!(attrs.positional.first().map(|s| s.as_str()), Some("quote") | Some("verse")))
                .map(|s| (Cow::Borrowed("attribution"), Cow::Owned(s.clone()))))
            .chain(attrs.positional.get(2).filter(|s| !s.is_empty())
                .filter(|_| matches!(attrs.positional.first().map(|s| s.as_str()), Some("quote") | Some("verse")))
                .map(|s| (Cow::Borrowed("citetitle"), Cow::Owned(s.clone()))))
            // Single-quoted attribution/citetitle values get normal
            // substitutions applied by the renderer — flagged via marker keys.
            .chain(attrs.single_quoted_positionals.iter()
                .filter(|_| matches!(attrs.positional.first().map(|s| s.as_str()), Some("quote") | Some("verse")))
                .filter_map(|&i| match i {
                    1 => Some((Cow::Borrowed("attribution-subs"), Cow::Borrowed(""))),
                    2 => Some((Cow::Borrowed("citetitle-subs"), Cow::Borrowed(""))),
                    _ => None,
                }))
            .collect();
        named.sort_by(|(a, _), (b, _)| a.cmp(b));
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

    /// Record an attribute entry into the title-resolution map: unset forms
    /// (`:!name:` / `:name!:`) remove the key, set forms store the value with
    /// `{refs}` resolved against the entries seen so far (definition-time
    /// resolution, like Asciidoctor). Names are lowercased on both store and
    /// lookup.
    fn record_attribute_entry(&mut self, name: &str, value: &str) {
        if let Some(n) = name.strip_prefix('!') {
            self.doc_attrs.remove(&n.to_ascii_lowercase());
        } else if let Some(n) = name.strip_suffix('!') {
            self.doc_attrs.remove(&n.to_ascii_lowercase());
        } else {
            let resolved = self.resolve_title_attr_refs(value).into_owned();
            self.doc_attrs.insert(name.to_ascii_lowercase(), resolved);
        }
    }

    /// Default source language from the `source-language` document attribute,
    /// if set. Presence-based (an empty value still counts) — mirrors
    /// Asciidoctor, where `:source-language:` promotes blocks to source even
    /// when the value is empty.
    fn default_source_language(&self) -> Option<String> {
        self.doc_attrs.get("source-language").cloned()
    }

    /// Substitute `{name}` attribute references against the recorded entries.
    /// Used on section/heading titles before auto-id generation (Asciidoctor
    /// applies attribute substitution to the title before deriving the id)
    /// and on entry values at definition time. Undefined references stay
    /// literal — `generate_id`'s character sanitization then drops the
    /// braces, matching Asciidoctor's default `attribute-missing=skip`.
    fn resolve_title_attr_refs<'b>(&self, text: &'b str) -> Cow<'b, str> {
        if self.doc_attrs.is_empty() || !text.contains('{') {
            return Cow::Borrowed(text);
        }
        let mut result = String::with_capacity(text.len());
        let mut rest = text;
        let mut changed = false;
        while let Some(start) = rest.find('{') {
            result.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            if let Some(end) = after.find('}') {
                let name = &after[..end];
                if let Some(value) = self.doc_attrs.get(&name.to_ascii_lowercase()) {
                    result.push_str(value);
                    changed = true;
                } else {
                    result.push('{');
                    result.push_str(name);
                    result.push('}');
                }
                rest = &after[end + 1..];
            } else {
                result.push('{');
                rest = after;
            }
        }
        result.push_str(rest);
        if changed {
            Cow::Owned(result)
        } else {
            Cow::Borrowed(text)
        }
    }

    fn is_in_list_context(&self) -> bool {
        self.context_stack.iter().rev().any(|ctx| {
            matches!(ctx, BlockContext::ListItem | BlockContext::DescriptionListEntry { .. } | BlockContext::CalloutListItem)
        })
    }

    /// True when the innermost open container is a list item — i.e. we are
    /// scanning directly in list-item content, not inside a delimited block
    /// (open `--`, example `====`, etc.) nested within the list.
    ///
    /// A blank line ends a list only in the former case. Inside a nested
    /// delimited block the blank line is just a block separator owned by that
    /// block, which closes only at its matching delimiter (handled earlier by
    /// `check_close_delimited_block`). The blank-line-driven list-closing
    /// guards must therefore use THIS check, not `is_in_list_context`: when a
    /// delimited block sits above the list item on the stack, firing the guard
    /// runs `close_list_contexts` (which finds no list at the stack top and
    /// returns nothing) and then pops an empty buffer — silently truncating the
    /// rest of the delimited block. Scanning from the top, a delimited-block /
    /// part-intro container is a barrier: it owns its blank lines.
    fn is_directly_in_list_context(&self) -> bool {
        for ctx in self.context_stack.iter().rev() {
            match ctx {
                BlockContext::ListItem
                | BlockContext::DescriptionListEntry { .. }
                | BlockContext::CalloutListItem => return true,
                BlockContext::DelimitedBlock { .. } | BlockContext::PartIntro => return false,
                _ => {}
            }
        }
        false
    }

    /// Close all list-related contexts from the top of the stack.
    /// Returns close events in emission order.
    fn close_list_contexts(&mut self) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        while let Some(ctx) = self.context_stack.last() {
            match ctx {
                BlockContext::ListItem => {
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
            matches!(ctx, BlockContext::ListItem | BlockContext::DescriptionListEntry { .. } | BlockContext::CalloutListItem)
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
                    BlockContext::ListItem => {
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
                Some(BlockContext::ListItem) => events.push(Event::End(TagEnd::ListItem)),
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
        // Iterative trampoline: `scan_next_block_once` sets `rescan_requested`
        // (instead of tail-recursing) when it consumes a metadata-only line, so
        // a long run of `[attr]`/`.title`/comment lines can't overflow the stack.
        loop {
            self.rescan_requested = false;
            let event = self.scan_next_block_once();
            if event.is_none() && self.rescan_requested {
                continue;
            }
            return event;
        }
    }

    /// Skip line comments (`//`) and comment blocks (`////`) at the current
    /// position. Asciidoctor allows comments before the document header and
    /// between its lines (title/author/revision/attributes) without ending
    /// the header. Returns true if any lines were consumed.
    fn skip_header_comments(&mut self) -> bool {
        let mut skipped = false;
        while let Some(line) = self.current_line() {
            if scanner::is_line_comment(line) {
                self.advance();
                skipped = true;
            } else if let Some((scanner::DelimiterType::Comment, delim_len)) =
                scanner::is_delimiter(line)
            {
                self.advance();
                while let Some(inner) = self.current_line() {
                    self.advance();
                    if let Some((dt, dl)) = scanner::is_delimiter(inner)
                        && dt == scanner::DelimiterType::Comment && dl == delim_len {
                            break;
                    }
                }
                skipped = true;
            } else {
                break;
            }
        }
        skipped
    }

    /// Pre-`body_started` constructs: document header, attribute-only header,
    /// block attribute `[...]`, block title `.Title`.
    /// `Some(r)` → handled (caller returns `r`); `None` → fall through.
    fn scan_header_constructs(&mut self, line: &'a str) -> Option<Option<Event<'a>>> {
        // Comments ahead of the header must not start the body — `= Title`
        // is still recognized after leading `//` lines and `////` blocks.
        if !self.body_started && self.skip_header_comments() {
            self.rescan_requested = true;
            return Some(None);
        }

        // Document header detection: `= Title` or `# Title` before any body content
        // Skip if [discrete] attribute is pending — treat as discrete heading instead
        if !self.header_emitted && !self.body_started
            && let Some((1, title)) = scanner::strip_any_section_marker(line)
            && !self.pending_block_attrs.as_ref().is_some_and(|a| {
                a.positional.first().is_some_and(|s| is_discrete_style(s))
            })
        {
                return Some(self.scan_document_header(title));
        }

        // Attribute-only document header: starts with attribute entries but no `= Title` yet
        if !self.header_emitted && !self.body_started
            && scanner::is_attribute_entry(line).is_some()
        {
            return Some(self.scan_attribute_only_header());
        }

        // Block attribute `[...]` — checked before body_started to allow metadata before header
        if let Some(attr_str) = scanner::is_block_attribute(line) {
            // If in list context with preceding blank line (not via continuation), close list
            if self.is_directly_in_list_context() && !self.in_continuation && self.had_blank_line {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return Some(self.event_buffer.pop());
            }
            self.advance();
            self.had_blank_line = false;
            // Stacked metadata lines accumulate (Asciidoctor merges every
            // `[...]` line above a block into one attribute set).
            let parsed = BlockAttributes::parse(attr_str);
            self.pending_block_attrs = Some(match self.pending_block_attrs.take() {
                Some(prev) => BlockAttributes::merge(prev, parsed),
                None => parsed,
            });
            self.rescan_requested = true;
            return Some(None);
        }

        // Block title `.Title` — checked before body_started
        if let Some(title) = scanner::is_block_title(line) {
            // If in list context with preceding blank line (not via continuation),
            // close list: a title line between lists keeps them separate
            if self.is_directly_in_list_context() && !self.in_continuation && self.had_blank_line {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return Some(self.event_buffer.pop());
            }
            self.advance();
            self.pending_block_title = Some(title);
            self.rescan_requested = true;
            return Some(None);
        }

        None
    }

    /// Book-part intro handling (Asciidoctor `next_section` REVIEW logic).
    /// Returns `Some(..)` when an event was produced; `None` — continue the
    /// normal block dispatch (possibly with restyled pending attributes).
    ///
    /// The first body block of a part is wrapped in an open block styled
    /// `partintro`; the wrapper stays open until the first child section. A
    /// bare open block is restyled in place instead of double-wrapping. An
    /// explicitly `[partintro]`-styled block is left alone — the existing
    /// paragraph masquerade / open-block style already renders it — and no
    /// wrapper context is kept, so later pre-section blocks render OUTSIDE
    /// it (Asciidoctor logs "illegal block content outside of partintro
    /// block" and appends them to the part).
    fn handle_part_intro(&mut self, line: &'a str) -> Option<Option<Event<'a>>> {
        if !self.part_awaiting_intro {
            return None;
        }
        // Comments are invisible — they don't start the intro.
        if scanner::is_line_comment(line)
            || matches!(scanner::is_delimiter(line), Some((scanner::DelimiterType::Comment, _)))
        {
            return None;
        }
        self.part_awaiting_intro = false;

        let pending_style = self
            .pending_block_attrs
            .as_ref()
            .filter(|a| a.first_positional_is_style)
            .and_then(|a| a.positional.first())
            .map(|s| s.as_str());
        // Explicit [partintro]: the block renders as the intro by itself.
        if pending_style == Some("partintro") {
            return None;
        }
        if matches!(scanner::is_delimiter(line), Some((scanner::DelimiterType::Open, _)))
            && matches!(pending_style, None | Some("" | "open"))
        {
            // Restyle the open block as the partintro itself; no wrapper.
            let restyled = BlockAttributes::parse("partintro");
            self.pending_block_attrs = Some(match self.pending_block_attrs.take() {
                Some(prev) => BlockAttributes::merge(prev, restyled),
                None => restyled,
            });
            return None;
        }

        // Wrap: open a partintro block around the leading part content.
        self.push_event(Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Open }));
        self.emit_block_metadata(&BlockAttributes::parse("partintro"), SubstitutionSet::NORMAL);
        self.context_stack.push(BlockContext::PartIntro);
        Some(self.event_buffer.pop())
    }

    /// Leaf body constructs: attribute entry, thematic/page break, section
    /// heading, toc macro, include directive.
    fn scan_leaf_blocks(&mut self, line: &'a str) -> Option<Option<Event<'a>>> {
        // Attribute entry `:name: value`
        if let Some((name, value)) = scanner::is_attribute_entry(line) {
            self.advance();
            let value = self.read_multiline_attribute_value(value);
            self.record_attribute_entry(name, &value);
            if name == "leveloffset" {
                self.update_leveloffset(&value);
                return Some(Some(Event::Attribute {
                    name: Cow::Borrowed(name),
                    value: Cow::Owned(self.leveloffset.to_string()),
                }));
            }
            self.update_id_settings(name, &value);
            // doctype is a header-only attribute — ignore in body
            if name == "doctype" {
                return Some(self.next());
            }
            return Some(Some(Event::Attribute {
                name: Cow::Borrowed(name),
                value,
            }));
        }

        // Thematic break `'''`
        if scanner::is_thematic_break(line) {
            self.advance();
            return Some(Some(Event::ThematicBreak));
        }

        // Page break `<<<`
        if scanner::is_page_break(line) {
            self.advance();
            return Some(Some(Event::PageBreak));
        }

        // Section heading `== Title` or `## Title`
        if let Some((level, title)) = scanner::strip_any_section_marker(line) {
            return Some(self.scan_section(level, title));
        }

        // TOC macro `toc::[]` — distinct from the `:toc:`-attribute auto TOC so
        // the renderer can honour Asciidoctor's rule that the macro renders only
        // under `:toc: macro` (otherwise it is inert).
        if scanner::is_toc_macro(line) {
            self.advance();
            return Some(Some(Event::TocMacro));
        }

        // NOTE: `include::path[attrs]` is NOT detected here. Asciidoctor resolves
        // include directives in the reader (our preprocessor); a line that reaches
        // the parser (e.g. produced by an escaped `\include::`) is ordinary text.

        None
    }

    /// Block macros with `::` syntax: image, video, audio, custom block macro.
    fn scan_block_macros(&mut self, line: &'a str) -> Option<Option<Event<'a>>> {
        // Block image `image::path[alt]`
        if let Some((target, alt)) = scanner::is_block_image(line) {
            self.advance();
            let title_events = self.take_pending_block_title();
            let mut block_attrs = self.pending_block_attrs.take().unwrap_or_default();
            let img_attrs = crate::attributes::parse_image_attrs(alt);
            // Merge align/float from image macro attrs into block metadata (block attrs take priority)
            if let Some(align) = img_attrs.align {
                block_attrs.named.entry("align".to_string()).or_insert_with(|| align.to_string());
            }
            if let Some(float) = img_attrs.float {
                block_attrs.named.entry("float".to_string()).or_insert_with(|| float.to_string());
            }
            // Merge role from the image macro attrs (`image::x[…,role=screenshot]`)
            // into the block roles so it lands on the imageblock wrapper class.
            if let Some(role) = img_attrs.role
                && !block_attrs.roles.iter().any(|r| r == role)
            {
                block_attrs.roles.push(role.to_string());
            }
            // `caption=` from the macro attrs overrides the "Figure N. " prefix.
            if let Some(caption) = img_attrs.caption {
                block_attrs.named.entry("caption".to_string()).or_insert_with(|| caption.to_string());
            }
            // `title=` from the macro attrs wins over a preceding `.Title` line.
            let title_events = if let Some(title) = img_attrs.title {
                vec![
                    Event::Start(Tag::BlockTitle),
                    Event::Text(Cow::Borrowed(title)),
                    Event::End(TagEnd::BlockTitle),
                ]
            } else {
                title_events
            };
            // `link=` may sit either in the macro attrlist or on the preceding
            // block attribute line (`[#id,link=…]`); the macro attrlist wins,
            // mirroring how Asciidoctor layers macro attrs over block-line attrs.
            let link = img_attrs
                .link
                .map(Cow::Borrowed)
                .or_else(|| block_attrs.named.get("link").map(|v| Cow::Owned(v.clone())));
            // An SVG image (format=svg or a `.svg` target) with the `interactive`
            // option renders as an `<object>` element (html5.rb convert_image).
            // The `inline` option (embed the SVG source) is not supported — it
            // requires reading the file; such images fall back to `<img>`.
            let is_svg = img_attrs.format == Some("svg") || target.contains(".svg");
            let interactive = is_svg && img_attrs.interactive;
            let fallback = img_attrs.fallback.map(Cow::Borrowed);
            self.push_event(Event::End(TagEnd::BlockImage));
            self.push_event(Event::Start(Tag::BlockImage {
                target: Cow::Borrowed(target),
                alt: Cow::Borrowed(img_attrs.alt),
                width: img_attrs.width.map(Cow::Borrowed),
                height: img_attrs.height.map(Cow::Borrowed),
                link,
                interactive,
                fallback,
            }));
            self.emit_block_metadata(&block_attrs, SubstitutionSet::NORMAL);
            self.push_title_then_events(title_events);
            return Some(self.event_buffer.pop());
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
            return Some(self.event_buffer.pop());
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
            return Some(self.event_buffer.pop());
        }

        // No catch-all for unknown `name::target[attrs]` block macros:
        // Asciidoctor matches only registered names, so an unknown form stays
        // a literal paragraph (probe-verified; mirrors the inline-macro rule).

        None
    }

    /// Block containers and the line comment: admonition, table, delimited
    /// block, markdown code fence, single-line comment.
    fn scan_block_containers(&mut self, line: &'a str) -> Option<Option<Event<'a>>> {
        // Admonition `NOTE: text`
        if let Some((label, text)) = scanner::is_admonition(line) {
            // If in list context with blank line (not continuation), close list first
            if self.is_directly_in_list_context() && !self.in_continuation && self.had_blank_line {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return Some(self.event_buffer.pop());
            }
            return Some(self.scan_admonition(label, text));
        }

        // Table `|===`
        if scanner::is_table_delimiter(line) {
            // If in list context with blank line (not continuation), close list first
            if self.is_directly_in_list_context() && !self.in_continuation && self.had_blank_line {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return Some(self.event_buffer.pop());
            }
            return Some(self.scan_table());
        }

        // Delimited block
        if let Some((delim_type, delim_len)) = scanner::is_delimiter(line) {
            // If in list context (and not via list continuation), close list first
            if self.is_directly_in_list_context() && !self.in_continuation {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return Some(self.event_buffer.pop());
            }
            self.in_continuation = false;
            return Some(self.scan_delimited_block(delim_type, delim_len));
        }

        // Markdown code fence ``` or ```lang
        if let Some((backtick_count, language)) = scanner::is_markdown_code_fence(line) {
            // If in list context (and not via list continuation), close list first
            if self.is_directly_in_list_context() && !self.in_continuation {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return Some(self.event_buffer.pop());
            }
            self.in_continuation = false;
            return Some(self.scan_markdown_code_fence(backtick_count, language));
        }

        // Single-line comment `// ...` — consume consecutive comment lines
        // iteratively rather than recursing, so large comment blocks can't
        // overflow the stack.
        if scanner::is_line_comment(line) {
            // A comment after a blank line forces adjacent lists apart
            // (Asciidoctor: line comment between lists keeps them separate).
            if self.is_directly_in_list_context() && !self.in_continuation && self.had_blank_line {
                let close_events = self.close_list_contexts();
                for ev in close_events.into_iter().rev() {
                    self.push_event(ev);
                }
                return Some(self.event_buffer.pop());
            }
            self.advance();
            while let Some(next) = self.current_line() {
                if scanner::is_line_comment(next) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.rescan_requested = true;
            return Some(None);
        }

        None
    }

    /// List constructs: callout, unordered, ordered, description list, and the
    /// list continuation `+`.
    fn scan_list_constructs(&mut self, line: &'a str) -> Option<Option<Event<'a>>> {
        // Callout list
        if let Some((number, text)) = scanner::is_callout_list_item(line) {
            return Some(self.scan_callout_list_item(number, text));
        }

        // Unordered list
        if let Some((depth, text)) = scanner::is_list_marker_unordered(line) {
            return Some(self.scan_unordered_list_item(depth, text));
        }

        // Ordered list
        if let Some((depth, text)) = scanner::is_list_marker_ordered(line) {
            return Some(self.scan_ordered_list_item(depth, text));
        }

        // Description list
        if let Some((depth, term, desc)) = scanner::is_description_list_marker(line) {
            return Some(self.scan_description_list_item(depth, term, desc));
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
                return Some(self.event_buffer.pop());
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
                    return Some(self.event_buffer.pop());
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
                                return Some(other);
                            }
                            // Buffer: block event + remaining attrs (reverse for FIFO)
                            if let Some(evt) = other {
                                self.event_buffer.push(evt);
                            }
                            for attr in attr_events.drain(1..).rev() {
                                self.event_buffer.push(attr);
                            }
                            return Some(attr_events.into_iter().next());
                        }
                    }
                }
            }
            // Outside list context, emit as a single-line paragraph
            self.push_event(Event::End(TagEnd::Paragraph));
            self.push_event(Event::Text(Cow::Borrowed("+")));
            self.push_event(Event::Start(Tag::Paragraph));
            return Some(self.event_buffer.pop());
        }

        None
    }

    fn scan_next_block_once(&mut self) -> Option<Event<'a>> {
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

        if let Some(r) = self.scan_header_constructs(line) {
            return r;
        }

        // From here on, we're in the document body
        self.body_started = true;

        if let Some(r) = self.scan_leaf_blocks(line) {
            return r;
        }

        if let Some(ev) = self.handle_part_intro(line) {
            return ev;
        }

        if let Some(r) = self.scan_block_macros(line) {
            return r;
        }

        if let Some(r) = self.scan_block_containers(line) {
            return r;
        }

        if let Some(r) = self.scan_list_constructs(line) {
            return r;
        }

        self.scan_paragraph_fallback(line)
    }

    /// Universal paragraph fallback (always handles): literal/normal indented
    /// paragraph, list-closing before a regular paragraph, regular paragraph.
    fn scan_paragraph_fallback(&mut self, line: &'a str) -> Option<Event<'a>> {
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
        if self.is_directly_in_list_context() && !self.in_continuation
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

    /// Consumes consecutive comment and attribute-entry lines in the document
    /// header (multiline values included), pushing Attribute events and
    /// recording the entries; stops at the first blank or other line without
    /// consuming it. Mirror of Asciidoctor's process_attribute_entries, which
    /// runs before the author line, between author and revision lines, and
    /// after the revision line.
    fn consume_header_attr_entries(&mut self, header_events: &mut Vec<Event<'a>>) {
        loop {
            if self.skip_header_comments() {
                continue;
            }
            let Some(line) = self.current_line() else { break };
            let Some((name, value)) = scanner::is_attribute_entry(line) else {
                break;
            };
            self.advance();
            let value = self.read_multiline_attribute_value(value);
            self.record_attribute_entry(name, &value);
            if name == "leveloffset" {
                self.update_leveloffset(&value);
            }
            self.update_id_settings(name, &value);
            header_events.push(Event::Attribute {
                name: Cow::Borrowed(name),
                value,
            });
        }
    }

    fn scan_document_header(&mut self, title: &'a str) -> Option<Event<'a>> {
        self.header_emitted = true;
        self.advance();

        let id = scanner::generate_id(&self.resolve_title_attr_refs(title), &self.idprefix, &self.idseparator);

        // Collect header content lines first
        let mut header_events: Vec<Event<'a>> = Vec::new();

        // Author line: the non-blank, non-attribute-entry line DIRECTLY after
        // the title — ANY line, even one shaped like a section marker (`== Sec`
        // right after the title becomes the author in Asciidoctor). NOTE:
        // Asciidoctor 2.0.23 also accepts an author line after attribute
        // entries (process_attribute_entries runs first), but the AsciiDoc
        // spec (parsing-lab block/header/adjacent-to-body) mandates a
        // paragraph there — we follow the spec; known divergence.
        self.skip_header_comments();
        if let Some(line) = self.current_line()
            && !scanner::is_blank(line)
            && scanner::is_attribute_entry(line).is_none()
        {
            // Parse as author line
            let authors = scanner::parse_authors(line);
            // Record the implied author attributes for title-id resolution
            // (`author`/`firstname`/… unsuffixed, `author_2`/… for the rest).
            for (idx, author) in authors.iter().enumerate() {
                let suffix = if idx == 0 { String::new() } else { format!("_{}", idx + 1) };
                self.doc_attrs.insert(format!("author{suffix}"), author.fullname.to_string());
                self.doc_attrs.insert(format!("firstname{suffix}"), author.firstname.to_string());
                if !author.middlename.is_empty() {
                    self.doc_attrs.insert(format!("middlename{suffix}"), author.middlename.to_string());
                }
                self.doc_attrs.insert(format!("lastname{suffix}"), author.lastname.to_string());
                self.doc_attrs.insert(format!("authorinitials{suffix}"), author.initials.clone());
                if !author.address.is_empty() {
                    self.doc_attrs.insert(format!("email{suffix}"), author.address.to_string());
                }
            }
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

            // Attribute entries between the author and revision lines are
            // transparent too (process_attribute_entries runs again).
            self.consume_header_attr_entries(&mut header_events);

            // Revision line: the next non-blank, non-attribute-entry line is
            // run through RevisionInfoLineRx, which matches nearly anything
            // (a freeform line becomes the revdate); set-but-empty version and
            // remark still set their attribute (renders `version ,` / an empty
            // remark span). A non-match falls through to the body.
            if let Some(rev_line) = self.current_line()
                && !scanner::is_blank(rev_line)
                && scanner::is_attribute_entry(rev_line).is_none()
                && let Some(rev_info) = scanner::parse_revision_line(rev_line)
            {
                header_events.push(Event::Revision {
                    version: Cow::Borrowed(rev_info.version.unwrap_or("")),
                    date: Cow::Borrowed(rev_info.date),
                    remark: Cow::Borrowed(rev_info.remark.unwrap_or("")),
                });
                if let Some(version) = rev_info.version {
                    self.record_attribute_entry("revnumber", version);
                    header_events.push(Event::Attribute {
                        name: Cow::Borrowed("revnumber"),
                        value: Cow::Borrowed(version),
                    });
                }
                if !rev_info.date.is_empty() {
                    self.record_attribute_entry("revdate", rev_info.date);
                    header_events.push(Event::Attribute {
                        name: Cow::Borrowed("revdate"),
                        value: Cow::Borrowed(rev_info.date),
                    });
                }
                if let Some(remark) = rev_info.remark {
                    self.record_attribute_entry("revremark", remark);
                    header_events.push(Event::Attribute {
                        name: Cow::Borrowed("revremark"),
                        value: Cow::Borrowed(remark),
                    });
                }
                self.advance();
            }
        }

        // Remaining attribute entries; a blank line (consumed) or any other
        // line (left in place) ends the header.
        self.consume_header_attr_entries(&mut header_events);
        if let Some(line) = self.current_line()
            && scanner::is_blank(line)
        {
            self.advance();
        }

        // Check if :toc: attribute is present in header events
        let has_toc = header_events.iter().any(|ev| {
            matches!(ev, Event::Attribute { name, .. } if name == "toc")
        });

        // Build buffer in reverse pop order:
        // Start(Header) -> Start(SectionTitle) -> Start(DocTitle) -> Text -> End(DocTitle) -> End(SectionTitle) -> [header content] -> [Toc] -> End(Header)
        // End(Header) is pushed first (bottom of stack), emitted last
        self.push_event(Event::End(TagEnd::Header));
        if has_toc {
            self.push_event(Event::Toc);
        }
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
            if self.skip_header_comments() {
                continue;
            }
            if scanner::is_blank(line) {
                self.advance();
                // After blank, check if next non-blank line is `= Title`
                continue;
            }
            if let Some((name, value)) = scanner::is_attribute_entry(line) {
                self.advance();
                let value = self.read_multiline_attribute_value(value);
                self.record_attribute_entry(name, &value);
                if name == "leveloffset" {
                    self.update_leveloffset(&value);
                }
                self.update_id_settings(name, &value);
                header_events.push(Event::Attribute {
                    name: Cow::Borrowed(name),
                    value,
                });
            } else if let Some((1, title)) = scanner::strip_any_section_marker(line)
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
        let id = scanner::generate_id(&self.resolve_title_attr_refs(title), &self.idprefix, &self.idseparator);

        // Collect header content lines after the title
        let mut header_events: Vec<Event<'a>> = Vec::new();

        while let Some(line) = self.current_line() {
            if self.skip_header_comments() {
                continue;
            }
            if scanner::is_blank(line) {
                self.advance();
                break;
            }
            if let Some((name, value)) = scanner::is_attribute_entry(line) {
                self.advance();
                let value = self.read_multiline_attribute_value(value);
                self.record_attribute_entry(name, &value);
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

        self.push_event(Event::End(TagEnd::Header));
        if has_toc {
            self.push_event(Event::Toc);
        }
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
            a.positional.first().is_some_and(|s| is_discrete_style(s))
        });
        let inside_delimited = self.is_inside_delimited_block();

        if is_discrete || inside_delimited {
            return self.scan_discrete_heading(level, title);
        }

        // Apply leveloffset to section level
        let effective_level = (level as i32 + self.leveloffset).max(1) as u8;

        // Section style (positional slot 1). A styled section is a "special
        // section" (Asciidoctor initialize_section): a level-0 special section
        // is displayed at level 1 (`[preface]` + `= T` → sect1/h2), and in a
        // book `[abstract]` becomes a chapter at level 1 regardless of its
        // marker depth. The COERCED level is display-only: section closing
        // uses the raw marker level (Asciidoctor decides nesting from the
        // peeked raw level before initialize_section runs).
        let sect_style: Option<String> = self
            .pending_block_attrs
            .as_ref()
            .filter(|a| a.first_positional_is_style)
            .and_then(|a| a.positional.first())
            .filter(|s| !s.is_empty() && s.as_str() != "float")
            .cloned();
        let book = self.doc_attrs.get("doctype").map(String::as_str) == Some("book");
        let is_sect_level_style = |s: &str| {
            s.len() == 5 && s.starts_with("sect") && s.as_bytes()[4].is_ascii_digit()
        };
        let mut display_level = effective_level;
        if let Some(style) = sect_style.as_deref()
            // book abstract → chapter at level 1 regardless of marker depth;
            // any other special style is lifted only from level 0.
            && ((book && style == "abstract")
                || (!is_sect_level_style(style) && effective_level == 1))
        {
            display_level = 2;
        }

        self.advance();
        let list_close_events = self.close_list_contexts();
        // Any section heading ends an open part intro before section closing.
        let mut close_events = Vec::new();
        if matches!(self.context_stack.last(), Some(BlockContext::PartIntro)) {
            self.context_stack.pop();
            close_events.push(Event::End(TagEnd::DelimitedBlock));
        }
        close_events.extend(self.close_sections_for_level(effective_level));
        // A bare level-0 section in a book is a part: its leading body blocks
        // (before the first child section) get wrapped in a partintro block.
        self.part_awaiting_intro = book && sect_style.is_none() && effective_level == 1;

        let id = match self.pending_block_attrs.as_ref().and_then(|a| a.id.clone()) {
            Some(explicit) => {
                self.register_explicit_id(&explicit);
                explicit
            }
            None => {
                let base = scanner::generate_id(&self.resolve_title_attr_refs(title), &self.idprefix, &self.idseparator);
                self.unique_auto_id(base)
            }
        };

        let block_attrs = self.pending_block_attrs.take();
        let title_events = self.take_pending_block_title();

        self.context_stack.push(BlockContext::Section { level: display_level });

        // Buffer (bottom to top): section content, then close events, then title
        self.push_event(Event::End(TagEnd::SectionTitle));
        self.push_event(Event::Text(Cow::Borrowed(title)));
        self.push_event(Event::Start(Tag::SectionTitle {
            level: display_level,
            id: Cow::Owned(id),
        }));
        self.push_event(Event::Start(Tag::Section { level: display_level }));
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
        let mut block_attrs = self.pending_block_attrs.take();
        let title_events = self.take_pending_block_title();

        // Apply leveloffset to heading level
        let effective_level = (level as i32 + self.leveloffset).max(1) as u8;

        // Auto-generate id for discrete headings if not explicitly set;
        // register/de-duplicate against the shared section-id registry.
        if let Some(ref mut attrs) = block_attrs
            && attrs.positional.first().is_some_and(|s| is_discrete_style(s))
        {
            match attrs.id.clone() {
                Some(explicit) => self.register_explicit_id(&explicit),
                None => {
                    let base = scanner::generate_id(&self.resolve_title_attr_refs(title), &self.idprefix, &self.idseparator);
                    attrs.id = Some(self.unique_auto_id(base));
                }
            }
        }

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
        // The table is terminated only by a line equal to the OPENING delimiter
        // (trimmed). A table delimiter of a different length appearing inside
        // (e.g. a `|====` cell inside a `|===` table) is cell content, not a
        // terminator — asciidoctor matches the exact opening delimiter string.
        let opening_delim = self.current_line().map_or("", str::trim);
        self.advance(); // skip opening delimiter
        let title_events = self.take_pending_block_title();
        let mut block_attrs = self.pending_block_attrs.take().unwrap_or_default();

        // Collect lines until a line equal to the opening delimiter, or EOF
        let mut content_lines: Vec<&'a str> = Vec::new();
        while let Some(line) = self.current_line() {
            if line.trim() == opening_delim {
                self.advance();
                break;
            }
            content_lines.push(line);
            self.advance();
        }

        // Check for CSV/DSV/TSV format. A shorthand delimiter prefix selects the
        // format directly (`,===` → CSV, `:===` → DSV); `|===` and `!===` defer
        // to the `format=` attribute (Native unless overridden).
        let format = match opening_delim.as_bytes().first() {
            Some(b',') => TableFormat::Csv,
            Some(b':') => TableFormat::Dsv,
            _ => block_attrs.table_format(),
        };
        if format != TableFormat::Native {
            return self.scan_delimited_format_table(&content_lines, block_attrs, format, title_events);
        }

        // PSV cell separator: `!===` (a table nested inside an AsciiDoc `a`
        // cell) splits cells on `!`; every other native table uses `|`.
        let sep = if opening_delim.as_bytes().first() == Some(&b'!') { b'!' } else { b'|' };

        // Parse cells from content lines. Text before the first `|` of a line
        // (or a line with no `|` at all) continues the previous cell — the
        // lines are joined with a newline inside the same cell paragraph.
        let mut all_cells: Vec<scanner::CellSpec<'a>> = Vec::new();
        let mut first_data_idx: Option<usize> = None;
        // Implicit column count: cells of the first row, which stays open
        // across continuation lines (cells opened there count too) and closes
        // at the first subsequent line that starts with a cell delimiter.
        let mut first_row_open = true;
        let mut first_row_width: usize = 0;
        let mut first_blank_idx: Option<usize> = None;
        let mut cells_before_blank_col_width: usize = 0;
        // For implicit header promotion the line after the first blank must
        // itself start a new cell (a continuation line cancels the header).
        let mut awaiting_post_blank_line = false;
        let mut post_blank_line_starts_cell = false;
        // A blank line before the first data line (comments are invisible)
        // suppresses implicit header promotion entirely
        let mut blank_before_first_data = false;

        for (idx, &line) in content_lines.iter().enumerate() {
            // Line comments are invisible inside tables — dropped from cell
            // content and ignored by the header/colcount bookkeeping
            if scanner::is_line_comment(line) {
                continue;
            }
            if scanner::is_blank(line) {
                if first_data_idx.is_none() {
                    blank_before_first_data = true;
                }
                if first_data_idx.is_some() && first_blank_idx.is_none() {
                    first_blank_idx = Some(idx);
                    // Sum of colspan values for cells before blank
                    cells_before_blank_col_width = all_cells
                        .iter()
                        .map(|c| c.colspan as usize * c.duplication as usize)
                        .sum();
                    awaiting_post_blank_line = true;
                }
                // A blank line is part of the open cell's content (structural
                // for AsciiDoc cells, preserved in literal cells; collapsed
                // for other styles at emission).
                if first_data_idx.is_some() && !all_cells.is_empty() {
                    Self::append_cell_continuation(&mut all_cells, "");
                }
                continue;
            }
            let parsed = scanner::parse_table_cells_with_sep(line, sep);
            let starts_fresh = matches!(&parsed, Some(t) if t.continuation.is_none());
            if awaiting_post_blank_line {
                awaiting_post_blank_line = false;
                post_blank_line_starts_cell = starts_fresh;
            }
            if first_data_idx.is_some() && starts_fresh {
                first_row_open = false;
            }
            match parsed {
                Some(t) => {
                    if let Some(text) = t.continuation {
                        Self::append_cell_continuation(&mut all_cells, &text);
                    }
                    all_cells.extend(t.cells);
                }
                // A line with no (unescaped) separator continues the open cell;
                // escaped `\|`/`\!` separators in it are unescaped like content.
                None => Self::append_cell_continuation(
                    &mut all_cells,
                    &scanner::unescape_cell_sep(line.trim_end(), sep),
                ),
            }
            if first_data_idx.is_none() {
                first_data_idx = Some(idx);
            }
            if first_row_open {
                first_row_width = all_cells
                    .iter()
                    .map(|c| c.colspan as usize * c.duplication as usize)
                    .sum();
            }
        }

        if all_cells.is_empty() {
            self.push_title_then_events(title_events);
            return self.event_buffer.pop().or_else(|| self.scan_next_block());
        }

        // Determine number of columns: from cols attribute or the first row
        let num_cols = if let Some(n) = block_attrs.table_cols_count() {
            n
        } else if first_row_width == 0 {
            1
        } else {
            first_row_width
        };

        // Synthesize cols attribute for tables without explicit cols=
        // so the renderer can generate <colgroup> with equal-width columns
        if block_attrs.table_cols_count().is_none() && num_cols > 0 {
            block_attrs.named.insert("cols".to_string(), num_cols.to_string());
        }

        // Determine header: %header option OR a blank line directly after the
        // first content line, with the next line starting a fresh cell;
        // %noheader suppresses the implicit promotion (explicit header wins)
        let implicit_header = matches!(
            (first_data_idx, first_blank_idx),
            (Some(d), Some(b)) if b == d + 1
        ) && post_blank_line_starts_cell
            && cells_before_blank_col_width == num_cols
            && !blank_before_first_data;
        let has_header = block_attrs.has_option("header")
            || (implicit_header && !block_attrs.has_option("noheader"));

        // Determine footer: %footer option
        let has_footer = block_attrs.has_option("footer");

        // Get column specs for alignment defaults
        let col_specs = block_attrs.table_col_specs();

        // Expand duplication factors (`3*|x` → three copies) now that every
        // cell's content is complete — copies carry the full content,
        // including continuation lines (asciidoctor copies the closed cell)
        let all_cells: Vec<scanner::CellSpec<'a>> = all_cells
            .into_iter()
            .flat_map(|c| {
                let n = c.duplication.max(1) as usize;
                std::iter::repeat_n(c, n)
            })
            .collect();

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

        // Resolve alignment for a cell: an explicit cell-level operator
        // (`<`/`^`/`>` or `.<`/`.^`/`.>`) wins; otherwise the cell inherits the
        // column's default. The `*_explicit` flags let us distinguish an
        // explicit `<` (Left) / `.<` (Top) from the indistinguishable defaults.
        let resolve_align = |cell: &scanner::CellSpec<'_>, col_idx: usize| -> (HAlign, VAlign) {
            let mut halign = cell.halign;
            let mut valign = cell.valign;
            if let Some(ref specs) = col_specs
                && col_idx < specs.len()
            {
                if !cell.halign_explicit {
                    halign = specs[col_idx].halign;
                }
                if !cell.valign_explicit {
                    valign = specs[col_idx].valign;
                }
            }
            (halign, valign)
        };

        // Resolve a cell's effective style: an explicit cell style wins,
        // otherwise the cell inherits its column's style (header `h` → <th>,
        // AsciiDoc `a` → nested block parse, e/s/m wrappers, literal `l`).
        // Header-section rows ignore column styles (asciidoctor: plain <th>).
        let resolve_style = |cell: &scanner::CellSpec<'_>, col_idx: usize| -> CellStyle {
            if !cell.style_explicit
                && cell.style == CellStyle::Default
                && let Some(ref specs) = col_specs
                && col_idx < specs.len()
            {
                return specs[col_idx].style;
            }
            cell.style
        };

        // Cell text by resolved style: AsciiDoc/literal cells keep inner blank
        // lines and indentation (only the edges of the whole text stripped);
        // all other styles collapse to trimmed, non-empty lines joined by \n.
        fn cell_text<'b>(cell: &scanner::CellSpec<'b>, style: CellStyle) -> Cow<'b, str> {
            if matches!(style, CellStyle::AsciiDoc | CellStyle::Literal) {
                match &cell.content {
                    Cow::Borrowed(s) => Cow::Borrowed(s.trim()),
                    Cow::Owned(s) => Cow::Owned(s.trim().to_string()),
                }
            } else if cell.content.contains('\n') {
                Cow::Owned(
                    cell.content
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            } else {
                cell.content.clone()
            }
        }

        // Partition a non-AsciiDoc/non-literal body cell into paragraphs on
        // blank lines (asciidoctor `Cell#content`: split on /\n{2,}/). Each
        // paragraph keeps its non-empty trimmed lines joined by '\n'. Returns at
        // most one entry for AsciiDoc/literal cells (no splitting) and for cells
        // without a blank line, so the single-paragraph fast path stays
        // byte-identical to `cell_text`.
        fn cell_paragraphs<'b>(cell: &scanner::CellSpec<'b>, style: CellStyle) -> Vec<Cow<'b, str>> {
            if matches!(style, CellStyle::AsciiDoc | CellStyle::Literal) {
                return vec![cell_text(cell, style)];
            }
            let mut paras: Vec<Cow<'b, str>> = Vec::new();
            let mut cur: Vec<&str> = Vec::new();
            for line in cell.content.lines() {
                let t = line.trim();
                if t.is_empty() {
                    if !cur.is_empty() {
                        paras.push(Cow::Owned(cur.join("\n")));
                        cur.clear();
                    }
                } else {
                    cur.push(t);
                }
            }
            if !cur.is_empty() {
                paras.push(Cow::Owned(cur.join("\n")));
            }
            paras
        }

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
                    let style = resolve_style(cell, col_indices[ci]);
                    if $is_header_section {
                        // Header-row cell (thead): <th> without a paragraph wrapper.
                        self.push_event(Event::End(TagEnd::TableHeaderCell));
                        self.push_event(Event::Text(cell_text(cell, CellStyle::Default)));
                        self.push_event(Event::Start(Tag::TableHeaderCell {
                            colspan: cell.colspan,
                            rowspan: cell.rowspan,
                            style: cell.style,
                            halign,
                            valign,
                        }));
                    } else {
                        // Body/footer cell. A header-style cell (explicit `h|` or an
                        // `h` column) renders as <th> but keeps the <p> wrapper; the
                        // renderer picks the tag from the resolved style.
                        self.push_event(Event::End(TagEnd::TableCell));
                        // A cell with a blank line becomes several
                        // <p class="tableblock"> paragraphs, joined by separator
                        // markers; the single-paragraph path keeps the old
                        // (zero-copy) text emission untouched.
                        let paras = cell_paragraphs(cell, style);
                        if paras.len() <= 1 {
                            self.push_event(Event::Text(cell_text(cell, style)));
                        } else {
                            for (i, para) in paras.into_iter().enumerate().rev() {
                                self.push_event(Event::Text(para));
                                if i > 0 {
                                    self.push_event(Event::TableCellParagraphBreak);
                                }
                            }
                        }
                        self.push_event(Event::Start(Tag::TableCell {
                            colspan: cell.colspan,
                            rowspan: cell.rowspan,
                            style,
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

    /// Append a table-line continuation (text before the first `|`, or a line
    /// with no `|`) to the last open cell, joined with a newline. With no cell
    /// open yet (start of table), the text starts a cell of its own.
    /// Limitation: a continuation after a blank line should open a second
    /// `<p class="tableblock">` inside the cell (asciidoctor); we keep it in
    /// the same paragraph.
    fn append_cell_continuation(cells: &mut Vec<scanner::CellSpec<'a>>, text: &str) {
        if let Some(last) = cells.last_mut() {
            let content = last.content.to_mut();
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(text);
        } else {
            cells.push(scanner::CellSpec {
                content: Cow::Owned(text.to_string()),
                duplication: 1,
                colspan: 1,
                rowspan: 1,
                style: CellStyle::Default,
                style_explicit: false,
                halign: HAlign::default(),
                valign: VAlign::default(),
                halign_explicit: false,
                valign_explicit: false,
            });
        }
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

            // If we've filled the row, start a new one. The skip loop below
            // consumes one row of occupancy for each leading column held by a
            // rowspan from a prior row (the top-of-loop skip handles mid-row
            // occupied columns) — so each occupied column is decremented exactly
            // once per row. A separate "decrement all" pass would double-count.
            if col >= num_cols {
                rows.push(std::mem::take(&mut current_row));
                col = 0;
                // Skip columns occupied by rowspan from previous rows
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

        // Push the last row only if it completes the grid — asciidoctor drops
        // cells from an incomplete row at the end of the table. Columns held
        // by a rowspan after the last cell count toward completeness.
        if !current_row.is_empty() {
            while col < num_cols && col_remaining[col] > 0 {
                col_remaining[col] -= 1;
                col += 1;
            }
            if col >= num_cols {
                rows.push(current_row);
            }
        }

        rows
    }

    fn scan_delimited_format_table(
        &mut self,
        content_lines: &[&'a str],
        mut block_attrs: BlockAttributes,
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
                // Defensive: native tables are parsed elsewhere and never reach
                // the delimiter-row parser; skip the line rather than panicking.
                TableFormat::Native => continue,
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

        // Synthesize a cols attribute for format tables without explicit cols=
        // so the renderer generates <colgroup> with equal-width columns (mirror
        // of the native-table path in scan_table).
        if block_attrs.table_cols_count().is_none() && num_cols > 0 {
            block_attrs.named.insert("cols".to_string(), num_cols.to_string());
        }

        // Determine header/footer; %noheader suppresses implicit promotion
        let has_header = block_attrs.has_option("header")
            || (first_blank_after_first_row
                && rows[0].len() == num_cols
                && !block_attrs.has_option("noheader"));
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

        // Verbatim paragraph styles (verse/literal/listing/source) preserve line
        // comments as content (Asciidoctor reads their lines raw), unlike normal/
        // quote/example/sidebar/pass paragraphs which strip them.
        let verbatim_paragraph = self
            .pending_block_attrs
            .as_ref()
            .is_some_and(|a| {
                matches!(a.block_style_kind(), Some("verse" | "literal" | "listing"))
                    || a.is_source_block()
            });

        while let Some(line) = self.current_line() {
            // A line comment inside a paragraph is dropped and the paragraph
            // continues (Asciidoctor reads paragraph lines with
            // skip_line_comments); verbatim paragraph styles keep it as content.
            if !verbatim_paragraph && scanner::is_line_comment(line) {
                if para_lines.is_empty() {
                    break;
                }
                self.advance();
                continue;
            }
            if scanner::is_blank(line)
                // NOTE: a section marker (`== Title`) mid-paragraph does NOT
                // interrupt an open paragraph — Asciidoctor's StartOfBlockProc
                // (read_paragraph_lines) breaks only on a block delimiter or a
                // block-attribute line, never a section title. Section titles
                // are recognized only at a block boundary (after a blank line),
                // by the scan_leaf_blocks dispatcher. So `==== <.>` written as a
                // continuation line is plain paragraph text.
                || scanner::is_delimiter(line).is_some()
                || scanner::is_markdown_code_fence(line).is_some()
                || scanner::is_list_marker_unordered(line).is_some()
                || scanner::is_list_marker_ordered(line).is_some()
                || scanner::is_admonition(line).is_some()
                || scanner::is_block_image(line).is_some()
                || scanner::is_block_video(line).is_some()
                || scanner::is_block_audio(line).is_some()
                || scanner::is_toc_macro(line)
                || scanner::is_thematic_break(line)
                || scanner::is_page_break(line)
                || scanner::is_block_attribute(line).is_some()
                || scanner::is_description_list_marker(line).is_some()
                // A callout-list marker (`<1>`) interrupts an open paragraph ONLY
                // when we are already inside a callout list — there it ends the
                // current item's continuation text and opens the next sibling item.
                // At top level a `<N>` line following paragraph content is plain
                // text: Asciidoctor recognizes a *new* callout list only at a block
                // boundary (after a blank line) via the scan_leaf_blocks dispatcher,
                // never as a paragraph continuation (cf. `|=== <1>` then `<2>`).
                // (Unordered/ordered markers above remain a known pre-existing
                // divergence: Asciidoctor absorbs those into an open paragraph too.)
                || (self.is_in_callout_list()
                    && scanner::is_callout_list_item(line).is_some())
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
        let block_style = self.pending_block_attrs.as_ref()
            .and_then(|a| a.block_style_kind())
            .map(|s| s.to_string());

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
            self.push_event(Event::Start(Tag::Admonition { kind, block: false }));
            if let Some(ref attrs) = block_attrs {
                self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
            }
        } else if let Some(style) = block_style {
            match style.as_str() {
                "listing" | "literal" => {
                    let kind = if style == "listing" {
                        DelimitedBlockKind::Listing
                    } else {
                        DelimitedBlockKind::Literal
                    };
                    self.push_event(Event::End(TagEnd::DelimitedBlock));
                    for (i, &pline) in para_lines.iter().enumerate().rev() {
                        if i < para_lines.len() - 1 {
                            self.push_event(Event::SoftBreak);
                        }
                        self.push_event(Event::Text(Cow::Borrowed(pline)));
                    }
                    self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
                    if let Some(ref attrs) = block_attrs {
                        self.emit_block_metadata(attrs, SubstitutionSet::VERBATIM);
                    }
                }
                "source" => {
                    let language = block_attrs.as_ref()
                        .and_then(|a| a.source_language())
                        .map(|l| l.to_string())
                        .or_else(|| self.default_source_language())
                        .map(Cow::Owned);
                    self.push_event(Event::End(TagEnd::SourceBlock));
                    for (i, &pline) in para_lines.iter().enumerate().rev() {
                        if i < para_lines.len() - 1 {
                            self.push_event(Event::SoftBreak);
                        }
                        self.push_event(Event::Text(Cow::Borrowed(pline)));
                    }
                    self.push_event(Event::Start(Tag::SourceBlock { language }));
                    if let Some(ref attrs) = block_attrs {
                        self.emit_block_metadata(attrs, SubstitutionSet::VERBATIM);
                    }
                }
                "pass" => {
                    self.push_event(Event::End(TagEnd::DelimitedBlock));
                    for (i, &pline) in para_lines.iter().enumerate().rev() {
                        if i < para_lines.len() - 1 {
                            self.push_event(Event::SoftBreak);
                        }
                        self.push_event(Event::Text(Cow::Borrowed(pline)));
                    }
                    self.push_event(Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Passthrough }));
                    if let Some(ref attrs) = block_attrs {
                        self.emit_block_metadata(attrs, SubstitutionSet::NONE);
                    }
                }
                "verse" => {
                    self.push_event(Event::End(TagEnd::DelimitedBlock));
                    for (i, &pline) in para_lines.iter().enumerate().rev() {
                        if i < para_lines.len() - 1 {
                            self.push_event(Event::SoftBreak);
                        }
                        self.push_event(Event::Text(Cow::Borrowed(pline)));
                    }
                    self.push_event(Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Verse }));
                    if let Some(ref attrs) = block_attrs {
                        self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
                    }
                }
                // A paragraph masqueraded by a block style carries its text bare
                // (no inner paragraph wrapper), unlike a delimited block that
                // happens to contain a single paragraph.
                "quote" | "example" | "sidebar" | "open" => {
                    let kind = match style.as_str() {
                        "quote" => DelimitedBlockKind::Quote,
                        "example" => DelimitedBlockKind::Example,
                        "sidebar" => DelimitedBlockKind::Sidebar,
                        _ => DelimitedBlockKind::Open,
                    };
                    self.push_event(Event::End(TagEnd::DelimitedBlock));
                    for (i, &pline) in para_lines.iter().enumerate().rev() {
                        if i < para_lines.len() - 1 {
                            self.push_event(Event::SoftBreak);
                        }
                        self.push_event(Event::Text(Cow::Borrowed(pline)));
                    }
                    self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
                    if let Some(ref attrs) = block_attrs {
                        self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
                    }
                }
                // partintro masquerades a paragraph as an open block and KEEPS
                // the paragraph wrapper inside (unlike the styles above); the
                // style is not consumed by emit_block_metadata, so the renderer
                // adds it to the wrapper class ("openblock partintro").
                "partintro" => {
                    self.push_event(Event::End(TagEnd::DelimitedBlock));
                    self.push_event(Event::End(TagEnd::Paragraph));
                    for (i, &pline) in para_lines.iter().enumerate().rev() {
                        if i < para_lines.len() - 1 {
                            self.push_event(Event::SoftBreak);
                        }
                        self.push_event(Event::Text(Cow::Borrowed(pline)));
                    }
                    self.push_event(Event::Start(Tag::Paragraph));
                    self.push_event(Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Open }));
                    if let Some(ref attrs) = block_attrs {
                        self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
                    }
                }
                // Defensive: block_style_kind() only yields the styles handled
                // above; degrade an unknown style to a normal paragraph.
                _ => {
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
            }
        } else if para_lines.len() > 1
            && para_lines[0].starts_with('"')
            && para_lines[para_lines.len() - 1].starts_with("-- ")
            && para_lines[para_lines.len() - 1].len() > 3
            && para_lines[para_lines.len() - 2].ends_with('"')
        {
            // Quoted paragraph shorthand (asciidoctor parser.rb): a paragraph
            // opening with `"`, whose second-to-last line ends with `"` and
            // whose last line is `-- attribution[, citetitle]` becomes a
            // quote block with BARE content (no paragraph wrapper).
            let credit = &para_lines[para_lines.len() - 1][3..];
            let mut lines: Vec<&'a str> = para_lines[..para_lines.len() - 1].to_vec();
            lines[0] = &lines[0][1..]; // strip leading quote
            let last = lines.len() - 1;
            lines[last] = &lines[last][..lines[last].len() - 1]; // strip trailing quote
            let mut attrs = block_attrs.unwrap_or_default();
            attrs.positional = vec!["quote".to_string(), credit.to_string()];
            if let Some((author, cite)) = credit.split_once(", ") {
                attrs.positional = vec!["quote".to_string(), author.to_string(), cite.to_string()];
            }
            // asciidoctor applies subs to the credit line (apply_subs)
            attrs.single_quoted_positionals = vec![1, 2];
            self.push_event(Event::End(TagEnd::DelimitedBlock));
            for (i, &pline) in lines.iter().enumerate().rev() {
                if i < lines.len() - 1 {
                    self.push_event(Event::SoftBreak);
                }
                self.push_event(Event::Text(Cow::Borrowed(pline)));
            }
            self.push_event(Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Quote }));
            self.emit_block_metadata(&attrs, SubstitutionSet::NORMAL);
        } else if para_lines[0].starts_with("> ") && self.md_quote_depth < 16 {
            // Markdown-style blockquote (asciidoctor parser.rb): `>`-prefixed
            // lines become a quote block with COMPOUND content — one `>` level
            // is stripped and the rest is parsed as nested blocks (`> >` →
            // nested quote, `> *` → list, a stripped bare `>` separates
            // paragraphs); a trailing `-- attribution[, citetitle]` line
            // supplies the attribution. Depth cap guards runaway recursion on
            // pathological `> > > …` chains (beyond it: plain paragraph).
            let mut lines: Vec<&'a str> = para_lines
                .iter()
                .map(|l| {
                    if *l == ">" {
                        &l[1..]
                    } else if let Some(rest) = l.strip_prefix("> ") {
                        rest
                    } else {
                        l
                    }
                })
                .collect();
            let mut credit: Option<&'a str> = None;
            if let Some(last) = lines.pop_if(|l| l.starts_with("-- ") && l.len() > 3) {
                credit = Some(&last[3..]);
                while lines.last().is_some_and(|l| l.trim().is_empty()) {
                    lines.pop();
                }
            }
            let mut attrs = block_attrs.unwrap_or_default();
            attrs.positional = vec!["quote".to_string()];
            if let Some(credit) = credit {
                if let Some((author, cite)) = credit.split_once(", ") {
                    attrs.positional = vec![
                        "quote".to_string(),
                        author.to_string(),
                        cite.to_string(),
                    ];
                } else {
                    attrs.positional.push(credit.to_string());
                }
                // asciidoctor applies subs to the credit line (apply_subs)
                attrs.single_quoted_positionals = vec![1, 2];
            }
            let nested_events: Vec<Event<'a>> =
                BlockScanner::new_nested(lines, self.md_quote_depth + 1).collect();
            self.push_event(Event::End(TagEnd::DelimitedBlock));
            for ev in nested_events.into_iter().rev() {
                self.push_event(ev);
            }
            self.push_event(Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Quote }));
            self.emit_block_metadata(&attrs, SubstitutionSet::NORMAL);
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
            if scanner::is_blank(line) {
                break;
            }
            // A line comment continuing an indented literal paragraph is kept as
            // content (verbatim), matching Asciidoctor; any other non-indented
            // line terminates the paragraph.
            if !line.starts_with(' ')
                && !line.starts_with('\t')
                && !scanner::is_line_comment(line)
            {
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

        // Collect continuation lines (same break conditions as scan_paragraph)
        let mut continuation_lines: Vec<&'a str> = Vec::new();
        while let Some(line) = self.current_line() {
            // Line comments inside the admonition paragraph are dropped and
            // the paragraph continues (skip_line_comments, as in scan_paragraph).
            if scanner::is_line_comment(line) {
                self.advance();
                continue;
            }
            if scanner::is_blank(line)
                // Section markers do not interrupt an open paragraph (see the
                // note in scan_paragraph) — the admonition's principal paragraph
                // follows the same Asciidoctor rule.
                || scanner::is_delimiter(line).is_some()
                || scanner::is_markdown_code_fence(line).is_some()
                || scanner::is_list_marker_unordered(line).is_some()
                || scanner::is_list_marker_ordered(line).is_some()
                || scanner::is_admonition(line).is_some()
                || scanner::is_block_image(line).is_some()
                || scanner::is_block_video(line).is_some()
                || scanner::is_block_audio(line).is_some()
                || scanner::is_toc_macro(line)
                || scanner::is_thematic_break(line)
                || scanner::is_page_break(line)
                || scanner::is_block_attribute(line).is_some()
                || scanner::is_description_list_marker(line).is_some()
                // A callout-list marker interrupts an open paragraph only inside a
                // callout list — see the note in scan_paragraph.
                || (self.is_in_callout_list()
                    && scanner::is_callout_list_item(line).is_some())
                || scanner::is_list_continuation(line)
                || scanner::is_table_delimiter(line)
            {
                break;
            }
            continuation_lines.push(line.trim_end());
            self.advance();
        }

        self.push_event(Event::End(TagEnd::Admonition));
        self.push_event(Event::End(TagEnd::Paragraph));

        // Push all lines in reverse (reversed stack pattern)
        // Stack order: text is popped first, then SoftBreak+cline pairs
        for &cline in continuation_lines.iter().rev() {
            self.push_event(Event::Text(Cow::Borrowed(cline)));
            self.push_event(Event::SoftBreak);
        }
        self.push_event(Event::Text(Cow::Borrowed(text)));

        self.push_event(Event::Start(Tag::Paragraph));
        self.push_event(Event::Start(Tag::Admonition { kind, block: false }));
        let block_attrs = self.pending_block_attrs.take();
        if let Some(ref attrs) = block_attrs {
            self.emit_block_metadata(attrs, SubstitutionSet::NORMAL);
        }
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    /// Check if a delimiter line closes any parent structural block in the context stack.
    /// Used by verbatim block scanning loops to avoid "eating" the parent's closing delimiter.
    fn closes_parent_block(&self, line: &str) -> bool {
        if let Some((dt, dl)) = scanner::is_delimiter(line) {
            for ctx in self.context_stack.iter().rev() {
                if let BlockContext::DelimitedBlock { kind, delimiter_len, .. } = ctx
                    && *kind == dt && *delimiter_len == dl
                {
                    return true;
                }
            }
        }
        false
    }

    fn scan_delimited_block(
        &mut self,
        delim_type: scanner::DelimiterType,
        delim_len: usize,
    ) -> Option<Event<'a>> {
        self.advance(); // skip opening delimiter
        let title_events = self.take_pending_block_title();

        let block_attrs = self.pending_block_attrs.take().unwrap_or_default();

        // Check for source block (applies to any delimiter type with [source] style)
        if block_attrs.is_source_block() {
            let language = block_attrs.source_language()
                .map(|l| l.to_string())
                .or_else(|| self.default_source_language())
                .map(Cow::Owned);
            return self.scan_source_block(delim_type, delim_len, language, title_events, &block_attrs);
        }

        // A bare listing block (`----` with no explicit style) is promoted to a
        // source block when the `source-language` document attribute is set —
        // Asciidoctor applies the default language to undecorated listings.
        // An explicit `[listing]`/`[literal]`/… style (block_style_kind() is
        // Some) opts out; `....` literal delimiters never reach this branch.
        if delim_type == scanner::DelimiterType::Listing
            && block_attrs.block_style_kind().is_none()
            && let Some(lang) = self.default_source_language()
        {
            return self.scan_source_block(
                delim_type,
                delim_len,
                Some(Cow::Owned(lang)),
                title_events,
                &block_attrs,
            );
        }

        // Verse block: [verse] on any delimiter type
        if block_attrs.is_verse_style() {
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

        // Style remapping: [style] on any delimited block
        if let Some(style) = block_attrs.block_style_kind() {
            match style {
                "source" | "verse" => {} // already handled above
                "listing" | "literal" | "pass" => {
                    let kind = match style {
                        "listing" => DelimitedBlockKind::Listing,
                        "literal" => DelimitedBlockKind::Literal,
                        _ => DelimitedBlockKind::Passthrough,
                    };
                    let default_subs = if style == "pass" {
                        SubstitutionSet::NONE
                    } else {
                        SubstitutionSet::VERBATIM
                    };
                    let mut content_lines: Vec<&'a str> = Vec::new();
                    let mut closed = false;
                    while let Some(line) = self.current_line() {
                        if let Some((dt, dl)) = scanner::is_delimiter(line)
                            && dt == delim_type && dl == delim_len {
                                self.advance();
                                closed = true;
                                break;
                        }
                        if self.closes_parent_block(line) {
                            break;
                        }
                        content_lines.push(line);
                        self.advance();
                    }
                    if !closed && content_lines.last().is_some_and(|l| l.is_empty()) {
                        content_lines.pop();
                    }
                    self.push_event(Event::End(TagEnd::DelimitedBlock));
                    for (i, &cline) in content_lines.iter().enumerate().rev() {
                        if i < content_lines.len() - 1 {
                            self.push_event(Event::SoftBreak);
                        }
                        self.push_event(Event::Text(Cow::Borrowed(cline)));
                    }
                    self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
                    self.emit_block_metadata(&block_attrs, default_subs);
                    self.push_title_then_events(title_events);
                    return self.event_buffer.pop();
                }
                "quote" | "example" | "sidebar" => {
                    let kind = match style {
                        "quote" => DelimitedBlockKind::Quote,
                        "example" => DelimitedBlockKind::Example,
                        _ => DelimitedBlockKind::Sidebar,
                    };
                    self.context_stack.push(BlockContext::DelimitedBlock {
                        kind: delim_type,
                        delimiter_len: delim_len,
                        admonition_kind: None,
                    });
                    self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
                    self.emit_block_metadata(&block_attrs, SubstitutionSet::NORMAL);
                    self.push_title_then_events(title_events);
                    return self.event_buffer.pop();
                }
                _ => {}
            }
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
                if self.closes_parent_block(line) {
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
            let resolved_subs = block_attrs
                .substitution_set(SubstitutionSet::VERBATIM)
                .unwrap_or(SubstitutionSet::VERBATIM);
            let process_callouts = matches!(kind, DelimitedBlockKind::Listing)
                || resolved_subs.has(SubstitutionSet::CALLOUTS);
            // Reindent per the `indent` attribute, then resolve callout markers.
            let content = match block_attrs.verbatim_indent() {
                Some(indent) => reindent_verbatim_lines(content_lines, indent),
                None => content_lines.into_iter().map(Cow::Borrowed).collect(),
            };
            let parsed_lines = resolve_callouts_in_lines(content, process_callouts);

            self.push_event(Event::End(TagEnd::DelimitedBlock));
            let line_count = parsed_lines.len();
            for (i, (text, markers)) in parsed_lines.into_iter().enumerate().rev() {
                if i < line_count - 1 {
                    self.push_event(Event::SoftBreak);
                }
                if markers.is_empty() {
                    self.push_event(Event::Text(text));
                } else {
                    self.push_callout_events_resolved(&markers, text);
                }
            }
            // Push Start on top of content
            self.push_event(Event::Start(Tag::DelimitedBlock { kind }));
            self.emit_block_metadata(&block_attrs, SubstitutionSet::VERBATIM);
            // Push title events on very top (emitted first)
            self.push_title_then_events(title_events);

            return self.event_buffer.pop();
        }

        // Structural blocks (example, sidebar, quote, open): recursively parse content.
        // An admonition style turns the block into an admonition only on example and
        // open delimiters; on sidebar/quote (and verbatim above) asciidoctor ignores it.
        let adm_kind = if matches!(
            delim_type,
            scanner::DelimiterType::Example | scanner::DelimiterType::Open
        ) {
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
            self.push_event(Event::Start(Tag::Admonition { kind: ak, block: true }));
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
            if self.closes_parent_block(line) {
                break;
            }
            content_lines.push(line);
            self.advance();
        }

        let resolved_subs = block_attrs
            .substitution_set(SubstitutionSet::VERBATIM)
            .unwrap_or(SubstitutionSet::VERBATIM);
        let process_callouts = resolved_subs.has(SubstitutionSet::CALLOUTS);

        // Reindent per the `indent` attribute, then resolve callout markers.
        let content = match block_attrs.verbatim_indent() {
            Some(indent) => reindent_verbatim_lines(content_lines, indent),
            None => content_lines.into_iter().map(Cow::Borrowed).collect(),
        };
        let parsed_lines = resolve_callouts_in_lines(content, process_callouts);

        self.push_event(Event::End(TagEnd::SourceBlock));
        let line_count = parsed_lines.len();
        for (i, (text, markers)) in parsed_lines.into_iter().enumerate().rev() {
            if i < line_count - 1 {
                self.push_event(Event::SoftBreak);
            }
            if markers.is_empty() {
                self.push_event(Event::Text(text));
            } else {
                self.push_callout_events_resolved(&markers, text);
            }
        }
        self.push_event(Event::Start(Tag::SourceBlock { language }));
        self.emit_block_metadata(block_attrs, SubstitutionSet::VERBATIM);
        self.push_title_then_events(title_events);

        self.event_buffer.pop()
    }

    fn scan_markdown_code_fence(
        &mut self,
        backtick_count: usize,
        language: Option<&'a str>,
    ) -> Option<Event<'a>> {
        self.advance(); // skip opening fence
        let title_events = self.take_pending_block_title();
        let block_attrs = self.pending_block_attrs.take().unwrap_or_default();

        let mut content_lines: Vec<&'a str> = Vec::new();
        let mut closed = false;
        while let Some(line) = self.current_line() {
            // Closing fence: >= backtick_count backticks, no info string
            if let Some((count, info)) = scanner::is_markdown_code_fence(line)
                && count >= backtick_count
                && info.is_none()
            {
                self.advance();
                closed = true;
                break;
            }
            content_lines.push(line);
            self.advance();
        }

        // For unclosed fences, trim one trailing empty line (artifact of split_lines)
        if !closed && content_lines.last().is_some_and(|l| l.is_empty()) {
            content_lines.pop();
        }

        // Asciidoctor ALWAYS renders a markdown code fence as a source block
        // (`<pre class="highlight"><code>`), even without an info-string
        // language. The language comes from the info string; otherwise it
        // falls back to the `:source-language:` default. A preceding block
        // style (`[source,lang]`, `[listing]`, …) does NOT contribute the
        // language — the fence resets style to `source` and only the info
        // string / `source-language` matter (verified vs asciidoctor 2.0.23).
        let resolved_lang: Option<CowStr<'a>> = match language {
            Some(l) => Some(Cow::Borrowed(l)),
            None => self.default_source_language().map(Cow::Owned),
        };
        self.push_event(Event::End(TagEnd::SourceBlock));
        for (i, &cline) in content_lines.iter().enumerate().rev() {
            if i < content_lines.len() - 1 {
                self.push_event(Event::SoftBreak);
            }
            self.push_event(Event::Text(Cow::Borrowed(cline)));
        }
        self.push_event(Event::Start(Tag::SourceBlock {
            language: resolved_lang,
        }));
        self.emit_block_metadata(&block_attrs, SubstitutionSet::VERBATIM);
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
            if self.closes_parent_block(line) {
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
            if self.closes_parent_block(line) {
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
                Some(BlockContext::ListItem)
                | Some(BlockContext::UnorderedList { .. })
                | Some(BlockContext::OrderedList { .. })
                | Some(BlockContext::CalloutListItem)
                | Some(BlockContext::CalloutList)
                    if has_parent_dl =>
                {
                    match self.context_stack.pop() {
                        Some(BlockContext::ListItem) => events.push(Event::End(TagEnd::ListItem)),
                        Some(BlockContext::UnorderedList { .. }) => events.push(Event::End(TagEnd::UnorderedList)),
                        Some(BlockContext::OrderedList { .. }) => events.push(Event::End(TagEnd::OrderedList)),
                        Some(BlockContext::CalloutListItem) => events.push(Event::End(TagEnd::CalloutListItem)),
                        Some(BlockContext::CalloutList) => events.push(Event::End(TagEnd::CalloutList)),
                        // Defensive: the outer guard already confirmed one of the
                        // list contexts above; ignore anything else instead of panic.
                        _ => {}
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
            && scanner::strip_any_section_marker(line).is_none()
            && scanner::is_delimiter(line).is_none()
            && scanner::is_markdown_code_fence(line).is_none()
            && scanner::is_list_marker_unordered(line).is_none()
            && scanner::is_list_marker_ordered(line).is_none()
            && scanner::is_admonition(line).is_none()
            && scanner::is_block_image(line).is_none()
            && scanner::is_block_video(line).is_none()
            && scanner::is_block_audio(line).is_none()
            && !scanner::is_toc_macro(line)
            && !scanner::is_thematic_break(line)
            && !scanner::is_page_break(line)
            && scanner::is_attribute_entry(line).is_none()
            && scanner::is_block_attribute(line).is_none()
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
                // An attribute-entry line here becomes the literal principal text
                // (Asciidoctor does not process it as an attribute in this position)
                if !check.is_empty()
                    && (self.is_dlist_continuation_line(check)
                        || scanner::is_attribute_entry(check).is_some())
                {
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
                // Asciidoctor silently drops an attribute-entry line wrapped inside
                // the principal text: not literal text, and not applied either
                if scanner::is_attribute_entry(line).is_some()
                    && !line.starts_with(' ') && !line.starts_with('\t')
                {
                    self.advance();
                    continue;
                }
                // Line comments inside the description text are dropped and the
                // text continues on the next line (skip_line_comments).
                if scanner::is_line_comment(line) {
                    self.advance();
                    continue;
                }
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

        self.had_blank_line = false;
        self.event_buffer.pop()
    }

    /// Check if a line is a continuation of a list item principal (not a block element or new list item).
    fn is_list_continuation_line(&self, line: &str) -> bool {
        !scanner::is_blank(line)
            && scanner::strip_any_section_marker(line).is_none()
            && scanner::is_delimiter(line).is_none()
            && scanner::is_markdown_code_fence(line).is_none()
            && scanner::is_list_marker_unordered(line).is_none()
            && scanner::is_list_marker_ordered(line).is_none()
            && scanner::is_admonition(line).is_none()
            && scanner::is_block_image(line).is_none()
            && scanner::is_block_video(line).is_none()
            && scanner::is_block_audio(line).is_none()
            && !scanner::is_toc_macro(line)
            && !scanner::is_thematic_break(line)
            && !scanner::is_page_break(line)
            && scanner::is_block_attribute(line).is_none()
            && !scanner::is_line_comment(line)
            && scanner::is_description_list_marker(line).is_none()
            && scanner::is_callout_list_item(line).is_none()
            && !scanner::is_list_continuation(line)
            && !scanner::is_table_delimiter(line)
    }

    fn scan_unordered_list_item(&mut self, depth: u8, text: &'a str) -> Option<Event<'a>> {
        self.advance();
        let title_events = self.take_pending_block_title();

        let (checked, actual_text) = scanner::parse_checklist_marker(text);

        // Collect wrapped continuation lines
        let mut continuation_lines: Vec<&'a str> = Vec::new();
        while let Some(line) = self.current_line() {
            // Line comments inside the item's wrapped text are dropped and the
            // text continues on the next line (skip_line_comments).
            if scanner::is_line_comment(line) {
                self.advance();
                continue;
            }
            if self.is_list_continuation_line(line) {
                continuation_lines.push(line);
                self.advance();
            } else {
                break;
            }
        }

        // Marker matching an open list (current or ancestor) → close up to it
        // and continue as a sibling item. An UNMATCHED marker — deeper,
        // shallower or different type — starts a list NESTED in the innermost
        // open item, closing nothing (Asciidoctor matches markers against the
        // list stack; probe /tmp/p_subs/p6: `. Linux` + `* Fedora` nests,
        // and even `** b` + `* c` nests the shallower marker).
        let has_parent_list = self.is_in_list_at_depth(depth, true);
        let mut close_events = if has_parent_list {
            self.close_to_parent_list(depth, true)
        } else {
            Vec::new()
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
            self.push_event(Event::Text(Cow::Borrowed(cline.trim_start())));
            self.push_event(Event::SoftBreak);
        }
        self.push_event(Event::Text(Cow::Borrowed(actual_text)));

        if need_new_list {
            self.context_stack.push(BlockContext::UnorderedList { depth });
            self.context_stack.push(BlockContext::ListItem);

            self.push_event(Event::Start(Tag::ListItem { depth, checked }));
            self.push_event(Event::Start(Tag::UnorderedList { has_checklist: checked.is_some() }));
        } else {
            self.context_stack.push(BlockContext::ListItem);

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

        // Collect wrapped continuation lines
        let mut continuation_lines: Vec<&'a str> = Vec::new();
        while let Some(line) = self.current_line() {
            // Line comments inside the item's wrapped text are dropped and the
            // text continues on the next line (skip_line_comments).
            if scanner::is_line_comment(line) {
                self.advance();
                continue;
            }
            if self.is_list_continuation_line(line) {
                continuation_lines.push(line);
                self.advance();
            } else {
                break;
            }
        }

        // Mirror of scan_unordered_list_item: a marker matching an open
        // ordered list closes up to it (cross-type contexts included); an
        // unmatched marker nests in the innermost open item.
        let has_parent_list = self.is_in_list_at_depth(depth, false);
        let mut close_events = if has_parent_list {
            self.close_to_parent_list(depth, false)
        } else {
            Vec::new()
        };

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

        // Push text events in reverse (bottom of stack = last to emit)
        for &cline in continuation_lines.iter().rev() {
            self.push_event(Event::Text(Cow::Borrowed(cline.trim_start())));
            self.push_event(Event::SoftBreak);
        }
        self.push_event(Event::Text(Cow::Borrowed(text)));

        if need_new_list {
            self.context_stack.push(BlockContext::OrderedList { depth });
            self.context_stack.push(BlockContext::ListItem);

            self.push_event(Event::Start(Tag::ListItem { depth, checked: None }));
            self.push_event(Event::Start(Tag::OrderedList { start: list_start, reversed: list_reversed, depth }));
        } else {
            self.context_stack.push(BlockContext::ListItem);

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

        // Collect wrapped continuation lines
        let mut continuation_lines: Vec<&'a str> = Vec::new();
        while let Some(line) = self.current_line() {
            // Line comments inside the item's wrapped text are dropped and the
            // text continues on the next line (skip_line_comments).
            if scanner::is_line_comment(line) {
                self.advance();
                continue;
            }
            if self.is_list_continuation_line(line) {
                continuation_lines.push(line);
                self.advance();
            } else {
                break;
            }
        }

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
        for &cline in continuation_lines.iter().rev() {
            self.push_event(Event::Text(Cow::Borrowed(cline.trim_start())));
            self.push_event(Event::SoftBreak);
        }
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

        self.had_blank_line = false;
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
                                        BlockContext::PartIntro => {
                                            events.push(Event::End(TagEnd::DelimitedBlock));
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
                                        BlockContext::ListItem => {
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
    fn test_comment_after_blank_separates_lists() {
        // A line comment after a blank line forces adjacent lists apart
        // (Asciidoctor: comment between lists keeps them separate).
        let input = "* a\n\n// comment\n* b";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("a")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("b")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);

        // Without the blank line the comment does NOT split the list.
        let input = "* a\n// comment\n* b";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("a")),
            Event::End(TagEnd::ListItem),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("b")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);
    }

    #[test]
    fn test_comment_after_dlist_entry_does_not_split_list() {
        // A blank line before a dlist entry must not leave `had_blank_line`
        // armed: a comment directly after that entry's description (no blank
        // in between) does not split the list (Asciidoctor keeps one dlist).
        let input = "a:: text a\n\nb:: text b\n// comment\n\nc:: text c";
        let events: Vec<_> = BlockScanner::new(input).collect();
        let list_starts = events
            .iter()
            .filter(|e| matches!(e, Event::Start(Tag::DescriptionList)))
            .count();
        assert_eq!(list_starts, 1, "expected a single dlist, got: {events:#?}");

        // Same shape with a comment after a blank line still splits.
        let input = "a:: text a\n\n// comment\nb:: text b";
        let events: Vec<_> = BlockScanner::new(input).collect();
        let list_starts = events
            .iter()
            .filter(|e| matches!(e, Event::Start(Tag::DescriptionList)))
            .count();
        assert_eq!(list_starts, 2, "expected two dlists, got: {events:#?}");
    }

    #[test]
    fn test_block_title_after_blank_separates_lists() {
        // A `.Title` line after a blank line closes the open list and
        // becomes the title of the next one (Asciidoctor: a block title
        // between lists keeps them separate).
        let input = "* a\n\n.Next\n* b";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("a")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
            Event::Start(Tag::BlockTitle),
            Event::Text(Cow::Borrowed("Next")),
            Event::End(TagEnd::BlockTitle),
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("b")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);

        // Without the blank line a `.Title`-looking line is wrapped item text,
        // not metadata (Asciidoctor slurps it into the item's principal text).
        let input = "* a\n.Attached\n* b";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("a")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed(".Attached")),
            Event::End(TagEnd::ListItem),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("b")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
        ]);
    }

    #[test]
    fn test_block_title_line_does_not_interrupt_paragraph() {
        // A `.Title`-looking line inside a paragraph is paragraph text
        // (Asciidoctor: block titles never interrupt a paragraph).
        let input = "para text\n.NotATitle\nmore";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("para text")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed(".NotATitle")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("more")),
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
        // A line comment inside a paragraph is dropped and the lines merge
        // into one paragraph (Asciidoctor skip_line_comments; probe-verified).
        let input = "First.\n// this is a comment\nSecond.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("First.")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("Second.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_line_comment_mid_list_item_merges_text() {
        // A comment between an item's wrapped lines is dropped and the text
        // continues in the same paragraph (probe-verified: `* a\n//c\nb`).
        let input = "* a\n// c\nb";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::UnorderedList { has_checklist: false }),
            Event::Start(Tag::ListItem { depth: 1, checked: None }),
            Event::Text(Cow::Borrowed("a")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("b")),
            Event::End(TagEnd::ListItem),
            Event::End(TagEnd::UnorderedList),
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
    fn test_document_header_after_leading_comments() {
        // Line comments and comment blocks ahead of `= Title` must not start
        // the body — the header (with author/revision) is still recognized.
        let input = "// tag::main[]\n////\nhidden\n////\n= Title\n// between\nAuthor Name\n// another\nv1.0, 2024-01-01\n:attr: x\n\nContent.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events[0], Event::Start(Tag::Header), "header must be detected: {events:?}");
        assert!(events.contains(&Event::Author {
            fullname: Cow::Borrowed("Author Name"),
            firstname: Cow::Borrowed("Author"),
            middlename: Cow::Borrowed(""),
            lastname: Cow::Borrowed("Name"),
            initials: Cow::Owned("AN".into()),
            address: Cow::Borrowed(""),
        }));
        assert!(events.contains(&Event::Revision {
            version: Cow::Borrowed("1.0"),
            date: Cow::Borrowed("2024-01-01"),
            remark: Cow::Borrowed(""),
        }));
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("attr"),
            value: Cow::Borrowed("x"),
        }));
        // Guard: a blank line still ends the header — a comment after it is body
        let input = "= Title\n\n// body comment\nHello.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(!events.iter().any(|e| matches!(e, Event::Author { .. })));
        assert!(events.contains(&Event::Text(Cow::Borrowed("Hello."))));
    }

    #[test]
    fn test_attribute_entry_inside_paragraph_is_literal() {
        // An attribute-entry line wrapped inside a paragraph is literal text,
        // not an attribute definition (Asciidoctor only recognizes entries at
        // block boundaries).
        let input = "line one\n:myattr: val\nline two";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(!events.iter().any(|e| matches!(e, Event::Attribute { .. })), "{events:?}");
        assert!(events.contains(&Event::Text(Cow::Borrowed(":myattr: val"))));
        assert!(events.contains(&Event::Text(Cow::Borrowed("line two"))));

        // Same inside a list-item principal
        let input = "* item\n:a: b\nmore";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(!events.iter().any(|e| matches!(e, Event::Attribute { .. })), "{events:?}");
        assert!(events.contains(&Event::Text(Cow::Borrowed(":a: b"))));

        // Description list: a wrapped entry is silently DROPPED (not literal,
        // not applied) — Asciidoctor quirk, verified against 2.0.23
        let input = "term:: desc\n:c: d\nmore dd";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(!events.iter().any(|e| matches!(e, Event::Attribute { .. })), "{events:?}");
        assert!(!events.contains(&Event::Text(Cow::Borrowed(":c: d"))));
        assert!(events.contains(&Event::Text(Cow::Borrowed("more dd"))));

        // ...but as the principal of a bare term it IS literal text
        let input = "term::\n:c: d\nmore dd";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(!events.iter().any(|e| matches!(e, Event::Attribute { .. })), "{events:?}");
        assert!(events.contains(&Event::Text(Cow::Borrowed(":c: d"))));

        // Guard: an entry at a block boundary (after blank) is still applied
        let input = "para\n\n:real: yes\n\nafter";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("real"),
            value: Cow::Borrowed("yes"),
        }), "{events:?}");
    }

    #[test]
    fn test_verbatim_paragraph_keeps_line_comment() {
        // Verse paragraph: a trailing comment line is kept as content
        // (Asciidoctor reads verbatim paragraph lines raw).
        let events: Vec<_> = BlockScanner::new("[verse]\nThe fog comes\n// end::para[]").collect();
        assert!(
            events.contains(&Event::Text(Cow::Borrowed("// end::para[]"))),
            "verse paragraph should keep comment as content: {events:?}"
        );

        // Indented literal paragraph: a col-0 comment continuation is kept.
        let events: Vec<_> = BlockScanner::new(" ~/secure/vault/defops\n// end::indent[]").collect();
        assert!(
            events.contains(&Event::Text(Cow::Borrowed("// end::indent[]"))),
            "literal paragraph should keep comment as content: {events:?}"
        );

        // Regression guard: a normal paragraph still strips comments.
        let events: Vec<_> = BlockScanner::new("Just text.\n// a comment").collect();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::Text(t) if t.contains("comment"))),
            "normal paragraph must strip comment: {events:?}"
        );
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
            Event::Toc,
            Event::End(TagEnd::Header),
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None },
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
    fn test_csv_shorthand_delimiter_routes_to_format_and_synthesizes_cols() {
        // A bare `,===` delimiter selects CSV format directly (no format= attr);
        // the format-table path synthesizes a cols attribute (here "2") so the
        // renderer can emit a <colgroup>, mirroring the native-table path.
        let input = ",===\nA,B\nC,D\n,===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None },
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
    fn test_dsv_shorthand_delimiter_routes_to_format() {
        // `:===` selects DSV format (colon field separator).
        let input = ":===\nA:B\n:===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        let texts: Vec<_> = events.iter().filter_map(|e| match e {
            Event::Text(t) => Some(t.as_ref()),
            _ => None,
        }).collect();
        assert_eq!(texts, vec!["A", "B"]);
        assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Table))));
    }

    #[test]
    fn test_bang_delimiter_nested_table_splits_on_bang() {
        // `!===` is the nested-table delimiter: cells split on `!`, a literal
        // `|` is ordinary content, and cols is synthesized like a native table.
        let input = "!===\n! Col1 ! Col2\n! C11\n! C12\n!===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        let texts: Vec<_> = events.iter().filter_map(|e| match e {
            Event::Text(t) => Some(t.as_ref()),
            _ => None,
        }).collect();
        assert_eq!(texts, vec!["Col1", "Col2", "C11", "C12"]);
        assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Table))));
        // First-row width is 2 → synthesized cols="2"
        assert!(events.iter().any(|e| matches!(
            e,
            Event::BlockMetadata { named, .. }
                if named.iter().any(|(k, v)| k == "cols" && v == "2")
        )));
    }

    #[test]
    fn test_table_with_header() {
        let input = "|===\n| Header 1 | Header 2\n\n| Cell 1 | Cell 2\n| Cell 3 | Cell 4\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("1".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![Cow::Owned("header".into())], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![Cow::Owned("footer".into())], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None },
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
        let input = "[%header%footer]\n|===\n| H1 | H2\n| C1 | C2\n| F1 | F2\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![Cow::Owned("header".into()), Cow::Owned("footer".into())], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("3".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("<,^,>".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("3".into()))], subs: None },
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
            Event::BlockMetadata { style: None, id: None, roles: vec![], options: vec![], named: vec![(Cow::Owned("cols".into()), Cow::Owned("<,<".into()))], subs: None },
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
            style: None, id: None, roles: vec![], options: vec![Cow::Owned("autowidth".into())], named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into()))], subs: None,
        });
        assert_eq!(events[1], Event::Start(Tag::Table));
    }

    #[test]
    fn test_table_stripes_named_attr() {
        let input = "[stripes=even]\n|===\n| A | B\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events[0], Event::BlockMetadata {
            style: None, id: None, roles: vec![], options: vec![],
            named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into())), (Cow::Owned("stripes".into()), Cow::Owned("even".into()))],
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
            named: vec![(Cow::Owned("caption".into()), Cow::Owned("Listing {counter:table-number}. ".into())), (Cow::Owned("cols".into()), Cow::Owned("2".into()))],
            subs: None,
        });
    }

    #[test]
    fn test_table_autowidth_stripes_combined() {
        let input = "[%autowidth,stripes=odd]\n|===\n| A | B\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events[0], Event::BlockMetadata {
            style: None, id: None, roles: vec![], options: vec![Cow::Owned("autowidth".into())],
            named: vec![(Cow::Owned("cols".into()), Cow::Owned("2".into())), (Cow::Owned("stripes".into()), Cow::Owned("odd".into()))],
            subs: None,
        });
    }

    #[test]
    fn test_table_cols_in_named() {
        // cols passes through to renderer for colgroup generation
        let input = "[cols=\"2\",stripes=even]\n|===\n| A\n| B\n| C\n| D\n|===";
        let events: Vec<_> = BlockScanner::new(input).collect();
        if let Event::BlockMetadata { ref named, .. } = events[0] {
            assert!(named.iter().any(|(k, _)| k == "cols"), "cols should be present");
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
            Event::Toc,
            Event::End(TagEnd::Header),
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Content.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_include_line_is_plain_text() {
        // Asciidoctor resolves includes in the reader (our preprocessor); a line
        // that reaches the parser (e.g. from an escaped `\include::`) is plain text.
        let input = "include::chapter.adoc[]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("include::chapter.adoc[]")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_include_line_does_not_break_paragraph() {
        let input = "Some text.\ninclude::file.adoc[leveloffset=+1]\nMore text.";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("Some text.")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("include::file.adoc[leveloffset=+1]")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("More text.")),
            Event::End(TagEnd::Paragraph),
        ]);
    }

    #[test]
    fn test_toc_macro() {
        let input = "toc::[]";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![Event::TocMacro]);
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
            Event::Text(Cow::Borrowed(" ")),
            Event::CalloutRef(2),
            Event::End(TagEnd::SourceBlock),
        ]);
    }

    #[test]
    fn test_source_language_default_for_bare_source() {
        // `[source]` without an explicit language inherits :source-language:.
        let input = ":source-language: ruby\n\n[source]\n----\nputs 1\n----";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SourceBlock {
            language: Some(Cow::Owned("ruby".into()))
        })));
    }

    #[test]
    fn test_source_language_promotes_bare_listing() {
        // A bare `----` listing becomes a source block when :source-language: is set.
        let input = ":source-language: ruby\n\n----\nputs 1\n----";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SourceBlock {
            language: Some(Cow::Owned("ruby".into()))
        })));
        assert!(!events.iter().any(|e| matches!(
            e,
            Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Listing })
        )));
    }

    #[test]
    fn test_source_language_does_not_promote_explicit_listing() {
        // Explicit [listing] opts out of source promotion.
        let input = ":source-language: ruby\n\n[listing]\n----\nx\n----";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::DelimitedBlock {
            kind: DelimitedBlockKind::Listing
        })));
        assert!(!events.iter().any(|e| matches!(e, Event::Start(Tag::SourceBlock { .. }))));
    }

    #[test]
    fn test_source_language_explicit_language_wins() {
        // An explicit `[source,lang]` beats the document default.
        let input = ":source-language: ruby\n\n[source,python]\n----\nprint(1)\n----";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SourceBlock {
            language: Some(Cow::Owned("python".into()))
        })));
    }

    #[test]
    fn test_bare_listing_not_promoted_without_source_language() {
        // Without :source-language:, a bare listing stays a listing.
        let input = "----\nx\n----";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::DelimitedBlock {
            kind: DelimitedBlockKind::Listing
        })));
        assert!(!events.iter().any(|e| matches!(e, Event::Start(Tag::SourceBlock { .. }))));
    }

    #[test]
    fn test_markdown_fence_without_language_is_source() {
        // Asciidoctor always renders a markdown fence as a source block,
        // even with no info-string language (F-J).
        let input = "```\nputs 1\n```";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SourceBlock { language: None })));
        assert!(!events.iter().any(|e| matches!(
            e,
            Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Listing })
        )));
    }

    #[test]
    fn test_markdown_fence_without_language_uses_source_language_default() {
        // A bare fence inherits :source-language: as its default (F-J × F-G).
        let input = ":source-language: ruby\n\n```\nputs 1\n```";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SourceBlock {
            language: Some(Cow::Owned("ruby".into()))
        })));
    }

    #[test]
    fn test_markdown_fence_info_string_language_still_wins() {
        // An explicit info-string language is honored over the default.
        let input = ":source-language: ruby\n\n```python\nx = 1\n```";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SourceBlock {
            language: Some(Cow::Borrowed("python"))
        })));
    }

    #[test]
    fn test_markdown_fence_block_style_language_ignored() {
        // A preceding `[source,python]` block style does NOT set the language
        // of a bare fence — only the info string / source-language do
        // (matches asciidoctor 2.0.23).
        let input = "[source,python]\n```\nx = 1\n```";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SourceBlock { language: None })));
    }

    #[test]
    fn test_markdown_fence_empty_is_source_without_text() {
        // An empty fence is a source block with no body (no spurious newline).
        let input = "```\n```";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SourceBlock { language: None })));
        assert!(!events.iter().any(|e| matches!(e, Event::Text(_))));
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
    fn test_callout_marker_does_not_interrupt_top_level_paragraph() {
        // A `<N>` line following paragraph content at top level is plain text,
        // not a new callout list — Asciidoctor recognizes a callout list only at
        // a block boundary. This is the `|=== <1>` / `<2>` table-doc shape.
        let input = "|=== <1>\n<2>\n| Cell A | Cell B <3>";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert_eq!(events, vec![
            Event::Start(Tag::Paragraph),
            Event::Text(Cow::Borrowed("|=== <1>")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("<2>")),
            Event::SoftBreak,
            Event::Text(Cow::Borrowed("| Cell A | Cell B <3>")),
            Event::End(TagEnd::Paragraph),
        ]);
        // Sanity: no callout list was opened.
        assert!(!events.iter().any(|e| matches!(e, Event::Start(Tag::CalloutList))));
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
            Event::Start(Tag::Admonition { kind: crate::event::AdmonitionKind::Note, block: true }),
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
            Event::Start(Tag::Admonition { kind: crate::event::AdmonitionKind::Warning, block: true }),
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
            version: Cow::Borrowed("1.0"),
            date: Cow::Borrowed("2024-01-01"),
            remark: Cow::Borrowed("Initial release"),
        }));
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("revnumber"),
            value: Cow::Borrowed("1.0"),
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
            version: Cow::Borrowed("2.0"),
            date: Cow::Borrowed(""),
            remark: Cow::Borrowed(""),
        }));
        assert!(events.contains(&Event::Attribute {
            name: Cow::Borrowed("revnumber"),
            value: Cow::Borrowed("2.0"),
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

    #[test]
    fn test_duplicate_section_ids_deduplicated() {
        // Repeated section titles get a numeric suffix (Asciidoctor parity).
        let input = "== Added\n\nx\n\n== Added\n\ny\n\n== Added\n\nz";
        let ids: Vec<String> = BlockScanner::new(input)
            .filter_map(|ev| match ev {
                Event::Start(Tag::SectionTitle { id, .. }) => Some(id.into_owned()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["_added", "_added_2", "_added_3"]);
    }

    #[test]
    fn test_auto_id_dedups_against_explicit_id() {
        // An explicit id is kept verbatim; a later auto id that would collide
        // with it skips to the next free suffix.
        let input = "[#_added]\n== First\n\nx\n\n== Added\n\ny";
        let ids: Vec<String> = BlockScanner::new(input)
            .filter_map(|ev| match ev {
                Event::Start(Tag::SectionTitle { id, .. }) => Some(id.into_owned()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["_added", "_added_2"]);
    }

    #[test]
    fn test_float_marker_emits_discrete_heading() {
        // `[float]` (alias of `[discrete]`) emits a standalone Heading, not a
        // Section, and auto-generates/registers an id like discrete does.
        let input = "[float]\n== Floating Heading";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(
            events.iter().any(|e| matches!(e, Event::Start(Tag::Heading { level: 2 }))),
            "float must emit a Heading: {events:?}"
        );
        assert!(
            !events.iter().any(|e| matches!(e, Event::Start(Tag::Section { .. }))),
            "float must NOT emit a Section: {events:?}"
        );
        assert!(is_discrete_style("float") && is_discrete_style("discrete"));
        assert!(!is_discrete_style("preface"));
    }

    #[test]
    fn test_section_id_dots_become_separators() {
        let input = "== 0.3.0 Milestone Build";
        let events: Vec<_> = BlockScanner::new(input).collect();
        assert!(events.contains(&Event::Start(Tag::SectionTitle {
            level: 2,
            id: Cow::Owned("_0_3_0_milestone_build".into()),
        })));
    }

    #[test]
    fn test_section_id_resolves_attr_refs() {
        // Attribute references in a section title resolve before auto-id
        // generation (defined entries, author-line attrs, definition-time
        // resolution inside values); undefined refs stay literal and the
        // braces are dropped by id sanitization. Unset removes the key.
        let input = "= T\nKismet R. Lee\n:foo: Bar Baz\n:nested: x {foo} y\n\n== About {author}\n\n== Counts {foo}\n\n== Deep {nested}\n\n== With {undefined} ref\n\n:!foo:\n\n== Gone {foo}";
        let ids: Vec<String> = BlockScanner::new(input)
            .filter_map(|ev| match ev {
                Event::Start(Tag::SectionTitle { id, level }) if level > 0 => {
                    Some(id.into_owned())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            ids,
            vec![
                "_about_kismet_r_lee",
                "_counts_bar_baz",
                "_deep_x_bar_baz_y",
                "_with_undefined_ref",
                "_gone_foo",
            ]
        );
    }
}
