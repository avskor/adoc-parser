use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::Path;

// ---------------------------------------------------------------------------
// Include directive options
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum LineRange {
    Single(usize),
    Range(usize, usize),
    From(usize),
}

#[derive(Debug, PartialEq)]
struct TagEntry<'a> {
    name: &'a str,
    include: bool,
}

#[derive(Debug, PartialEq)]
struct TagFilter<'a> {
    entries: Vec<TagEntry<'a>>,
}

#[derive(Debug, Default)]
struct IncludeAttrs<'a> {
    lines: Option<Vec<LineRange>>,
    tags: Option<TagFilter<'a>>,
    optional: bool,
}

fn parse_include_attrs(attrs: &str) -> IncludeAttrs<'_> {
    let mut result = IncludeAttrs::default();
    if attrs.is_empty() {
        return result;
    }

    for part in attrs.split(',') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("lines=") {
            let mut ranges = Vec::new();
            for seg in value.split(';') {
                let seg = seg.trim();
                if seg.is_empty() {
                    continue;
                }
                if let Some(range) = parse_line_range(seg) {
                    ranges.push(range);
                }
            }
            if !ranges.is_empty() {
                result.lines = Some(ranges);
            }
        } else if let Some(value) = part.strip_prefix("tags=") {
            let mut entries = Vec::new();
            for seg in value.split(';') {
                let seg = seg.trim();
                if seg.is_empty() {
                    continue;
                }
                if let Some(stripped) = seg.strip_prefix('!') {
                    entries.push(TagEntry { name: stripped, include: false });
                } else {
                    entries.push(TagEntry { name: seg, include: true });
                }
            }
            if !entries.is_empty() {
                result.tags = Some(TagFilter { entries });
            }
        } else if let Some(value) = part.strip_prefix("tag=") {
            let value = value.trim();
            if !value.is_empty() {
                let (name, include) = if let Some(stripped) = value.strip_prefix('!') {
                    (stripped, false)
                } else {
                    (value, true)
                };
                result.tags = Some(TagFilter {
                    entries: vec![TagEntry { name, include }],
                });
            }
        } else if part == "opts=optional" {
            result.optional = true;
        }
        // Other keys (leveloffset, indent, encoding) — ignored
    }

    result
}

fn parse_line_range(s: &str) -> Option<LineRange> {
    if let Some(pos) = s.find("..") {
        let left = s[..pos].trim();
        let right = s[pos + 2..].trim();
        let start: usize = left.parse().ok()?;
        if right.is_empty() || right == "-1" {
            Some(LineRange::From(start))
        } else {
            let end: usize = right.parse().ok()?;
            Some(LineRange::Range(start, end))
        }
    } else {
        let n: usize = s.trim().parse().ok()?;
        Some(LineRange::Single(n))
    }
}

// ---------------------------------------------------------------------------
// Line filtering
// ---------------------------------------------------------------------------

fn filter_by_lines(content: &str, ranges: &[LineRange]) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let mut included = vec![false; total];

    for range in ranges {
        match *range {
            LineRange::Single(n) => {
                if n >= 1 && n <= total {
                    included[n - 1] = true;
                }
            }
            LineRange::Range(a, b) => {
                let start = a.max(1);
                let end = b.min(total);
                for i in start..=end {
                    included[i - 1] = true;
                }
            }
            LineRange::From(n) => {
                let start = n.max(1);
                for i in start..=total {
                    included[i - 1] = true;
                }
            }
        }
    }

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        if included[i] {
            result.push_str(line);
            result.push('\n');
        }
    }
    if result.ends_with('\n') {
        result.pop();
    }
    result
}

// ---------------------------------------------------------------------------
// Tag filtering
// ---------------------------------------------------------------------------

fn is_tag_directive(line: &str) -> Option<(&str, bool)> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("tag::") {
        let name = rest.strip_suffix("[]")?;
        if !name.is_empty() {
            return Some((name, true));
        }
    }
    if let Some(rest) = trimmed.strip_prefix("end::") {
        let name = rest.strip_suffix("[]")?;
        if !name.is_empty() {
            return Some((name, false));
        }
    }
    None
}

