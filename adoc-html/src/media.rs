//! Media rendering: block/inline images, video (incl. YouTube/Vimeo) and audio.

use crate::*;

pub(crate) struct MediaAttrs<'a> {
    width: Option<&'a str>,
    height: Option<&'a str>,
    poster: Option<&'a str>,
    start: Option<&'a str>,
    end: Option<&'a str>,
    list: Option<&'a str>,
    playlist: Option<&'a str>,
    autoplay: bool,
    loop_: bool,
    nocontrols: bool,
}

pub(crate) fn parse_media_attrs(attrs: &str) -> MediaAttrs<'_> {
    let mut result = MediaAttrs {
        width: None,
        height: None,
        poster: None,
        start: None,
        end: None,
        list: None,
        playlist: None,
        autoplay: false,
        loop_: false,
        nocontrols: false,
    };
    if attrs.is_empty() {
        return result;
    }

    // Split on commas, but respect quoted strings
    let mut parts: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    for (i, ch) in attrs.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                parts.push(&attrs[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&attrs[start..]);

    for part in parts {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "width" => result.width = Some(value),
                "height" => result.height = Some(value),
                "poster" => result.poster = Some(value),
                "start" => result.start = Some(value),
                "end" => result.end = Some(value),
                "list" => result.list = Some(value),
                "playlist" => result.playlist = Some(value),
                // `opts` is the documented shorthand for `options` (Asciidoctor
                // treats them identically).
                "opts" | "options" => {
                    for opt in value.split(',') {
                        let opt = opt.trim();
                        match opt {
                            "autoplay" => result.autoplay = true,
                            "loop" => result.loop_ = true,
                            "nocontrols" => result.nocontrols = true,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
    result
}

/// Detect video provider from the first positional attribute.
pub(crate) fn detect_video_provider(attrs: &str) -> Option<&str> {
    let first = attrs.split(',').next().unwrap_or("").trim();
    match first {
        "youtube" | "vimeo" => Some(first),
        _ => None,
    }
}

pub(crate) fn render_video_tag(output: &mut String, target: &str, attrs: &str) {
    let media = parse_media_attrs(attrs);

    match detect_video_provider(attrs) {
        Some("youtube") => {
            // Asciidoctor: target `video_id/list_id` carries an optional playlist
            // (else the `list` attribute); failing that, `video_id1,video_id2,...`
            // is a dynamic playlist (else the `playlist` attribute), emitted as
            // `&playlist=` with the video id prepended. A bare `loop` still needs
            // a playlist for YouTube to actually loop, so the video id is used.
            let (video_id, list) = match target.split_once('/') {
                Some((id, list)) => (id, Some(list)),
                None => (target, media.list),
            };
            let (video_id, playlist) = if list.is_some() {
                (video_id, None)
            } else {
                match video_id.split_once(',') {
                    Some((id, rest)) => (id, Some(rest)),
                    None => (video_id, media.playlist),
                }
            };
            output.push_str("<iframe");
            if let Some(w) = media.width {
                write_attr(output, "width", w);
            }
            if let Some(h) = media.height {
                write_attr(output, "height", h);
            }
            output.push_str(" src=\"https://www.youtube.com/embed/");
            html_escape(output, video_id);
            // Query parameters follow Asciidoctor's order: rel, start, end,
            // autoplay, loop, controls, list/playlist.
            output.push_str("?rel=0");
            if let Some(s) = media.start {
                output.push_str("&amp;start=");
                html_escape(output, s);
            }
            if let Some(e) = media.end {
                output.push_str("&amp;end=");
                html_escape(output, e);
            }
            if media.autoplay {
                output.push_str("&amp;autoplay=1");
            }
            if media.loop_ {
                output.push_str("&amp;loop=1");
            }
            if media.nocontrols {
                output.push_str("&amp;controls=0");
            }
            if let Some(list) = list {
                output.push_str("&amp;list=");
                html_escape(output, list);
            } else if let Some(playlist) = playlist {
                output.push_str("&amp;playlist=");
                html_escape(output, video_id);
                output.push(',');
                html_escape(output, playlist);
            } else if media.loop_ {
                output.push_str("&amp;playlist=");
                html_escape(output, video_id);
            }
            output.push_str("\" frameborder=\"0\" allowfullscreen></iframe>\n</div>\n");
        }
        Some("vimeo") => {
            output.push_str("<iframe");
            if let Some(w) = media.width {
                write_attr(output, "width", w);
            }
            if let Some(h) = media.height {
                write_attr(output, "height", h);
            }
            output.push_str(" src=\"https://player.vimeo.com/video/");
            html_escape(output, target);
            output.push_str("\" frameborder=\"0\" allowfullscreen></iframe>\n</div>\n");
        }
        _ => {
            // Regular HTML5 video
            output.push_str("<video src=\"");
            html_escape(output, target);
            push_media_time_fragment(output, media.start, media.end);
            output.push('"');

            if let Some(w) = media.width {
                write_attr(output, "width", w);
            }
            if let Some(h) = media.height {
                write_attr(output, "height", h);
            }
            if let Some(p) = media.poster {
                write_attr(output, "poster", p);
            }
            if !media.nocontrols {
                output.push_str(" controls");
            }
            if media.autoplay {
                output.push_str(" autoplay");
            }
            if media.loop_ {
                output.push_str(" loop");
            }
            output.push_str(">\nYour browser does not support the video tag.\n</video>\n</div>\n");
        }
    }
}

/// Append the `#t=start,end` media time fragment shared by the HTML5 video
/// and audio `src` attributes.
pub(crate) fn push_media_time_fragment(output: &mut String, start: Option<&str>, end: Option<&str>) {
    match (start, end) {
        (Some(s), Some(e)) => {
            output.push_str("#t=");
            html_escape(output, s);
            output.push(',');
            html_escape(output, e);
        }
        (Some(s), None) => {
            output.push_str("#t=");
            html_escape(output, s);
        }
        (None, Some(e)) => {
            output.push_str("#t=,");
            html_escape(output, e);
        }
        (None, None) => {}
    }
}

pub(crate) fn render_audio_tag(output: &mut String, target: &str, attrs: &str) {
    let media = parse_media_attrs(attrs);

    // Build src with an optional media time fragment (#t=start,end), mirroring
    // the HTML5 video tag.
    output.push_str("<audio src=\"");
    html_escape(output, target);
    push_media_time_fragment(output, media.start, media.end);
    output.push('"');

    // Asciidoctor emits the boolean attributes in this order: autoplay, loop,
    // controls (controls is on by default unless `nocontrols`).
    if media.autoplay {
        output.push_str(" autoplay");
    }
    if media.loop_ {
        output.push_str(" loop");
    }
    if !media.nocontrols {
        output.push_str(" controls");
    }
    output.push_str(">\nYour browser does not support the audio tag.\n</audio>\n</div>\n");
}

/// Generate alt text from image target: strip path, strip extension, replace `-_` with spaces.
pub(crate) fn auto_alt_from_target(target: &str) -> String {
    // Get filename (last path component)
    let filename = target.rsplit('/').next().unwrap_or(target);
    // Strip extension
    let stem = match filename.rfind('.') {
        Some(pos) if pos > 0 => &filename[..pos],
        _ => filename,
    };
    // Replace hyphens and underscores with spaces
    stem.replace(['-', '_'], " ")
}

/// Mirror of Asciidoctor's `UriSniffRx` (`\A\p{Alpha}[\p{Alnum}.+-]+:/{0,2}`):
/// a scheme of at least two characters followed by `:` marks a URI (so a
/// single-letter Windows drive prefix stays a file path). Kept in sync with the
/// parser's `preprocessor::is_uriish`.
fn is_uriish(target: &str) -> bool {
    let mut chars = target.chars();
    if !chars.next().is_some_and(char::is_alphabetic) {
        return false;
    }
    let mut middle = 0usize;
    for c in chars {
        match c {
            ':' => return middle >= 1,
            c if c.is_alphanumeric() || matches!(c, '.' | '+' | '-') => middle += 1,
            _ => return false,
        }
    }
    false
}

impl HtmlRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_block_image(&mut self, output: &mut String, target: &CowStr<'_>, alt: &CowStr<'_>, width: &Option<CowStr<'_>>, height: &Option<CowStr<'_>>, link: &Option<CowStr<'_>>, meta: &Option<BlockMeta>) {
        // Build base class with align/float CSS classes from named attrs
        let base_class = Self::image_base_class("imageblock", meta);
        output.push_str("<div");
        Self::write_meta_attrs(output, meta, &base_class);
        output.push_str(">\n<div class=\"content\">\n");
        let has_link = link.is_some();
        if let Some(href) = link {
            output.push_str("<a class=\"image\" href=\"");
            html_escape(output, href);
            output.push_str("\">");
        }
        output.push_str("<img");
        write_attr(output, "src", &self.image_uri(target));
        // Auto-generate alt from filename if empty
        let effective_alt = if alt.as_ref().is_empty() {
            auto_alt_from_target(target)
        } else {
            alt.to_string()
        };
        write_attr(output, "alt", &effective_alt);
        if let Some(w) = width {
            write_attr(output, "width", w);
        }
        if let Some(h) = height {
            write_attr(output, "height", h);
        }
        output.push('>');
        if has_link {
            output.push_str("</a>");
        }
        output.push_str("\n</div>\n");
        // Asciidoctor places a titled image's caption AFTER the content div,
        // prefixed "Figure N. " (figure-caption attribute + shared counter).
        if let Some(title) = self.block_title_inner_html.take() {
            output.push_str("<div class=\"title\">");
            let label = self.document_attrs.get("figure-caption").cloned();
            self.push_caption_prefix(output, meta, label.as_deref(), CaptionKind::Figure);
            output.push_str(&title);
            output.push_str("</div>\n");
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_inline_image(&self, output: &mut String, target: &CowStr<'_>, alt: &CowStr<'_>, width: &Option<CowStr<'_>>, height: &Option<CowStr<'_>>, float: &Option<CowStr<'_>>, link: &Option<CowStr<'_>>, role: &Option<CowStr<'_>>, title: &Option<CowStr<'_>>) {
        // Inline image span class = `image` + float + role (Asciidoctor's
        // convert_inline_image; `align` is not emitted for inline images).
        let mut img_class = String::from("image");
        if let Some(f) = float {
            img_class.push(' ');
            img_class.push_str(f);
        }
        if let Some(r) = role {
            img_class.push(' ');
            img_class.push_str(r);
        }
        output.push_str("<span class=\"");
        output.push_str(&img_class);
        output.push_str("\">");
        let has_link = link.is_some();
        if let Some(href) = link {
            output.push_str("<a class=\"image\" href=\"");
            html_escape(output, href);
            output.push_str("\">");
        }
        output.push_str("<img");
        write_attr(output, "src", &self.image_uri(target));
        let effective_alt = if alt.as_ref().is_empty() {
            auto_alt_from_target(target)
        } else {
            alt.to_string()
        };
        write_attr(output, "alt", &effective_alt);
        if let Some(w) = width {
            write_attr(output, "width", w);
        }
        if let Some(h) = height {
            write_attr(output, "height", h);
        }
        if let Some(t) = title {
            write_attr(output, "title", t);
        }
        output.push('>');
        if has_link {
            output.push_str("</a>");
        }
        output.push_str("</span>");
    }

    /// Resolve an image target to its final `src`, mirroring Asciidoctor's
    /// `image_uri` on the unsecure, non-`data-uri` path
    /// (`normalize_web_path target, imagesdir`): a URI target or a web-root
    /// (`/…`) target is used verbatim (spaces percent-encoded); otherwise a
    /// non-empty `imagesdir` document attribute is prefixed
    /// (`imagesdir` + `/` + target). `imagesdir` is read live from
    /// `document_attrs`, so a mid-document `:imagesdir:` entry applies to the
    /// images that follow it. (`.`/`..` segment collapsing inside the joined
    /// path is not performed — no corpus case needs it.)
    fn image_uri(&self, target: &str) -> String {
        let encoded = target.replace(' ', "%20");
        if is_uriish(target) || target.starts_with('/') {
            return encoded;
        }
        match self.document_attrs.get("imagesdir") {
            Some(dir) if !dir.is_empty() => {
                let sep = if dir.ends_with('/') { "" } else { "/" };
                format!("{dir}{sep}{encoded}")
            }
            _ => encoded,
        }
    }

    /// Build a base CSS class for block images, appending float then align
    /// classes from named attrs. Order is fixed (float before align), mirroring
    /// Asciidoctor's `convert_image` (`classes << float; classes << text-align`)
    /// rather than the unstable `named` map iteration order.
    pub(crate) fn image_base_class(default: &str, meta: &Option<BlockMeta>) -> String {
        let mut class = String::from(default);
        if let Some(m) = meta {
            let lookup = |key: &str| m.named.iter().find(|(k, _)| k == key).map(|(_, v)| v);
            // Build raw tokens; escaping happens once at the emission boundary
            // in write_meta_attrs (D1/D7 rule, avoids double-escape).
            if let Some(f) = lookup("float") {
                class.push(' ');
                class.push_str(f);
            }
            if let Some(a) = lookup("align") {
                let css = match a.as_str() {
                    "left" => "text-left",
                    "center" => "text-center",
                    "right" => "text-right",
                    other => other,
                };
                class.push(' ');
                class.push_str(css);
            }
        }
        class
    }
}
