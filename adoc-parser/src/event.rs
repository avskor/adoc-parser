use std::borrow::Cow;

pub type CowStr<'a> = Cow<'a, str>;

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
    AttributeReference(CowStr<'a>),
    Footnote {
        id: Option<CowStr<'a>>,
        text: CowStr<'a>,
    },
    FootnoteRef {
        id: CowStr<'a>,
    },
    CalloutRef(u32),
    Toc,
    Include {
        path: CowStr<'a>,
        attrs: CowStr<'a>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdmonitionKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DelimitedBlockKind {
    Listing,
    Literal,
    Example,
    Sidebar,
    Quote,
    Open,
    Comment,
    Passthrough,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tag<'a> {
    // Document structure
    Header,
    DocumentTitle,
    Section { level: u8 },
    SectionTitle { level: u8, id: CowStr<'a> },

    // Block elements
    Paragraph,
    LiteralParagraph,
    DelimitedBlock { kind: DelimitedBlockKind },
    SourceBlock { language: Option<CowStr<'a>> },
    BlockTitle,

    // Lists
    UnorderedList { has_checklist: bool },
    OrderedList,
    ListItem { depth: u8, checked: Option<bool> },
    DescriptionList,
    DescriptionTerm,
    DescriptionDescription,
    CalloutList,
    CalloutListItem { number: u32 },

    // Admonitions
    Admonition { kind: AdmonitionKind },

    // Tables
    Table,
    TableHead,
    TableBody,
    TableFoot,
    TableRow,
    TableCell,
    TableHeaderCell,

    // Media
    BlockImage { target: CowStr<'a>, alt: CowStr<'a> },
    InlineImage { target: CowStr<'a>, alt: CowStr<'a> },

    // Inline formatting
    Strong,
    Emphasis,
    Monospace,
    Highlight,
    Superscript,
    Subscript,

    // Links and references
    Link { url: CowStr<'a> },
    CrossReference { target: CowStr<'a>, label: Option<CowStr<'a>> },

    // Anchors
    Anchor { id: CowStr<'a> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagEnd {
    Header,
    DocumentTitle,
    Section { level: u8 },
    SectionTitle,

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
    InlineImage,

    Strong,
    Emphasis,
    Monospace,
    Highlight,
    Superscript,
    Subscript,

    Link,
    CrossReference,

    Anchor,
}

impl<'a> Tag<'a> {
    pub fn to_end(&self) -> TagEnd {
        match self {
            Tag::Header => TagEnd::Header,
            Tag::DocumentTitle => TagEnd::DocumentTitle,
            Tag::Section { level } => TagEnd::Section { level: *level },
            Tag::SectionTitle { .. } => TagEnd::SectionTitle,
            Tag::Paragraph => TagEnd::Paragraph,
            Tag::LiteralParagraph => TagEnd::LiteralParagraph,
            Tag::DelimitedBlock { .. } => TagEnd::DelimitedBlock,
            Tag::SourceBlock { .. } => TagEnd::SourceBlock,
            Tag::BlockTitle => TagEnd::BlockTitle,
            Tag::UnorderedList { .. } => TagEnd::UnorderedList,
            Tag::OrderedList => TagEnd::OrderedList,
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
            Tag::TableCell => TagEnd::TableCell,
            Tag::TableHeaderCell => TagEnd::TableHeaderCell,
            Tag::BlockImage { .. } => TagEnd::BlockImage,
            Tag::InlineImage { .. } => TagEnd::InlineImage,
            Tag::Strong => TagEnd::Strong,
            Tag::Emphasis => TagEnd::Emphasis,
            Tag::Monospace => TagEnd::Monospace,
            Tag::Highlight => TagEnd::Highlight,
            Tag::Superscript => TagEnd::Superscript,
            Tag::Subscript => TagEnd::Subscript,
            Tag::Link { .. } => TagEnd::Link,
            Tag::CrossReference { .. } => TagEnd::CrossReference,
            Tag::Anchor { .. } => TagEnd::Anchor,
        }
    }
}
