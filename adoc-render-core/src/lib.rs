//! Renderer-agnostic AsciiDoc semantics shared by `adoc_parser` event consumers.
//!
//! Backends (the HTML renderer, the ASG builder in the compatibility test
//! suite, future renderers) must agree on the parts of AsciiDoc that are not
//! syntax but document semantics: intrinsic attribute values, the resolution
//! precedence of `{name}` attribute references, and so on. This crate is the
//! single source of truth for those rules so consumers cannot drift apart.

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
    fn resolve_refs_in_string() {
        let doc = |n: &str| (n == "name").then_some("World");
        assert_eq!(resolve_attr_refs_text("Hello {name}!", doc), "Hello World!");
        assert_eq!(resolve_attr_refs_text("a{nbsp}b", doc), "a\u{00a0}b");
        assert_eq!(resolve_attr_refs_text("{missing} stays", doc), "{missing} stays");
        assert_eq!(resolve_attr_refs_text("brace { only", doc), "brace { only");
        assert_eq!(resolve_attr_refs_text("{NAME}", doc), "World");
    }
}
