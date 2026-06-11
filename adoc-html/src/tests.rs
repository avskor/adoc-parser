use super::*;

#[test]
fn test_simple_paragraph() {
    let html = to_html("Hello world.");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>Hello world.</p>\n</div>\n");
}

#[test]
fn test_nul_byte_stripped_from_text() {
    // D5: NUL bytes are stripped from escaped text so they can never collide
    // with the internal xref placeholder sentinel (\x00XREF_N\x00).
    let html = to_html("a\u{0}b");
    assert!(!html.contains('\u{0}'), "NUL leaked into output: {html:?}");
    assert!(html.contains("ab"), "text around NUL not preserved: {html}");
}

#[test]
fn test_attribute_escaping_invariant() {
    // D1/D7 systemic rule: every value entering an HTML attribute is HTML-escaped.
    // Each case injects a payload through a distinct user-controllable attribute
    // channel and asserts BOTH:
    //   (a) the raw breakout substring is ABSENT — no attribute/tag injection;
    //   (b) the escaped form is PRESENT — proving the payload actually reached the
    //       attribute, so the case can't pass vacuously by being dropped upstream.
    //
    // Angle payload `<XSS>` is used for channels whose tokenizer strips `"`/space
    // (e.g. ordered-list style); quote payload `XSS"Q` for the rest.
    const A_RAW: &str = "<XSS>";
    const A_ESC: &str = "&lt;XSS&gt;";
    const Q_RAW: &str = "XSS\"Q";
    const Q_ESC: &str = "XSS&quot;Q";

    // (input, raw_breakout_must_be_absent, escaped_form_must_be_present)
    let cases: &[(&str, &str, &str)] = &[
        // D7: ordered-list style flows raw into <ol class>/<div class> — the hole this fixes.
        ("[<XSS>]\n. item", A_RAW, A_ESC),
        // image align=other → image_base_class → write_meta_attrs boundary (no double-escape).
        ("image::a.png[align=<XSS>]", A_RAW, A_ESC),
        // section id rendered onto the heading element.
        ("[#s<XSS>]\n== Title\n\nbody", A_RAW, A_ESC),
        // icon role appended to the <i> class list.
        ("icon:home[role=<XSS>]", A_RAW, A_ESC),
        // block id / role via write_meta_attrs.
        ("[#XSS\"Q]\nHello", Q_RAW, Q_ESC),
        ("[.XSS\"Q]\nHello", Q_RAW, Q_ESC),
        // link href with a quote in the URL.
        ("https://example.test/XSS\"Q[label]", Q_RAW, Q_ESC),
        // image target (src) and auto alt.
        ("image::XSS\"Q.png[]", Q_RAW, Q_ESC),
        // video width — the D1 media channel, now routed through write_attr.
        ("video::v[width=XSS\"Q]", Q_RAW, Q_ESC),
        // icon title.
        ("icon:home[title=XSS\"Q]", Q_RAW, Q_ESC),
    ];

    for (input, raw, esc) in cases {
        let html = to_html(input);
        assert!(
            !html.contains(raw),
            "attribute breakout for input {input:?}: raw {raw:?} present:\n{html}"
        );
        assert!(
            html.contains(esc),
            "vacuous case for input {input:?}: escaped {esc:?} absent — payload never reached an attribute:\n{html}"
        );
    }
}

#[test]
fn test_attribute_escaping_no_overescape() {
    // The boundary-escape rule must be a no-op for legitimate class tokens: escaping
    // exactly once (never double-escaping) keeps ordinary output byte-for-byte stable.
    let ol = to_html("[loweralpha]\n. item");
    assert!(ol.contains("<div class=\"olist loweralpha\">"), "ol wrapper class corrupted: {ol}");
    assert!(ol.contains("<ol class=\"loweralpha\""), "ol class corrupted: {ol}");

    // image align: float/align tokens are escaped once at the boundary, not in
    // image_base_class — so a plain value stays clean (no &amp;quot; double-escape).
    let img = to_html("image::a.png[align=center]");
    assert!(img.contains("class=\"imageblock text-center\""), "image align class corrupted: {img}");
}

#[test]
fn test_bold_text() {
    let html = to_html("Hello *bold* world.");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>Hello <strong>bold</strong> world.</p>\n</div>\n");
}

#[test]
fn test_italic_text() {
    let html = to_html("Hello _italic_ world.");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>Hello <em>italic</em> world.</p>\n</div>\n");
}

#[test]
fn test_document_title_no_duplicate_h1() {
    let html = to_html_with_options("= Document Title\n\nContent.", HtmlOptions { standalone: true, ..Default::default() });
    // Must produce exactly one <h1> opening tag, not <h1 id="..."><h1>
    let h1_count = html.matches("<h1").count();
    assert_eq!(h1_count, 1, "expected exactly one <h1> tag, got {h1_count}. HTML:\n{html}");
    assert!(
        html.contains("<h1>Document Title</h1>"),
        "expected <h1>Document Title</h1> (no id in header), got:\n{html}"
    );
}

#[test]
fn test_section() {
    let html = to_html("== My Section\n\nContent.");
    assert!(html.contains("<h2 id=\"_my_section\">My Section</h2>"));
    assert!(html.contains("<p>Content.</p>"));
}

#[test]
fn test_unordered_list() {
    let html = to_html("* item 1\n* item 2");
    assert!(html.contains("<div class=\"ulist\">\n<ul>"));
    assert!(html.contains("<li>\n<p>item 1</p>\n</li>"));
    assert!(html.contains("<li>\n<p>item 2</p>\n</li>"));
    assert!(html.contains("</ul>\n</div>"));
}

#[test]
fn test_ordered_list() {
    let html = to_html(". first\n. second");
    assert!(html.contains("<div class=\"olist arabic\">\n<ol"));
    assert!(!html.contains("type="));
    assert!(!html.contains("start="));
    assert!(!html.contains("reversed"));
    assert!(html.contains("<li>\n<p>first</p>\n</li>"));
    assert!(html.contains("<li>\n<p>second</p>\n</li>"));
    assert!(html.contains("</ol>\n</div>"));
}

#[test]
fn test_ordered_list_loweralpha() {
    let html = to_html("[loweralpha]\n. a\n. b");
    assert!(html.contains("<ol class=\"loweralpha\" type=\"a\""), "expected ol with class and type. Got:\n{html}");
    assert!(html.contains("class=\"olist loweralpha\""));
}

#[test]
fn test_ordered_list_upperroman() {
    let html = to_html("[upperroman]\n. x\n. y");
    assert!(html.contains("<ol class=\"upperroman\" type=\"I\""), "expected ol with class and type. Got:\n{html}");
    assert!(html.contains("class=\"olist upperroman\""));
}

#[test]
fn test_ordered_list_start() {
    let html = to_html("[start=3]\n. x\n. y");
    assert!(html.contains("start=\"3\""));
}

#[test]
fn test_ordered_list_reversed() {
    let html = to_html("[%reversed]\n. z\n. y");
    assert!(html.contains(" reversed"));
}

#[test]
fn test_ordered_list_combined() {
    let html = to_html("[loweralpha,start=2]\n. x");
    assert!(html.contains("type=\"a\""));
    assert!(html.contains("start=\"2\""));
    assert!(html.contains("class=\"olist loweralpha\""));
}

#[test]
fn test_source_block() {
    let html = to_html("[source,rust]\n----\nfn main() {\n    println!(\"hello\");\n}\n----");
    assert!(html.contains("language-rust"));
    assert!(html.contains("fn main()"));
    assert!(html.contains("\"hello\""));
}

#[test]
fn test_admonition() {
    let html = to_html("NOTE: This is important.");
    assert!(html.contains("admonitionblock note"));
    assert!(html.contains("This is important."));
}

#[test]
fn test_link() {
    let html = to_html("Visit link:https://example.com[Example].");
    assert!(html.contains("<a href=\"https://example.com\">Example</a>"));
}

#[test]
fn test_link_with_window_html() {
    let html = to_html("link:https://example.com[Example,window=_blank]");
    assert!(html.contains("<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener\">Example</a>"));
}

#[test]
fn test_link_with_nofollow_html() {
    let html = to_html("link:https://example.com[Example,opts=nofollow]");
    assert!(html.contains("<a href=\"https://example.com\" rel=\"nofollow\">Example</a>"));
}

#[test]
fn test_link_with_window_and_nofollow_html() {
    let html = to_html("link:https://example.com[Example,window=_blank,opts=nofollow]");
    assert!(html.contains("<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener nofollow\">Example</a>"));
}

#[test]
fn test_link_no_attrs_unchanged_html() {
    let html = to_html("link:https://example.com[Example]");
    assert!(html.contains("<a href=\"https://example.com\">Example</a>"));
    assert!(!html.contains("target="));
    assert!(!html.contains("rel="));
}

#[test]
fn test_link_passthrough_url_with_spaces() {
    let html = to_html("link:++https://example.com/my page++[Click]");
    assert!(html.contains("<a href=\"https://example.com/my page\">Click</a>"));
}

#[test]
fn test_link_passthrough_url_with_brackets() {
    let html = to_html("link:++https://example.com/path[1]++[Click]");
    assert!(html.contains("<a href=\"https://example.com/path[1]\">Click</a>"));
}

#[test]
fn test_link_macro_empty_text_bare_class() {
    // link:target[] with no explicit text → class="bare" (matches Asciidoctor).
    let html = to_html("See link:LICENSE[] for details.");
    assert!(html.contains("<a href=\"LICENSE\" class=\"bare\">LICENSE</a>"), "{html}");
    // Explicit text → no bare class.
    let html2 = to_html("See link:LICENSE[the license].");
    assert!(html2.contains("<a href=\"LICENSE\">the license</a>"), "{html2}");
    // mailto with empty text is NOT bare.
    let html3 = to_html("mailto:user@example.com[]");
    assert!(html3.contains("<a href=\"mailto:user@example.com\">user@example.com</a>"), "{html3}");
    assert!(!html3.contains("class=\"bare\""), "{html3}");
}

#[test]
fn test_attribute_reference_link_target() {
    // `{url}[text^]` — attributes substitute before macros, so once the URL
    // attribute resolves the trailing `[text^]` forms a link macro with a
    // blank-window target (matches Asciidoctor). No leftover literal bracket.
    let html = to_html(":url-x: https://example.com/foo\n\nSee {url-x}[the page^] now.");
    assert!(
        html.contains("<a href=\"https://example.com/foo\" target=\"_blank\" rel=\"noopener\">the page</a>"),
        "{html}"
    );
    assert!(!html.contains("[the page"), "leftover bracket not consumed: {html}");
    assert!(!html.contains("class=\"bare\""), "{html}");

    // A non-URL attribute value followed by `[...]` stays literal (the
    // re-parsed `value[text]` matches no macro) — same as Asciidoctor.
    let html2 = to_html(":nm: John\n\nName {nm}[bracket] here.");
    assert!(html2.contains("John[bracket]"), "{html2}");
    assert!(!html2.contains("<a "), "{html2}");

    // An undefined attribute keeps both the reference and the brackets.
    let html3 = to_html("Name {undefined-attr}[bracket] here.");
    assert!(html3.contains("{undefined-attr}[bracket]"), "{html3}");
}

#[test]
fn test_attribute_reference_path_before_brackets_link() {
    // `{url}/issues[text]` — a path between `}` and `[` is part of the URL once
    // the attribute resolves, so the whole thing forms a link macro (matches
    // Asciidoctor's attributes-before-macros order). No leftover literal text.
    let html = to_html(
        ":url-repo: https://github.com/asciidoctor/asciidoctor\n\n\
         File a ticket in the {url-repo}/issues[Asciidoctor issue tracker].",
    );
    assert!(
        html.contains(
            "<a href=\"https://github.com/asciidoctor/asciidoctor/issues\">Asciidoctor issue tracker</a>"
        ),
        "{html}"
    );
    assert!(!html.contains("/issues["), "leftover path/bracket not consumed: {html}");
    assert!(!html.contains("class=\"bare\""), "{html}");
}

#[test]
fn test_link_passthrough_url_empty_text() {
    // Empty link text → the link is "bare" (matches Asciidoctor).
    let html = to_html("link:++https://example.com/my page++[]");
    assert!(html.contains("<a href=\"https://example.com/my page\" class=\"bare\">https://example.com/my page</a>"));
}

#[test]
fn test_link_passthrough_url_with_attrs() {
    let html = to_html("link:++https://example.com/my page++[Click,window=_blank]");
    assert!(html.contains("<a href=\"https://example.com/my page\""));
    assert!(html.contains("target=\"_blank\""));
    assert!(html.contains(">Click</a>"));
}

#[test]
fn test_email_autolink_html() {
    // Email autolinks get no class="bare" (matches Asciidoctor — bare is only
    // for URL autolinks and link:/URL macros with empty text).
    let html = to_html("Contact user@example.com for info");
    assert!(html.contains("<a href=\"mailto:user@example.com\">user@example.com</a>"), "{html}");
    assert!(!html.contains("class=\"bare\""), "{html}");
}

#[test]
fn test_link_role_and_mailto_query_html() {
    // role= named attr → class on <a>; with empty text the bare class comes
    // first ("bare green"), matching Asciidoctor.
    let html = to_html("https://x.org[text,role=green]");
    assert!(html.contains("<a href=\"https://x.org\" class=\"green\">text</a>"), "{html}");
    let html = to_html("https://x.org[role=green]");
    assert!(html.contains("<a href=\"https://x.org\" class=\"bare green\">https://x.org</a>"), "{html}");
    let html = to_html("https://x.org[*chat*^,role=green]");
    assert!(
        html.contains("<a href=\"https://x.org\" class=\"green\" target=\"_blank\" rel=\"noopener\"><strong>chat</strong></a>"),
        "{html}"
    );

    // mailto positional attrs 2/3 → percent-encoded subject/body query.
    let html = to_html("mailto:join@discuss.example.org[Subscribe,Subscribe me,I want to join!]");
    assert!(
        html.contains("<a href=\"mailto:join@discuss.example.org?subject=Subscribe%20me&amp;body=I%20want%20to%20join%21\">Subscribe</a>"),
        "{html}"
    );

    // irc:// and ftp:// are autolink schemes (bare), like http(s).
    let html = to_html("Chat in the irc://irc.freenode.org/#fedora[Fedora IRC channel].");
    assert!(html.contains("<a href=\"irc://irc.freenode.org/#fedora\">Fedora IRC channel</a>"), "{html}");
    let html = to_html("Get ftp://ftp.example.org/pub/file now");
    assert!(html.contains("<a href=\"ftp://ftp.example.org/pub/file\" class=\"bare\">ftp://ftp.example.org/pub/file</a>"), "{html}");
}

#[test]
fn test_thematic_break() {
    let html = to_html("Before.\n\n'''\n\nAfter.");
    assert!(html.contains("<hr>"));
}

#[test]
fn test_html_escaping() {
    let html = to_html("Use <b> & \"quotes\".");
    assert!(html.contains("&lt;b&gt;"));
    assert!(html.contains("&amp;"));
    assert!(html.contains("\"quotes\""));
    assert!(!html.contains("&quot;"));
}

#[test]
fn test_superscript() {
    let html = to_html("E=mc^2^");
    assert!(html.contains("<sup>2</sup>"));
}

#[test]
fn test_subscript() {
    let html = to_html("H~2~O");
    assert!(html.contains("<sub>2</sub>"));
}

#[test]
fn test_document_header() {
    // In embedded mode, document header (h1) is not rendered
    let html = to_html("= My Document\n\nContent.");
    assert!(!html.contains("<h1>"), "embedded mode should not render document header h1. Got:\n{html}");
    // In standalone mode, the header is rendered
    let html = to_html_with_options("= My Document\n\nContent.", HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("<h1>My Document</h1>"),
        "expected <h1>My Document</h1> in standalone mode, got:\n{html}");
}

#[test]
fn test_stem_mathjax_docinfo() {
    // When `:stem:` is set, the MathJax loader is injected before </body>.
    let html = to_html_with_options(
        "= Doc\n:stem: asciimath\n\nHello.",
        HtmlOptions { standalone: true, ..Default::default() },
    );
    assert!(html.contains("<script type=\"text/x-mathjax-config\">"),
        "stem doc should inject MathJax config. Got:\n{html}");
    assert!(html.contains("cdnjs.cloudflare.com/ajax/libs/mathjax/2.7.9/MathJax.js?config=TeX-MML-AM_HTMLorMML"),
        "stem doc should inject MathJax loader. Got:\n{html}");
    // Block sits before </body>, after content.
    let body_pos = html.find("</body>").unwrap();
    let mathjax_pos = html.find("x-mathjax-config").unwrap();
    assert!(mathjax_pos < body_pos, "MathJax must precede </body>");

    // latexmath produces the identical (notation-agnostic) block.
    let html_tex = to_html_with_options(
        "= Doc\n:stem: latexmath\n\nHello.",
        HtmlOptions { standalone: true, ..Default::default() },
    );
    assert!(html_tex.contains("<script type=\"text/x-mathjax-config\">"),
        "latexmath stem doc should also inject MathJax. Got:\n{html_tex}");

    // No `:stem:` → no injection.
    let html_none = to_html_with_options(
        "= Doc\n\nHello.",
        HtmlOptions { standalone: true, ..Default::default() },
    );
    assert!(!html_none.contains("x-mathjax-config"),
        "doc without stem must not inject MathJax. Got:\n{html_none}");
}

#[test]
fn test_description_list_html() {
    let html = to_html("CPU:: The brain\nRAM:: Memory");
    assert_eq!(
        html,
        "<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">CPU</dt>\n<dd>\n<p>The brain</p>\n</dd>\n<dt class=\"hdlist1\">RAM</dt>\n<dd>\n<p>Memory</p>\n</dd>\n</dl>\n</div>\n"
    );
}

