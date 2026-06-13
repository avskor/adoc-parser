use std::borrow::Cow;

pub type CowStr<'a> = Cow<'a, str>;

/// Bitflag set controlling which substitutions apply to a block's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubstitutionSet(u8);

impl SubstitutionSet {
    pub const SPECIALCHARS: u8 = 0b0000_0001;
    pub const QUOTES: u8 = 0b0000_0010;
    pub const ATTRIBUTES: u8 = 0b0000_0100;
    pub const REPLACEMENTS: u8 = 0b0000_1000;
    pub const MACROS: u8 = 0b0001_0000;
    pub const POST_REPLACEMENTS: u8 = 0b0010_0000;
    pub const CALLOUTS: u8 = 0b0100_0000;

    pub const NORMAL: Self = Self(
        Self::SPECIALCHARS
            | Self::QUOTES
            | Self::ATTRIBUTES
            | Self::REPLACEMENTS
            | Self::MACROS
            | Self::POST_REPLACEMENTS,
    );
    pub const VERBATIM: Self = Self(Self::SPECIALCHARS | Self::CALLOUTS);
    pub const NONE: Self = Self(0);

    pub fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn add(&mut self, flag: u8) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u8) {
        self.0 &= !flag;
    }

    pub fn without(self, flag: u8) -> Self {
        Self(self.0 & !flag)
    }

    /// Whether inline parsing is needed (quotes, macros, attributes, replacements, post_replacements).
    pub fn needs_inline_parsing(self) -> bool {
        self.0
            & (Self::QUOTES
                | Self::MACROS
                | Self::ATTRIBUTES
                | Self::REPLACEMENTS
                | Self::POST_REPLACEMENTS)
            != 0
    }
}