fn filter_by_tags<'a>(content: &str, filter: &TagFilter<'a>) -> String {
    // Determine default inclusion for untagged lines.
    // If the first filter entry is a positive include, untagged lines are excluded.
    // If the first filter entry is a negation, untagged lines are included.
    let default_include = filter
        .entries
        .first()
        .map(|e| !e.include)
        .unwrap_or(true);

    // Build sets of included and excluded tag names
    let mut include_tags: HashSet<&str> = HashSet::new();
    let mut exclude_tags: HashSet<&str> = HashSet::new();
    let mut wildcard_include = false;
    let mut wildcard_exclude = false;
    let mut double_wildcard_include = false;
    let mut double_wildcard_exclude = false;

    for entry in &filter.entries {
        match (entry.name, entry.include) {
            ("**", true) => double_wildcard_include = true,
            ("**", false) => double_wildcard_exclude = true,
            ("*", true) => wildcard_include = true,
            ("*", false) => wildcard_exclude = true,
            (name, true) => { include_tags.insert(name); }
            (name, false) => { exclude_tags.insert(name); }
        }
    }

    let mut result = String::new();
    let mut tag_stack: Vec<&str> = Vec::new();

    for line in content.lines() {
        if let Some((name, is_start)) = is_tag_directive(line) {
            if is_start {
                tag_stack.push(name);
            } else {
                // Pop matching tag from stack
                if let Some(pos) = tag_stack.iter().rposition(|&t| t == name) {
                    tag_stack.remove(pos);
                }
            }
            // Tag directive lines are always removed
            continue;
        }

        let in_tag = !tag_stack.is_empty();
        let should_include = if in_tag {
            // Check if any active tag matches the filter
            let mut included = false;
            for &tag in &tag_stack {
                if include_tags.contains(tag) || wildcard_include {
                    included = true;
                }
                if exclude_tags.contains(tag) || wildcard_exclude {
                    included = false;
                    break;
                }
            }
            included
        } else {
            // Untagged line
            if double_wildcard_include {
                true
            } else if double_wildcard_exclude {
                false
            } else {
                default_include
            }
        };

        if should_include {
            result.push_str(line);
            result.push('\n');
        }
    }

    if result.ends_with('\n') {
        result.pop();
    }
    result
}

/// Resolve `include::path[]` directives by reading and splicing file content.
///
/// This is a text-to-text transformation that should run before conditional
/// directive processing and parsing.
pub fn resolve_includes(input: &str, base_dir: &Path) -> String {
    let mut output = String::with_capacity(input.len());

    for line in input.lines() {
        if let Some((path, attrs_str)) = crate::scanner::is_include_directive(line) {
            let attrs = parse_include_attrs(attrs_str);
            let file_path = base_dir.join(path);
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    let filtered = if let Some(ref ranges) = attrs.lines {
                        filter_by_lines(&content, ranges)
                    } else if let Some(ref tag_filter) = attrs.tags {
                        filter_by_tags(&content, tag_filter)
                    } else {
                        let trimmed = content.trim_end_matches(['\n', '\r']);
                        trimmed.to_string()
                    };
                    if !filtered.is_empty() {
                        output.push_str(&filtered);
                        output.push('\n');
                    }
                }
                Err(_) if attrs.optional => { /* skip silently */ }
                Err(_) => { /* skip (current behavior) */ }
            }
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }

    // Remove trailing newline if original didn't end with one
    if !input.ends_with('\n') && output.ends_with('\n') {
        output.pop();
    }

    output
}

// ---------------------------------------------------------------------------
// Counter support ({counter:name}, {counter2:name})
// ---------------------------------------------------------------------------

/// Determine the initial value from a seed string.
///
/// - A single ASCII letter → that letter (alphabetic counter)
/// - A parseable integer → that number
/// - Anything else → `"1"`
fn initialize_from_seed(seed: &str) -> String {
    if seed.len() == 1 {
        let ch = seed.as_bytes()[0];
        if ch.is_ascii_alphabetic() {
            return seed.to_string();
        }
    }
    if seed.parse::<i64>().is_ok() {
        return seed.to_string();
    }
    "1".to_string()
}

/// Increment a counter value.
///
/// - Single uppercase letter: A→B … Y→Z, Z→Z (saturation)
/// - Single lowercase letter: a→b … y→z, z→z (saturation)
/// - Integer string: +1
/// - Anything else: `"1"`
fn increment_counter_value(current: &str) -> String {
    if current.len() == 1 {
        let ch = current.as_bytes()[0];
        if ch.is_ascii_uppercase() {
            return if ch < b'Z' {
                String::from((ch + 1) as char)
            } else {
                "Z".to_string()
            };
        }
        if ch.is_ascii_lowercase() {
            return if ch < b'z' {
                String::from((ch + 1) as char)
            } else {
                "z".to_string()
            };
        }
    }
    if let Ok(n) = current.parse::<i64>() {
        return (n + 1).to_string();
    }
    "1".to_string()
}

