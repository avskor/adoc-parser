//! Sentinel-string representation and the final tokenizer.
//!
//! The sequential-pass engine rewrites the paragraph text into a working
//! `String` ([`Work::buf`]) in which formatting spans are marked by *sentinel*
//! byte sequences rather than by their source markers. A sentinel is
//!
//! ```text
//! <TAG_LEAD> <decimal index> <TAG_TAIL>
//! ```
//!
//! where the index points into a side table of [`TagToken`]s (open with
//! id/roles, or close with a kind). `TAG_LEAD`/`TAG_TAIL` are control bytes
//! (`0x01`/`0x02`) that the engine refuses to run on if they already occur in
//! the input, so a sentinel can never collide with real text.
//!
//! [`tokenize`] walks the finished buffer and turns each sentinel into a
//! `Start`/`End` event and each literal run into a `Text` event, **without
//! balancing** — open and close events are emitted strictly in the order their
//! sentinels appear, so a span whose open and close were spliced into different
//! sibling spans (the Asciidoctor cross-span overlap) survives as overlapping,
//! non-nested events. The renderer emits those literally.

use std::borrow::Cow;

use crate::event::{Event, Tag, TagEnd};

/// Leading byte of a tag sentinel.
pub(super) const TAG_LEAD: u8 = 0x01;
/// Trailing byte of a tag sentinel.
pub(super) const TAG_TAIL: u8 = 0x02;

/// The kind of inline formatting span a sentinel opens or closes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SpanKind {
    Strong,
    Emphasis,
    Monospace,
    Highlight,
    Superscript,
    Subscript,
    InlineSpan,
}

impl SpanKind {
    fn into_tag<'a>(self, id: Option<String>, roles: Vec<String>) -> Tag<'a> {
        let id = id.map(Cow::Owned);
        let roles: Vec<Cow<'a, str>> = roles.into_iter().map(Cow::Owned).collect();
        match self {
            SpanKind::Strong => Tag::Strong { id, roles },
            SpanKind::Emphasis => Tag::Emphasis { id, roles },
            SpanKind::Monospace => Tag::Monospace { id, roles },
            SpanKind::Highlight => Tag::Highlight,
            SpanKind::Superscript => Tag::Superscript,
            SpanKind::Subscript => Tag::Subscript,
            SpanKind::InlineSpan => Tag::InlineSpan { id, roles },
        }
    }

    fn into_end(self) -> TagEnd {
        match self {
            SpanKind::Strong => TagEnd::Strong,
            SpanKind::Emphasis => TagEnd::Emphasis,
            SpanKind::Monospace => TagEnd::Monospace,
            SpanKind::Highlight => TagEnd::Highlight,
            SpanKind::Superscript => TagEnd::Superscript,
            SpanKind::Subscript => TagEnd::Subscript,
            SpanKind::InlineSpan => TagEnd::InlineSpan,
        }
    }
}

/// One restored fragment of an extracted passthrough. `raw` content becomes an
/// `InlinePassthrough` event (the renderer emits it verbatim); non-`raw`
/// content becomes a `Text` event (the renderer html-escapes it — the
/// `specialcharacters`-only semantics of `+…+`/`++…++`).
#[derive(Debug)]
pub(super) struct PassPiece {
    pub text: String,
    pub raw: bool,
}

