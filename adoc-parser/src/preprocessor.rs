use std::collections::{HashMap, HashSet};

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

        // 5. Attribute definitions
        if let Some((name, value)) = parse_attribute_entry(trimmed) {
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
        output.push_str(line);
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
}
