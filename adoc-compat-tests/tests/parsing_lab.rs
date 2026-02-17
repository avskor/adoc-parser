use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use adoc_compat_tests::asg::AsgNode;
use adoc_compat_tests::builder::build_asg;
use adoc_parser::{Parser, preprocess_with_attrs, resolve_includes};

/// Patterns to skip (relative to the test root).
/// These tests require features our parser doesn't support yet.
const SKIP_PATTERNS: &[&str] = &[];

fn should_skip(test_path: &str) -> bool {
    SKIP_PATTERNS
        .iter()
        .any(|pattern| test_path.contains(pattern))
}

struct TestConfig {
    ensure_trailing_newline: bool,
    external_attributes: HashMap<String, Option<String>>,
    locked_attributes: HashSet<String>,
}

impl TestConfig {
    fn from_path(config_path: &Path) -> Self {
        let content = match std::fs::read_to_string(config_path) {
            Ok(s) => s,
            Err(_) => return Self::default(),
        };
        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };

        let ensure_trailing_newline = value
            .get("ensureTrailingNewline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let attrs_value = value
            .get("options")
            .and_then(|o| o.get("attributes"));

        let mut external_attributes = HashMap::new();
        let mut locked_attributes = HashSet::new();

        if let Some(attrs) = attrs_value {
            if let Some(obj) = attrs.as_object() {
                for (key, val) in obj {
                    let is_soft = key.ends_with('@');
                    let name = key.strip_suffix('@').unwrap_or(key).to_string();
                    match val {
                        serde_json::Value::Null => {
                            external_attributes.insert(name.clone(), None);
                            if !is_soft {
                                locked_attributes.insert(name);
                            }
                        }
                        serde_json::Value::String(s) => {
                            external_attributes.insert(name.clone(), Some(s.clone()));
                            if !is_soft {
                                locked_attributes.insert(name);
                            }
                        }
                        _ => {
                            // Non-string/null values — skip
                        }
                    }
                }
            } else if let Some(arr) = attrs.as_array() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        if let Some(key) = s.strip_prefix('!') {
                            // `!key` → unset, locked
                            external_attributes.insert(key.to_string(), None);
                            locked_attributes.insert(key.to_string());
                        } else if let Some((key, val)) = s.split_once("@=") {
                            // `key@=value` → soft-set (not locked)
                            external_attributes.insert(key.to_string(), Some(val.to_string()));
                        } else if let Some((key, val)) = s.split_once('=') {
                            // `key=value` → hard-set (locked)
                            external_attributes.insert(key.to_string(), Some(val.to_string()));
                            locked_attributes.insert(key.to_string());
                        }
                    }
                }
            }
        }

        Self {
            ensure_trailing_newline,
            external_attributes,
            locked_attributes,
        }
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            ensure_trailing_newline: false,
            external_attributes: HashMap::new(),
            locked_attributes: HashSet::new(),
        }
    }
}

/// (input_path, output_path, optional config)
fn find_test_pairs(root: &Path) -> Vec<(PathBuf, PathBuf, Option<PathBuf>)> {
    let mut pairs = Vec::new();
    collect_test_pairs(root, &mut pairs);
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

fn collect_test_pairs(dir: &Path, pairs: &mut Vec<(PathBuf, PathBuf, Option<PathBuf>)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_test_pairs(&path, pairs);
        } else if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with("-input.adoc"))
        {
            let stem = path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .strip_suffix("-input.adoc")
                .unwrap()
                .to_string();
            let output_path = path.with_file_name(format!("{stem}-output.json"));
            if output_path.exists() {
                let config_path = path.with_file_name(format!("{stem}-config.json"));
                let config = if config_path.exists() {
                    Some(config_path)
                } else {
                    None
                };
                pairs.push((path, output_path, config));
            }
        }
    }
}