#[test]
fn test_nested_description_list_html() {
    let html = to_html("CPU:: The brain\nSpeed::: Fast");
    assert_eq!(
        html,
        "<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">CPU</dt>\n<dd>\n<p>The brain</p>\n<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">Speed</dt>\n<dd>\n<p>Fast</p>\n</dd>\n</dl>\n</div>\n</dd>\n</dl>\n</div>\n"
    );
}

#[test]
fn test_list_continuation_html() {
    let html = to_html("* item\n+\nContinued.");
    assert!(html.contains("<p>item</p>\n<div class=\"paragraph\">\n<p>Continued.</p>\n</div>"), "continuation should be wrapped in div.paragraph:\n{html}");
}

#[test]
fn test_description_list_continuation_html() {
    let html = to_html("Term:: desc\n+\nMore.");
    assert!(html.contains("<p>desc</p>\n<div class=\"paragraph\">\n<p>More.</p>\n</div>"), "dlist continuation should be wrapped in div.paragraph:\n{html}");
}

#[test]
fn test_inline_passthrough_html() {
    let html = to_html("hello +++<b>bold</b>+++ world");
    assert!(html.contains("hello <b>bold</b> world"));
}

#[test]
fn test_table_html() {
    let html = to_html("|===\n| A | B\n| C | D\n|===");
    assert!(html.contains("<table class=\"tableblock frame-all grid-all stretch\">"), "expected table classes. Got:\n{html}");
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<tr>"));
    assert!(html.contains("<p class=\"tableblock\">A</p>"));
    assert!(html.contains("<p class=\"tableblock\">B</p>"));
    assert!(html.contains("<p class=\"tableblock\">C</p>"));
    assert!(html.contains("<p class=\"tableblock\">D</p>"));
    assert!(html.contains("</tbody>"));
    assert!(html.contains("</table>"));
    assert!(!html.contains("<thead>"));
}

#[test]
fn test_table_noheader_option_html() {
    // %noheader suppresses the implicit first-row promotion
    let html = to_html("[%noheader]\n|===\n|A |B\n\n|1 |2\n|===");
    assert!(!html.contains("<thead>"));
    assert!(!html.contains("<th class="));

    // formal syntax: options=noheader
    let html = to_html("[options=noheader]\n|===\n|A |B\n\n|1 |2\n|===");
    assert!(!html.contains("<thead>"));

    // explicit header wins over noheader
    let html = to_html("[%header%noheader]\n|===\n|A |B\n\n|1 |2\n|===");
    assert!(html.contains("<thead>"));

    // formal options=header works without the implicit blank-line layout
    let html = to_html("[options=header]\n|===\n|A |B\n|1 |2\n|===");
    assert!(html.contains("<thead>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">A</th>"));

    // opts= is an alias for options=
    let html = to_html("[opts=header]\n|===\n|A |B\n|1 |2\n|===");
    assert!(html.contains("<thead>"));
}

#[test]
fn test_table_with_header_html() {
    let html = to_html("|===\n| Header 1 | Header 2\n\n| Cell 1 | Cell 2\n| Cell 3 | Cell 4\n|===");
    assert!(html.contains("<thead>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Header 1</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Header 2</th>"));
    assert!(html.contains("</thead>"));
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<p class=\"tableblock\">Cell 1</p>"));
    assert!(html.contains("<p class=\"tableblock\">Cell 2</p>"));
    assert!(html.contains("<p class=\"tableblock\">Cell 3</p>"));
    assert!(html.contains("<p class=\"tableblock\">Cell 4</p>"));
    assert!(html.contains("</tbody>"));
    assert!(html.contains("</table>"));
}

#[test]
fn test_table_with_cols_html() {
    let html = to_html("[cols=\"2\"]\n|===\n| A\n| B\n| C\n| D\n|===");
    assert!(html.contains("<table class=\"tableblock frame-all grid-all stretch\">"));
    assert!(html.contains("<tbody>"));
    // Should have 2 rows of 2 cells
    let td_count = html.matches("<td class=\"tableblock").count();
    assert_eq!(td_count, 4);
    let tr_count = html.matches("<tr>").count();
    assert_eq!(tr_count, 2);
    assert!(html.contains("</tbody>"));
    assert!(html.contains("</table>"));
}

#[test]
fn test_table_footer_html() {
    let html = to_html("[%footer]\n|===\n| A | B\n| F1 | F2\n|===");
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<p class=\"tableblock\">A</p>"));
    assert!(html.contains("<p class=\"tableblock\">B</p>"));
    assert!(html.contains("</tbody>"));
    assert!(html.contains("<tfoot>"));
    assert!(html.contains("<p class=\"tableblock\">F1</p>"));
    assert!(html.contains("<p class=\"tableblock\">F2</p>"));
    assert!(html.contains("</tfoot>"));
    assert!(!html.contains("<thead>"));
}

#[test]
fn test_footnote_html() {
    let html = to_html("Hello footnote:[This is a note] world.");
    assert!(html.contains("<sup class=\"footnote\">[<a class=\"footnote\" id=\"_footnoteref_1\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>"));
    assert!(html.contains("<div id=\"footnotes\">"));
    assert!(html.contains("<hr>"));
    assert!(html.contains("<div class=\"footnote\" id=\"_footnotedef_1\">"));
    assert!(html.contains("<a href=\"#_footnoteref_1\">1</a>. This is a note"));
}

#[test]
fn test_footnote_named_html() {
    let html = to_html("First footnote:fn1[Named note] and again footnote:fn1[].");
    // Definition
    assert!(html.contains("<sup class=\"footnote\" id=\"_footnote_fn1\">[<a class=\"footnote\" id=\"_footnoteref_1\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>"));
    // Reference should use the same number
    let refs: Vec<_> = html.match_indices("_footnoteref_1").collect();
    assert!(refs.len() >= 2, "Expected at least 2 references to footnote 1, got {}", refs.len());
}

#[test]
fn test_footnote_multiple_html() {
    let html = to_html("A footnote:[First] B footnote:[Second] C footnote:[Third].");
    assert!(html.contains("_footnoteref_1"));
    assert!(html.contains("_footnoteref_2"));
    assert!(html.contains("_footnoteref_3"));
    assert!(html.contains("_footnotedef_1"));
    assert!(html.contains("_footnotedef_2"));
    assert!(html.contains("_footnotedef_3"));
    assert!(html.contains(">1</a>. First"));
    assert!(html.contains(">2</a>. Second"));
    assert!(html.contains(">3</a>. Third"));
}

#[test]
fn test_toc_html() {
    let input = "= Document Title\n:toc:\n\n== Section One\n\nContent.\n\n== Section Two\n\nMore content.";
    let html = to_html(input);
    assert!(html.contains("<div id=\"toc\" class=\"toc\">"));
    assert!(html.contains("<div id=\"toctitle\">Table of Contents</div>"));
    assert!(html.contains("<a href=\"#_section_one\">Section One</a>"));
    assert!(html.contains("<a href=\"#_section_two\">Section Two</a>"));
    assert!(html.contains("</ul>"));
    assert!(html.contains("</div>"));
}

#[test]
fn test_toc_levels() {
    let input = "= Document Title\n:toc:\n:toclevels: 3\n\n== Level 2\n\n=== Level 3\n\n==== Level 4\n\n===== Level 5";
    let html = to_html(input);
    assert!(html.contains("<a href=\"#_level_2\">Level 2</a>"));
    assert!(html.contains("<a href=\"#_level_3\">Level 3</a>"));
    assert!(html.contains("<a href=\"#_level_4\">Level 4</a>"));
    // Level 5 should NOT be in TOC (toclevels: 3 → levels 2..4)
    assert!(!html.contains("<a href=\"#_level_5\">Level 5</a>"));
}

#[test]
fn test_toc_default_levels() {
    let input = "= Document Title\n:toc:\n\n== Level 2\n\n=== Level 3\n\n==== Level 4";
    let html = to_html(input);
    assert!(html.contains("<a href=\"#_level_2\">Level 2</a>"));
    assert!(html.contains("<a href=\"#_level_3\">Level 3</a>"));
    // Default toclevels: 2 → levels 2..3, so level 4 should NOT be in TOC
    assert!(!html.contains("<a href=\"#_level_4\">Level 4</a>"));
}

#[test]
fn test_toc_macro_html() {
    let input = "= Document Title\n\n== Before\n\ntoc::[]\n\n== After";
    let html = to_html(input);
    assert!(html.contains("<div id=\"toc\" class=\"toc\">"));
    // TOC should be placed where toc::[] macro is (after "Before" section start)
    let toc_pos = html.find("<div id=\"toc\"").unwrap();
    let before_pos = html.find("Before</h2>").unwrap();
    assert!(toc_pos > before_pos, "TOC should appear after the Before heading");
}

#[test]
fn test_unresolved_include_html() {
    let html = to_html("include::chapter.adoc[]");
    assert_eq!(html, "<!-- include::chapter.adoc[] -->\n");
}

#[test]
fn test_unresolved_include_with_special_chars_html() {
    let html = to_html("include::path/to/<file>.adoc[]");
    assert_eq!(html, "<!-- include::path/to/&lt;file&gt;.adoc[] -->\n");
}

#[test]
fn test_no_toc_without_attribute() {
    let input = "= Document Title\n\n== Section\n\nContent.";
    let html = to_html(input);
    assert!(!html.contains("<div id=\"toc\""));
}

#[test]
fn test_toc_custom_title() {
    let input = "= Doc\n:toc:\n:toc-title: Содержание\n\n== S1\n\nText.";
    let html = to_html(input);
    assert!(html.contains("<div id=\"toctitle\">Содержание</div>"));
    assert!(!html.contains("Table of Contents"));
}

#[test]
fn test_toc_left() {
    let input = "= Doc\n:toc: left\n\n== S1\n\nText.";
    let html = to_html_with_options(input, HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("<body class=\"article toc2 toc-left\">"));
    assert!(html.contains("<div id=\"toc\" class=\"toc2\">"));
}

#[test]
fn test_toc_right() {
    let input = "= Doc\n:toc: right\n\n== S1\n\nText.";
    let html = to_html_with_options(input, HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("<body class=\"article toc2 toc-right\">"));
    assert!(html.contains("<div id=\"toc\" class=\"toc2\">"));
}

#[test]
fn test_toc_mid_document_no_body_class() {
    // `:toc:` set after the header has no effect: no TOC and no toc2 body class
    // (Asciidoctor normalizes toc placement from header attributes only).
    let input = "= Doc\n\npara\n\n:toc: left\n\n== S1\n\nText.";
    let html = to_html_with_options(input, HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("<body class=\"article\">"));
    assert!(!html.contains("<div id=\"toc\""));
}

#[test]
fn test_toc_preamble() {
    let input = "= Title\n:toc: preamble\n\nPreamble text.\n\n== Section One\n\nContent.";
    let html = to_html(input);
    assert!(html.contains("<div id=\"toc\""), "should contain TOC. Got:\n{html}");
    let toc_pos = html.find("<div id=\"toc\"").unwrap();
    let section_pos = html.find("<div class=\"sect1\"").unwrap();
    assert!(toc_pos < section_pos, "TOC should be before first section");
}

#[test]
fn test_toc_macro_only() {
    let input = "= Title\n:toc: macro\n\n== S1\n\ntoc::[]\n\n== S2";
    let html = to_html(input);
    assert!(html.contains("<div id=\"toc\""));
    // TOC should be placed at the macro position, after S1 heading
    let s1_pos = html.find("S1</h2>").unwrap();
    let toc_pos = html.find("<div id=\"toc\"").unwrap();
    assert!(toc_pos > s1_pos, "TOC should appear after S1 heading");
}

#[test]
fn test_source_block_callouts_html() {
    let input = "[source,ruby]\n----\nrequire 'sinatra' <1>\nget '/hi' do <2>\n  \"Hello World!\" <3>\nend\n----\n<1> Library import\n<2> URL mapping\n<3> Response";
    let html = to_html(input);
    assert!(html.contains("<b class=\"conum\">(1)</b>"));
    assert!(html.contains("<b class=\"conum\">(2)</b>"));
    assert!(html.contains("<b class=\"conum\">(3)</b>"));
    assert!(html.contains("<div class=\"colist arabic\">"));
    assert!(html.contains("<li><p>Library import</p></li>"));
    assert!(html.contains("<li><p>URL mapping</p></li>"));
    assert!(html.contains("<li><p>Response</p></li>"));
}

#[test]
fn test_callout_multiple_per_line_html() {
    let input = "[source]\n----\ncode <1> <2>\n----\n<1> First\n<2> Second";
    let html = to_html(input);
    assert!(html.contains("<b class=\"conum\">(1)</b> <b class=\"conum\">(2)</b>"));
    assert!(html.contains("<li><p>First</p></li>"));
    assert!(html.contains("<li><p>Second</p></li>"));
}

#[test]
fn test_callout_item_with_continuation_note_html() {
    // A NOTE attached to a callout item with `+` is a child block: the
    // item's principal `<p>` must close before the admonition, not nest it.
    let input = "[source]\n----\ncode <1>\nmore <2>\n----\n<1> simple\n<2> has note\n+\nNOTE: the note";
    let html = to_html(input);
    // Simple item unchanged.
    assert!(html.contains("<li><p>simple</p></li>"));
    // Item 2: <p> closed before the admonition (not nested inside it).
    assert!(html.contains("<li><p>has note</p>\n<div class=\"admonitionblock note\">"));
    // The item closes after the admonition; no stray </p> nesting the block.
    assert!(html.contains("</table>\n</div>\n</li>"));
    assert!(!html.contains("has note<div"));
    assert!(!html.contains("</div></p></li>"));
}

#[test]
fn test_source_lang_shifted_by_leading_named_attr_html() {
    // `[id=app, source, yaml]` — the leading `id=` shifts positionals, so
    // `source` is the language (slot 2), not `yaml` (slot 3).
    let html = to_html("[id=app, source, yaml]\n----\nspring:\n  x: 1\n----");
    assert!(html.contains("class=\"language-source\" data-lang=\"source\""));
    assert!(!html.contains("language-yaml"));
    // Explicit `[source, yaml]` is unaffected.
    let html = to_html("[source, yaml]\n----\na: 1\n----");
    assert!(html.contains("class=\"language-yaml\" data-lang=\"yaml\""));
}

#[test]
fn test_checklist_html() {
    let html = to_html("* [x] Done\n* [ ] Todo");
    assert!(html.contains("<div class=\"ulist checklist\">\n<ul class=\"checklist\">"));
    assert!(html.contains("<li>\n<p>&#10003; Done</p>\n</li>"));
    assert!(html.contains("<li>\n<p>&#10063; Todo</p>\n</li>"));
    assert!(html.contains("</ul>\n</div>"));
}

#[test]
fn test_checklist_mixed_html() {
    let html = to_html("* [x] Checked\n* Regular\n* [ ] Unchecked");
    assert!(html.contains("<div class=\"ulist checklist\">\n<ul class=\"checklist\">"));
    assert!(html.contains("<li>\n<p>&#10003; Checked</p>\n</li>"));
    assert!(html.contains("<li>\n<p>Regular</p>\n</li>"));
    assert!(html.contains("<li>\n<p>&#10063; Unchecked</p>\n</li>"));
}

#[test]
fn test_regular_list_no_checklist_class() {
    let html = to_html("* item 1\n* item 2");
    assert!(html.contains("<ul>"));
    assert!(!html.contains("checklist"));
}

#[test]
fn test_verse_block_html() {
    let html = to_html("[verse]\n____\nline one\nline two\n____");
    assert_eq!(
        html,
        "<div class=\"verseblock\">\n<pre class=\"content\">line one\nline two</pre>\n</div>\n"
    );
}

#[test]
fn test_verse_block_with_formatting_html() {
    let html = to_html("[verse]\n____\nhello *bold* world\nand _italic_ too\n____");
    assert!(html.contains("<div class=\"verseblock\">"));
    assert!(html.contains("<pre class=\"content\">"));
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<em>italic</em>"));
    assert!(html.contains("</pre>\n</div>\n"));
}

#[test]
fn test_table_colspan_html() {
    let html = to_html("|===\n| A 2+| B spans\n| C | D | E\n|===");
    assert!(html.contains("<p class=\"tableblock\">A</p>"));
    assert!(html.contains("colspan=\"2\"><p class=\"tableblock\">B spans</p>"));
    assert!(html.contains("<p class=\"tableblock\">C</p>"));
    assert!(html.contains("<p class=\"tableblock\">D</p>"));
    assert!(html.contains("<p class=\"tableblock\">E</p>"));
}

#[test]
fn test_table_rowspan_html() {
    let html = to_html("|===\n.2+| A | B\n| C\n|===");
    assert!(html.contains("rowspan=\"2\"><p class=\"tableblock\">A</p>"));
    assert!(html.contains("<p class=\"tableblock\">B</p>"));
    assert!(html.contains("<p class=\"tableblock\">C</p>"));
    // Should have 2 rows
    assert_eq!(html.matches("<tr>").count(), 2);
}

#[test]
fn test_table_colspan_rowspan_html() {
    let html = to_html("|===\n2.3+| cell | B\n| C\n| D\n|===");
    assert!(html.contains("colspan=\"2\" rowspan=\"3\"><p class=\"tableblock\">cell</p>"));
}

