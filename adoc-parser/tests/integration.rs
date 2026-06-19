use std::borrow::Cow;

use adoc_parser::{
    AdmonitionKind, DelimitedBlockKind, Event, Parser, Tag, TagEnd,
};

fn parse(input: &str) -> Vec<Event<'_>> {
    Parser::new(input).collect()
}

#[test]
fn test_empty_input() {
    let events = parse("");
    assert!(events.is_empty());
}

#[test]
fn test_simple_paragraph() {
    let events = parse("Hello world.");
    assert_eq!(events, vec![
        Event::Start(Tag::Paragraph),
        Event::Text(Cow::Borrowed("Hello world.")),
        Event::End(TagEnd::Paragraph),
    ]);
}

#[test]
fn test_multiline_paragraph() {
    let events = parse("Line one\nLine two\nLine three");
    assert_eq!(events, vec![
        Event::Start(Tag::Paragraph),
        Event::Text(Cow::Owned("Line one\nLine two\nLine three".to_string())),
        Event::End(TagEnd::Paragraph),
    ]);
}

#[test]
fn test_two_paragraphs() {
    let events = parse("First paragraph.\n\nSecond paragraph.");
    assert_eq!(events, vec![
        Event::Start(Tag::Paragraph),
        Event::Text(Cow::Borrowed("First paragraph.")),
        Event::End(TagEnd::Paragraph),
        Event::Start(Tag::Paragraph),
        Event::Text(Cow::Borrowed("Second paragraph.")),
        Event::End(TagEnd::Paragraph),
    ]);
}

#[test]
fn test_section_with_content() {
    let events = parse("== Introduction\n\nThis is the intro.\n\n=== Details\n\nMore details here.");
    assert_eq!(events[0], Event::Start(Tag::Section { level: 2 }));
    assert!(matches!(&events[1], Event::Start(Tag::SectionTitle { level: 2, .. })));
    assert_eq!(events[2], Event::Text(Cow::Borrowed("Introduction")));
    assert_eq!(events[3], Event::End(TagEnd::SectionTitle));

    // Find the nested section
    let section3_idx = events.iter().position(|e| matches!(e, Event::Start(Tag::Section { level: 3 }))).unwrap();
    assert!(section3_idx > 4);

    // Verify closing order
    let last_events: Vec<_> = events.iter().rev().take(4).cloned().collect();
    assert_eq!(last_events[0], Event::End(TagEnd::Section { level: 2 }));
    assert_eq!(last_events[1], Event::End(TagEnd::Section { level: 3 }));
}

#[test]
fn test_document_header_with_attributes() {
    let events = parse("= My Document\nAuthor Name\n:toc: left\n:icons: font\n\nContent.");

    assert_eq!(events[0], Event::Start(Tag::Header));
    assert!(matches!(&events[1], Event::Start(Tag::SectionTitle { level: 0, .. })));
    assert_eq!(events[2], Event::Start(Tag::DocumentTitle));
    assert_eq!(events[3], Event::Text(Cow::Borrowed("My Document")));
    assert_eq!(events[4], Event::End(TagEnd::DocumentTitle));
    assert_eq!(events[5], Event::End(TagEnd::SectionTitle));

    // Author and attributes follow
    let has_author = events.iter().any(|e| matches!(e, Event::Author { fullname, .. } if fullname == "Author Name"));
    assert!(has_author);

    let has_toc = events.iter().any(|e| matches!(e, Event::Attribute { name, value } if name == "toc" && value == "left"));
    assert!(has_toc);
}

#[test]
fn test_unordered_list() {
    let events = parse("* First\n* Second\n* Third");

    assert_eq!(events[0], Event::Start(Tag::UnorderedList { has_checklist: false }));
    assert_eq!(events[1], Event::Start(Tag::ListItem { depth: 1, checked: None }));
    assert_eq!(events[2], Event::Text(Cow::Borrowed("First")));
    assert_eq!(events[3], Event::End(TagEnd::ListItem));
    assert_eq!(events[4], Event::Start(Tag::ListItem { depth: 1, checked: None }));
}

#[test]
fn test_ordered_list() {
    let events = parse(". Alpha\n. Beta\n. Gamma");

    assert_eq!(events[0], Event::Start(Tag::OrderedList { start: None, reversed: false, depth: 1 }));
    assert_eq!(events[1], Event::Start(Tag::ListItem { depth: 1, checked: None }));
    assert_eq!(events[2], Event::Text(Cow::Borrowed("Alpha")));
}

#[test]
fn test_source_block() {
    let events = parse("[source,rust]\n----\nfn main() {}\n----");

    assert!(matches!(&events[0], Event::Start(Tag::SourceBlock { language: Some(lang) }) if lang == "rust"));
    assert_eq!(events[1], Event::Text(Cow::Borrowed("fn main() {}")));
    assert_eq!(events[2], Event::End(TagEnd::SourceBlock));
}

