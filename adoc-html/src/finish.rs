//! Document finalization: xref sentinel resolution, TOC/footnotes emission,
//! author/revision details and the standalone document head/tail.

use crate::*;

pub(crate) const DEFAULT_STYLESHEET: &str = include_str!("asciidoctor.css");

// MathJax docinfo injected before `</body>` when the document sets the `stem`
// attribute (any value). Asciidoctor emits this fixed block regardless of the
// notation (asciimath/latexmath) or whether actual stem content is present.
// `autoNumber: "none"` is the default; the `eqnums` attribute (not in corpus)
// would change it.
pub(crate) const MATHJAX_DOCINFO: &str = r#"<script type="text/x-mathjax-config">
MathJax.Hub.Config({
  messageStyle: "none",
  tex2jax: {
    inlineMath: [["\\(", "\\)"]],
    displayMath: [["\\[", "\\]"]],
    ignoreClass: "nostem|nolatexmath"
  },
  asciimath2jax: {
    delimiters: [["\\$", "\\$"]],
    ignoreClass: "nostem|noasciimath"
  },
  TeX: { equationNumbers: { autoNumber: "none" } }
})
MathJax.Hub.Register.StartupHook("AsciiMath Jax Ready", function () {
  MathJax.InputJax.AsciiMath.postfilterHooks.Add(function (data, node) {
    if ((node = data.script.parentNode) && (node = node.parentNode) && node.classList.contains("stemblock")) {
      data.math.root.display = "block"
    }
    return data
  })
})
</script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/mathjax/2.7.9/MathJax.js?config=TeX-MML-AM_HTMLorMML"></script>
"#;

/// Replace every `\x00…\x00` xref sentinel found in `text` in a single pass,
/// appending the result to `out`. A replacement value may itself contain
/// sentinels (a block title holding an xref renders them into the registered
/// title HTML), so matched replacements are resolved recursively; `depth`
/// bounds that so a self-referential title cannot recurse forever. A NUL that
/// does not open a known sentinel is kept as-is.
pub(crate) fn resolve_sentinels_into(
    out: &mut String,
    text: &str,
    replacements: &HashMap<&str, String>,
    depth: u8,
) {
    let mut rest = text;
    while let Some(start) = rest.find('\0') {
        out.push_str(&rest[..start]);
        if let Some(len) = rest[start + 1..].find('\0')
            && let Some(replacement) = replacements.get(&rest[start..start + len + 2])
        {
            if depth > 0 && replacement.contains('\0') {
                resolve_sentinels_into(out, replacement, replacements, depth - 1);
            } else {
                out.push_str(replacement);
            }
            rest = &rest[start + len + 2..];
        } else {
            out.push('\0');
            rest = &rest[start + 1..];
        }
    }
    out.push_str(rest);
}

