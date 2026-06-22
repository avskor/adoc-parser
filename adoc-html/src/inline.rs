//! Inline element rendering: cross references, UI macros (kbd/btn/menu),
//! icons and STEM content.

use crate::*;
use std::borrow::Cow;

impl HtmlRenderer {
    pub(crate) fn start_cross_reference(&mut self, output: &mut String, target: &CowStr<'_>, label: &Option<CowStr<'_>>, is_macro: bool) {
        // Resolve attribute references in the target before any xref processing.
        // Asciidoctor runs the global `attributes` pass before `macros`, so an
        // `{name}` written in an xref target (`xref:{rel}.adoc[]`,
        // `xref:{frag}[]`) is already resolved when the cross reference is
        // interpreted — the resolved value drives both the interdoc/internal
        // classification and, for an internal ref, the looked-up id and the
        // bracketed fallback. Our `macros` pass runs first, so the reference
        // survives literally in `Tag::CrossReference.target`; resolve it here
        // (defined → value, undefined → kept literal, same as link/image
        // targets). Borrows when there is no `{`, so every plain xref is
        // byte-for-byte untouched.
        let resolved = self.resolve_inline_attr_value(target);
        let target: &str = resolved.as_ref();
        // Classify the target into an inter-document or internal reference,
        // honouring the shorthand/macro form (different extension rules). For an
        // inter-document ref Asciidoctor uses the rewritten path (extension →
        // `.html`, `#fragment` only in the href) both as href and, when the xref
        // has no explicit text, as the auto-generated link text (without the
        // fragment).
        match adoc_render_core::resolve_xref(target, is_macro) {
            adoc_render_core::XrefResolution::Interdoc { href, text } => {
                output.push_str("<a href=\"");
                html_escape(output, &href);
                output.push_str("\">");
                if label.is_none() {
                    self.in_unlabeled_xref = true;
                    self.xref_placeholder_counter += 1;
                    let placeholder = format!("\x00XREF_{}\x00", self.xref_placeholder_counter);
                    // The rewritten path is final (no section lookup); the
                    // placeholder resolves to it verbatim in `finish()`.
                    self.xref_placeholders.push((placeholder, text, false));
                }
            }
            adoc_render_core::XrefResolution::Internal { id } => {
                // Internal xref (anchor reference). The id is resolved lazily in
                // `finish()` via a placeholder so a forward natural cross reference
                // (`<<Substitutions>>` before its `== Substitutions` section) can be
                // rewritten to the section id (`#_substitutions`).
                output.push_str("<a href=\"#");
                self.xref_placeholder_counter += 1;
                let href_placeholder = format!("\x00XREFHREF_{}\x00", self.xref_placeholder_counter);
                output.push_str(&href_placeholder);
                self.xref_href_placeholders.push((href_placeholder, id.clone()));
                output.push_str("\">");
                if label.is_none() {
                    self.in_unlabeled_xref = true;
                    self.xref_placeholder_counter += 1;
                    let placeholder = format!("\x00XREF_{}\x00", self.xref_placeholder_counter);
                    // The stored value is the target id — the lookup key (resolved
                    // to a section/block title in `finish`) and the bracketed fallback.
                    self.xref_placeholders.push((placeholder, id, true));
                }
            }
        }
    }