#[test]
fn test_listing_block_without_source() {
    let events = parse("----\nsome code\n----");

    assert_eq!(events[0], Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Listing }));
    assert_eq!(events[1], Event::Text(Cow::Borrowed("some code")));
    assert_eq!(events[2], Event::End(TagEnd::DelimitedBlock));
}

#[test]
fn test_literal_block() {
    let events = parse("....\nverbatim text\n....");

    assert_eq!(events[0], Event::Start(Tag::DelimitedBlock { kind: DelimitedBlockKind::Literal }));
    assert_eq!(events[1], Event::Text(Cow::Borrowed("verbatim text")));
    assert_eq!(events[2], Event::End(TagEnd::DelimitedBlock));
}

#[test]
fn test_admonition_note() {
    let events = parse("NOTE: Pay attention.");

    assert_eq!(events[0], Event::Start(Tag::Admonition { kind: AdmonitionKind::Note, block: false }));
    assert_eq!(events[1], Event::Start(Tag::Paragraph));
    assert_eq!(events[2], Event::Text(Cow::Borrowed("Pay attention.")));
    assert_eq!(events[3], Event::End(TagEnd::Paragraph));
    assert_eq!(events[4], Event::End(TagEnd::Admonition));
}

#[test]
fn test_block_image() {
    let events = parse("image::sunset.jpg[A beautiful sunset]");

    assert!(matches!(&events[0], Event::Start(Tag::BlockImage { target, alt, width, height, link, interactive, fallback }) if target == "sunset.jpg" && alt == "A beautiful sunset" && width.is_none() && height.is_none() && link.is_none() && !interactive && fallback.is_none()));
    assert_eq!(events[1], Event::End(TagEnd::BlockImage));
}

#[test]
fn test_thematic_and_page_breaks() {
    let events = parse("Before\n\n'''\n\n<<<\n\nAfter");

    let has_thematic = events.iter().any(|e| matches!(e, Event::ThematicBreak));
    let has_page = events.iter().any(|e| matches!(e, Event::PageBreak));
    assert!(has_thematic);
    assert!(has_page);
}

#[test]
fn test_inline_bold_in_paragraph() {
    let events = parse("This is *important* text.");

    assert_eq!(events[0], Event::Start(Tag::Paragraph));
    assert_eq!(events[1], Event::Text(Cow::Borrowed("This is ")));
    assert_eq!(events[2], Event::Start(Tag::Strong { id: None, roles: Vec::new() }));
    assert_eq!(events[3], Event::Text(Cow::Borrowed("important")));
    assert_eq!(events[4], Event::End(TagEnd::Strong));
    assert_eq!(events[5], Event::Text(Cow::Borrowed(" text.")));
    assert_eq!(events[6], Event::End(TagEnd::Paragraph));
}

#[test]
fn test_inline_link_in_paragraph() {
    let events = parse("Visit link:https://example.com[our site] for more.");

    let has_link = events.iter().any(|e| matches!(e, Event::Start(Tag::Link { url, .. }) if url == "https://example.com"));
    assert!(has_link);
}

#[test]
fn test_cross_reference() {
    let events = parse("See <<introduction>> for details.");

    let has_xref = events.iter().any(|e| matches!(e, Event::Start(Tag::CrossReference { target, .. }) if target == "introduction"));
    assert!(has_xref);
}

#[test]
fn test_attribute_entry_and_reference() {
    let events = parse(":version: 1.0\n\nVersion is {version}.");

    let has_attr = events.iter().any(|e| matches!(e, Event::Attribute { name, value } if name == "version" && value == "1.0"));
    assert!(has_attr);

    let has_ref = events.iter().any(|e| matches!(e, Event::AttributeReference { name, .. } if name == "version"));
    assert!(has_ref);
}

#[test]
fn test_attribute_value_resolved_at_definition_time() {
    // Asciidoctor resolves `{ref}` in an attribute value when the entry is
    // *defined*, using attributes seen so far. `:dan-uri:` therefore stores the
    // fully expanded URL, so the emitted Event::Attribute carries it.
    let events = parse(":github-uri: https://github.com\n:dan-uri: {github-uri}/mojavelinux\n\nx");

    let dan = events.iter().find_map(|e| match e {
        Event::Attribute { name, value } if name == "dan-uri" => Some(value.as_ref()),
        _ => None,
    });
    assert_eq!(dan, Some("https://github.com/mojavelinux"));
}