impl HtmlRenderer {
    pub(crate) fn finish(&mut self, output: &mut String) {
        // If preamble_start is still set, no section followed — leave content as-is
        self.preamble_start = None;

        if let Some(pos) = self.toc_insert_position {
            let toc_html = self.generate_toc();
            if !toc_html.is_empty() {
                output.insert_str(pos, &toc_html);
                // Shift content_start if TOC was inserted before it
                if let Some(ref mut cs) = self.content_start
                    && pos <= *cs
                {
                    *cs += toc_html.len();
                }
            }
        }

        // Resolve xref placeholders (link text and internal hrefs) against the
        // unified id/title registries in a single pass over the output.
        if !self.xref_placeholders.is_empty() || !self.xref_href_placeholders.is_empty() {
            // Section titles are accumulated as plain text (escaped on use);
            // block titles and bibliography reftexts are already rendered HTML.
            let mut ctx = XrefResolver::new();
            for entry in self.toc_builder.entries() {
                ctx.add_section(&entry.id, &entry.title);
            }
            for (id, title_html) in &self.block_ref_titles {
                ctx.add_block(id, RefText::Markup(title_html));
            }
            for (id, reftext) in &self.bibliography_reftexts {
                ctx.add_block(id, RefText::Markup(reftext));
            }
            let mut replacements: HashMap<&str, String> = HashMap::with_capacity(
                self.xref_placeholders.len() + self.xref_href_placeholders.len(),
            );
            for (placeholder, fallback, is_internal) in &self.xref_placeholders {
                let replacement = match ctx.link_text(fallback) {
                    Some(RefText::Markup(html)) => html.to_string(),
                    Some(RefText::Plain(text)) => {
                        let mut escaped = String::new();
                        html_escape(&mut escaped, text);
                        escaped
                    }
                    None => {
                        // Unresolved internal anchor reference: Asciidoctor's
                        // default xreflabel wraps the target id in brackets.
                        let plain = if *is_internal {
                            adoc_render_core::unresolved_xref_label(fallback)
                        } else {
                            fallback.clone()
                        };
                        let mut escaped = String::new();
                        html_escape(&mut escaped, &plain);
                        escaped
                    }
                };
                replacements.insert(placeholder.as_str(), replacement);
            }
            for (placeholder, target) in &self.xref_href_placeholders {
                let mut escaped = String::new();
                html_escape(&mut escaped, ctx.href_id(target));
                replacements.insert(placeholder.as_str(), escaped);
            }
            let mut resolved = String::with_capacity(output.len());
            resolve_sentinels_into(&mut resolved, output, &replacements, 8);
            *output = resolved;
        }

        if !self.footnote_registry.is_empty() {
            self.render_footnotes(output);
        }

        // In standalone mode, docinfo is handled by write_document_head/tail
        if !self.standalone {
            if let Some(ref footer) = self.docinfo_footer
                && !footer.is_empty()
            {
                output.push('\n');
                output.push_str(footer);
            }

            if let Some(ref head) = self.docinfo_head
                && !head.is_empty()
            {
                let mut prefix = head.clone();
                prefix.push('\n');
                output.insert_str(0, &prefix);
            }
        }
    }

    pub(crate) fn render_footnotes(&self, output: &mut String) {
        output.push_str("<div id=\"footnotes\">\n<hr>\n");
        for note in self.footnote_registry.footnotes() {
            let num = note.number.to_string();
            output.push_str("<div class=\"footnote\" id=\"_footnotedef_");
            output.push_str(&num);
            output.push_str("\">\n<a href=\"#_footnoteref_");
            output.push_str(&num);
            output.push_str("\">");
            output.push_str(&num);
            output.push_str("</a>. ");
            html_escape_text(output, &note.text);
            output.push_str("\n</div>\n");
        }
        output.push_str("</div>\n");
    }

    pub(crate) fn generate_toc(&self) -> String {
        let steps = self.toc_builder.toc_steps(self.toc_levels);
        if steps.is_empty() {
            return String::new();
        }

        let mut toc = String::new();
        match self.toc_position.as_str() {
            // Side placement: the div is just `toc2`; `toc-left`/`toc-right` go on <body>
            "left" | "right" => toc.push_str("<div id=\"toc\" class=\"toc2\">\n"),
            _ => toc.push_str("<div id=\"toc\" class=\"toc\">\n"),
        }
        toc.push_str("<div id=\"toctitle\">");
        html_escape(&mut toc, &self.toc_title);
        toc.push_str("</div>\n");

        for step in steps {
            match step {
                TocStep::EnterLevel(level) => {
                    if !toc.ends_with('\n') {
                        toc.push('\n');
                    }
                    let sl = level - 1;
                    writeln!(toc, "<ul class=\"sectlevel{sl}\">").unwrap();
                }
                TocStep::Item(entry) => {
                    toc.push_str("<li><a href=\"#");
                    html_escape(&mut toc, &entry.id);
                    toc.push_str("\">");
                    html_escape(&mut toc, &entry.title);
                    toc.push_str("</a>");
                }
                TocStep::CloseItem => toc.push_str("</li>\n"),
                TocStep::LeaveLevel => toc.push_str("</li>\n</ul>\n"),
            }
        }
        toc.push_str("</div>\n");

        toc
    }