/// A deferred tag, referenced by a sentinel's index.
#[derive(Debug)]
pub(super) enum TagToken {
    Open {
        kind: SpanKind,
        id: Option<String>,
        roles: Vec<String>,
    },
    Close(SpanKind),
    /// A standalone event with no span pairing (e.g. a hard line break emitted
    /// by the `post_replacements` pass).
    HardBreak,
    /// An extracted passthrough (`+…+`/`++…++`/`+++…+++`/`pass:[…]`), restored
    /// verbatim as a run of leaf events. Extracted FIRST so no later pass can
    /// reach inside the protected content.
    Passthrough(Vec<PassPiece>),
    /// An attribute reference `{name}` (optionally followed by `[brackets]` /
    /// `/path[brackets]`), restored as an `AttributeReference` event. The
    /// reference is NOT resolved here — the legacy parser also defers resolution
    /// to the renderer, so the engine mirrors that by emitting the same event.
    /// `fallback` is always `None` (the legacy parser has no `{name:fallback}`
    /// syntax).
    AttrRef {
        name: String,
        trailing_brackets: Option<String>,
    },
    /// An inline attribute assignment `{set:name:value}` / `{set:name}` /
    /// `{set:name!}` (unset → `name` is `!name`), restored as an `Attribute`
    /// event the way the legacy `{set:…}` inline macro emits it.
    AttrSet {
        name: String,
        value: String,
    },
    /// A curved/smart quote character (`“`/`”`/`‘`/`’`) produced by the
    /// `:double`/`:single` quote substitution, restored as a literal `Text`
    /// event (the legacy parser emits the character directly, not the `&#8220;`
    /// entity). `opening` marks the LEFT quote so the constrained
    /// monospace/emphasis/mark passes can suppress an immediate open right after
    /// it — mirroring the legacy `smart_quote_leading_edge`: those quotes run
    /// after `:double`/`:single` and see the boundary it leaves, which their open
    /// assertion forbids.
    SmartQuote {
        text: &'static str,
        opening: bool,
    },
    /// A backslash-escaped literal character (or short pattern) produced by the
    /// [`super::escape`] pass — the backslash is dropped and the escaped
    /// character left literal (`\{name}` → `{name}`, `\*bold*` → `*bold*`). Unlike
    /// every other token, a `Literal` does NOT start a fresh `Text` event: the
    /// tokenizer flushes the text accumulated *before* it and then *seeds* the
    /// pending run with this text, so the literal COALESCES with the following
    /// run into one `Text` event — mirroring the legacy parser, which drops the
    /// backslash and leaves the escaped character in the next text flush.
    Literal(String),
    /// A valid HTML character reference (`&#167;` / `&copy;` / `&#x2026;`). `raw`
    /// `true` (the bare-reference survival pass, [`super::char_refs`]) restores it
    /// as an `InlinePassthrough` so the renderer emits the `&` verbatim; `raw`
    /// `false` (an escaped `\&#…;`, handled by [`super::escape`]) restores it as a
    /// `Text` event so the renderer escapes the `&` to `&amp;`. Both mirror the
    /// legacy parser, which keeps a survived reference intact (passthrough) and
    /// emits an escaped one as literal text. Unlike [`TagToken::Literal`], a
    /// `CharRef` is a SEPARATE event — it flushes the pending run and does not
    /// coalesce — exactly as the legacy parser emits each reference on its own.
    CharRef { text: String, raw: bool },
    /// A complete inline macro (cross reference `xref:…[…]` / `<<…>>`, and — in
    /// later phases — link/image/footnote/…), extracted by [`super::macros`] as a
    /// single opaque leaf: its `Start` tag, its already-computed label events, and
    /// its `End` tag, stored as one owned event sequence. The label was re-parsed
    /// by an inner pipeline with `MACROS` disabled (mirroring the legacy
    /// `push_macro_label`), so unlike a span `Open`/`Close` pair the label content
    /// is NOT left in the buffer — the macro is atomic and cannot participate in
    /// cross-span overlap, exactly as the legacy parser emits a macro and its
    /// sub-parsed label together. Restored verbatim, opaque to every later pass.
    Macro(Vec<Event<'static>>),
}

/// The mutable working state of the pipeline: the rewritten buffer plus the
/// side table that the buffer's sentinels index into.
pub(super) struct Work {
    pub buf: String,
    pub tags: Vec<TagToken>,
}

impl Work {
    pub(super) fn new(text: &str) -> Self {
        Work {
            buf: text.to_string(),
            tags: Vec::new(),
        }
    }

    /// Register an open tag and return its sentinel string.
    pub(super) fn open_sentinel(
        &mut self,
        kind: SpanKind,
        id: Option<String>,
        roles: Vec<String>,
    ) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::Open { kind, id, roles });
        sentinel(idx)
    }

    /// Register a close tag and return its sentinel string.
    pub(super) fn close_sentinel(&mut self, kind: SpanKind) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::Close(kind));
        sentinel(idx)
    }

    /// Register a standalone hard-break and return its sentinel string.
    pub(super) fn break_sentinel(&mut self) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::HardBreak);
        sentinel(idx)
    }

    /// Register an extracted passthrough leaf and return its sentinel string.
    pub(super) fn passthrough_sentinel(&mut self, pieces: Vec<PassPiece>) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::Passthrough(pieces));
        sentinel(idx)
    }

    /// Register an attribute reference leaf and return its sentinel string.
    pub(super) fn attr_ref_sentinel(
        &mut self,
        name: String,
        trailing_brackets: Option<String>,
    ) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::AttrRef { name, trailing_brackets });
        sentinel(idx)
    }

    /// Register an inline attribute assignment leaf and return its sentinel.
    pub(super) fn attr_set_sentinel(&mut self, name: String, value: String) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::AttrSet { name, value });
        sentinel(idx)
    }

    /// Register a smart/curved quote leaf and return its sentinel string.
    pub(super) fn smart_quote_sentinel(&mut self, text: &'static str, opening: bool) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::SmartQuote { text, opening });
        sentinel(idx)
    }

    /// Register an escaped-literal leaf and return its sentinel string.
    pub(super) fn literal_sentinel(&mut self, text: String) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::Literal(text));
        sentinel(idx)
    }

    /// Register a character-reference leaf and return its sentinel string.
    /// `raw` controls restoration: `true` → `InlinePassthrough` (verbatim `&`,
    /// the survival pass), `false` → `Text` (escaped `&`, an escaped `\&#…;`).
    pub(super) fn char_ref_sentinel(&mut self, text: String, raw: bool) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::CharRef { text, raw });
        sentinel(idx)
    }

    /// Register a fully-formed inline-macro leaf (its `Start`, label events, and
    /// `End`) and return its sentinel string.
    pub(super) fn macro_sentinel(&mut self, events: Vec<Event<'static>>) -> String {
        let idx = self.tags.len();
        self.tags.push(TagToken::Macro(events));
        sentinel(idx)
    }
}