#[test]
fn test_attribute_value_nested_resolution_is_deep() {
    // Each entry is resolved at its own definition, so chained references
    // collapse fully (no leftover braces) regardless of nesting depth.
    let events = parse(
        ":id: asciidoclet\n\
         :base: https://bintray.com/maven/{id}\n\
         :detail: {base}/view\n\n\
         x",
    );
    let detail = events.iter().find_map(|e| match e {
        Event::Attribute { name, value } if name == "detail" => Some(value.as_ref()),
        _ => None,
    });
    assert_eq!(detail, Some("https://bintray.com/maven/asciidoclet/view"));
}

#[test]
fn test_attribute_value_forward_reference_stays_literal() {
    // A reference to a not-yet-defined attribute resolves to nothing at
    // definition time (default `attribute-missing=skip`) and stays literal —
    // it is NOT back-patched once the target is later defined.
    let events = parse(":b: {a}/x\n:a: https://example.com\n\nx");

    let b = events.iter().find_map(|e| match e {
        Event::Attribute { name, value } if name == "b" => Some(value.as_ref()),
        _ => None,
    });
    assert_eq!(b, Some("{a}/x"));
}

#[test]
fn test_comment_block_not_in_output() {
    let events = parse("Before\n\n////\nThis is a comment\n////\n\nAfter");

    // Comment content should not appear as events
    let has_comment_text = events.iter().any(|e| matches!(e, Event::Text(t) if t.contains("This is a comment")));
    assert!(!has_comment_text);
}

#[test]
fn test_literal_paragraph() {
    let events = parse(" This is indented\n More indented");

    assert_eq!(events[0], Event::Start(Tag::LiteralParagraph));
}

#[test]
fn test_block_title() {
    let events = parse(".My Block Title\n----\ncode\n----");

    assert_eq!(events[0], Event::Start(Tag::BlockTitle));
    assert_eq!(events[1], Event::Text(Cow::Borrowed("My Block Title")));
    assert_eq!(events[2], Event::End(TagEnd::BlockTitle));
}

#[test]
fn test_complex_document() {
    let input = "\
= My Document
Author Name
:toc: left

== Introduction

This is the *introduction* with a link:https://example.com[link].

=== Getting Started

. Step one
. Step two
. Step three

NOTE: Read carefully.

== Code Examples

[source,rust]
----
fn main() {
    println!(\"Hello\");
}
----

'''

== Conclusion

The end.";

    let events = parse(input);

    // Verify basic structure exists
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Header))));
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::DocumentTitle))));
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Section { level: 2 }))));
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Section { level: 3 }))));
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::OrderedList { .. }))));
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Admonition { kind: AdmonitionKind::Note, .. }))));
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::SourceBlock { .. }))));
    assert!(events.iter().any(|e| matches!(e, Event::ThematicBreak)));
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Strong { .. }))));
    assert!(events.iter().any(|e| matches!(e, Event::Start(Tag::Link { .. }))));

    // Verify all Start/End pairs are balanced
    let mut depth = 0i32;
    for event in &events {
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            _ => {}
        }
        assert!(depth >= 0, "End without matching Start");
    }
    assert_eq!(depth, 0, "Unmatched Start events");
}

#[test]
fn test_many_comment_lines_no_stack_overflow() {
    // Regression: consecutive comment lines used to recurse once per line,
    // overflowing the stack on large comment blocks. They are now consumed
    // iteratively, so even a huge run parses without crashing.
    let mut input = String::with_capacity(50_000 * 10);
    for i in 0..50_000 {
        input.push_str("// comment ");
        input.push_str(&i.to_string());
        input.push('\n');
    }
    input.push_str("Real paragraph.");

    let events = parse(&input);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Text(t) if t.as_ref() == "Real paragraph.")),
        "paragraph after a large comment block must still be parsed"
    );
}

#[test]
fn test_many_block_attribute_lines_no_stack_overflow() {
    // Regression (D3): consecutive `[attr]` lines used to tail-recurse once per
    // line in scan_next_block (overflowing the ~2 MB test-thread stack). They now
    // loop via the rescan_requested trampoline, so a huge run parses fine.
    let mut input = String::with_capacity(50_000 * 8);
    for _ in 0..50_000 {
        input.push_str("[.role]\n");
    }
    input.push_str("Real paragraph.");

    let events = parse(&input);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Text(t) if t.as_ref() == "Real paragraph.")),
        "paragraph after a large run of block-attribute lines must still parse"
    );
}

#[test]
fn test_many_block_title_lines_no_stack_overflow() {
    // Regression (D3): consecutive `.Title` lines likewise recursed per line.
    let mut input = String::with_capacity(50_000 * 8);
    for _ in 0..50_000 {
        input.push_str(".Title\n");
    }
    input.push_str("Real paragraph.");

    let events = parse(&input);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Text(t) if t.as_ref() == "Real paragraph.")),
        "paragraph after a large run of block-title lines must still parse"
    );
}