    /// Resolve attribute references in an inline role/id value, mirroring
    /// Asciidoctor's global `attributes` substitution. There `quotes` runs before
    /// `attributes`, so a `{name}` written inside an inline attrlist
    /// (`[.{role}]_x_`, `[#{anchor}]*y*`) survives into the role/id and is
    /// resolved against the document attributes afterwards: defined → value,
    /// undefined → kept literal (`attribute-missing` default of `skip`). A value
    /// with no `{` is borrowed unchanged, so every non-attributed role (i.e. all
    /// legacy / gated output) is byte-for-byte untouched and allocates nothing.
    ///
    /// Shared by the inline phrase elements ([`Self::push_inline_id_class`]) and
    /// by the link / inline-image renderers, whose `role` carries the literal
    /// `{name}` written as `link:u[t,role={name}]` (the `macros` pass runs before
    /// `attributes`, so the reference survives unresolved into the role).
    pub(crate) fn resolve_inline_attr_value<'v>(&self, value: &'v str) -> std::borrow::Cow<'v, str> {
        if !value.contains('{') {
            return std::borrow::Cow::Borrowed(value);
        }
        std::borrow::Cow::Owned(adoc_render_core::resolve_attr_refs_text(value, |n| {
            self.document_attrs.get(n).map(|s| s.as_str())
        }))
    }

    /// Write ` id="..."` and ` class="..."` for an inline phrase element (em/strong/code)
    /// carrying an explicit id and/or roles, e.g. `[.path]_x_` → `<em class="path">`.
    /// Attribute references in the id/roles are resolved first (see
    /// [`Self::resolve_inline_attr_value`]).
    pub(crate) fn push_inline_id_class<S: AsRef<str>>(&self, output: &mut String, id: &Option<S>, roles: &[S]) {
        if let Some(id) = id {
            output.push_str(" id=\"");
            html_escape(output, self.resolve_inline_attr_value(id.as_ref()).as_ref());
            output.push('"');
        }
        if !roles.is_empty() {
            output.push_str(" class=\"");
            for (i, role) in roles.iter().enumerate() {
                if i > 0 {
                    output.push(' ');
                }
                html_escape(output, self.resolve_inline_attr_value(role.as_ref()).as_ref());
            }
            output.push('"');
        }
    }

    pub(crate) fn render_kbd_keys(&self, output: &mut String, text: &str) {
        // Mirror Asciidoctor's `kbd:[…]` key splitting (substitutors.rb): the
        // delimiter is whichever of `,` or `+` first appears at char-position >= 1
        // (a leading delimiter at position 0 is a literal key, e.g. `kbd:[+]`).
        // The chosen delimiter splits the keys; the renderer always joins them
        // with `+` regardless of the original separator (html5.rb convert_inline_kbd).
        let text = text.trim();
        let delim = text
            .char_indices()
            .skip(1)
            .find_map(|(_, c)| (c == ',' || c == '+').then_some(c));

        let keys: Vec<Cow<'_, str>> = match delim {
            None => vec![Cow::Borrowed(text)],
            Some(d) if text.ends_with(d) => {
                // Trailing-delimiter special case (`Ctrl++`, `Ctrl,,`): split the
                // body without the final delimiter, trim each key, then re-attach
                // the delimiter to the last key so it renders as a literal key.
                let body = &text[..text.len() - d.len_utf8()];
                let mut parts: Vec<Cow<'_, str>> =
                    body.split(d).map(|k| Cow::Borrowed(k.trim())).collect();
                if let Some(last) = parts.last_mut() {
                    *last = Cow::Owned(format!("{last}{d}"));
                }
                parts
            }
            Some(d) => text.split(d).map(|k| Cow::Borrowed(k.trim())).collect(),
        };

        if keys.len() == 1 {
            output.push_str("<kbd>");
            html_escape_preserving_refs(output, &keys[0]);
            output.push_str("</kbd>");
        } else {
            output.push_str("<span class=\"keyseq\">");
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    output.push('+');
                }
                output.push_str("<kbd>");
                html_escape_preserving_refs(output, key);
                output.push_str("</kbd>");
            }
            output.push_str("</span>");
        }
    }

    /// The caret separator between menu-sequence parts. Under `:icons: font`
    /// Asciidoctor uses a FontAwesome glyph; otherwise a bold `&#8250;`. Both carry
    /// a leading `&#160;` and a trailing space (`html5.rb` `convert_inline_menu`).
    pub(crate) fn menu_caret(&self) -> &'static str {
        if self.document_attrs.get("icons").map(|v| v.as_str()) == Some("font") {
            "&#160;<i class=\"fa fa-angle-right caret\"></i> "
        } else {
            "&#160;<b class=\"caret\">&#8250;</b> "
        }
    }

    pub(crate) fn render_menu(&mut self, output: &mut String) {
        let target = match self.menu_target.take() {
            Some(t) => t,
            None => return,
        };
        let items = self.menu_items.take();

        let items_str = items.unwrap_or_default();
        if items_str.is_empty() {
            // menu:File[] — single menu reference (Asciidoctor: <b class="menuref">…)
            output.push_str("<b class=\"menuref\">");
            html_escape_preserving_refs(output, &target);
            output.push_str("</b>");
        } else {
            let parts: Vec<&str> = items_str.split('>').map(|s| s.trim()).collect();
            output.push_str("<span class=\"menuseq\"><b class=\"menu\">");
            html_escape_preserving_refs(output, &target);
            output.push_str("</b>");
            for (i, part) in parts.iter().enumerate() {
                output.push_str("&#160;<b class=\"caret\">&#8250;</b> ");
                if i < parts.len() - 1 {
                    output.push_str("<b class=\"submenu\">");
                    html_escape_preserving_refs(output, part);
                    output.push_str("</b>");
                } else {
                    output.push_str("<b class=\"menuitem\">");
                    html_escape_preserving_refs(output, part);
                    output.push_str("</b>");
                }
            }
            output.push_str("</span>");
        }
    }

    pub(crate) fn render_icon(&mut self, output: &mut String) {
        let name = match self.icon_name.take() {
            Some(n) => n,
            None => return,
        };
        let attrs_str = self.icon_attrs.take().unwrap_or_default();

        let mut size = None;
        let mut rotate = None;
        let mut flip = None;
        let mut role = None;
        let mut link = None;
        let mut title = None;
        let mut alt = None;
        let mut window = None;

        if !attrs_str.is_empty() {
            for (i, part) in attrs_str.split(',').enumerate() {
                let part = part.trim();
                if let Some((key, val)) = part.split_once('=') {
                    match key.trim() {
                        "role" => role = Some(val.trim().to_string()),
                        "link" => link = Some(val.trim().to_string()),
                        "title" => title = Some(val.trim().to_string()),
                        // `size` is also the first positional attribute (Asciidoctor
                        // `posattrs = ['size']`), so both `icon:x[2x]` and the named
                        // `icon:x[size=2x]` set it.
                        "size" => size = Some(val.trim().to_string()),
                        "rotate" => rotate = Some(val.trim().to_string()),
                        "flip" => flip = Some(val.trim().to_string()),
                        "alt" => alt = Some(val.trim().to_string()),
                        "window" => window = Some(val.trim().to_string()),
                        _ => {}
                    }
                } else if i == 0 {
                    // First positional = size
                    size = Some(part.to_string());
                }
            }
        }

        // When icons are not enabled (`:icons:` unset), Asciidoctor renders an
        // inline icon as literal bracketed text `[name]` rather than a glyph:
        // `<span class="icon">[name&#93;</span>`. The `alt` attribute replaces
        // the name; `role` lands on the span; a `link` wraps the text in an
        // `<a class="image">`. size/title/rotate/flip are ignored in this mode.
        // (`:icons: font` and image modes keep the glyph path below unchanged.)
        if !self.document_attrs.contains_key("icons") {
            output.push_str("<span class=\"icon");
            if let Some(ref r) = role {
                output.push(' ');
                html_escape(output, r);
            }
            output.push_str("\">");
            if let Some(href) = &link {
                output.push_str("<a class=\"image\" href=\"");
                html_escape(output, href);
                output.push('"');
                if let Some(ref w) = window {
                    write_attr(output, "target", w);
                    if w == "_blank" {
                        output.push_str(" rel=\"noopener\"");
                    }
                }
                output.push('>');
            }
            output.push('[');
            match alt {
                Some(ref a) => html_escape_preserving_refs(output, a),
                None => html_escape_preserving_refs(output, &adoc_parser::icon_default_alt(&name)),
            }
            output.push_str("&#93;");
            if link.is_some() {
                output.push_str("</a>");
            }
            output.push_str("</span>");
            return;
        }

        // `:icons: font` glyph mode. Mirror of Asciidoctor `convert_inline_image`
        // (html5.rb): the `<i>` class carries only `fa fa-NAME` + size + flip/rotate
        // (flip wins over rotate when both are set); `role` lands on the wrapping
        // `<span class="icon …">`, NOT on the `<i>`; a `link` wraps the `<i>` in an
        // inner `<a class="image">` while the outer wrapper stays the icon span.
        let mut classes = format!("fa fa-{name}");
        if let Some(ref s) = size {
            classes.push_str(&format!(" fa-{s}"));
        }
        if let Some(ref f) = flip {
            classes.push_str(&format!(" fa-flip-{f}"));
        } else if let Some(ref r) = rotate {
            classes.push_str(&format!(" fa-rotate-{r}"));
        }

        output.push_str("<span class=\"icon");
        if let Some(ref r) = role {
            output.push(' ');
            html_escape(output, r.trim_matches('"'));
        }
        output.push_str("\">");

        if let Some(href) = &link {
            output.push_str("<a class=\"image\" href=\"");
            html_escape(output, href);
            output.push('"');
            if let Some(ref w) = window {
                write_attr(output, "target", w);
                if w == "_blank" {
                    output.push_str(" rel=\"noopener\"");
                }
            }
            output.push('>');
        }

        output.push_str("<i class=\"");
        html_escape_preserving_refs(output, &classes);
        output.push('"');
        if let Some(ref t) = title {
            // The `title` value carries the line's inline substitutions: in
            // Asciidoctor the whole paragraph runs specialchars+quotes+replacements
            // BEFORE the macros pass extracts the icon, so `title=~Title~` arrives as
            // `<sub>Title</sub>`. Mirror that by rendering the (de-quoted) value
            // through the current block's subs; a plain title takes the no-markup
            // fast path and stays byte-for-byte identical to the previous output.
            let mut rendered = String::new();
            self.render_inline_value(&mut rendered, t.trim_matches('"'));
            output.push_str(" title=\"");
            output.push_str(&rendered);
            output.push('"');
        }
        output.push_str("></i>");

        if link.is_some() {
            output.push_str("</a>");
        }
        output.push_str("</span>");
    }

    pub(crate) fn render_inline_stem(&mut self, output: &mut String) {
        let variant = match self.stem_variant.take() {
            Some(v) => v,
            None => return,
        };
        let content = self.stem_content.take().unwrap_or_default();

        // Resolve "stem" to the document attribute :stem: value
        let resolved = if variant == "stem" {
            self.document_attrs.get("stem")
                .map(|s| s.as_str())
                .unwrap_or("asciimath")
        } else {
            &variant
        };

        // Asciidoctor applies the `specialcharacters` substitution to stem
        // content, so `<`/`>`/`&` are escaped (`stem:[a < b]` → `\$a &lt; b\$`)
        // and a character reference is treated as literal text, not preserved
        // (`stem:[a&#167;b]` → `\$a&amp;#167;b\$`) — unlike the verbatim UI macros.
        if resolved == "latexmath" {
            output.push_str("\\(");
            html_escape_text(output, &content);
            output.push_str("\\)");
        } else {
            // stem and asciimath
            output.push_str("\\$");
            html_escape_text(output, &content);
            output.push_str("\\$");
        }
    }

    pub(crate) fn render_stem_block(&mut self, output: &mut String) {
        let variant = match self.stem_block_variant.take() {
            Some(v) => v,
            None => return,
        };
        let content = self.stem_block_content.take().unwrap_or_default();

        // Resolve "stem" to the document attribute :stem: value
        let resolved = if variant == "stem" {
            self.document_attrs.get("stem")
                .map(|s| s.as_str())
                .unwrap_or("asciimath")
        } else {
            &variant
        };

        // `specialcharacters` subst, as in `render_inline_stem`.
        if resolved == "latexmath" {
            output.push_str("\\[");
            html_escape_text(output, &content);
            output.push_str("\\]");
        } else {
            output.push_str("\\$");
            html_escape_text(output, &content);
            output.push_str("\\$");
        }
        output.push_str("\n</div>\n</div>\n");
    }
}
