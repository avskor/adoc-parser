fn standalone_html(input: &str) -> String {
    adoc_html::to_html_with_options(
        input,
        adoc_html::HtmlOptions {
            standalone: true,
            ..Default::default()
        },
    )
}

#[test]
fn author_without_email() {
    let html = standalone_html("= Title\nJohn Doe\n\nContent\n");
    assert!(
        html.contains("<div class=\"details\">"),
        "should have details div: {html}"
    );
    assert!(
        html.contains("<span id=\"author\" class=\"author\">John Doe</span><br>"),
        "should have author span: {html}"
    );
    assert!(
        !html.contains("<span id=\"email\""),
        "should not have email span: {html}"
    );
}

#[test]
fn author_with_email() {
    let html = standalone_html("= Title\nJohn Doe <john@example.com>\n\nContent\n");
    assert!(
        html.contains("<span id=\"author\" class=\"author\">John Doe</span><br>"),
        "should have author span: {html}"
    );
    assert!(
        html.contains(
            "<span id=\"email\" class=\"email\"><a href=\"mailto:john@example.com\">john@example.com</a></span><br>"
        ),
        "should have email span: {html}"
    );
}

#[test]
fn author_with_revision() {
    let html =
        standalone_html("= Title\nJohn Doe\nv1.0, 2024-01-01: Initial release\n\nContent\n");
    assert!(
        html.contains("<span id=\"author\" class=\"author\">John Doe</span><br>"),
        "should have author: {html}"
    );
    assert!(
        html.contains("<span id=\"revnumber\">version 1.0,</span>"),
        "should have revnumber: {html}"
    );
    assert!(
        html.contains("<span id=\"revdate\">2024-01-01</span>"),
        "should have revdate: {html}"
    );
    assert!(
        html.contains("<br><span id=\"revremark\">Initial release</span>"),
        "should have revremark: {html}"
    );
}

#[test]
fn multiple_authors() {
    let html = standalone_html(
        "= Title\nDoc Writer <doc@example.com>; Jane Smith <jane@example.com>\n\nContent\n",
    );
    assert!(
        html.contains("<span id=\"author\" class=\"author\">Doc Writer</span><br>"),
        "should have first author: {html}"
    );
    assert!(
        html.contains("<span id=\"email\" class=\"email\"><a href=\"mailto:doc@example.com\">doc@example.com</a></span><br>"),
        "should have first email: {html}"
    );
    assert!(
        html.contains("<span id=\"author2\" class=\"author\">Jane Smith</span><br>"),
        "should have second author: {html}"
    );
    assert!(
        html.contains("<span id=\"email2\" class=\"email\"><a href=\"mailto:jane@example.com\">jane@example.com</a></span><br>"),
        "should have second email: {html}"
    );
}

#[test]
fn revision_without_remark() {
    let html = standalone_html("= Title\nJohn Doe\nv2.0, 2024-06-15\n\nContent\n");
    assert!(
        html.contains("<span id=\"revnumber\">version 2.0,</span>"),
        "should have revnumber: {html}"
    );
    assert!(
        html.contains("<span id=\"revdate\">2024-06-15</span>"),
        "should have revdate: {html}"
    );
    assert!(
        !html.contains("<span id=\"revremark\">"),
        "should not have revremark span: {html}"
    );
}

#[test]
fn no_author_no_details() {
    let html = standalone_html("= Title\n\nContent\n");
    assert!(
        !html.contains("<div class=\"details\">"),
        "should not have details div when no author: {html}"
    );
}
