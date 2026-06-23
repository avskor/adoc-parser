use std::borrow::Cow;
use std::collections::HashMap;

use crate::event::{CowStr, CellStyle, HAlign, SubstitutionSet, VAlign};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableFormat {
    Native,
    Csv,
    Dsv,
    Tsv,
}

/// A bare positional token: non-empty, not a `key=value` pair, and not pure
/// shorthand (`#id`/`.role`/`%opt`). Used to detect implied source shorthand
/// like `[,ruby]`, where slot 1 has no style but slot 2 is a language.
fn is_bare_positional(seg: &str) -> bool {
    let seg = seg.trim();
    !seg.is_empty() && !seg.contains('=') && !seg.starts_with(['#', '.', '%'])
}

fn split_respecting_quotes(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    // Open quote char (`"` or `'`). A quote only OPENS at the start of a
    // value — right after a comma (positional) or `=` (named) — so a
    // mid-word apostrophe (`Dad's words`) is plain text, mirroring
    // Asciidoctor's attrlist pattern that matches quoted values from the
    // slot start.
    let mut quote: Option<char> = None;
    let mut at_value_start = true;
    // True immediately after a closing quote (before any other character). In
    // Asciidoctor's AttributeList a quoted value's closing quote terminates the
    // attribute, so the following run of whitespace acts as a separator —
    // `[cols="1,2" options="header"]` is two attributes, not one. Whitespace
    // after an *unquoted* value or a shorthand token is part of that token
    // (scan_to_delimiter consumes up to the next comma), so it does not split.
    let mut after_close_quote = false;

    for (i, ch) in s.char_indices() {
        match ch {
            _ if quote == Some(ch) => {
                quote = None;
                after_close_quote = true;
            }
            '"' | '\'' if quote.is_none() && at_value_start => {
                quote = Some(ch);
                at_value_start = false;
            }
            ',' | '=' if quote.is_none() => {
                if ch == ',' {
                    parts.push(&s[start..i]);
                    start = i + 1;
                }
                at_value_start = true;
                after_close_quote = false;
            }
            c if c.is_whitespace() => {
                // Whitespace directly after a closing quote splits attributes.
                if after_close_quote {
                    parts.push(&s[start..i]);
                    start = i + ch.len_utf8();
                    at_value_start = true;
                    after_close_quote = false;
                }
            }
            _ => {
                at_value_start = false;
                after_close_quote = false;
            }
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Strip a single matching pair of enclosing quotes — either double (`"…"`) or
/// single (`'…'`) — mirroring the quote-awareness of `split_respecting_quotes`.
/// Asciidoctor drops the enclosing quotes for both forms (the difference is only
/// in substitution semantics: single-quoted values additionally get normal subs
/// applied — not yet reproduced for named values). Returns the input unchanged
/// when a matching pair is not present (so `"x`, `x"`, `'x`, or `x'` are intact,
/// and a mismatched `'x"` is left as-is).
fn strip_enclosing_quotes(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        // The quote byte is ASCII, so slicing past it stays on a char boundary.
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Extract the link text (first positional) and the `xrefstyle` named attribute
/// from a formal `xref:id[…]` bracket body. Mirrors Asciidoctor's
/// `extract_attributes_from_text`, which the inline xref handler runs only when
/// the body contains `=` (`substitutors.rb`); without `=` the whole body is the
/// literal link text, so the caller must gate on that. Returns borrowed slices
/// of `text` (each trimmed, enclosing quotes dropped). Named attributes other
/// than `xrefstyle` are ignored — this slice only consumes the style.
pub fn extract_xref_attrs(text: &str) -> (Option<&str>, Option<&str>) {
    let mut label = None;
    let mut xrefstyle = None;
    for part in split_respecting_quotes(text) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((key, value)) = part.split_once('=') {
            if key.trim() == "xrefstyle" {
                xrefstyle = Some(strip_enclosing_quotes(value.trim()));
            }
        } else if label.is_none() {
            label = Some(strip_enclosing_quotes(part));
        }
    }
    (label, xrefstyle)
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct AttributeStore {
    attrs: HashMap<String, String>,
}

#[allow(dead_code)]
impl AttributeStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: &str, value: &str) {
        self.attrs.insert(name.to_string(), value.to_string());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(|s| s.as_str())
    }

    pub fn resolve<'a>(&self, name: &str) -> Option<CowStr<'a>> {
        self.attrs.get(name).map(|v| Cow::Owned(v.clone()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockAttributes {
    pub id: Option<String>,
    pub roles: Vec<String>,
    pub options: Vec<String>,
    pub positional: Vec<String>,
    pub named: HashMap<String, String>,
    #[allow(dead_code)]
    pub title: Option<String>,
    /// Language for an *implied* source block written with shorthand
    /// (`[,ruby]`, `[#id,ruby]`, `[.role,ruby]`), where slot 1 carries no
    /// explicit block style but slot 2 is a bare language token. `None` for
    /// explicit `[source,...]` blocks (their language comes from `positional`).
    pub implied_source_lang: Option<String>,
    /// `true` when the first comma-separated attribute is a bare positional —
    /// i.e. an explicit block style sits at slot 1. `false` when slot 1 is
    /// consumed by a named (`id=`/`role=`/`key=`) or shorthand
    /// (`#id`/`.role`/`%opt`) attribute. AsciiDoc counts *every* attribute
    /// toward the positional index, so a leading named/shorthand attribute
    /// shifts the bare positionals: `[id=app, source, yaml]` puts `source` at
    /// slot 2 (the language) — not slot 1 (the style).
    pub first_positional_is_style: bool,
    /// Indices into `positional` whose values were written in single quotes —
    /// those get normal substitutions applied when used (attribution,
    /// citetitle), unlike double-quoted/bare values.
    pub single_quoted_positionals: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ColSpec {
    pub halign: HAlign,
    pub valign: VAlign,
    pub width: u8,
    pub style: CellStyle,
}

/// Parse a single column spec: `[multiplier*][halign][.valign][width][style]`
/// Returns `(count, ColSpec)` where count is the multiplier (default 1).
fn parse_col_spec(s: &str) -> (usize, ColSpec) {
    let mut rest = s;
    let mut count: usize = 1;

    // Check for multiplier: `N*`
    if let Some(star_pos) = rest.find('*') {
        let before = &rest[..star_pos];
        if !before.is_empty() && before.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(n) = before.parse::<usize>() {
                count = n.max(1);
            }
            rest = &rest[star_pos + 1..];
        }
    }

    let mut spec = ColSpec::default();

    // Parse halign: <, ^, >
    if let Some(stripped) = rest.strip_prefix('<') {
        spec.halign = HAlign::Left;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('^') {
        spec.halign = HAlign::Center;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('>') {
        spec.halign = HAlign::Right;
        rest = stripped;
    }

    // Parse valign: .<, .^, .>
    if let Some(stripped) = rest.strip_prefix(".<") {
        spec.valign = VAlign::Top;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix(".^") {
        spec.valign = VAlign::Middle;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix(".>") {
        spec.valign = VAlign::Bottom;
        rest = stripped;
    }

    // Parse width: digits (optionally with a trailing `%`) or the autowidth
    // marker `~`. asciidoctor's column-spec width token is `(\d+%?|~)`; a `~`
    // must be consumed here so the following style letter still parses
    // (e.g. `^~m` → center + autowidth + monospace).
    let digit_count = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count > 0 {
        if let Ok(w) = rest[..digit_count].parse::<u8>() {
            spec.width = w;
        }
        rest = &rest[digit_count..];
        if let Some(stripped) = rest.strip_prefix('%') {
            rest = stripped;
        }
    } else if let Some(stripped) = rest.strip_prefix('~') {
        rest = stripped;
    }

    // Parse style letter
    if rest.len() == 1 {
        match rest.as_bytes()[0] {
            b'a' => spec.style = CellStyle::AsciiDoc,
            b'h' => spec.style = CellStyle::Header,
            b'e' => spec.style = CellStyle::Emphasis,
            b'm' => spec.style = CellStyle::Monospace,
            b's' => spec.style = CellStyle::Strong,
            b'l' => spec.style = CellStyle::Literal,
            _ => {}
        }
    }

    (count, spec)
}

impl BlockAttributes {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.id.is_none()
            && self.roles.is_empty()
            && self.options.is_empty()
            && self.positional.is_empty()
            && self.named.is_empty()
            && self.title.is_none()
    }

    pub fn parse(attr_str: &str) -> Self {
        let mut attrs = BlockAttributes::new();
        if attr_str.is_empty() {
            return attrs;
        }

        // Legacy anchor syntax: [[id]] → outer brackets stripped by is_block_attribute,
        // so attr_str is "[id]". Treat as ID shorthand.
        if attr_str.starts_with('[') && attr_str.ends_with(']') && attr_str.len() > 2 {
            let id = &attr_str[1..attr_str.len() - 1];
            // Only treat as legacy anchor if the inner part doesn't contain brackets
            if !id.contains('[') && !id.contains(']') {
                // `[[id,xreflabel]]` — the part after the comma is reference text
                // for xrefs (never part of the id). Mirror the inline `try_anchor`
                // trimming (id trim_end, reftext trim_start) and stash the reftext
                // as the `reftext` attribute so an unlabeled `<<id>>` resolves to
                // it — the same channel as the named `[reftext=…]` form.
                match id.split_once(',') {
                    Some((i, reftext)) => {
                        attrs.id = Some(i.trim_end().to_string());
                        let reftext = reftext.trim_start();
                        if !reftext.is_empty() {
                            attrs.named.insert("reftext".to_string(), reftext.to_string());
                        }
                    }
                    None => attrs.id = Some(id.to_string()),
                }
                return attrs;
            }
        }

        let parts = split_respecting_quotes(attr_str);
        for (idx, part) in parts.iter().enumerate() {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((key, value)) = part.split_once('=') {
                // named: key=value — drop a matching pair of enclosing quotes
                // (`caption=''` → empty, `caption='Foo. '` → `Foo. `), both
                // single and double, as Asciidoctor does.
                let key = key.trim();
                let value = strip_enclosing_quotes(value.trim());
                // Promote id=, role= and options=/opts= to structural fields
                match key {
                    "id" => { attrs.id = Some(value.to_string()); }
                    "role" => { attrs.roles.push(value.to_string()); }
                    "options" | "opts" => {
                        attrs.options.extend(
                            value
                                .split(',')
                                .map(str::trim)
                                .filter(|o| !o.is_empty())
                                .map(str::to_string),
                        );
                    }
                    _ => { attrs.named.insert(key.to_string(), value.to_string()); }
                }
            } else if idx == 0
                && (part.starts_with('#') || part.starts_with('.') || part.starts_with('%'))
            {
                // Pure shorthand: #id.role1.role2%opt1 — only valid in the
                // style slot (first comma-part); elsewhere it is verbatim
                // positional text (`[quote,#bar]` → attribution "#bar").
                Self::parse_shorthand(part, &mut attrs);
            } else if idx == 0
                && let Some(pos) = part.find(['#', '.', '%'])
                && !part[..pos].contains(' ')
            {
                // Mixed: "discrete#myid.role" → positional + shorthand.
                // A marker preceded by a space is plain text, not shorthand
                // (e.g., "Captain James T. Kirk").
                attrs.positional.push(part[..pos].to_string());
                Self::parse_shorthand(&part[pos..], &mut attrs);
            } else {
                // Quoted positional values lose their enclosing quotes;
                // single-quoted ones additionally get normal substitutions
                // applied when used (Asciidoctor: only `'…'` values are
                // substituted — probe /tmp/p_subs/p12).
                if part.len() >= 2 && part.starts_with('\'') && part.ends_with('\'') {
                    attrs
                        .single_quoted_positionals
                        .push(attrs.positional.len());
                    attrs.positional.push(part[1..part.len() - 1].to_string());
                } else if part.len() >= 2 && part.starts_with('"') && part.ends_with('"') {
                    attrs.positional.push(part[1..part.len() - 1].to_string());
                } else {
                    attrs.positional.push(part.to_string());
                }
            }
        }

        // Does slot 1 hold an explicit block style? Only when the first
        // comma-separated part is a bare positional. A leading named/shorthand
        // attribute (`id=`/`#id`/`.role`/`%opt`) occupies slot 1 and shifts the
        // bare positionals down by one (AsciiDoc increments the positional index
        // for every attribute).
        attrs.first_positional_is_style = parts.first().is_some_and(|s| is_bare_positional(s));

        // Implied source shorthand: slot 1 carries no explicit block style
        // (it is empty or pure shorthand like #id/.role/%opt, or a named attr
        // like id=) while slot 2 is a bare language token. AsciiDoc renders such
        // verbatim blocks as `source` (e.g. `[,ruby]`, `[#hello,ruby]`,
        // `[.role,ruby]`, `[id=app, source, yaml]` → language `source`).
        if !attrs.first_positional_is_style
            && let Some(lang) = parts.get(1)
            && is_bare_positional(lang)
        {
            attrs.implied_source_lang = Some(lang.trim().to_string());
        }

        // Slot 3 of a source block is `linenums`: any non-empty positional
        // value there enables numbering (`[source,ruby,linenums]`,
        // `[source,ruby,%linenums]`, `[,ruby,linenums]`). A named attribute
        // in that slot does not fill it (`[source,ruby,start=10]` → off).
        let is_source_style = attrs.first_positional_is_style
            && parts.first().is_some_and(|s| s.trim() == "source");
        if (is_source_style || attrs.implied_source_lang.is_some())
            && let Some(third) = parts.get(2).map(|s| s.trim())
            && !third.is_empty()
            && !third.contains('=')
        {
            attrs.options.push("linenums".to_string());
        }

        attrs
    }

    /// Merge a later block-attribute line into an earlier one, mirroring how
    /// Asciidoctor accumulates stacked metadata lines above a block: named
    /// attributes override by key, the id is last-wins, roles and options
    /// accumulate, and positional slots override per raw slot (an empty slot
    /// in the later line keeps the earlier value: `[source,ruby]` + `[,python]`
    /// → language `python`, `[quote,Author]` + `[verse]` → verse with the
    /// attribution kept).
    pub fn merge(older: Self, newer: Self) -> Self {
        let mut result = older;
        if newer.id.is_some() {
            result.id = newer.id;
        }
        result.roles.extend(newer.roles);
        result.options.extend(newer.options);
        result.named.extend(newer.named);
        if newer.title.is_some() {
            result.title = newer.title;
        }

        if newer.first_positional_is_style {
            // The later line claims the style slot: its bare positionals
            // override the earlier ones index by index; earlier tail slots
            // beyond the later line's length are kept.
            let older_implied = result.implied_source_lang.take();
            let older_pos = std::mem::take(&mut result.positional);
            let aligned_older: Vec<String> = if result.first_positional_is_style {
                older_pos
            } else {
                // The earlier line had no style slot: its language token
                // (if any) sits at raw slot 2, i.e. right after the style.
                let mut v = vec![String::new()];
                v.extend(older_implied);
                v
            };
            let mut merged = newer.positional;
            for (i, val) in aligned_older.into_iter().enumerate() {
                if i >= merged.len() && !val.is_empty() {
                    merged.push(val);
                }
            }
            result.positional = merged;
            result.first_positional_is_style = true;
            result.implied_source_lang = newer.implied_source_lang;
            // The later line's positionals replaced the slots, so its
            // single-quote flags travel with them (older flags would point
            // at overridden slots).
            result.single_quoted_positionals = newer.single_quoted_positionals;
        } else if let Some(lang) = newer.implied_source_lang {
            // Later line of shape `[,lang]`: raw slot 2 overrides the
            // language slot of the merged attributes.
            if result.first_positional_is_style {
                if result.positional.len() >= 2 {
                    result.positional[1] = lang;
                } else {
                    result.positional.push(lang);
                }
            } else {
                result.implied_source_lang = Some(lang);
            }
        }
        result
    }

    fn parse_shorthand(s: &str, attrs: &mut Self) {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let marker = bytes[i];
            i += 1;
            let start = i;
            while i < bytes.len() && !matches!(bytes[i], b'#' | b'.' | b'%') {
                i += 1;
            }
            let value = &s[start..i];
            if value.is_empty() {
                continue;
            }
            match marker {
                b'#' => attrs.id = Some(value.to_string()),
                b'.' => attrs.roles.push(value.to_string()),
                b'%' => attrs.options.push(value.to_string()),
                _ => {}
            }
        }
    }

    pub fn block_style_kind(&self) -> Option<&str> {
        match self.positional.first().map(|s| s.as_str())? {
            s @ ("listing" | "literal" | "source" | "verse" | "quote" | "example" | "sidebar" | "pass" | "partintro" | "open") => Some(s),
            _ => None,
        }
    }

    pub fn admonition_kind(&self) -> Option<crate::event::AdmonitionKind> {
        match self.positional.first().map(|s| s.as_str())? {
            "NOTE" => Some(crate::event::AdmonitionKind::Note),
            "TIP" => Some(crate::event::AdmonitionKind::Tip),
            "IMPORTANT" => Some(crate::event::AdmonitionKind::Important),
            "WARNING" => Some(crate::event::AdmonitionKind::Warning),
            "CAUTION" => Some(crate::event::AdmonitionKind::Caution),
            _ => None,
        }
    }

    pub fn source_language(&self) -> Option<&str> {
        // Explicit `[source, lang]` only when `source` actually sits at slot 1.
        // When a leading named/shorthand attribute shifts the positionals,
        // the language is captured by `implied_source_lang` instead.
        if self.first_positional_is_style && self.positional.first().map(|s| s.as_str()) == Some("source") {
            self.positional.get(1).map(|s| s.as_str())
        } else {
            self.implied_source_lang.as_deref()
        }
    }

    pub fn is_source_block(&self) -> bool {
        (self.first_positional_is_style
            && self.positional.first().map(|s| s.as_str()) == Some("source"))
            || self.implied_source_lang.is_some()
    }

    pub fn is_verse_style(&self) -> bool {
        self.positional.first().map(|s| s.as_str()) == Some("verse")
    }

    pub fn table_cols_count(&self) -> Option<usize> {
        if let Some(specs) = self.table_col_specs() {
            return Some(specs.len());
        }
        None
    }

    pub fn table_col_specs(&self) -> Option<Vec<ColSpec>> {
        let val = self.named.get("cols")?;
        let trimmed = val.trim();

        // Simple numeric: cols="3" → 3 columns with defaults
        if let Ok(n) = trimmed.parse::<usize>() {
            return Some(vec![ColSpec::default(); n]);
        }

        // Comma- or semicolon-separated specs: cols="<,^,>" or cols="^.>2,<1"
        // or cols="3*^". Asciidoctor picks the separator by presence: if a
        // comma appears it splits on comma, otherwise on semicolon. This is
        // why `[cols=1;m;m]` works unquoted — semicolons survive the attrlist
        // splitter (which itself consumes commas), so authors use `;` to avoid
        // quoting. Mixed separators yield garbage specs on the non-split char,
        // matching asciidoctor's lone-separator rule.
        let sep = if trimmed.contains(',') { ',' } else { ';' };
        let mut specs = Vec::new();
        for part in trimmed.split(sep) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (count, spec) = parse_col_spec(part);
            for _ in 0..count {
                specs.push(spec.clone());
            }
        }

        if specs.is_empty() {
            None
        } else {
            Some(specs)
        }
    }

    pub fn stem_variant(&self) -> Option<&str> {
        match self.positional.first().map(|s| s.as_str()) {
            Some("stem") | Some("latexmath") | Some("asciimath") => {
                self.positional.first().map(|s| s.as_str())
            }
            _ => None,
        }
    }

    pub fn has_option(&self, name: &str) -> bool {
        self.options.iter().any(|o| o == name)
    }

    pub fn list_start(&self) -> Option<u32> {
        self.named.get("start")?.parse().ok()
    }

    pub fn is_reversed(&self) -> bool {
        self.has_option("reversed")
    }

    pub fn substitution_set(&self, default: SubstitutionSet) -> Option<SubstitutionSet> {
        self.named.get("subs").map(|v| parse_subs_value(v, default))
    }

    /// The `indent` attribute that reindents verbatim block content
    /// (Asciidoctor `adjust_indentation!`). Parsed like Ruby `String#to_i`
    /// (optional sign + leading digits; non-numeric → 0). `None` when the
    /// attribute is absent — in that case the content indentation is preserved.
    pub fn verbatim_indent(&self) -> Option<i32> {
        let raw = self.named.get("indent")?.trim();
        let (neg, digits) = match raw.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, raw.strip_prefix('+').unwrap_or(raw)),
        };
        let n: i32 = digits
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        Some(if neg { -n } else { n })
    }

    pub fn table_format(&self) -> TableFormat {
        // Check named attribute: format=csv / format=dsv / format=tsv
        if let Some(fmt) = self.named.get("format") {
            match fmt.as_str() {
                "csv" => return TableFormat::Csv,
                "dsv" => return TableFormat::Dsv,
                "tsv" => return TableFormat::Tsv,
                _ => {}
            }
        }
        // Check positional shorthand: [csv], [dsv], [tsv]
        if let Some(first) = self.positional.first() {
            match first.as_str() {
                "csv" => return TableFormat::Csv,
                "dsv" => return TableFormat::Dsv,
                "tsv" => return TableFormat::Tsv,
                _ => {}
            }
        }
        TableFormat::Native
    }
}

pub struct ImageAttrs<'a> {
    pub alt: &'a str,
    pub width: Option<&'a str>,
    pub height: Option<&'a str>,
    pub align: Option<&'a str>,
    pub float: Option<&'a str>,
    pub link: Option<&'a str>,
    pub role: Option<&'a str>,
    pub caption: Option<&'a str>,
    pub title: Option<&'a str>,
    /// `format=` named attribute (e.g. `svg`); selects the SVG rendering path
    /// alongside a `.svg` target extension.
    pub format: Option<&'a str>,
    /// `fallback=` named attribute: image shown when an interactive `<object>`
    /// SVG cannot be displayed by the browser.
    pub fallback: Option<&'a str>,
    /// Set when the `interactive` option (`opts=interactive`) is present.
    pub interactive: bool,
}

pub fn parse_image_attrs(bracket_content: &str) -> ImageAttrs<'_> {
    if bracket_content.is_empty() {
        return ImageAttrs {
            alt: "",
            width: None,
            height: None,
            align: None,
            float: None,
            link: None,
            role: None,
            caption: None,
            title: None,
            format: None,
            fallback: None,
            interactive: false,
        };
    }

    let mut alt: Option<&str> = None;
    let mut width: Option<&str> = None;
    let mut height: Option<&str> = None;
    let mut align: Option<&str> = None;
    let mut float: Option<&str> = None;
    let mut link: Option<&str> = None;
    let mut role: Option<&str> = None;
    let mut caption: Option<&str> = None;
    let mut title: Option<&str> = None;
    let mut format: Option<&str> = None;
    let mut fallback: Option<&str> = None;
    let mut interactive = false;
    let mut positional = Vec::new();

    for part in split_respecting_quotes(bracket_content) {
        let part = part.trim();
        if part.is_empty() {
            positional.push(part);
            continue;
        }
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = strip_enclosing_quotes(value.trim());
            match key {
                "alt" => alt = Some(value),
                "width" => width = Some(value),
                "height" => height = Some(value),
                "align" => align = Some(value),
                "float" => float = Some(value),
                "link" => link = Some(value),
                "role" => role = Some(value),
                "caption" => caption = Some(value),
                "title" => title = Some(value),
                "format" => format = Some(value),
                "fallback" => fallback = Some(value),
                // `opts`/`options` is a comma-separated list; only `interactive`
                // affects the HTML output (SVG `<object>` rendering).
                "opts" | "options" => {
                    interactive = value.split(',').any(|o| o.trim() == "interactive");
                }
                _ => {}
            }
        } else {
            // Positional values (alt is positional[0]) may be quoted, e.g.
            // `image::x["Alt text",role=…]`. Asciidoctor strips the enclosing
            // quotes; the named branch above already does.
            positional.push(strip_enclosing_quotes(part));
        }
    }

    // alt: named "alt" or positional[0]; with only named attrs present the alt
    // is empty and the renderer auto-generates it from the filename, matching
    // Asciidoctor (`image::a.png[width=100]` → alt="a").
    let alt = alt.unwrap_or_else(|| positional.first().copied().unwrap_or(""));
    // width: named "width" or positional[1]
    if width.is_none()
        && let Some(&w) = positional.get(1)
        && !w.is_empty()
    {
        width = Some(w);
    }
    // height: named "height" or positional[2]
    if height.is_none()
        && let Some(&h) = positional.get(2)
        && !h.is_empty()
    {
        height = Some(h);
    }

    ImageAttrs { alt, width, height, align, float, link, role, caption, title, format, fallback, interactive }
}

pub struct LinkAttrs<'a> {
    pub text: &'a str,
    pub window: Option<&'a str>,
    pub nofollow: bool,
    pub role: Option<&'a str>,
    /// Named `id=` — rendered as the `<a>`'s `id` attribute (Asciidoctor `node.id`).
    pub id: Option<&'a str>,
    /// Named `title=` — rendered as the `<a>`'s `title` attribute.
    pub title: Option<&'a str>,
    /// Positional attrs 2 and 3 — mailto subject/body (other macros ignore them).
    pub subject: Option<&'a str>,
    pub body: Option<&'a str>,
}

/// Which inline macro is requesting attribute parsing. This governs whether the
/// bracketed content is parsed as an attribute list (commas split positional /
/// named attributes) or kept verbatim as the link text.
///
/// Asciidoctor parses a `link:`/URL-macro/autolink bracket as an attribute list
/// ONLY when it contains a named attribute (`key=value`, key being a valid
/// attribute name) or is quote-wrapped; otherwise the whole content is the
/// visible text, commas and all (`link:url[A, B, C]` → "A, B, C"). `mailto:` is
/// the exception: it always splits positionally so the 2nd/3rd values become the
/// `?subject=&body=` query parameters even without an `=` sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Link,
    Mailto,
}