    /// Apply Asciidoctor's end-of-header author rescan (parser.rb
    /// `parse_header_metadata`): when the `author` document attribute exists
    /// and differs from the value the implicit author line produced, it was
    /// set by an attribute entry — re-derive `author`/`firstname`/
    /// `middlename`/`lastname`/`authorinitials` from it (names-only parse,
    /// single author). An explicit `:authorinitials:` that differs from the
    /// line-derived value survives the rescan; `middlename`/`lastname` keys
    /// the derivation doesn't produce keep their old values (update, not
    /// replace). Called at `TagEnd::Header` in both standalone and embedded
    /// modes — the derived attributes must resolve in body references.
    pub(crate) fn finalize_header_authors(&mut self) {
        let Some(author_attr) = self.document_attrs.get("author").cloned() else {
            return;
        };
        let implicit = self.authors.authors().first();
        if implicit.is_some_and(|a| a.fullname == author_attr) {
            return;
        }
        if author_attr.is_empty() {
            // Asciidoctor skips empty author entries (`next if author_entry.empty?`)
            return;
        }
        let derived = Author::from_attribute_value(&author_attr);
        // `author_metadata.delete 'authorinitials' if doc_attrs['authorinitials']
        // != implicit_authorinitials` — an explicitly set value wins.
        let keep_initials = self.document_attrs.get("authorinitials").map(String::as_str)
            != implicit.map(|a| a.initials.as_str());
        self.document_attrs.insert("author".to_string(), derived.fullname);
        self.document_attrs.insert("firstname".to_string(), derived.firstname);
        if !derived.middlename.is_empty() {
            self.document_attrs.insert("middlename".to_string(), derived.middlename);
        }
        if !derived.lastname.is_empty() {
            self.document_attrs.insert("lastname".to_string(), derived.lastname);
        }
        if !keep_initials {
            self.document_attrs.insert("authorinitials".to_string(), derived.initials);
        }
        // "do not allow multiple" — the override always yields a single author
        self.document_attrs.insert("authorcount".to_string(), "1".to_string());
    }

