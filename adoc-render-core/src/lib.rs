//! Renderer-agnostic AsciiDoc semantics shared by `adoc_parser` event consumers.
//!
//! Backends (the HTML renderer, the ASG builder in the compatibility test
//! suite, future renderers) must agree on the parts of AsciiDoc that are not
//! syntax but document semantics: intrinsic attribute values, the resolution
//! precedence of `{name}` attribute references, and so on. This crate is the
//! single source of truth for those rules so consumers cannot drift apart.

use std::collections::HashMap;

/// A predefined (intrinsic) document attribute.
///
/// `text` is the semantic value — what the attribute *means*, independent of
/// any output format. `html` is the exact form Asciidoctor writes into HTML
/// output; byte-for-byte compatibility requires reproducing it verbatim:
/// non-ASCII characters appear as decimal character references, and `plus`/
/// `pp` use `&#43;` (Asciidoctor encodes the plus sign so the substituted
/// value cannot be re-read as passthrough syntax — note `cpp` stays a literal
/// `C++`, so this is per-attribute data, not a derivable encoding rule).
pub struct IntrinsicAttribute {
    pub name: &'static str,
    pub text: &'static str,
    pub html: &'static str,
}

/// Intrinsic attribute table, sorted by name.
pub const INTRINSIC_ATTRIBUTES: &[IntrinsicAttribute] = &[
    IntrinsicAttribute { name: "amp", text: "&", html: "&amp;" },
    IntrinsicAttribute { name: "apos", text: "'", html: "&#39;" },
    IntrinsicAttribute { name: "asterisk", text: "*", html: "*" },
    IntrinsicAttribute { name: "backslash", text: "\\", html: "\\" },
    IntrinsicAttribute { name: "backtick", text: "`", html: "`" },
    IntrinsicAttribute { name: "blank", text: "", html: "" },
    IntrinsicAttribute { name: "brvbar", text: "\u{00a6}", html: "&#166;" },
    IntrinsicAttribute { name: "caret", text: "^", html: "^" },
    IntrinsicAttribute { name: "cpp", text: "C++", html: "C++" },
    IntrinsicAttribute { name: "deg", text: "\u{00b0}", html: "&#176;" },
    IntrinsicAttribute { name: "empty", text: "", html: "" },
    IntrinsicAttribute { name: "endsb", text: "]", html: "]" },
    IntrinsicAttribute { name: "gt", text: ">", html: "&gt;" },
    IntrinsicAttribute { name: "ldquo", text: "\u{201c}", html: "&#8220;" },
    IntrinsicAttribute { name: "lsquo", text: "\u{2018}", html: "&#8216;" },
    IntrinsicAttribute { name: "lt", text: "<", html: "&lt;" },
    IntrinsicAttribute { name: "nbsp", text: "\u{00a0}", html: "&#160;" },
    IntrinsicAttribute { name: "plus", text: "+", html: "&#43;" },
    IntrinsicAttribute { name: "pp", text: "++", html: "&#43;&#43;" },
    IntrinsicAttribute { name: "quot", text: "\"", html: "&#34;" },
    IntrinsicAttribute { name: "rdquo", text: "\u{201d}", html: "&#8221;" },
    IntrinsicAttribute { name: "rsquo", text: "\u{2019}", html: "&#8217;" },
    IntrinsicAttribute { name: "sp", text: " ", html: " " },
    IntrinsicAttribute { name: "startsb", text: "[", html: "[" },
    IntrinsicAttribute { name: "tilde", text: "~", html: "~" },
    IntrinsicAttribute { name: "two-colons", text: "::", html: "::" },
    IntrinsicAttribute { name: "two-semicolons", text: ";;", html: ";;" },
    IntrinsicAttribute { name: "vbar", text: "|", html: "|" },
    IntrinsicAttribute { name: "wj", text: "\u{2060}", html: "&#8288;" },
    IntrinsicAttribute { name: "zwsp", text: "\u{200b}", html: "&#8203;" },
];

/// Look up an intrinsic attribute by (already lowercased) name.
pub fn intrinsic_attribute(name: &str) -> Option<&'static IntrinsicAttribute> {
    INTRINSIC_ATTRIBUTES.iter().find(|a| a.name == name)
}

/// Outcome of resolving an `{name}` attribute reference.
///
/// `Document`/`Fallback` borrow from the caller's storage; what to do with a
/// trailing `[...]` captured after the reference is consumer policy (the HTML
/// renderer re-parses `value[...]` together so URL-valued attributes form
/// link macros; plain-text consumers append the bracket text literally).
pub enum AttrRefOutcome<'a> {
    /// Resolved from document attributes.
    Document(&'a str),
    /// Resolved from the intrinsic table; consumer picks `text` or `html`.
    Intrinsic(&'static IntrinsicAttribute),
    /// `env-*` reference resolved from the process environment.
    Env(String),
    /// Unresolved, but the reference carried an explicit fallback.
    Fallback(&'a str),
    /// Unresolved — emit the `{name}` reference literally (the default
    /// `attribute-missing=skip` behavior, also used for `warn`).
    MissingSkip,
    /// Unresolved under `attribute-missing=drop`/`drop-line` — emit nothing.
    MissingDrop,
}

/// Resolve an attribute reference with Asciidoctor's precedence:
/// document attribute → intrinsic → `env-*` environment lookup → fallback →
/// `attribute-missing` handling. Lookups use the lowercased name.
///
/// An unresolved `env-*` reference without fallback always emits the literal
/// reference, regardless of `attribute-missing` (mirrors the established
/// renderer behavior).
pub fn resolve_attribute_reference<'a>(
    name: &str,
    doc_lookup: impl Fn(&str) -> Option<&'a str>,
    env_lookup: impl Fn(&str) -> Option<String>,
    fallback: Option<&'a str>,
    attribute_missing: Option<&str>,
) -> AttrRefOutcome<'a> {
    let lower_name = name.to_ascii_lowercase();
    if let Some(value) = doc_lookup(&lower_name) {
        return AttrRefOutcome::Document(value);
    }
    if let Some(attr) = intrinsic_attribute(&lower_name) {
        return AttrRefOutcome::Intrinsic(attr);
    }
    if let Some(env_name) = name.strip_prefix("env-") {
        if let Some(value) = env_lookup(env_name) {
            return AttrRefOutcome::Env(value);
        }
        return match fallback {
            Some(fb) => AttrRefOutcome::Fallback(fb),
            None => AttrRefOutcome::MissingSkip,
        };
    }
    if let Some(fb) = fallback {
        return AttrRefOutcome::Fallback(fb);
    }
    match attribute_missing {
        Some("drop") | Some("drop-line") => AttrRefOutcome::MissingDrop,
        _ => AttrRefOutcome::MissingSkip,
    }
}

