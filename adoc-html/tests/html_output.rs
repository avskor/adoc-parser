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
    // `<<introduction>>` (lowercase) matches neither the `_introduction` id nor
    // the "Introduction" title (case-sensitive), so Asciidoctor renders the
    // bracketed-id fallback as link text.
    assert!(html.contains("<a href=\"#introduction\">[introduction]</a>"));

    // Sub/superscript
    assert!(html.contains("<sub>2</sub>"));
    assert!(html.contains("<sup>2</sup>"));
}

#[test]
fn test_setext_doctitle_enables_compat_mode_html() {
    // A two-line (underlined) doctitle renders as <h1> and turns on
    // compat-mode, so single-quoted text becomes emphasis in the body.
    let input = "Document Title\n==============\n\nText with 'quoted' word.";
    let html = to_html_with_options(input, adoc_html::HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("<h1>Document Title</h1>"), "doctitle h1 missing: {html}");
    assert!(html.contains("<em>quoted</em>"), "compat-mode emphasis missing: {html}");
}

#[test]
fn test_setext_section_renders_as_heading_html() {
    // `-` underline → level 1 section (sect1, <h2>).
    let input = "Section Title\n-------------\n\nbody";
    let html = to_html(input);
    assert!(html.contains("class=\"sect1\""), "sect1 wrapper missing: {html}");
    assert!(html.contains("id=\"_section_title\""), "section id missing: {html}");
    assert!(html.contains("Section Title</h2>"), "h2 heading missing: {html}");
}