pub fn parse_link_attrs(bracket_content: &str, kind: LinkKind) -> LinkAttrs<'_> {
    if bracket_content.is_empty() {
        return LinkAttrs {
            text: "",
            window: None,
            nofollow: false,
            role: None,
            id: None,
            title: None,
            subject: None,
            body: None,
        };
    }

    let mut window: Option<&str> = None;
    let mut nofollow = false;
    let mut role: Option<&str> = None;
    let mut id: Option<&str> = None;
    let mut title: Option<&str> = None;
    let mut positional = Vec::new();
    let mut found_named = false;

    for part in split_respecting_quotes(bracket_content) {
        let part = part.trim();
        if part.is_empty() {
            positional.push(part);
            continue;
        }
        // Named attr only when the key is a plausible attribute name; a quoted
        // positional containing '=' ("a=b") must stay positional.
        let named = part.split_once('=').filter(|(key, _)| {
            let key = key.trim();
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        });
        if let Some((key, value)) = named {
            found_named = true;
            let key = key.trim();
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            match key {
                "window" => window = Some(value),
                "opts" if value == "nofollow" => nofollow = true,
                "role" => role = Some(value),
                "id" => id = Some(value),
                "title" => title = Some(value),
                _ => {}
            }
        } else {
            positional.push(part);
        }
    }

    fn strip_quotes(s: &str) -> &str {
        s.strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(s)
    }
    let subject = positional.get(1).copied().map(strip_quotes).filter(|s| !s.is_empty());
    let body = positional.get(2).copied().map(strip_quotes).filter(|s| !s.is_empty());

    // Link text. For a `link:`/URL macro whose bracket is NOT an attribute list
    // — no named attribute and not quote-wrapped — Asciidoctor keeps the ENTIRE
    // content as the visible text, commas included (`link:url[A, B, C]` →
    // "A, B, C"). The trim mirrors the old single-positional path, so no-comma /
    // no-`=` inputs are byte-for-byte unchanged. mailto and any attribute-list
    // bracket keep the positional split (positional[0] is the text); a named-only
    // attrlist leaves text empty so the caller falls back to the bare form.
    let mut text = if kind == LinkKind::Link
        && !found_named
        && !bracket_content.trim_start().starts_with('"')
    {
        bracket_content.trim()
    } else {
        strip_quotes(positional.first().copied().unwrap_or(""))
    };

    // Blank-window shorthand: a trailing `^` on the link text opens the link in a
    // new window. Asciidoctor strips the caret from the visible text and sets
    // window=_blank (the renderer then adds target="_blank" rel="noopener"); an
    // explicit `window=` attribute wins.
    if let Some(stripped) = text.strip_suffix('^') {
        text = stripped;
        if window.is_none() {
            window = Some("_blank");
        }
    }

    LinkAttrs { text, window, nofollow, role, id, title, subject, body }
}

