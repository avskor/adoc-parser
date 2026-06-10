//! Inline element rendering: cross references, UI macros (kbd/btn/menu),
//! icons and STEM content.

use crate::*;

impl HtmlRenderer {
    pub(crate) fn start_cross_reference(&mut self, output: &mut String, target: &CowStr<'_>, label: &Option<CowStr<'_>>) {
        let is_interdoc = adoc_render_core::is_interdoc_xref_target(target);
        // Inter-document target with the .adoc extension rewritten to .html.
        // Asciidoctor uses this rewritten path both as href and, when the xref
        // has no explicit text, as the auto-generated link text.
        let interdoc_href = if is_interdoc {
            adoc_render_core::interdoc_xref_href(target)
        } else {
            String::new()
        };
        if is_interdoc {
            output.push_str("<a href=\"");
            html_escape(output, &interdoc_href);
        } else {
            // Internal xref (anchor reference). The id is resolved lazily in
            // `finish()` via a placeholder so a forward natural cross reference
            // (`<<Substitutions>>` before its `== Substitutions` section) can be
            // rewritten to the section id (`#_substitutions`).
            output.push_str("<a href=\"#");
            self.xref_placeholder_counter += 1;
            let href_placeholder = format!("\x00XREFHREF_{}\x00", self.xref_placeholder_counter);
            output.push_str(&href_placeholder);
            self.xref_href_placeholders
                .push((href_placeholder, target.to_string()));
        }
        output.push_str("\">");
        if label.is_none() {
            self.in_unlabeled_xref = true;
            self.xref_placeholder_counter += 1;
            let placeholder = format!("\x00XREF_{}\x00", self.xref_placeholder_counter);
            // For an internal xref the stored value is the target id — used as the
            // lookup key (resolved to a section/block title in `finish`) and as the
            // fallback. For an inter-document xref it is the rewritten .html path.
            let fallback = if is_interdoc {
                interdoc_href
            } else {
                target.to_string()
            };
            self.xref_placeholders
                .push((placeholder, fallback, !is_interdoc));
        }
    }

    /// Write ` id="..."` and ` class="..."` for an inline phrase element (em/strong/code)
    /// carrying an explicit id and/or roles, e.g. `[.path]_x_` → `<em class="path">`.
    pub(crate) fn push_inline_id_class<S: AsRef<str>>(output: &mut String, id: &Option<S>, roles: &[S]) {
        if let Some(id) = id {
            output.push_str(" id=\"");
            html_escape(output, id.as_ref());
            output.push('"');
        }
        if !roles.is_empty() {
            output.push_str(" class=\"");
            for (i, role) in roles.iter().enumerate() {
                if i > 0 {
                    output.push(' ');
                }
                html_escape(output, role.as_ref());
            }
            output.push('"');
        }
    }

    pub(crate) fn render_kbd_keys(&self, output: &mut String, text: &str) {
        let keys: Vec<&str> = text.split('+').map(|k| k.trim()).collect();
        if keys.len() == 1 {
            output.push_str("<kbd>");
            html_escape(output, keys[0]);
            output.push_str("</kbd>");
        } else {
            output.push_str("<span class=\"keyseq\">");
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    output.push('+');
                }
                output.push_str("<kbd>");
                html_escape(output, key);
                output.push_str("</kbd>");
            }
            output.push_str("</span>");
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
            // menu:File[] — just the menu name
            output.push_str("<span class=\"menu\">");
            html_escape(output, &target);
            output.push_str("</span>");
        } else {
            let parts: Vec<&str> = items_str.split('>').map(|s| s.trim()).collect();
            output.push_str("<span class=\"menuseq\"><b class=\"menu\">");
            html_escape(output, &target);
            output.push_str("</b>");
            for (i, part) in parts.iter().enumerate() {
                output.push_str("&#160;<b class=\"caret\">&#8250;</b> ");
                if i < parts.len() - 1 {
                    output.push_str("<b class=\"submenu\">");
                    html_escape(output, part);
                    output.push_str("</b>");
                } else {
                    output.push_str("<b class=\"menuitem\">");
                    html_escape(output, part);
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

        if !attrs_str.is_empty() {
            for (i, part) in attrs_str.split(',').enumerate() {
                let part = part.trim();
                if let Some((key, val)) = part.split_once('=') {
                    match key.trim() {
                        "role" => role = Some(val.trim().to_string()),
                        "link" => link = Some(val.trim().to_string()),
                        "title" => title = Some(val.trim().to_string()),
                        "rotate" => rotate = Some(val.trim().to_string()),
                        "flip" => flip = Some(val.trim().to_string()),
                        _ => {}
                    }
                } else if i == 0 {
                    // First positional = size
                    size = Some(part.to_string());
                }
            }
        }

        // Build class list
        let mut classes = format!("fa fa-{name}");
        if let Some(ref s) = size {
            classes.push_str(&format!(" fa-{s}"));
        }
        if let Some(ref r) = rotate {
            classes.push_str(&format!(" fa-rotate-{r}"));
        }
        if let Some(ref f) = flip {
            classes.push_str(&format!(" fa-flip-{f}"));
        }
        if let Some(ref r) = role {
            classes.push(' ');
            classes.push_str(r);
        }

        if let Some(href) = &link {
            output.push_str("<a class=\"icon\" href=\"");
            html_escape(output, href);
            output.push_str("\">");
        } else {
            output.push_str("<span class=\"icon\">");
        }

        output.push_str("<i class=\"");
        html_escape(output, &classes);
        output.push('"');
        if let Some(ref t) = title {
            write_attr(output, "title", t);
        }
        output.push_str("></i>");

        if link.is_some() {
            output.push_str("</a>");
        } else {
            output.push_str("</span>");
        }
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

        if resolved == "latexmath" {
            output.push_str("\\(");
            output.push_str(&content);
            output.push_str("\\)");
        } else {
            // stem and asciimath
            output.push_str("\\$");
            output.push_str(&content);
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

        if resolved == "latexmath" {
            output.push_str("\\[");
            output.push_str(&content);
            output.push_str("\\]");
        } else {
            output.push_str("\\$");
            output.push_str(&content);
            output.push_str("\\$");
        }
        output.push_str("\n</div>\n</div>\n");
    }
}