/// Resolve `{name}` references inside a flat string value (attribute-entry
/// values, macro targets). Precedence: document attribute → intrinsic
/// (semantic `text` form) → leave the reference literal. Unterminated braces
/// pass through unchanged.
pub fn resolve_attr_refs_text<'a>(
    value: &str,
    doc_lookup: impl Fn(&str) -> Option<&'a str>,
) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let after_brace = &rest[start + 1..];
        if let Some(end) = after_brace.find('}') {
            let name = &after_brace[..end];
            let lower_name = name.to_ascii_lowercase();
            if let Some(resolved) = doc_lookup(&lower_name) {
                result.push_str(resolved);
            } else if let Some(attr) = intrinsic_attribute(&lower_name) {
                result.push_str(attr.text);
            } else {
                result.push('{');
                result.push_str(name);
                result.push('}');
            }
            rest = &after_brace[end + 1..];
        } else {
            result.push('{');
            rest = after_brace;
        }
    }
    result.push_str(rest);
    result
}

/// Reference text registered for an anchor, distinguishing how much
/// processing it still needs from the consumer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefText<'a> {
    /// Plain text — the consumer must escape it for its output format
    /// (section titles are accumulated unescaped).
    Plain(&'a str),
    /// Already in the consumer's output markup (e.g. a block title rendered
    /// to HTML, inline formatting included) — insert verbatim.
    Markup(&'a str),
}

/// Cross-reference lookup built from every id/title registry a renderer
/// accumulates while walking the event stream: section (TOC) entries, titled
/// blocks and bibliography entries. Encodes Asciidoctor's resolution
/// precedence for unlabeled xrefs and natural cross references.
#[derive(Default)]
pub struct XrefResolver<'a> {
    /// Anchor id -> link text. Key membership doubles as the "known id"
    /// check for href resolution.
    id_to_text: HashMap<&'a str, RefText<'a>>,
    /// Section title -> section id (case-sensitive natural cross reference).
    title_to_id: HashMap<&'a str, &'a str>,
}

impl<'a> XrefResolver<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a section. For duplicate ids the last registration wins; for
    /// duplicate titles the first section keeps the natural-xref mapping.
    pub fn add_section(&mut self, id: &'a str, title: &'a str) {
        self.id_to_text.insert(id, RefText::Plain(title));
        self.title_to_id.entry(title).or_insert(id);
    }

    /// Register a titled block or bibliography entry. Sections win over
    /// blocks on id collision, and earlier blocks win over later ones, so
    /// call this after all sections are registered.
    pub fn add_block(&mut self, id: &'a str, reftext: RefText<'a>) {
        self.id_to_text.entry(id).or_insert(reftext);
    }

    /// Link text for an unlabeled xref: a registered id (section/block/
    /// bibliography) wins, then a target matching a section title resolves to
    /// that section's text (natural cross reference).
    pub fn link_text(&self, target: &str) -> Option<RefText<'a>> {
        if let Some(&text) = self.id_to_text.get(target) {
            return Some(text);
        }
        self.title_to_id
            .get(target)
            .and_then(|id| self.id_to_text.get(id))
            .copied()
    }

    /// Anchor id for an internal xref href. Precedence matches Asciidoctor:
    /// a target that is itself a registered id stays literal; otherwise a
    /// target exactly matching a section title (case-sensitive) becomes that
    /// section's id (natural cross reference); else it stays literal.
    pub fn href_id(&self, target: &'a str) -> &'a str {
        if self.id_to_text.contains_key(target) {
            target
        } else {
            self.title_to_id.get(target).copied().unwrap_or(target)
        }
    }
}

/// Default xreflabel for an unresolved internal reference: Asciidoctor wraps
/// the target id in square brackets.
pub fn unresolved_xref_label(target: &str) -> String {
    format!("[{target}]")
}

/// True when an xref target refers to another document (a path containing a
/// dot) rather than an in-document anchor.
pub fn is_interdoc_xref_target(target: &str) -> bool {
    target.contains('.') && !target.starts_with('#')
}

/// Rewrite an inter-document xref target for HTML conversion: the `.adoc`
/// extension becomes `.html` (Asciidoctor's default `outfilesuffix`),
/// preserving a `#fragment`; other targets pass through unchanged. The
/// rewritten path doubles as the auto-generated link text when the xref has
/// no explicit label.
pub fn interdoc_xref_href(target: &str) -> String {
    if let Some(base) = target.strip_suffix(".adoc") {
        format!("{base}.html")
    } else if let Some((file_part, anchor)) = target.split_once('#')
        && let Some(base) = file_part.strip_suffix(".adoc")
    {
        format!("{base}.html#{anchor}")
    } else {
        target.to_string()
    }
}

