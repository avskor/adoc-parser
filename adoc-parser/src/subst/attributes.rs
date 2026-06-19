//! Attribute-reference extraction pass (Asciidoctor `attributes` sub).
//!
//! Runs after passthrough extraction and before quotes. Every attribute
//! reference `{name}` (and the `{set:…}` inline assignment macro) is lifted out
//! of the working buffer into a tag sentinel pointing at a [`TagToken::AttrRef`]
//! / [`TagToken::AttrSet`] leaf, so the later quotes/replacements/post passes
//! cannot reach inside it.
//!
//! **The reference is NOT resolved here.** The legacy recursive parser does not
//! resolve `{name}` either — it emits an `Event::AttributeReference` and lets the
//! renderer resolve it (so the attributes-before-macros ordering, and the
//! trailing `[brackets]`/`/path[brackets]` capture that turns a URL-valued
//! attribute into a link, are reproduced downstream). This pass therefore mirrors
//! [`crate::inline::InlineState::try_attribute_reference`] /
//! `try_inline_set` exactly, so the events it emits match what the legacy parser
//! would have produced for the same reference.
//!
//! ## Why before quotes (vs. Asciidoctor's quotes-then-attributes order)
//!
//! Because the reference is not resolved, the only thing that matters for parity
//! is reproducing the legacy events. The legacy parser captures a trailing
//! `[brackets]` onto the reference; if quotes ran first it would instead consume
//! that bracket as a formatting attrlist (`{a}[.role]*x*` → an attributed
//! strong), diverging from legacy. Extracting `{name}[…]` up front protects the
//! brackets. The boundary bytes seen by quotes are identical either way (`{`/`}`
//! and the sentinel are both non-word), so quote spans are unaffected.
//!
//! An escaped reference (`\{name}`) never reaches this pass as a live reference:
//! the earlier [`super::escape`] pass already dropped the backslash and sealed
//! `{name}` as a literal leaf, exactly as the legacy parser leaves it as text.

use super::tokenize::{desentinelize, utf8_char_len, Work};

/// Extract every attribute reference / `{set:…}` macro from `work.buf` into
/// sentinels.
pub(super) fn extract(work: &mut Work) {
    let src = std::mem::take(&mut work.buf);
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'{'
            && let Some((extracted, end)) = try_attr(&src, i)
        {
            let sentinel = match extracted {
                Extracted::Ref { name, trailing } => {
                    // The trailing `[brackets]`/`/path[...]` is a raw slice of the
                    // post-escape/macro buffer, so it can still hold sentinel bytes
                    // from an EARLIER pass (most often an escaped typographic literal
                    // `\...`/`\--`/`\(C)` sealed by `escape::run`). Those bytes index
                    // THIS pipeline's tag table; once they reach the renderer they are
                    // re-parsed in a fresh `Work` whose table is empty, orphaning the
                    // sentinel — its index digit leaks into the output (`\...` → `0`).
                    // Resolve them back to their literal source text here so the
                    // `AttributeReference` event the renderer re-parses is clean
                    // (a URL-valued attribute then forms a link whose target carries
                    // the literal `...`, matching Asciidoctor's replacements-before-
                    // macros order). A trailing with no sentinel byte is returned
                    // unchanged (fast-path in `desentinelize`).
                    let trailing = trailing.map(|t| desentinelize(&work.tags, &t));
                    work.attr_ref_sentinel(name, trailing)
                }
                Extracted::Set { name, value } => work.attr_set_sentinel(name, value),
            };
            out.push_str(&sentinel);
            i = end;
            continue;
        }
        // Not a reference: copy the whole UTF-8 character verbatim (copying a
        // single byte would corrupt multibyte text — `byte as char` reinterprets
        // a continuation byte as Latin-1).
        let len = utf8_char_len(bytes[i]);
        out.push_str(&src[i..i + len]);
        i += len;
    }

    work.buf = out;
}

/// What an extracted `{…}` becomes.
enum Extracted {
    Ref {
        name: String,
        trailing: Option<String>,
    },
    Set {
        name: String,
        value: String,
    },
}

/// At a `{` (caller guarantees `src.as_bytes()[i] == b'{'`), try to match an
/// attribute reference or a `{set:…}` macro. Returns the extracted leaf plus the
/// index in `src` just past the consumed span, or `None` to leave the `{`
/// literal. Mirror of `InlineState::try_attribute_reference`.
fn try_attr(src: &str, i: usize) -> Option<(Extracted, usize)> {
    let rest = src.get(i + 1..)?; // after '{'
    let close = rest.find('}')?;
    let content = &rest[..close];

    // {set:name:value} / {set:name} / {set:name!} inline assignment.
    if let Some(set_rest) = content.strip_prefix("set:") {
        return try_set(set_rest, i, close);
    }

    // Reference name is `\w[\w-]*` (Asciidoctor); anything else (e.g. `{n!}`,
    // `{counter:x}`) is not a reference and stays literal.
    let attr_name = content;
    if attr_name.is_empty() {
        return None;
    }
    let first = attr_name.as_bytes()[0];
    if !(first.is_ascii_alphanumeric() || first == b'_') {
        return None;
    }
    if !attr_name
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return None;
    }

    // Capture a `[...]` following the reference (optionally after a non-space,
    // non-bracket path segment) so the renderer can re-parse `value<path>[...]`
    // together — an attribute holding a URL then forms a link macro. Skip `[[`
    // (inline anchor) and a bracket with no closing `]`.
    let after_brace = i + 1 + close + 1;
    let tail = &src[after_brace..];
    let path_len = tail
        .bytes()
        .take_while(|&b| b != b'[' && b != b']' && !b.is_ascii_whitespace())
        .count();
    let after_path = &tail[path_len..];
    let trailing = if after_path.starts_with('[') && !after_path.starts_with("[[") {
        after_path
            .find(']')
            .map(|rb| tail[..path_len + rb + 1].to_string())
    } else {
        None
    };
    let consumed = trailing.as_ref().map_or(0, |b| b.len());

    Some((
        Extracted::Ref {
            name: attr_name.to_string(),
            trailing,
        },
        after_brace + consumed,
    ))
}

/// Mirror of `InlineState::try_inline_set`. `set_rest` is the content after
/// `set:`; `i`/`close` locate the `{` and its `}` so the consumed span ends just
/// past the brace (no trailing brackets for a set macro).
fn try_set(set_rest: &str, i: usize, close: usize) -> Option<(Extracted, usize)> {
    let end = i + 1 + close + 1;

    // {set:name!} — unset.
    if let Some(name) = set_rest.strip_suffix('!') {
        if name.is_empty() || !is_valid_attr_name(name) {
            return None;
        }
        return Some((
            Extracted::Set {
                name: format!("!{name}"),
                value: String::new(),
            },
            end,
        ));
    }

    // {set:name:value} or {set:name} (empty value).
    let (name, value) = if let Some(colon) = set_rest.find(':') {
        (&set_rest[..colon], &set_rest[colon + 1..])
    } else {
        (set_rest, "")
    };
    if name.is_empty() || !is_valid_attr_name(name) {
        return None;
    }
    Some((
        Extracted::Set {
            name: name.to_string(),
            value: value.to_string(),
        },
        end,
    ))
}

/// Mirror of `InlineState::is_valid_attr_name`.
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
