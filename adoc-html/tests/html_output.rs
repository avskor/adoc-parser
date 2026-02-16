use adoc_html::to_html;

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

    let html = to_html(input);

    // Document header
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
    assert!(html.contains("<li>Item one</li>"));
    assert!(html.contains("<ol>"));
    assert!(html.contains("<li>First</li>"));

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
    assert!(html.contains("&lt;div class=&quot;test&quot;&gt;&amp;amp;&lt;/div&gt;"));
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