pub fn parse_subs_value(value: &str, default: SubstitutionSet) -> SubstitutionSet {
    let trimmed = value.trim();
    match trimmed {
        "normal" => SubstitutionSet::NORMAL,
        "verbatim" => SubstitutionSet::VERBATIM,
        "none" => SubstitutionSet::NONE,
        v => {
            // Modifier tokens trigger incremental mode: leading `+` (append),
            // trailing `+` (prepend) or leading `-` (remove).
            let has_incremental = v.split(',').any(|p| {
                let p = p.trim();
                p.starts_with('+') || p.starts_with('-') || p.ends_with('+')
            });

            if has_incremental {
                // Mirrors asciidoctor resolve_subs: the first MODIFIER token
                // seeds the accumulator with the block's default subs, while a
                // plain token appearing first seeds it empty (replacement);
                // later tokens of either kind operate on the accumulated set.
                // Asciidoctor tracks application ORDER (prepend runs the sub
                // before the defaults), which a flag set cannot represent —
                // membership only.
                let mut acc: Option<SubstitutionSet> = None;
                for part in v.split(',') {
                    let part = part.trim();
                    if let Some(name) = part.strip_prefix('+') {
                        if let Some(f) = sub_name_to_flags(name.trim()) {
                            acc.get_or_insert(default).add(f);
                        }
                    } else if let Some(name) = part.strip_prefix('-') {
                        if let Some(f) = sub_name_to_flags(name.trim()) {
                            acc.get_or_insert(default).remove(f);
                        }
                    } else if let Some(name) = part.strip_suffix('+') {
                        if let Some(f) = sub_name_to_flags(name.trim()) {
                            acc.get_or_insert(default).add(f);
                        }
                    } else if let Some(f) = sub_name_to_flags(part) {
                        acc.get_or_insert(SubstitutionSet::NONE).add(f);
                    }
                }
                acc.unwrap_or(default)
            } else {
                // Explicit list: "specialchars,attributes"
                let mut result = SubstitutionSet::NONE;
                for part in v.split(',') {
                    match part.trim() {
                        "normal" => return SubstitutionSet::NORMAL,
                        "verbatim" => {
                            result.add(SubstitutionSet::SPECIALCHARS | SubstitutionSet::CALLOUTS);
                        }
                        name => {
                            if let Some(f) = sub_name_to_flag(name) {
                                result.add(f);
                            }
                        }
                    }
                }
                result
            }
        }
    }
}

