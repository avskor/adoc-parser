//! Integration tests for the `adoc` binary, driven through `std::process`.
//!
//! `CARGO_BIN_EXE_adoc` points at the freshly built binary and
//! `CARGO_TARGET_TMPDIR` is a per-run scratch directory — both are provided by
//! Cargo to integration tests, so no extra dependencies are needed.

use std::path::PathBuf;
use std::process::Command;

/// Run the `adoc` binary against `source`, written to a temp file named `name`,
/// with the given extra args, and return its stdout.
fn run_adoc(name: &str, source: &str, args: &[&str]) -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let path = dir.join(name);
    std::fs::write(&path, source).expect("write input");
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(args)
        .arg(&path)
        .output()
        .expect("run adoc");
    assert!(
        output.status.success(),
        "adoc exited with failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

/// The CLI targets the html5 backend, so Asciidoctor's backend intrinsics must
/// be defined both for preprocessor conditionals (`ifdef::backend-html5[]`) and
/// for inline references (`{basebackend}` / `{filetype}` / `{outfilesuffix}`).
#[test]
fn seeds_backend_intrinsics() {
    let source = "= Probe\n\n\
        ifdef::backend-html5[YES backend-html5]\n\
        ifndef::backend-html5[NO backend-html5]\n\n\
        ifdef::basebackend-html[YES basebackend-html]\n\n\
        ifdef::filetype-html[YES filetype-html]\n\n\
        backend={backend} basebackend={basebackend} filetype={filetype} suffix={outfilesuffix}\n";
    let html = run_adoc("intrinsics.adoc", source, &["--no-standalone"]);

    assert!(html.contains("YES backend-html5"), "html was: {html}");
    assert!(!html.contains("NO backend-html5"), "html was: {html}");
    assert!(html.contains("YES basebackend-html"), "html was: {html}");
    assert!(html.contains("YES filetype-html"), "html was: {html}");
    assert!(
        html.contains("backend=html5 basebackend=html filetype=html suffix=.html"),
        "html was: {html}"
    );
}

/// A header `:outfilesuffix:` entry must still override the seeded default —
/// the intrinsics are seeded unlocked, like Asciidoctor allows for the suffix.
#[test]
fn header_can_override_outfilesuffix() {
    let source = "= Probe\n:outfilesuffix: .adoc\n\nsuffix={outfilesuffix}\n";
    let html = run_adoc("suffix_override.adoc", source, &["--no-standalone"]);
    assert!(html.contains("suffix=.adoc"), "html was: {html}");
}
