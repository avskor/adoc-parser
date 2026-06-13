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
fn test_section_marker_does_not_interrupt_paragraph() {
    // Asciidoctor's read_paragraph_lines (StartOfBlockProc) breaks a paragraph
    // only on a block delimiter or block-attribute line — never a section
    // title. A `==`/`====` line that appears as a paragraph continuation line
    // (no preceding blank line) is therefore plain text, NOT a new section.

    // Mid-paragraph `== Heading` is absorbed as text.
    let html = to_html("para line one\n== Heading no blank\nmore text");
    assert!(
        html.contains("<p>para line one\n== Heading no blank\nmore text</p>"),
        "section marker should not split the paragraph: {html}"
    );
    assert!(!html.contains("<h2"), "no section should be emitted: {html}");

    // admonition.adoc `bl-c` shape: `[IMPORTANT] <.>` is not a block attribute
    // (no trailing `]`), so it opens a paragraph; the `==== <.>` continuation
    // line must stay inside it rather than becoming a level-3 section.
    let html = to_html("[IMPORTANT] <.>\n.Feeding\n==== <.>\nbody text");
    assert!(
        html.contains(
            "<p>[IMPORTANT] &lt;.&gt;\n.Feeding\n==== &lt;.&gt;\nbody text</p>"
        ),
        "==== continuation line must stay in the paragraph: {html}"
    );
    assert!(!html.contains("class=\"sect3\""), "no section: {html}");

    // Negative: a section marker AFTER a blank line still starts a section
    // (recognized at the block boundary by the dispatcher).
    let html = to_html("first para\n\n== Real Section\n\nbody");
    assert!(html.contains("<h2 id=\"_real_section\">Real Section</h2>"));
    assert!(html.contains("<p>first para</p>"));
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
fn test_mixed_marker_list_nesting() {
    // Probe-verified vs asciidoctor (/tmp/p_subs/p6, p8): a marker that
    // doesn't match any OPEN list nests inside the current item; a marker
    // matching an open (ancestor) list closes back up to it.
    // `. Linux` + `* Fedora` → ulist nested in the olist <li>
    let html = to_html(". Linux\n* Fedora\n* Ubuntu\n. BSD\n* FreeBSD");
    assert!(
        html.contains("<p>Linux</p>\n<div class=\"ulist\">\n<ul>\n<li>\n<p>Fedora</p>"),
        "ulist must nest inside the olist item. Got:\n{html}"
    );
    assert!(
        html.contains("</ul>\n</div>\n</li>\n<li>\n<p>BSD</p>"),
        ".BSD returns to the parent olist as a sibling item. Got:\n{html}"
    );
    // reverse: olist nested in ulist item, return to ulist after
    let html = to_html("* a\n. num\n. num2\n* b");
    assert!(
        html.contains("<p>a</p>\n<div class=\"olist arabic\">\n<ol class=\"arabic\">"),
        "olist must nest inside the ulist item. Got:\n{html}"
    );
    assert!(
        html.contains("</ol>\n</div>\n</li>\n<li>\n<p>b</p>"),
        "* b returns to the parent ulist. Got:\n{html}"
    );
    // an unmatched SHALLOWER marker also nests (asciidoctor stack matching)
    let html = to_html("** b\n* c");
    assert!(
        html.contains("<p>b</p>\n<div class=\"ulist\">\n<ul>\n<li>\n<p>c</p>"),
        "unmatched shallower marker nests. Got:\n{html}"
    );
}

#[test]
fn test_unordered_dash_marker_nests_under_star() {
    // `-` is a SEPARATE marker family from `*` (identity 0 vs the `*`-count),
    // so `- x` under `* y` nests instead of rendering as a flat sibling, and a
    // following `*` matches the outer star list as a sibling (probe
    // /tmp/p_un1 = corpus unordered.adoc `nest-alt` tag).
    let html = to_html("* Level 1\n- Level 2\n* Level 1 again");
    assert!(
        html.contains("<p>Level 1</p>\n<div class=\"ulist\">\n<ul>\n<li>\n<p>Level 2</p>\n</li>\n</ul>\n</div>\n</li>"),
        "`- ` must nest inside the `* ` item. Got:\n{html}"
    );
    assert!(
        html.contains("</div>\n</li>\n<li>\n<p>Level 1 again</p>"),
        "the second `* ` is a sibling of the first. Got:\n{html}"
    );
    // `-` outer, `*` nested, `-` matches outer → sibling (probe /tmp/p_un2)
    let html = to_html("- a\n* b\n- c");
    assert!(
        html.contains("<p>a</p>\n<div class=\"ulist\">\n<ul>\n<li>\n<p>b</p>\n</li>\n</ul>\n</div>\n</li>\n<li>\n<p>c</p>"),
        "`* ` nests in `- a`, second `- ` is its sibling. Got:\n{html}"
    );
    // `*` after `**` still nests deeper — count is identity, not level
    // (probe /tmp/p_un5: `- a` / `** b` / `* c`)
    let html = to_html("- a\n** b\n* c");
    assert!(
        html.contains("<p>b</p>\n<div class=\"ulist\">\n<ul>\n<li>\n<p>c</p>"),
        "`* ` after `** ` nests deeper, not back to a level. Got:\n{html}"
    );
}

#[test]
fn test_unordered_list_marker_style_class() {
    // An explicit block style on a `*`/`-` list (`[square]`, `[circle]`, or any
    // keyword) is the marker class on BOTH the wrapper div (`ulist {style}
    // {roles}`) and the `<ul>` (style only — roles/id stay on the div). Probes
    // /tmp/p_sq, p_sqr, p_role, p_ov.
    let html = to_html("[square]\n* one\n* two");
    assert!(
        html.contains("<div class=\"ulist square\">\n<ul class=\"square\">"),
        "[square] → class on div and ul. Got:\n{html}"
    );
    // A role lands only on the div, never the `<ul>`.
    let html = to_html("[.myrole]\n* a");
    assert!(
        html.contains("<div class=\"ulist myrole\">\n<ul>\n"),
        "role goes on the div only, ul stays plain. Got:\n{html}"
    );
    // Style + role: div gets both (style first), ul only the style.
    let html = to_html("[square.myrole]\n* a");
    assert!(
        html.contains("<div class=\"ulist square myrole\">\n<ul class=\"square\">"),
        "style+role: div `ulist square myrole`, ul `square`. Got:\n{html}"
    );
    // A nested list carries its own style (marker-override, probe /tmp/p_ov):
    // the inner `[circle]` list gets the class on its div and ul.
    let html = to_html("[square]\n* squares\n** up top\n[circle]\n*** circles\n**** down below");
    assert!(
        html.contains("<div class=\"ulist circle\">\n<ul class=\"circle\">"),
        "nested [circle] list must carry its own style. Got:\n{html}"
    );
    // The style does NOT propagate to unstyled nested lists (the `**` list
    // under `* squares` stays plain even though its parent is `[square]`).
    assert!(
        html.contains("<p>squares</p>\n<div class=\"ulist\">\n<ul>\n"),
        "unstyled nested list stays plain. Got:\n{html}"
    );
}

#[test]
fn test_ordered_list_style_from_marker_depth() {
    // Implicit olist style comes from the marker's dot count, not the
    // ol-nesting count (probe /tmp/p_subs/p8, p9): `..` nested directly
    // in a ulist item is loweralpha.
    let html = to_html("* u1\n.. deep\n* u2");
    assert!(
        html.contains("<ol class=\"loweralpha\" type=\"a\""),
        "`..` marker → loweralpha even as the first ol. Got:\n{html}"
    );
    let html = to_html(".. alone");
    assert!(html.contains("<ol class=\"loweralpha\" type=\"a\""));
    let html = to_html("... three");
    assert!(html.contains("<ol class=\"lowerroman\" type=\"i\""));
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
fn test_inline_anchor_macro_and_xreflabel_html() {
    // anchor:id[] renders <a id></a>; the xreflabel is never rendered in place.
    let html = to_html("anchor:bookmark-c[]Use a cross reference.");
    assert!(html.contains("<a id=\"bookmark-c\"></a>Use a cross reference."), "{html}");
    let html = to_html("anchor:bm-x[Custom Label]Text after.");
    assert!(html.contains("<a id=\"bm-x\"></a>Text after."), "{html}");
    assert!(!html.contains("Custom Label"), "{html}");

    // [[id,xreflabel]] — inline and block forms strip the label from the id.
    let html = to_html("[[bookmark-d,last paragraph]]The xreflabel attribute.");
    assert!(html.contains("<a id=\"bookmark-d\"></a>The xreflabel attribute."), "{html}");
    let html = to_html("[[tiger-image,Image of a tiger]]\nimage::tiger.png[]");
    assert!(html.contains("id=\"tiger-image\""), "{html}");
    assert!(!html.contains("Image of a tiger\""), "{html}");

    // [[id]] with trailing content is a paragraph with an inline anchor,
    // not a block-attribute line.
    let html = to_html("[[tiger-image]]image:tiger.png[Image of a tiger]");
    assert!(html.contains("<div class=\"paragraph\">"), "{html}");
    assert!(html.contains("<a id=\"tiger-image\"></a><span class=\"image\">"), "{html}");

    // [[id,label]] on a description-list term.
    let html = to_html("[[cpu,CPU]]Central Processing Unit (CPU)::\nThe brain of the computer.");
    assert!(html.contains("<dt class=\"hdlist1\"><a id=\"cpu\"></a>Central Processing Unit (CPU)</dt>"), "{html}");
}

#[test]
fn test_anchor_reftext_xref_resolution() {
    // A leading [[id]] anchor in a dlist term catalogs the rendered term as
    // the anchor's reference text: an empty <<id>> resolves to the term.
    let html = to_html("[[el]]element:: An element is a chunk of text.\n\nSee <<el>>.");
    assert!(html.contains("<a href=\"#el\">element</a>"), "{html}");

    // Inline markup in the term is preserved in the reference text.
    let html = to_html("[[bt]]term with *bold*:: def.\n\nSee <<bt>>.");
    assert!(html.contains("<a href=\"#bt\">term with <strong>bold</strong></a>"), "{html}");

    // An explicit xreflabel wins over the term, and is formatted at use.
    let html = to_html("[[ba,boxed *attribute* list]]boxed attrlist:: def.\n\nSee <<ba>>.");
    assert!(html.contains("<a href=\"#ba\">boxed <strong>attribute</strong> list</a>"), "{html}");

    // anchor:id[label] registers its label too.
    let html = to_html("anchor:ff[FLabel]inline anchored text.\n\nSee <<ff>>.");
    assert!(html.contains("<a href=\"#ff\">FLabel</a>"), "{html}");

    // Forward reference: the xref appears before the anchor is defined.
    let html = to_html("Refs first: <<aa>>.\n\n[[aa]]term text:: definition.");
    assert!(html.contains("<a href=\"#aa\">term text</a>"), "{html}");

    // A mid-term anchor (not leading) gets no default reftext — fallback [id].
    let html = to_html("middle [[jj]]anchored term:: def.\n\nSee <<jj>>.");
    assert!(html.contains("<a href=\"#jj\">[jj]</a>"), "{html}");

    // A bare [[id]] in a paragraph gets no default reftext either.
    let html = to_html("[[cc]]Some paragraph text.\n\nSee <<cc>>.");
    assert!(html.contains("<a href=\"#cc\">[cc]</a>"), "{html}");
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
fn test_dlist_continuation_openblock_multiple_children_html() {
    // A `+` continuation attaching a `--` open block must keep scanning the
    // open block's content past internal blank lines. Previously a blank line
    // inside the open block fired a list-closing guard (we are nested in a
    // dlist), close_list_contexts found no list at the stack top, and the
    // parser truncated everything after the first child block.
    let html = to_html(
        "term::\n+\n--\nFirst paragraph.\n\n.Solution A\n====\nInside example.\n====\n\nAfter example.\n--",
    );
    // All three children survive, properly closed inside the open block.
    assert!(html.contains("<p>First paragraph.</p>"), "first paragraph:\n{html}");
    assert!(
        html.contains("<div class=\"exampleblock\">"),
        "example block must not be dropped:\n{html}"
    );
    assert!(
        html.contains("Example 1. Solution A"),
        "example title must survive:\n{html}"
    );
    assert!(html.contains("<p>Inside example.</p>"), "example body:\n{html}");
    assert!(
        html.contains("<p>After example.</p>"),
        "trailing paragraph after the nested block must survive:\n{html}"
    );
    // The dd/openblock/dlist wrappers must all close (no premature truncation).
    assert!(html.contains("</dd>\n</dl>\n</div>"), "wrappers must close:\n{html}");

    // Negative: a blank line still closes a list when scanning directly in
    // list-item content (not inside a nested delimited block).
    let closed = to_html("* item one\n\nParagraph after list.");
    assert!(
        closed.contains("</ul>\n</div>\n<div class=\"paragraph\">\n<p>Paragraph after list.</p>"),
        "blank line must still close a top-level list:\n{closed}"
    );
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
fn test_table_leading_blank_suppresses_implicit_header_html() {
    // blank line directly after |=== cancels first-row header promotion
    let html = to_html("|===\n\n|A |B\n\n|1 |2\n|===");
    assert!(!html.contains("<thead>"), "leading blank must suppress header. Got:\n{html}");
    // ...even several blanks
    let html = to_html("|===\n\n\n|A |B\n\n|1 |2\n|===");
    assert!(!html.contains("<thead>"));
    // ...and a blank after a leading comment line still suppresses
    let html = to_html("|===\n// c\n\n|A |B\n\n|1 |2\n|===");
    assert!(!html.contains("<thead>"));
    // a leading comment alone is invisible — header is still promoted
    let html = to_html("|===\n// c\n|A |B\n\n|1 |2\n|===");
    assert!(html.contains("<thead>"));
    // explicit %header overrides the leading blank
    let html = to_html("[%header]\n|===\n\n|A |B\n\n|1 |2\n|===");
    assert!(html.contains("<thead>"));
    // column count is still derived from the first row
    let html = to_html("|===\n\n|a |b\n\n|c |d\n|===");
    assert_eq!(html.matches("<col ").count(), 2);
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
    // Reference: footnoteref class, no anchor ids, same number
    assert!(html.contains("<sup class=\"footnoteref\">[<a class=\"footnote\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>"));
    // Only one definition in the footnotes section
    assert!(html.contains("<div class=\"footnote\" id=\"_footnotedef_1\">"));
    assert!(!html.contains("_footnotedef_2"));
}

#[test]
fn test_footnote_named_reuse_and_unresolved() {
    // Reuse with text: id already defined wins — text ignored, counter not bumped
    let html = to_html(
        "Define.footnote:dis[Text A.]\n\nReuse.footnote:dis[Text B.]\n\nAnon.footnote:[anon two.]",
    );
    assert!(html.contains("<sup class=\"footnote\" id=\"_footnote_dis\">[<a class=\"footnote\" id=\"_footnoteref_1\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>"));
    assert!(html.contains("<sup class=\"footnoteref\">[<a class=\"footnote\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>"));
    // The anonymous footnote gets number 2 (reuse did not bump)
    assert!(html.contains(">2</a>. anon two."));
    assert!(!html.contains(">2</a>. Text B."));
    assert!(!html.contains("Text B.</"));

    // Empty reference to an undefined id: unresolved marker, no definition
    let html = to_html("Before def.footnote:nope[]");
    assert!(html.contains(
        "<sup class=\"footnoteref red\" title=\"Unresolved footnote reference.\">[nope]</sup>"
    ));
    assert!(!html.contains("footnotedef"));
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
fn test_footnotes_outside_content_div_standalone() {
    let html = to_html_with_options(
        "= Doc\n\nText.footnote:[Note here.]\n",
        HtmlOptions { standalone: true, ..Default::default() },
    );
    let content = html.find("<div id=\"content\">").expect("content div");
    let footnotes = html.find("<div id=\"footnotes\">").expect("footnotes div");
    let footer = html.find("<div id=\"footer\">").expect("footer div");
    assert!(footnotes < footer, "footnotes must precede the footer, got:\n{html}");
    // <div id="content"> must already be closed when footnotes start: div
    // opens and closes balance out across the content section.
    let between = &html[content..footnotes];
    assert_eq!(
        between.matches("<div").count(),
        between.matches("</div>").count(),
        "footnotes div must sit outside <div id=\"content\">, got:\n{html}"
    );
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
    // Includes are resolved by the preprocessor (reader); a line reaching the
    // parser — e.g. from an escaped `\include::` — is plain paragraph text,
    // matching Asciidoctor.
    let html = to_html("include::chapter.adoc[]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>include::chapter.adoc[]</p>\n</div>\n");
}

#[test]
fn test_unresolved_include_with_special_chars_html() {
    let html = to_html("include::path/to/<file>.adoc[]");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>include::path/to/&lt;file&gt;.adoc[]</p>\n</div>\n");
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
fn test_checklist_interactive_html() {
    // %interactive: checkbox items render real <input> controls (probe-verified).
    let html = to_html("[%interactive]\n* [x] Done\n* [ ] Todo\n* Regular");
    assert!(html.contains("<li>\n<p><input type=\"checkbox\" data-item-complete=\"1\" checked> Done</p>\n</li>"));
    assert!(html.contains("<li>\n<p><input type=\"checkbox\" data-item-complete=\"0\"> Todo</p>\n</li>"));
    assert!(html.contains("<li>\n<p>Regular</p>\n</li>"));

    // Formal options= form is an alias.
    let html = to_html("[options=interactive]\n* [x] Done");
    assert!(html.contains("<input type=\"checkbox\" data-item-complete=\"1\" checked> Done"));

    // Nested list does not inherit the option from the outer list.
    let html = to_html("[%interactive]\n* [x] outer\n** [x] nested\n* [ ] outer2");
    assert!(html.contains("<input type=\"checkbox\" data-item-complete=\"1\" checked> outer"));
    assert!(html.contains("<p>&#10003; nested</p>"));
    assert!(html.contains("<input type=\"checkbox\" data-item-complete=\"0\"> outer2"));

    // Without the option the NCR markers stay.
    let html = to_html("* [x] Done");
    assert!(html.contains("<p>&#10003; Done</p>"));
    assert!(!html.contains("<input"));
}

#[test]
fn test_list_block_title_html() {
    // `.Title` on a list renders <div class="title"> inside the wrapper div,
    // before the list element (probe-verified for all list shapes).
    let html = to_html(".TODO list\n- a\n- b");
    assert!(html.contains("<div class=\"ulist\">\n<div class=\"title\">TODO list</div>\n<ul>"));

    let html = to_html(".Ordered\n. one");
    assert!(html.contains("<div class=\"olist arabic\">\n<div class=\"title\">Ordered</div>\n<ol class=\"arabic\">"));

    let html = to_html(".Desc\nterm:: def");
    assert!(html.contains("<div class=\"dlist\">\n<div class=\"title\">Desc</div>\n<dl>"));

    let html = to_html(".Horiz\n[horizontal]\na:: 1");
    assert!(html.contains("<div class=\"hdlist\">\n<div class=\"title\">Horiz</div>\n<table>"));

    let html = to_html(".Q\n[qanda]\nQ?:: A.");
    assert!(html.contains("<div class=\"qlist qanda\">\n<div class=\"title\">Q</div>\n<ol>"));

    let html = to_html("[source]\n----\nx <1>\n----\n.ColistTitle\n<1> note");
    assert!(html.contains("<div class=\"colist arabic\">\n<div class=\"title\">ColistTitle</div>\n<ol>"));

    // A title line after a blank splits adjacent lists and titles the second.
    let html = to_html("- a\n\n.Second\n- b");
    assert!(html.contains("</ul>\n</div>\n<div class=\"ulist\">\n<div class=\"title\">Second</div>\n<ul>"));
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
    // Literal cell: <div class="literal"><pre> with verbatim subs — no inline
    // formatting or attribute refs, special chars escaped (probe-verified).
    let html = to_html("|===\nl|*not bold* <tag> {empty}\n|===");
    assert!(
        html.contains("<div class=\"literal\"><pre>*not bold* &lt;tag&gt; {empty}</pre></div>"),
        "expected literal div cell. Got:\n{html}"
    );
}

#[test]
fn test_table_cell_style_asciidoc_html() {
    // `a|` cell: nested block parse inside <div class="content"> (probe-verified).
    let html = to_html("|===\na|\n* one\n* two\n|plain\n|===");
    assert!(
        html.contains("<td class=\"tableblock halign-left valign-top\"><div class=\"content\"><div class=\"ulist\">"),
        "expected content div with nested ulist. Got:\n{html}"
    );
    assert!(html.contains("<li>\n<p>one</p>\n</li>"), "expected nested list items. Got:\n{html}");
    // No newline before the closing </div></td>
    assert!(html.contains("</div></div></td>"), "expected trimmed close. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">plain</p>"), "plain cell unaffected. Got:\n{html}");

    // Simple text still becomes a nested paragraph (not a tableblock p)
    let html = to_html("|===\na|Simple text.\n|===");
    assert!(
        html.contains("<div class=\"content\"><div class=\"paragraph\">\n<p>Simple text.</p>\n</div></div></td>"),
        "expected nested paragraph. Got:\n{html}"
    );

    // Empty a-cell: empty content div (probe-verified)
    let html = to_html("|===\na| |x\n|===");
    assert!(html.contains("<div class=\"content\"></div></td>"), "expected empty content div. Got:\n{html}");

    // Blank lines inside the cell are structural: two nested paragraphs
    let html = to_html("|===\na|Para one.\n\nPara two.\n|===");
    assert!(
        html.contains("<p>Para one.</p>\n</div>\n<div class=\"paragraph\">\n<p>Para two.</p>"),
        "expected two nested paragraphs. Got:\n{html}"
    );
}

#[test]
fn test_table_cell_literal_preserves_blank_and_indent() {
    // Literal cell keeps inner blank lines and indentation; the edges of the
    // whole cell text are stripped; a plain cell still collapses blank lines
    // and trims continuation lines (probe-verified /tmp/p_acell/p12).
    let html = to_html("|===\nl|line1\n  indented\nline3\n\nafter blank\n|===");
    assert!(
        html.contains("<div class=\"literal\"><pre>line1\n  indented\nline3\n\nafter blank</pre></div>"),
        "expected preserved blank+indent in literal cell. Got:\n{html}"
    );

    // A blank line splits a default cell into two <p class="tableblock">
    // paragraphs (asciidoctor Cell#content; probe /tmp/p_cellp/p6). Within a
    // paragraph the lines stay joined by '\n'; continuation-line indentation is
    // trimmed (pre-existing — asciidoctor preserves it, a separate limitation).
    let html = to_html("|===\n|one\n  two\n\nthree\n|===");
    assert!(
        html.contains("<p class=\"tableblock\">one\ntwo</p><p class=\"tableblock\">three</p>"),
        "expected split plain cell. Got:\n{html}"
    );
}

#[test]
fn test_table_cell_multi_paragraph_html() {
    // A default/styled body cell whose text has a blank line becomes several
    // <p class="tableblock"> paragraphs, each carrying the style wrapper; inline
    // subs still apply within each (probe-verified /tmp/p_cellp/p1..p6).

    // Default cell: two paragraphs, second carries inline formatting.
    let html = to_html("|===\n|Not applicable.\n\n*emphasized line*\n|===");
    assert!(
        html.contains("<p class=\"tableblock\">Not applicable.</p><p class=\"tableblock\"><strong>emphasized line</strong></p>"),
        "expected two default paragraphs. Got:\n{html}"
    );

    // Three paragraphs.
    let html = to_html("|===\n|one\n\ntwo\n\nthree\n|===");
    assert!(
        html.contains("<p class=\"tableblock\">one</p><p class=\"tableblock\">two</p><p class=\"tableblock\">three</p>"),
        "expected three paragraphs. Got:\n{html}"
    );

    // Monospace column: the <code> wrapper repeats per paragraph.
    let html = to_html("[cols=\"m\"]\n|===\n|first para\n\nsecond para\n|===");
    assert!(
        html.contains("<p class=\"tableblock\"><code>first para</code></p><p class=\"tableblock\"><code>second para</code></p>"),
        "expected per-paragraph <code> wrappers. Got:\n{html}"
    );

    // Explicit emphasis cell: per-paragraph <em>.
    let html = to_html("|===\ne|alpha\n\nbeta\n|===");
    assert!(
        html.contains("<p class=\"tableblock\"><em>alpha</em></p><p class=\"tableblock\"><em>beta</em></p>"),
        "expected per-paragraph <em> wrappers. Got:\n{html}"
    );

    // Several consecutive blank lines collapse to a single split.
    let html = to_html("|===\n|first\n\n\nlast\n|===");
    assert!(
        html.contains("<p class=\"tableblock\">first</p><p class=\"tableblock\">last</p>"),
        "expected one split for multiple blanks. Got:\n{html}"
    );

    // Single-paragraph cell is unaffected (no extra paragraph wrapper).
    let html = to_html("|===\n|just one line\n|===");
    assert!(
        html.contains("<td class=\"tableblock halign-left valign-top\"><p class=\"tableblock\">just one line</p></td>"),
        "single-paragraph cell unchanged. Got:\n{html}"
    );
}

#[test]
fn test_table_cell_empty_styled_no_wrapper_html() {
    // An empty styled cell (m/e/s) renders a bare <td></td>, like a default
    // empty cell — asciidoctor's `Cell#content` returns [] for empty text, so
    // no paragraph (and no inner <code>/<em>/<strong>) is emitted. Root of
    // table-ref.adoc @848 (empty `m` column cell). Probe /tmp/p_empty2.
    for style in ["m", "e", "s"] {
        let html = to_html(&format!("[cols=\"1{style},1\"]\n|===\n|filled |x\n| |y\n|==="));
        assert!(
            html.contains("<td class=\"tableblock halign-left valign-top\"></td>"),
            "empty {style} cell should be a bare <td></td>. Got:\n{html}"
        );
        // The non-empty cell in the same column keeps its wrapper.
        assert!(
            !html.contains("<code></code>") && !html.contains("<em></em>") && !html.contains("<strong></strong>"),
            "empty {style} cell must not emit an empty inline wrapper. Got:\n{html}"
        );
    }

    // Empty default and header cells were already bare; confirm no regression.
    let html = to_html("[cols=\"1,1h\"]\n|===\n| |\n|===");
    assert!(
        html.contains("<td class=\"tableblock halign-left valign-top\"></td>"),
        "empty default cell bare. Got:\n{html}"
    );
    assert!(
        html.contains("<th class=\"tableblock halign-left valign-top\"></th>"),
        "empty header-column cell bare. Got:\n{html}"
    );

    // A non-empty monospace cell is unchanged.
    let html = to_html("[cols=\"m\"]\n|===\n|rotate\n|===");
    assert!(
        html.contains("<p class=\"tableblock\"><code>rotate</code></p>"),
        "non-empty m cell keeps wrapper. Got:\n{html}"
    );
}

#[test]
fn test_table_column_style_inheritance_html() {
    // Column styles apply to cells without an explicit style; explicit styles
    // (including `d` and `v` → default) win; header rows ignore column styles
    // (probe-verified /tmp/p_acell/p7, p9, p10).
    let html = to_html("[cols=\"1m,1s\"]\n|===\n|h1 |h2\n\nd|over |body\n|mono s|strong\n|===");
    assert!(html.contains("<th class=\"tableblock halign-left valign-top\">h1</th>"), "header plain. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">over</p>"), "explicit d overrides m column. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\"><strong>body</strong></p>"), "s column inherited. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\"><code>mono</code></p>"), "m column inherited. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\"><strong>strong</strong></p>"), "explicit s. Got:\n{html}");

    // AsciiDoc column style inherits too
    let html = to_html("[cols=\"1a,1\"]\n|===\n|nested *bold*\n|plain\n|===");
    assert!(
        html.contains("<div class=\"content\"><div class=\"paragraph\">\n<p>nested <strong>bold</strong></p>\n</div></div></td>"),
        "a column inherited. Got:\n{html}"
    );
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
fn test_table_cell_continuation_lines_html() {
    // A line without `|` continues the previous cell, joined with a newline
    // inside the same tableblock paragraph (probe-verified vs asciidoctor)
    let html = to_html("|===\n|first line\nsecond line\n|cell two\n|===");
    assert!(html.contains("<p class=\"tableblock\">first line\nsecond line</p>"), "expected joined cell content. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">cell two</p>"));

    // Text before a mid-line `|` continues the previous cell; the cell opened
    // on the continuation line still counts toward the implicit column count
    let html = to_html("|===\n|a\nmid |late\n|b |c\n|===");
    assert!(html.contains("<p class=\"tableblock\">a\nmid</p>"), "expected continuation before pipe. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">late</p>"));
    assert!(html.contains("<p class=\"tableblock\">b</p>"));

    // A continuation line cancels implicit header promotion: the row is not
    // a single line followed by a blank
    let html = to_html("|===\n|h\ncont\n\n|body\n|===");
    assert!(!html.contains("<thead>"), "continuation must cancel implicit header. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">h\ncont</p>"));

    // A continuation line directly after the implicit-header blank also
    // cancels the promotion (the cell of the first row continues)
    let html = to_html("|===\n|a\n\ncont after blank\n|b\n|===");
    assert!(!html.contains("<thead>"), "post-blank continuation must cancel header. Got:\n{html}");

    // Normal substitutions apply to merged (owned) cell content
    let html = to_html("|===\n|isn't done\nsecond *bold* line\n|===");
    assert!(html.contains("isn’t"), "expected curly apostrophe in merged cell. Got:\n{html}");
    assert!(html.contains("<strong>bold</strong>"));

    // Line comments are invisible inside tables: dropped from cell content,
    // and a standalone comment doesn't cancel the implicit header
    let html = to_html("|===\n|h\n\n// note\n|a\ncont\n// mid-cell\nmore\n|===");
    assert!(html.contains("<thead>"), "comment must not cancel header. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">a\ncont\nmore</p>"), "expected comment dropped from cell. Got:\n{html}");
}

#[test]
fn test_table_empty_cell_html() {
    // An empty cell renders as a bare <td></td> without the tableblock
    // paragraph (probe-verified vs asciidoctor: `|a |` and `|a | |c`)
    let html = to_html("|===\n|a |\n|b |c\n|===");
    assert!(html.contains("<td class=\"tableblock halign-left valign-top\"></td>"), "expected bare empty td. Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">a</p>"));
    assert!(html.contains("<p class=\"tableblock\">c</p>"));
}

#[test]
fn test_table_cell_duplication_and_chained_specs_html() {
    // `2*>m|x` duplicates the cell across two columns, right-aligned mono;
    // copies carry the full content including continuation lines
    let html = to_html("|===\n|h1 |h2\n\n2*>m|dup\nmore\n|===");
    let needle = "<td class=\"tableblock halign-right valign-top\"><p class=\"tableblock\"><code>dup\nmore</code></p></td>";
    assert_eq!(html.matches(needle).count(), 2, "expected two duplicated cells. Got:\n{html}");
    assert!(html.contains("<thead>"), "implicit header must survive a spec line. Got:\n{html}");

    // Chained span+align+style spec `.2+^.>s|`
    let html = to_html("|===\n|a |b\n.2+^.>s|tall |x\n|y |z\n|===");
    assert!(html.contains("rowspan=\"2\"><p class=\"tableblock\"><strong>tall</strong></p>"), "expected chained spec parsed. Got:\n{html}");
}

#[test]
fn test_table_incomplete_last_row_dropped_html() {
    // asciidoctor drops cells from an incomplete row at the end of the table
    let html = to_html("|===\n|a |b\n|c\n|===");
    assert!(html.contains("<p class=\"tableblock\">a</p>"));
    assert!(html.contains("<p class=\"tableblock\">b</p>"));
    assert!(!html.contains("<p class=\"tableblock\">c</p>"), "incomplete trailing row must be dropped. Got:\n{html}");
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
fn test_table_cell_explicit_left_overrides_cols_align_html() {
    // An explicit `<` (Left) / `.<` (Top) operator must win over the column
    // default, even though Left/Top are also the implicit defaults. Without
    // tracking explicitness the cell would inherit the column's right/bottom.
    let html = to_html("[cols=\">\"]\n|===\n<| left\n|===");
    assert!(
        html.contains("halign-left"),
        "explicit `<` must override the column's right default. Got:\n{html}"
    );
    assert!(!html.contains("halign-right"), "Got:\n{html}");

    let html = to_html("[cols=\".>\"]\n|===\n.<| top\n|===");
    assert!(
        html.contains("valign-top"),
        "explicit `.<` must override the column's bottom default. Got:\n{html}"
    );
    assert!(!html.contains("valign-bottom"), "Got:\n{html}");

    // Negative: a cell without an alignment operator still inherits the
    // column's right default (no spurious explicit flag).
    let html = to_html("[cols=\">\"]\n|===\n| plain\n|===");
    assert!(
        html.contains("halign-right"),
        "an unaligned cell inherits the column default. Got:\n{html}"
    );
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
    let html = to_html("[source#code1,rust]\n----\nfn main() {}\n----");
    assert!(html.contains("id=\"code1\""));
    // Shorthand outside the first comma-part is verbatim positional text:
    // slot 3 of a source block is `linenums`, so `#code1` there enables
    // numbering and sets no id (matches Asciidoctor).
    let html = to_html(":source-highlighter: rouge\n\n[source,rust,#code1]\n----\nfn main() {}\n----");
    assert!(!html.contains("id=\"code1\""));
    assert!(html.contains("<table class=\"linenotable\">"));
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
fn test_table_stacked_attr_lines_merge_html() {
    // Stacked metadata lines accumulate (probe-verified): [caption=...] above
    // .Title above [cols=...] — the caption must not be lost, the custom
    // label must not bump the counter, and the cols multiplier must expand.
    let html = to_html(concat!(
        "[caption=\"Table A. \"]\n.Custom\n[cols=\"3*\"]\n|===\n|Null\n|A mystery\n|See Appendix R\n|===\n",
        "\n.Numbered\n|===\n| X\n|===",
    ));
    assert!(html.contains("<caption class=\"title\">Table A. Custom</caption>"), "Got:\n{html}");
    assert!(html.contains("<col style=\"width: 33.3333%;\">"), "Got:\n{html}");
    assert!(html.contains("<col style=\"width: 33.3334%;\">"), "Got:\n{html}");
    // All three single-column cells in one row of three columns
    assert!(html.contains("See Appendix R"), "Got:\n{html}");
    assert!(html.contains("<caption class=\"title\">Table 1. Numbered</caption>"), "Got:\n{html}");
}

#[test]
fn test_table_cols_semicolon_separator_html() {
    // Unquoted `[cols=1;m;m]` uses the semicolon separator → 3 columns with
    // equal weight (33.3333/33.3333/33.3334). The first single-line row of
    // three cells followed by a blank line becomes a header row (thead).
    let html = to_html(concat!(
        "[cols=1;m;m]\n|===\n|H1 | H2 | H3\n\n|a\n|b\n|c\n|===",
    ));
    assert_eq!(html.matches("<col ").count(), 3, "Got:\n{html}");
    assert!(html.contains("<col style=\"width: 33.3333%;\">"), "Got:\n{html}");
    assert!(html.contains("<col style=\"width: 33.3334%;\">"), "Got:\n{html}");
    assert!(html.contains("<thead>"), "Got:\n{html}");
    // The m-styled body columns wrap their cells in <code>
    assert!(html.contains("<code>b</code>"), "Got:\n{html}");
}

#[test]
fn test_table_cols_multiplier_widths_html() {
    // `2*1,3` → 20/20/60; trailing single-letter cell content survives
    let html = to_html("[cols=\"2*1,3\"]\n|===\n|a |b |c\n|===");
    assert!(html.contains("<col style=\"width: 20%;\">"), "Got:\n{html}");
    assert!(html.contains("<col style=\"width: 60%;\">"), "Got:\n{html}");

    // multiplier with full spec: 2*<.^2,>1 → 40/40/20
    let html = to_html("[cols=\"2*<.^2,>1\"]\n|===\n|g |h |i\n|===");
    assert!(html.contains("<col style=\"width: 40%;\">"), "Got:\n{html}");
    assert!(html.contains("<col style=\"width: 20%;\">"), "Got:\n{html}");
    assert!(html.contains("halign-left valign-middle"), "Got:\n{html}");
    assert!(html.contains("halign-right valign-top"), "Got:\n{html}");
}

#[test]
fn test_table_single_letter_cell_content_html() {
    // `|a` at end of line is a cell "a", not an AsciiDoc-style spec
    let html = to_html("|===\n|a\n|===");
    assert!(html.contains("<p class=\"tableblock\">a</p>"), "Got:\n{html}");
    let html = to_html("|===\n|d |e\n|===");
    assert!(html.contains("<p class=\"tableblock\">d</p>"), "Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">e</p>"), "Got:\n{html}");
}

#[test]
fn test_counter_literal_in_listing_block_html() {
    // Counters expand in the preprocessor, but not inside verbatim blocks
    // (listing content gets no attributes substitution)
    let pre = adoc_parser::preprocess("----\n{counter:n}\n----\n\npara {counter:n}");
    let html = to_html(&pre);
    assert!(html.contains("<pre>{counter:n}</pre>"), "Got:\n{html}");
    assert!(html.contains("<p>para 1</p>"), "Got:\n{html}");
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

#[test]
fn test_float_heading_alias_of_discrete() {
    // `[float]` is the legacy alias of `[discrete]`: a standalone heading
    // (no section wrapper), with the literal style name as the class. The
    // level maps to hN the same way sections do (level 0 → h1, …).
    let html = to_html("para\n\n[float]\n= Level 0\n\n[float]\n== Level 1\n\n[float]\n=== Level 2");
    assert!(html.contains("<h1 id=\"_level_0\" class=\"float\">Level 0</h1>"), "{html}");
    assert!(html.contains("<h2 id=\"_level_1\" class=\"float\">Level 1</h2>"), "{html}");
    assert!(html.contains("<h3 id=\"_level_2\" class=\"float\">Level 2</h3>"), "{html}");
    // No section wrapper div around a float heading.
    assert!(!html.contains("class=\"sect0\""), "{html}");
    // `[float.role]` carries the role; explicit id honored.
    let html = to_html("[float.myrole]\n== Styled");
    assert!(html.contains("class=\"float myrole\""), "{html}");
    // float heading is not numbered and not in the TOC.
    let html = to_html("= D\n:toc:\n:sectnums:\n\n== Real One\n\n[float]\n== Floating\n\n== Real Two");
    assert!(html.contains("<h2 id=\"_floating\" class=\"float\">Floating</h2>"), "{html}");
    assert!(!html.contains(">Floating</a>"), "float must not be in TOC: {html}");
    assert!(html.contains("<a href=\"#_real_two\">2. Real Two</a>"), "float must not consume a number: {html}");
}

#[test]
fn test_sectnumlevels_caps_numbering_depth() {
    // Default sectnumlevels=3: Asciidoctor levels 1..3 (display 2..4) are
    // numbered, deeper ones are not.
    let html = to_html("= D\n:sectnums:\n\n== L1\n\n=== L2\n\n==== L3\n\n===== L4");
    assert!(html.contains("<h2 id=\"_l1\">1. L1</h2>"), "{html}");
    assert!(html.contains("<h4 id=\"_l3\">1.1.1. L3</h4>"), "{html}");
    assert!(html.contains("<h5 id=\"_l4\">L4</h5>"), "level 4 unnumbered by default: {html}");

    // sectnumlevels=2 → only levels 1..2 numbered.
    let html = to_html("= D\n:sectnums:\n:sectnumlevels: 2\n\n== L1\n\n=== L2\n\n==== L3");
    assert!(html.contains("<h3 id=\"_l2\">1.1. L2</h3>"), "{html}");
    assert!(html.contains("<h4 id=\"_l3\">L3</h4>"), "level 3 unnumbered when sectnumlevels=2: {html}");

    // Value parsed Ruby-to_i style: leading digits, trailing junk ignored.
    let html = to_html("= D\n:sectnums:\n:sectnumlevels: 2 <.>\n\n== L1\n\n=== L2\n\n==== L3");
    assert!(html.contains("<h4 id=\"_l3\">L3</h4>"), "'2 <.>' must parse as 2: {html}");
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
fn test_bareword_role_span_html() {
    // [big]##O## — a bare-word attrlist becomes the role (Asciidoctor parity);
    // the single-tick form [big]#O#nce stays literal (constrained close before a
    // word char).
    let html = to_html("[big]##O##nce upon a loop.");
    assert_eq!(html, "<div class=\"paragraph\">\n<p><span class=\"big\">O</span>nce upon a loop.</p>\n</div>\n");
}

#[test]
fn test_backtick_apostrophe_not_monospace_html() {
    // Two `' (right single quote) markers in one run must not fold into a <code>
    // span: the monospace close assertion forbids a backtick followed by `'`.
    let html = to_html("the `'00s and werewolves`' desks.");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>the \u{2019}00s and werewolves\u{2019} desks.</p>\n</div>\n");
}

#[test]
fn test_superscript_content_full_subs_html() {
    // Superscript/subscript content gets the full normal substitution group:
    // attribute refs ({sp}), quotes (*strong*), replacements, macros.
    let html = to_html("x^a{sp}b^ and ^*z*^");
    assert_eq!(html, "<div class=\"paragraph\">\n<p>x<sup>a b</sup> and <sup><strong>z</strong></sup></p>\n</div>\n");
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
fn test_book_part_numbering_partnums_html() {
    // Book parts under :partnums: get a "{signifier} {roman}: " prefix on the
    // <h1 class="sect0"> heading and in the TOC; roman numerals are sequential
    // and document-global. The signifier set adds "Part ".
    let src = "= Doc\n:doctype: book\n:partnums:\n:part-signifier: Part\n:toc:\n\n\
               = First Part\n\n== A Chapter\n\nx\n\n= Second Part\n\n== B Chapter\n\ny";
    let html = to_html(src);
    assert!(html.contains("<h1 id=\"_first_part\" class=\"sect0\">Part I: First Part</h1>"), "{html}");
    assert!(html.contains("<h1 id=\"_second_part\" class=\"sect0\">Part II: Second Part</h1>"), "{html}");
    // Chapters are NOT numbered (no :sectnums:).
    assert!(html.contains("<h2 id=\"_a_chapter\">A Chapter</h2>"), "{html}");
    // TOC: parts list is sectlevel0, prefix carried into the entry.
    assert!(html.contains("<ul class=\"sectlevel0\">"), "{html}");
    assert!(html.contains("<a href=\"#_first_part\">Part I: First Part</a>"), "{html}");

    // Without :partnums: → no prefix (parity with Asciidoctor).
    let html = to_html("= Doc\n:doctype: book\n\n= First Part\n\n== A\n\nx");
    assert!(html.contains("<h1 id=\"_first_part\" class=\"sect0\">First Part</h1>"), "{html}");

    // No part-signifier → bare roman ("I: ").
    let html = to_html("= Doc\n:doctype: book\n:partnums:\n\n= First Part\n\n== A\n\nx");
    assert!(html.contains("<h1 id=\"_first_part\" class=\"sect0\">I: First Part</h1>"), "{html}");
}

#[test]
fn test_article_sect0_toc_sectlevel0_html() {
    // A body sect0 (level-0 section) in an article also lists at TOC
    // sectlevel0 — the list class is the section's real Asciidoctor level.
    let html = to_html("= Doc\n:toc:\n\n= Body Zero\n\n== Sub\n\nx");
    assert!(html.contains("<ul class=\"sectlevel0\">"), "{html}");
    assert!(html.contains("<a href=\"#_body_zero\">Body Zero</a>"), "{html}");
    // The nested sub-section is sectlevel1.
    assert!(html.contains("<ul class=\"sectlevel1\">"), "{html}");
    // Regular article sections (no sect0) stay sectlevel1 only.
    let html = to_html("= Doc\n:toc:\n\n== Section A\n\nx\n\n== Section B\n\ny");
    assert!(html.contains("<ul class=\"sectlevel1\">"), "{html}");
    assert!(!html.contains("sectlevel0"), "{html}");
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

#[test]
fn test_part_intro_implicit_wrap() {
    // Leading body blocks of a book part are wrapped in an implicit
    // [partintro] open block that closes at the first child section
    // (Asciidoctor next_section). Multiple blocks share one wrapper.
    let html = to_html("= Book\n:doctype: book\n\n= Part I\n\nFirst intro.\n\n* item\n\n== Chapter A\n\ntext");
    assert!(html.contains(
        "<div class=\"openblock partintro\">\n<div class=\"content\">\n<div class=\"paragraph\">\n<p>First intro.</p>\n</div>\n<div class=\"ulist\">"
    ), "{html}");
    // The wrapper closed before the chapter.
    assert!(html.contains("</ul>\n</div>\n</div>\n</div>\n<div class=\"sect1\">\n<h2 id=\"_chapter_a\">"), "{html}");

    // A bare open block as the first part block is restyled in place — no
    // double wrapper; a following pre-section block renders OUTSIDE it
    // (Asciidoctor: "illegal block content outside of partintro block").
    let html = to_html("= Book\n:doctype: book\n\n= Part I\n\n--\nOpen intro.\n--\n\nOutside.\n\n== Chapter A\n\ntext");
    assert_eq!(html.matches("openblock partintro").count(), 1, "{html}");
    assert!(html.contains(
        "</div>\n</div>\n</div>\n<div class=\"paragraph\">\n<p>Outside.</p>\n</div>\n<div class=\"sect1\">"
    ), "{html}");

    // No partintro without book doctype or for a non-part section.
    let html = to_html("= Doc\n\n== Section\n\nText.");
    assert!(!html.contains("partintro"), "{html}");
}

#[test]
fn test_special_section_level_zero_coerced() {
    // A special-styled level-0 section ([preface] = T) is displayed at
    // level 1: sect1/h2 with a sectionbody; its subsection nests inside.
    let html = to_html("= Book\n:doctype: book\n\n[preface]\n= Book Preface\n\nPreface text.\n\n=== Sub\n\nSub text.\n\n= Part 1\n\n== Chapter 1\n\nMud.");
    assert!(html.contains(
        "<div class=\"sect1\">\n<h2 id=\"_book_preface\">Book Preface</h2>\n<div class=\"sectionbody\">"
    ), "{html}");
    assert!(html.contains("<div class=\"sect2\">\n<h3 id=\"_sub\">Sub</h3>"), "{html}");
    // The bare level-0 part stays a sect0 h1.
    assert!(html.contains("<h1 id=\"_part_1\" class=\"sect0\">Part 1</h1>"), "{html}");

    // [appendix] = X after a part closes the part (nesting is decided from
    // the RAW level) and renders as a numbered sect1 sibling.
    let html = to_html("= Book\n:doctype: book\n\n= Part 1\n\n== Chapter 1\n\nMud.\n\n[appendix]\n= The Appendix\n\nApp text.");
    assert!(html.contains(
        "<div class=\"sect1\">\n<h2 id=\"_the_appendix\">Appendix A: The Appendix</h2>"
    ), "{html}");
}

#[test]
fn test_toc_includes_book_parts() {
    // Parts enter the TOC at depth 1 (ul class sectlevel1); chapters nest
    // under them in their own sectlevel1 list; a coerced special section
    // (level-0 [colophon] → level 1) is a TOC SIBLING of the parts.
    let source = "= Book\nAuthor Name\n:doctype: book\n:toc:\n\n[colophon]\n= The Colophon\n\nText.\n\n= The First Part\n\n== The First Chapter\n\nText.\n\n[appendix]\n= The Appendix\n\n=== Basics\n\nText.";
    // The header auto-TOC sits AFTER the author details div (standalone).
    let full = to_html_with_options(source, HtmlOptions { standalone: true, ..Default::default() });
    let details_pos = full.find("<div class=\"details\">").unwrap();
    let toc_pos = full.find("<div id=\"toc\"").unwrap();
    assert!(details_pos < toc_pos, "{full}");
    let html = to_html(source);
    assert!(html.contains(
        "<ul class=\"sectlevel1\">\n<li><a href=\"#_the_colophon\">The Colophon</a></li>\n<li><a href=\"#_the_first_part\">The First Part</a>\n<ul class=\"sectlevel1\">\n<li><a href=\"#_the_first_chapter\">The First Chapter</a></li>\n</ul>\n</li>\n<li><a href=\"#_the_appendix\">Appendix A: The Appendix</a>\n<ul class=\"sectlevel2\">\n<li><a href=\"#_basics\">Basics</a></li>\n</ul>\n</li>\n</ul>"
    ), "{html}");
}

#[test]
fn test_styled_dlist_class_and_plain_dt() {
    // Any dlist style other than horizontal/qanda joins the wrapper class
    // and drops the hdlist1 class from <dt> (Asciidoctor convert_dlist).
    let html = to_html("[glossary]\nmud:: wet dirt");
    assert!(html.contains("<div class=\"dlist glossary\">\n<dl>\n<dt>mud</dt>"), "{html}");
    // Unstyled dlists keep hdlist1.
    let html = to_html("mud:: wet dirt");
    assert!(html.contains("<div class=\"dlist\">\n<dl>\n<dt class=\"hdlist1\">mud</dt>"), "{html}");
}

#[test]
fn test_style_masqueraded_paragraph_bare_content() {
    // A paragraph masqueraded by a block style carries its text bare —
    // no inner <div class="paragraph"><p> wrapper (unlike partintro above).
    let html = to_html("[example]\nExample para.");
    assert!(html.contains("<div class=\"exampleblock\">\n<div class=\"content\">\nExample para.\n</div>\n</div>"));
    // example%collapsible → <details> with bare content
    let html = to_html("[example%collapsible]\nHidden para.");
    assert!(html.contains("<details>\n<summary class=\"title\">Details</summary>\n<div class=\"content\">\nHidden para.\n</div>\n</details>"));
    // sidebar
    let html = to_html("[sidebar]\nAside text.");
    assert!(html.contains("<div class=\"sidebarblock\">\n<div class=\"content\">\nAside text.\n</div>\n</div>"));
    // quote: bare text directly inside <blockquote>
    let html = to_html("[quote]\nQuoted text.");
    assert!(html.contains("<div class=\"quoteblock\">\n<blockquote>\nQuoted text.\n</blockquote>\n</div>"));
    // open style masquerades a paragraph as an open block; style does not leak into class
    let html = to_html("[open]\nOpen para.");
    assert!(html.contains("<div class=\"openblock\">\n<div class=\"content\">\nOpen para.\n</div>\n</div>"));
    assert!(!html.contains("openblock open"));
    // multi-line paragraph keeps its line break in bare content
    let html = to_html("[example]\nFirst line\nsecond line.");
    assert!(html.contains("<div class=\"content\">\nFirst line\nsecond line.\n</div>"));
    // a real delimited block containing one paragraph keeps the wrapper
    let html = to_html("====\nWrapped para.\n====");
    assert!(html.contains("<div class=\"content\">\n<div class=\"paragraph\">\n<p>Wrapped para.</p>\n</div>\n</div>"));
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
fn test_appendix_numbering_html() {
    // Probe-verified (/tmp/p_appx/p1): appendix subsections chain off the
    // letter (`A.1.`, `A.1.1.`), the `appendix-caption` attribute customizes
    // the label, and the appendix does NOT consume the parent's arabic
    // ordinal — the next regular sibling continues from where it left off.
    let html = to_html(
        "= Article Title\n:appendix-caption: Exhibit\n:sectnums:\n\n== Section\n\n=== Subsection\n\n[appendix]\n== First Appendix\n\n=== First Subsection\n\n==== Deep\n\n=== Second Subsection\n\n[appendix]\n== Second Appendix\n\n== After Appendix",
    );
    assert!(html.contains(">1. Section</h2>"));
    assert!(html.contains(">1.1. Subsection</h3>"));
    assert!(html.contains(">Exhibit A: First Appendix</h2>"));
    assert!(html.contains(">A.1. First Subsection</h3>"));
    assert!(html.contains(">A.1.1. Deep</h4>"));
    assert!(html.contains(">A.2. Second Subsection</h3>"));
    assert!(html.contains(">Exhibit B: Second Appendix</h2>"));
    assert!(html.contains(">2. After Appendix</h2>"));
}

#[test]
fn test_appendix_caption_forms_html() {
    // Unset `:appendix-caption!:` → bare numeral form "A. " (probe p2/p5);
    // the letter shows even without :sectnums: (appendix is always numbered),
    // but subsections are only numbered under :sectnums: (probe p4).
    let html = to_html("= T\n:sectnums:\n:appendix-caption!:\n\n== Section\n\n[appendix]\n== First Appendix\n\n=== Sub");
    assert!(html.contains(">A. First Appendix</h2>"));
    assert!(html.contains(">A.1. Sub</h3>"));

    let html = to_html("= T\n\n== Section\n\n[appendix]\n== First Appendix\n\n=== Sub");
    assert!(html.contains(">Appendix A: First Appendix</h2>"));
    assert!(html.contains(">Sub</h3>"));
    assert!(!html.contains("A.1."));

    // Nested appendix: caption replaces the sectnum on its own heading, but
    // descendants keep the full ancestor chain (probe p7: "1.A.1.").
    let html = to_html("= T\n:sectnums:\n\n== Section\n\n[appendix]\n=== Nested Appendix\n\n==== Sub\n\n== After");
    assert!(html.contains(">Appendix A: Nested Appendix</h3>"));
    assert!(html.contains(">1.A.1. Sub</h4>"));
    assert!(html.contains(">2. After</h2>"));
}

#[test]
fn test_sect0_resets_ordinals_article_not_book_html() {
    // Probe-verified (appendix.adoc corpus file): in an article, a body
    // sect0 restarts its children's per-parent ordinals at 1; in a book,
    // chapters number sequentially across parts (global chapter-number).
    let html = to_html("= T\n:sectnums:\n\n== One\n\n= Part Like\n\n== Chapter");
    assert!(html.contains(">1. One</h2>"));
    assert!(html.contains(">1. Chapter</h2>"));

    let html = to_html("= T\n:doctype: book\n:sectnums:\n\n= First Part\n\n== Chapter\n\n== Second\n\n= Part Two\n\n== Third");
    assert!(html.contains(">1. Chapter</h2>"));
    assert!(html.contains(">2. Second</h2>"));
    assert!(html.contains(">3. Third</h2>"));
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
fn test_dd_empty_principal_with_attached_block_no_paragraph_html() {
    // A description-list item whose principal text is empty but which has an
    // attached block (list, open block, nested dlist) must NOT emit an empty
    // `<p></p>` before the block — Asciidoctor omits the principal paragraph
    // entirely when there is no principal text.

    // Horizontal: empty principal + ulist (sdr-007 case)
    let h = to_html("[horizontal]\nRelated::\n* item one\n* item two");
    assert!(
        h.contains("<td class=\"hdlist2\">\n<div class=\"ulist\">"),
        "horizontal dd with list must not emit empty <p>: {h}"
    );
    assert!(!h.contains("<p>\n</p>"), "no empty <p></p>: {h}");

    // Normal: empty principal + open block (ts-url-format case)
    let o = to_html("Term::\n+\n--\npara inside\n--");
    assert!(
        o.contains("<dd>\n<div class=\"openblock\">"),
        "normal dd with open block must not emit empty <p>: {o}"
    );
    assert!(!o.contains("<p>\n</p>"));

    // Normal: empty principal + nested dlist
    let n = to_html("Term::\nNested::: nested value");
    assert!(
        n.contains("<dd>\n<div class=\"dlist\">"),
        "normal dd with nested dlist must not emit empty <p>: {n}"
    );
    assert!(!n.contains("<p>\n</p>"));

    // Positive: WITH principal text + block keeps the principal `<p>`
    let p = to_html("Term:: principal text\n+\n--\npara inside\n--");
    assert!(
        p.contains("<dd>\n<p>principal text</p>\n<div class=\"openblock\">"),
        "principal text must be preserved before the block: {p}"
    );
}

#[test]
fn test_list_item_literal_paragraph_closes_principal_p_html() {
    // A literal paragraph attached to a list item (indented, no `+` needed)
    // is a separate block: Asciidoctor closes the principal `</p>` BEFORE the
    // literalblock. We previously left the `<p>` open and nested the
    // literalblock inside it, closing `</p>` only after (complex.adoc root A).
    let html = to_html("* A literal paragraph does not require a list continuation.\n\n $ cd projects/my-book");
    assert!(
        html.contains(
            "<p>A literal paragraph does not require a list continuation.</p>\n<div class=\"literalblock\">"
        ),
        "principal <p> must close before the attached literalblock: {html}"
    );
    // The literalblock must not be wrapped inside the principal paragraph.
    assert!(!html.contains("continuation.<div class=\"literalblock\">"), "{html}");
}

#[test]
fn test_list_item_empty_principal_keeps_p_with_block_html() {
    // A regular list item (olist/ulist) whose principal text is empty but
    // which has an attached block keeps an empty `<p></p>` — Asciidoctor always
    // wraps a list-item principal, even when empty (`. {empty}` → `<p></p>`).
    // This is the OPPOSITE of a description `dd`, which rolls the empty `<p>`
    // back (see test_dd_empty_principal_with_attached_block_no_paragraph_html).
    // complex.adoc root B.
    let ol = to_html(". {empty}\n+\n----\nprint(\"one\")\n----");
    assert!(
        ol.contains("<li>\n<p></p>\n<div class=\"listingblock\">"),
        "ordered item empty principal + block must keep <p></p>: {ol}"
    );
    let ul = to_html("* {empty}\n+\n----\nx\n----");
    assert!(
        ul.contains("<li>\n<p></p>\n<div class=\"listingblock\">"),
        "unordered item empty principal + block must keep <p></p>: {ul}"
    );
    // Sanity: an empty-principal item WITHOUT an attached block still emits
    // `<p></p>` (unchanged behaviour).
    let bare = to_html(". {empty}");
    assert!(bare.contains("<li>\n<p></p>\n</li>"), "{bare}");
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
    // The answer is wrapped in <p>…</p> (Asciidoctor convert_dlist qanda).
    assert_eq!(
        html,
        "<div class=\"qlist qanda\">\n<ol>\n\
         <li>\n<p><em>What is Rust?</em></p>\n<p>A systems programming language.</p>\n</li>\n\
         <li>\n<p><em>Why use it?</em></p>\n<p>Memory safety.</p>\n</li>\n\
         </ol>\n</div>\n"
    );
}

#[test]
fn test_qanda_adjacent_terms_grouped_html() {
    // Consecutive `term::` lines sharing one answer collapse into a single
    // <li> with one <p><em>…</em></p> per term; the answer is one <p>.
    // An empty answer leaves just the term paragraph (no answer <p>).
    let html = to_html(
        "[qanda]\nWhat is the answer?::\nThis is the answer.\n\nAre cameras allowed?::\nAre backpacks allowed?::\nNo.",
    );
    assert_eq!(
        html,
        "<div class=\"qlist qanda\">\n<ol>\n\
         <li>\n<p><em>What is the answer?</em></p>\n<p>This is the answer.</p>\n</li>\n\
         <li>\n<p><em>Are cameras allowed?</em></p>\n<p><em>Are backpacks allowed?</em></p>\n<p>No.</p>\n</li>\n\
         </ol>\n</div>\n"
    );
    // Empty answer: term paragraph only, no answer <p>.
    let empty = to_html("[qanda]\nQuestion?::");
    assert_eq!(
        empty,
        "<div class=\"qlist qanda\">\n<ol>\n<li>\n<p><em>Question?</em></p>\n</li>\n</ol>\n</div>\n"
    );
}

#[test]
fn test_horizontal_dlist_colgroup_widths_html() {
    // labelwidth/itemwidth on a horizontal dlist emit a <colgroup> with two
    // <col> elements (Asciidoctor convert_dlist horizontal); each <col> gets a
    // width style only when its own attribute is set.
    let both = to_html("[horizontal,labelwidth=25,itemwidth=75]\nTerm:: desc.");
    assert!(
        both.contains(
            "<table>\n<colgroup>\n<col style=\"width: 25%;\">\n<col style=\"width: 75%;\">\n</colgroup>\n<tr>"
        ),
        "{both}"
    );
    // Only labelwidth → second col is bare; a trailing % in the value is dropped.
    let label_only = to_html("[horizontal,labelwidth=30%]\nTerm:: desc.");
    assert!(
        label_only.contains("<colgroup>\n<col style=\"width: 30%;\">\n<col>\n</colgroup>"),
        "{label_only}"
    );
    // Only itemwidth → first col is bare.
    let item_only = to_html("[horizontal,itemwidth=80]\nTerm:: desc.");
    assert!(
        item_only.contains("<colgroup>\n<col>\n<col style=\"width: 80%;\">\n</colgroup>"),
        "{item_only}"
    );
    // Plain [horizontal] (no widths) emits no colgroup.
    let plain = to_html("[horizontal]\nTerm:: desc.");
    assert!(!plain.contains("<colgroup>"), "{plain}");
    assert!(plain.contains("<table>\n<tr>"), "{plain}");
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
fn test_block_image_interactive_svg_html() {
    // SVG (.svg target) + opts=interactive → <object> with <span class="alt"> fallback
    let html = to_html("image::sample.svg[Interactive,300,opts=interactive]");
    assert!(html.contains("<object type=\"image/svg+xml\" data=\"sample.svg\" width=\"300\"><span class=\"alt\">Interactive</span></object>"), "interactive svg → object. Got:\n{html}");
    // fallback= attribute → <img> fallback (object + fallback both carry width/height)
    let html = to_html("image::sample.svg[Big,300,200,opts=interactive,fallback=alt.png]");
    assert!(html.contains("<object type=\"image/svg+xml\" data=\"sample.svg\" width=\"300\" height=\"200\"><img src=\"alt.png\" alt=\"Big\" width=\"300\" height=\"200\"></object>"), "fallback img. Got:\n{html}");
    // format=svg with a non-.svg target also selects the object path
    let html = to_html("image::diagram[Diag,opts=interactive,format=svg]");
    assert!(html.contains("<object type=\"image/svg+xml\" data=\"diagram\"><span class=\"alt\">Diag</span></object>"), "format=svg → object. Got:\n{html}");
    // a raster image with opts=interactive stays an <img> (object is SVG-only)
    let html = to_html("image::photo.png[Raster,300,opts=interactive]");
    assert!(html.contains("<img src=\"photo.png\" alt=\"Raster\" width=\"300\">"), "raster stays img. Got:\n{html}");
    assert!(!html.contains("<object"), "no object for raster. Got:\n{html}");
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
fn test_inline_image_align_ignored() {
    // Asciidoctor's convert_inline_image emits only float and role in the span
    // class — `align` is not rendered for inline images.
    let html = to_html("text image:icon.png[Icon,align=center] more");
    assert!(html.contains("class=\"image\""));
    assert!(!html.contains("text-center"));
}

#[test]
fn test_inline_image_role_and_title() {
    // role appends to the span class (`image` + float + role); title becomes
    // an attribute on the <img>.
    let html = to_html("image:logo.png[role=\"related thumb right\"]");
    assert!(html.contains("<span class=\"image related thumb right\">"));
    let titled = to_html("image:pause.png[title=Pause]");
    assert!(titled.contains("title=\"Pause\""));
    // float precedes role: `image` + float + role.
    let both = to_html("image:i.png[Icon,float=right,role=screenshot]");
    assert!(both.contains("<span class=\"image right screenshot\">"));
}

#[test]
fn test_block_image_with_link() {
    let html = to_html("image::thumb.jpg[Alt,link=fullsize.jpg]");
    assert!(html.contains("<a class=\"image\" href=\"fullsize.jpg\"><img src=\"thumb.jpg\" alt=\"Alt\"></a>"));
}

#[test]
fn test_block_image_link_from_block_attr_line() {
    // `link=` on the preceding block attribute line wraps the <img> in an anchor.
    let html = to_html("[#img-sunset,link=https://example.com/photo]\nimage::sunset.jpg[Sunset,200,100]");
    assert!(html.contains("<div id=\"img-sunset\" class=\"imageblock\">"));
    assert!(html.contains("<a class=\"image\" href=\"https://example.com/photo\"><img src=\"sunset.jpg\" alt=\"Sunset\" width=\"200\" height=\"100\"></a>"));
}

#[test]
fn test_block_image_trailing_content_is_paragraph() {
    // Trailing content after `]` demotes the block image to a paragraph.
    let html = to_html("image::sunset.jpg[] <.> <.>");
    assert!(html.contains("class=\"paragraph\""));
    assert!(!html.contains("class=\"imageblock\""));
}

#[test]
fn test_block_image_float_align_order() {
    // Class order is fixed: imageblock, float, text-align (then role).
    let html = to_html("image::tiger.png[Tiger,200,200,float=\"right\",align=\"center\"]");
    assert!(html.contains("<div class=\"imageblock right text-center\">"));
}

#[test]
fn test_block_image_imagesdir_prefix() {
    // A non-empty imagesdir is prefixed to a non-URI target; a URI target and a
    // subsequent reset are honored live (mid-document attribute state).
    let html = to_html(":imagesdir: https://cdn.example.com/img\n\nimage::a/b.svg[Pic,10,10]");
    assert!(html.contains("src=\"https://cdn.example.com/img/a/b.svg\""));
    // URI targets ignore imagesdir.
    let uri = to_html(":imagesdir: https://cdn.example.com/img\n\nimage::https://other.example.com/x.png[X]");
    assert!(uri.contains("src=\"https://other.example.com/x.png\""));
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
    let html = to_html(":source-highlighter: rouge\n\n[source,rust,%linenums]\n----\nfn main() {}\n----");
    assert!(html.contains("linenums"), "linenums option should add linenums class. Got: {html}");
    assert!(html.contains("rouge highlight"), "rouge highlight classes should be present. Got: {html}");
    assert!(html.contains("<table class=\"linenotable\">"), "linenums should produce linenotable. Got: {html}");
}

#[test]
fn test_source_block_linenums_needs_build_time_highlighter() {
    // Asciidoctor renders line numbers only under a build-time highlighter
    // (rouge/pygments/coderay); without one, or with the client-side
    // highlight.js, the option is ignored entirely.
    let html = to_html("[source,rust,%linenums]\n----\nfn main() {}\n----");
    assert!(!html.contains("linenums"), "no highlighter ignores linenums. Got: {html}");
    assert!(!html.contains("linenotable"), "no highlighter gives no table. Got: {html}");
    let html = to_html(":source-highlighter: highlight.js\n\n[source,rust,%linenums]\n----\nfn main() {}\n----");
    assert!(!html.contains("linenotable"), "highlight.js gives no table. Got: {html}");
    assert!(html.contains("highlightjs highlight"), "highlightjs classes stay. Got: {html}");
}

#[test]
fn test_source_block_linenums_basic() {
    let html = to_html(":source-highlighter: rouge\n\n[source,ruby,%linenums]\n----\nputs \"Hello\"\nx = 42\nputs x\n----");
    assert!(html.contains("<td class=\"linenos\"><pre class=\"linenos\">1\n2\n3</pre></td>"), "should have line numbers 1-3. Got: {html}");
    assert!(html.contains("<td class=\"code\"><pre>puts \"Hello\"\nx = 42\nputs x</pre></td>"), "should have code in td. Got: {html}");
}

#[test]
fn test_source_block_linenums_start() {
    let html = to_html(":source-highlighter: rouge\n\n[source,ruby,%linenums,start=10]\n----\nputs \"Hello\"\nx = 42\nputs x\n----");
    assert!(html.contains("<td class=\"linenos\"><pre class=\"linenos\">10\n11\n12</pre></td>"), "should have line numbers 10-12. Got: {html}");
}

#[test]
fn test_source_block_linenums_with_highlight() {
    let html = to_html(":source-highlighter: rouge\n\n[source,rust,%linenums,highlight=2]\n----\nlet a = 1;\nlet b = 2;\nlet c = 3;\n----");
    assert!(html.contains("<table class=\"linenotable\">"), "should have linenotable. Got: {html}");
    assert!(html.contains("<span class=\"hll\">let b = 2;</span>"), "should have highlight span in code. Got: {html}");
    assert!(html.contains("<td class=\"code\">"), "should have code td. Got: {html}");
}

#[test]
fn test_source_block_linenums_single_line() {
    let html = to_html(":source-highlighter: rouge\n\n[source,ruby,%linenums]\n----\nputs \"hi\"\n----");
    assert!(html.contains("<pre class=\"linenos\">1</pre>"), "single line should have just 1. Got: {html}");
}

#[test]
fn test_source_block_linenums_with_callouts() {
    let html = to_html(":source-highlighter: rouge\n\n[source,ruby,%linenums]\n----\nputs \"Hello\" <1>\nx = 42 <2>\n----");
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
fn test_monospace_replacements_and_char_refs_html() {
    // Constrained monospace `` `text` `` undergoes the full normal substitution group
    // (Asciidoctor), so char-refs are restored and `(C)`/word-flanked `--` are replaced
    // inside `<code>`; a standalone `--` at the span edge stays literal.
    let html = to_html("`&#167;` and `(C)` and `x--y` and `--`");
    assert!(html.contains("<code>&#167;</code>"), "char-ref preserved in monospace. Got: {html}");
    assert!(html.contains("<code>\u{00A9}</code>"), "(C) replaced in monospace. Got: {html}");
    assert!(html.contains("<code>x\u{2014}\u{200B}y</code>"), "word-flanked em-dash in monospace. Got: {html}");
    assert!(html.contains("<code>--</code>"), "edge `--` stays literal in monospace. Got: {html}");
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
fn test_revision_line_freeform_and_after_attr_entries() {
    let opts = HtmlOptions { standalone: true, ..Default::default() };
    // Probe-verified vs Asciidoctor (/tmp/p_meta): attribute entries between
    // the author and revision lines are transparent (metadata.adoc corpus
    // root) — the next non-attr line is still the revision line.
    let html = to_html_with_options(
        "= T\nAnne Bell\n:k: v\nv2.0, 2020-01-01\n\nbody",
        opts.clone(),
    );
    assert!(html.contains("<span id=\"revnumber\">version 2.0,</span>"), "Got: {html}");
    assert!(html.contains("<span id=\"revdate\">2020-01-01</span>"), "Got: {html}");
    // A freeform line (no comma/colon/v-prefix) is consumed as the revdate.
    let html = to_html_with_options("= T\nAnne Bell\njust some words\n\nbody", opts.clone());
    assert!(html.contains("<span id=\"revdate\">just some words</span>"), "Got: {html}");
    assert!(!html.contains("<p>just some words</p>"), "Got: {html}");
    // A comma with no digits before it SETS an empty revnumber (`version ,`).
    let html = to_html_with_options("= T\nAnne Bell\nhello, world\n\nbody", opts.clone());
    assert!(html.contains("<span id=\"revnumber\">version ,</span>"), "Got: {html}");
    assert!(html.contains("<span id=\"revdate\">world</span>"), "Got: {html}");
    // A trailing bare colon sets an EMPTY revremark span.
    let html = to_html_with_options("= T\nAnne Bell\n2020-01-01:\n\nbody", opts.clone());
    assert!(html.contains("<br><span id=\"revremark\"></span>"), "Got: {html}");
    // A line whose component starts with a colon is thrown back to the body.
    let html = to_html_with_options("= T\nAnne Bell\n:weird\n\nbody", opts.clone());
    assert!(html.contains("<p>:weird</p>"), "Got: {html}");
    assert!(!html.contains("<span id=\"revremark\""), "Got: {html}");
    // A section-marker-shaped line directly after the title is the AUTHOR
    // (header runs to the first blank line) — probe p14.
    let html = to_html_with_options("= T\n== Sec\n\nbody", opts);
    assert!(html.contains("<span id=\"author\" class=\"author\">== Sec</span>"), "Got: {html}");
    assert!(!html.contains("<h2"), "Got: {html}");
}

#[test]
fn test_revision_attrs_from_attribute_entries() {
    let opts = HtmlOptions { standalone: true, ..Default::default() };
    // Revision spans are attribute-driven: header attribute entries alone produce
    // them, and the explicit value keeps its `v` prefix (no revision-line strip).
    let html = to_html_with_options(
        "= T\nA U\n:revnumber: v8.3\n:revdate: July 29, 2025\n:revremark: Summertime!\n\nbody",
        opts.clone(),
    );
    assert!(html.contains("<span id=\"revnumber\">version v8.3,</span>"), "Got: {html}");
    assert!(html.contains("<span id=\"revdate\">July 29, 2025</span>"), "Got: {html}");
    assert!(html.contains("<br><span id=\"revremark\">Summertime!</span>"), "Got: {html}");
    // No author required: a lone revdate still opens the details div.
    let html = to_html_with_options("= T\n:revdate: 2025-07-29\n\nbody", opts.clone());
    assert!(html.contains("<div class=\"details\">"), "Got: {html}");
    assert!(html.contains("<span id=\"revdate\">2025-07-29</span>"), "Got: {html}");
    assert!(!html.contains("<span id=\"revnumber\""), "Got: {html}");
    // An attribute entry overrides the revision line; `:!revdate:` removes the
    // span AND the trailing comma after the version.
    let html = to_html_with_options(
        "= T\nA U\nv1.0, 2020-01-01\n:revnumber: 9.9\n:!revdate:\n\nbody",
        opts.clone(),
    );
    assert!(html.contains("<span id=\"revnumber\">version 9.9</span>"), "Got: {html}");
    assert!(!html.contains("<span id=\"revdate\""), "Got: {html}");
    // A body attribute entry does NOT reach the header details.
    let html = to_html_with_options("= T\n\nbody\n\n:revnumber: 5.5\n\nmore", opts);
    assert!(!html.contains("<div class=\"details\">"), "Got: {html}");
}

#[test]
fn test_revision_attr_refs_resolved_in_details() {
    // Attribute references in the revision-line components resolve against the
    // document attributes (Asciidoctor applies header substitutions as the line
    // is read); undefined refs stay literal. `docdate` arrives here as an
    // API-level attribute, like the intrinsics the CLI seeds from the input
    // file's mtime.
    let mut attributes = HashMap::new();
    attributes.insert("docdate".to_string(), "2026-03-15".to_string());
    let html = to_html_with_options(
        "= T\nA U\nLPR55, {docdate}: Edition {undefinedx}\n\nbody",
        HtmlOptions { standalone: true, attributes, ..Default::default() },
    );
    // The revision-line parse strips the non-digit version prefix ("LPR").
    assert!(html.contains("<span id=\"revnumber\">version 55,</span>"), "Got: {html}");
    assert!(html.contains("<span id=\"revdate\">2026-03-15</span>"), "Got: {html}");
    assert!(html.contains("<span id=\"revremark\">Edition {undefinedx}</span>"), "Got: {html}");
}

#[test]
fn test_passthrough_block_bare_content_no_stray_div() {
    // A standalone passthrough block emits its content bare — no wrapper is
    // opened, so nothing must be closed (was: a stray `</div>`).
    let html = to_html("++++\n<video x=\"1\">\n</video>\n++++");
    assert!(!html.contains("</div>"), "Got: {html}");
    assert!(html.contains("<video x=\"1\">\n</video>\n"), "Got: {html}");
    // `[pass]`-style paragraph: same bare emission; the following block is
    // unaffected.
    let html = to_html("[pass]\n<del>a</del> b.\n\nnext para");
    assert!(
        html.contains("<del>a</del> b.\n<div class=\"paragraph\">\n<p>next para</p>\n</div>"),
        "Got: {html}"
    );
}

#[test]
fn test_author_attrs_from_attribute_entries() {
    let opts = HtmlOptions { standalone: true, ..Default::default() };
    // `:author:`/`:email:` header entries alone produce the detail spans,
    // derive firstname/middlename/lastname/authorinitials, and the section
    // auto-id is generated from the title with attribute refs resolved.
    let html = to_html_with_options(
        "= T\n:author: Kismet R. Lee\n:email: kismet@asciidoctor.org\n\n== About {author}\n\n{firstname}/{middlename}/{lastname}/{authorinitials}",
        opts.clone(),
    );
    assert!(html.contains("<span id=\"author\" class=\"author\">Kismet R. Lee</span>"), "Got: {html}");
    assert!(html.contains("<span id=\"email\" class=\"email\"><a href=\"mailto:kismet@asciidoctor.org\">"), "Got: {html}");
    assert!(html.contains("<h2 id=\"_about_kismet_r_lee\">About Kismet R. Lee</h2>"), "Got: {html}");
    assert!(html.contains("Kismet/R./Lee/KRL"), "derived attrs. Got: {html}");
    // The entry overrides the author line and re-derives the names (the
    // rescan clobbers an explicit `:firstname:`), and underscores in the
    // value become spaces in the recomposed fullname.
    let html = to_html_with_options(
        "= T\nReal Author <real@x.org>\n:author: Mara_Moss Wirribi\n:firstname: Manual\n\n{firstname}/{lastname}/{authorinitials}",
        opts.clone(),
    );
    assert!(html.contains("<span id=\"author\" class=\"author\">Mara Moss Wirribi</span>"), "Got: {html}");
    assert!(html.contains("Mara Moss/Wirribi/MW"), "re-derived names. Got: {html}");
    // An explicit `:authorinitials:` differing from the line-derived value
    // survives the rescan.
    let html = to_html_with_options(
        "= T\n:author: Mary Sue Jones\n:authorinitials: XX\n\n{authorinitials}",
        opts.clone(),
    );
    assert!(html.contains("<p>XX</p>"), "explicit initials win. Got: {html}");
    // `:email:` alone opens no details (author required); `:!author:` after an
    // author line suppresses the whole div.
    let html = to_html_with_options("= T\n:email: solo@x.org\n\nbody", opts.clone());
    assert!(!html.contains("<div class=\"details\">"), "Got: {html}");
    let html = to_html_with_options("= T\nReal Author\n:!author:\n\nbody", opts.clone());
    assert!(!html.contains("<div class=\"details\">"), "Got: {html}");
    // A mid-document `:author:` derives nothing and opens no details.
    let html = to_html_with_options("= T\n\nbody\n\n:author: Mid Document\n\n{firstname}|{author}", opts);
    assert!(!html.contains("<div class=\"details\">"), "Got: {html}");
    assert!(html.contains("{firstname}|Mid Document"), "no mid-doc derivation. Got: {html}");
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
fn test_attr_reference_with_bang_stays_literal() {
    // `{name!…}` is not a reference in Asciidoctor (`!` is outside the name
    // charset) — the braces stay literal even when the attribute is defined
    // (probe-verified).
    let html = to_html("{undefined!fallback value}");
    assert!(html.contains("<p>{undefined!fallback value}</p>"),
        "bang reference should stay literal. Got: {html}");

    let html = to_html(":name: real\n\n{name!fallback}");
    assert!(html.contains("<p>{name!fallback}</p>"),
        "bang reference stays literal even for a defined attr. Got: {html}");

    let html = to_html("{undefined!}");
    assert!(html.contains("<p>{undefined!}</p>"),
        "bang reference should stay literal. Got: {html}");
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
fn test_quoted_paragraph_shorthand() {
    // Probe-verified vs asciidoctor (/tmp/p_subs/p11): a paragraph wrapped in
    // quotes followed by `-- attribution, citetitle` becomes a quote block
    // with BARE content (no paragraph wrapper) and an attribution trailer.
    let html = to_html("\"Two line quote,\nsecond line.\"\n-- Thomas Jefferson, Papers Volume 11");
    assert!(html.contains("<div class=\"quoteblock\">\n<blockquote>\nTwo line quote,\nsecond line.\n</blockquote>"),
        "bare content in blockquote. Got: {html}");
    assert!(html.contains("<div class=\"attribution\">\n&#8212; Thomas Jefferson<br>\n<cite>Papers Volume 11</cite>"),
        "attribution + citetitle. Got: {html}");
    // attribution without citetitle
    let html = to_html("\"Q.\"\n-- Solo Author");
    assert!(html.contains("&#8212; Solo Author\n</div>"), "no <br>/<cite>. Got: {html}");
    // a quoted paragraph without the credit line stays a plain paragraph
    let html = to_html("\"Just quoted text.\"");
    assert!(html.contains("<p>\"Just quoted text.\"</p>"), "plain paragraph. Got: {html}");
}

#[test]
fn test_markdown_blockquote() {
    // Probe-verified vs asciidoctor (/tmp/p_subs/p11): `>`-prefixed lines are
    // a quote block with COMPOUND content (paragraph wrappers kept); the
    // trailing `-- ...` line becomes the attribution.
    let html = to_html("> Md quote line one,\n> line two.\n> -- Author Name, Cite Source");
    assert!(html.contains("<blockquote>\n<div class=\"paragraph\">\n<p>Md quote line one,\nline two.</p>\n</div>\n</blockquote>"),
        "paragraph wrapper inside blockquote. Got: {html}");
    assert!(html.contains("&#8212; Author Name<br>\n<cite>Cite Source</cite>"), "attribution. Got: {html}");
    // without attribution
    let html = to_html("> Bare md quote\n> no attribution.");
    assert!(html.contains("quoteblock") && !html.contains("<div class=\"attribution\">"), "no attribution div. Got: {html}");
    // a stripped bare `>` separates paragraphs; nested `> >` nests a quote
    let html = to_html("> > Inner quote\n>\n> Outer para");
    assert!(html.contains("<blockquote>\n<div class=\"quoteblock\">\n<blockquote>\n<div class=\"paragraph\">\n<p>Inner quote</p>"),
        "nested quoteblock. Got: {html}");
    assert!(html.contains("<p>Outer para</p>"), "second paragraph. Got: {html}");
}

#[test]
fn test_single_quoted_attrlist_value_gets_subs() {
    // Probe-verified vs asciidoctor (/tmp/p_subs/p12): only SINGLE-quoted
    // attrlist values receive normal substitutions; double-quoted and bare
    // values stay literal (escaped).
    let html = to_html("[quote,Auth,'cite with https://e.org[L] and *b*']\n____\nq\n____");
    assert!(html.contains("<cite>cite with <a href=\"https://e.org\">L</a> and <strong>b</strong></cite>"),
        "single-quoted citetitle gets subs. Got: {html}");
    let html = to_html("[quote,Auth,\"double https://e.org[L] and *b*\"]\n____\nq\n____");
    assert!(html.contains("<cite>double https://e.org[L] and *b*</cite>"),
        "double-quoted stays literal. Got: {html}");
    // single-quoted value also protects its comma (one citetitle, not two slots)
    let html = to_html("[quote,Auth,'one, two']\n____\nq\n____");
    assert!(html.contains("<cite>one, two</cite>"), "comma protected. Got: {html}");
    // mid-word apostrophe is not a quote opener
    let html = to_html("[quote,Dad's words]\n____\nq\n____");
    assert!(html.contains("&#8212; Dad's words"), "apostrophe is plain text (no subs on bare values). Got: {html}");
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
fn test_env_attribute_missing_var_with_bang_literal() {
    // `!` is outside the reference-name charset, so this is not a reference
    // at all — literal braces, no env lookup, no fallback.
    let html = to_html("Value: {env-ADOC_PARSER_TEST_VAR_12345!fallback}");
    assert!(html.contains("Value: {env-ADOC_PARSER_TEST_VAR_12345!fallback}"),
        "bang form should stay literal. Got: {html}");
}

#[test]
fn test_unknown_inline_macro_stays_literal() {
    // Asciidoctor matches only registered inline macro names — an unknown
    // `name:target[attrs]` form is plain text (probe-verified).
    let html = to_html("chart:sales[Q1,Q2]");
    assert!(html.contains("<p>chart:sales[Q1,Q2]</p>"),
        "unknown inline macro should stay literal. Got: {html}");
    assert!(!html.contains("custom-macro"),
        "no custom-macro span for unknown inline macro. Got: {html}");
    // The bracket interior still flows through normal substitutions:
    // foo:bar[*b*] → foo:bar[<strong>b</strong>] (probe-verified).
    let html = to_html("foo:bar[*b*]");
    assert!(html.contains("<p>foo:bar[<strong>b</strong>]</p>"),
        "unknown macro interior gets normal subs. Got: {html}");
    // A `word: …` prose pattern must never be misread as a macro, even with
    // brackets later on the line.
    let html = to_html("Mono with content: `+abc+` [not macro].");
    assert!(html.contains("<p>Mono with content: <code>abc</code> [not macro].</p>"),
        "prose colon must not be misread as a macro. Got: {html}");
}

#[test]
fn test_unknown_block_macro_stays_literal() {
    // Asciidoctor matches only registered block macro names — an unknown
    // `name::target[attrs]` line is a plain paragraph (probe-verified).
    let html = to_html("chart::sales-data[type=bar]");
    assert!(html.contains("<p>chart::sales-data[type=bar]</p>"),
        "unknown block macro should stay a literal paragraph. Got: {html}");
    assert!(!html.contains("custom-macro"),
        "no custom-macro div for unknown block macro. Got: {html}");

    // A preceding `.Title` attaches to the resulting paragraph.
    let html = to_html(".Exponential growth\nstem::[x_0(1 + r)^2]");
    assert!(html.contains("<div class=\"title\">Exponential growth</div>"),
        "title should attach to the paragraph. Got: {html}");
    assert!(html.contains("<p>stem::[x_0(1 + r)^2]</p>"),
        "stem:: block form is not a macro. Got: {html}");
}

#[test]
fn test_stem_inline_escaped_brackets_html() {
    // `\]` inside stem:[…] does not close the macro and is unescaped
    // (probe-verified: → \$[[a,b],[c,d]]((n),(k))\$).
    let html = to_html(":stem:\n\nA matrix can be written as stem:[[[a,b\\],[c,d\\]\\]((n),(k))].");
    assert!(html.contains(r"\$[[a,b],[c,d]]((n),(k))\$"),
        "escaped brackets should be unescaped inside stem content. Got: {html}");
}

#[test]
fn test_empty_double_plus_passthrough_html() {
    // `++++` inline is an empty passthrough — renders as nothing (probe-verified).
    let html = to_html("para with ++++ inline.");
    assert!(html.contains("<p>para with  inline.</p>"),
        "++++ should collapse to nothing. Got: {html}");
}

#[test]
fn test_double_plus_passthrough_escapes_specialchars_html() {
    // Double-plus `++…++` applies the specialcharacters sub: `<`/`>`/`&` are escaped,
    // unlike triple-plus (raw). Asciidoctor: `++[<LABEL>]++` → `[&lt;LABEL&gt;]`.
    let html = to_html("a ++[<LABEL>]++ b");
    assert!(html.contains("<p>a [&lt;LABEL&gt;] b</p>"),
        "++…++ should escape < and >. Got: {html}");

    // In a monospace (`m`) table column the escaped passthrough sits inside <code>.
    let html = to_html("[cols=1m]\n|===\n|++[<LABEL>]++\n|===");
    assert!(html.contains("<code>[&lt;LABEL&gt;]</code>"),
        "++…++ in m-column should escape inside <code>. Got: {html}");

    // Triple-plus stays raw — the angle brackets pass through unescaped.
    let html = to_html("a +++[<LABEL>]+++ b");
    assert!(html.contains("<p>a [<LABEL>] b</p>"),
        "+++…+++ should pass through raw. Got: {html}");
}

#[test]
fn test_unknown_inline_macro_empty_attrs_stays_literal() {
    let html = to_html("widget:component[]");
    assert!(html.contains("<p>widget:component[]</p>"),
        "unknown inline macro with empty attrs should stay literal. Got: {html}");
}

#[test]
fn test_pass_macro_subs_spec() {
    // pass:SPEC[content] applies exactly the spec'd substitutions
    // (all cases probe-verified against asciidoctor).
    let html = to_html("A pass:c[<b>not bold</b>] B.");
    assert!(html.contains("<p>A &lt;b&gt;not bold&lt;/b&gt; B.</p>"),
        "pass:c escapes specialchars, no formatting. Got: {html}");

    let html = to_html("C pass:q[*bold* and `mono`] D.");
    assert!(html.contains("<p>C <strong>bold</strong> and <code>mono</code> D.</p>"),
        "pass:q runs quotes only. Got: {html}");

    let html = to_html("I pass:n[<x> *b*] J.");
    assert!(html.contains("<p>I &lt;x&gt; <strong>b</strong> J.</p>"),
        "pass:n runs the normal set. Got: {html}");

    let html = to_html("O pass:v[<y>] P.");
    assert!(html.contains("<p>O &lt;y&gt; P.</p>"),
        "pass:v (verbatim) escapes specialchars. Got: {html}");

    let html = to_html("G pass:quotes[*b*] H.");
    assert!(html.contains("<p>G <strong>b</strong> H.</p>"),
        "full sub names are accepted in the spec. Got: {html}");

    // q without specialchars: raw markup passes through unescaped.
    let html = to_html("M pass:q[<b>x</b>] N.");
    assert!(html.contains("<p>M <b>x</b> N.</p>"),
        "pass:q must not escape specialchars. Got: {html}");

    // No bracket after the spec — not a macro, literal text.
    let html = to_html("Bare pass:c here.");
    assert!(html.contains("<p>Bare pass:c here.</p>"),
        "pass:c without brackets stays literal. Got: {html}");

    // Single-plus passthrough extracts the spec'd macro too.
    let html = to_html("S +pass:c[<b>]+ T.");
    assert!(html.contains("<p>S &lt;b&gt; T.</p>"),
        "pass:c inside +…+ is extracted and escaped. Got: {html}");
}

#[test]
fn test_escaped_pass_macro_with_spec() {
    // \pass:SPEC[…] drops the backslash; "pass:SPEC[" stays literal and the
    // content plus trailing "]" flow through normal subs (probe-verified).
    // The corpus case: `\pass:c[]` inside monospace (literal-monospace.adoc).
    let html = to_html("The `\\pass:c[]` enclosure.");
    assert!(html.contains("<p>The <code>pass:c[]</code> enclosure.</p>"),
        "escaped pass:c[] in monospace renders literally. Got: {html}");

    let html = to_html("E \\pass:c[*b*] F.");
    assert!(html.contains("<p>E pass:c[<strong>b</strong>] F.</p>"),
        "escaped pass content still gets normal subs. Got: {html}");

    // Only one backslash takes part in the escape.
    let html = to_html("Double `\\\\pass:c[abc]` tail.");
    assert!(html.contains("<p>Double <code>\\pass:c[abc]</code> tail.</p>"),
        "double-escaped pass keeps one literal backslash. Got: {html}");
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
fn test_unknown_macro_with_hyphen_underscore_name_stays_literal() {
    let html = to_html("my-custom_macro:target[attrs]");
    assert!(html.contains("<p>my-custom_macro:target[attrs]</p>"),
        "unknown macro names with hyphen/underscore stay literal. Got: {html}");
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

#[test]
fn test_subs_trailing_plus_and_attr_value_pass_macro() {
    // `subs=attributes+` — the trailing plus is asciidoctor's prepend
    // modifier: the verbatim defaults (specialchars) are KEPT and
    // attributes is added.
    let html = to_html(":rv: 1.2.3\n\n[source,xml,subs=attributes+]\n----\n<v>{rv}</v>\n----");
    assert!(html.contains("&lt;v&gt;1.2.3&lt;/v&gt;"),
        "attributes+ must keep specialchars and resolve refs. Got: {html}");

    // Plain token in a mixed list REPLACES the defaults (asciidoctor
    // resolve_subs): "quotes,+attributes" drops specialchars.
    let html = to_html(":rv: 1.2.3\n\n[source,xml,subs=\"quotes,+attributes\"]\n----\n<q>*b* {rv}</q>\n----");
    assert!(html.contains("<q><strong>b</strong> 1.2.3</q>"),
        "plain token must seed empty set (no specialchars). Got: {html}");

    // Full-value `pass:a[...]` attribute entry: attributes resolve at
    // definition time; an undefined ref stays literal and is NOT re-scanned
    // at use (asciidoctor apply_attribute_value_subs).
    let html = to_html(":release-version: pass:a[{release-version}]\n\nv={release-version}");
    assert!(html.contains("v={release-version}"),
        "self-protected pass:a value must stay literal. Got: {html}");
    assert!(!html.contains("pass:"), "pass wrapper must be stripped. Got: {html}");

    // pass:a resolves refs that ARE defined at definition time.
    let html = to_html(":a: one\n:b: pass:a[{a} two]\n:a: changed\n\nb={b}");
    assert!(html.contains("b=one two"),
        "pass:a must resolve refs at definition time. Got: {html}");

    // Bare `pass:[...]` values keep the wrapper: the inline pass macro
    // handles it at use, inserting the content verbatim.
    let html = to_html(":v: pass:[<em>x</em>]\n\nval {v} end");
    assert!(html.contains("val <em>x</em> end"),
        "bare pass value must insert content verbatim at use. Got: {html}");
}

#[test]
fn test_self_referential_attribute_no_recursion() {
    // `:x: {x}` defines x as the literal text `{x}` (undefined at definition
    // time). Using `{x}` must emit the literal, not recurse — this used to
    // stack-overflow.
    let html = to_html(":x: {x}\n\nval {x} end");
    assert!(html.contains("val {x} end"),
        "self-referential attribute must render literally. Got: {html}");

    // Mutual recursion through render_inline_value must also terminate.
    let html = to_html(":a: {b}\n:b: {a}\n\nval {a} end");
    assert!(html.contains("val"), "mutual recursion must terminate. Got: {html}");
}

#[test]
fn test_example_caption_document_attribute() {
    // Default label, numbered.
    let html = to_html(".Optional title\n====\ncontent\n====");
    assert!(html.contains("<div class=\"title\">Example 1. Optional title</div>"), "default Example label. Got: {html}");
    // Unset attribute suppresses the prefix entirely.
    let html = to_html(":!example-caption:\n\n.Optional title\n====\ncontent\n====");
    assert!(html.contains("<div class=\"title\">Optional title</div>"), "unset example-caption gives bare title. Got: {html}");
    // Custom label keeps the shared counter.
    let html = to_html(":example-caption: Demo\n\n.A\n====\nx\n====\n\n.B\n====\ny\n====");
    assert!(html.contains("<div class=\"title\">Demo 1. A</div>"), "custom label first. Got: {html}");
    assert!(html.contains("<div class=\"title\">Demo 2. B</div>"), "custom label second. Got: {html}");
}

#[test]
fn test_attrlist_shorthand_only_in_first_position() {
    // Shorthand markers inside later comma-parts are verbatim positional
    // text: the quote attribution keeps its periods and nothing leaks into
    // the wrapper class (matches Asciidoctor).
    let html = to_html("[quote#roads,Dr. Emmett Brown,Back to the Future]\nRoads.");
    assert!(html.contains("<div id=\"roads\" class=\"quoteblock\">"), "id from first-part shorthand, no role leak. Got: {html}");
    assert!(html.contains("&#8212; Dr. Emmett Brown<br>"), "attribution stays whole. Got: {html}");
    assert!(html.contains("<cite>Back to the Future</cite>"), "citetitle stays whole. Got: {html}");
    // Pure shorthand in the second part is attribution text, not an id/role.
    let html = to_html("[quote,#bar]\nText.");
    assert!(html.contains("&#8212; #bar"), "verbatim #bar attribution. Got: {html}");
    assert!(!html.contains("id=\"bar\""), "no id from second-part shorthand. Got: {html}");
}

#[test]
fn test_comment_after_list_entry_keeps_single_list() {
    // A blank line before a dlist/colist entry must not arm the
    // "comment separates lists" rule: a comment directly after that
    // entry's text (no blank in between) keeps a single list.
    let html = to_html("a:: text a\n\nb:: text b\n// comment\n\nc:: text c");
    assert_eq!(html.matches("<div class=\"dlist\">").count(), 1, "single dlist. Got: {html}");
    let html = to_html("----\nx <1>\ny <2>\nz <3>\n----\n<1> one\n\n<2> two\n// comment\n\n<3> three");
    assert_eq!(html.matches("<div class=\"colist").count(), 1, "single colist. Got: {html}");
    // Comment after a blank line still separates (unchanged behavior).
    let html = to_html("a:: text a\n\n// comment\nb:: text b");
    assert_eq!(html.matches("<div class=\"dlist\">").count(), 2, "comment after blank splits. Got: {html}");
}

#[test]
fn test_line_comment_mid_paragraph_merges_lines() {
    // Asciidoctor reads paragraph-ish content with skip_line_comments: a `//`
    // line inside the text is dropped and the lines merge (probe-verified).
    let html = to_html("import foo\n// tag::classdef[]\nclass Bar {");
    assert!(html.contains("<p>import foo\nclass Bar {</p>"), "paragraph merges. Got: {html}");
    let html = to_html("NOTE: a\n// c\nb");
    assert!(html.contains("a\nb"), "admonition merges. Got: {html}");
    let html = to_html("t:: a\n// c\nb");
    assert!(html.contains("<p>a\nb</p>"), "dd merges. Got: {html}");
    let html = to_html(". a\n// c\nb");
    assert!(html.contains("<p>a\nb</p>"), "olist item merges. Got: {html}");
    // Verse paragraphs keep comment lines as content (verbatim).
    let html = to_html("[verse]\na\n// c\nb");
    assert!(html.contains("a\n// c\nb"), "verse keeps comment. Got: {html}");
    // A comment followed by a blank line still ends the paragraph.
    let html = to_html("a\n// c\n\nb");
    assert!(html.contains("<p>a</p>") && html.contains("<p>b</p>"), "blank still splits. Got: {html}");
}

#[test]
fn test_autolink_boundary_and_trailing_paren() {
    // InlineLinkRx boundary: a bare URL only autolinks after start-of-text,
    // whitespace, or one of `<>()[];` (probe-verified). A literal
    // `include::https://…[]` line (from an escaped include) stays plain text.
    let html = to_html("see https://x.example near");
    assert!(html.contains("<a href=\"https://x.example\""), "after space links. Got: {html}");
    let html = to_html("x include::https://x.example/a.adoc[] y");
    assert!(!html.contains("<a "), "after colon stays literal. Got: {html}");
    let html = to_html("word-https://x.example near");
    assert!(!html.contains("<a "), "after dash stays literal. Got: {html}");
    let html = to_html("a\"https://x.example\" near");
    assert!(!html.contains("<a "), "after straight quote stays literal. Got: {html}");
    // Trailing `)` is never part of a bare URL (all trailing ones strip).
    let html = to_html("(https://x.example) near");
    assert!(html.contains("<a href=\"https://x.example\""), "paren-wrapped links. Got: {html}");
    assert!(!html.contains("x.example)"), "closing paren outside url. Got: {html}");
    // Escaped bare autolink: backslash drops, URL stays literal — but only at
    // a valid boundary; after a non-boundary char the backslash stays too.
    let html = to_html("see \\https://x.example z");
    assert!(html.contains("see https://x.example z") && !html.contains("<a "), "escape drops backslash. Got: {html}");
    let html = to_html("code `\\https://x.example?a=b` z");
    assert!(html.contains("<code>https://x.example?a=b</code>"), "escape inside monospace. Got: {html}");
    let html = to_html("word-\\https://x.example z");
    assert!(html.contains("word-\\https://x.example"), "no boundary keeps backslash. Got: {html}");
}

#[test]
fn test_table_width_attribute() {
    // width != 100 → inline style instead of stretch (probe-verified)
    let html = to_html("[cols=\"1,1\",width=50%]\n|===\n|a |b\n|===");
    assert!(html.contains("<table class=\"tableblock frame-all grid-all\" style=\"width: 50%;\">"), "explicit width → style. Got:\n{html}");
    // width=100% → stretch class, no style
    let html = to_html("[cols=\"1,1\",width=100%]\n|===\n|a |b\n|===");
    assert!(html.contains("<table class=\"tableblock frame-all grid-all stretch\">"));
    // bare number, Ruby to_i semantics (33 → 33%)
    let html = to_html("[cols=\"1,1\",width=33]\n|===\n|a |b\n|===");
    assert!(html.contains("style=\"width: 33%;\""));
    // out-of-range / non-numeric fall back to 100 → stretch
    let html = to_html("[cols=\"1,1\",width=150]\n|===\n|a |b\n|===");
    assert!(html.contains("stretch"));
    let html = to_html("[cols=\"1,1\",width=abc]\n|===\n|a |b\n|===");
    assert!(html.contains("stretch"));
    // %autowidth + explicit width → no fit-content, style wins; bare <col>
    let html = to_html("[%autowidth,width=50%]\n|===\n|a |b\n|===");
    assert!(html.contains("<table class=\"tableblock frame-all grid-all\" style=\"width: 50%;\">"));
    assert!(html.contains("<col>\n<col>"));
    // %autowidth + width=100% → stretch (not fit-content)
    let html = to_html("[%autowidth,width=100%]\n|===\n|a |b\n|===");
    assert!(html.contains("stretch"));
    // no width: autowidth → fit-content, otherwise stretch (unchanged)
    let html = to_html("[%autowidth]\n|===\n|a |b\n|===");
    assert!(html.contains("fit-content"));
}

#[test]
fn test_table_frame_grid_classes_html() {
    // frame/grid named attrs emit frame-{val} grid-{val} (html5.rb:859-860)
    let html = to_html("[frame=ends,grid=none]\n|===\n|a\n|===");
    assert!(html.contains("<table class=\"tableblock frame-ends grid-none stretch\">"), "frame=ends grid=none. Got:\n{html}");
    // topbot is aliased to ends
    let html = to_html("[frame=topbot]\n|===\n|a\n|===");
    assert!(html.contains("<table class=\"tableblock frame-ends grid-all stretch\">"), "topbot→ends. Got:\n{html}");
    // sides/cols emitted verbatim
    let html = to_html("[frame=sides,grid=cols]\n|===\n|a\n|===");
    assert!(html.contains("<table class=\"tableblock frame-sides grid-cols stretch\">"), "frame=sides grid=cols. Got:\n{html}");
    // default unchanged
    let html = to_html("|===\n|a\n|===");
    assert!(html.contains("<table class=\"tableblock frame-all grid-all stretch\">"), "default. Got:\n{html}");
    // document attrs table-frame/table-grid as fallback; named attr overrides frame only
    let html = to_html(":table-frame: sides\n:table-grid: cols\n\n|===\n|a\n|===\n\n[frame=none]\n|===\n|b\n|===");
    assert!(html.contains("<table class=\"tableblock frame-sides grid-cols stretch\">"), "doc-attr fallback. Got:\n{html}");
    assert!(html.contains("<table class=\"tableblock frame-none grid-cols stretch\">"), "named frame overrides doc-attr, grid inherited. Got:\n{html}");
}

#[test]
fn test_table_col_widths_with_style_letters() {
    // cols="1m,3m": trailing style letter is not part of the weight → 25%/75%
    let html = to_html("[cols=\"1m,3m\"]\n|===\n|a |b\n|===");
    assert!(html.contains("<col style=\"width: 25%;\">"), "1:3 ratio. Got:\n{html}");
    assert!(html.contains("<col style=\"width: 75%;\">"));
}

#[test]
fn test_unconstrained_strong_skips_passthrough() {
    // Passthroughs are extracted before quote subs: the `**` inside +++…+++
    // must not close the surrounding span (pass-macro.adoc, probe-verified)
    let html = to_html("**a+++**+++b**");
    assert!(html.contains("<strong>a**b</strong>"), "Got:\n{html}");
}

#[test]
fn test_table_escaped_pipe_cells() {
    // `\|` in a cell is a literal pipe, not a separator
    let html = to_html("|===\n|a \\| b |c\n|===");
    assert!(html.contains("<p class=\"tableblock\">a | b</p>"), "Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">c</p>"));
    // an entire cell of delimiters with a continuation line (delimited.adoc)
    let html = to_html("|===\n|\\|===\n,===\n|===");
    assert!(html.contains("<p class=\"tableblock\">|===\n,===</p>"), "Got:\n{html}");
    // continuation line with only escaped pipes joins the open cell unescaped
    let html = to_html("[cols=\"1,1\"]\n|===\n|a |b\ntail \\| more\n|c |d\n|===");
    assert!(html.contains("<p class=\"tableblock\">b\ntail | more</p>"), "Got:\n{html}");
}

#[test]
fn test_table_delimiter_four_plus_equals() {
    // Asciidoctor accepts a pipe followed by THREE OR MORE `=` as a table
    // delimiter (`|====`, `|=====`, …) — not just exactly `|===` (image-size.adoc).
    let html = to_html("|====\n|A |B\n|c |d\n|====");
    assert!(html.contains("<table class=\"tableblock"), "|==== should open a table: {html}");
    assert!(html.contains("<p class=\"tableblock\">A</p>"), "Got:\n{html}");
    assert!(html.contains("<p class=\"tableblock\">d</p>"), "Got:\n{html}");
}

#[test]
fn test_table_terminator_matches_opening_delimiter_exactly() {
    // The table is closed only by a line equal to the OPENING delimiter. A
    // table delimiter of a different length inside is cell content, not a
    // terminator (delimited.adoc: a `|====` cell inside a `|===` table).
    let html = to_html("|===\n|A\n|====\n|B\n|===");
    // The inner `|====` becomes a cell whose content is `====`, not a closer.
    assert!(html.contains("<p class=\"tableblock\">====</p>"), "inner |==== must be a cell: {html}");
    // Exactly one table is produced (the inner line did not close it early).
    assert_eq!(html.matches("<table").count(), 1, "exactly one table expected: {html}");
    assert!(html.contains("<p class=\"tableblock\">A</p>") && html.contains("<p class=\"tableblock\">B</p>"));
}

#[test]
fn test_verbatim_block_indent_attribute() {
    // `indent=0` strips the common leading indentation (min over non-blank
    // lines); `indent=N` replaces it with N spaces; absent preserves it.
    let stripped = to_html("[indent=0]\n----\n  a\n   b\n----");
    assert!(stripped.contains("<pre>a\n b</pre>"), "indent=0 strip min: {stripped}");
    let padded = to_html("[indent=3]\n----\n a\n  b\n----");
    assert!(padded.contains("<pre>   a\n    b</pre>"), "indent=3 strip+pad: {padded}");
    let preserved = to_html("----\n a\n  b\n----");
    assert!(preserved.contains("<pre> a\n  b</pre>"), "no indent preserves: {preserved}");
    // A flush-left non-blank line cancels stripping entirely.
    let flush = to_html("[indent=0]\n----\nflush\n  in\n----");
    assert!(flush.contains("<pre>flush\n  in</pre>"), "flush-left cancels: {flush}");
}

#[test]
fn test_listing_indented_conditional_directive_is_literal() {
    // An INDENTED `ifdef`/`endif` inside a verbatim block is literal text
    // (directives are only recognized at column 0); `indent=0` then strips the
    // guard space, yielding the directive verbatim (image-size.adoc pattern).
    let html = to_html("[source,indent=0]\n----\n ifdef::backend-html5[]\n :x: 1\n endif::[]\n----");
    assert!(html.contains("ifdef::backend-html5[]"), "directive kept literal: {html}");
    assert!(html.contains(":x: 1"), "guarded content survives: {html}");
    assert!(!html.contains(" ifdef::backend-html5[]"), "indent should be stripped: {html}");
}