/// A section heading collected while walking the event stream. Doubles as
/// the section registry for xref resolution (see [`XrefResolver`]) and as
/// the source of TOC entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    /// Heading level as emitted by the parser (1 = book part / body sect0,
    /// 2 = top-level section). Special sections are coerced (`[preface]` +
    /// `= T` arrives as level 2), so this is the DISPLAY level — it drives
    /// the `sectlevelN` class, not the nesting.
    pub level: u8,
    /// Tree depth of the section (1 = child of the document). Drives the
    /// TOC nesting: a coerced special section (level 2) is still a sibling
    /// of a part (level 1) when both sit at depth 1.
    pub depth: u8,
    /// Anchor id of the section.
    pub id: String,
    /// Title as plain text (unescaped), including any number prefix or
    /// caption the consumer prepended while accumulating it.
    pub title: String,
}

/// Depth math for `toclevels` is anchored at this heading level (level 1 in
/// the body is a book part / article sect0, displayed at TOC depth 1 too).
const TOC_MIN_LEVEL: u8 = 2;

/// Asciidoctor's default `toc-title`.
pub const DEFAULT_TOC_TITLE: &str = "Table of Contents";

/// One structural step of a TOC layout produced by [`TocBuilder::toc_steps`].
/// The consumer maps each step to its output format (e.g. `<ul>`/`<li>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TocStep<'a> {
    /// Open a nested list for sections at `level` (the display depth is
    /// `level - 1`, e.g. HTML's `sectlevel1` class for level-2 sections).
    EnterLevel(u8),
    /// Emit one entry. The item stays open so a deeper list can nest inside
    /// it; it is closed by a later `CloseItem` or `LeaveLevel`.
    Item(&'a TocEntry),
    /// Close the current item before a sibling at the same level.
    CloseItem,
    /// Close the current item and its enclosing list.
    LeaveLevel,
}

/// Collects section entries in document order and lays out the TOC
/// structure. The semantics (which levels are visible, how lists nest) live
/// here; the markup is the consumer's.
#[derive(Default)]
pub struct TocBuilder {
    entries: Vec<TocEntry>,
}

impl TocBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: TocEntry) {
        self.entries.push(entry);
    }

    /// All collected sections in document order.
    pub fn entries(&self) -> &[TocEntry] {
        &self.entries
    }

    /// Structural steps for a TOC limited to `toc_levels` levels of depth
    /// (Asciidoctor's `toclevels`, default 2): section levels 1 (book parts /
    /// body sect0) through `toc_levels + 1` are included. Lists are opened
    /// only for levels that actually occur — a skipped level nests its list
    /// directly, mirroring Asciidoctor's `convert_outline` recursion over the
    /// real section tree. An empty result means no entry is in range and no
    /// TOC should be emitted at all.
    pub fn toc_steps(&self, toc_levels: u8) -> Vec<TocStep<'_>> {
        let max_level = TOC_MIN_LEVEL as u16 + toc_levels as u16 - 1;
        let mut steps = Vec::new();
        // Stack of tree depths with an open list.
        let mut open: Vec<u8> = Vec::new();
        let visible = self
            .entries
            .iter()
            .filter(|e| (e.level as u16) <= max_level);
        for entry in visible {
            while open.last().is_some_and(|&top| top > entry.depth) {
                steps.push(TocStep::LeaveLevel);
                open.pop();
            }
            if open.last() == Some(&entry.depth) {
                steps.push(TocStep::CloseItem);
            } else {
                steps.push(TocStep::EnterLevel(entry.level));
                open.push(entry.depth);
            }
            steps.push(TocStep::Item(entry));
        }
        // Close all levels left open
        for _ in open {
            steps.push(TocStep::LeaveLevel);
        }
        steps
    }
}

/// Counters behind Asciidoctor's `sectnums` numbering and appendix captions.
/// Whether numbering applies at all (the `sectnums` attribute, caption
/// suppression for special sections) is the consumer's call; this type only
/// owns the counter state and the prefix format.
#[derive(Default)]
pub struct SectionNumberer {
    /// Per-level section counters; indices 2..=5 are used.
    counters: [u32; 6],
    /// Letter numeral of an appendix open at that level. While set, it
    /// stands in for the arabic counter in descendants' sectnum chains
    /// (`A.1.`, `1.A.1.`); the level's own counter stays untouched, so a
    /// regular sibling after the appendix continues the arabic sequence
    /// (Asciidoctor: appendices don't consume the parent's ordinal).
    appendix_letters: [Option<char>; 6],
    /// Document-global appendix counter (`appendix-number`): letters keep
    /// advancing across parts and nesting levels.
    appendix_counter: u8,
}

