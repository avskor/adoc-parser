use std::path::{Path, PathBuf};

use adoc_compat_tests::asg::AsgNode;
use adoc_compat_tests::builder::build_asg;
use adoc_parser::Parser;

/// Patterns to skip (relative to the test root).
/// These tests require features our parser doesn't support yet.
const SKIP_PATTERNS: &[&str] = &[
    // Include directives require file I/O
    "block/include/",
    // Conditional directives — partial support
    "block/conditional/",
    // Attribute resolution in headers
    "block/header/reference-",
    "block/header/redefined-",
    "block/header/escaped-",
    "block/header/negated-",
    "block/header/suppressed-",
    "block/header/attribute-entries-",
    // Author parsing in header
    "block/header/author",
    // Header with empty lines above (parser requires pos==0)
    "block/header/empty-lines-above",
    // Header adjacent to body — parser doesn't stop header at non-attribute line
    "block/header/adjacent-to-body",
    // Attribute list parsing
    "block/attrlist/",
    // Attribute entries in body
    "block/attributes/",
    // Leveloffset
    "block/section/leveloffset",
    "block/section/bogus-leveloffset",
    "block/section/relative-leveloffset",
    "block/heading/leveloffset",
    // Title attribute on section/heading
    "block/section/title-attribute",
    "block/heading/title-attribute",
    // Tests with config files often require special options
    "block/section/title-body-",
    "block/section/title-adjacent-body-",
    // Discrete headings — parser doesn't distinguish heading vs section
    "block/heading/heading-only",
    "block/heading/heading-paragraph",
    "block/heading/heading-adjacent-paragraph",
    // Heading level-0 at top (becomes document title)
    "block/heading/level-0-at-top",
    // Heading implicit when inside block
    "block/heading/implicit-when-inside-block",
    // Source style requiring attribute resolution
    "block/listing/source-style",
    "block/listing/implicit-source-style",
    "block/listing/inherited-implicit-source-style",
    "block/listing/source-style-with-default-language",
    // Image — attribute list parsing
    "block/image/",
    // Complex list features with metadata
    "block/list/unordered/attached-block-with-metadata",
    "block/list/unordered/attached-indented-block-with-metadata",
    "block/list/unordered/attached-orphaned-metadata",
    "block/list/unordered/adjacent-with-metadata",
    "block/list/unordered/nested-with-metadata",
    "block/list/unordered/sibling-indented-with-metadata",
    "block/list/unordered/separated-by-block-attribute-line",
    // Wrapped principal — parser can't distinguish from attached paragraph
    "block/list/unordered/wrapped-principal",
    // Adjacent delimited block requires attribute processing
    "block/list/unordered/adjacent-delimited-block",
    // Unlimited nesting — parser doesn't pop back to ancestor correctly
    "block/list/unordered/unlimited-nesting",
    // Ancestor depth handling issues in parser
    "block/list/unordered/ancestor-dlist-like",
    "block/list/unordered/ancestor-with-nested-marker",
    "block/list/ordered/unlimited-nesting",
    // List continuation edge cases
    "block/list/unordered/list-continuation-as-attached-paragraph",
    "block/list/unordered/trailing-list-continuation",
    "block/list/unordered/attached-paragraph-plus-only",
    // Attached paragraphs
    "block/list/unordered/attached-paragraphs-",
    // Hyphen marker
    "block/list/unordered/hyphen-marker",
    // Isolated marker
    "block/list/unordered/isolated-marker",
    // Block attached to ancestor/parent
    "block/list/unordered/block-attached-to-",
    // Continue ancestor/parent
    "block/list/unordered/continue-",
    "block/list/ordered/adjacent-with-metadata",
    "block/list/ordered/nested-with-metadata",
    "block/list/ordered/separated-by-block-attribute-line",
    "block/list/ordered/start-",
    "block/list/ordered/numbered-marker",
    "block/list/ordered/implicit-start-from-marker",
    "block/list/ordered/list-continuation-between-siblings",
    "block/list/ordered/principal-interrupted",
    "block/list/ordered/ventilated",
    // Callout lists
    "block/list/callout/",
    // Paragraph separated by block attribute line
    "block/paragraph/separated-by-block-attribute-line",
    // Paragraph separated by list continuation
    "block/paragraph/separated-by-list-continuation",
    "block/paragraph/sole-list-continuation",
    // Paragraph with config
    "block/paragraph/paragraph-empty-lines-paragraph-",
    // Description list complex features
    "block/dlist/attached-block",
    "block/dlist/attached-paragraphs",
    "block/dlist/indented-principal-below-term",
    "block/dlist/indented-sibling-following-nested-list",
    "block/dlist/list-continuation-between-siblings",
    "block/dlist/ancestor-list-like",
    "block/dlist/parent-list-indented-marker",
    "block/dlist/nested",
    // Dlist: wrapped principal (continuation lines)
    "block/dlist/wrapped-principal",
    // Dlist: multiple terms per item — parser creates separate items
    "block/dlist/multiple-terms",
    // Document-level tests
    "block/document/",
    // Attached indented blocks — parser preserves leading space
    "block/list/unordered/attached-indented-block-",
    "block/list/ordered/attached-indented-block",
    // Literal: [normal] style on indented block converts to Paragraph
    "block/literal/indented-with-normal-style",
    // Literal: source style creates Literal instead of Listing
    "block/literal/source-style-with-language",
    // Sidebar: orphaned metadata creates extra block
    "block/sidebar/orphaned-metadata",
    // Sidebar advanced
    "block/sidebar/containing-blocks-separated-by-empty-lines",
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

        let parser = Parser::new(&input);
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