    pub(crate) fn render_author_details(&self, output: &mut String) {
        // Revision spans are attribute-driven (Asciidoctor html5.rb checks the
        // revnumber/revdate/revremark document attributes, whether they came from
        // the revision line or from header attribute entries). Called at
        // TagEnd::Header, so document_attrs holds exactly the header-final state;
        // a set-but-empty attribute still produces its span.
        // Attribute references in the values resolve against the document
        // attributes (Asciidoctor applies header substitutions to the revision
        // line as it is read — e.g. `{docdate}` in a revdate); undefined ones
        // stay literal.
        let resolve = |v: &String| {
            adoc_render_core::resolve_attr_refs_text(v, |n| {
                self.document_attrs.get(n).map(|s| s.as_str())
            })
        };
        let revnumber = self.document_attrs.get("revnumber").map(resolve);
        let revdate = self.document_attrs.get("revdate").map(resolve);
        let revremark = self.document_attrs.get("revremark").map(resolve);
        // Author spans are attribute-backed too (Asciidoctor's Document#authors
        // reads `author`/`email` and `author_N`/`email_N` gated by `authorcount`)
        // — so an `:author:` attribute entry opens the details and `:!author:`
        // suppresses it even when an author line was present.
        let author_attr = self.document_attrs.get("author");
        if author_attr.is_none()
            && revnumber.is_none()
            && revdate.is_none()
            && revremark.is_none()
        {
            return;
        }
        output.push_str("<div class=\"details\">\n");
        if author_attr.is_some() {
            let authorcount = self
                .document_attrs
                .get("authorcount")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1)
                .max(1);
            for idx in 0..authorcount {
                let id_suffix = AuthorRegistry::id_suffix(idx);
                let name_suffix = AuthorRegistry::name_suffix(idx);
                let Some(name) = self.document_attrs.get(&format!("author{name_suffix}")) else {
                    continue;
                };
                output.push_str("<span id=\"author");
                output.push_str(&id_suffix);
                output.push_str("\" class=\"author\">");
                html_escape(output, name);
                output.push_str("</span><br>\n");
                if let Some(email) = self
                    .document_attrs
                    .get(&format!("email{name_suffix}"))
                    .filter(|s| !s.is_empty())
                {
                    output.push_str("<span id=\"email");
                    output.push_str(&id_suffix);
                    output.push_str("\" class=\"email\"><a href=\"mailto:");
                    html_escape(output, email);
                    output.push_str("\">");
                    html_escape(output, email);
                    output.push_str("</a></span><br>\n");
                }
            }
        }
        if let Some(version) = &revnumber {
            // Mirrors Asciidoctor: `{version-label.downcase} {revnumber}` with a
            // trailing comma only when a revdate follows; an unset version-label
            // leaves the leading space in place. The value renders verbatim — the
            // `v` prefix is only stripped when parsing the revision line.
            output.push_str("<span id=\"revnumber\">");
            let label = self
                .document_attrs
                .get("version-label")
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            html_escape(output, &label);
            output.push(' ');
            html_escape(output, version);
            if revdate.is_some() {
                output.push(',');
            }
            output.push_str("</span>\n");
        }
        if let Some(date) = &revdate {
            output.push_str("<span id=\"revdate\">");
            html_escape(output, date);
            output.push_str("</span>\n");
        }
        if let Some(remark) = &revremark {
            output.push_str("<br><span id=\"revremark\">");
            html_escape(output, remark);
            output.push_str("</span>\n");
        }
        output.push_str("</div>\n");
    }

    pub(crate) fn write_document_head(&self, output: &mut String) {
        let doctitle = self.document_attrs.get("doctitle")
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .unwrap_or("Untitled");
        let doctype = self.document_attrs.get("doctype").map(|s| s.as_str()).unwrap_or("article");

        output.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        output.push_str("<meta charset=\"UTF-8\">\n");
        output.push_str("<meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\">\n");
        output.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        output.push_str("<meta name=\"generator\" content=\"adoc-parser\">\n");
        output.push_str("<title>");
        html_escape(output, doctitle);
        output.push_str("</title>\n");
        output.push_str("<link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css?family=Open+Sans:300,300italic,400,400italic,600,600italic%7CNoto+Serif:400,400italic,700,700italic%7CDroid+Sans+Mono:400,700\">\n");
        output.push_str("<style>\n");
        output.push_str(DEFAULT_STYLESHEET);
        output.push_str("\n</style>\n");
        if let Some(ref head) = self.docinfo_head
            && !head.is_empty()
        {
            output.push_str(head);
            output.push('\n');
        }
        output.push_str("</head>\n");

        // Build body classes. `toc2` (+ `toc-left`/`toc-right`) only for side placement,
        // and only when the toc attribute came from the header (`toc_auto_seen` — the
        // parser emits Event::Toc inside the header; mid-document `:toc:` has no effect,
        // mirroring Asciidoctor's header-only toc normalization).
        let mut body_classes = String::from(doctype);
        if self.toc_auto_seen && matches!(self.toc_position.as_str(), "left" | "right") {
            body_classes.push_str(" toc2 toc-");
            body_classes.push_str(&self.toc_position);
        }
        output.push_str("<body class=\"");
        output.push_str(&body_classes);
        output.push_str("\">\n");
    }

    pub(crate) fn write_document_tail(&self, output: &mut String) {
        if let Some(ref footer) = self.docinfo_footer
            && !footer.is_empty()
        {
            output.push_str(footer);
            output.push('\n');
        }
        // Asciidoctor injects the MathJax loader before `</body>` whenever the
        // `stem` attribute is set on the document.
        if self.document_attrs.contains_key("stem") {
            output.push_str(MATHJAX_DOCINFO);
        }
        output.push_str("</body>\n</html>");
    }
}