/// Like [`sub_name_to_flag`], but also accepts the composite group names
/// asciidoctor allows in incremental `subs=` tokens (`+verbatim`, `-normal`).
/// Shared with the inline `pass:SPEC[…]` macro (full-name tokens).
pub(crate) fn sub_name_to_flags(name: &str) -> Option<u8> {
    match name {
        "normal" => Some(
            SubstitutionSet::SPECIALCHARS
                | SubstitutionSet::QUOTES
                | SubstitutionSet::ATTRIBUTES
                | SubstitutionSet::REPLACEMENTS
                | SubstitutionSet::MACROS
                | SubstitutionSet::POST_REPLACEMENTS,
        ),
        "verbatim" => Some(SubstitutionSet::SPECIALCHARS | SubstitutionSet::CALLOUTS),
        "none" => Some(0),
        _ => sub_name_to_flag(name),
    }
}

fn sub_name_to_flag(name: &str) -> Option<u8> {
    match name {
        "specialchars" | "specialcharacters" => Some(SubstitutionSet::SPECIALCHARS),
        "quotes" => Some(SubstitutionSet::QUOTES),
        "attributes" => Some(SubstitutionSet::ATTRIBUTES),
        "replacements" => Some(SubstitutionSet::REPLACEMENTS),
        "macros" => Some(SubstitutionSet::MACROS),
        "post_replacements" => Some(SubstitutionSet::POST_REPLACEMENTS),
        "callouts" => Some(SubstitutionSet::CALLOUTS),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_store() {
        let mut store = AttributeStore::new();
        store.set("toc", "left");
        assert_eq!(store.get("toc"), Some("left"));
        assert_eq!(store.get("missing"), None);
    }

    #[test]
    fn test_link_attrs_blank_window_caret() {
        // Trailing `^` on the link text ⇒ window=_blank, caret stripped from text.
        let a = parse_link_attrs("macro^", LinkKind::Link);
        assert_eq!(a.text, "macro");
        assert_eq!(a.window, Some("_blank"));
        assert!(!a.nofollow);

        // Caret combined with a named role attribute: caret is on the positional text.
        let a = parse_link_attrs("label^,role=external", LinkKind::Link);
        assert_eq!(a.text, "label");
        assert_eq!(a.window, Some("_blank"));

        // No caret ⇒ no implied window.
        let a = parse_link_attrs("plain", LinkKind::Link);
        assert_eq!(a.text, "plain");
        assert_eq!(a.window, None);

        // Explicit window= wins over the caret shorthand (no override).
        let a = parse_link_attrs("x^,window=_self", LinkKind::Link);
        assert_eq!(a.text, "x");
        assert_eq!(a.window, Some("_self"));
    }

    #[test]
    fn test_block_attributes_parse_source() {
        let attrs = BlockAttributes::parse("source,rust");
        assert_eq!(attrs.positional, vec!["source", "rust"]);
        assert!(attrs.is_source_block());
        assert_eq!(attrs.source_language(), Some("rust"));
    }

    #[test]
    fn test_implied_source_shorthand() {
        // `[,ruby]` — empty style + language ⇒ implied source block.
        let attrs = BlockAttributes::parse(",ruby");
        assert!(attrs.is_source_block(), "[,ruby] should be a source block");
        assert_eq!(attrs.source_language(), Some("ruby"));

        // `[#hello,ruby]` — id shorthand + language ⇒ implied source.
        let attrs = BlockAttributes::parse("#hello,ruby");
        assert_eq!(attrs.id.as_deref(), Some("hello"));
        assert!(attrs.is_source_block());
        assert_eq!(attrs.source_language(), Some("ruby"));

        // `[.role,ruby]` — role shorthand + language ⇒ implied source.
        let attrs = BlockAttributes::parse(".role,ruby");
        assert_eq!(attrs.roles, vec!["role"]);
        assert!(attrs.is_source_block());
        assert_eq!(attrs.source_language(), Some("ruby"));
    }

    #[test]
    fn test_single_positional_is_not_implied_source() {
        // `[ruby]` — a lone positional is a block style, NOT a source language.
        let attrs = BlockAttributes::parse("ruby");
        assert!(!attrs.is_source_block(), "[ruby] must not become a source block");
        assert_eq!(attrs.implied_source_lang, None);
    }

    #[test]
    fn test_leading_named_attr_shifts_positionals() {
        // A leading named attribute occupies slot 1, so `source` lands at slot 2
        // (the language) and `yaml` at slot 3 (ignored): AsciiDoc renders this as
        // a source block with language `source`, not `yaml`.
        let attrs = BlockAttributes::parse("id=app, source, yaml");
        assert_eq!(attrs.id.as_deref(), Some("app"));
        assert!(!attrs.first_positional_is_style);
        assert!(attrs.is_source_block());
        assert_eq!(attrs.source_language(), Some("source"));

        // Same shift via a leading shorthand id.
        let attrs = BlockAttributes::parse("#app, source, yaml");
        assert_eq!(attrs.source_language(), Some("source"));

        // Two leading named attributes consume slots 1 and 2, so the language
        // slot is taken by `role=x` (not bare) — NOT a source block.
        let attrs = BlockAttributes::parse("id=app, role=x, source, yaml");
        assert!(!attrs.is_source_block());

        // Explicit `[source, lang]` (style at slot 1) is unchanged.
        let attrs = BlockAttributes::parse("source, yaml");
        assert!(attrs.first_positional_is_style);
        assert_eq!(attrs.source_language(), Some("yaml"));

        // `[src, yaml]` — `src` is an unknown style at slot 1, not `source`.
        let attrs = BlockAttributes::parse("src, yaml");
        assert!(!attrs.is_source_block());
    }

    #[test]
    fn test_block_attributes_parse_id() {
        let attrs = BlockAttributes::parse("#my-id");
        assert_eq!(attrs.id.as_deref(), Some("my-id"));
    }

    #[test]
    fn test_legacy_anchor_xreflabel_captured_as_reftext() {
        // [[id,xreflabel]] → attr_str "[id,xreflabel]"; the part after the comma
        // is reference text for xrefs (never part of the id), captured as the
        // `reftext` attribute so an unlabeled `<<id>>` resolves to it.
        let attrs = BlockAttributes::parse("[tiger-image,Image of a tiger]");
        assert_eq!(attrs.id.as_deref(), Some("tiger-image"));
        assert_eq!(attrs.named.get("reftext").map(String::as_str), Some("Image of a tiger"));
        // Leading whitespace after the comma is trimmed (mirrors inline anchor).
        let attrs = BlockAttributes::parse("[id2, Spaced Ref]");
        assert_eq!(attrs.id.as_deref(), Some("id2"));
        assert_eq!(attrs.named.get("reftext").map(String::as_str), Some("Spaced Ref"));
        // No comma → no reftext; empty reftext (`[id,]`) → none.
        let attrs = BlockAttributes::parse("[plain-id]");
        assert_eq!(attrs.id.as_deref(), Some("plain-id"));
        assert!(attrs.named.get("reftext").is_none());
        let attrs = BlockAttributes::parse("[id3,]");
        assert_eq!(attrs.id.as_deref(), Some("id3"));
        assert!(attrs.named.get("reftext").is_none());
    }

    #[test]
    fn test_block_attributes_parse_role() {
        let attrs = BlockAttributes::parse(".role1.role2");
        assert_eq!(attrs.roles, vec!["role1", "role2"]);

        // Shorthand in a later comma-part is verbatim positional text,
        // not a role (matches Asciidoctor).
        let attrs = BlockAttributes::parse(".role1,.role2");
        assert_eq!(attrs.roles, vec!["role1"]);
        assert_eq!(attrs.positional, vec![".role2"]);
    }

    #[test]
    fn test_block_attributes_empty() {
        let attrs = BlockAttributes::new();
        assert!(attrs.is_empty());
    }

    #[test]
    fn test_table_cols_count() {
        let attrs = BlockAttributes::parse("cols=\"3\"");
        assert_eq!(attrs.table_cols_count(), Some(3));

        let attrs = BlockAttributes::parse("cols=\"1,1,1\"");
        assert_eq!(attrs.table_cols_count(), Some(3));

        let attrs = BlockAttributes::new();
        assert_eq!(attrs.table_cols_count(), None);
    }

    #[test]
    fn test_whitespace_after_quote_splits_attributes() {
        // Asciidoctor: a quoted value's closing quote terminates the attribute,
        // so the following whitespace separates it from the next one. Both
        // `cols` and `options` must be recognized in `[cols="1,2" options="header"]`.
        let attrs = BlockAttributes::parse("cols=\"1,2\" options=\"header\"");
        assert_eq!(attrs.table_cols_count(), Some(2));
        assert!(attrs.has_option("header"));

        // Order does not matter; multiple spaces collapse the same way.
        let attrs = BlockAttributes::parse("options=\"header\"    cols=\"1,2\"");
        assert_eq!(attrs.table_cols_count(), Some(2));
        assert!(attrs.has_option("header"));

        // A quoted value followed by an unquoted attribute also splits.
        let attrs = BlockAttributes::parse("cols=\"1,2\" options=header");
        assert_eq!(attrs.table_cols_count(), Some(2));
        assert!(attrs.has_option("header"));

        // Whitespace after an *unquoted* value does NOT split — it is consumed
        // into that value up to the next comma (so `options` is not a separate
        // attribute here), matching Asciidoctor's scan_to_delimiter.
        let attrs = BlockAttributes::parse("cols=2 options=header");
        assert!(!attrs.has_option("header"));

        // Whitespace after a shorthand token likewise does not split.
        let attrs = BlockAttributes::parse(".cls options=\"header\"");
        assert!(!attrs.has_option("header"));
    }

    #[test]
    fn test_is_verse_style() {
        let attrs = BlockAttributes::parse("verse");
        assert!(attrs.is_verse_style());

        let attrs = BlockAttributes::parse("source,rust");
        assert!(!attrs.is_verse_style());

        let attrs = BlockAttributes::new();
        assert!(!attrs.is_verse_style());
    }

    #[test]
    fn test_has_option() {
        let attrs = BlockAttributes::parse("%header");
        assert!(attrs.has_option("header"));
        assert!(!attrs.has_option("footer"));

        let attrs = BlockAttributes::parse("%header%footer");
        assert!(attrs.has_option("header"));
        assert!(attrs.has_option("footer"));

        // An option only parses in the first comma-part; "%footer" in the
        // second part is positional text (matches Asciidoctor).
        let attrs = BlockAttributes::parse("%header,%footer");
        assert!(attrs.has_option("header"));
        assert!(!attrs.has_option("footer"));
    }

    #[test]
    fn test_shorthand_id_and_roles() {
        let attrs = BlockAttributes::parse("#notice.important");
        assert_eq!(attrs.id.as_deref(), Some("notice"));
        assert_eq!(attrs.roles, vec!["important"]);
    }

    #[test]
    fn test_shorthand_multiple_roles() {
        let attrs = BlockAttributes::parse(".role1.role2.role3");
        assert_eq!(attrs.roles, vec!["role1", "role2", "role3"]);
        assert!(attrs.id.is_none());
    }

    #[test]
    fn test_shorthand_id_and_multiple_roles() {
        let attrs = BlockAttributes::parse("#myid.role1.role2");
        assert_eq!(attrs.id.as_deref(), Some("myid"));
        assert_eq!(attrs.roles, vec!["role1", "role2"]);
    }

    #[test]
    fn test_shorthand_options() {
        let attrs = BlockAttributes::parse("%opt1%opt2");
        assert_eq!(attrs.options, vec!["opt1", "opt2"]);
    }

    #[test]
    fn test_shorthand_id_and_options() {
        let attrs = BlockAttributes::parse("#myid%opt1%opt2");
        assert_eq!(attrs.id.as_deref(), Some("myid"));
        assert_eq!(attrs.options, vec!["opt1", "opt2"]);
    }

    #[test]
    fn test_block_attributes_merge_stacked_lines() {
        // Named attributes from both lines survive; later overrides by key
        let merged = BlockAttributes::merge(
            BlockAttributes::parse("caption=\"Table A. \""),
            BlockAttributes::parse("cols=\"3*\""),
        );
        assert_eq!(merged.named.get("caption").map(String::as_str), Some("Table A. "));
        assert_eq!(merged.named.get("cols").map(String::as_str), Some("3*"));

        // id last-wins, roles accumulate (probe-verified vs asciidoctor)
        let merged = BlockAttributes::merge(
            BlockAttributes::parse("#id1.r1"),
            BlockAttributes::parse("#id2.r2"),
        );
        assert_eq!(merged.id.as_deref(), Some("id2"));
        assert_eq!(merged.roles, vec!["r1", "r2"]);

        // Later style slot overrides, earlier tail positionals are kept:
        // [quote,Author] + [verse] → verse with attribution
        let merged = BlockAttributes::merge(
            BlockAttributes::parse("quote,Author Name"),
            BlockAttributes::parse("verse"),
        );
        assert_eq!(merged.positional, vec!["verse", "Author Name"]);
        assert!(merged.first_positional_is_style);

        // Empty slot 1 in the later line keeps the style, slot 2 overrides:
        // [source,ruby] + [,python] → python
        let merged = BlockAttributes::merge(
            BlockAttributes::parse("source,ruby"),
            BlockAttributes::parse(",python"),
        );
        assert_eq!(merged.source_language(), Some("python"));

        // Options accumulate: [%header] + [cols="2*"]
        let merged = BlockAttributes::merge(
            BlockAttributes::parse("%header"),
            BlockAttributes::parse("cols=\"2*\""),
        );
        assert!(merged.has_option("header"));
        assert_eq!(merged.table_cols_count(), Some(2));
    }

    #[test]
    fn test_named_attr_single_quoted_value_unquoted() {
        // Asciidoctor drops the enclosing quotes for both single- and
        // double-quoted named values. An empty single-quoted caption
        // (`[caption='']`) becomes the empty string, which the renderer
        // treats as "no caption prefix" (matches asciidoctor 2.0.23).
        let attrs = BlockAttributes::parse("caption=''");
        assert_eq!(attrs.named.get("caption").map(String::as_str), Some(""));

        let attrs = BlockAttributes::parse("caption='Foo. '");
        assert_eq!(attrs.named.get("caption").map(String::as_str), Some("Foo. "));

        // Double-quoted form unchanged (regression guard).
        let attrs = BlockAttributes::parse("caption=\"Bar. \"");
        assert_eq!(attrs.named.get("caption").map(String::as_str), Some("Bar. "));

        // A bare value with an inner apostrophe is left intact — the value
        // neither starts nor ends with a quote.
        let attrs = BlockAttributes::parse("caption=don't");
        assert_eq!(attrs.named.get("caption").map(String::as_str), Some("don't"));

        // A lone quote (no matching closer) is not stripped.
        let attrs = BlockAttributes::parse("caption='");
        assert_eq!(attrs.named.get("caption").map(String::as_str), Some("'"));

        // Single quotes are also dropped for structural keys (id/role).
        let attrs = BlockAttributes::parse("role='myrole'");
        assert_eq!(attrs.roles, vec!["myrole"]);
    }

    #[test]
    fn test_strip_enclosing_quotes_both_forms() {
        assert_eq!(strip_enclosing_quotes("\"abc\""), "abc");
        assert_eq!(strip_enclosing_quotes("'abc'"), "abc");
        assert_eq!(strip_enclosing_quotes("''"), "");
        assert_eq!(strip_enclosing_quotes("\"\""), "");
        // Mismatched / lone / inner quotes are preserved.
        assert_eq!(strip_enclosing_quotes("'abc\""), "'abc\"");
        assert_eq!(strip_enclosing_quotes("'"), "'");
        assert_eq!(strip_enclosing_quotes("\""), "\"");
        assert_eq!(strip_enclosing_quotes("don't"), "don't");
        assert_eq!(strip_enclosing_quotes("abc"), "abc");
    }

    #[test]
    fn test_mixed_positional_and_shorthand() {
        let attrs = BlockAttributes::parse("discrete#myid.role");
        assert_eq!(attrs.positional, vec!["discrete"]);
        assert_eq!(attrs.id.as_deref(), Some("myid"));
        assert_eq!(attrs.roles, vec!["role"]);
    }

    #[test]
    fn test_source_with_shorthand_id() {
        let attrs = BlockAttributes::parse("source#code1,rust");
        assert_eq!(attrs.positional, vec!["source", "rust"]);
        assert_eq!(attrs.id.as_deref(), Some("code1"));

        // "#code1" in slot 3 is no id — it is a non-empty positional that
        // turns on linenums (matches Asciidoctor).
        let attrs = BlockAttributes::parse("source,rust,#code1");
        assert_eq!(attrs.positional, vec!["source", "rust", "#code1"]);
        assert_eq!(attrs.id, None);
        assert!(attrs.options.iter().any(|o| o == "linenums"));
    }

    #[test]
    fn test_list_start() {
        let attrs = BlockAttributes::parse("start=5");
        assert_eq!(attrs.list_start(), Some(5));

        let attrs = BlockAttributes::parse("start=1");
        assert_eq!(attrs.list_start(), Some(1));

        let attrs = BlockAttributes::new();
        assert_eq!(attrs.list_start(), None);
    }

    #[test]
    fn test_is_reversed() {
        let attrs = BlockAttributes::parse("%reversed");
        assert!(attrs.is_reversed());

        let attrs = BlockAttributes::new();
        assert!(!attrs.is_reversed());
    }

    #[test]
    fn test_table_format_named() {
        let attrs = BlockAttributes::parse("format=csv");
        assert_eq!(attrs.table_format(), TableFormat::Csv);

        let attrs = BlockAttributes::parse("format=dsv");
        assert_eq!(attrs.table_format(), TableFormat::Dsv);

        let attrs = BlockAttributes::parse("format=tsv");
        assert_eq!(attrs.table_format(), TableFormat::Tsv);
    }

    #[test]
    fn test_table_format_shorthand() {
        let attrs = BlockAttributes::parse("csv");
        assert_eq!(attrs.table_format(), TableFormat::Csv);

        let attrs = BlockAttributes::parse("dsv");
        assert_eq!(attrs.table_format(), TableFormat::Dsv);

        let attrs = BlockAttributes::parse("tsv");
        assert_eq!(attrs.table_format(), TableFormat::Tsv);
    }

    #[test]
    fn test_table_format_default() {
        let attrs = BlockAttributes::new();
        assert_eq!(attrs.table_format(), TableFormat::Native);

        let attrs = BlockAttributes::parse("source,rust");
        assert_eq!(attrs.table_format(), TableFormat::Native);
    }

    #[test]
    fn test_parse_col_spec_simple_number() {
        let (count, spec) = parse_col_spec("1");
        assert_eq!(count, 1);
        assert_eq!(spec.width, 1);
        assert_eq!(spec.halign, HAlign::Left);
        assert_eq!(spec.valign, VAlign::Top);
    }

    #[test]
    fn test_parse_col_spec_halign() {
        let (_, spec) = parse_col_spec("<");
        assert_eq!(spec.halign, HAlign::Left);

        let (_, spec) = parse_col_spec("^");
        assert_eq!(spec.halign, HAlign::Center);

        let (_, spec) = parse_col_spec(">");
        assert_eq!(spec.halign, HAlign::Right);
    }

    #[test]
    fn test_parse_col_spec_valign() {
        let (_, spec) = parse_col_spec(".<");
        assert_eq!(spec.valign, VAlign::Top);

        let (_, spec) = parse_col_spec(".^");
        assert_eq!(spec.valign, VAlign::Middle);

        let (_, spec) = parse_col_spec(".>");
        assert_eq!(spec.valign, VAlign::Bottom);
    }

    #[test]
    fn test_parse_col_spec_combined_align_and_width() {
        let (count, spec) = parse_col_spec("^.>2");
        assert_eq!(count, 1);
        assert_eq!(spec.halign, HAlign::Center);
        assert_eq!(spec.valign, VAlign::Bottom);
        assert_eq!(spec.width, 2);
    }

    #[test]
    fn test_parse_col_spec_multiplier() {
        let (count, spec) = parse_col_spec("3*^");
        assert_eq!(count, 3);
        assert_eq!(spec.halign, HAlign::Center);
    }

    #[test]
    fn test_parse_col_spec_with_style() {
        let (_, spec) = parse_col_spec("2a");
        assert_eq!(spec.width, 2);
        assert_eq!(spec.style, CellStyle::AsciiDoc);

        let (_, spec) = parse_col_spec(">1e");
        assert_eq!(spec.halign, HAlign::Right);
        assert_eq!(spec.width, 1);
        assert_eq!(spec.style, CellStyle::Emphasis);
    }

    #[test]
    fn test_parse_col_spec_autowidth_marker_keeps_style() {
        // `~` is the autowidth width token; it must be consumed so the trailing
        // style letter still parses (regression: `^~m` lost its monospace style).
        let (_, spec) = parse_col_spec("^~m");
        assert_eq!(spec.halign, HAlign::Center);
        assert_eq!(spec.width, 0);
        assert_eq!(spec.style, CellStyle::Monospace);

        let (_, spec) = parse_col_spec("^~l");
        assert_eq!(spec.halign, HAlign::Center);
        assert_eq!(spec.style, CellStyle::Literal);

        let (_, spec) = parse_col_spec("^~");
        assert_eq!(spec.halign, HAlign::Center);
        assert_eq!(spec.style, CellStyle::Default);

        // A trailing `%` after a numeric width is likewise consumed.
        let (_, spec) = parse_col_spec("50%s");
        assert_eq!(spec.width, 50);
        assert_eq!(spec.style, CellStyle::Strong);
    }

    #[test]
    fn test_table_col_specs_numeric() {
        let attrs = BlockAttributes::parse("cols=\"3\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        for s in &specs {
            assert_eq!(s.halign, HAlign::Left);
            assert_eq!(s.valign, VAlign::Top);
        }
    }

    #[test]
    fn test_table_col_specs_align_list() {
        let attrs = BlockAttributes::parse("cols=\"<,^,>\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0].halign, HAlign::Left);
        assert_eq!(specs[1].halign, HAlign::Center);
        assert_eq!(specs[2].halign, HAlign::Right);
    }

    #[test]
    fn test_table_col_specs_multiplier() {
        let attrs = BlockAttributes::parse("cols=\"3*^\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        for s in &specs {
            assert_eq!(s.halign, HAlign::Center);
        }
    }

    #[test]
    fn test_table_col_specs_mixed() {
        let attrs = BlockAttributes::parse("cols=\"^.>2,<1\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].halign, HAlign::Center);
        assert_eq!(specs[0].valign, VAlign::Bottom);
        assert_eq!(specs[0].width, 2);
        assert_eq!(specs[1].halign, HAlign::Left);
        assert_eq!(specs[1].width, 1);
    }

    #[test]
    fn test_table_col_specs_semicolon_separator() {
        // Semicolon is the column separator when no comma is present, so
        // `[cols=1;m;m]` survives the attrlist splitter unquoted → 3 columns.
        let attrs = BlockAttributes::parse("cols=1;m;m");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0].width, 1);
        assert_eq!(specs[0].style, CellStyle::Default);
        assert_eq!(specs[1].style, CellStyle::Monospace);
        assert_eq!(specs[2].style, CellStyle::Monospace);
        assert_eq!(attrs.table_cols_count(), Some(3));
        // Multiplier survives semicolon split too: `2*;m` → 3 columns.
        let attrs = BlockAttributes::parse("cols=2*;m");
        assert_eq!(attrs.table_cols_count(), Some(3));
        // A comma anywhere forces comma-splitting (semicolons stay literal,
        // yielding lenient default specs — mirrors asciidoctor's lone sep).
        let attrs = BlockAttributes::parse("cols=\"1,m;m\"");
        assert_eq!(attrs.table_cols_count(), Some(2));
    }

    #[test]
    fn test_table_col_specs_with_style() {
        let attrs = BlockAttributes::parse("cols=\"1,2a,3\"");
        let specs = attrs.table_col_specs().unwrap();
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0].style, CellStyle::Default);
        assert_eq!(specs[1].style, CellStyle::AsciiDoc);
        assert_eq!(specs[1].width, 2);
        assert_eq!(specs[2].style, CellStyle::Default);
        assert_eq!(specs[2].width, 3);
    }

    #[test]
    fn test_parse_image_attrs_all() {
        let attrs = parse_image_attrs("Alt text,600,400");
        assert_eq!(attrs.alt, "Alt text");
        assert_eq!(attrs.width, Some("600"));
        assert_eq!(attrs.height, Some("400"));
    }

    #[test]
    fn test_parse_image_attrs_alt_only() {
        let attrs = parse_image_attrs("A beautiful sunset");
        assert_eq!(attrs.alt, "A beautiful sunset");
        assert_eq!(attrs.width, None);
        assert_eq!(attrs.height, None);
    }

    #[test]
    fn test_parse_image_attrs_quoted_alt() {
        // Positional alt in double quotes: enclosing quotes are stripped
        // (Asciidoctor: `image::x["Alt text",role=r]` → alt="Alt text").
        let attrs = parse_image_attrs("\"Byline with custom version label\",role=screenshot");
        assert_eq!(attrs.alt, "Byline with custom version label");
        assert_eq!(attrs.role, Some("screenshot"));
    }

    #[test]
    fn test_parse_image_attrs_named() {
        let attrs = parse_image_attrs("alt=Photo,width=800");
        assert_eq!(attrs.alt, "Photo");
        assert_eq!(attrs.width, Some("800"));
        assert_eq!(attrs.height, None);
    }

    #[test]
    fn test_parse_image_attrs_named_all() {
        let attrs = parse_image_attrs("alt=Photo,width=800,height=600");
        assert_eq!(attrs.alt, "Photo");
        assert_eq!(attrs.width, Some("800"));
        assert_eq!(attrs.height, Some("600"));
    }

    #[test]
    fn test_parse_image_attrs_empty() {
        let attrs = parse_image_attrs("");
        assert_eq!(attrs.alt, "");
        assert_eq!(attrs.width, None);
        assert_eq!(attrs.height, None);
    }

    #[test]
    fn test_parse_image_attrs_width_only() {
        let attrs = parse_image_attrs("Alt,300");
        assert_eq!(attrs.alt, "Alt");
        assert_eq!(attrs.width, Some("300"));
        assert_eq!(attrs.height, None);
    }

    #[test]
    fn test_parse_link_attrs_text_only() {
        let attrs = parse_link_attrs("Example Site", LinkKind::Link);
        assert_eq!(attrs.text, "Example Site");
        assert_eq!(attrs.window, None);
        assert!(!attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_with_window() {
        let attrs = parse_link_attrs("Example,window=_blank", LinkKind::Link);
        assert_eq!(attrs.text, "Example");
        assert_eq!(attrs.window, Some("_blank"));
        assert!(!attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_with_nofollow() {
        let attrs = parse_link_attrs("Example,opts=nofollow", LinkKind::Link);
        assert_eq!(attrs.text, "Example");
        assert_eq!(attrs.window, None);
        assert!(attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_with_all() {
        let attrs = parse_link_attrs("Example,window=_blank,opts=nofollow", LinkKind::Link);
        assert_eq!(attrs.text, "Example");
        assert_eq!(attrs.window, Some("_blank"));
        assert!(attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_empty() {
        let attrs = parse_link_attrs("", LinkKind::Link);
        assert_eq!(attrs.text, "");
        assert_eq!(attrs.window, None);
        assert!(!attrs.nofollow);
    }

    #[test]
    fn test_parse_link_attrs_comma_in_text_kept() {
        // F-A: a `link:`/URL bracket with no named attribute keeps the WHOLE
        // content as the link text, commas included (not split at the 1st comma).
        let attrs = parse_link_attrs("A, B, C", LinkKind::Link);
        assert_eq!(attrs.text, "A, B, C");
        assert_eq!(attrs.window, None);
        assert!(!attrs.nofollow);
        assert_eq!(attrs.role, None);

        let attrs = parse_link_attrs("NFJS, the Magazine", LinkKind::Link);
        assert_eq!(attrs.text, "NFJS, the Magazine");
    }

    #[test]
    fn test_parse_link_attrs_comma_text_with_named_attr_splits() {
        // A named attribute (`role=`/`window=`) switches the bracket into
        // attribute-list mode, so the comma separates the positional text.
        let attrs = parse_link_attrs("Google, window=_blank", LinkKind::Link);
        assert_eq!(attrs.text, "Google");
        assert_eq!(attrs.window, Some("_blank"));
    }

    #[test]
    fn test_parse_link_attrs_quoted_comma_text_with_role() {
        // Quoted text preserves a comma even alongside a named attribute; the
        // surrounding quotes are stripped from the visible text.
        let attrs = parse_link_attrs("\"A, B\",role=green", LinkKind::Link);
        assert_eq!(attrs.text, "A, B");
        assert_eq!(attrs.role, Some("green"));
    }

    #[test]
    fn test_parse_link_attrs_mailto_positional_subject_body() {
        // mailto always splits positionally so subject/body survive without `=`.
        let attrs = parse_link_attrs("Email, Subject, Body", LinkKind::Mailto);
        assert_eq!(attrs.text, "Email");
        assert_eq!(attrs.subject, Some("Subject"));
        assert_eq!(attrs.body, Some("Body"));
    }

    #[test]
    fn test_subs_parse_normal() {
        let result = parse_subs_value("normal", SubstitutionSet::VERBATIM);
        assert_eq!(result, SubstitutionSet::NORMAL);
    }

    #[test]
    fn test_subs_parse_none() {
        let result = parse_subs_value("none", SubstitutionSet::NORMAL);
        assert_eq!(result, SubstitutionSet::NONE);
    }

    #[test]
    fn test_subs_parse_verbatim() {
        let result = parse_subs_value("verbatim", SubstitutionSet::NORMAL);
        assert_eq!(result, SubstitutionSet::VERBATIM);
    }

    #[test]
    fn test_subs_parse_explicit_list() {
        let result = parse_subs_value("specialchars,attributes", SubstitutionSet::NORMAL);
        assert!(result.has(SubstitutionSet::SPECIALCHARS));
        assert!(result.has(SubstitutionSet::ATTRIBUTES));
        assert!(!result.has(SubstitutionSet::QUOTES));
        assert!(!result.has(SubstitutionSet::MACROS));
    }

    #[test]
    fn test_subs_parse_incremental_add() {
        let result = parse_subs_value("+macros", SubstitutionSet::VERBATIM);
        assert!(result.has(SubstitutionSet::SPECIALCHARS));
        assert!(result.has(SubstitutionSet::CALLOUTS));
        assert!(result.has(SubstitutionSet::MACROS));
    }

    #[test]
    fn test_subs_parse_incremental_remove() {
        let result = parse_subs_value("-callouts", SubstitutionSet::VERBATIM);
        assert!(result.has(SubstitutionSet::SPECIALCHARS));
        assert!(!result.has(SubstitutionSet::CALLOUTS));
    }

    #[test]
    fn test_subs_parse_combined() {
        let result = parse_subs_value("+macros,-callouts", SubstitutionSet::VERBATIM);
        assert!(result.has(SubstitutionSet::SPECIALCHARS));
        assert!(result.has(SubstitutionSet::MACROS));
        assert!(!result.has(SubstitutionSet::CALLOUTS));
    }

    #[test]
    fn test_subs_parse_mixed_explicit_incremental() {
        // "macros,+attributes" — a plain token FIRST seeds the set empty
        // (replacement), the modifier then adds to it; the defaults are lost
        // (asciidoctor resolve_subs, verified: [source,subs="quotes,+attributes"]
        // drops specialchars).
        let result = parse_subs_value("macros,+attributes", SubstitutionSet::VERBATIM);
        assert!(result.has(SubstitutionSet::MACROS));
        assert!(result.has(SubstitutionSet::ATTRIBUTES));
        assert!(!result.has(SubstitutionSet::SPECIALCHARS));
        assert!(!result.has(SubstitutionSet::CALLOUTS));
    }

    #[test]
    fn test_subs_parse_trailing_plus() {
        // "attributes+" — trailing plus is asciidoctor's PREPEND modifier:
        // the defaults are kept and the sub is added.
        let result = parse_subs_value("attributes+", SubstitutionSet::VERBATIM);
        assert!(result.has(SubstitutionSet::SPECIALCHARS));
        assert!(result.has(SubstitutionSet::CALLOUTS));
        assert!(result.has(SubstitutionSet::ATTRIBUTES));
        assert!(!result.has(SubstitutionSet::MACROS));

        // Mixed with remove: "attributes+,-specialchars"
        let result = parse_subs_value("attributes+,-specialchars", SubstitutionSet::VERBATIM);
        assert!(result.has(SubstitutionSet::ATTRIBUTES));
        assert!(result.has(SubstitutionSet::CALLOUTS));
        assert!(!result.has(SubstitutionSet::SPECIALCHARS));

        // Composite group name as incremental token: "verbatim+" is a no-op
        // on verbatim defaults.
        let result = parse_subs_value("verbatim+", SubstitutionSet::VERBATIM);
        assert_eq!(result, SubstitutionSet::VERBATIM);
    }

    #[test]
    fn test_subs_parse_mixed_remove_in_middle() {
        // "quotes,-specialchars" — the plain token seeds the set empty with
        // just quotes; the remove is then a no-op (asciidoctor resolve_subs).
        let result = parse_subs_value("quotes,-specialchars", SubstitutionSet::NORMAL);
        assert!(!result.has(SubstitutionSet::SPECIALCHARS));
        assert!(result.has(SubstitutionSet::QUOTES));
        assert!(!result.has(SubstitutionSet::MACROS));

        // Modifier FIRST seeds from the defaults; a later plain token adds.
        let result = parse_subs_value("-specialchars,quotes", SubstitutionSet::NORMAL);
        assert!(!result.has(SubstitutionSet::SPECIALCHARS));
        assert!(result.has(SubstitutionSet::QUOTES));
        assert!(result.has(SubstitutionSet::MACROS));
    }

    #[test]
    fn test_subs_block_attributes() {
        let attrs = BlockAttributes::parse("subs=normal");
        assert_eq!(attrs.substitution_set(SubstitutionSet::VERBATIM), Some(SubstitutionSet::NORMAL));

        let attrs = BlockAttributes::parse("source,rust");
        assert_eq!(attrs.substitution_set(SubstitutionSet::VERBATIM), None);
    }

    #[test]
    fn test_parse_image_attrs_with_align() {
        let attrs = parse_image_attrs("Alt,align=center");
        assert_eq!(attrs.alt, "Alt");
        assert_eq!(attrs.align, Some("center"));
        assert_eq!(attrs.float, None);
    }

    #[test]
    fn test_parse_image_attrs_with_float() {
        let attrs = parse_image_attrs("Alt,float=left");
        assert_eq!(attrs.alt, "Alt");
        assert_eq!(attrs.float, Some("left"));
        assert_eq!(attrs.align, None);
    }

    #[test]
    fn test_parse_image_attrs_with_align_and_float() {
        let attrs = parse_image_attrs("Alt,align=center,float=right");
        assert_eq!(attrs.alt, "Alt");
        assert_eq!(attrs.align, Some("center"));
        assert_eq!(attrs.float, Some("right"));
    }

    #[test]
    fn test_parse_image_attrs_caption_title_and_named_only_alt() {
        let attrs = parse_image_attrs("caption=\"My Caption. \",title=AttrTitle");
        assert_eq!(attrs.caption, Some("My Caption. "));
        assert_eq!(attrs.title, Some("AttrTitle"));
        // Named-only attrs: alt stays empty (the renderer auto-generates it
        // from the filename), not the raw bracket content.
        assert_eq!(attrs.alt, "");

        let attrs = parse_image_attrs("width=100");
        assert_eq!(attrs.alt, "");
        assert_eq!(attrs.width, Some("100"));

        // A positional alt still wins.
        let attrs = parse_image_attrs("Alt text,caption=C");
        assert_eq!(attrs.alt, "Alt text");
        assert_eq!(attrs.caption, Some("C"));
    }

    #[test]
    fn test_shorthand_only_in_first_position() {
        // Mixed shorthand parses in the style slot only.
        let attrs = BlockAttributes::parse("quote#roads,Dr. Emmett Brown,Back to the Future");
        assert_eq!(attrs.id.as_deref(), Some("roads"));
        assert!(attrs.roles.is_empty());
        assert_eq!(attrs.positional, vec!["quote", "Dr. Emmett Brown", "Back to the Future"]);

        // Pure shorthand in a later part is verbatim positional text.
        let attrs = BlockAttributes::parse("quote,#bar");
        assert_eq!(attrs.id, None);
        assert_eq!(attrs.positional, vec!["quote", "#bar"]);

        let attrs = BlockAttributes::parse("quote,.baz");
        assert!(attrs.roles.is_empty());
        assert_eq!(attrs.positional, vec!["quote", ".baz"]);
    }

    #[test]
    fn test_source_third_slot_is_linenums() {
        // Any non-empty positional value in slot 3 of a source block
        // enables line numbering.
        for attr_str in ["source,ruby,linenums", "source,ruby,%linenums", ",ruby,linenums"] {
            let attrs = BlockAttributes::parse(attr_str);
            assert!(attrs.options.iter().any(|o| o == "linenums"), "expected linenums for [{attr_str}]");
        }
        // A named attribute does not fill the slot; non-source styles are unaffected.
        for attr_str in ["source,ruby,start=10", "source,ruby", "quote,a,b"] {
            let attrs = BlockAttributes::parse(attr_str);
            assert!(!attrs.options.iter().any(|o| o == "linenums"), "no linenums for [{attr_str}]");
        }
    }
}