#[test]
fn test_setext_underline_not_confused_with_example_terminator_html() {
    // The closing `====` of an example block must not be read as a setext
    // underline for the line above it — `2.3` stays a paragraph.
    let input = "====\n2.3\n====";
    let html = to_html(input);
    assert!(html.contains("class=\"exampleblock\""), "example block missing: {html}");
    assert!(html.contains("<p>2.3</p>"), "content paragraph missing: {html}");
    assert!(!html.contains("<h1>2.3</h1>"), "2.3 wrongly became a heading: {html}");
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
fn test_literal_paragraph_absorbs_following_lines_html() {
    // An indented opener turns the run into one literal block; the following
    // non-indented lines are absorbed verbatim (not split into a paragraph), and
    // the opener's leading whitespace is kept when the common indent is 0. This
    // matches Asciidoctor for indented diagram-source open-block content
    // (e.g. `  node1 -> node2\n}\n@enddot`).
    let input = "  node1 -> node2\n}\n@enddot\n";
    let html = to_html(input);
    assert!(
        html.contains("<pre>  node1 -&gt; node2\n}\n@enddot</pre>"),
        "literal block must absorb non-indented continuation lines verbatim: {html}"
    );
    // Exactly one literal block, no stray paragraph for `}`/`@enddot`.
    assert_eq!(html.matches("class=\"literalblock\"").count(), 1, "{html}");
    assert!(!html.contains("<p>}"), "`}}` must not split into its own paragraph: {html}");
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
    // Section div does not carry bibliography class; it propagates to child list
    assert!(html.contains("class=\"sect1\""));
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
    assert!(html.contains("<div id=\"intro\" class=\"paragraph lead\">"), "id/role on wrapper div. Got: {html}");
    assert!(html.contains("<p>This is the introduction.</p>"), "p should be plain. Got: {html}");
}

#[test]
fn test_lead_paragraph() {
    let input = "\
[.lead]
This is a lead paragraph.";

    let html = to_html(input);
    assert!(html.contains("class=\"paragraph lead\""), "Expected lead class on div. Got: {html}");
    assert!(html.contains("<p>This is a lead paragraph.</p>"), "p should be plain. Got: {html}");
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
fn test_standalone_nofooter_attribute() {
    let html = to_html_with_options(":nofooter:\n\nHello", HtmlOptions {
        standalone: true,
        last_updated: Some("2026-03-01 12:00:00 +0300".to_string()),
        ..Default::default()
    });
    assert!(!html.contains("<div id=\"footer\">"), "nofooter should suppress footer. Got: {html}");
    assert!(!html.contains("Last updated"), "nofooter should suppress last_updated. Got: {html}");
}

#[test]
fn test_standalone_footer_present_by_default() {
    let html = to_html_with_options("Hello", HtmlOptions {
        standalone: true,
        ..Default::default()
    });
    assert!(html.contains("<div id=\"footer\">"), "footer should be present by default. Got: {html}");
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
fn test_xref_block_anchor_reftext() {
    // A block anchor `[[id,reftext]]` registers reference text; an unlabeled
    // `<<id>>` resolves to it instead of the `[id]` fallback (п.20).
    let input = "[[myid,My Ref]]\nText with <<myid>>.";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#myid\">My Ref</a>"),
        "block anchor reftext should resolve. Got: {html}"
    );
}

#[test]
fn test_xref_named_reftext_attribute() {
    // The named `[reftext=…]` form is equivalent to `[[id,reftext]]`.
    let input = "[#myid,reftext=My Ref]\nText with <<myid>>.";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#myid\">My Ref</a>"),
        "named reftext attribute should resolve. Got: {html}"
    );
}

#[test]
fn test_xref_reftext_beats_block_title() {
    // An explicit reftext outranks a block's own title for an unlabeled xref.
    let input = "[[myid,Ref Text]]\n.Block Title\n====\nbody\n====\n\nSee <<myid>>.";
    let html = to_html(input);
    assert!(
        html.contains("<a href=\"#myid\">Ref Text</a>"),
        "explicit reftext should win over block title. Got: {html}"
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
        html.contains("<a href=\"#nonexistent\">[nonexistent]</a>"),
        "unresolvable xref should fall back to bracketed target id. Got: {html}"
    );
}

#[test]
fn test_xref_to_block_whose_title_contains_xref() {
    // The registered title HTML of `blk` itself holds xref placeholders; the
    // reference to `blk` must resolve them instead of leaking a raw sentinel.
    let input = "[#blk]\n.See <<Later>>\n----\nx\n----\n\nRef: <<blk>>\n\n== Later\n\ntext";
    let html = to_html(input);
    assert!(
        html.contains("Ref: <a href=\"#blk\">See <a href=\"#_later\">Later</a></a>"),
        "xref to block must resolve nested xref inside the block title. Got: {html}"
    );
    assert!(
        !html.contains('\u{0}'),
        "no placeholder sentinel may leak into output. Got: {html:?}"
    );
}

#[test]
fn test_attr_ref_in_inline_role_resolves() {
    // A defined attribute used as an inline role (shorthand `[.{name}]`) resolves
    // to its value in the rendered `class`, matching Asciidoctor's global
    // attributes substitution running after quotes.
    let html = to_html(":rn: fancy\n\n[.{rn}]_y_");
    assert!(
        html.contains("<em class=\"fancy\">y</em>"),
        "defined attr ref in inline role must resolve. Got: {html}"
    );

    // An undefined reference is kept literal (the `attribute-missing` default of
    // `skip`), exactly as Asciidoctor renders `class="{undef}"`.
    let html = to_html("[.{undef}]_y_");
    assert!(
        html.contains("<em class=\"{undef}\">y</em>"),
        "undefined attr ref in inline role must stay literal. Got: {html}"
    );

    // No raw extraction sentinel (control bytes) may leak into the class.
    assert!(
        !html.bytes().any(|b| b == 0x01 || b == 0x02),
        "no extraction sentinel may leak into output. Got: {html:?}"
    );
}

#[test]
fn test_attr_ref_in_link_and_image_role_resolves() {
    // A `{name}` written as a named role on a link or inline image survives the
    // `macros` pass (which runs before `attributes`) as a literal in the role
    // field; the renderer resolves it against the document attributes, exactly
    // like the inline phrase roles above. Matches Asciidoctor byte-for-byte.
    let html = to_html(":rn: fancy\n\nlink:https://example.com[Home,role={rn}]");
    assert!(
        html.contains("<a href=\"https://example.com\" class=\"fancy\">Home</a>"),
        "defined attr ref in link role must resolve. Got: {html}"
    );

    let html = to_html(":rn: fancy\n\nimage:img.png[Logo,role={rn}]");
    assert!(
        html.contains("<span class=\"image fancy\"><img src=\"img.png\" alt=\"Logo\">"),
        "defined attr ref in inline-image role must resolve. Got: {html}"
    );

    // Undefined reference is kept literal (`attribute-missing` default `skip`),
    // and a quoted multi-role value resolves only the reference, keeping the
    // rest verbatim.
    let html = to_html("link:https://example.com[Home,role={undef}]");
    assert!(
        html.contains("class=\"{undef}\""),
        "undefined attr ref in link role must stay literal. Got: {html}"
    );

    let html = to_html(":rn: fancy\n\nlink:https://example.com[Home,role=\"{rn} external\"]");
    assert!(
        html.contains("class=\"fancy external\""),
        "attr ref inside a quoted multi-role value must resolve in place. Got: {html}"
    );

    // No raw extraction sentinel (control bytes) may leak into the class.
    assert!(
        !html.bytes().any(|b| b == 0x01 || b == 0x02),
        "no extraction sentinel may leak into output. Got: {html:?}"
    );
}

#[test]
fn test_attr_ref_in_link_and_image_target_resolves() {
    // A `{name}` in a link/image macro target (`link:{u}[…]`/`image:{p}[…]`)
    // survives the `macros` pass — which runs before `attributes` — as a literal
    // in the `url`/`target` field; the renderer resolves it against the document
    // attributes, matching Asciidoctor (which substitutes attributes first).
    let html = to_html(":u: https://example.com\n\nlink:{u}[home]");
    assert!(
        html.contains("<a href=\"https://example.com\">home</a>"),
        "defined attr ref in link target must resolve. Got: {html}"
    );

    // A trailing path after the reference is kept verbatim.
    let html = to_html(":u: https://example.com\n\nlink:{u}/issues[issues]");
    assert!(
        html.contains("<a href=\"https://example.com/issues\">issues</a>"),
        "attr ref + path in link target must resolve and keep the path. Got: {html}"
    );

    // Inline image target resolves before `imagesdir` is applied.
    let html = to_html(":p: tiger.png\n:imagesdir: img\n\nimage:{p}[Tiger]");
    assert!(
        html.contains("<img src=\"img/tiger.png\" alt=\"Tiger\">"),
        "attr ref in image target must resolve, then imagesdir prefixes. Got: {html}"
    );

    // The inline image `link=` href resolves too.
    let html = to_html(":u: https://example.com\n\nimage:cat.png[Cat,link={u}]");
    assert!(
        html.contains("<a class=\"image\" href=\"https://example.com\"><img src=\"cat.png\""),
        "attr ref in inline-image link target must resolve. Got: {html}"
    );

    // A bare link (empty bracket) repeats the target as visible text; both the
    // href and that text resolve the reference.
    let html = to_html(":u: https://example.com\n\nlink:{u}[]");
    assert!(
        html.contains("<a href=\"https://example.com\" class=\"bare\">https://example.com</a>"),
        "bare link must resolve the attr ref in both href and visible text. Got: {html}"
    );

    // Undefined reference is kept literal (`attribute-missing` default `skip`).
    let html = to_html("link:{undef}[gone]");
    assert!(
        html.contains("<a href=\"{undef}\">gone</a>"),
        "undefined attr ref in link target must stay literal. Got: {html}"
    );

    // No raw extraction sentinel (control bytes) may leak into the output.
    assert!(
        !html.bytes().any(|b| b == 0x01 || b == 0x02),
        "no extraction sentinel may leak into output. Got: {html:?}"
    );
}

#[test]
fn test_attr_ref_in_image_target_alt_link_resolves() {
    // Block and inline image macros carry an attribute reference written in the
    // target, alt text or `link=` href literally past the `macros` pass; the
    // renderer resolves each against the document attributes (attributes-first,
    // like Asciidoctor) before `imagesdir` is prefixed to the target.
    let doc = ":p: tiger.png\n:imagesdir: img\n:u: https://example.com\n:a: My Tiger\n\n";

    // Block image: target + alt both resolve, then imagesdir prefixes the src.
    let html = to_html(&format!("{doc}image::{{p}}[{{a}}]"));
    assert!(
        html.contains("<img src=\"img/tiger.png\" alt=\"My Tiger\">"),
        "block image target and alt attr refs must resolve. Got: {html}"
    );

    // Empty alt derives the auto-alt from the *resolved* basename ("tiger").
    let html = to_html(&format!("{doc}image::{{p}}[]"));
    assert!(
        html.contains("<img src=\"img/tiger.png\" alt=\"tiger\">"),
        "block image auto-alt must come from the resolved target basename. Got: {html}"
    );

    // Block image `link=` href resolves; the target still resolves alongside it.
    let html = to_html(&format!("{doc}image::{{p}}[Cat,link={{u}}]"));
    assert!(
        html.contains("<a class=\"image\" href=\"https://example.com\"><img src=\"img/tiger.png\" alt=\"Cat\">"),
        "block image link href and target attr refs must resolve. Got: {html}"
    );

    // Inline image: target resolved by an earlier change; the alt resolves here.
    let html = to_html(&format!("{doc}image:{{p}}[{{a}}]"));
    assert!(
        html.contains("<span class=\"image\"><img src=\"img/tiger.png\" alt=\"My Tiger\"></span>"),
        "inline image alt attr ref must resolve. Got: {html}"
    );

    // Inline empty alt also derives from the resolved basename.
    let html = to_html(&format!("{doc}image:{{p}}[]"));
    assert!(
        html.contains("<img src=\"img/tiger.png\" alt=\"tiger\">"),
        "inline image auto-alt must come from the resolved target basename. Got: {html}"
    );

    // An undefined reference is kept literal in both target and alt
    // (`attribute-missing` default `skip`); imagesdir still prefixes it.
    let html = to_html(":imagesdir: img\n\nimage::{undef}[Alt {undef2}]");
    assert!(
        html.contains("<img src=\"img/{undef}\" alt=\"Alt {undef2}\">"),
        "undefined image target/alt refs must stay literal. Got: {html}"
    );

    // No raw extraction sentinel (control bytes) may leak into the output.
    assert!(
        !html.bytes().any(|b| b == 0x01 || b == 0x02),
        "no extraction sentinel may leak into output. Got: {html:?}"
    );
}

#[test]
fn test_attr_ref_in_xref_target_resolves() {
    // An xref target (`xref:{rel}.adoc[]`, `xref:{frag}[]`, `<<{id}>>`) carries
    // its attribute reference literally past the `macros` pass; the renderer
    // resolves it against the document attributes before classifying the target
    // as inter-document vs internal, matching Asciidoctor (attributes first).
    let doc = ":rel: intro\n:frag: section-one\n:secid: _real_section\n\n";

    // Inter-document target: `{rel}` resolves, then `.adoc` rewrites to `.html`.
    let html = to_html(&format!("{doc}See xref:{{rel}}.adoc[Intro]."));
    assert!(
        html.contains("<a href=\"intro.html\">Intro</a>"),
        "inter-document xref target attr ref must resolve. Got: {html}"
    );

    // Inter-document target with a `#fragment`: both `{rel}` and `{frag}` resolve.
    let html = to_html(&format!("{doc}See xref:{{rel}}.adoc#{{frag}}[Deep]."));
    assert!(
        html.contains("<a href=\"intro.html#section-one\">Deep</a>"),
        "xref target with attr-ref path and fragment must resolve. Got: {html}"
    );

    // Internal xref to an unknown id: the resolved id drives both the href and
    // the bracketed fallback label (`#section-one` / `[section-one]`).
    let html = to_html(&format!("{doc}See xref:{{frag}}[]."));
    assert!(
        html.contains("<a href=\"#section-one\">[section-one]</a>"),
        "internal xref target attr ref must resolve in href and fallback. Got: {html}"
    );

    // Internal xref whose resolved id matches a real section: the natural cross
    // reference picks up the section title as the link text.
    let html = to_html(&format!("{doc}See xref:{{secid}}[].\n\n== Real Section"));
    assert!(
        html.contains("<a href=\"#_real_section\">Real Section</a>"),
        "resolved xref id must still resolve to the section title. Got: {html}"
    );

    // The angle-bracket form resolves the attribute reference identically.
    let html = to_html(&format!("{doc}<<{{secid}}>>.\n\n== Real Section"));
    assert!(
        html.contains("<a href=\"#_real_section\">Real Section</a>"),
        "angle-bracket xref attr ref must resolve. Got: {html}"
    );

    // An undefined reference is kept literal (`attribute-missing` default `skip`):
    // `xref:{undef}.adoc[]` still rewrites the extension, internal `{undef}` stays.
    let html = to_html("Doc xref:{undef}.adoc[F]. Internal xref:{undef2}[].");
    assert!(
        html.contains("<a href=\"{undef}.html\">F</a>"),
        "undefined inter-document xref target stays literal. Got: {html}"
    );
    assert!(
        html.contains("<a href=\"#{undef2}\">[{undef2}]</a>"),
        "undefined internal xref target stays literal. Got: {html}"
    );

    // No extraction sentinel (control bytes) may leak into the output.
    assert!(
        !html.bytes().any(|b| b == 0x01 || b == 0x02),
        "no extraction sentinel may leak into output. Got: {html:?}"
    );
}
