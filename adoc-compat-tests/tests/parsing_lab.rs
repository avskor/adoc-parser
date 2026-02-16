use std::path::{Path, PathBuf};

use adoc_compat_tests::asg::AsgNode;
use adoc_compat_tests::builder::build_asg;
use adoc_parser::{Parser, preprocess};

/// Patterns to skip (relative to the test root).
/// These tests require features our parser doesn't support yet.
const SKIP_PATTERNS: &[&str] = &[
    // Attribute entries in body (requires Unknown(attributes) node)
    "block/attributes/in-block",
    // Leveloffset in body — requires heading vs section distinction + leveloffset
    "block/attributes/leveloffset-in-body",
    // Description list complex features
    "block/dlist/ancestor-list-like",
    "block/dlist/attached-block-no-principal",
    "block/dlist/indented-principal-below-term",
    "block/dlist/indented-sibling-following-nested-list",
    "block/dlist/multiple-terms",
    "block/dlist/nested",
    "block/dlist/parent-list-indented-marker",
    "block/dlist/wrapped-principal",
    // Document metadata and preamble
    "block/document/metadata",
    "block/document/preamble",
    // Header: attribute entries ordering, authors, adjacency
    "block/header/adjacent-to-body",
    "block/header/attribute-entries-above-title",
    "block/header/attribute-entries-around-title",
    "block/header/author",
    "block/header/empty-lines-above",
    // Discrete headings and heading-specific features
    "block/heading/heading-adjacent-paragraph",
    "block/heading/heading-only",
    "block/heading/heading-paragraph",
    "block/heading/implicit-when-inside-block",
    "block/heading/level-0-at-top",
    "block/heading/leveloffset",
    "block/heading/title-attribute",
    // Image: empty target
    "block/image/empty-target",
    // Callout list: advanced features
    "block/list/callout/attached-paragraph",
    "block/list/callout/autonumbered",
    // Ordered list: metadata, markers, nesting
    "block/list/ordered/adjacent-with-metadata",
    "block/list/ordered/implicit-start-from-marker",
    "block/list/ordered/numbered-marker",
    "block/list/ordered/principal-interrupted",
    "block/list/ordered/separated-by-block-attribute-line",
    "block/list/ordered/unlimited-nesting",
    // Unordered list: complex features
    "block/list/unordered/adjacent-delimited-block",
    "block/list/unordered/adjacent-with-metadata",
    "block/list/unordered/ancestor-dlist-like",
    "block/list/unordered/ancestor-with-nested-marker",
    "block/list/unordered/attached-paragraph-plus-only",
    "block/list/unordered/block-attached-to-",
    "block/list/unordered/continue-",
    "block/list/unordered/hyphen-marker",
    "block/list/unordered/isolated-marker",
    "block/list/unordered/list-continuation-as-attached-paragraph",
    "block/list/unordered/separated-by-block-attribute-line",
    "block/list/unordered/sibling-indented-with-metadata",
    "block/list/unordered/unlimited-nesting",
    "block/list/unordered/wrapped-principal",
    // Section: leveloffset processing
    "block/section/leveloffset-input",
    "block/section/relative-leveloffset",
];

fn should_skip(test_path: &str) -> bool {
    SKIP_PATTERNS
        .iter()
        .any(|pattern| test_path.contains(pattern))
}

fn find_test_pairs(root: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut pairs = Vec::new();
    collect_test_pairs(root, &mut pairs);
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

fn collect_test_pairs(dir: &Path, pairs: &mut Vec<(PathBuf, PathBuf)>) {
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
                // Skip tests with config files
                let config_path = path.with_file_name(format!("{stem}-config.json"));
                if !config_path.exists() {
                    pairs.push((path, output_path));
                }
            }
        }
    }
}

#[test]
fn asciidoc_parsing_lab_block_tests() {
    let test_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../vendor/asciidoc-parsing-lab/test/tests/block");

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

    for (input_path, output_path) in &pairs {
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

        let input = match std::fs::read_to_string(input_path) {
            Ok(s) => s,
            Err(e) => {
                failed.push((rel_path, format!("Failed to read input: {e}")));
                continue;
            }
        };
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

        let expected = AsgNode::from_value(&expected_value);

        let preprocessed = preprocess(&input);
        let parser = Parser::new(&preprocessed);
        let actual = build_asg(parser);

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