#[test]
fn test_table_rowspan_shifts_following_row_cells_html() {
    // A rowspan cell occupies its column in the rows it spans, so the next
    // row holds one FEWER cell. Regression: the spanned column must be
    // skipped exactly once (no double-decrement that lets the cell slip
    // back into the spanned column).
    let html = to_html(
        "[cols=\"1,1\"]\n|===\n|A |B\n\n.2+|X\n|1\n\n|2\n\n|Y |Z\n|===",
    );
    // The row continuing the rowspan ("2") must be a standalone <tr> with a
    // single cell, NOT merged with the following "Y".
    assert!(
        html.contains("<td class=\"tableblock halign-left valign-top\"><p class=\"tableblock\">2</p></td>\n</tr>"),
        "rowspan continuation cell '2' must close its own row. Got:\n{html}"
    );
    // "Y" starts a fresh row (preceded by <tr>, not by cell "2").
    assert!(
        html.contains("<tr>\n<td class=\"tableblock halign-left valign-top\"><p class=\"tableblock\">Y</p></td>"),
        "cell 'Y' must begin a new row. Got:\n{html}"
    );
    // Three body rows: [X,1], [2], [Y,Z] → 4 <tr> total incl. header.
    assert_eq!(html.matches("<tr>").count(), 4, "expected 4 rows. Got:\n{html}");
}

#[test]
fn test_table_cell_style_emphasis_html() {
    let html = to_html("|===\ne| italic\n|===");
    assert!(html.contains("<p class=\"tableblock\"><em>italic</em></p>"), "expected emphasis in tableblock p. Got:\n{html}");
}

#[test]
fn test_table_cell_style_strong_html() {
    let html = to_html("|===\ns| bold\n|===");
    assert!(html.contains("<p class=\"tableblock\"><strong>bold</strong></p>"), "expected strong in tableblock p. Got:\n{html}");
}

#[test]
fn test_table_cell_style_monospace_html() {
    let html = to_html("|===\nm| code\n|===");
    assert!(html.contains("<p class=\"tableblock\"><code>code</code></p>"), "expected code in tableblock p. Got:\n{html}");
}

#[test]
fn test_table_cell_style_literal_html() {
    let html = to_html("|===\nl| literal\n|===");
    assert!(html.contains("<p class=\"tableblock\"><code>literal</code></p>"), "expected code in tableblock p. Got:\n{html}");
}

#[test]
fn test_table_cell_style_header_in_body_html() {
    // A header-style cell (`h|`) in a body row renders as <th> but, unlike a
    // header-ROW cell, KEEPS the <p class="tableblock"> wrapper (Asciidoctor parity).
    let html = to_html("|===\nh| header cell\n|===");
    assert!(html.contains("<tbody>"), "h-cell stays in body, not thead. Got:\n{html}");
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\"><p class=\"tableblock\">header cell</p></th>"), "expected th with wrapped tableblock p. Got:\n{html}");
}

#[test]
fn test_table_header_column_style_html() {
    // The `h` column style (`[cols="1h,1"]`) makes that column's body cells
    // render as <th> (with the <p> wrapper); other columns stay <td>.
    let html = to_html("[cols=\"1h,1\"]\n|===\n|key |value\n|===");
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\"><p class=\"tableblock\">key</p></th>"), "expected h-column cell as wrapped th. Got:\n{html}");
    assert!(html.contains("<td class=\"tableblock halign-left valign-top\"><p class=\"tableblock\">value</p></td>"), "expected non-h column cell as td. Got:\n{html}");
}

#[test]
fn test_table_cell_style_with_colspan_html() {
    let html = to_html("|===\n2+e| wide italic | B\n| C | D\n|===");
    assert!(html.contains("colspan=\"2\"><p class=\"tableblock\"><em>wide italic</em></p>"), "expected colspan with emphasis in tableblock p. Got:\n{html}");
}

#[test]
fn test_table_cell_style_no_false_positive_html() {
    // "data" ends with 'a' but should NOT be treated as AsciiDoc style
    let html = to_html("|===\n| data | more\n|===");
    assert!(html.contains("<p class=\"tableblock\">data</p>"));
    assert!(html.contains("<p class=\"tableblock\">more</p>"));
}

#[test]
fn test_table_cols_alignment_html() {
    let html = to_html("[cols=\"<,^,>\"]\n|===\n| A | B | C\n|===");
    assert!(html.contains("halign-left"), "Left-aligned should have halign-left class");
    assert!(html.contains("halign-center"), "Center should have halign-center class");
    assert!(html.contains("halign-right"), "Right should have halign-right class");
    assert!(html.contains("<p class=\"tableblock\">A</p>"));
    assert!(html.contains("<p class=\"tableblock\">B</p>"));
    assert!(html.contains("<p class=\"tableblock\">C</p>"));
}