fn sentinel(idx: usize) -> String {
    let mut s = String::with_capacity(8);
    s.push(TAG_LEAD as char);
    s.push_str(itoa(idx).as_str());
    s.push(TAG_TAIL as char);
    s
}

/// Minimal usize→decimal without pulling in formatting machinery overhead;
/// kept tiny and allocation-light for the per-sentinel hot path.
fn itoa(mut n: usize) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    while n > 0 {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    // SAFETY-free: bytes are ASCII digits.
    String::from_utf8(digits[i..].to_vec()).expect("ascii digits")
}

/// Byte length of the UTF-8 character whose leading byte is `b`.
pub(super) fn utf8_char_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

/// Given that `bytes[i] == TAG_LEAD`, return the index just past the matching
/// `TAG_TAIL` (or end of slice if malformed — never happens for buffers this
/// engine builds).
pub(super) fn sentinel_end(bytes: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < bytes.len() && bytes[j] != TAG_TAIL {
        j += 1;
    }
    // Step past the TAG_TAIL when present; clamp to the slice length otherwise.
    if j < bytes.len() { j + 1 } else { bytes.len() }
}

/// Replace every tag sentinel in `s` with the literal source text it stands
/// for. Used by the quotes pass when a span's `[attrlist]` captured a sentinel
/// during an earlier extraction — most importantly an attribute reference
/// (`[{role}]*x*`): the role/id must carry the literal `{role}` so the renderer
/// resolves it the same way Asciidoctor's global `attributes` substitution does.
/// Asciidoctor runs `quotes` *before* `attributes`, so the literal `{role}`
/// survives into the captured attrlist and is resolved afterwards; this engine
/// runs `attributes` first (lifting `{role}` into a sentinel), so the captured
/// attrlist would otherwise hold a raw sentinel — this restores the source text
/// before the role/id is parsed, and the renderer's attribute-reference
/// resolution finishes the job (undefined → kept literal, defined → value).
///
/// Only leaf tokens with a well-defined source/text are reconstructed; a
/// structural sentinel (span open/close, hard break, macro) cannot legitimately
/// appear inside an attrlist and is dropped — such inputs diverge from the legacy
/// parser anyway and are caught by the Phase-1 gate.
pub(super) fn desentinelize(tags: &[TagToken], s: &str) -> String {
    if !s.as_bytes().contains(&TAG_LEAD) {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            let mut j = i + 1;
            let mut idx = 0usize;
            while j < bytes.len() && bytes[j] != TAG_TAIL {
                idx = idx * 10 + (bytes[j] - b'0') as usize;
                j += 1;
            }
            match tags.get(idx) {
                Some(TagToken::AttrRef { name, trailing_brackets }) => {
                    out.push('{');
                    out.push_str(name);
                    out.push('}');
                    if let Some(t) = trailing_brackets {
                        out.push_str(t);
                    }
                }
                Some(TagToken::Literal(t)) => out.push_str(t),
                Some(TagToken::CharRef { text, .. }) => out.push_str(text),
                Some(TagToken::SmartQuote { text, .. }) => out.push_str(text),
                Some(TagToken::Passthrough(pieces)) => {
                    for p in pieces {
                        out.push_str(&p.text);
                    }
                }
                Some(TagToken::AttrSet { name, value }) => {
                    // `{set:…}` is nonsensical inside an attrlist, but reconstruct
                    // it rather than drop it so nothing is silently lost.
                    out.push_str("{set:");
                    if let Some(unset) = name.strip_prefix('!') {
                        out.push_str(unset);
                        out.push('!');
                    } else {
                        out.push_str(name);
                        out.push(':');
                        out.push_str(value);
                    }
                    out.push('}');
                }
                // Open/Close/HardBreak/Macro/None: drop (cannot appear in a
                // well-formed attrlist; mediated by the gate).
                _ => {}
            }
            i = if j < bytes.len() { j + 1 } else { j };
        } else {
            let len = utf8_char_len(bytes[i]);
            out.push_str(&s[i..i + len]);
            i += len;
        }
    }
    out
}