fn cow_owned(s: CowStr<'_>) -> CowStr<'static> {
    Cow::Owned(s.into_owned())
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event<'a> {
    Start(Tag<'a>),
    End(TagEnd),
    Text(CowStr<'a>),
    Code(CowStr<'a>),
    InlinePassthrough(CowStr<'a>),
    SoftBreak,
    HardBreak,
    ThematicBreak,
    PageBreak,
    Attribute {
        name: CowStr<'a>,
        value: CowStr<'a>,
    },
    AttributeReference {
        name: CowStr<'a>,
        fallback: Option<CowStr<'a>>,
        /// Raw `[...]` text (brackets included) immediately following the
        /// reference, e.g. the `[text]` in `{url}[text]`. Asciidoctor performs
        /// attribute substitution before macro substitution, so once `{url}`
        /// resolves to a URL the trailing brackets form a link macro. The
        /// renderer re-parses `value` + these brackets together to reproduce
        /// that ordering; for non-URL values the result is the same literal.
        trailing_brackets: Option<CowStr<'a>>,
    },
    Footnote {
        id: Option<CowStr<'a>>,
        text: CowStr<'a>,
    },
    FootnoteRef {
        id: CowStr<'a>,
    },
    CalloutRef(u32),
    XmlCalloutRef(u32),
    IndexTerm {
        text: CowStr<'a>,
    },
    ConcealedIndexTerm {
        primary: CowStr<'a>,
        secondary: Option<CowStr<'a>>,
        tertiary: Option<CowStr<'a>>,
    },
    BibliographyAnchor {
        id: CowStr<'a>,
        label: Option<CowStr<'a>>,
    },
    Toc,
    Include {
        path: CowStr<'a>,
        attrs: CowStr<'a>,
    },
    Author {
        fullname: CowStr<'a>,
        firstname: CowStr<'a>,
        middlename: CowStr<'a>,
        lastname: CowStr<'a>,
        initials: CowStr<'a>,
        address: CowStr<'a>,
    },
    Revision {
        version: CowStr<'a>,
        date: CowStr<'a>,
        remark: CowStr<'a>,
    },
    BlockMetadata {
        style: Option<CowStr<'a>>,
        id: Option<CowStr<'a>>,
        roles: Vec<CowStr<'a>>,
        options: Vec<CowStr<'a>>,
        named: Vec<(CowStr<'a>, CowStr<'a>)>,
        subs: Option<SubstitutionSet>,
    },
}

impl<'a> Event<'a> {
    pub fn into_static(self) -> Event<'static> {
        match self {
            Event::Start(tag) => Event::Start(tag.into_static()),
            Event::End(tag_end) => Event::End(tag_end),
            Event::Text(s) => Event::Text(cow_owned(s)),
            Event::Code(s) => Event::Code(cow_owned(s)),
            Event::InlinePassthrough(s) => Event::InlinePassthrough(cow_owned(s)),
            Event::SoftBreak => Event::SoftBreak,
            Event::HardBreak => Event::HardBreak,
            Event::ThematicBreak => Event::ThematicBreak,
            Event::PageBreak => Event::PageBreak,
            Event::Attribute { name, value } => Event::Attribute {
                name: cow_owned(name),
                value: cow_owned(value),
            },
            Event::AttributeReference { name, fallback, trailing_brackets } => Event::AttributeReference {
                name: cow_owned(name),
                fallback: fallback.map(cow_owned),
                trailing_brackets: trailing_brackets.map(cow_owned),
            },
            Event::Footnote { id, text } => Event::Footnote {
                id: id.map(cow_owned),
                text: cow_owned(text),
            },
            Event::FootnoteRef { id } => Event::FootnoteRef {
                id: cow_owned(id),
            },
            Event::CalloutRef(n) => Event::CalloutRef(n),
            Event::XmlCalloutRef(n) => Event::XmlCalloutRef(n),
            Event::IndexTerm { text } => Event::IndexTerm {
                text: cow_owned(text),
            },
            Event::ConcealedIndexTerm {
                primary,
                secondary,
                tertiary,
            } => Event::ConcealedIndexTerm {
                primary: cow_owned(primary),
                secondary: secondary.map(cow_owned),
                tertiary: tertiary.map(cow_owned),
            },
            Event::BibliographyAnchor { id, label } => Event::BibliographyAnchor {
                id: cow_owned(id),
                label: label.map(cow_owned),
            },
            Event::Toc => Event::Toc,
            Event::Include { path, attrs } => Event::Include {
                path: cow_owned(path),
                attrs: cow_owned(attrs),
            },
            Event::Author {
                fullname,
                firstname,
                middlename,
                lastname,
                initials,
                address,
            } => Event::Author {
                fullname: cow_owned(fullname),
                firstname: cow_owned(firstname),
                middlename: cow_owned(middlename),
                lastname: cow_owned(lastname),
                initials: cow_owned(initials),
                address: cow_owned(address),
            },
            Event::Revision {
                version,
                date,
                remark,
            } => Event::Revision {
                version: cow_owned(version),
                date: cow_owned(date),
                remark: cow_owned(remark),
            },
            Event::BlockMetadata { style, id, roles, options, named, subs } => Event::BlockMetadata {
                style: style.map(cow_owned),
                id: id.map(cow_owned),
                roles: roles.into_iter().map(cow_owned).collect(),
                options: options.into_iter().map(cow_owned).collect(),
                named: named.into_iter().map(|(k, v)| (cow_owned(k), cow_owned(v))).collect(),
                subs,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdmonitionKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellStyle {
    #[default]
    Default,
    AsciiDoc,
    Header,
    Emphasis,
    Monospace,
    Strong,
    Literal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DelimitedBlockKind {
    Listing,
    Literal,
    Example,
    Sidebar,
    Quote,
    Open,
    Comment,
    Passthrough,
    Verse,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tag<'a> {
    // Document structure
    Header,
    DocumentTitle,
    Section { level: u8 },
    SectionTitle { level: u8, id: CowStr<'a> },

    // Standalone heading (discrete or inside delimited block)
    Heading { level: u8 },

    // Block elements
    Paragraph,
    LiteralParagraph,
    DelimitedBlock { kind: DelimitedBlockKind },
    SourceBlock { language: Option<CowStr<'a>> },
    BlockTitle,

    // Lists
    UnorderedList { has_checklist: bool },
    OrderedList { start: Option<u32>, reversed: bool, depth: u8 },
    ListItem { depth: u8, checked: Option<bool> },
    DescriptionList,
    DescriptionTerm,
    DescriptionDescription,
    CalloutList,
    CalloutListItem { number: u32 },

    // Admonitions
    /// `block: false` — admonition paragraph (`NOTE: text` / `[NOTE]` on a paragraph),
    /// content is the admonition's own text (rendered bare).
    /// `block: true` — admonition block (`[NOTE]` on `====`/`--`), content is
    /// nested blocks (rendered with normal wrappers).
    Admonition { kind: AdmonitionKind, block: bool },

    // Tables
    Table,
    TableHead,
    TableBody,
    TableFoot,
    TableRow,
    TableCell { colspan: u8, rowspan: u8, style: CellStyle, halign: HAlign, valign: VAlign },
    TableHeaderCell { colspan: u8, rowspan: u8, style: CellStyle, halign: HAlign, valign: VAlign },

    // Media
    BlockImage { target: CowStr<'a>, alt: CowStr<'a>, width: Option<CowStr<'a>>, height: Option<CowStr<'a>>, link: Option<CowStr<'a>> },
    BlockVideo { target: CowStr<'a>, attrs: CowStr<'a> },
    BlockAudio { target: CowStr<'a>, attrs: CowStr<'a> },
    InlineImage { target: CowStr<'a>, alt: CowStr<'a>, width: Option<CowStr<'a>>, height: Option<CowStr<'a>>, align: Option<CowStr<'a>>, float: Option<CowStr<'a>>, link: Option<CowStr<'a>>, role: Option<CowStr<'a>>, title: Option<CowStr<'a>> },

    // Inline formatting
    Strong { id: Option<CowStr<'a>>, roles: Vec<CowStr<'a>> },
    Emphasis { id: Option<CowStr<'a>>, roles: Vec<CowStr<'a>> },
    Monospace { id: Option<CowStr<'a>>, roles: Vec<CowStr<'a>> },
    Highlight,
    InlineSpan { id: Option<CowStr<'a>>, roles: Vec<CowStr<'a>> },
    Superscript,
    Subscript,

    // Links and references
    Link { url: CowStr<'a>, window: Option<CowStr<'a>>, nofollow: bool, is_bare: bool, role: Option<CowStr<'a>> },
    CrossReference { target: CowStr<'a>, label: Option<CowStr<'a>> },

    // UI macros
    Keyboard,
    Button,
    Menu { target: CowStr<'a> },
    Icon { name: CowStr<'a> },

    // STEM (math)
    Stem { variant: CowStr<'a> },
    StemBlock { variant: CowStr<'a> },

    // Anchors
    /// `label` is the xreflabel from `[[id,label]]` / `anchor:id[label]` —
    /// reference text for xrefs targeting this anchor, never rendered in place.
    Anchor { id: CowStr<'a>, label: Option<CowStr<'a>> },

    // Custom macros
    CustomInlineMacro { name: CowStr<'a>, target: CowStr<'a> },
    CustomBlockMacro { name: CowStr<'a>, target: CowStr<'a> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagEnd {
    Header,
    DocumentTitle,
    Section { level: u8 },
    SectionTitle,

    Heading { level: u8 },

    Paragraph,
    LiteralParagraph,
    DelimitedBlock,
    SourceBlock,
    BlockTitle,

    UnorderedList,
    OrderedList,
    ListItem,
    DescriptionList,
    DescriptionTerm,
    DescriptionDescription,
    CalloutList,
    CalloutListItem,

    Admonition,

    Table,
    TableHead,
    TableBody,
    TableFoot,
    TableRow,
    TableCell,
    TableHeaderCell,

    BlockImage,
    BlockVideo,
    BlockAudio,
    InlineImage,

    Strong,
    Emphasis,
    Monospace,
    Highlight,
    InlineSpan,
    Superscript,
    Subscript,

    Link,
    CrossReference,

    Keyboard,
    Button,
    Menu,
    Icon,

    Stem,
    StemBlock,

    Anchor,

    CustomInlineMacro,
    CustomBlockMacro,
}

impl<'a> Tag<'a> {
    pub fn into_static(self) -> Tag<'static> {
        match self {
            Tag::Header => Tag::Header,
            Tag::DocumentTitle => Tag::DocumentTitle,
            Tag::Section { level } => Tag::Section { level },
            Tag::SectionTitle { level, id } => Tag::SectionTitle {
                level,
                id: Cow::Owned(id.into_owned()),
            },
            Tag::Heading { level } => Tag::Heading { level },
            Tag::Paragraph => Tag::Paragraph,
            Tag::LiteralParagraph => Tag::LiteralParagraph,
            Tag::DelimitedBlock { kind } => Tag::DelimitedBlock { kind },
            Tag::SourceBlock { language } => Tag::SourceBlock {
                language: language.map(|l| Cow::Owned(l.into_owned())),
            },
            Tag::BlockTitle => Tag::BlockTitle,
            Tag::UnorderedList { has_checklist } => Tag::UnorderedList { has_checklist },
            Tag::OrderedList { start, reversed, depth } => Tag::OrderedList { start, reversed, depth },
            Tag::ListItem { depth, checked } => Tag::ListItem { depth, checked },
            Tag::DescriptionList => Tag::DescriptionList,
            Tag::DescriptionTerm => Tag::DescriptionTerm,
            Tag::DescriptionDescription => Tag::DescriptionDescription,
            Tag::CalloutList => Tag::CalloutList,
            Tag::CalloutListItem { number } => Tag::CalloutListItem { number },
            Tag::Admonition { kind, block } => Tag::Admonition { kind, block },
            Tag::Table => Tag::Table,
            Tag::TableHead => Tag::TableHead,
            Tag::TableBody => Tag::TableBody,
            Tag::TableFoot => Tag::TableFoot,
            Tag::TableRow => Tag::TableRow,
            Tag::TableCell { colspan, rowspan, style, halign, valign } => Tag::TableCell { colspan, rowspan, style, halign, valign },
            Tag::TableHeaderCell { colspan, rowspan, style, halign, valign } => Tag::TableHeaderCell { colspan, rowspan, style, halign, valign },
            Tag::BlockImage { target, alt, width, height, link } => Tag::BlockImage {
                target: Cow::Owned(target.into_owned()),
                alt: Cow::Owned(alt.into_owned()),
                width: width.map(|w| Cow::Owned(w.into_owned())),
                height: height.map(|h| Cow::Owned(h.into_owned())),
                link: link.map(cow_owned),
            },
            Tag::BlockVideo { target, attrs } => Tag::BlockVideo {
                target: Cow::Owned(target.into_owned()),
                attrs: Cow::Owned(attrs.into_owned()),
            },
            Tag::BlockAudio { target, attrs } => Tag::BlockAudio {
                target: Cow::Owned(target.into_owned()),
                attrs: Cow::Owned(attrs.into_owned()),
            },
            Tag::InlineImage { target, alt, width, height, align, float, link, role, title } => Tag::InlineImage {
                target: Cow::Owned(target.into_owned()),
                alt: Cow::Owned(alt.into_owned()),
                width: width.map(|w| Cow::Owned(w.into_owned())),
                height: height.map(|h| Cow::Owned(h.into_owned())),
                align: align.map(cow_owned),
                float: float.map(cow_owned),
                link: link.map(cow_owned),
                role: role.map(cow_owned),
                title: title.map(cow_owned),
            },
            Tag::Strong { id, roles } => Tag::Strong {
                id: id.map(cow_owned),
                roles: roles.into_iter().map(cow_owned).collect(),
            },
            Tag::Emphasis { id, roles } => Tag::Emphasis {
                id: id.map(cow_owned),
                roles: roles.into_iter().map(cow_owned).collect(),
            },
            Tag::Monospace { id, roles } => Tag::Monospace {
                id: id.map(cow_owned),
                roles: roles.into_iter().map(cow_owned).collect(),
            },
            Tag::Highlight => Tag::Highlight,
            Tag::InlineSpan { id, roles } => Tag::InlineSpan {
                id: id.map(cow_owned),
                roles: roles.into_iter().map(cow_owned).collect(),
            },
            Tag::Superscript => Tag::Superscript,
            Tag::Subscript => Tag::Subscript,
            Tag::Link { url, window, nofollow, is_bare, role } => Tag::Link {
                url: Cow::Owned(url.into_owned()),
                window: window.map(|w| Cow::Owned(w.into_owned())),
                nofollow,
                is_bare,
                role: role.map(|r| Cow::Owned(r.into_owned())),
            },
            Tag::CrossReference { target, label } => Tag::CrossReference {
                target: Cow::Owned(target.into_owned()),
                label: label.map(|l| Cow::Owned(l.into_owned())),
            },
            Tag::Keyboard => Tag::Keyboard,
            Tag::Button => Tag::Button,
            Tag::Menu { target } => Tag::Menu {
                target: Cow::Owned(target.into_owned()),
            },
            Tag::Icon { name } => Tag::Icon {
                name: Cow::Owned(name.into_owned()),
            },
            Tag::Stem { variant } => Tag::Stem {
                variant: Cow::Owned(variant.into_owned()),
            },
            Tag::StemBlock { variant } => Tag::StemBlock {
                variant: Cow::Owned(variant.into_owned()),
            },
            Tag::Anchor { id, label } => Tag::Anchor {
                id: Cow::Owned(id.into_owned()),
                label: label.map(|l| Cow::Owned(l.into_owned())),
            },
            Tag::CustomInlineMacro { name, target } => Tag::CustomInlineMacro {
                name: cow_owned(name),
                target: cow_owned(target),
            },
            Tag::CustomBlockMacro { name, target } => Tag::CustomBlockMacro {
                name: cow_owned(name),
                target: cow_owned(target),
            },
        }
    }

    pub fn to_end(&self) -> TagEnd {
        match self {
            Tag::Header => TagEnd::Header,
            Tag::DocumentTitle => TagEnd::DocumentTitle,
            Tag::Section { level } => TagEnd::Section { level: *level },
            Tag::SectionTitle { .. } => TagEnd::SectionTitle,
            Tag::Heading { level } => TagEnd::Heading { level: *level },
            Tag::Paragraph => TagEnd::Paragraph,
            Tag::LiteralParagraph => TagEnd::LiteralParagraph,
            Tag::DelimitedBlock { .. } => TagEnd::DelimitedBlock,
            Tag::SourceBlock { .. } => TagEnd::SourceBlock,
            Tag::BlockTitle => TagEnd::BlockTitle,
            Tag::UnorderedList { .. } => TagEnd::UnorderedList,
            Tag::OrderedList { .. } => TagEnd::OrderedList,
            Tag::ListItem { .. } => TagEnd::ListItem,
            Tag::DescriptionList => TagEnd::DescriptionList,
            Tag::DescriptionTerm => TagEnd::DescriptionTerm,
            Tag::DescriptionDescription => TagEnd::DescriptionDescription,
            Tag::CalloutList => TagEnd::CalloutList,
            Tag::CalloutListItem { .. } => TagEnd::CalloutListItem,
            Tag::Admonition { .. } => TagEnd::Admonition,
            Tag::Table => TagEnd::Table,
            Tag::TableHead => TagEnd::TableHead,
            Tag::TableBody => TagEnd::TableBody,
            Tag::TableFoot => TagEnd::TableFoot,
            Tag::TableRow => TagEnd::TableRow,
            Tag::TableCell { .. } => TagEnd::TableCell,
            Tag::TableHeaderCell { .. } => TagEnd::TableHeaderCell,
            Tag::BlockImage { .. } => TagEnd::BlockImage,
            Tag::BlockVideo { .. } => TagEnd::BlockVideo,
            Tag::BlockAudio { .. } => TagEnd::BlockAudio,
            Tag::InlineImage { .. } => TagEnd::InlineImage,
            Tag::Strong { .. } => TagEnd::Strong,
            Tag::Emphasis { .. } => TagEnd::Emphasis,
            Tag::Monospace { .. } => TagEnd::Monospace,
            Tag::Highlight => TagEnd::Highlight,
            Tag::InlineSpan { .. } => TagEnd::InlineSpan,
            Tag::Superscript => TagEnd::Superscript,
            Tag::Subscript => TagEnd::Subscript,
            Tag::Link { .. } => TagEnd::Link,
            Tag::CrossReference { .. } => TagEnd::CrossReference,
            Tag::Keyboard => TagEnd::Keyboard,
            Tag::Button => TagEnd::Button,
            Tag::Menu { .. } => TagEnd::Menu,
            Tag::Icon { .. } => TagEnd::Icon,
            Tag::Stem { .. } => TagEnd::Stem,
            Tag::StemBlock { .. } => TagEnd::StemBlock,
            Tag::Anchor { .. } => TagEnd::Anchor,
            Tag::CustomInlineMacro { .. } => TagEnd::CustomInlineMacro,
            Tag::CustomBlockMacro { .. } => TagEnd::CustomBlockMacro,
        }
    }
}