#[test]
fn asciidoc_parsing_lab_block_tests() {
    let test_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../vendor/asciidoc-parsing-lab/test/tests");

    if !test_root.exists() {
        eprintln!(
            "WARNING: asciidoc-parsing-lab submodule not found at {}",
            test_root.display()
        );
        eprintln!("Run: git submodule update --init");
        return;
    }

    let pairs = find_test_pairs(&test_root);
    assert!(
        !pairs.is_empty(),
        "No test pairs found in {}",
        test_root.display()
    );

    let mut passed = 0;
    let mut skipped = 0;
    let mut failed = Vec::new();

    for (input_path, output_path, config_path) in &pairs {
        // Compute relative path for skip matching
        let rel_path = input_path
            .strip_prefix(test_root.parent().unwrap().parent().unwrap())
            .unwrap_or(input_path)
            .to_string_lossy()
            .replace('\\', "/");

        if should_skip(&rel_path) {
            skipped += 1;
            continue;
        }

        // Parse config if present
        let config = config_path
            .as_ref()
            .map(|p| TestConfig::from_path(p))
            .unwrap_or_default();

        let mut input = match std::fs::read_to_string(input_path) {
            Ok(s) => s,
            Err(e) => {
                failed.push((rel_path, format!("Failed to read input: {e}")));
                continue;
            }
        };

        // Apply ensureTrailingNewline config
        if config.ensure_trailing_newline && !input.ends_with('\n') {
            input.push('\n');
        }
        let expected_json = match std::fs::read_to_string(output_path) {
            Ok(s) => s,
            Err(e) => {
                failed.push((rel_path, format!("Failed to read output: {e}")));
                continue;
            }
        };

        let expected_value: serde_json::Value = match serde_json::from_str(&expected_json) {
            Ok(v) => v,
            Err(e) => {
                failed.push((rel_path, format!("Failed to parse JSON: {e}")));
                continue;
            }
        };

        let input_dir = input_path.parent().unwrap();
        let input = resolve_includes(&input, input_dir);
        let preprocessed = preprocess_with_attrs(&input, &config.external_attributes, &config.locked_attributes);
        let parser = Parser::new(&preprocessed);
        let actual = build_asg(parser, config.external_attributes.clone());

        if expected_value.is_array() {
            // Inline test: expected is a JSON array of inline nodes
            let expected_inlines: Vec<AsgNode> = expected_value
                .as_array()
                .unwrap()
                .iter()
                .map(AsgNode::from_value)
                .collect();

            // Extract inlines from actual Document's first paragraph
            let actual_inlines = extract_first_paragraph_inlines(&actual);

            if actual_inlines.as_ref() == Some(&expected_inlines) {
                passed += 1;
            } else {
                let expected_str = expected_inlines
                    .iter()
                    .map(|n| n.pretty_print(2))
                    .collect::<Vec<_>>()
                    .join("\n");
                let actual_str = match &actual_inlines {
                    Some(inlines) => inlines
                        .iter()
                        .map(|n| n.pretty_print(2))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    None => format!("  (no paragraph found in: {})", actual.pretty_print(0)),
                };
                let msg = format!(
                    "Inline ASG mismatch\n--- expected ---\n{expected_str}\n--- actual ---\n{actual_str}",
                );
                failed.push((rel_path, msg));
            }
        } else {
            let expected = AsgNode::from_value(&expected_value);

            if actual == expected {
                passed += 1;
            } else {
                let msg = format!(
                    "ASG mismatch\n--- expected ---\n{}\n--- actual ---\n{}",
                    expected.pretty_print(0),
                    actual.pretty_print(0),
                );
                failed.push((rel_path, msg));
            }
        }
    }

    let total = passed + skipped + failed.len();
    eprintln!("\n=== Parsing Lab Results ===");
    eprintln!("Total: {total}, Passed: {passed}, Skipped: {skipped}, Failed: {}", failed.len());

    if !failed.is_empty() {
        eprintln!("\n=== Failed tests ===");
        for (path, msg) in &failed {
            eprintln!("\n--- {path} ---");
            eprintln!("{msg}");
        }
        panic!(
            "{} of {} tests failed (see above for details)",
            failed.len(),
            total
        );
    }
}

fn extract_first_paragraph_inlines(doc: &AsgNode) -> Option<Vec<AsgNode>> {
    if let AsgNode::Document { blocks, .. } = doc {
        if let Some(AsgNode::Paragraph { inlines }) = blocks.first() {
            return Some(inlines.clone());
        }
    }
    None
}