/// Flush the accumulated literal run as a single `Text` event (no-op when empty).
fn flush_pending<'a>(events: &mut Vec<Event<'a>>, pending: &mut String) {
    if !pending.is_empty() {
        events.push(Event::Text(Cow::Owned(std::mem::take(pending))));
    }
}

/// Turn a finished working buffer into an event stream. Sentinels become
/// `Start`/`End` events (in appearance order, unbalanced); literal runs become
/// owned `Text` events.
///
/// Literal text is accumulated into `pending` so that a [`TagToken::Literal`]
/// (an escaped character) coalesces with the surrounding run into one `Text`
/// event: it flushes whatever preceded it, then seeds `pending` so the following
/// run merges with it. Every other token flushes `pending` first.
pub(super) fn tokenize<'a>(work: Work) -> Vec<Event<'a>> {
    let Work { buf, tags } = work;
    let bytes = buf.as_bytes();
    let mut events = Vec::new();
    let mut pending = String::new();
    let mut i = 0;
    let mut text_start = 0;

    while i < bytes.len() {
        if bytes[i] == TAG_LEAD {
            // Fold the literal run preceding this sentinel into `pending`.
            pending.push_str(&buf[text_start..i]);
            // Parse the decimal index up to TAG_TAIL.
            let mut j = i + 1;
            let mut idx = 0usize;
            while j < bytes.len() && bytes[j] != TAG_TAIL {
                idx = idx * 10 + (bytes[j] - b'0') as usize;
                j += 1;
            }
            match tags.get(idx) {
                // A literal flushes the preceding run, then seeds `pending` so
                // the following run merges with it into one `Text` event.
                Some(TagToken::Literal(s)) => {
                    flush_pending(&mut events, &mut pending);
                    pending.push_str(s);
                }
                Some(TagToken::Open { kind, id, roles }) => {
                    flush_pending(&mut events, &mut pending);
                    events.push(Event::Start(kind.into_tag(id.clone(), roles.clone())));
                }
                Some(TagToken::Close(kind)) => {
                    flush_pending(&mut events, &mut pending);
                    events.push(Event::End(kind.into_end()));
                }
                Some(TagToken::HardBreak) => {
                    flush_pending(&mut events, &mut pending);
                    events.push(Event::HardBreak);
                }
                Some(TagToken::Passthrough(pieces)) => {
                    flush_pending(&mut events, &mut pending);
                    for p in pieces {
                        let text = Cow::Owned(p.text.clone());
                        events.push(if p.raw {
                            Event::InlinePassthrough(text)
                        } else {
                            Event::Text(text)
                        });
                    }
                }
                Some(TagToken::AttrRef { name, trailing_brackets }) => {
                    flush_pending(&mut events, &mut pending);
                    events.push(Event::AttributeReference {
                        name: Cow::Owned(name.clone()),
                        fallback: None,
                        trailing_brackets: trailing_brackets.clone().map(Cow::Owned),
                    });
                }
                Some(TagToken::AttrSet { name, value }) => {
                    flush_pending(&mut events, &mut pending);
                    events.push(Event::Attribute {
                        name: Cow::Owned(name.clone()),
                        value: Cow::Owned(value.clone()),
                    });
                }
                Some(TagToken::SmartQuote { text, .. }) => {
                    flush_pending(&mut events, &mut pending);
                    events.push(Event::Text(Cow::Borrowed(text)));
                }
                Some(TagToken::CharRef { text, raw }) => {
                    flush_pending(&mut events, &mut pending);
                    let t = Cow::Owned(text.clone());
                    events.push(if *raw {
                        Event::InlinePassthrough(t)
                    } else {
                        Event::Text(t)
                    });
                }
                Some(TagToken::Macro(evs)) => {
                    flush_pending(&mut events, &mut pending);
                    // Each stored `Event<'static>` coerces to `Event<'a>` on push
                    // (covariance: `'static` outlives `'a`).
                    for e in evs {
                        events.push(e.clone());
                    }
                }
                None => flush_pending(&mut events, &mut pending),
            }
            i = j + 1;
            text_start = i;
        } else {
            i += 1;
        }
    }

    pending.push_str(&buf[text_start..]);
    flush_pending(&mut events, &mut pending);

    events
}
