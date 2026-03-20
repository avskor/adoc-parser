use adoc_html::{to_html, to_html_with_options, HtmlOptions};

#[test]
fn test_full_document() {
    let input = "\
= My Document

== Introduction

This is a *bold* and _italic_ paragraph.

== Lists

* Item one
* Item two
* Item three

. First
. Second

== Code

[source,rust]
----
fn hello() {
    println!(\"world\");
}
----

NOTE: Important note here.

'''

== Links

Visit link:https://example.com[Example Site] for more info.

See <<introduction>> for the intro.

H~2~O and E=mc^2^.";

    let html = to_html_with_options(input, adoc_html::HtmlOptions { standalone: true, ..Default::default() });

    // Document header (standalone mode)
    assert!(html.contains("<h1>My Document</h1>"));

    // Sections
    assert!(html.contains("id=\"_introduction\""));
    assert!(html.contains("id=\"_lists\""));
    assert!(html.contains("id=\"_code\""));
    assert!(html.contains("id=\"_links\""));

    // Inline formatting
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<em>italic</em>"));

    // Lists
    assert!(html.contains("<ul>"));
    assert!(html.contains("<li>\n<p>Item one</p>\n</li>"));
    assert!(html.contains("<ol class=\"arabic\">"));
    assert!(html.contains("<li>\n<p>First</p>\n</li>"));

    // Source block
    assert!(html.contains("language-rust"));
    assert!(html.contains("fn hello()"));

    // Admonition
    assert!(html.contains("admonitionblock note"));

    // Thematic break
    assert!(html.contains("<hr>"));

    // Link
    assert!(html.contains("<a href=\"https://example.com\">Example Site</a>"));

    // Cross reference
    assert!(html.contains("<a href=\"#introduction\">introduction</a>"));

    // Sub/superscript
    assert!(html.contains("<sub>2</sub>"));
    assert!(html.contains("<sup>2</sup>"));
}

#[test]
fn test_html_escaping_in_source_block() {
    let input = "[source,html]\n----\n<div class=\"test\">&amp;</div>\n----";
    let html = to_html(input);
    assert!(html.contains("&lt;div class=\"test\"&gt;&amp;amp;&lt;/div&gt;"));
}

#[test]
fn test_nested_sections_html() {
    let input = "== Level 2\n\n=== Level 3\n\nContent\n\n== Another Level 2\n\nMore content";
    let html = to_html(input);

    // Both sections should have correct heading levels
    assert!(html.contains("<h2"));
    assert!(html.contains("<h3"));

    // Content should be in paragraphs
    assert!(html.contains("<p>Content</p>"));
    assert!(html.contains("<p>More content</p>"));
}

#[test]
fn test_block_image_html() {
    let html = to_html("image::photo.jpg[My Photo]");
    assert!(html.contains("<img src=\"photo.jpg\" alt=\"My Photo\">"));
}

#[test]
fn test_inline_image_html() {
    let html = to_html("See image:icon.png[icon] here.");
    assert!(html.contains("<img src=\"icon.png\" alt=\"icon\">"));
}

#[test]
fn test_page_break_html() {
    let html = to_html("Before\n\n<<<\n\nAfter");
    assert!(html.contains("page-break-after"));
}

#[test]
fn test_hard_break_html() {
    let html = to_html("Line one +\nLine two");
    assert!(html.contains("<br>"));
}

#[test]
fn test_smart_double_quotes_html() {
    let html = to_html("\"`curly`\"");
    assert!(html.contains("\u{201C}curly\u{201D}"));
}

#[test]
fn test_smart_single_quotes_html() {
    let html = to_html("'`curly`'");
    assert!(html.contains("\u{2018}curly\u{2019}"));
}

#[test]
fn test_smart_quotes_with_bold_html() {
    let html = to_html("\"`*bold* text`\"");
    assert!(html.contains("\u{201C}<strong>bold</strong> text\u{201D}"));
}

#[test]
fn test_bibliography_section_with_anchors() {
    let input = "\
[bibliography]
== References

* [[[pp]]] Ralph Johnson. Pragmatic Programmer.
* [[[gof, 2]]] Gang of Four. Design Patterns.";

    let html = to_html(input);
    // Bibliography anchor with default label
    assert!(html.contains("<a id=\"pp\"></a>[pp]"));
    // Bibliography anchor with custom label
    assert!(html.contains("<a id=\"gof\"></a>[2]"));
    // Section has bibliography style class
    assert!(html.contains("class=\"sect1 bibliography\""));
}

#[test]
fn test_bibliography_anchor_with_cross_reference() {
    let input = "\
See <<pp>> for details.

[bibliography]
== References

* [[[pp]]] Pragmatic Programmer.";

    let html = to_html(input);
    assert!(html.contains("<a href=\"#pp\">"));
    assert!(html.contains("<a id=\"pp\"></a>[pp]"));
}

#[test]
fn test_section_with_id_still_works() {
    let input = "\
[#myid]
== My Section

Some text.";

    let html = to_html(input);
    assert!(html.contains("id=\"myid\""));
}

#[test]
fn test_block_metadata_paragraph_id_and_role() {
    let input = "\
[#intro.lead]
This is the introduction.";

    let html = to_html(input);
    assert!(html.contains("<p id=\"intro\" class=\"lead\">"));
    assert!(html.contains("This is the introduction."));
}

#[test]
fn test_lead_paragraph() {
    let input = "\
[.lead]
This is a lead paragraph.";

    let html = to_html(input);
    assert!(html.contains("<p class=\"lead\">"), "Expected lead class. Got: {html}");
    assert!(html.contains("This is a lead paragraph."));
}

// ─── Standalone mode tests ───

#[test]
fn test_standalone_empty_document() {
    let html = to_html_with_options("", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.starts_with("<!DOCTYPE html>"), "should start with DOCTYPE. Got: {html}");
    assert!(html.contains("<title>Untitled</title>"), "empty doc should have Untitled title. Got: {html}");
    assert!(html.contains("</html>"), "should close html tag. Got: {html}");
}

#[test]
fn test_standalone_with_title() {
    let html = to_html_with_options("= My Title\n\nHello", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.contains("<title>My Title</title>"), "should use document title. Got: {html}");
}

#[test]
fn test_standalone_body_class_article() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.contains("<body class=\"article\">"), "default doctype should be article. Got: {html}");
}

#[test]
fn test_standalone_has_style_block() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.contains("<style>"), "should contain style tag. Got: {html}");
    assert!(html.contains("</style>"), "should close style tag. Got: {html}");
}

#[test]
fn test_standalone_meta_tags() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.contains("<meta charset=\"UTF-8\">"), "should have charset meta. Got: {html}");
    assert!(html.contains("name=\"viewport\""), "should have viewport meta. Got: {html}");
    assert!(html.contains("name=\"generator\" content=\"adoc-parser\""), "should have generator meta. Got: {html}");
}

#[test]
fn test_standalone_docinfo_head() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        docinfo_head: Some("<link rel=\"icon\" href=\"favicon.ico\">".to_string()),
        ..Default::default()
    });
    assert!(html.contains("<link rel=\"icon\" href=\"favicon.ico\">"), "docinfo_head should be in <head>. Got: {html}");
    let head_end = html.find("</head>").unwrap();
    let docinfo_pos = html.find("<link rel=\"icon\" href=\"favicon.ico\">").unwrap();
    assert!(docinfo_pos < head_end, "docinfo_head should be inside <head>. Got: {html}");
}

#[test]
fn test_standalone_docinfo_footer() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        docinfo_footer: Some("<script src=\"app.js\"></script>".to_string()),
        ..Default::default()
    });
    assert!(html.contains("<script src=\"app.js\"></script>"), "docinfo_footer should be present. Got: {html}");
    let body_end = html.find("</body>").unwrap();
    let docinfo_pos = html.find("<script src=\"app.js\"></script>").unwrap();
    assert!(docinfo_pos < body_end, "docinfo_footer should be inside <body>. Got: {html}");
}

#[test]
fn test_standalone_last_updated() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        last_updated: Some("2026-03-01 12:00:00 +0300".to_string()),
        ..Default::default()
    });
    assert!(html.contains("Last updated 2026-03-01 12:00:00 +0300"), "should contain last_updated. Got: {html}");
    assert!(html.contains("<div id=\"footer-text\">"), "should have footer-text div. Got: {html}");
}

#[test]
fn test_standalone_content_wrapped() {
    let html = to_html_with_options("= Title\n\nHello", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.contains("<div id=\"content\">"), "content should be wrapped. Got: {html}");
    assert!(html.contains("<div id=\"header\">"), "should have id=header in standalone. Got: {html}");
}

#[test]
fn test_standalone_no_title_has_empty_header() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.contains("<div id=\"header\">\n</div>"), "should have empty header div. Got: {html}");
    assert!(html.contains("<div id=\"content\">"), "should have content div. Got: {html}");
}

#[test]
fn test_to_html_still_fragment() {
    let html = to_html("= Title\n\nHello");
    assert!(!html.contains("<!DOCTYPE"), "to_html should NOT produce DOCTYPE. Got: {html}");
    assert!(!html.contains("<html"), "to_html should NOT produce <html>. Got: {html}");
}

#[test]
fn test_standalone_footer_div() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.contains("<div id=\"footer\">"), "should have footer div. Got: {html}");
}

#[test]
fn test_xref_unlabeled_resolves_section_title() {
    let input = "[#requests]\n== Запросы\n\nСм. <<requests>>";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#requests\">Запросы</a>"),
        "unlabeled xref should resolve to section title. Got: {html}"
    );
}

#[test]
fn test_xref_forward_reference() {
    let input = "См. <<later-section>>\n\n[#later-section]\n== Later Section";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#later-section\">Later Section</a>"),
        "forward xref should resolve to section title. Got: {html}"
    );
}

#[test]
fn test_xref_with_explicit_label_unchanged() {
    let input = "[#requests]\n== Запросы\n\nСм. <<requests,Все запросы>>";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#requests\">Все запросы</a>"),
        "xref with explicit label should use that label. Got: {html}"
    );
}

#[test]
fn test_xref_hash_prefix_resolves_section_title() {
    let input = "[#divide-large-responses]\n== Разделение больших ответов\n\nСм. <<#divide-large-responses>>";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#divide-large-responses\">Разделение больших ответов</a>"),
        "xref with # prefix should strip # and resolve section title. Got: {html}"
    );
}

#[test]
fn test_xref_hash_prefix_no_double_hash() {
    let input = "См. <<#some-id>>";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#some-id\">"),
        "xref with # prefix should not produce ## in href. Got: {html}"
    );
    assert!(
        !html.contains("##"),
        "href must not contain double ##. Got: {html}"
    );
}

#[test]
fn test_xref_unresolvable_falls_back_to_id() {
    let input = "См. <<nonexistent>>";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#nonexistent\">nonexistent</a>"),
        "unresolvable xref should fall back to target ID. Got: {html}"
    );
}