/// Try to parse a counter macro starting at `input[0] == '{'`.
///
/// Recognised forms:
/// - `{counter:name}`
/// - `{counter:name:seed}`
/// - `{counter2:name}`
/// - `{counter2:name:seed}`
///
/// Returns `(replacement_text, bytes_consumed)` on success.
fn try_parse_counter(
    input: &str,
    attributes: &mut HashMap<String, String>,
) -> Option<(String, usize)> {
    if !input.starts_with('{') {
        return None;
    }

    let close = input.find('}')?;
    let inner = &input[1..close]; // between { and }

    let (silent, rest) = if let Some(r) = inner.strip_prefix("counter2:") {
        (true, r)
    } else if let Some(r) = inner.strip_prefix("counter:") {
        (false, r)
    } else {
        return None;
    };

    // Split rest into name and optional seed
    let (name, seed) = if let Some(colon_pos) = rest.find(':') {
        (&rest[..colon_pos], Some(&rest[colon_pos + 1..]))
    } else {
        (rest, None)
    };

    // Validate name: non-empty, alphanumeric + '-' + '_'
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }

    let new_value = if let Some(current) = attributes.get(name) {
        increment_counter_value(current)
    } else if let Some(s) = seed {
        initialize_from_seed(s)
    } else {
        "1".to_string()
    };

    attributes.insert(name.to_string(), new_value.clone());

    let replacement = if silent {
        String::new()
    } else {
        new_value
    };

    Some((replacement, close + 1)) // +1 for the closing '}'
}

/// Expand all `{counter:…}` / `{counter2:…}` macros in a single line.
///
/// Returns `None` when the line contains no counters (zero-allocation fast path).
fn expand_counters(line: &str, attributes: &mut HashMap<String, String>) -> Option<String> {
    if !line.contains("{counter") {
        return None;
    }

    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut result = String::with_capacity(len);
    let mut i = 0;
    let mut any_expanded = false;

    while i < len {
        if bytes[i] == b'{' && line[i..].starts_with("{counter")
            && let Some((replacement, consumed)) = try_parse_counter(&line[i..], attributes)
        {
            result.push_str(&replacement);
            i += consumed;
            any_expanded = true;
            continue;
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    if any_expanded { Some(result) } else { None }
}

/// Preprocess AsciiDoc source by evaluating conditional directives
/// (`ifdef`, `ifndef`, `ifeval`, `endif`) and tracking document attributes.
///
/// This is a text-to-text transformation that should run after include
/// resolution and before parsing.
pub fn preprocess(input: &str) -> String {
    preprocess_with_attrs(input, &HashMap::new(), &HashSet::new())
}

/// Like [`preprocess`], but accepts initial external attributes and a set of
/// locked attribute names.  Locked attributes cannot be overridden by attribute
/// entries (`:name: value` / `:!name:`) in the document.
pub fn preprocess_with_attrs(
    input: &str,
    initial_attrs: &HashMap<String, Option<String>>,
    locked_attrs: &HashSet<String>,
) -> String {
    let mut attributes: HashMap<String, String> = HashMap::new();
    for (k, v) in initial_attrs {
        if let Some(val) = v {
            attributes.insert(k.clone(), val.clone());
        }
    }
    let mut skip_stack: Vec<bool> = Vec::new();
    let mut output = String::with_capacity(input.len());

    for line in input.lines() {
        let trimmed = line.trim();

        // 1. endif::[] — always processed regardless of skip state
        if trimmed == "endif::[]" {
            skip_stack.pop();
            continue;
        }

        // 2–4. Conditional directives
        if let Some(cond) = parse_conditional(trimmed) {
            let condition_met = match cond.kind {
                ConditionalKind::Ifdef => evaluate_condition(cond.attrs, &attributes),
                ConditionalKind::Ifndef => !evaluate_condition(cond.attrs, &attributes),
                ConditionalKind::Ifeval => {
                    evaluate_ifeval(cond.attrs, &attributes)
                }
            };

            match cond.inline_content {
                Some(content) => {
                    // Inline form: emit content if condition met and not skipping
                    if condition_met && !is_skipping(&skip_stack) {
                        output.push_str(content);
                        output.push('\n');
                    }
                }
                None => {
                    // Block form: push onto skip stack
                    skip_stack.push(!condition_met || is_skipping(&skip_stack));
                }
            }
            continue;
        }

        // If currently skipping, don't process or output the line
        if is_skipping(&skip_stack) {
            continue;
        }

        // 5a. Expand counters
        let effective_line: Cow<'_, str> = match expand_counters(line, &mut attributes) {
            Some(expanded) => Cow::Owned(expanded),
            None => Cow::Borrowed(line),
        };

        // 5b. Attribute definitions (sees expanded counter values)
        if let Some((name, value)) = parse_attribute_entry(effective_line.trim()) {
            if locked_attrs.contains(name) {
                // Locked attribute — don't modify and don't output line
                continue;
            }
            match value {
                Some(v) => {
                    attributes.insert(name.to_string(), v.to_string());
                }
                None => {
                    attributes.remove(name);
                }
            }
        }

        // 6. Output the line
        output.push_str(&effective_line);
        output.push('\n');
    }

    // Remove trailing newline if original didn't end with one
    if !input.ends_with('\n') && output.ends_with('\n') {
        output.pop();
    }

    output
}

fn is_skipping(stack: &[bool]) -> bool {
    stack.iter().any(|&s| s)
}

#[derive(Debug, PartialEq)]
enum ConditionalKind {
    Ifdef,
    Ifndef,
    Ifeval,
}

struct Conditional<'a> {
    kind: ConditionalKind,
    attrs: &'a str,
    inline_content: Option<&'a str>,
}