impl SectionNumberer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number prefix (`"1.2. "`, trailing space included) for the next
    /// section at `level`: bumps that level's counter and resets all deeper
    /// levels. Returns `None` outside the numbered range (levels 2 through
    /// 5), leaving the counters untouched. Open-appendix levels in the
    /// ancestor chain contribute their letter (`A.1. `).
    pub fn number_prefix(&mut self, level: u8) -> Option<String> {
        if !(2..=5).contains(&level) {
            return None;
        }
        let lvl = level as usize;
        self.counters[lvl] += 1;
        self.appendix_letters[lvl] = None;
        for l in (lvl + 1)..6 {
            self.counters[l] = 0;
            self.appendix_letters[l] = None;
        }
        let mut prefix = String::new();
        for l in 2..=lvl {
            if !prefix.is_empty() {
                prefix.push('.');
            }
            match self.appendix_letters[l] {
                Some(letter) => prefix.push(letter),
                None => prefix.push_str(&self.counters[l].to_string()),
            }
        }
        prefix.push_str(". ");
        Some(prefix)
    }

    /// Caption prefix for the next appendix section at `level`. The letter
    /// comes from the document-global counter; `caption` is the
    /// `appendix-caption` attribute value (`Some` → `"{caption} {L}: "`,
    /// even when empty; `None` (unset) → `"{L}. "` — both per Asciidoctor's
    /// `assign_numeral`). The parent ordinal at `level` is not consumed;
    /// deeper counters reset so subsections number `A.1.`, `A.2.`, …
    pub fn appendix_prefix(&mut self, level: u8, caption: Option<&str>) -> String {
        self.appendix_counter += 1;
        let letter = (b'A' + (self.appendix_counter - 1).min(25)) as char;
        let lvl = (level as usize).min(5);
        if (2..=5).contains(&lvl) {
            self.appendix_letters[lvl] = Some(letter);
            for l in (lvl + 1)..6 {
                self.counters[l] = 0;
                self.appendix_letters[l] = None;
            }
        }
        match caption {
            Some(caption) => format!("{caption} {letter}: "),
            None => format!("{letter}. "),
        }
    }

    /// Reset all descendant ordinals (and open-appendix letters). Asciidoctor
    /// numbers sections from a per-parent ordinal, so an article body sect0
    /// starts its children at 1 again. Book parts must NOT call this: the
    /// chapter numeral is a document-global counter (`chapter-number`),
    /// sequential across parts. The appendix letter counter is likewise
    /// document-global and survives the reset.
    pub fn reset_descendant_ordinals(&mut self) {
        for l in 2..6 {
            self.counters[l] = 0;
            self.appendix_letters[l] = None;
        }
    }
}

/// Kinds of titled blocks that carry a numbered caption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionKind {
    Figure,
    Table,
    Example,
}

/// How a titled block's caption prefix renders, as decided by
/// [`CaptionCounters::caption_prefix`]. Text is plain — the consumer escapes
/// it and formats `Numbered` in its own markup (`"Label N. "` in HTML).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionPrefix<'a> {
    /// No prefix: suppressed via `caption=""` or an unset caption document
    /// attribute (`:figure-caption!:`).
    None,
    /// A block-level `caption="…"` override, emitted verbatim in place of
    /// the numbered label.
    Custom(&'a str),
    /// Numbered prefix: the caption label (e.g. `"Figure"`) and this block's
    /// number.
    Numbered { label: &'a str, number: usize },
}

/// Per-kind counters behind Asciidoctor's `Figure N.` / `Table N.` /
/// `Example N.` caption numbering. Where the label comes from (the
/// `figure-caption`-style document attributes, their defaults and unsetting)
/// is the consumer's call; this type owns the counter state and the
/// prefix-selection rule.
#[derive(Default)]
pub struct CaptionCounters {
    figure: usize,
    table: usize,
    example: usize,
}

impl CaptionCounters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decide the caption prefix for the next titled block of `kind`.
    ///
    /// `caption_attr` is the block-level `caption=` named attribute
    /// (empty string suppresses the prefix, any other value replaces it
    /// verbatim); `doc_label` is the resolved caption label, `None` when the
    /// corresponding document attribute is unset.
    ///
    /// Counter semantics differ by kind: figure and table bump their counter
    /// only when a `Numbered` prefix is actually produced, while example
    /// bumps for every titled block — even when `caption=` overrides or
    /// suppresses the text.
    pub fn caption_prefix<'a>(
        &mut self,
        kind: CaptionKind,
        caption_attr: Option<&'a str>,
        doc_label: Option<&'a str>,
    ) -> CaptionPrefix<'a> {
        let counter = match kind {
            CaptionKind::Figure => &mut self.figure,
            CaptionKind::Table => &mut self.table,
            CaptionKind::Example => &mut self.example,
        };
        if kind == CaptionKind::Example {
            *counter += 1;
        }
        match caption_attr {
            Some("") => CaptionPrefix::None,
            Some(prefix) => CaptionPrefix::Custom(prefix),
            None => match doc_label {
                Some(label) => {
                    if kind != CaptionKind::Example {
                        *counter += 1;
                    }
                    CaptionPrefix::Numbered { label, number: *counter }
                }
                None => CaptionPrefix::None,
            },
        }
    }
}

/// A footnote definition collected while walking the event stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footnote {
    /// 1-based document-order number.
    pub number: usize,
    /// Anchor id of a named footnote (`footnote:id[text]`).
    pub id: Option<String>,
    /// Footnote text as plain text (the consumer escapes when rendering).
    pub text: String,
}

/// Footnote numbering and the named-footnote registry: definitions are
/// numbered in document order, named definitions can be referenced again by
/// id (`footnote:id[]`), and the collected list drives the consumer's
/// footnote section at the end of the document.
#[derive(Default)]
pub struct FootnoteRegistry {
    footnotes: Vec<Footnote>,
    by_id: HashMap<String, usize>,
}

impl FootnoteRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the next footnote definition and return its number. A named
    /// footnote also registers its id for later references; redefining an id
    /// keeps both definitions and points the id at the newest one.
    pub fn define(&mut self, id: Option<&str>, text: &str) -> usize {
        let number = self.footnotes.len() + 1;
        if let Some(id) = id {
            self.by_id.insert(id.to_string(), number);
        }
        self.footnotes.push(Footnote {
            number,
            id: id.map(str::to_string),
            text: text.to_string(),
        });
        number
    }

    /// Number of the named footnote `id` refers to, if defined.
    pub fn lookup(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    /// All definitions in document order.
    pub fn footnotes(&self) -> &[Footnote] {
        &self.footnotes
    }

    pub fn is_empty(&self) -> bool {
        self.footnotes.is_empty()
    }
}

/// An author parsed from the document header's author line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub fullname: String,
    pub firstname: String,
    pub middlename: String,
    pub lastname: String,
    pub initials: String,
    /// Email address (or other contact), empty when absent.
    pub address: String,
}

