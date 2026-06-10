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
    /// Heading level as emitted by the parser (1 = document title,
    /// 2 = top-level section).
    pub level: u8,
    /// Anchor id of the section.
    pub id: String,
    /// Title as plain text (unescaped), including any number prefix or
    /// caption the consumer prepended while accumulating it.
    pub title: String,
}

/// Sections eligible for the TOC start at this heading level (level 1 is
/// the document title).
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
    /// (Asciidoctor's `toclevels`, default 2): section levels 2 through
    /// `toc_levels + 1` are included. An empty result means no entry is in
    /// range and no TOC should be emitted at all.
    pub fn toc_steps(&self, toc_levels: u8) -> Vec<TocStep<'_>> {
        let max_level = TOC_MIN_LEVEL as u16 + toc_levels as u16 - 1;
        let mut steps = Vec::new();
        let mut current_level = TOC_MIN_LEVEL - 1;
        let visible = self
            .entries
            .iter()
            .filter(|e| e.level >= TOC_MIN_LEVEL && (e.level as u16) <= max_level);
        for entry in visible {
            if entry.level > current_level {
                // Going deeper — open new list(s)
                while current_level < entry.level {
                    current_level += 1;
                    steps.push(TocStep::EnterLevel(current_level));
                }
            } else if entry.level < current_level {
                // Going shallower — close nested lists, then the sibling item
                while current_level > entry.level {
                    steps.push(TocStep::LeaveLevel);
                    current_level -= 1;
                }
                steps.push(TocStep::CloseItem);
            } else {
                steps.push(TocStep::CloseItem);
            }
            steps.push(TocStep::Item(entry));
        }
        // Close all levels left open
        while current_level >= TOC_MIN_LEVEL {
            steps.push(TocStep::LeaveLevel);
            current_level -= 1;
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
    appendix_counter: u8,
}

impl SectionNumberer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number prefix (`"1.2. "`, trailing space included) for the next
    /// section at `level`: bumps that level's counter and resets all deeper
    /// levels. Returns `None` outside the numbered range (levels 2 through
    /// 5), leaving the counters untouched.
    pub fn number_prefix(&mut self, level: u8) -> Option<String> {
        if !(2..=5).contains(&level) {
            return None;
        }
        let lvl = level as usize;
        self.counters[lvl] += 1;
        for l in (lvl + 1)..6 {
            self.counters[l] = 0;
        }
        let mut prefix = String::new();
        for l in 2..=lvl {
            if !prefix.is_empty() {
                prefix.push('.');
            }
            prefix.push_str(&self.counters[l].to_string());
        }
        prefix.push_str(". ");
        Some(prefix)
    }

    /// Caption prefix (`"Appendix A: "`) for the next appendix section.
    pub fn appendix_caption(&mut self) -> String {
        self.appendix_counter += 1;
        let letter = (b'A' + self.appendix_counter - 1) as char;
        format!("Appendix {letter}: ")
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
        for (level, id) in [(2, "a"), (3, "a1"), (3, "a2"), (5, "deep"), (2, "b")] {
            b.push(TocEntry { level, id: id.to_string(), title: id.to_uppercase() });
        }
        assert_eq!(b.entries().len(), 5);

        // toclevels=4 → levels 2..=5 visible; level 5 under level 3 opens
        // two lists at once, returning to level 2 closes three.
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
            [">2", "a", ">3", "a1", "/i", "a2", ">4", ">5", "deep",
             "<", "<", "<", "/i", "b", "<"]
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

        assert_eq!(n.appendix_caption(), "Appendix A: ");
        assert_eq!(n.appendix_caption(), "Appendix B: ");
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