/// Parse a conditional directive line.
///
/// Returns `None` if the line is not a conditional directive.
/// For ifdef/ifndef, `attrs` is the attribute expression and `inline_content`
/// is `Some(text)` for inline form or `None` for block form.
/// For ifeval, `attrs` contains the full expression inside `[...]`.
fn parse_conditional(line: &str) -> Option<Conditional<'_>> {
    if let Some(rest) = line.strip_prefix("ifdef::") {
        return parse_ifdef_ifndef(ConditionalKind::Ifdef, rest);
    }
    if let Some(rest) = line.strip_prefix("ifndef::") {
        return parse_ifdef_ifndef(ConditionalKind::Ifndef, rest);
    }
    if let Some(rest) = line.strip_prefix("ifeval::[") {
        let expr = rest.strip_suffix(']')?;
        return Some(Conditional {
            kind: ConditionalKind::Ifeval,
            attrs: expr,
            inline_content: None, // ifeval is always block form
        });
    }
    None
}

fn parse_ifdef_ifndef<'a>(kind: ConditionalKind, rest: &'a str) -> Option<Conditional<'a>> {
    let bracket_start = rest.find('[')?;
    let bracket_end = rest.rfind(']')?;
    if bracket_end <= bracket_start {
        return None;
    }

    let attrs = &rest[..bracket_start];
    let content = &rest[bracket_start + 1..bracket_end];

    if content.is_empty() {
        // Block form
        Some(Conditional {
            kind,
            attrs,
            inline_content: None,
        })
    } else {
        // Inline form
        Some(Conditional {
            kind,
            attrs,
            inline_content: Some(content),
        })
    }
}

/// Evaluate an ifdef/ifndef condition against the attribute map.
///
/// - If `attrs` contains `,` → ANY (at least one defined)
/// - If `attrs` contains `+` → ALL (all defined)
/// - Otherwise → single attribute check
fn evaluate_condition(attrs: &str, attributes: &HashMap<String, String>) -> bool {
    if attrs.contains(',') {
        // ANY: at least one must be defined
        attrs.split(',').any(|a| attributes.contains_key(a.trim()))
    } else if attrs.contains('+') {
        // ALL: all must be defined
        attrs.split('+').all(|a| attributes.contains_key(a.trim()))
    } else {
        attributes.contains_key(attrs.trim())
    }
}

/// Evaluate an ifeval expression.
///
/// 1. Substitute `{attr}` references with values from the attribute map
/// 2. Find the comparison operator
/// 3. Compare operands (numeric if both parse as numbers, otherwise string)
fn evaluate_ifeval(expr: &str, attributes: &HashMap<String, String>) -> bool {
    // Substitute attribute references
    let substituted = substitute_attributes(expr, attributes);

    // Find operator and split
    let operators = ["==", "!=", "<=", ">=", "<", ">"];
    for op in &operators {
        if let Some(pos) = substituted.find(op) {
            let left = extract_operand(&substituted[..pos]);
            let right = extract_operand(&substituted[pos + op.len()..]);
            return compare(&left, op, &right);
        }
    }

    false
}