impl Author {
    /// Parse an `:author:` attribute-entry value into author metadata —
    /// Asciidoctor's "names only" mode (`process_authors author_line, true,
    /// false`): the value is split into at most three whitespace-separated
    /// segments (firstname / middlename / rest-as-lastname), underscores in
    /// each segment become spaces, the initials are the first character of
    /// each segment, and the fullname is recomposed from the segments (so
    /// `Mara_Moss Wirribi` yields the fullname `Mara Moss Wirribi`). No email
    /// is ever extracted — `address` is always empty.
    pub fn from_attribute_value(value: &str) -> Author {
        let mut words = value.split_whitespace();
        let seg1 = words.next().unwrap_or("");
        let seg2 = words.next();
        let rest: Vec<&str> = words.collect();
        let seg3 = if rest.is_empty() { None } else { Some(rest.join(" ")) };

        let tr = |s: &str| s.replace('_', " ");
        let initial = |s: &str| s.chars().next().map(String::from).unwrap_or_default();

        let firstname = tr(seg1);
        let (middlename, lastname) = match (seg2, &seg3) {
            (Some(s2), Some(s3)) => (tr(s2), tr(s3)),
            (Some(s2), None) => (String::new(), tr(s2)),
            _ => (String::new(), String::new()),
        };

        let mut fullname = firstname.clone();
        let mut initials = initial(&firstname);
        if !middlename.is_empty() {
            fullname.push(' ');
            fullname.push_str(&middlename);
            initials.push_str(&initial(&middlename));
        }
        if !lastname.is_empty() {
            fullname.push(' ');
            fullname.push_str(&lastname);
            initials.push_str(&initial(&lastname));
        }

        Author {
            fullname,
            firstname,
            middlename,
            lastname,
            initials,
            address: String::new(),
        }
    }
}

/// Authors collected from the document header, plus the attribute-naming
/// rule behind `{author}` / `{author_2}` / … references.
#[derive(Default)]
pub struct AuthorRegistry {
    authors: Vec<Author>,
}

impl AuthorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// HTML-id suffix for the author at `index`: the first author's detail
    /// spans are unsuffixed (`id="author"`, `id="email"`), subsequent authors
    /// are numbered from 2 without a separator (`author2`, `email3`, …).
    pub fn id_suffix(index: usize) -> String {
        if index == 0 { String::new() } else { (index + 1).to_string() }
    }

    /// Document-attribute-name suffix for the author at `index`: the first
    /// author's attributes are unsuffixed (`author`, `email`), subsequent
    /// authors are numbered from 2 with an underscore (`author_2`,
    /// `email_3`, …) — distinct from the separator-less [`Self::id_suffix`]
    /// used for the detail-span HTML ids.
    pub fn name_suffix(index: usize) -> String {
        if index == 0 { String::new() } else { format!("_{}", index + 1) }
    }

    /// Register the next author and return the document-attribute entries it
    /// implies: `author{suffix}`, `firstname{suffix}`, `lastname{suffix}`,
    /// `authorinitials{suffix}` always, `middlename{suffix}` and
    /// `email{suffix}` only when non-empty.
    pub fn add(&mut self, author: Author) -> Vec<(String, String)> {
        let suffix = Self::name_suffix(self.authors.len());
        let mut entries = vec![
            (format!("author{suffix}"), author.fullname.clone()),
            (format!("firstname{suffix}"), author.firstname.clone()),
        ];
        if !author.middlename.is_empty() {
            entries.push((format!("middlename{suffix}"), author.middlename.clone()));
        }
        entries.push((format!("lastname{suffix}"), author.lastname.clone()));
        entries.push((format!("authorinitials{suffix}"), author.initials.clone()));
        if !author.address.is_empty() {
            entries.push((format!("email{suffix}"), author.address.clone()));
        }
        self.authors.push(author);
        entries
    }

    /// All authors in document order.
    pub fn authors(&self) -> &[Author] {
        &self.authors
    }

    pub fn is_empty(&self) -> bool {
        self.authors.is_empty()
    }
}

/// The revision line from the document header. Components are plain text
/// (the consumer escapes when rendering).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Revision {
    pub version: String,
    pub date: String,
    pub remark: String,
}

impl Revision {
    /// Document-attribute entries this revision implies (`revnumber`,
    /// `revdate`, `revremark`); empty components contribute nothing.
    pub fn attr_entries(&self) -> Vec<(&'static str, &str)> {
        let mut entries = Vec::new();
        if !self.version.is_empty() {
            entries.push(("revnumber", self.version.as_str()));
        }
        if !self.date.is_empty() {
            entries.push(("revdate", self.date.as_str()));
        }
        if !self.remark.is_empty() {
            entries.push(("revremark", self.remark.as_str()));
        }
        entries
    }