#[test]
fn test_table_cell_align_html() {
    let html = to_html("|===\n^| centered\n|===");
    assert!(html.contains("halign-center"), "expected halign-center class. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">centered</p>"));
}

#[test]
fn test_table_cell_combined_align_html() {
    let html = to_html("|===\n>.^| text\n|===");
    assert!(html.contains("halign-right valign-middle"), "expected halign-right valign-middle. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">text</p>"));
}

#[test]
fn test_table_cell_override_cols_align_html() {
    // cols says left, cell overrides to center
    let html = to_html("[cols=\"<,<\"]\n|===\n^| centered | normal\n|===");
    assert!(html.contains("halign-center"), "cell should override to center. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">centered</p>"));
    assert!(html.contains("<p class=\"tableblock\">normal</p>"));
}

#[test]
fn test_table_valign_only_html() {
    let html = to_html("|===\n.>| bottom\n|===");
    assert!(html.contains("valign-bottom"), "expected valign-bottom class. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">bottom</p>"));
}

#[test]
fn test_table_cols_valign_html() {
    let html = to_html("[cols=\".^,1\"]\n|===\n| A | B\n|===");
    assert!(html.contains("valign-middle"), "expected valign-middle class. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">A</p>"));
    assert!(html.contains("<p class=\"tableblock\">B</p>"));
}

#[test]
fn test_kbd_single_key_html() {
    let html = to_html(":experimental:\n\nkbd:[F11]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><kbd>F11</kbd></p>\n</div>\n");
}

#[test]
fn test_ui_macros_literal_without_experimental_html() {
    // Without :experimental:, kbd:/btn:/menu: render as literal text.
    assert_eq!(
        to_html("kbd:[F11]"),
        "<div class=\"paragraph\">\n<p>kbd:[F11]</p>\n</div>\n"
    );
    assert_eq!(
        to_html("btn:[OK]"),
        "<div class=\"paragraph\">\n<p>btn:[OK]</p>\n</div>\n"
    );
    assert_eq!(
        to_html("menu:File[Save]"),
        "<div class=\"paragraph\">\n<p>menu:File[Save]</p>\n</div>\n"
    );
}

#[test]
fn test_kbd_combo_html() {
    let html = to_html(":experimental:\n\nkbd:[Ctrl+C]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"keyseq\"><kbd>Ctrl</kbd>+<kbd>C</kbd></span></p>\n</div>\n");
}

#[test]
fn test_btn_html() {
    let html = to_html(":experimental:\n\nbtn:[OK]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><b class=\"button\">OK</b></p>\n</div>\n");
}

#[test]
fn test_menu_html() {
    let html = to_html(":experimental:\n\nmenu:File[Save As]");
    assert_eq!(
        html,
        "<div class=\"paragraph\">\n<p><span class=\"menuseq\"><b class=\"menu\">File</b>&#160;<b class=\"caret\">&#8250;</b> <b class=\"menuitem\">Save As</b></span></p>\n</div>\n"
    );
}

#[test]
fn test_menu_no_items_html() {
    let html = to_html(":experimental:\n\nmenu:File[]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"menu\">File</span></p>\n</div>\n");
}

#[test]
fn test_icon_basic_html() {
    let html = to_html("icon:heart[]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-heart\"></i></span></p>\n</div>\n");
}

#[test]
fn test_icon_size_html() {
    let html = to_html("icon:heart[2x]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-heart fa-2x\"></i></span></p>\n</div>\n");
}

#[test]
fn test_icon_role_html() {
    let html = to_html("icon:tags[role=blue]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-tags blue\"></i></span></p>\n</div>\n");
}

#[test]
fn test_icon_title_html() {
    let html = to_html("icon:info[title=Info]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-info\" title=\"Info\"></i></span></p>\n</div>\n");
}

#[test]
fn test_icon_rotate_html() {
    let html = to_html("icon:shield[rotate=90]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-shield fa-rotate-90\"></i></span></p>\n</div>\n");
}

#[test]
fn test_icon_flip_html() {
    let html = to_html("icon:shield[flip=vertical]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-shield fa-flip-vertical\"></i></span></p>\n</div>\n");
}

#[test]
fn test_icon_link_html() {
    let html = to_html("icon:download[link=https://example.com]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><a class=\"icon\" href=\"https://example.com\"><i class=\"fa fa-download\"></i></a></p>\n</div>\n");
}

#[test]
fn test_icon_combined_html() {
    let html = to_html("icon:heart[2x,role=red]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"icon\"><i class=\"fa fa-heart fa-2x red\"></i></span></p>\n</div>\n");
}

#[test]
fn test_menu_submenus_html() {
    let html = to_html(":experimental:\n\nmenu:File[New > Doc]");
    assert_eq!(
        html,
        "<div class=\"paragraph\">\n<p><span class=\"menuseq\"><b class=\"menu\">File</b>&#160;<b class=\"caret\">&#8250;</b> <b class=\"submenu\">New</b>&#160;<b class=\"caret\">&#8250;</b> <b class=\"menuitem\">Doc</b></span></p>\n</div>\n"
    );
}

// Stem macro tests

#[test]
fn test_stem_inline_html() {
    let html = to_html("stem:[x^2]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>\\$x^2\\$</p>\n</div>\n");
}

#[test]
fn test_latexmath_inline_html() {
    let html = to_html("latexmath:[C = \\alpha]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>\\(C = \\alpha\\)</p>\n</div>\n");
}

#[test]
fn test_asciimath_inline_html() {
    let html = to_html("asciimath:[sqrt(4)]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>\\$sqrt(4)\\$</p>\n</div>\n");
}

#[test]
fn test_stem_no_escape_html() {
    let html = to_html("stem:[a < b]");
    assert!(html.contains("a < b"), "stem content should not be HTML-escaped");
    assert!(!html.contains("&lt;"), "stem content must not contain &lt;");
}

#[test]
fn test_stem_block_html() {
    let html = to_html("[stem]\n++++\nx^2\n++++");
    assert!(html.contains("<div class=\"stemblock\">"));
    assert!(html.contains("<div class=\"content\">"));
    assert!(html.contains("\\$x^2\\$"));
    assert!(html.contains("</div>\n</div>\n"));
}

#[test]
fn test_latexmath_block_html() {
    let html = to_html("[latexmath]\n++++\nx^2\n++++");
    assert!(html.contains("<div class=\"stemblock\">"));
    assert!(html.contains("\\[x^2\\]"));
}

#[test]
fn test_video_basic_html() {
    let html = to_html("video::video.mp4[]");
    assert_eq!(
        html,
        "<div class=\"videoblock\">\n<div class=\"content\">\n<video src=\"video.mp4\" controls>\nYour browser does not support the video tag.\n</video>\n</div>\n</div>\n"
    );
}

#[test]
fn test_video_and_stem_block_title() {
    // `.Title` before a video renders inside the videoblock, before the
    // content div (mirrors audio); it must not leak into the next block.
    let html = to_html(".VideoTitle\nvideo::cast.mp4[]\n\nafter\n");
    assert!(html.contains(
        "<div class=\"videoblock\">\n<div class=\"title\">VideoTitle</div>\n<div class=\"content\">"
    ));
    assert!(html.contains("<div class=\"paragraph\">\n<p>after</p>"));
    assert!(!html.contains("<div class=\"paragraph\">\n<div class=\"title\">"));

    // Same rule for a stem block.
    let html = to_html(":stem:\n\n.StemTitle\n[stem]\n++++\nx^2\n++++\n\nafter\n");
    assert!(html.contains(
        "<div class=\"stemblock\">\n<div class=\"title\">StemTitle</div>\n<div class=\"content\">"
    ));
    assert!(!html.contains("<div class=\"paragraph\">\n<div class=\"title\">"));
}

#[test]
fn test_video_attrs_html() {
    let html = to_html("video::video.mp4[width=640,height=480,poster=preview.jpg]");
    assert!(html.contains("<video src=\"video.mp4\" width=\"640\" height=\"480\" poster=\"preview.jpg\" controls>"));
}

#[test]
fn test_video_options_html() {
    let html = to_html("video::video.mp4[options=\"autoplay,loop,nocontrols\"]");
    assert!(html.contains("<video src=\"video.mp4\" autoplay loop>"));
    assert!(!html.contains("controls"));
}

#[test]
fn test_video_youtube_playlist_params() {
    // `list=` attribute and `video_id/list_id` target are equivalent.
    let html = to_html("video::RvRhUHTV_8k[youtube,list=PLDitloy]");
    assert!(html.contains("src=\"https://www.youtube.com/embed/RvRhUHTV_8k?rel=0&amp;list=PLDitloy\""), "{html}");
    let html = to_html("video::RvRhUHTV_8k/PLDitloy[youtube]");
    assert!(html.contains("src=\"https://www.youtube.com/embed/RvRhUHTV_8k?rel=0&amp;list=PLDitloy\""), "{html}");
    // `playlist=` attribute and comma-separated target both emit `&playlist=`
    // with the video id prepended.
    let html = to_html("video::RvRhUHTV_8k[youtube,playlist=\"_Svw,SGqg\"]");
    assert!(html.contains("src=\"https://www.youtube.com/embed/RvRhUHTV_8k?rel=0&amp;playlist=RvRhUHTV_8k,_Svw,SGqg\""), "{html}");
    let html = to_html("video::RvRhUHTV_8k,_Svw,SGqg[youtube]");
    assert!(html.contains("src=\"https://www.youtube.com/embed/RvRhUHTV_8k?rel=0&amp;playlist=RvRhUHTV_8k,_Svw,SGqg\""), "{html}");
    // A bare `loop` needs a playlist for YouTube to loop: the video id is used.
    let html = to_html("video::RvRhUHTV_8k[youtube,opts=loop]");
    assert!(html.contains("src=\"https://www.youtube.com/embed/RvRhUHTV_8k?rel=0&amp;loop=1&amp;playlist=RvRhUHTV_8k\""), "{html}");
}

#[test]
fn test_video_start_end_html() {
    let html = to_html("video::video.mp4[start=60,end=120]");
    assert!(html.contains("src=\"video.mp4#t=60,120\""));
}

#[test]
fn test_video_start_only_html() {
    let html = to_html("video::video.mp4[start=30]");
    assert!(html.contains("src=\"video.mp4#t=30\""));
}

#[test]
fn test_video_width_attr_escaped() {
    // Regression (D1): media attribute values must be HTML-escaped so a quote
    // inside the value cannot break out of the attribute and inject markup.
    let html = to_html("video::v.mp4[width=1\" onmouseover=\"alert(1)]");
    assert!(
        !html.contains("onmouseover=\"alert"),
        "attribute breakout not prevented: {html}"
    );
    assert!(html.contains("&quot;"), "value was not escaped: {html}");
}

#[test]
fn test_audio_basic_html() {
    let html = to_html("audio::audio.mp3[]");
    assert_eq!(
        html,
        "<div class=\"audioblock\">\n<div class=\"content\">\n<audio src=\"audio.mp3\" controls>\nYour browser does not support the audio tag.\n</audio>\n</div>\n</div>\n"
    );
}

#[test]
fn test_audio_options_html() {
    let html = to_html("audio::audio.mp3[options=\"autoplay,loop\"]");
    // Asciidoctor order: autoplay, loop, controls.
    assert!(html.contains("<audio src=\"audio.mp3\" autoplay loop controls>"));
}

#[test]
fn test_audio_nocontrols_html() {
    let html = to_html("audio::audio.mp3[options=\"nocontrols\"]");
    assert!(html.contains("<audio src=\"audio.mp3\">"));
    assert!(!html.contains("controls"));
}

#[test]
fn test_audio_start_opts_and_title() {
    // `start`/`end` add a #t= media fragment; `opts` is the shorthand for
    // `options`; a `.Title` renders before the content div.
    let html = to_html(".Take a zen moment\naudio::ocean-waves.wav[start=60,opts=autoplay]");
    assert!(
        html.contains(
            "<div class=\"audioblock\">\n<div class=\"title\">Take a zen moment</div>\n<div class=\"content\">\n<audio src=\"ocean-waves.wav#t=60\" autoplay controls>"
        ),
        "audio start/opts/title rendering wrong: {html}"
    );
}

// Index term tests

#[test]
fn test_flow_index_term_html() {
    let html = to_html("I love ((tigers)) very much");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>I love tigers very much</p>\n</div>\n");
}

#[test]
fn test_concealed_index_term_html() {
    let html = to_html("(((animals, cats)))Visible text");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>Visible text</p>\n</div>\n");
}

#[test]
fn test_indexterm2_macro_html() {
    let html = to_html("indexterm2:[tigers]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>tigers</p>\n</div>\n");
}

#[test]
fn test_indexterm_macro_html() {
    let html = to_html("indexterm:[animals, cats]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p></p>\n</div>\n");
}

#[test]
fn test_flow_index_term_escaping_html() {
    let html = to_html("((a <b> & c))");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>a &lt;b&gt; &amp; c</p>\n</div>\n");
}

// Block metadata: custom id/class tests

#[test]
fn test_paragraph_with_id_and_role() {
    let html = to_html("[#notice.important]\nText");
    assert!(html.contains("id=\"notice\""), "should have id on div. Got: {html}");
    assert!(html.contains("class=\"paragraph important\""), "should have class on div. Got: {html}");
    assert!(html.contains("<p>Text</p>"), "p should be plain. Got: {html}");
}

#[test]
fn test_paragraph_with_id_only() {
    let html = to_html("[#myid]\nHello");
    assert!(html.contains("<div id=\"myid\" class=\"paragraph\">"), "id on div. Got: {html}");
    assert!(html.contains("<p>Hello</p>"), "p should be plain. Got: {html}");
}

#[test]
fn test_paragraph_with_role_only() {
    let html = to_html("[.lead]\nText");
    assert!(html.contains("class=\"paragraph lead\""), "role on div. Got: {html}");
    assert!(html.contains("<p>Text</p>"), "p should be plain. Got: {html}");
}

#[test]
fn test_paragraph_with_multiple_roles() {
    let html = to_html("[.r1.r2.r3]\nText");
    assert!(html.contains("class=\"paragraph r1 r2 r3\""), "roles on div. Got: {html}");
}

#[test]
fn test_sidebar_with_id_and_role() {
    let html = to_html("[#tips.custom]\n****\nContent\n****");
    assert!(html.contains("id=\"tips\""));
    assert!(html.contains("class=\"sidebarblock custom\""));
}

#[test]
fn test_source_block_with_id() {
    let html = to_html("[source,rust,#code1]\n----\nfn main() {}\n----");
    assert!(html.contains("id=\"code1\""));
}

#[test]
fn test_admonition_with_id_and_role() {
    let html = to_html("[#w1.special]\nWARNING: Danger!");
    assert!(html.contains("id=\"w1\""));
    assert!(html.contains("admonitionblock warning special"));
}

#[test]
fn test_admonition_custom_caption() {
    // A block-level caption overrides the label text but not the type class.
    let html = to_html("[caption=\"Work in Progress\"]\nCAUTION: hi.");
    assert!(html.contains("admonitionblock caution"), "type class kept. Got:\n{html}");
    assert!(
        html.contains("<div class=\"title\">Work in Progress</div>"),
        "caption overrides label. Got:\n{html}"
    );
    assert!(!html.contains(">Caution<"), "default label suppressed. Got:\n{html}");
    // Empty caption renders an empty title (matches Asciidoctor).
    let empty = to_html("[caption=]\nNOTE: hi.");
    assert!(empty.contains("<div class=\"title\"></div>"), "empty caption. Got:\n{empty}");
    // Caption values are HTML-escaped (escaping discipline; stricter than Asciidoctor).
    let esc = to_html("[caption=\"A & B\"]\nTIP: hi.");
    assert!(esc.contains("A &amp; B"), "caption escaped. Got:\n{esc}");
}

#[test]
fn test_list_with_id() {
    let html = to_html("[#mylist]\n* item 1\n* item 2");
    assert!(html.contains("<div id=\"mylist\" class=\"ulist\">"));
}

#[test]
fn test_table_with_id_and_role() {
    let html = to_html("[#data.striped]\n|===\n| A | B\n|===");
    assert!(html.contains("id=\"data\""), "expected id=\"data\". Got:\n{html}");
    assert!(html.contains("striped"), "expected striped in class. Got:\n{html}");
}

#[test]
fn test_table_autowidth_html() {
    let html = to_html("[%autowidth]\n|===\n| A | B\n|===");
    assert!(html.contains("fit-content"), "expected fit-content class. Got:\n{html}");
    assert!(html.contains("tableblock frame-all grid-all"), "expected tableblock classes. Got:\n{html}");
}

#[test]
fn test_table_stripes_html() {
    let html = to_html("[stripes=even]\n|===\n| A | B\n|===");
    assert!(html.contains("stripes-even"), "expected stripes-even class. Got:\n{html}");
    assert!(html.contains("tableblock frame-all grid-all"), "expected tableblock classes. Got:\n{html}");
}

#[test]
fn test_table_stripes_odd_html() {
    let html = to_html("[stripes=odd]\n|===\n| A | B\n|===");
    assert!(html.contains("stripes-odd"), "expected stripes-odd class. Got:\n{html}");
    assert!(html.contains("tableblock frame-all grid-all"), "expected tableblock classes. Got:\n{html}");
}

#[test]
fn test_table_autowidth_stripes_html() {
    let html = to_html("[%autowidth,stripes=even]\n|===\n| A | B\n|===");
    assert!(html.contains("fit-content"));
    assert!(html.contains("stripes-even"));
}

#[test]
fn test_table_caption_default_html() {
    let html = to_html(".My Table\n|===\n| A | B\n|===");
    assert!(html.contains("<caption class=\"title\">Table 1. My Table</caption>"));
}

#[test]
fn test_table_caption_auto_numbering_html() {
    let html = to_html(".First\n|===\n| A\n|===\n\n.Second\n|===\n| B\n|===");
    assert!(html.contains("<caption class=\"title\">Table 1. First</caption>"));
    assert!(html.contains("<caption class=\"title\">Table 2. Second</caption>"));
}

#[test]
fn test_table_caption_custom_prefix_html() {
    let html = to_html("[caption=\"Data Set \"]\n.Results\n|===\n| A | B\n|===");
    assert!(html.contains("<caption class=\"title\">Data Set Results</caption>"));
}

#[test]
fn test_table_caption_empty_prefix_html() {
    let html = to_html("[caption=]\n.Results\n|===\n| A | B\n|===");
    assert!(html.contains("<caption class=\"title\">Results</caption>"));
    assert!(!html.contains("Table 1"));
}

#[test]
fn test_table_no_title_no_caption_html() {
    let html = to_html("|===\n| A | B\n|===");
    assert!(!html.contains("<caption"));
}

#[test]
fn test_table_caption_doc_attr_html() {
    // `:table-caption!:` unsets the label for every table in the document.
    let off = to_html(":table-caption!:\n\n.My Table\n|===\n| A | B\n|===");
    assert!(
        off.contains("<caption class=\"title\">My Table</caption>"),
        "unset table-caption drops the label. Got:\n{off}"
    );

    // A custom `:table-caption: Data Set` replaces the label word but keeps numbering.
    let custom = to_html(":table-caption: Data Set\n\n.First\n|===\n| A\n|===\n\n.Second\n|===\n| B\n|===");
    assert!(custom.contains("<caption class=\"title\">Data Set 1. First</caption>"), "Got:\n{custom}");
    assert!(custom.contains("<caption class=\"title\">Data Set 2. Second</caption>"), "Got:\n{custom}");

    // {table-caption} resolves to the default "Table" like Asciidoctor.
    let reference = to_html("{table-caption}");
    assert!(reference.contains("<p>Table</p>"), "Got:\n{reference}");
}

#[test]
fn test_table_caption_suppressed_not_counted_html() {
    // A table whose label is suppressed (empty caption= or unset table-caption) must not
    // advance the counter, so the next default table keeps the right number.
    let html = to_html(".T1\n|===\n| A\n|===\n\n[caption=]\n.T2\n|===\n| B\n|===\n\n.T3\n|===\n| C\n|===");
    assert!(html.contains("<caption class=\"title\">Table 1. T1</caption>"), "Got:\n{html}");
    assert!(html.contains("<caption class=\"title\">T2</caption>"), "Got:\n{html}");
    assert!(html.contains("<caption class=\"title\">Table 2. T3</caption>"), "Got:\n{html}");
}

#[test]
fn test_table_autowidth_with_id_and_role_html() {
    let html = to_html("[%autowidth#mytable.custom]\n|===\n| A | B\n|===");
    assert!(html.contains("id=\"mytable\""));
    assert!(html.contains("fit-content"));
    assert!(html.contains("custom"));
}

#[test]
fn test_csv_table_html() {
    let html = to_html("[%header,format=csv]\n|===\nName,Age,City\nAlice,30,NYC\nBob,25,LA\n|===");
    assert!(html.contains("<table class=\"tableblock frame-all grid-all stretch\">"));
    assert!(html.contains("<thead>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Age</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">City</th>"));
    assert!(html.contains("</thead>"));
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
    assert!(html.contains("<p class=\"tableblock\">30</p>"));
    assert!(html.contains("<p class=\"tableblock\">NYC</p>"));
    assert!(html.contains("<p class=\"tableblock\">Bob</p>"));
    assert!(html.contains("<p class=\"tableblock\">25</p>"));
    assert!(html.contains("<p class=\"tableblock\">LA</p>"));
    assert!(html.contains("</tbody>"));
    assert!(html.contains("</table>"));
}

#[test]
fn test_csv_table_shorthand_html() {
    let html = to_html("[%header,csv]\n|===\nName,Age\nAlice,30\n|===");
    assert!(html.contains("<thead>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Age</th>"));
    assert!(html.contains("</thead>"));
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
    assert!(html.contains("<p class=\"tableblock\">30</p>"));
    assert!(html.contains("</tbody>"));
}

#[test]
fn test_dsv_table_html() {
    let html = to_html("[%header,format=dsv]\n|===\nName:Age:City\nAlice:30:NYC\n|===");
    assert!(html.contains("<thead>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Age</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">City</th>"));
    assert!(html.contains("</thead>"));
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
    assert!(html.contains("<p class=\"tableblock\">30</p>"));
    assert!(html.contains("<p class=\"tableblock\">NYC</p>"));
    assert!(html.contains("</tbody>"));
}

#[test]
fn test_tsv_table_html() {
    let html = to_html("[%header,format=tsv]\n|===\nName\tAge\tCity\nAlice\t30\tNYC\n|===");
    assert!(html.contains("<thead>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Age</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">City</th>"));
    assert!(html.contains("</thead>"));
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
    assert!(html.contains("<p class=\"tableblock\">30</p>"));
    assert!(html.contains("<p class=\"tableblock\">NYC</p>"));
    assert!(html.contains("</tbody>"));
}

#[test]
fn test_csv_table_no_header_html() {
    let html = to_html("[format=csv]\n|===\nAlice,30\nBob,25\n|===");
    assert!(!html.contains("<thead>"));
    assert!(html.contains("<tbody>"));
    assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
    assert!(html.contains("<p class=\"tableblock\">30</p>"));
    assert!(html.contains("<p class=\"tableblock\">Bob</p>"));
    assert!(html.contains("<p class=\"tableblock\">25</p>"));
    assert!(html.contains("</tbody>"));
}

#[test]
fn test_csv_table_quoted_fields_html() {
    let html = to_html("[%header,csv]\n|===\nName,Description\nAlice,\"Has a, comma\"\n|===");
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Name</th>"));
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">Description</th>"));
    assert!(html.contains("<p class=\"tableblock\">Alice</p>"));
    assert!(html.contains("<p class=\"tableblock\">Has a, comma</p>"));
}

#[test]
fn test_discrete_heading_with_id_and_role() {
    let html = to_html("[discrete#myh.special]\n== Heading");
    assert!(html.contains("id=\"myh\""), "should have explicit id. Got: {html}");
    assert!(html.contains("class=\"discrete special\""), "should have discrete + role class. Got: {html}");
}

// Inline span tests

#[test]
fn test_inline_span_single_role_html() {
    let html = to_html("[.lead]#text#");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"lead\">text</span></p>\n</div>\n");
}

#[test]
fn test_inline_span_multiple_roles_html() {
    let html = to_html("[.big.red]#text#");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"big red\">text</span></p>\n</div>\n");
}

#[test]
fn test_inline_span_id_and_role_html() {
    let html = to_html("[#myid.lead]#text#");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span id=\"myid\" class=\"lead\">text</span></p>\n</div>\n");
}

#[test]
fn test_inline_span_unconstrained_html() {
    let html = to_html("hel[.x]##lo##rld");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>hel<span class=\"x\">lo</span>rld</p>\n</div>\n");
}

#[test]
fn test_bare_highlight_no_regression_html() {
    let html = to_html("#highlight#");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><mark>highlight</mark></p>\n</div>\n");
}

#[test]
fn test_block_admonition_html() {
    let html = to_html("[NOTE]\n====\nThis is a note.\n====");
    assert!(html.contains("<div class=\"admonitionblock note\">"), "no admonitionblock note in:\n{html}");
    assert!(html.contains("<div class=\"title\">Note</div>"), "no title in:\n{html}");
    // Block-form admonition content keeps normal paragraph wrappers
    assert!(
        html.contains("<td class=\"content\">\n<div class=\"paragraph\">\n<p>This is a note.</p>\n</div>\n</td>"),
        "no wrapped td content in:\n{html}"
    );
    assert!(html.contains("</td>\n</tr>\n</table>\n</div>"), "no closing tags in:\n{html}");
}

#[test]
fn test_block_admonition_multi_para_html() {
    let html = to_html("[NOTE]\n====\nFirst paragraph.\n\nSecond paragraph.\n====");
    assert!(html.contains("<div class=\"admonitionblock note\">"), "no admonition class in:\n{html}");
    // Each child paragraph gets its own full wrapper (asciidoctor compound content)
    assert!(html.contains("<div class=\"paragraph\">\n<p>First paragraph.</p>\n</div>"), "no wrapped first para in:\n{html}");
    assert!(html.contains("<div class=\"paragraph\">\n<p>Second paragraph.</p>\n</div>"), "no wrapped second para in:\n{html}");
    assert!(html.contains("<td class=\"content\">"), "no td content in:\n{html}");
}

#[test]
fn test_admonition_block_vs_paragraph_forms() {
    // Open block with admonition style: compound admonition, wrapped paragraph
    let html = to_html("[NOTE]\n--\nopen note.\n--");
    assert!(html.contains("<div class=\"admonitionblock note\">"), "open form should be admonition:\n{html}");
    assert!(html.contains("<div class=\"paragraph\">\n<p>open note.</p>\n</div>"), "open form should wrap para:\n{html}");

    // Paragraph forms render bare text in the content td
    for src in ["TIP: bare text.", "[TIP]\nbare text."] {
        let html = to_html(src);
        assert!(html.contains("<td class=\"content\">\nbare text.\n</td>"), "paragraph form should be bare for {src:?}:\n{html}");
    }

    // Admonition style on sidebar/quote is ignored (native block kept)
    let html = to_html("[NOTE]\n****\nsidebar text.\n****");
    assert!(html.contains("sidebarblock"), "sidebar should stay sidebar:\n{html}");
    assert!(!html.contains("admonitionblock"), "sidebar must not become admonition:\n{html}");
    let html = to_html("[NOTE]\n____\nquote text.\n____");
    assert!(html.contains("quoteblock"), "quote should stay quote:\n{html}");
    assert!(!html.contains("admonitionblock"), "quote must not become admonition:\n{html}");

    // Paragraph-form admonition nested in a block-form one: inner stays bare
    let html = to_html("[TIP]\n====\nNOTE: nested.\n====");
    assert!(html.contains("<div class=\"admonitionblock note\">"), "nested admonition missing:\n{html}");
    assert!(html.contains("<td class=\"content\">\nnested.\n</td>"), "nested paragraph form should be bare:\n{html}");
}

// Admonition icons tests

#[test]
fn test_admonition_icons_font() {
    let html = to_html(":icons: font\n\nNOTE: This is a note.");
    assert!(html.contains("<i class=\"fa icon-note\" title=\"Note\"></i>"));
    assert!(!html.contains("<div class=\"title\">Note</div>"));
}

#[test]
fn test_admonition_icons_font_all_kinds() {
    for (marker, icon, label) in [
        ("NOTE", "note", "Note"),
        ("TIP", "tip", "Tip"),
        ("IMPORTANT", "important", "Important"),
        ("WARNING", "warning", "Warning"),
        ("CAUTION", "caution", "Caution"),
    ] {
        let input = format!(":icons: font\n\n{marker}: Some text.");
        let html = to_html(&input);
        let expected = format!("<i class=\"fa icon-{icon}\" title=\"{label}\"></i>");
        assert!(
            html.contains(&expected),
            "Expected {expected} in HTML for {marker}, got: {html}"
        );
    }
}

#[test]
fn test_admonition_default_no_icons() {
    let html = to_html("NOTE: This is a note.");
    assert!(html.contains("<div class=\"title\">Note</div>"));
    assert!(!html.contains("<i class=\"fa"));
}

#[test]
fn test_block_admonition_icons_font() {
    let html = to_html(":icons: font\n\n[NOTE]\n====\nBlock note content.\n====");
    assert!(html.contains("<i class=\"fa icon-note\" title=\"Note\"></i>"));
    assert!(!html.contains("<div class=\"title\">Note</div>"));
}

#[test]
fn test_admonition_icons_image() {
    // Any `icons` value other than "font" selects image-based icons.
    let html = to_html(":icons: image\n\nNOTE: This is a note.");
    assert!(html.contains("<img src=\"./images/icons/note.png\" alt=\"Note\">"));
    assert!(!html.contains("<div class=\"title\">Note</div>"));

    // Empty value works too, and iconsdir/icontype are honored.
    let html = to_html(":icons:\n:iconsdir: img/i\n:icontype: svg\n\nTIP: hint");
    assert!(html.contains("<img src=\"img/i/tip.svg\" alt=\"Tip\">"));

    // A value that merely starts with "font" is not font mode (mid-document
    // entries skip Asciidoctor's init-time icons/icontype normalization).
    let html = to_html(":icons: font <1>\n\nNOTE: hello");
    assert!(html.contains("<img src=\"./images/icons/note.png\" alt=\"Note\">"));

    // caption= overrides the alt text.
    let html = to_html(":icons: image\n\n[NOTE,caption=Custom]\n====\nx\n====");
    assert!(html.contains("alt=\"Custom\""));
}

#[test]
fn test_sect0_heading_standalone() {
    // A level-0 section in the body renders as a bare <h1 class="sect0">
    // with no wrapper div and no sectionbody (article and book alike).
    let html = to_html("para\n\n= Heading Zero\n\nafter\n\n== Real Section\n\ninside");
    assert!(html.contains("<h1 id=\"_heading_zero\" class=\"sect0\">Heading Zero</h1>"));
    assert!(!html.contains("<div class=\"sect0\">"));
    // Following content is not nested inside a sect0 div.
    assert!(html.contains("</h1>\n<div class=\"paragraph\">\n<p>after</p>"));
    // Regular sections keep their wrapper.
    assert!(html.contains("<div class=\"sect1\">\n<h2 id=\"_real_section\">"));
}

#[test]
fn test_partintro_paragraph_masquerades_as_open_block() {
    // [partintro] on a paragraph masquerades it as an open block:
    // <div class="openblock partintro"><div class="content"><div class="paragraph"><p>…
    let html = to_html("= Book\n:doctype: book\n\n= Part I\n\n[partintro]\nIntro text.\n\n== Chapter A\n\ntext");
    assert!(html.contains(
        "<div class=\"openblock partintro\">\n<div class=\"content\">\n<div class=\"paragraph\">\n<p>Intro text.</p>\n</div>\n</div>\n</div>"
    ));
    assert!(!html.contains("<div class=\"paragraph partintro\">"));
    // [partintro] on an explicit open block keeps working unchanged.
    let html = to_html("= Book\n:doctype: book\n\n= Part I\n\n[partintro]\n--\nIntro text.\n--\n\n== Chapter A\n\ntext");
    assert!(html.contains(
        "<div class=\"openblock partintro\">\n<div class=\"content\">\n<div class=\"paragraph\">\n<p>Intro text.</p>\n</div>\n</div>\n</div>"
    ));
}

// Preamble tests

#[test]
fn test_preamble_html() {
    let html = to_html("= Title\n\nPreamble text.\n\n== Section");
    assert!(html.contains("<div id=\"preamble\">"));
    assert!(html.contains("<div class=\"sectionbody\">"));
    assert!(html.contains("<p>Preamble text.</p>"));
    assert!(html.contains("</div>\n</div>\n<div class=\"sect"));
}

#[test]
fn test_preamble_multiple_blocks_html() {
    let html = to_html("= Title\n\nFirst para.\n\nSecond para.\n\n== Section");
    assert!(html.contains("<div id=\"preamble\">"));
    assert!(html.contains("<p>First para.</p>"));
    assert!(html.contains("<p>Second para.</p>"));
}

#[test]
fn test_no_preamble_without_title_html() {
    let html = to_html("Content.\n\n== Section");
    assert!(!html.contains("preamble"));
}

#[test]
fn test_no_preamble_without_section_html() {
    let html = to_html("= Title\n\nContent only.");
    assert!(!html.contains("preamble"));
    assert!(html.contains("<p>Content only.</p>"));
}

// Appendix section tests

#[test]
fn test_appendix_section_html() {
    let html = to_html("[appendix]\n== My Appendix\n\nContent.");
    assert!(html.contains("class=\"sect1\""));
    assert!(html.contains("Appendix A: My Appendix</h2>"));
}

#[test]
fn test_appendix_multiple_html() {
    let html = to_html("[appendix]\n== First\n\nContent.\n\n[appendix]\n== Second\n\nMore.");
    assert!(html.contains("Appendix A: First</h2>"));
    assert!(html.contains("Appendix B: Second</h2>"));
}

#[test]
fn test_appendix_no_caption_without_style_html() {
    let html = to_html("== Regular Section\n\nContent.");
    assert!(!html.contains("Appendix"));
}

#[test]
fn test_glossary_section_html() {
    let html = to_html("[glossary]\n== Terms\n\nSome terms here.");
    assert!(html.contains("class=\"sect1\""));
}

#[test]
fn test_bibliography_section_html() {
    let html = to_html("[bibliography]\n== References\n\n* [[[ref1]]] First ref.");
    // bibliography style is not added to section div, but propagated to child list
    assert!(html.contains("class=\"sect1\""));
    assert!(html.contains("class=\"ulist bibliography\""));
    assert!(html.contains("class=\"bibliography\""));
}

#[test]
fn test_bibliography_xref_uses_bracketed_reftext() {
    // `<<pp>>` to `[[[pp]]]` -> link text `[pp]`; labeled `[[[gof,gang]]]`
    // resolves `<<gof>>` to the bracketed label `[gang]` (not `[gof]`).
    let html = to_html(
        "See <<pp>> and <<gof>>.\n\n\
         [bibliography]\n== Refs\n\n\
         * [[[pp]]] Pragmatic Programmer.\n\
         * [[[gof,gang]]] Gang of Four.",
    );
    assert!(html.contains("<a href=\"#pp\">[pp]</a>"), "{html}");
    assert!(html.contains("<a href=\"#gof\">[gang]</a>"), "{html}");
}

#[test]
fn test_unresolved_internal_xref_falls_back_to_bracketed_id() {
    // An internal `<<id>>` with no matching section/block/bibliography and no
    // explicit text falls back to `[id]`, matching Asciidoctor's xreflabel.
    let html = to_html("See <<anchors>> and <<missing,custom text>>.");
    assert!(html.contains("<a href=\"#anchors\">[anchors]</a>"), "{html}");
    // Explicit text still wins over the bracketed fallback.
    assert!(html.contains(">custom text</a>"), "{html}");
    // Inter-document refs keep their raw rewritten path (no brackets).
    let interdoc = to_html("See <<other.adoc#sec>>.");
    assert!(!interdoc.contains("[other"), "{interdoc}");
}

#[test]
fn test_resolved_internal_xref_not_bracketed() {
    // A natural cross reference that resolves to a section title is rendered
    // with the raw title, not bracketed.
    let html = to_html("See <<Target Section>>.\n\n== Target Section\n\nBody.");
    assert!(html.contains(">Target Section</a>"), "{html}");
    assert!(!html.contains("[Target Section]"), "{html}");
}

#[test]
fn test_colophon_section_html() {
    let html = to_html("[colophon]\n== Colophon\n\nPublishing info.");
    assert!(html.contains("class=\"sect1\""));
}

#[test]
fn test_abstract_section_html() {
    let html = to_html("[abstract]\n== Summary\n\nBrief summary.");
    assert!(html.contains("class=\"sect1\""));
}

#[test]
fn test_special_section_no_sectnums() {
    let html = to_html(":sectnums:\n\n== Numbered\n\n[glossary]\n== Terms\n\n[bibliography]\n== Refs\n\n== Also Numbered");
    // Regular sections should be numbered
    assert!(html.contains("1. Numbered"));
    assert!(html.contains("2. Also Numbered"));
    // Special sections should NOT be numbered
    assert!(html.contains(">Terms</h2>"));
    assert!(html.contains(">Refs</h2>"));
}

// Section numbering tests

#[test]
fn test_sectnums_basic() {
    let html = to_html("= Doc\n:sectnums:\n\n== First\n\n== Second");
    assert!(html.contains("1. First</h2>"));
    assert!(html.contains("2. Second</h2>"));
}

#[test]
fn test_sectnums_nested() {
    let html = to_html("= Doc\n:sectnums:\n\n== Chapter\n\n=== Sub One\n\n=== Sub Two\n\n== Next");
    assert!(html.contains("1. Chapter</h2>"));
    assert!(html.contains("1.1. Sub One</h3>"));
    assert!(html.contains("1.2. Sub Two</h3>"));
    assert!(html.contains("2. Next</h2>"));
}

#[test]
fn test_sectnums_disabled() {
    let html = to_html("= Doc\n\n== First\n\n== Second");
    assert!(html.contains(">First</h2>"));
    assert!(html.contains(">Second</h2>"));
    assert!(!html.contains("1. "));
}

#[test]
fn test_sectnums_unset() {
    let html = to_html("= Doc\n:sectnums:\n\n== Numbered\n\n:!sectnums:\n\n== Not Numbered");
    assert!(html.contains("1. Numbered</h2>"));
    assert!(html.contains(">Not Numbered</h2>"));
}

#[test]
fn test_sectnums_appendix_not_numbered() {
    let html = to_html("= Doc\n:sectnums:\n\n== Regular\n\n[appendix]\n== My Appendix");
    assert!(html.contains("1. Regular</h2>"));
    assert!(html.contains("Appendix A: My Appendix</h2>"));
    assert!(!html.contains("2. My Appendix"));
}

// Horizontal description list tests

#[test]
fn test_horizontal_description_list_html() {
    let html = to_html("[horizontal]\nCPU:: The brain\nRAM:: Memory");
    assert_eq!(
        html,
        "<div class=\"hdlist\">\n<table>\n\
         <tr>\n<td class=\"hdlist1\">CPU</td>\n<td class=\"hdlist2\">\n<p>The brain</p>\n</td>\n</tr>\n\
         <tr>\n<td class=\"hdlist1\">RAM</td>\n<td class=\"hdlist2\">\n<p>Memory</p>\n</td>\n</tr>\n\
         </table>\n</div>\n"
    );
}

#[test]
fn test_horizontal_description_list_multiple_terms_html() {
    // Parser treats each term:: line as separate entry
    // This test verifies multiple entries render correctly
    let html = to_html("[horizontal]\nTerm1:: Desc1\nTerm2:: Desc2");
    assert!(html.contains("<td class=\"hdlist1\">Term1</td>"));
    assert!(html.contains("<td class=\"hdlist2\">\n<p>Desc1</p>\n</td>"));
    assert!(html.contains("<td class=\"hdlist1\">Term2</td>"));
    assert!(html.contains("<td class=\"hdlist2\">\n<p>Desc2</p>\n</td>"));
    assert_eq!(html.matches("<tr>").count(), 2);
}

#[test]
fn test_horizontal_description_list_empty_desc_html() {
    let html = to_html("[horizontal]\nTerm:: ");
    assert!(html.contains("<div class=\"hdlist\">"));
    assert!(html.contains("<td class=\"hdlist1\">Term</td>"));
    assert!(html.contains("<td class=\"hdlist2\">"));
}

#[test]
fn test_horizontal_description_list_with_id_html() {
    let html = to_html("[horizontal#mylist]\nA:: B");
    assert!(html.contains("id=\"mylist\""));
    assert!(html.contains("class=\"hdlist\""));
    assert!(html.contains("<table>"));
}

#[test]
fn test_normal_description_list_unchanged_html() {
    let html = to_html("CPU:: The brain\nRAM:: Memory");
    assert_eq!(
        html,
        "<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">CPU</dt>\n<dd>\n<p>The brain</p>\n</dd>\n<dt class=\"hdlist1\">RAM</dt>\n<dd>\n<p>Memory</p>\n</dd>\n</dl>\n</div>\n"
    );
}

#[test]
fn test_qanda_description_list_html() {
    let html = to_html("[qanda]\nWhat is Rust?:: A systems programming language.\nWhy use it?:: Memory safety.");
    assert_eq!(
        html,
        "<div class=\"qlist qanda\">\n<ol>\n\
         <li>\n<p><em>What is Rust?</em></p>\nA systems programming language.</li>\n\
         <li>\n<p><em>Why use it?</em></p>\nMemory safety.</li>\n\
         </ol>\n</div>\n"
    );
}

#[test]
fn test_qanda_description_list_empty_answer_html() {
    let html = to_html("[qanda]\nQuestion?:: ");
    assert!(html.contains("<div class=\"qlist qanda\">"));
    assert!(html.contains("<li>\n<p><em>Question?</em></p>"));
    assert!(html.contains("</li>"));
}

#[test]
fn test_qanda_description_list_with_id_html() {
    let html = to_html("[qanda#faq]\nQ:: A");
    assert!(html.contains("id=\"faq\""));
    assert!(html.contains("class=\"qlist qanda\""));
    assert!(html.contains("<ol>"));
}

#[test]
fn test_block_image_dimensions_html() {
    let html = to_html("image::sunset.jpg[A beautiful sunset,600,400]");
    assert!(html.contains("src=\"sunset.jpg\""));
    assert!(html.contains("alt=\"A beautiful sunset\""));
    assert!(html.contains("width=\"600\""));
    assert!(html.contains("height=\"400\""));
}

#[test]
fn test_block_image_named_dimensions_html() {
    let html = to_html("image::photo.jpg[alt=Photo,width=800,height=600]");
    assert!(html.contains("src=\"photo.jpg\""));
    assert!(html.contains("alt=\"Photo\""));
    assert!(html.contains("width=\"800\""));
    assert!(html.contains("height=\"600\""));
}

#[test]
fn test_block_image_width_only_html() {
    let html = to_html("image::photo.jpg[Photo,300]");
    assert!(html.contains("src=\"photo.jpg\""));
    assert!(html.contains("alt=\"Photo\""));
    assert!(html.contains("width=\"300\""));
    assert!(!html.contains("height="));
}

#[test]
fn test_block_image_no_dimensions_html() {
    let html = to_html("image::sunset.jpg[A beautiful sunset]");
    assert!(html.contains("src=\"sunset.jpg\""));
    assert!(html.contains("alt=\"A beautiful sunset\""));
    assert!(!html.contains("width="));
    assert!(!html.contains("height="));
}

#[test]
fn test_block_image_figure_caption() {
    // Titled image: caption AFTER the content div, "Figure N. " prefix,
    // shared counter bumped only by titled images. The title must NOT
    // leak into the following block (regression guard).
    let html = to_html(
        ".First\nimage::a.png[]\n\nimage::b.png[]\n\n.Second *bold*\nimage::c.png[]\n\nplain\n",
    );
    assert!(html.contains(
        "<img src=\"a.png\" alt=\"a\">\n</div>\n<div class=\"title\">Figure 1. First</div>\n</div>"
    ));
    assert!(html.contains(
        "<div class=\"title\">Figure 2. Second <strong>bold</strong></div>"
    ));
    assert!(html.contains("<div class=\"paragraph\">\n<p>plain</p>"));
    assert!(!html.contains("<div class=\"paragraph\">\n<div class=\"title\">"));

    // Unset figure-caption: bare title, no number, no counter bump.
    let html = to_html(":figure-caption!:\n\n.Bare\nimage::a.png[]");
    assert!(html.contains("<div class=\"title\">Bare</div>"));
    assert!(!html.contains("Figure"));

    // Custom label via :figure-caption:.
    let html = to_html(":figure-caption: Рисунок\n\n.Custom\nimage::a.png[]");
    assert!(html.contains("<div class=\"title\">Рисунок 1. Custom</div>"));

    // caption= macro attr: verbatim prefix, no counter bump; the next
    // titled image is still Figure 1.
    let html = to_html(
        ".Titled\nimage::a.png[caption=\"My Caption. \"]\n\n.Counted\nimage::b.png[]",
    );
    assert!(html.contains("<div class=\"title\">My Caption. Titled</div>"));
    assert!(html.contains("<div class=\"title\">Figure 1. Counted</div>"));

    // title= macro attr creates the caption and wins over `.Title`;
    // named-only attrs leave alt auto-generated from the filename.
    let html = to_html(".DotTitle\nimage::b.png[title=AttrTitle]");
    assert!(html.contains("alt=\"b\""));
    assert!(html.contains("<div class=\"title\">Figure 1. AttrTitle</div>"));
    assert!(!html.contains("DotTitle"));
}

#[test]
fn test_inline_image_dimensions_html() {
    let html = to_html("see image:icon.png[Icon,32,32]");
    assert!(html.contains("src=\"icon.png\""));
    assert!(html.contains("alt=\"Icon\""));
    assert!(html.contains("width=\"32\""));
    assert!(html.contains("height=\"32\""));
}

#[test]
fn test_block_image_align_center() {
    let html = to_html("image::photo.jpg[Alt,align=center]");
    assert!(html.contains("class=\"imageblock text-center\""));
}

#[test]
fn test_block_image_float_left() {
    let html = to_html("image::photo.jpg[Alt,float=left]");
    assert!(html.contains("class=\"imageblock left\""));
}

#[test]
fn test_block_image_align_float_class_escaped() {
    // Regression (D1): align/float values flow into the class attribute and
    // must be HTML-escaped (no raw markup characters leak into the class).
    let html = to_html("image::photo.jpg[Alt,float=a<b>c]");
    assert!(html.contains("a&lt;b&gt;c"), "float value not escaped: {html}");
    assert!(!html.contains("a<b>c"), "raw unescaped value present: {html}");
}

#[test]
fn test_block_image_align_from_block_attrs() {
    let html = to_html("[align=center]\nimage::photo.jpg[Alt]");
    assert!(html.contains("class=\"imageblock text-center\""));
}

#[test]
fn test_block_image_float_right() {
    let html = to_html("image::photo.jpg[Alt,float=right]");
    assert!(html.contains("class=\"imageblock right\""));
}

#[test]
fn test_block_image_align_right() {
    let html = to_html("image::photo.jpg[Alt,align=right]");
    assert!(html.contains("class=\"imageblock text-right\""));
}

#[test]
fn test_inline_image_float_left() {
    let html = to_html("text image:icon.png[Icon,float=left] more");
    assert!(html.contains("class=\"image left\""));
}

#[test]
fn test_inline_image_align_center() {
    let html = to_html("text image:icon.png[Icon,align=center] more");
    assert!(html.contains("class=\"image text-center\""));
}

#[test]
fn test_block_image_with_link() {
    let html = to_html("image::thumb.jpg[Alt,link=fullsize.jpg]");
    assert!(html.contains("<a class=\"image\" href=\"fullsize.jpg\"><img src=\"thumb.jpg\" alt=\"Alt\"></a>"));
}

#[test]
fn test_inline_image_with_link() {
    let html = to_html("text image:icon.png[Icon,link=https://example.com] more");
    assert!(html.contains("<a class=\"image\" href=\"https://example.com\"><img src=\"icon.png\" alt=\"Icon\"></a>"));
}

#[test]
fn test_block_image_with_link_and_dimensions() {
    let html = to_html("image::photo.jpg[Alt,300,200,link=big.jpg]");
    assert!(html.contains("<a class=\"image\" href=\"big.jpg\"><img src=\"photo.jpg\" alt=\"Alt\" width=\"300\" height=\"200\"></a>"));
}

#[test]
fn test_block_image_without_link_no_anchor() {
    let html = to_html("image::photo.jpg[Alt]");
    assert!(!html.contains("<a "));
}

#[test]
fn test_collapsible_block_html() {
    let html = to_html("[%collapsible]\n====\nContent\n====");
    assert!(html.contains("<details"));
    assert!(html.contains("<summary class=\"title\">Details</summary>"));
    assert!(html.contains("<div class=\"content\">"));
    assert!(html.contains("<p>Content</p>"));
    assert!(html.contains("</div>\n</details>"));
    assert!(!html.contains("<div class=\"exampleblock\">"));
}

#[test]
fn test_collapsible_block_with_title_html() {
    let html = to_html(".Click to expand\n[%collapsible]\n====\nContent\n====");
    assert!(html.contains("<details"));
    assert!(html.contains("<summary class=\"title\">Click to expand</summary>"));
    assert!(!html.contains("<div class=\"title\">Click to expand</div>"));
    assert!(html.contains("<p>Content</p>"));
    assert!(html.contains("</div>\n</details>"));
}

#[test]
fn test_collapsible_block_open_html() {
    let html = to_html("[%collapsible%open]\n====\nContent\n====");
    assert!(html.contains("<details"));
    assert!(html.contains(" open>"));
    assert!(html.contains("<summary class=\"title\">Details</summary>"));
    assert!(html.contains("<p>Content</p>"));
}

#[test]
fn test_collapsible_block_with_id_html() {
    let html = to_html("[%collapsible#myid]\n====\nContent\n====");
    assert!(html.contains("<details id=\"myid\""));
    assert!(html.contains("<summary class=\"title\">Details</summary>"));
}

#[test]
fn test_example_block_unchanged_html() {
    let html = to_html("====\nContent\n====");
    assert!(html.contains("<div class=\"exampleblock\">"));
    assert!(html.contains("<div class=\"content\">"));
    assert!(html.contains("<p>Content</p>"));
    assert!(html.contains("</div>\n</div>"));
    assert!(!html.contains("<details"));
    assert!(!html.contains("<summary"));
}

// === Block substitution tests ===

#[test]
fn test_listing_block_subs_normal() {
    let html = to_html("[subs=normal]\n----\n*bold*\n----");
    assert!(html.contains("<strong>bold</strong>"), "subs=normal on listing block should enable inline parsing. Got: {html}");
}

#[test]
fn test_paragraph_subs_none() {
    let html = to_html("[subs=none]\n*bold* & <tag>");
    assert!(!html.contains("<strong>"), "subs=none should disable inline parsing. Got: {html}");
    assert!(!html.contains("&amp;"), "subs=none should disable specialchars. Got: {html}");
    assert!(html.contains("*bold*"), "subs=none should preserve literal asterisks. Got: {html}");
    assert!(html.contains("<tag>"), "subs=none should pass through raw tags. Got: {html}");
}

#[test]
fn test_listing_block_subs_plus_quotes() {
    let html = to_html("[subs=\"+quotes\"]\n----\n*bold*\n----");
    assert!(html.contains("<strong>bold</strong>"), "subs=+quotes on listing block should enable quote formatting. Got: {html}");
}

#[test]
fn test_paragraph_subs_minus_replacements() {
    let html = to_html("[subs=\"-replacements\"]\nHello (C)");
    assert!(html.contains("(C)"), "subs=-replacements should not replace (C) with ©. Got: {html}");
    assert!(!html.contains("\u{00A9}"), "subs=-replacements should not produce ©. Got: {html}");
}

#[test]
fn test_example_block_no_subs_unchanged() {
    let html = to_html("====\n*bold* text\n====");
    assert!(html.contains("<strong>bold</strong>"), "Example block without subs should process inline normally. Got: {html}");
}

#[test]
fn test_listing_block_default_no_inline() {
    let html = to_html("----\n*bold*\n----");
    assert!(!html.contains("<strong>"), "Listing block default should NOT process inline formatting. Got: {html}");
    assert!(html.contains("*bold*"), "Listing block default should preserve raw markup. Got: {html}");
}

#[test]
fn test_literal_paragraph_subs_normal() {
    let html = to_html("[subs=normal]\n  literal *bold*");
    assert!(html.contains("<strong>bold</strong>"), "subs=normal on literal paragraph should enable inline parsing. Got: {html}");
}

#[test]
fn test_literal_paragraph_block_title() {
    // A `.Title` preceding an indented literal paragraph must render a
    // `<div class="title">` inside the literalblock, exactly like a
    // delimited literal block (`....`) does. Previously the inline
    // LiteralParagraph arm forgot to flush the pending block title.
    let html = to_html(".TOC enabled via the CLI\n $ asciidoctor -a toc my-document.adoc");
    assert!(
        html.contains("<div class=\"literalblock\">\n<div class=\"title\">TOC enabled via the CLI</div>\n<div class=\"content\">"),
        "indented literal paragraph must emit its block title. Got: {html}"
    );
    // Regression guard: a literal paragraph without a title must not gain a
    // spurious empty title div.
    let no_title = to_html(" $ plain literal");
    assert!(
        !no_title.contains("class=\"title\""),
        "title-less literal paragraph must not emit a title div. Got: {no_title}"
    );
}

#[test]
fn test_verbatim_block_unknown_style_dropped_from_class() {
    // An unrecognized block style (e.g. [plantuml] with no diagram extension)
    // is dropped from the verbatim block class, matching Asciidoctor: a literal
    // block stays `literalblock`, a listing block stays `listingblock`. Roles
    // applied alongside the style must still survive.
    let lit = to_html("[plantuml]\n....\n@startuml\n....");
    assert!(lit.contains("class=\"literalblock\""), "unknown style must not leak into literal class. Got: {lit}");
    assert!(!lit.contains("plantuml"), "plantuml style must be dropped. Got: {lit}");

    let listing = to_html("[plantuml]\n----\ncode\n----");
    assert!(listing.contains("class=\"listingblock\""), "unknown style must not leak into listing class. Got: {listing}");
    assert!(!listing.contains("plantuml"), "plantuml style must be dropped. Got: {listing}");

    let with_role = to_html("[plantuml.diagram]\n....\nx\n....");
    assert!(with_role.contains("class=\"literalblock diagram\""), "role must survive while style is dropped. Got: {with_role}");
}

#[test]
fn test_paragraph_subs_verbatim() {
    let html = to_html("[subs=verbatim]\n*bold* & <tag>");
    assert!(!html.contains("<strong>"), "subs=verbatim should disable inline parsing. Got: {html}");
    assert!(html.contains("&amp;"), "subs=verbatim should still escape specialchars. Got: {html}");
    assert!(html.contains("&lt;tag&gt;"), "subs=verbatim should escape angle brackets. Got: {html}");
}

#[test]
fn test_source_block_subs_plus_quotes() {
    let html = to_html("[source,rust,subs=\"+quotes\"]\n----\nlet x = *bold*;\n----");
    assert!(html.contains("<strong>bold</strong>"), "subs=+quotes on source block should enable formatting. Got: {html}");
}

#[test]
fn test_source_block_subs_minus_callouts() {
    // With -callouts, callout markers should be left as-is (not stripped)
    let html = to_html("[source,rust,subs=\"-callouts\"]\n----\nlet x = 1; // <1>\n----");
    assert!(!html.contains("<b class=\"conum\""), "subs=-callouts should not produce callout markers. Got: {html}");
}

#[test]
fn test_listing_block_subs_plus_attributes() {
    let html = to_html(":myattr: hello\n\n[subs=\"+attributes\"]\n----\nValue is {myattr}\n----");
    assert!(html.contains("Value is hello"), "subs=+attributes on listing block should resolve attribute refs. Got: {html}");
}

#[test]
fn test_listing_block_attr_ref_no_replacements() {
    // A resolved attribute value follows the block's substitution set. In a verbatim
    // listing block (specialchars + attributes, no replacements) an apostrophe stays
    // straight; in a normal paragraph the same value is curled by replacements.
    let listing = to_html(":replace-me: I've been replaced!\n\n[subs=\"+attributes\"]\n----\n{replace-me}\n----");
    assert!(listing.contains("I've been replaced!"), "listing +attributes must keep straight apostrophe. Got: {listing}");
    assert!(!listing.contains('\u{2019}'), "listing +attributes must not curl apostrophe. Got: {listing}");

    let para = to_html(":replace-me: I've been replaced!\n\n{replace-me}");
    assert!(para.contains('\u{2019}'), "normal paragraph must curl apostrophe in resolved attr value. Got: {para}");
}

#[test]
fn test_source_block_subs_normal() {
    let html = to_html("[source,subs=normal]\n----\n*bold* & (C)\n----");
    assert!(html.contains("<strong>bold</strong>"), "subs=normal on source block should enable inline parsing. Got: {html}");
}

#[test]
fn test_listing_block_subs_explicit_list() {
    // Only specialchars and quotes — no replacements
    let html = to_html("[subs=\"specialchars,quotes\"]\n----\n*bold* & (C)\n----");
    assert!(html.contains("<strong>bold</strong>"), "explicit subs should enable quotes. Got: {html}");
    assert!(html.contains("&amp;"), "explicit subs with specialchars should escape &. Got: {html}");
    assert!(html.contains("(C)"), "explicit subs without replacements should not replace (C). Got: {html}");
}

#[test]
fn test_sidebar_block_subs_none() {
    let html = to_html("[subs=none]\n****\n*bold* & <tag>\n****");
    assert!(!html.contains("<strong>"), "subs=none on sidebar should disable inline. Got: {html}");
    assert!(html.contains("<tag>"), "subs=none on sidebar should pass raw tags. Got: {html}");
}

#[test]
fn test_quote_block_subs_verbatim() {
    let html = to_html("[subs=verbatim]\n____\n*bold* & <tag>\n____");
    assert!(!html.contains("<strong>"), "subs=verbatim on quote block should disable inline. Got: {html}");
    assert!(html.contains("&amp;"), "subs=verbatim on quote block should escape &. Got: {html}");
}

#[test]
fn test_source_block_no_highlighter() {
    let html = to_html("[source,rust]\n----\nfn main() {}\n----");
    assert!(html.contains("<pre class=\"highlight\"><code class=\"language-rust\" data-lang=\"rust\">"), "Without highlighter: <pre class=\"highlight\"><code class=\"language-X\" data-lang=\"X\">. Got: {html}");
}

#[test]
fn test_source_block_highlightjs() {
    let html = to_html(":source-highlighter: highlight.js\n\n[source,rust]\n----\nfn main() {}\n----");
    assert!(html.contains("<pre class=\"highlightjs highlight\">"), "highlight.js: pre class. Got: {html}");
    assert!(html.contains("class=\"hljs language-rust\""), "highlight.js: hljs + language class on code. Got: {html}");
    assert!(html.contains("data-lang=\"rust\""), "highlight.js: data-lang on code. Got: {html}");
}

#[test]
fn test_source_block_rouge() {
    let html = to_html(":source-highlighter: rouge\n\n[source,ruby]\n----\nputs 'hi'\n----");
    assert!(html.contains("<pre class=\"rouge highlight\">"), "rouge: pre class. Got: {html}");
    assert!(html.contains("data-lang=\"ruby\""), "rouge: data-lang on code. Got: {html}");
    assert!(!html.contains("class=\"language-ruby\""), "rouge: no language- class on code. Got: {html}");
}

#[test]
fn test_source_block_linenums() {
    let html = to_html(":source-highlighter: highlight.js\n\n[source,rust,%linenums]\n----\nfn main() {}\n----");
    assert!(html.contains("linenums"), "linenums option should add linenums class. Got: {html}");
    assert!(html.contains("highlightjs highlight"), "highlightjs highlight classes should be present. Got: {html}");
    assert!(html.contains("<table class=\"linenotable\">"), "linenums should produce linenotable. Got: {html}");
}

#[test]
fn test_source_block_linenums_no_highlighter() {
    let html = to_html("[source,rust,%linenums]\n----\nfn main() {}\n----");
    assert!(html.contains("linenums"), "linenums should work even without highlighter. Got: {html}");
    assert!(html.contains("<table class=\"linenotable\">"), "linenums should produce linenotable. Got: {html}");
}

#[test]
fn test_source_block_linenums_basic() {
    let html = to_html("[source,ruby,%linenums]\n----\nputs \"Hello\"\nx = 42\nputs x\n----");
    assert!(html.contains("<td class=\"linenos\"><pre class=\"linenos\">1\n2\n3</pre></td>"), "should have line numbers 1-3. Got: {html}");
    assert!(html.contains("<td class=\"code\"><pre>puts \"Hello\"\nx = 42\nputs x</pre></td>"), "should have code in td. Got: {html}");
}

#[test]
fn test_source_block_linenums_start() {
    let html = to_html("[source,ruby,%linenums,start=10]\n----\nputs \"Hello\"\nx = 42\nputs x\n----");
    assert!(html.contains("<td class=\"linenos\"><pre class=\"linenos\">10\n11\n12</pre></td>"), "should have line numbers 10-12. Got: {html}");
}

#[test]
fn test_source_block_linenums_with_highlight() {
    let html = to_html("[source,rust,%linenums,highlight=2]\n----\nlet a = 1;\nlet b = 2;\nlet c = 3;\n----");
    assert!(html.contains("<table class=\"linenotable\">"), "should have linenotable. Got: {html}");
    assert!(html.contains("<span class=\"hll\">let b = 2;</span>"), "should have highlight span in code. Got: {html}");
    assert!(html.contains("<td class=\"code\">"), "should have code td. Got: {html}");
}

#[test]
fn test_source_block_linenums_single_line() {
    let html = to_html("[source,ruby,%linenums]\n----\nputs \"hi\"\n----");
    assert!(html.contains("<pre class=\"linenos\">1</pre>"), "single line should have just 1. Got: {html}");
}

#[test]
fn test_source_block_linenums_with_callouts() {
    let html = to_html("[source,ruby,%linenums]\n----\nputs \"Hello\" <1>\nx = 42 <2>\n----");
    assert!(html.contains("<td class=\"code\">"), "should have code td. Got: {html}");
    assert!(html.contains("<b class=\"conum\">(1)</b>"), "should have callout. Got: {html}");
}

#[test]
fn test_source_block_no_language() {
    let html = to_html(":source-highlighter: highlight.js\n\n[source]\n----\nsome code\n----");
    assert!(html.contains("<pre class=\"highlightjs highlight\">"), "No language: pre class should still have highlighter. Got: {html}");
    assert!(!html.contains("data-lang"), "No language: no data-lang. Got: {html}");
    assert!(!html.contains("language-"), "No language: no language- class. Got: {html}");
}

#[test]
fn test_source_block_pygments() {
    let html = to_html(":source-highlighter: pygments\n\n[source,python]\n----\nprint('hi')\n----");
    assert!(html.contains("<pre class=\"pygments highlight\">"), "pygments: pre class. Got: {html}");
    assert!(html.contains("data-lang=\"python\""), "pygments: data-lang. Got: {html}");
    assert!(!html.contains("class=\"language-python\""), "pygments: no language- class. Got: {html}");
}

#[test]
fn test_source_block_coderay() {
    let html = to_html(":source-highlighter: coderay\n\n[source,java]\n----\nSystem.out.println();\n----");
    assert!(html.contains("<pre class=\"CodeRay highlight\">"), "coderay: pre class. Got: {html}");
    assert!(html.contains("data-lang=\"java\""), "coderay: data-lang. Got: {html}");
}

#[test]
fn test_source_highlight_single_line() {
    let html = to_html("[source,rust,highlight=2]\n----\nlet a = 1;\nlet b = 2;\nlet c = 3;\n----");
    assert!(html.contains("let a = 1;\n<span class=\"hll\">let b = 2;</span>\nlet c = 3;"), "single line highlight. Got: {html}");
}

#[test]
fn test_source_highlight_multiple_lines() {
    let html = to_html("[source,rust,highlight=1;3]\n----\nlet a = 1;\nlet b = 2;\nlet c = 3;\n----");
    assert!(html.contains("<span class=\"hll\">let a = 1;</span>\nlet b = 2;\n<span class=\"hll\">let c = 3;</span>"), "multiple lines highlight. Got: {html}");
}

#[test]
fn test_source_highlight_range() {
    let html = to_html("[source,rust,highlight=2..4]\n----\nline 1\nline 2\nline 3\nline 4\nline 5\n----");
    assert!(html.contains("line 1\n<span class=\"hll\">line 2</span>\n<span class=\"hll\">line 3</span>\n<span class=\"hll\">line 4</span>\nline 5"), "range highlight. Got: {html}");
}

#[test]
fn test_source_no_highlight_no_change() {
    let html = to_html("[source,rust]\n----\nlet a = 1;\nlet b = 2;\n----");
    assert!(!html.contains("hll"), "no highlight attr should produce no hll. Got: {html}");
}

#[test]
fn test_source_highlight_last_line() {
    let html = to_html("[source,rust,highlight=3]\n----\nline 1\nline 2\nline 3\n----");
    assert!(html.contains("<span class=\"hll\">line 3</span></code>"), "last line highlight should close span before </code>. Got: {html}");
}

#[test]
fn test_source_highlight_comma_separator() {
    let html = to_html("[source,rust,highlight=\"1,3\"]\n----\nline 1\nline 2\nline 3\n----");
    assert!(html.contains("<span class=\"hll\">line 1</span>"), "comma-separated highlight. Got: {html}");
    assert!(html.contains("<span class=\"hll\">line 3</span>"), "comma-separated highlight. Got: {html}");
}

#[test]
fn test_idprefix_idseparator() {
    // Default: prefix=_ separator=_
    let html = to_html("== My Section\n\nContent.");
    assert!(html.contains("id=\"_my_section\""), "default id. Got: {html}");

    // Empty prefix + dash separator
    let html = to_html(":idprefix:\n:idseparator: -\n\n== My Section\n\nContent.");
    assert!(html.contains("id=\"my-section\""), "custom id. Got: {html}");

    // Custom prefix
    let html = to_html(":idprefix: sec-\n\n== My Section\n\nContent.");
    assert!(html.contains("id=\"sec-my_section\""), "custom prefix. Got: {html}");
}

#[test]
fn test_natural_cross_reference() {
    // `<<Title>>` matching a section title (even a forward reference)
    // resolves to that section's auto-generated id, like Asciidoctor.
    let html = to_html("See <<Substitutions>>.\n\n== Substitutions\n\nx");
    assert!(html.contains("href=\"#_substitutions\""), "natural ref. Got: {html}");

    // A target that matches no section title stays literal.
    let html = to_html("See <<Foo Bar>>.\n\n== Other\n\nx");
    assert!(html.contains("href=\"#Foo Bar\""), "unmatched ref literal. Got: {html}");

    // Title match resolves to the section's explicit id when present.
    let html = to_html("See <<Substitutions>>.\n\n[#myid]\n== Substitutions\n\nx");
    assert!(html.contains("href=\"#myid\""), "explicit id. Got: {html}");

    // Matching is case-sensitive: lowercase target does not match the title.
    let html = to_html("See <<substitutions>>.\n\n== Substitutions\n\nx");
    assert!(html.contains("href=\"#substitutions\""), "case-sensitive. Got: {html}");

    // An explicit-text xref still resolves its href via the title.
    let html = to_html("See <<Substitutions,go here>>.\n\n== Substitutions\n\nx");
    assert!(html.contains("href=\"#_substitutions\""), "labeled href. Got: {html}");
}

#[test]
fn test_builtin_attr_backend() {
    let html = to_html("{backend}");
    assert!(html.contains("html5"), "backend should be html5. Got: {html}");
}

#[test]
fn test_intrinsic_char_replacement_attrs() {
    // quot/apos/pp are character-replacement attributes Asciidoctor resolves
    // (to &#34;/&#39;/&#43;&#43;), including inside monospace spans.
    let html = to_html("{quot} {apos} {pp} `{quot}` `{apos}`");
    assert!(html.contains("&#34;"), "quot should resolve to &#34;. Got: {html}");
    assert!(html.contains("&#39;"), "apos should resolve to &#39;. Got: {html}");
    assert!(html.contains("&#43;&#43;"), "pp should resolve to ++. Got: {html}");
    assert!(!html.contains("{quot}") && !html.contains("{apos}") && !html.contains("{pp}"),
        "no unresolved references should remain. Got: {html}");
    assert!(html.contains("<code>&#34;</code>"), "quot inside monospace. Got: {html}");
}

#[test]
fn test_builtin_attr_doctype() {
    let html = to_html("{doctype}");
    assert!(html.contains("article"), "doctype should be article. Got: {html}");
}

#[test]
fn test_builtin_attr_doctype_override() {
    let html = to_html(":doctype: book\n\n{doctype}");
    assert!(html.contains("book"), "doctype should be overridden to book. Got: {html}");
    assert!(!html.contains("article"), "should not contain default article. Got: {html}");
}

#[test]
fn test_body_doctype_ignored() {
    let html = to_html_with_options(
        "= Article Title\n\n== Section\n\n:doctype: book\n\ntext",
        HtmlOptions { standalone: true, ..Default::default() },
    );
    assert!(html.contains("<body class=\"article\">"), "body doctype should be ignored. Got: {html}");
}

#[test]
fn test_builtin_attr_author() {
    let html = to_html("= Title\nJohn Doe <john@example.com>\n\n{author} {firstname} {lastname} {authorinitials} {email}");
    assert!(html.contains("John Doe"), "author. Got: {html}");
    assert!(html.contains("John"), "firstname. Got: {html}");
    assert!(html.contains("Doe"), "lastname. Got: {html}");
    assert!(html.contains("JD"), "authorinitials. Got: {html}");
    assert!(html.contains("john@example.com"), "email. Got: {html}");
}

#[test]
fn test_multi_author_attr_names_underscore() {
    // Attribute names for authors 2+ use an underscore (`{author_2}`), while
    // the detail-span HTML ids stay separator-less (`id="author2"`).
    let html = to_html_with_options(
        "= Title\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>\n\n{author_2} / {lastname_2} / {firstname_3} / {authorinitials_3} / {email_3}\n\n{author2} stays literal",
        HtmlOptions { standalone: true, ..Default::default() },
    );
    assert!(
        html.contains("B. Steppenwolf / Steppenwolf / Pax / PD / "),
        "underscore attr refs should resolve. Got: {html}"
    );
    assert!(html.contains("mailto:pax@asciidoctor.org"), "email_3. Got: {html}");
    assert!(html.contains("{author2} stays literal"), "no-underscore form is not an attribute. Got: {html}");
    assert!(html.contains("<span id=\"author2\" class=\"author\">B. Steppenwolf</span>"), "span id without underscore. Got: {html}");
    assert!(html.contains("<span id=\"email3\" class=\"email\">"), "email span id without underscore. Got: {html}");
}

#[test]
fn test_builtin_attr_revision() {
    let html = to_html("= Title\nAuthor Name\nv1.0, 2024-01-01: Initial\n\n{revnumber} {revdate} {revremark}");
    // Asciidoctor strips the leading `v` from the revision number, so
    // `{revnumber}` resolves to `1.0` (not `v1.0`).
    assert!(html.contains("1.0"), "revnumber. Got: {html}");
    assert!(!html.contains("v1.0"), "v-prefix should be stripped. Got: {html}");
    assert!(html.contains("2024-01-01"), "revdate. Got: {html}");
    assert!(html.contains("Initial"), "revremark. Got: {html}");
}

#[test]
fn test_revnumber_version_label() {
    let opts = HtmlOptions { standalone: true, ..Default::default() };
    // Default label, no revdate: lowercase `version`, NO trailing comma
    let html = to_html_with_options("= T\nA U\nv3\n\nbody", opts.clone());
    assert!(html.contains("<span id=\"revnumber\">version 3</span>"), "Got: {html}");
    // With revdate: comma re-appears
    let html = to_html_with_options("= T\nA U\nv3, 2024-01-02\n\nbody", opts.clone());
    assert!(html.contains("<span id=\"revnumber\">version 3,</span>"), "Got: {html}");
    // Custom `:version-label:` is downcased
    let html = to_html_with_options("= T\nA U\nv3: remark\n:version-label: Edition\n\nbody", opts.clone());
    assert!(html.contains("<span id=\"revnumber\">edition 3</span>"), "Got: {html}");
    // Unset label keeps the leading space (Asciidoctor template artifact)
    let html = to_html_with_options("= T\nA U\nv3\n:!version-label:\n\nbody", opts);
    assert!(html.contains("<span id=\"revnumber\"> 3</span>"), "Got: {html}");
}

#[test]
fn test_paragraph_hardbreaks_option() {
    // `[%hardbreaks]` turns every soft line break into a hard break.
    let html = to_html("[%hardbreaks]\nLine one\nLine two\nLine three");
    assert!(
        html.contains("Line one<br>\nLine two<br>\nLine three"),
        "hardbreaks. Got: {html}"
    );
    // A plain paragraph still joins lines with a bare newline (regression guard).
    let plain = to_html("Line one\nLine two");
    assert!(!plain.contains("<br>"), "no hardbreaks. Got: {plain}");
    // The `hardbreaks-option` document attribute applies to every paragraph.
    let doc = to_html(":hardbreaks-option:\n\nLine one\nLine two");
    assert!(doc.contains("Line one<br>\nLine two"), "doc-attr hardbreaks. Got: {doc}");
}

#[test]
fn test_builtin_attr_doctitle() {
    // In embedded mode, document header is not rendered, so doctitle only appears in the body reference
    let html = to_html("= My Title\n\n{doctitle}");
    assert!(html.contains("My Title"), "doctitle should resolve in body. Got: {html}");
    // In standalone mode, it appears in both header and body
    let html = to_html_with_options("= My Title\n\n{doctitle}", HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("<h1>My Title</h1>"), "standalone should have h1. Got: {html}");
    assert!(html.contains("<p>My Title</p>"), "doctitle should resolve in body. Got: {html}");
}

#[test]
fn test_attr_fallback() {
    let html = to_html("{undefined!fallback value}");
    assert!(html.contains("fallback value"), "fallback should be used when attr undefined. Got: {html}");
    assert!(!html.contains("{undefined}"), "should not show raw reference. Got: {html}");
}

#[test]
fn test_attr_fallback_not_used() {
    let html = to_html(":name: real\n\n{name!fallback}");
    assert!(html.contains("real"), "defined attr should be used. Got: {html}");
    assert!(!html.contains("fallback"), "fallback should not be used when attr defined. Got: {html}");
}

#[test]
fn test_attr_fallback_empty() {
    let html = to_html("{undefined!}");
    assert!(!html.contains("{undefined}"), "should not show raw reference. Got: {html}");
    // Empty fallback means nothing is rendered for the attribute
    assert!(!html.contains("undefined"), "empty fallback should render nothing for the attr. Got: {html}");
}

// --- Markdown compatibility tests ---

#[test]
fn test_markdown_heading_h2() {
    let html = to_html("## Title\n\nContent.");
    assert!(html.contains("id=\"_title\""), "should generate id. Got: {html}");
    assert!(html.contains("<h2"), "should render h2. Got: {html}");
    assert!(html.contains("Title"), "should contain title text. Got: {html}");
}

#[test]
fn test_markdown_heading_h3() {
    let html = to_html("### Level Three\n\nContent.");
    assert!(html.contains("id=\"_level_three\""), "should generate id. Got: {html}");
    assert!(html.contains("<h3"), "should render h3. Got: {html}");
}

#[test]
fn test_markdown_heading_document_title() {
    // In embedded mode, document header (h1) is suppressed
    let html = to_html("# Doc Title\n\nBody text.");
    assert!(html.contains("Body text"), "should contain body. Got: {html}");
    // In standalone mode, h1 is rendered
    let html = to_html_with_options("# Doc Title\n\nBody text.", HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("Doc Title"), "should contain title. Got: {html}");
    assert!(html.contains("<h1"), "document title should render h1. Got: {html}");
}

#[test]
fn test_markdown_heading_mixed_with_asciidoc() {
    let html = to_html("= Doc Title\n\n== AsciiDoc Section\n\nPara 1.\n\n### Markdown Section\n\nPara 2.");
    assert!(html.contains("<h2"), "should have h2 for asciidoc section. Got: {html}");
    assert!(html.contains("<h3"), "should have h3 for markdown section. Got: {html}");
    assert!(html.contains("AsciiDoc Section"), "asciidoc heading. Got: {html}");
    assert!(html.contains("Markdown Section"), "markdown heading. Got: {html}");
}

#[test]
fn test_markdown_code_fence_with_language() {
    let html = to_html("```rust\nfn main() {}\n```");
    assert!(html.contains("class=\"language-rust\""), "should have language class. Got: {html}");
    assert!(html.contains("fn main() {}"), "should contain code. Got: {html}");
    assert!(html.contains("<code"), "should have <code> tag. Got: {html}");
    assert!(html.contains("listingblock"), "should have listingblock class. Got: {html}");
}

#[test]
fn test_markdown_code_fence_without_language() {
    let html = to_html("```\nsome code\n```");
    assert!(html.contains("some code"), "should contain code. Got: {html}");
    assert!(html.contains("listingblock"), "should have listingblock class. Got: {html}");
    assert!(html.contains("<pre>"), "should have <pre> tag. Got: {html}");
    assert!(!html.contains("<code"), "listing block should not have <code> tag. Got: {html}");
}

#[test]
fn test_markdown_code_fence_4_backticks() {
    let html = to_html("````python\nprint('hi')\n````");
    assert!(html.contains("class=\"language-python\""), "should have python language. Got: {html}");
    assert!(html.contains("print('hi')"), "should contain code. Got: {html}");
}

#[test]
fn test_markdown_code_fence_nested() {
    // 4-backtick fence can contain 3-backtick fences
    let html = to_html("````\n```\ninner\n```\n````");
    assert!(html.contains("```"), "inner backticks should be verbatim. Got: {html}");
    assert!(html.contains("inner"), "should contain inner text. Got: {html}");
}

#[test]
fn test_markdown_code_fence_unclosed() {
    let html = to_html("```rust\nunclosed code");
    assert!(html.contains("unclosed code"), "should contain code even if unclosed. Got: {html}");
    assert!(html.contains("class=\"language-rust\""), "should still have language. Got: {html}");
}

#[test]
fn test_markdown_code_fence_with_highlighter() {
    let html = to_html(":source-highlighter: highlight.js\n\n```rust\nfn main() {}\n```");
    assert!(html.contains("highlightjs highlight"), "should use highlighter. Got: {html}");
    assert!(html.contains("data-lang=\"rust\""), "should have data-lang. Got: {html}");
    assert!(html.contains("class=\"hljs language-rust\""), "should have hljs + language class. Got: {html}");
}

#[test]
fn test_markdown_code_fence_with_title() {
    let html = to_html(".My Code\n```rust\nfn main() {}\n```");
    assert!(html.contains("My Code"), "should contain block title. Got: {html}");
    assert!(html.contains("class=\"language-rust\""), "should have language class. Got: {html}");
}

// === Block style attributes (4.13) ===

#[test]
fn test_listing_style_on_paragraph() {
    let html = to_html("[listing]\nsome code here");
    assert!(html.contains("listingblock"), "should have listingblock class. Got: {html}");
    assert!(html.contains("<pre>"), "should have <pre>. Got: {html}");
    assert!(html.contains("some code here"), "should contain text. Got: {html}");
    assert!(!html.contains("<p>"), "should NOT have <p>. Got: {html}");
}

#[test]
fn test_source_style_on_paragraph() {
    let html = to_html("[source,rust]\nfn main() {}");
    assert!(html.contains("language-rust"), "should have language-rust. Got: {html}");
    assert!(html.contains("fn main()"), "should contain code. Got: {html}");
    assert!(!html.contains("<p>"), "should NOT have <p>. Got: {html}");
}

#[test]
fn test_verse_style_on_paragraph() {
    let html = to_html("[verse]\nline one\nline two");
    assert!(html.contains("verseblock"), "should have verseblock class. Got: {html}");
    assert!(html.contains("<pre class=\"content\">"), "should have verse pre. Got: {html}");
    assert!(html.contains("line one"), "should contain text. Got: {html}");
}

#[test]
fn test_quote_style_on_paragraph() {
    let html = to_html("[quote]\nThis is a quote.");
    assert!(html.contains("quoteblock"), "should have quoteblock class. Got: {html}");
    assert!(html.contains("<blockquote>"), "should have blockquote. Got: {html}");
    assert!(html.contains("This is a quote."), "should contain text. Got: {html}");
}

#[test]
fn test_sidebar_style_on_paragraph() {
    let html = to_html("[sidebar]\nSidebar content.");
    assert!(html.contains("sidebarblock"), "should have sidebarblock class. Got: {html}");
    assert!(html.contains("Sidebar content."), "should contain text. Got: {html}");
}

#[test]
fn test_example_style_on_paragraph() {
    let html = to_html("[example]\nExample content.");
    assert!(html.contains("exampleblock"), "should have exampleblock class. Got: {html}");
    assert!(html.contains("Example content."), "should contain text. Got: {html}");
}

#[test]
fn test_listing_style_on_open_block() {
    let html = to_html("[listing]\n--\ncode inside open\n--");
    assert!(html.contains("listingblock"), "should have listingblock class. Got: {html}");
    assert!(html.contains("<pre>"), "should have <pre>. Got: {html}");
    assert!(html.contains("code inside open"), "should contain text. Got: {html}");
}

#[test]
fn test_source_style_on_open_block() {
    let html = to_html("[source,py]\n--\nprint('hello')\n--");
    assert!(html.contains("language-py"), "should have language-py. Got: {html}");
    assert!(html.contains("print("), "should contain code. Got: {html}");
}

#[test]
fn test_quote_style_on_open_block() {
    let html = to_html("[quote]\n--\nQuoted text.\n--");
    assert!(html.contains("quoteblock"), "should have quoteblock class. Got: {html}");
    assert!(html.contains("<blockquote>"), "should have blockquote. Got: {html}");
    assert!(html.contains("Quoted text."), "should contain text. Got: {html}");
}

#[test]
fn test_note_style_on_open_block() {
    let html = to_html("[NOTE]\n--\nNote content.\n--");
    assert!(html.contains("admonitionblock note"), "should have admonition. Got: {html}");
    assert!(html.contains("Note content."), "should contain text. Got: {html}");
}

// --- Universal style remapping on non-native delimiters ---

#[test]
fn test_source_style_on_example_delimiter() {
    let html = to_html("[source,rust]\n====\nfn main() {}\n====");
    assert!(html.contains("language-rust"), "should have language-rust. Got: {html}");
    assert!(html.contains("fn main()"), "should contain code. Got: {html}");
    assert!(!html.contains("exampleblock"), "should NOT have exampleblock. Got: {html}");
}

#[test]
fn test_listing_style_on_example_delimiter() {
    let html = to_html("[listing]\n====\ncode here\n====");
    assert!(html.contains("listingblock"), "should have listingblock. Got: {html}");
    assert!(html.contains("<pre>"), "should have <pre>. Got: {html}");
    assert!(html.contains("code here"), "should contain text. Got: {html}");
    assert!(!html.contains("exampleblock"), "should NOT have exampleblock. Got: {html}");
}

#[test]
fn test_quote_style_on_listing_delimiter() {
    let html = to_html("[quote]\n----\nQuoted text.\n----");
    assert!(html.contains("quoteblock"), "should have quoteblock. Got: {html}");
    assert!(html.contains("<blockquote>"), "should have blockquote. Got: {html}");
    assert!(html.contains("Quoted text."), "should contain text. Got: {html}");
    assert!(!html.contains("listingblock"), "should NOT have listingblock. Got: {html}");
}

#[test]
fn test_verse_style_on_listing_delimiter() {
    let html = to_html("[verse]\n----\nVerse line one\nVerse line two\n----");
    assert!(html.contains("verseblock"), "should have verseblock. Got: {html}");
    assert!(html.contains("Verse line one"), "should contain text. Got: {html}");
    assert!(!html.contains("listingblock"), "should NOT have listingblock. Got: {html}");
}

#[test]
fn test_note_style_on_listing_delimiter() {
    // Admonition style is only honored on example/open delimiters; on a listing
    // (and literal/sidebar/quote) asciidoctor ignores it and keeps the native block.
    let html = to_html("[NOTE]\n----\nNote content.\n----");
    assert!(html.contains("<div class=\"listingblock\">"), "should stay a listingblock. Got: {html}");
    assert!(html.contains("Note content."), "should contain text. Got: {html}");
    assert!(!html.contains("admonitionblock"), "should NOT become admonition. Got: {html}");
}

#[test]
fn test_sidebar_style_on_example_delimiter() {
    let html = to_html("[sidebar]\n====\nSidebar content.\n====");
    assert!(html.contains("sidebarblock"), "should have sidebarblock. Got: {html}");
    assert!(html.contains("Sidebar content."), "should contain text. Got: {html}");
    assert!(!html.contains("exampleblock"), "should NOT have exampleblock. Got: {html}");
}

// === Nested delimited blocks (4.12) ===

#[test]
fn test_nested_example_blocks_different_lengths() {
    let html = to_html("======\nOuter\n====\nInner\n====\nAfter inner\n======");
    // Should have two exampleblocks
    assert_eq!(html.matches("<div class=\"exampleblock\">").count(), 2,
        "should have two example blocks. Got: {html}");
    assert!(html.contains("Outer"), "should contain outer text. Got: {html}");
    assert!(html.contains("Inner"), "should contain inner text. Got: {html}");
    assert!(html.contains("After inner"), "should contain text after inner. Got: {html}");
}

#[test]
fn test_nested_quote_inside_example() {
    let html = to_html("====\nBefore\n____\nQuote text\n____\nAfter\n====");
    assert!(html.contains("<div class=\"exampleblock\">"),
        "should have example block. Got: {html}");
    assert!(html.contains("<div class=\"quoteblock\">"),
        "should have quote block. Got: {html}");
    assert!(html.contains("Quote text"), "should contain quote text. Got: {html}");
}

#[test]
fn test_listing_inside_sidebar() {
    let html = to_html("****\nBefore\n----\ncode here\n----\nAfter\n****");
    assert!(html.contains("<div class=\"sidebarblock\">"),
        "should have sidebar block. Got: {html}");
    assert!(html.contains("<div class=\"listingblock\">"),
        "should have listing block. Got: {html}");
    assert!(html.contains("code here"), "should contain code. Got: {html}");
    assert!(html.contains("After"), "should contain text after listing. Got: {html}");
}

#[test]
fn test_open_block_inside_example() {
    let html = to_html("====\nBefore\n--\nOpen content\n--\nAfter\n====");
    assert!(html.contains("<div class=\"exampleblock\">"),
        "should have example block. Got: {html}");
    assert!(html.contains("<div class=\"openblock\">"),
        "should have open block. Got: {html}");
    assert!(html.contains("Open content"), "should contain open block text. Got: {html}");
}

#[test]
fn test_unclosed_listing_inside_example_parent_delimiter_wins() {
    // Listing block is not closed, but parent example delimiter should take priority
    let html = to_html("====\nBefore\n----\ncode here\n====");
    assert!(html.contains("<div class=\"exampleblock\">"),
        "should have example block. Got: {html}");
    assert!(html.contains("code here"), "should contain code. Got: {html}");
    // The example block should be properly closed
    assert!(html.contains("Before"), "should contain text before listing. Got: {html}");
}

#[test]
fn test_three_level_nesting() {
    let html = to_html("======\nL1\n=====\nL2\n====\nL3\n====\nL2 after\n=====\nL1 after\n======");
    assert_eq!(html.matches("<div class=\"exampleblock\">").count(), 3,
        "should have three example blocks. Got: {html}");
    assert!(html.contains("L1"), "should contain L1 text. Got: {html}");
    assert!(html.contains("L2"), "should contain L2 text. Got: {html}");
    assert!(html.contains("L3"), "should contain L3 text. Got: {html}");
}

#[test]
fn test_source_block_inside_sidebar() {
    let html = to_html("****\n[source,rust]\n----\nfn main() {}\n----\n****");
    assert!(html.contains("<div class=\"sidebarblock\">"),
        "should have sidebar block. Got: {html}");
    assert!(html.contains("<code"), "should have code element. Got: {html}");
    assert!(html.contains("fn main() {}"), "should contain source code. Got: {html}");
}

#[test]
fn test_env_attribute_existing_var() {
    // PATH is set on all platforms
    let html = to_html("Value: {env-PATH}");
    assert!(!html.contains("{env-PATH}"), "env-PATH should be resolved, not literal. Got: {html}");
    assert!(html.contains("Value: "), "should contain prefix. Got: {html}");
}

#[test]
fn test_env_attribute_missing_var() {
    let html = to_html("Value: {env-ADOC_PARSER_TEST_VAR_12345}");
    assert!(html.contains("{env-ADOC_PARSER_TEST_VAR_12345}"),
        "missing env var should render as literal. Got: {html}");
}

#[test]
fn test_env_attribute_missing_var_with_fallback() {
    let html = to_html("Value: {env-ADOC_PARSER_TEST_VAR_12345!fallback}");
    assert!(html.contains("Value: fallback"),
        "missing env var with fallback should use fallback. Got: {html}");
}

#[test]
fn test_custom_inline_macro_with_attrs() {
    let html = to_html("chart:sales[Q1,Q2]");
    assert!(html.contains("<span class=\"custom-macro macro-chart\">Q1,Q2</span>"),
        "custom inline macro should render. Got: {html}");
}

#[test]
fn test_custom_block_macro_with_attrs() {
    let html = to_html("chart::sales-data[type=bar]");
    assert!(html.contains("<div class=\"custom-macro macro-chart\">"),
        "custom block macro should render div. Got: {html}");
    assert!(html.contains("type=bar"),
        "custom block macro should contain attrs text. Got: {html}");
    assert!(html.contains("</div>"),
        "custom block macro should close div. Got: {html}");
}

#[test]
fn test_custom_inline_macro_empty_attrs() {
    let html = to_html("widget:component[]");
    assert!(html.contains("<span class=\"custom-macro macro-widget\"></span>"),
        "custom inline macro with empty attrs should render empty span. Got: {html}");
}

#[test]
fn test_kbd_not_captured_as_custom() {
    // With :experimental:, kbd: is a built-in macro — never a custom inline macro.
    let html = to_html(":experimental:\n\nkbd:[Ctrl+S]");
    assert!(html.contains("<kbd>"),
        "kbd should remain a built-in macro, not custom. Got: {html}");
    assert!(!html.contains("custom-macro"),
        "kbd should not be treated as custom macro. Got: {html}");
    // Without :experimental:, kbd: is literal text — still never a custom macro.
    let literal = to_html("kbd:[Ctrl+S]");
    assert!(literal.contains("kbd:[Ctrl+S]"),
        "disabled kbd should remain literal. Got: {literal}");
    assert!(!literal.contains("custom-macro"),
        "disabled kbd should not be treated as custom macro. Got: {literal}");
}

#[test]
fn test_block_image_not_captured_as_custom() {
    let html = to_html("image::photo.jpg[alt]");
    assert!(html.contains("<img"),
        "image:: should remain a built-in block image. Got: {html}");
    assert!(!html.contains("custom-macro"),
        "image:: should not be treated as custom macro. Got: {html}");
}

#[test]
fn test_custom_macro_with_hyphen_underscore_name() {
    let html = to_html("my-custom_macro:target[attrs]");
    assert!(html.contains("<span class=\"custom-macro macro-my-custom_macro\">attrs</span>"),
        "macro names with hyphen/underscore should work. Got: {html}");
}

#[test]
fn test_docinfo_head() {
    let html = to_html_with_options("Hello world", HtmlOptions {
        docinfo_head: Some("<meta name=\"test\" content=\"value\">".to_string()),
        ..Default::default()
    });
    assert!(html.starts_with("<meta name=\"test\" content=\"value\">\n"),
        "docinfo head should be prepended. Got: {html}");
    assert!(html.contains("<p>Hello world</p>"),
        "content should follow head. Got: {html}");
}

#[test]
fn test_docinfo_footer() {
    let html = to_html_with_options("Hello world", HtmlOptions {
        docinfo_footer: Some("<script src=\"app.js\"></script>".to_string()),
        ..Default::default()
    });
    assert!(html.ends_with("\n<script src=\"app.js\"></script>"),
        "docinfo footer should be appended. Got: {html}");
    assert!(html.contains("<p>Hello world</p>"),
        "content should precede footer. Got: {html}");
}

#[test]
fn test_docinfo_head_and_footer() {
    let html = to_html_with_options("Hello world", HtmlOptions {
        docinfo_head: Some("<meta name=\"x\">".to_string()),
        docinfo_footer: Some("<script></script>".to_string()),
        ..Default::default()
    });
    assert!(html.starts_with("<meta name=\"x\">\n"),
        "head should be first. Got: {html}");
    assert!(html.ends_with("\n<script></script>"),
        "footer should be last. Got: {html}");
}

#[test]
fn test_docinfo_default_options_same_as_to_html() {
    let input = "= Title\n\nHello world";
    let html_default = to_html(input);
    let html_options = to_html_with_options(input, HtmlOptions::default());
    assert_eq!(html_default, html_options,
        "default options should produce identical output");
}

#[test]
fn test_docinfo_head_before_toc() {
    let input = "= Title\n:toc:\n\n== Section 1\n\nContent";
    let html = to_html_with_options(input, HtmlOptions {
        docinfo_head: Some("<meta name=\"toc-test\">".to_string()),
        ..Default::default()
    });
    let head_pos = html.find("<meta name=\"toc-test\">").unwrap();
    let toc_pos = html.find("<div id=\"toc\"").unwrap();
    assert!(head_pos < toc_pos,
        "head should appear before TOC. Got: {html}");
}

#[test]
fn test_docinfo_empty_content_no_extra_newlines() {
    let input = "Hello world";
    let html_empty = to_html_with_options(input, HtmlOptions {
        docinfo_head: Some(String::new()),
        docinfo_footer: Some(String::new()),
        ..Default::default()
    });
    let html_none = to_html(input);
    assert_eq!(html_empty, html_none,
        "empty docinfo should not add extra content");
}

#[test]
fn test_manpage_title_suffix() {
    let input = "= command(1)\n:doctype: manpage\n\n== SYNOPSIS\n\ntext";
    // In standalone mode, h1 is rendered with manpage suffix
    let html = to_html_with_options(input, HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("command(1) Manual Page</h1>"),
        "manpage title should have ' Manual Page' suffix. Got: {html}");
}

#[test]
fn test_manpage_auto_attrs() {
    let input = "= command(1)\n:doctype: manpage\n\nmanvol={manvolnum} mantitle={mantitle}";
    let html = to_html(input);
    assert!(html.contains("manvol=1"), "manvolnum should be '1'. Got: {html}");
    assert!(html.contains("mantitle=command"), "mantitle should be 'command'. Got: {html}");
}

#[test]
fn test_manpage_name_extraction() {
    let input = "= command(1)\n:doctype: manpage\n\n== NAME\n\nmycmd - manage things\n\n== SYNOPSIS\n\nname={manname} purpose={manpurpose}";
    let html = to_html(input);
    assert!(html.contains("name=mycmd"), "manname should be 'mycmd'. Got: {html}");
    assert!(html.contains("purpose=manage things"), "manpurpose should be 'manage things'. Got: {html}");
}

#[test]
fn test_no_manpage_suffix_for_article() {
    let input = "= Title\n\ntext";
    // In standalone mode, verify article title doesn't get manpage suffix
    let html = to_html_with_options(input, HtmlOptions { standalone: true, ..Default::default() });
    assert!(html.contains("<h1>Title</h1>"),
        "article title should not have ' Manual Page'. Got: {html}");
    assert!(!html.contains("Manual Page"),
        "article should not contain 'Manual Page'. Got: {html}");
}

#[test]
fn test_manpage_doctype_attr_ref() {
    let input = "= command(1)\n:doctype: manpage\n\ntype={doctype}";
    let html = to_html(input);
    assert!(html.contains("type=manpage"), "doctype should be 'manpage'. Got: {html}");
}

#[test]
fn test_book_part_rendering() {
    let input = "= Book Title\n:doctype: book\n\n= Part One\n\npart intro\n\n== Chapter 1\n\ntext";
    let html = to_html(input);
    assert!(html.contains("class=\"sect0\""), "part title should have class=\"sect0\". Got: {html}");
    assert!(html.contains("<h1 id=\"_part_one\" class=\"sect0\">Part One</h1>"),
        "part should render as <h1> with sect0 class. Got: {html}");
    // Part should NOT be wrapped in <div class="sect1">
    assert!(!html.contains("<div class=\"sect1\">\n<h1 id=\"_part_one\""),
        "part should not have div wrapper. Got: {html}");
}

#[test]
fn test_book_chapter_rendering() {
    let input = "= Book Title\n:doctype: book\n\n= Part One\n\n== Chapter 1\n\ntext";
    let html = to_html(input);
    assert!(html.contains("<div class=\"sect1\">"), "chapter should have div wrapper. Got: {html}");
    assert!(html.contains("<h2 id=\"_chapter_1\">Chapter 1</h2>"),
        "chapter should render as <h2>. Got: {html}");
}

#[test]
fn test_article_no_part_behavior() {
    let input = "= Title\n\n== Section\n\ntext";
    let html = to_html(input);
    assert!(html.contains("<div class=\"sect1\">"), "article sections should have div wrapper. Got: {html}");
}

#[test]
fn test_book_special_section_not_part() {
    let input = "= Book Title\n:doctype: book\n\n[appendix]\n= Appendix A\n\ntext";
    let html = to_html(input);
    // Special section styles should NOT appear as CSS classes
    assert!(!html.contains("class=\"sect0 appendix\""),
        "appendix style should not be in CSS class. Got: {html}");
    assert!(html.contains("Appendix A:"),
        "appendix should have caption. Got: {html}");
    // TODO: Asciidoctor treats level-1 special sections in book as sect1/h2,
    // not sect0/h1. Fix in #9 (doctype=book handling).
}

#[test]
fn test_book_multiple_parts() {
    let input = "= Book Title\n:doctype: book\n\n= Part 1\n\n== Ch1\n\ntext1\n\n= Part 2\n\n== Ch2\n\ntext2";
    let html = to_html(input);
    assert!(html.contains("<h1 id=\"_part_1\" class=\"sect0\">Part 1</h1>"),
        "first part should have sect0. Got: {html}");
    assert!(html.contains("<h1 id=\"_part_2\" class=\"sect0\">Part 2</h1>"),
        "second part should have sect0. Got: {html}");
}

#[test]
fn test_book_doctype_attr_ref() {
    let input = "= Book Title\n:doctype: book\n\ntype={doctype}";
    let html = to_html(input);
    assert!(html.contains("type=book"), "doctype should be 'book'. Got: {html}");
}

#[test]
fn test_paragraph_trailing_whitespace_stripped_before_softbreak() {
    // Asciidoctor rstrips every source line: trailing spaces/tabs before a
    // soft line break are dropped. A leading-space line is preserved, and a
    // trailing ` +` hard break still renders <br>.
    let html = to_html("First line.  \nSecond\twith tab\t\nThird.");
    assert!(html.contains("First line.\nSecond\twith tab\nThird."),
        "trailing ws before \\n must be stripped. Got: {html:?}");
    assert!(!html.contains("First line.  \n") && !html.contains("tab\t\n"),
        "no trailing ws should survive before \\n. Got: {html:?}");

    let hb = to_html("alpha +\nbeta.");
    assert!(hb.contains("alpha<br>\nbeta."), "hard break preserved. Got: {hb:?}");
}

#[test]
fn test_listing_block_trailing_whitespace_stripped() {
    // Verbatim blocks arrive as separate Text + SoftBreak events; trailing
    // whitespace before each interior line break is stripped too.
    let html = to_html("----\nfirst.  \nsecond.\n----");
    assert!(html.contains("first.\nsecond."),
        "listing interior line ws must be stripped. Got: {html:?}");
}