/// Substitute `{name}` attribute references in a string.
fn substitute_attributes(input: &str, attributes: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut name = String::new();
            let mut found_close = false;
            for inner in chars.by_ref() {
                if inner == '}' {
                    found_close = true;
                    break;
                }
                name.push(inner);
            }
            if found_close {
                if let Some(value) = attributes.get(&name) {
                    result.push_str(value);
                }
                // If not found, substitute with empty string (nothing pushed)
            } else {
                // No closing brace found, output as-is
                result.push('{');
                result.push_str(&name);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Extract an operand from an expression fragment, stripping whitespace and quotes.
fn extract_operand(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Compare two operands with the given operator.
/// If both operands parse as numbers, use numeric comparison; otherwise string comparison.
fn compare(left: &str, op: &str, right: &str) -> bool {
    if let (Ok(l), Ok(r)) = (left.parse::<f64>(), right.parse::<f64>()) {
        match op {
            "==" => l == r,
            "!=" => l != r,
            "<" => l < r,
            ">" => l > r,
            "<=" => l <= r,
            ">=" => l >= r,
            _ => false,
        }
    } else {
        match op {
            "==" => left == right,
            "!=" => left != right,
            "<" => left < right,
            ">" => left > right,
            "<=" => left <= right,
            ">=" => left >= right,
            _ => false,
        }
    }
}

/// Parse an attribute entry line like `:name: value` or `:!name:` or `:name!:`.
///
/// Returns `Some((name, Some(value)))` for definitions
/// and `Some((name, None))` for unsets.
/// Returns `None` if the line is not an attribute entry.
fn parse_attribute_entry(line: &str) -> Option<(&str, Option<&str>)> {
    let rest = line.strip_prefix(':')?;

    // Unset form: :!name:
    if let Some(rest) = rest.strip_prefix('!') {
        let end = rest.find(':')?;
        let name = &rest[..end];
        if !name.is_empty() {
            return Some((name, None));
        }
        return None;
    }

    let end = rest.find(':')?;
    if end == 0 {
        return None;
    }

    let name = &rest[..end];

    // Unset form: :name!:
    if let Some(name) = name.strip_suffix('!') {
        if !name.is_empty() {
            return Some((name, None));
        }
        return None;
    }

    let after_colon = &rest[end + 1..];
    let value = if after_colon.is_empty() {
        ""
    } else if let Some(v) = after_colon.strip_prefix(' ') {
        v
    } else {
        // Not a valid attribute entry (no space after second colon)
        return None;
    };

    Some((name, Some(value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ifdef_inline_defined() {
        let input = ":backend: html\nifdef::backend[Backend is set.]";
        let result = preprocess(input);
        assert_eq!(result, ":backend: html\nBackend is set.");
    }

    #[test]
    fn test_ifdef_inline_undefined() {
        let input = "ifdef::backend[Backend is set.]";
        let result = preprocess(input);
        assert_eq!(result, "");
    }

    #[test]
    fn test_ifndef_inline_defined() {
        let input = ":backend: html\nifndef::backend[No backend.]";
        let result = preprocess(input);
        assert_eq!(result, ":backend: html");
    }

    #[test]
    fn test_ifndef_inline_undefined() {
        let input = "ifndef::backend[No backend.]";
        let result = preprocess(input);
        assert_eq!(result, "No backend.");
    }

    #[test]
    fn test_ifdef_block_defined() {
        let input = "\
:flag:
ifdef::flag[]
visible content
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "\
:flag:
visible content");
    }

    #[test]
    fn test_ifdef_block_undefined() {
        let input = "\
ifdef::flag[]
hidden content
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "");
    }

    #[test]
    fn test_ifndef_block() {
        let input = "\
ifndef::flag[]
visible because undefined
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "visible because undefined");
    }

    #[test]
    fn test_nested_ifdef() {
        let input = "\
:a:
:b:
ifdef::a[]
outer visible
ifdef::b[]
inner visible
endif::[]
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "\
:a:
:b:
outer visible
inner visible");
    }

    #[test]
    fn test_nested_ifdef_inner_false() {
        let input = "\
:a:
ifdef::a[]
outer visible
ifdef::b[]
inner hidden
endif::[]
still outer
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "\
:a:
outer visible
still outer");
    }

    #[test]
    fn test_ifdef_any() {
        // ANY: at least one attribute defined
        let input = "\
:a:
ifdef::a,b[]
any matched
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "\
:a:
any matched");
    }

    #[test]
    fn test_ifdef_any_none() {
        let input = "\
ifdef::a,b[]
none matched
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "");
    }

    #[test]
    fn test_ifdef_all() {
        // ALL: all attributes must be defined
        let input = "\
:a:
:b:
ifdef::a+b[]
all matched
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "\
:a:
:b:
all matched");
    }

    #[test]
    fn test_ifdef_all_missing_one() {
        let input = "\
:a:
ifdef::a+b[]
not all matched
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, ":a:");
    }

    #[test]
    fn test_attribute_unset() {
        let input = "\
:flag: yes
:!flag:
ifdef::flag[still here]";
        let result = preprocess(input);
        assert_eq!(result, ":flag: yes\n:!flag:");
    }

    #[test]
    fn test_attribute_unset_suffix() {
        let input = "\
:flag: yes
:flag!:
ifdef::flag[still here]";
        let result = preprocess(input);
        assert_eq!(result, ":flag: yes\n:flag!:");
    }

    #[test]
    fn test_ifeval_string_equal() {
        let input = "\
:backend: html
ifeval::[\"{backend}\" == \"html\"]
html output
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "\
:backend: html
html output");
    }

    #[test]
    fn test_ifeval_string_not_equal() {
        let input = "\
:backend: html
ifeval::[\"{backend}\" != \"pdf\"]
not pdf
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "\
:backend: html
not pdf");
    }

    #[test]
    fn test_ifeval_numeric_comparison() {
        let input = "\
:level: 3
ifeval::[\"{level}\" > \"1\"]
level is greater
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "\
:level: 3
level is greater");
    }

    #[test]
    fn test_ifeval_undefined_attr() {
        let input = "\
ifeval::[\"{missing}\" == \"\"]
missing is empty
endif::[]";
        let result = preprocess(input);
        assert_eq!(result, "missing is empty");
    }

    #[test]
    fn test_passthrough_normal_lines() {
        let input = "Hello world\n\nThis is normal text.";
        let result = preprocess(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_attribute_with_value() {
        let input = ":author: John Doe\nifdef::author[By {author}]";
        // Note: inline content is emitted as-is, no attribute substitution in inline content
        let result = preprocess(input);
        assert_eq!(result, ":author: John Doe\nBy {author}");
    }

    // -----------------------------------------------------------------------
    // parse_include_attrs tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_include_attrs_empty() {
        let attrs = parse_include_attrs("");
        assert!(attrs.lines.is_none());
        assert!(attrs.tags.is_none());
        assert!(!attrs.optional);
    }

    #[test]
    fn test_parse_include_attrs_lines_range() {
        let attrs = parse_include_attrs("lines=1..5");
        assert_eq!(attrs.lines, Some(vec![LineRange::Range(1, 5)]));
    }

    #[test]
    fn test_parse_include_attrs_lines_single() {
        let attrs = parse_include_attrs("lines=5");
        assert_eq!(attrs.lines, Some(vec![LineRange::Single(5)]));
    }

    #[test]
    fn test_parse_include_attrs_lines_multiple() {
        let attrs = parse_include_attrs("lines=1..5;10..15");
        assert_eq!(
            attrs.lines,
            Some(vec![LineRange::Range(1, 5), LineRange::Range(10, 15)])
        );
    }

    #[test]
    fn test_parse_include_attrs_lines_from_negative() {
        let attrs = parse_include_attrs("lines=5..-1");
        assert_eq!(attrs.lines, Some(vec![LineRange::From(5)]));
    }

    #[test]
    fn test_parse_include_attrs_lines_from_open() {
        let attrs = parse_include_attrs("lines=5..");
        assert_eq!(attrs.lines, Some(vec![LineRange::From(5)]));
    }

    #[test]
    fn test_parse_include_attrs_tags() {
        let attrs = parse_include_attrs("tags=foo;bar");
        let filter = attrs.tags.unwrap();
        assert_eq!(filter.entries.len(), 2);
        assert_eq!(filter.entries[0], TagEntry { name: "foo", include: true });
        assert_eq!(filter.entries[1], TagEntry { name: "bar", include: true });
    }

    #[test]
    fn test_parse_include_attrs_tag_negated() {
        let attrs = parse_include_attrs("tag=!foo");
        let filter = attrs.tags.unwrap();
        assert_eq!(filter.entries.len(), 1);
        assert_eq!(filter.entries[0], TagEntry { name: "foo", include: false });
    }

    #[test]
    fn test_parse_include_attrs_optional() {
        let attrs = parse_include_attrs("opts=optional");
        assert!(attrs.optional);
    }

    #[test]
    fn test_parse_include_attrs_ignore_unknown() {
        let attrs = parse_include_attrs("leveloffset=+1,lines=1..3");
        assert_eq!(attrs.lines, Some(vec![LineRange::Range(1, 3)]));
        assert!(!attrs.optional);
    }

    // -----------------------------------------------------------------------
    // filter_by_lines tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_filter_by_lines_single() {
        let content = "line1\nline2\nline3\nline4\nline5";
        assert_eq!(filter_by_lines(content, &[LineRange::Single(3)]), "line3");
    }

    #[test]
    fn test_filter_by_lines_range() {
        let content = "line1\nline2\nline3\nline4\nline5";
        assert_eq!(
            filter_by_lines(content, &[LineRange::Range(2, 4)]),
            "line2\nline3\nline4"
        );
    }

    #[test]
    fn test_filter_by_lines_from() {
        let content = "line1\nline2\nline3\nline4\nline5";
        assert_eq!(
            filter_by_lines(content, &[LineRange::From(4)]),
            "line4\nline5"
        );
    }

    #[test]
    fn test_filter_by_lines_multiple_ranges() {
        let content = "line1\nline2\nline3\nline4\nline5";
        assert_eq!(
            filter_by_lines(
                content,
                &[LineRange::Single(1), LineRange::Range(4, 5)]
            ),
            "line1\nline4\nline5"
        );
    }

    #[test]
    fn test_filter_by_lines_out_of_bounds() {
        let content = "line1\nline2";
        assert_eq!(
            filter_by_lines(content, &[LineRange::Single(10)]),
            ""
        );
    }

    // -----------------------------------------------------------------------
    // filter_by_tags tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_filter_by_tags_single() {
        let content = "\
before
tag::foo[]
inside foo
end::foo[]
after";
        let filter = TagFilter {
            entries: vec![TagEntry { name: "foo", include: true }],
        };
        assert_eq!(filter_by_tags(content, &filter), "inside foo");
    }

    #[test]
    fn test_filter_by_tags_negation() {
        let content = "\
before
tag::foo[]
inside foo
end::foo[]
after";
        let filter = TagFilter {
            entries: vec![TagEntry { name: "foo", include: false }],
        };
        assert_eq!(filter_by_tags(content, &filter), "before\nafter");
    }

    #[test]
    fn test_filter_by_tags_multiple() {
        let content = "\
before
tag::foo[]
in foo
end::foo[]
middle
tag::bar[]
in bar
end::bar[]
after";
        let filter = TagFilter {
            entries: vec![
                TagEntry { name: "foo", include: true },
                TagEntry { name: "bar", include: true },
            ],
        };
        assert_eq!(filter_by_tags(content, &filter), "in foo\nin bar");
    }

    #[test]
    fn test_filter_by_tags_wildcard() {
        let content = "\
before
tag::foo[]
in foo
end::foo[]
middle
tag::bar[]
in bar
end::bar[]
after";
        let filter = TagFilter {
            entries: vec![TagEntry { name: "*", include: true }],
        };
        assert_eq!(filter_by_tags(content, &filter), "in foo\nin bar");
    }

    #[test]
    fn test_filter_by_tags_negated_wildcard() {
        let content = "\
before
tag::foo[]
in foo
end::foo[]
after";
        let filter = TagFilter {
            entries: vec![TagEntry { name: "*", include: false }],
        };
        assert_eq!(filter_by_tags(content, &filter), "before\nafter");
    }

    #[test]
    fn test_filter_by_tags_nested() {
        let content = "\
tag::outer[]
outer line
tag::inner[]
inner line
end::inner[]
outer again
end::outer[]";
        let filter = TagFilter {
            entries: vec![TagEntry { name: "inner", include: true }],
        };
        assert_eq!(filter_by_tags(content, &filter), "inner line");
    }

    #[test]
    fn test_filter_by_tags_directives_removed() {
        let content = "\
tag::foo[]
content
end::foo[]";
        let filter = TagFilter {
            entries: vec![TagEntry { name: "foo", include: true }],
        };
        // tag/end directives must not appear in output
        assert_eq!(filter_by_tags(content, &filter), "content");
    }

    // -----------------------------------------------------------------------
    // resolve_includes integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_includes_with_lines() {
        let dir = std::env::temp_dir().join("adoc_test_lines");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sample.adoc");
        std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\n").unwrap();

        let input = "include::sample.adoc[lines=2..4]";
        let result = resolve_includes(input, &dir);
        assert_eq!(result, "line2\nline3\nline4");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_includes_with_tag() {
        let dir = std::env::temp_dir().join("adoc_test_tags");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("tagged.adoc");
        std::fs::write(
            &file,
            "preamble\ntag::example[]\nshown content\nend::example[]\nepilogue\n",
        )
        .unwrap();

        let input = "include::tagged.adoc[tag=example]";
        let result = resolve_includes(input, &dir);
        assert_eq!(result, "shown content");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_includes_optional_missing() {
        let dir = std::env::temp_dir().join("adoc_test_optional");
        let _ = std::fs::create_dir_all(&dir);

        let input = "before\ninclude::nonexistent.adoc[opts=optional]\nafter";
        let result = resolve_includes(input, &dir);
        assert_eq!(result, "before\nafter");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_includes_optional_existing() {
        let dir = std::env::temp_dir().join("adoc_test_optional_exists");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("exists.adoc");
        std::fs::write(&file, "hello\n").unwrap();

        let input = "include::exists.adoc[opts=optional]";
        let result = resolve_includes(input, &dir);
        assert_eq!(result, "hello");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // -----------------------------------------------------------------------
    // Counter: increment_counter_value tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_increment_numbers() {
        assert_eq!(increment_counter_value("1"), "2");
        assert_eq!(increment_counter_value("5"), "6");
        assert_eq!(increment_counter_value("0"), "1");
        assert_eq!(increment_counter_value("-1"), "0");
    }

    #[test]
    fn test_increment_uppercase() {
        assert_eq!(increment_counter_value("A"), "B");
        assert_eq!(increment_counter_value("Y"), "Z");
        assert_eq!(increment_counter_value("Z"), "Z"); // saturation
    }

    #[test]
    fn test_increment_lowercase() {
        assert_eq!(increment_counter_value("a"), "b");
        assert_eq!(increment_counter_value("y"), "z");
        assert_eq!(increment_counter_value("z"), "z"); // saturation
    }

    #[test]
    fn test_increment_fallback() {
        assert_eq!(increment_counter_value("foo"), "1");
    }

    // -----------------------------------------------------------------------
    // Counter: initialize_from_seed tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_seed_number() {
        assert_eq!(initialize_from_seed("5"), "5");
        assert_eq!(initialize_from_seed("0"), "0");
        assert_eq!(initialize_from_seed("-3"), "-3");
    }

    #[test]
    fn test_seed_letter() {
        assert_eq!(initialize_from_seed("A"), "A");
        assert_eq!(initialize_from_seed("z"), "z");
    }

    #[test]
    fn test_seed_fallback() {
        assert_eq!(initialize_from_seed("foo"), "1");
        assert_eq!(initialize_from_seed(""), "1");
    }

    // -----------------------------------------------------------------------
    // Counter: expand_counters tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_expand_counter_basic() {
        let mut attrs = HashMap::new();
        assert_eq!(
            expand_counters("Item {counter:item}", &mut attrs),
            Some("Item 1".to_string())
        );
        assert_eq!(attrs.get("item").unwrap(), "1");

        assert_eq!(
            expand_counters("Item {counter:item}", &mut attrs),
            Some("Item 2".to_string())
        );
        assert_eq!(attrs.get("item").unwrap(), "2");
    }

    #[test]
    fn test_expand_counter_with_seed() {
        let mut attrs = HashMap::new();
        assert_eq!(
            expand_counters("{counter:n:5}", &mut attrs),
            Some("5".to_string())
        );
        assert_eq!(
            expand_counters("{counter:n}", &mut attrs),
            Some("6".to_string())
        );
    }

    #[test]
    fn test_expand_counter_alpha_seed() {
        let mut attrs = HashMap::new();
        assert_eq!(
            expand_counters("{counter:a:A}", &mut attrs),
            Some("A".to_string())
        );
        assert_eq!(
            expand_counters("{counter:a}", &mut attrs),
            Some("B".to_string())
        );
        assert_eq!(
            expand_counters("{counter:a}", &mut attrs),
            Some("C".to_string())
        );
    }

    #[test]
    fn test_expand_counter2_silent() {
        let mut attrs = HashMap::new();
        assert_eq!(
            expand_counters("{counter2:x}", &mut attrs),
            Some(String::new())
        );
        assert_eq!(attrs.get("x").unwrap(), "1");

        assert_eq!(
            expand_counters("{counter2:x}", &mut attrs),
            Some(String::new())
        );
        assert_eq!(attrs.get("x").unwrap(), "2");
    }

    #[test]
    fn test_expand_multiple_counters() {
        let mut attrs = HashMap::new();
        assert_eq!(
            expand_counters("{counter:a} and {counter:b}", &mut attrs),
            Some("1 and 1".to_string())
        );
    }

    #[test]
    fn test_expand_no_counters() {
        let mut attrs = HashMap::new();
        assert_eq!(expand_counters("plain line", &mut attrs), None);
    }

    #[test]
    fn test_expand_counter_empty_name() {
        let mut attrs = HashMap::new();
        // Empty name → not a valid counter, returned as-is
        assert_eq!(expand_counters("{counter:}", &mut attrs), None);
    }

    #[test]
    fn test_expand_counter_unclosed() {
        let mut attrs = HashMap::new();
        assert_eq!(expand_counters("{counter:name", &mut attrs), None);
    }

    // -----------------------------------------------------------------------
    // Counter: integration through preprocess()
    // -----------------------------------------------------------------------

    #[test]
    fn test_preprocess_counter_sequential() {
        let input = "Item {counter:item}\nItem {counter:item}\nItem {counter:item}";
        let result = preprocess(input);
        assert_eq!(result, "Item 1\nItem 2\nItem 3");
    }

    #[test]
    fn test_preprocess_counter2_silent() {
        let input = "{counter2:n}\nValue is not shown";
        let result = preprocess(input);
        assert_eq!(result, "\nValue is not shown");
    }

    #[test]
    fn test_preprocess_counter_alpha() {
        let input = "\
Appendix {counter:app:A}
Appendix {counter:app}
Appendix {counter:app}";
        let result = preprocess(input);
        assert_eq!(result, "\
Appendix A
Appendix B
Appendix C");
    }

    #[test]
    fn test_preprocess_counter_skipped_in_ifdef() {
        let input = "\
ifdef::nonexistent[]
{counter:x}
endif::[]
Value: {counter:x}";
        let result = preprocess(input);
        // Counter inside skipped ifdef must not execute, so first use starts at 1
        assert_eq!(result, "Value: 1");
    }
}