    /// Version as displayed in the header details: Asciidoctor strips one
    /// leading `v`/`V` marker (`v1.0` → `1.0`). Revision-line versions
    /// arrive pre-stripped from the parser (which strips any non-digit
    /// prefix), so this only changes explicitly set version strings.
    pub fn display_version(&self) -> &str {
        self.version
            .strip_prefix('v')
            .or_else(|| self.version.strip_prefix('V'))
            .unwrap_or(&self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsic_table_sorted_and_consistent() {
        for pair in INTRINSIC_ATTRIBUTES.windows(2) {
            assert!(pair[0].name < pair[1].name, "table must stay sorted: {}", pair[1].name);
        }
        // Spot-check the two columns agree semantically.
        let nbsp = intrinsic_attribute("nbsp").unwrap();
        assert_eq!(nbsp.text, "\u{00a0}");
        assert_eq!(nbsp.html, "&#160;");
        let cpp = intrinsic_attribute("cpp").unwrap();
        assert_eq!(cpp.text, "C++");
        assert_eq!(cpp.html, "C++");
        let pp = intrinsic_attribute("pp").unwrap();
        assert_eq!(pp.text, "++");
        assert_eq!(pp.html, "&#43;&#43;");
        assert!(intrinsic_attribute("unknown").is_none());
    }

    #[test]
    fn author_from_attribute_value_names_only() {
        // Three segments: first/middle/last, initials from each.
        let a = Author::from_attribute_value("Mary Sue Jones");
        assert_eq!(a.fullname, "Mary Sue Jones");
        assert_eq!((a.firstname.as_str(), a.middlename.as_str(), a.lastname.as_str()), ("Mary", "Sue", "Jones"));
        assert_eq!(a.initials, "MSJ");
        assert!(a.address.is_empty());
        // Four+ words: the remainder joins into the lastname.
        let a = Author::from_attribute_value("A B C D");
        assert_eq!((a.lastname.as_str(), a.initials.as_str()), ("C D", "ABC"));
        // Underscores become spaces; the fullname is recomposed.
        let a = Author::from_attribute_value("Mara_Moss Wirribi");
        assert_eq!(a.fullname, "Mara Moss Wirribi");
        assert_eq!((a.firstname.as_str(), a.lastname.as_str(), a.initials.as_str()), ("Mara Moss", "Wirribi", "MW"));
        assert!(a.middlename.is_empty());
        // Single name: firstname only.
        let a = Author::from_attribute_value("Mary");
        assert_eq!((a.fullname.as_str(), a.initials.as_str()), ("Mary", "M"));
        assert!(a.middlename.is_empty() && a.lastname.is_empty());
    }

    #[test]
    fn attr_ref_precedence() {
        let doc = |n: &str| (n == "x").then_some("doc-value");
        let no_env = |_: &str| None;
        // Document attribute wins over intrinsic.
        assert!(matches!(
            resolve_attribute_reference("x", doc, no_env, Some("fb"), None),
            AttrRefOutcome::Document("doc-value")
        ));
        // Name is lowercased for lookup.
        assert!(matches!(
            resolve_attribute_reference("X", doc, no_env, None, None),
            AttrRefOutcome::Document("doc-value")
        ));
        // Intrinsic before fallback.
        assert!(matches!(
            resolve_attribute_reference("nbsp", |_| None, no_env, Some("fb"), None),
            AttrRefOutcome::Intrinsic(IntrinsicAttribute { name: "nbsp", .. })
        ));
        // Fallback before missing.
        assert!(matches!(
            resolve_attribute_reference("nope", |_| None, no_env, Some("fb"), None),
            AttrRefOutcome::Fallback("fb")
        ));
        // attribute-missing modes.
        assert!(matches!(
            resolve_attribute_reference("nope", |_| None, no_env, None, None),
            AttrRefOutcome::MissingSkip
        ));
        assert!(matches!(
            resolve_attribute_reference("nope", |_| None, no_env, None, Some("drop")),
            AttrRefOutcome::MissingDrop
        ));
        assert!(matches!(
            resolve_attribute_reference("nope", |_| None, no_env, None, Some("drop-line")),
            AttrRefOutcome::MissingDrop
        ));
        assert!(matches!(
            resolve_attribute_reference("nope", |_| None, no_env, None, Some("warn")),
            AttrRefOutcome::MissingSkip
        ));
    }

    #[test]
    fn attr_ref_env() {
        let env = |n: &str| (n == "HOME").then(|| "/home/user".to_string());
        assert!(matches!(
            resolve_attribute_reference("env-HOME", |_| None, env, None, None),
            AttrRefOutcome::Env(ref v) if v == "/home/user"
        ));
        // env miss → fallback if present.
        assert!(matches!(
            resolve_attribute_reference("env-MISSING", |_| None, env, Some("fb"), None),
            AttrRefOutcome::Fallback("fb")
        ));
        // env miss without fallback emits the literal reference even under drop.
        assert!(matches!(
            resolve_attribute_reference("env-MISSING", |_| None, env, None, Some("drop")),
            AttrRefOutcome::MissingSkip
        ));
        // A document attribute named env-* still wins.
        let doc = |n: &str| (n == "env-home").then_some("doc");
        assert!(matches!(
            resolve_attribute_reference("env-HOME", doc, env, None, None),
            AttrRefOutcome::Document("doc")
        ));
    }

    #[test]
    fn xref_resolver_precedence() {
        let mut r = XrefResolver::new();
        r.add_section("_intro", "Introduction");
        r.add_section("_dup", "Shared Title");
        r.add_section("_dup2", "Shared Title"); // natural xref: first section keeps the title
        r.add_block("_intro", RefText::Markup("<em>loser</em>")); // section wins on id collision
        r.add_block("_fig", RefText::Markup("<em>Figure</em>"));
        r.add_block("_fig", RefText::Markup("ignored")); // first block wins

        // Registered id resolves to its text.
        assert_eq!(r.link_text("_intro"), Some(RefText::Plain("Introduction")));
        assert_eq!(r.link_text("_fig"), Some(RefText::Markup("<em>Figure</em>")));
        // Natural xref: a target matching a section title resolves to that section.
        assert_eq!(r.link_text("Introduction"), Some(RefText::Plain("Introduction")));
        assert_eq!(r.link_text("Shared Title"), Some(RefText::Plain("Shared Title")));
        assert_eq!(r.link_text("unknown"), None);

        // Href: known id stays literal, section title becomes its id, else literal.
        assert_eq!(r.href_id("_intro"), "_intro");
        assert_eq!(r.href_id("Introduction"), "_intro");
        assert_eq!(r.href_id("Shared Title"), "_dup");
        assert_eq!(r.href_id("unknown"), "unknown");

        assert_eq!(unresolved_xref_label("missing-id"), "[missing-id]");
    }

    #[test]
    fn interdoc_xref_targets() {
        assert!(is_interdoc_xref_target("other.adoc"));
        assert!(is_interdoc_xref_target("docs/guide.html"));
        assert!(!is_interdoc_xref_target("_section"));
        assert!(!is_interdoc_xref_target("#frag.with.dot"));

        assert_eq!(interdoc_xref_href("other.adoc"), "other.html");
        assert_eq!(interdoc_xref_href("dir/other.adoc#sec"), "dir/other.html#sec");
        assert_eq!(interdoc_xref_href("page.html"), "page.html");
        assert_eq!(interdoc_xref_href("page.html#sec"), "page.html#sec");
    }

    #[test]
    fn toc_structure_steps() {
        let mut b = TocBuilder::new();
        for (level, depth, id) in [
            (2, 1, "a"),
            (3, 2, "a1"),
            (3, 2, "a2"),
            (5, 3, "deep"),
            (2, 1, "b"),
        ] {
            b.push(TocEntry { level, depth, id: id.to_string(), title: id.to_uppercase() });
        }
        assert_eq!(b.entries().len(), 5);

        // toclevels=4 → levels 2..=5 visible; nesting follows tree depth —
        // the level-5 child opens ONE list (labelled with its own level),
        // returning to depth 1 closes two.
        let ids: Vec<String> = b
            .toc_steps(4)
            .iter()
            .map(|s| match s {
                TocStep::EnterLevel(l) => format!(">{l}"),
                TocStep::Item(e) => e.id.clone(),
                TocStep::CloseItem => "/i".to_string(),
                TocStep::LeaveLevel => "<".to_string(),
            })
            .collect();
        assert_eq!(
            ids,
            [">2", "a", ">3", "a1", "/i", "a2", ">5", "deep",
             "<", "<", "/i", "b", "<"]
        );

        // Book parts: a coerced special section (level 2, depth 1) is a
        // SIBLING of a part (level 1, depth 1); the part's chapters nest.
        let mut book = TocBuilder::new();
        for (level, depth, id) in [
            (2, 1, "colophon"),
            (1, 1, "part1"),
            (2, 2, "ch1"),
            (2, 1, "appendix"),
            (3, 2, "basics"),
        ] {
            book.push(TocEntry { level, depth, id: id.to_string(), title: id.to_uppercase() });
        }
        let ids: Vec<String> = book
            .toc_steps(2)
            .iter()
            .map(|s| match s {
                TocStep::EnterLevel(l) => format!(">{l}"),
                TocStep::Item(e) => e.id.clone(),
                TocStep::CloseItem => "/i".to_string(),
                TocStep::LeaveLevel => "<".to_string(),
            })
            .collect();
        assert_eq!(
            ids,
            [">2", "colophon", "/i", "part1", ">2", "ch1", "<", "/i",
             "appendix", ">3", "basics", "<", "<"]
        );

        // Default toclevels=2 → only levels 2..=3.
        let visible: Vec<&str> = b
            .toc_steps(2)
            .iter()
            .filter_map(|s| match s {
                TocStep::Item(e) => Some(e.id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(visible, ["a", "a1", "a2", "b"]);

        // toclevels=0 → nothing in range, no steps at all.
        assert!(b.toc_steps(0).is_empty());
        assert!(TocBuilder::new().toc_steps(2).is_empty());
    }

    #[test]
    fn section_numbering() {
        let mut n = SectionNumberer::new();
        assert_eq!(n.number_prefix(2).as_deref(), Some("1. "));
        assert_eq!(n.number_prefix(3).as_deref(), Some("1.1. "));
        assert_eq!(n.number_prefix(4).as_deref(), Some("1.1.1. "));
        assert_eq!(n.number_prefix(3).as_deref(), Some("1.2. "));
        // Deeper counter was reset by the level-3 bump.
        assert_eq!(n.number_prefix(4).as_deref(), Some("1.2.1. "));
        assert_eq!(n.number_prefix(2).as_deref(), Some("2. "));
        assert_eq!(n.number_prefix(3).as_deref(), Some("2.1. "));
        // Outside the numbered range: no prefix, counters untouched.
        assert_eq!(n.number_prefix(1), None);
        assert_eq!(n.number_prefix(6), None);
        assert_eq!(n.number_prefix(3).as_deref(), Some("2.2. "));

        // Appendix at level 2: caption forms, letter chain in subsections,
        // and the level-2 arabic ordinal is NOT consumed.
        assert_eq!(n.appendix_prefix(2, Some("Appendix")), "Appendix A: ");
        assert_eq!(n.number_prefix(3).as_deref(), Some("A.1. "));
        assert_eq!(n.number_prefix(4).as_deref(), Some("A.1.1. "));
        assert_eq!(n.number_prefix(3).as_deref(), Some("A.2. "));
        assert_eq!(n.appendix_prefix(2, Some("Exhibit")), "Exhibit B: ");
        // Empty caption attribute keeps the format (leading space, Asciidoctor parity).
        assert_eq!(n.appendix_prefix(2, Some("")), " C: ");
        // Unset caption attribute → bare numeral form.
        assert_eq!(n.appendix_prefix(2, None), "D. ");
        // Regular sibling after the appendices continues from 2 (was "2.2.").
        assert_eq!(n.number_prefix(2).as_deref(), Some("3. "));

        // Nested appendix (level 3) keeps ancestors' numerals in descendants.
        assert_eq!(n.appendix_prefix(3, Some("Appendix")), "Appendix E: ");
        assert_eq!(n.number_prefix(4).as_deref(), Some("3.E.1. "));
        // Sibling section at the appendix level clears the letter.
        assert_eq!(n.number_prefix(3).as_deref(), Some("3.1. "));
    }

    #[test]
    fn caption_counters() {
        let mut c = CaptionCounters::new();
        // Figure/table: numbered prefix bumps the counter…
        assert_eq!(
            c.caption_prefix(CaptionKind::Figure, None, Some("Figure")),
            CaptionPrefix::Numbered { label: "Figure", number: 1 }
        );
        // …but caption="" / caption=X / unset doc label do NOT.
        assert_eq!(c.caption_prefix(CaptionKind::Figure, Some(""), Some("Figure")), CaptionPrefix::None);
        assert_eq!(
            c.caption_prefix(CaptionKind::Figure, Some("Fig X: "), Some("Figure")),
            CaptionPrefix::Custom("Fig X: ")
        );
        assert_eq!(c.caption_prefix(CaptionKind::Figure, None, None), CaptionPrefix::None);
        assert_eq!(
            c.caption_prefix(CaptionKind::Figure, None, Some("Рисунок")),
            CaptionPrefix::Numbered { label: "Рисунок", number: 2 }
        );
        // Counters are independent per kind.
        assert_eq!(
            c.caption_prefix(CaptionKind::Table, None, Some("Table")),
            CaptionPrefix::Numbered { label: "Table", number: 1 }
        );
        // Example bumps for every titled block, even under caption= override.
        assert_eq!(
            c.caption_prefix(CaptionKind::Example, None, Some("Example")),
            CaptionPrefix::Numbered { label: "Example", number: 1 }
        );
        assert_eq!(c.caption_prefix(CaptionKind::Example, Some(""), Some("Example")), CaptionPrefix::None);
        assert_eq!(
            c.caption_prefix(CaptionKind::Example, None, Some("Example")),
            CaptionPrefix::Numbered { label: "Example", number: 3 }
        );
    }

    #[test]
    fn footnote_registry() {
        let mut f = FootnoteRegistry::new();
        assert!(f.is_empty());
        assert_eq!(f.define(None, "first"), 1);
        assert_eq!(f.define(Some("note"), "second"), 2);
        assert_eq!(f.define(None, "third"), 3);
        assert_eq!(f.lookup("note"), Some(2));
        assert_eq!(f.lookup("unknown"), None);
        // Redefinition keeps both entries; the id points at the newest.
        assert_eq!(f.define(Some("note"), "fourth"), 4);
        assert_eq!(f.lookup("note"), Some(4));
        let texts: Vec<&str> = f.footnotes().iter().map(|n| n.text.as_str()).collect();
        assert_eq!(texts, ["first", "second", "third", "fourth"]);
        assert_eq!(f.footnotes()[1].id.as_deref(), Some("note"));
        assert_eq!(f.footnotes()[0].id, None);
        assert!(!f.is_empty());
    }

    #[test]
    fn author_registry_attr_entries() {
        let mut reg = AuthorRegistry::new();
        assert!(reg.is_empty());
        // First author: unsuffixed names, middlename/email skipped when empty.
        let entries = reg.add(Author {
            fullname: "John Doe".into(),
            firstname: "John".into(),
            middlename: String::new(),
            lastname: "Doe".into(),
            initials: "JD".into(),
            address: String::new(),
        });
        assert_eq!(
            entries,
            [
                ("author".to_string(), "John Doe".to_string()),
                ("firstname".to_string(), "John".to_string()),
                ("lastname".to_string(), "Doe".to_string()),
                ("authorinitials".to_string(), "JD".to_string()),
            ]
        );
        // Second author: `_2`-suffixed attribute names, full set when all
        // components present.
        let entries = reg.add(Author {
            fullname: "Ann B. Lee".into(),
            firstname: "Ann".into(),
            middlename: "B.".into(),
            lastname: "Lee".into(),
            initials: "ABL".into(),
            address: "ann@example.com".into(),
        });
        assert_eq!(
            entries,
            [
                ("author_2".to_string(), "Ann B. Lee".to_string()),
                ("firstname_2".to_string(), "Ann".to_string()),
                ("middlename_2".to_string(), "B.".to_string()),
                ("lastname_2".to_string(), "Lee".to_string()),
                ("authorinitials_2".to_string(), "ABL".to_string()),
                ("email_2".to_string(), "ann@example.com".to_string()),
            ]
        );
        assert_eq!(reg.authors().len(), 2);
        assert_eq!(reg.authors()[1].address, "ann@example.com");
        // HTML-id suffix stays separator-less; attribute-name suffix gets `_`.
        assert_eq!(AuthorRegistry::id_suffix(2), "3");
        assert_eq!(AuthorRegistry::name_suffix(2), "_3");
    }

    #[test]
    fn revision_entries_and_display() {
        let rev = Revision {
            version: "v8.3".into(),
            date: "2024-01-01".into(),
            remark: String::new(),
        };
        assert_eq!(
            rev.attr_entries(),
            [("revnumber", "v8.3"), ("revdate", "2024-01-01")]
        );
        assert_eq!(rev.display_version(), "8.3");
        let rev = Revision { version: "V2".into(), ..Default::default() };
        assert_eq!(rev.display_version(), "2");
        assert_eq!(rev.attr_entries(), [("revnumber", "V2")]);
        let rev = Revision { version: "1.0".into(), ..Default::default() };
        assert_eq!(rev.display_version(), "1.0");
        assert!(Revision::default().attr_entries().is_empty());
    }

    #[test]
    fn resolve_refs_in_string() {
        let doc = |n: &str| (n == "name").then_some("World");
        assert_eq!(resolve_attr_refs_text("Hello {name}!", doc), "Hello World!");
        assert_eq!(resolve_attr_refs_text("a{nbsp}b", doc), "a\u{00a0}b");
        assert_eq!(resolve_attr_refs_text("{missing} stays", doc), "{missing} stays");
        assert_eq!(resolve_attr_refs_text("brace { only", doc), "brace { only");
        assert_eq!(resolve_attr_refs_text("{NAME}", doc), "World");
    }
}
