use std::path::{Path, PathBuf};

use adoc_html_tests::normalize::assert_html_eq;

/// Patterns to skip (relative to fixtures root).
/// These fixtures test features not yet implemented in the renderer.
const SKIP_PATTERNS: &[&str] = &[
    // :sectanchors: attribute (anchor links in section headings) not implemented
    "document/attribute-sectanchors",
    // :showtitle: attribute (render document title in embedded mode) not implemented
    "document/attribute-showtitle",
    // Bibliography class propagation to nested ulist/ul not implemented
    "inline/bibliography-anchor",
];

fn should_skip(test_path: &str) -> bool {
    SKIP_PATTERNS
        .iter()
        .any(|pattern| test_path.contains(pattern))
}

/// Find all .adoc + .expected.html pairs under the fixtures directory.
fn find_fixture_pairs(root: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut pairs = Vec::new();
    collect_pairs(root, &mut pairs);
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

fn collect_pairs(dir: &Path, pairs: &mut Vec<(PathBuf, PathBuf)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_pairs(&path, pairs);
        } else if path.extension().is_some_and(|e| e == "adoc") {
            let expected = path.with_extension("expected.html");
            if expected.exists() {
                pairs.push((path, expected));
            }
        }
    }
}

#[test]
fn html_compatibility_tests() {
    let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");

    if !fixtures_root.exists() {
        eprintln!(
            "WARNING: fixtures directory not found at {}",
            fixtures_root.display()
        );
        return;
    }

    let pairs = find_fixture_pairs(&fixtures_root);
    if pairs.is_empty() {
        eprintln!(
            "WARNING: No fixture pairs found. Run: bash adoc-html-tests/scripts/generate-expected.sh"
        );
        return;
    }

    let mut passed = 0;
    let mut skipped = 0;
    let mut failed = Vec::new();

    for (adoc_path, expected_path) in &pairs {
        let rel_path = adoc_path
            .strip_prefix(&fixtures_root)
            .unwrap_or(adoc_path)
            .to_string_lossy()
            .replace('\\', "/");

        if should_skip(&rel_path) {
            skipped += 1;
            eprintln!("  SKIP: {rel_path}");
            continue;
        }

        let input = match std::fs::read_to_string(adoc_path) {
            Ok(s) => s,
            Err(e) => {
                failed.push((rel_path, format!("Failed to read input: {e}")));
                continue;
            }
        };

        let expected_html = match std::fs::read_to_string(expected_path) {
            Ok(s) => s,
            Err(e) => {
                failed.push((rel_path, format!("Failed to read expected HTML: {e}")));
                continue;
            }
        };

        let actual_html = adoc_html::to_html(&input);

        match assert_html_eq(&expected_html, &actual_html) {
            Ok(()) => {
                passed += 1;
                eprintln!("  PASS: {rel_path}");
            }
            Err(diff) => {
                failed.push((rel_path, diff));
            }
        }
    }

    let total = passed + skipped + failed.len();
    eprintln!("\n=== HTML Compatibility Results ===");
    eprintln!(
        "Total: {total}, Passed: {passed}, Skipped: {skipped}, Failed: {}",
        failed.len()
    );

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
