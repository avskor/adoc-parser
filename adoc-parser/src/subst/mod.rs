//! Sequential-pass inline substitution engine (Asciidoctor `Substitutors`
//! model).
//!
//! This is the **default** top-level inline engine: [`crate::inline`] routes
//! every top-level paragraph through [`try_parse`] first, falling back to the
//! legacy recursive parser ([`crate::inline::parse_legacy`]) only when this
//! engine declines (no inline-needing substitution requested, or the input
//! already contains a reserved sentinel byte). It handles every inline-needing
//! substitution set, including ones without `quotes` (`[subs=attributes]`,
//! `[subs=+macros]`, …).
//!
//! ## Why a string-rewriting model
//!
//! Asciidoctor applies inline substitutions as an ordered sequence of flat
//! passes over the whole paragraph string (passthrough-extract → specialchars →
//! quotes → attributes → replacements → macros → post_replacements → restore).
//! The `quotes` step is itself a sequence of independent gsub passes (strong
//! before monospace, …). Because an earlier pass splices literal tag text into
//! the string before a later pass wraps markers around it, a quote span can
//! physically *overlap* a sibling span — output Asciidoctor itself emits as
//! invalid, non-nested HTML. A recursive/tree parser (the legacy engine) can
//! only ever produce *nested* tags, so it cannot reproduce this; replicating it
//! requires the string-rewriting pipeline this module houses. This is exactly
//! why the engine is now the default: it is the only one that reproduces those
//! overlapping spans (the `outline.adoc` flip).
//!
//! ## History
//!
//! The engine was built incrementally behind a differential-equality gate (a
//! now-removed `ADOC_QUOTES_SEQUENTIAL` toggle, plus an `ADOC_SUBST_FORCE`
//! diagnostic that bypassed the gate): [`try_parse`] used to run both this
//! pipeline and the legacy parser and adopt the new result only when the two
//! event streams were byte-identical. Once the pipeline reached corpus parity
//! with Asciidoctor across every implemented pass (passthrough, escape,
//! character references, attributes, quotes, macros, replacements,
//! post-replacements), the gate was removed and the raw pipeline result is now
//! adopted directly — letting the divergent (overlapping-span) cases flip.

mod attributes;
mod char_refs;
mod escape;
mod macros;
mod passthrough;
mod post_replacements;
mod quotes;
mod replacements;
mod tokenize;

use std::cell::Cell;

use crate::event::{Event, SubstitutionSet};
use crate::inline::InlineOptions;
use tokenize::{Work, TAG_LEAD, TAG_TAIL};

/// Re-exported for the public `adoc_parser::char_ref_len` wrapper (see
/// [`crate::char_ref_len`]): renderers reuse this syntactic char-reference
/// validator to avoid re-escaping an already-formed entity in a URL/attribute.
pub(crate) use char_refs::char_ref_len;

thread_local! {
    /// Set by a pass that recognises a construct it cannot yet form faithfully and
    /// must defer to the legacy recursive parser: a macro whose span swallowed an
    /// earlier-extracted sentinel ([`macros`]), or a deferred escaped-plus form
    /// (`\++…`, [`passthrough`]). Read-and-cleared by [`try_parse`], which then
    /// falls back to legacy for the whole paragraph. Thread-local (not threaded
    /// through every matcher) and confined to one synchronous pipeline run; the
    /// recursive label re-parse shares it, so a deferred construct inside a macro
    /// label propagates the decline to the top-level paragraph. This is the
    /// explicit, per-construct replacement for the old differential-equality gate
    /// — each construct that gains native handling stops flagging.
    static DECLINED: Cell<bool> = const { Cell::new(false) };
}

/// Record that the engine cannot faithfully handle a construct and must defer to
/// the legacy parser (see [`DECLINED`]).
pub(super) fn flag_decline() {
    DECLINED.with(|d| d.set(true));
}

/// Read and clear the decline flag (see [`DECLINED`]).
fn take_decline() -> bool {
    DECLINED.with(|d| d.replace(false))
}

/// Attempt to parse top-level inline `text` with the sequential-pass engine.
///
/// Returns `None` only when the engine genuinely cannot run: no inline-needing
/// substitution is requested (a verbatim block whose `subs` is just
/// `specialchars`/`callouts` — see [`SubstitutionSet::needs_inline_parsing`]),
/// or the input already contains a reserved sentinel byte. In both cases the
/// caller falls back to the legacy recursive parser
/// ([`crate::inline::parse_legacy`]). Otherwise the raw pipeline result is
/// returned and adopted. Only called for top-level paragraph text, never for
/// inner-span reparses.
pub(crate) fn try_parse<'a>(
    text: &'a str,
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Option<Vec<Event<'a>>> {
    // No inline-needing substitution requested (e.g. a verbatim block, whose
    // `subs` carries only `specialchars`/`callouts`) — the pipeline would be a
    // no-op, so defer to legacy. This mirrors the caller's own gate
    // (`SubstitutionSet::needs_inline_parsing`, the condition under which the
    // parser routes `Text` through inline parsing at all). The engine handles
    // every inline-needing set, INCLUDING ones without `quotes` (e.g.
    // `[subs=attributes]`, `[subs=+macros]`): each pass is gated on its own flag
    // (see [`run_pipeline_with`]), and the escape pass drops quote-marker
    // backslashes itself when `quotes` is off (no quotes pass would run).
    if !subs.needs_inline_parsing() {
        return None;
    }
    // A sentinel byte in the source would be indistinguishable from an
    // engine-inserted sentinel; refuse such input outright (legacy handles it).
    if text
        .bytes()
        .any(|b| b == TAG_LEAD || b == TAG_TAIL)
    {
        return None;
    }

    // Clear any stale decline flag from an earlier paragraph on this thread, run
    // the pipeline, then check whether any pass had to defer a construct it cannot
    // yet form faithfully (a macro whose span swallowed a sentinel, or a deferred
    // escaped-plus form). When one did, fall back to the legacy recursive parser
    // (which still has the raw source) for the whole paragraph. See [`flag_decline`].
    take_decline();
    let candidate = run_pipeline(text, subs, options);
    if take_decline() {
        return None;
    }
    Some(candidate)
}

/// Run the implemented substitution passes (in Asciidoctor `subs=normal` order,
/// each gated on its presence in `subs`) and tokenize the result.
///
/// Implemented so far: `escape` (non-marker `\`-prefixed literals — `\{`, `\"`/
/// `\'` smart-quote openers, `\[`/`\<`, typographic, the `\&#…;` character-
/// reference escape, and the escaped inline-macro form `\name:target[…]`),
/// passthrough extract/restore, character-reference survival
/// (`&#167;` / `&copy;` → `InlinePassthrough`), `attributes` (`{name}` /
/// `{set:…}`, unresolved leaf events mirroring legacy), `quotes` (including the
/// `:double`/`:single` curved smart quotes), `macros` (so far the cross-reference
/// family — `xref:target[label]` and `<<target>>` — with the label re-parsed via
/// an inner `MACROS`-cleared pipeline) plus the link family (`link:`/`mailto:`
/// macros and bare URL/email autolinks), the inline image (`image:`), the leaf
/// macros `icon:`/STEM (`stem:`/`latexmath:`/`asciimath:`), the anchor
/// (`[[id]]`/`[[[id]]]`/`anchor:`) and index-term (`((…))`/`indexterm:`/
/// `indexterm2:`) families, the `footnote:`/`footnote:id[]` family, and the
/// `:experimental:`-gated UI macros (`kbd:`/`btn:`/`menu:`, dispatched only when
/// `options.experimental`), `replacements`, `post_replacements`. The
/// single-backslash escapes (quote-marker `\*`/`\_`/`` \` ``/`\#`/`\^`/`\~`,
/// `\pass:`, `\+`, `\https://`-autolink, macro `\image:`/`\link:`/…, `\&#…;`
/// char-ref, `\((…))` index) are ported into their span-aware passes, AS ARE the
/// DOUBLED-backslash forms (`\\*bold*` → `\*bold*`, `\\pass:`, `\\+`, `\\image:`,
/// `\\&#…;`, `\\((…))`, `\\++…++` → `++…++`, and bare `\\` kept intact) — only the
/// construct-adjacent backslash is consumed, and only when the construct would
/// form. The two doubled forms still deferred are pathological in Asciidoctor
/// itself: the URL-target `\\link:http://…[…]` (rendered as a link there) and the
/// triple-plus `\\+++…+++`; inputs that need them diverge from legacy and stay on
/// the legacy path (the macro pass punts them — see [`flag_decline`]).
/// `options` is threaded through so the
/// experimental flag reaches the macros pass and every inner reparse (label,
/// cross-reference label, passthrough spec content), mirroring the legacy
/// parser's `self.options` propagation into nested `InlineState`s.
fn run_pipeline<'a>(text: &str, subs: SubstitutionSet, options: InlineOptions) -> Vec<Event<'a>> {
    run_pipeline_with(Work::new(text), text, subs, options)
}

/// Re-parse a macro label whose raw text already carries an earlier-extracted
/// sentinel, seeding the working tag table with a clone of the outer pipeline's
/// tokens (`seed`) so those sentinels resolve against the same passthrough /
/// escaped-`Literal` / char-ref leaves. The inner passes step over the seeded
/// sentinels verbatim (every pass already skips a `TAG_LEAD` run) and append
/// fresh tokens after them, so the inner [`tokenize`](tokenize::tokenize) restores
/// the seeded leaves exactly as the top-level tokenizer would — the engine's
/// native replacement for the old "sentinel in the label → punt to legacy" guard.
/// Mirrors Asciidoctor, where a passthrough placeholder survives the label's
/// `subs.without(:macros)` re-substitution and is restored globally at the end.
fn run_pipeline_seeded<'a>(
    text: &str,
    seed: &[tokenize::TagToken],
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'a>> {
    run_pipeline_with(Work::with_tags(text, seed.to_vec()), text, subs, options)
}

/// Shared body of [`run_pipeline`] / [`run_pipeline_seeded`]: run the gated
/// substitution passes over `work` (already carrying `buf == text` plus any seed
/// tokens) and tokenize. `text` is kept for the empty-result guard only.
fn run_pipeline_with<'a>(
    mut work: Work,
    text: &str,
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Vec<Event<'a>> {
    // Passthroughs are extracted FIRST so their content is verbatim — opaque to
    // every later pass INCLUDING `escape` (mirrors Asciidoctor's
    // `extract_passthroughs`, which runs before all substitutions). A backslash
    // inside `+…+`/`pass:[…]` is literal content, never an escape, so extracting
    // first is what stops `escape` from mangling it (`` `+\{name}+` `` →
    // `<code>\{name}</code>`). Unconditional: the legacy parser runs
    // `+…+`/`pass:[…]` regardless of the subs flags (the engine matches that, now
    // that it also handles non-`quotes` inline sets).
    passthrough::extract(&mut work, subs, options);
    // Escapes are neutralised next, before the attribute/quote passes: a
    // recognised `\x` drops the backslash and turns `x` into a literal leaf that
    // is opaque to those passes (mirrors Asciidoctor's per-substitution `\\?`
    // capture). Running after passthrough means a `\` left in the buffer is always
    // top-level (passthrough-internal backslashes are already inside a sentinel).
    // `subs` is threaded in for the macro-escape arm (`\link:u[t]` / `\indexterm:`
    // …), which is gated on `MACROS` and seals the whole macro form so the later
    // `macros` pass never fires on it.
    escape::run(&mut work, subs);
    // Valid character references (`&#167;` / `&copy;`) are extracted next, before
    // the attribute/quote passes, into opaque survival leaves
    // (`InlinePassthrough`, so the renderer does not escape the `&`). Extracting
    // before quotes is what stops the `#` inside `&#…;` from being read as a mark
    // marker, exactly as the legacy parser consumes the whole reference
    // atomically. Gated on the legacy `preserve_char_refs` condition: both
    // `specialcharacters` AND `replacements` active (a verbatim block has the
    // former but not the latter, so it keeps references escaped). An escaped
    // `\&#…;` was already sealed by `escape::run`, so it is not re-extracted here.
    if subs.has(SubstitutionSet::SPECIALCHARS) && subs.has(SubstitutionSet::REPLACEMENTS) {
        char_refs::run(&mut work);
    }
    // Inline macros are extracted next, BEFORE attributes: the legacy parser
    // consumes a macro whole, so an attribute reference inside the target
    // (`xref:{anchor}[]`) stays literal in the target rather than becoming its own
    // event. Extracting `attributes` first would lift it into a sentinel and the
    // macro would be declined. A label is re-parsed with MACROS cleared (mirroring
    // `push_macro_label`). Every inline macro family the legacy parser recognises
    // is ported: the cross-reference family (`xref:`/`<<>>`), the link family
    // (`link:`/`mailto:`, bare URL/email autolinks), the inline image (`image:`),
    // the leaf macros `icon:` and the STEM family (`stem:`/`latexmath:`/
    // `asciimath:`), the anchor (`[[id]]`/`[[[id]]]`/`anchor:`) and index-term
    // (`((…))`/`indexterm:`/`indexterm2:`) families, footnotes, and the
    // experimental UI macros. A sentinel in a re-parsed label is now handled
    // natively (seeded re-parse, see `macros::reparse_label`); one in a verbatim
    // target / non-label attribute, or in a still-verbatim leaf family, punts to
    // legacy (see `flag_decline`).
    if subs.has(SubstitutionSet::MACROS) {
        macros::extract(&mut work, subs, options);
    }
    // Attribute references are extracted next, before quotes. The legacy parser
    // emits an unresolved `AttributeReference`; extracting `{name}[…]` up front
    // both reproduces that and protects a trailing `[brackets]` from being eaten
    // by the quotes attrlist (`{a}[.role]*x*`). See `attributes` for the
    // before-quotes rationale.
    if subs.has(SubstitutionSet::ATTRIBUTES) {
        attributes::extract(&mut work);
    }
    if subs.has(SubstitutionSet::QUOTES) {
        quotes::run_all(&mut work, options);
    }
    if subs.has(SubstitutionSet::REPLACEMENTS) {
        replacements::run(&mut work);
    }
    if subs.has(SubstitutionSet::POST_REPLACEMENTS) {
        post_replacements::run(&mut work);
    }
    let events = tokenize::tokenize(work);
    // Mirror `parse_legacy`'s empty-result guard: a buffer that tokenizes to no
    // events (e.g. an empty `++++` passthrough) becomes a single literal `Text`
    // of the original input.
    if events.is_empty() {
        vec![Event::Text(std::borrow::Cow::Owned(text.to_string()))]
    } else {
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{MenuPart, Tag, TagEnd};

    fn legacy(text: &str) -> Vec<Event<'_>> {
        crate::inline::parse_legacy(text, SubstitutionSet::NORMAL, InlineOptions::default())
    }

    fn pipeline(text: &str) -> Vec<Event<'_>> {
        run_pipeline(text, SubstitutionSet::NORMAL, InlineOptions::default())
    }

    /// Like [`legacy`], but with an explicit [`SubstitutionSet`] — for the
    /// non-`quotes` differential tests (`[subs=attributes]`, `[subs=+macros]`, …).
    fn legacy_subs(text: &str, subs: SubstitutionSet) -> Vec<Event<'_>> {
        crate::inline::parse_legacy(text, subs, InlineOptions::default())
    }

    /// Like [`pipeline`], but with an explicit [`SubstitutionSet`].
    fn pipeline_subs(text: &str, subs: SubstitutionSet) -> Vec<Event<'_>> {
        run_pipeline(text, subs, InlineOptions::default())
    }

    /// Like [`legacy`], but with `:experimental:` set so the legacy parser
    /// recognises the `kbd:`/`btn:`/`menu:` UI macros — the reference the ported
    /// new-engine arms must reproduce.
    fn legacy_exp(text: &str) -> Vec<Event<'_>> {
        crate::inline::parse_legacy(
            text,
            SubstitutionSet::NORMAL,
            InlineOptions { experimental: true, ..Default::default() },
        )
    }

    /// Like [`pipeline`], but with `:experimental:` set so the macros pass
    /// dispatches the UI macros.
    fn pipeline_exp(text: &str) -> Vec<Event<'_>> {
        run_pipeline(
            text,
            SubstitutionSet::NORMAL,
            InlineOptions { experimental: true, ..Default::default() },
        )
    }

    /// Like [`pipeline`], but with `:compat-mode:` set so `+text+`/`++text++`
    /// render as monospace instead of being extracted as passthroughs.
    fn pipeline_compat(text: &str) -> Vec<Event<'_>> {
        run_pipeline(
            text,
            SubstitutionSet::NORMAL,
            InlineOptions { compat_mode: true, ..Default::default() },
        )
    }

    /// In compat mode the `+`/`++` markers move from passthrough to monospace
    /// (Asciidoctor `QUOTE_SUBS[true]`), while the raw triple `+++…+++` stays a
    /// passthrough and the non-compat behaviour is unchanged.
    #[test]
    fn compat_mode_plus_renders_monospace() {
        // constrained `+text+` → monospace with normal subs
        assert_eq!(
            pipeline_compat("+text+"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("text".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
        // unconstrained `++text++` → monospace (no word boundary needed)
        assert_eq!(
            pipeline_compat("a++x++b"),
            vec![
                Event::Text("a".into()),
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("x".into()),
                Event::End(TagEnd::Monospace),
                Event::Text("b".into()),
            ],
        );
        // nested strong inside `+…+` (strong pass runs before the `+` pass)
        assert_eq!(
            pipeline_compat("+a *b* c+"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("a ".into()),
                Event::Start(Tag::Strong { id: None, roles: vec![] }),
                Event::Text("b".into()),
                Event::End(TagEnd::Strong),
                Event::Text(" c".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
        // attrlist id/roles on a compat-mode plus span
        assert_eq!(
            pipeline_compat("[#i.r]+x+"),
            vec![
                Event::Start(Tag::Monospace {
                    id: Some("i".into()),
                    roles: vec!["r".into()],
                }),
                Event::Text("x".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
    }

    /// `[x-]` (and `[<attrs> x-]`) is Asciidoctor's literal-monospace marker:
    /// the role `x-` is dropped and the content renders as `<code>` with the OLD
    /// behaviour — a backtick close uses BASIC_SUBS (specialchars only, so
    /// `*b*`/`_em_`/`{attr}` stay literal), a `+` close uses NORMAL_SUBS.
    /// `[<attrs> x-]` keeps the leading role. A non-`x-` attrlist (`[x-y]`,
    /// `[foo]`) is an ordinary role and is left untouched.
    #[test]
    fn x_marker_literal_monospace() {
        // backtick → BASIC_SUBS: role dropped, content literal (no strong, `{v}`
        // unresolved).
        assert_eq!(
            pipeline("[x-]`*b* {v}`"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("*b* {v}".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
        // plus → NORMAL_SUBS: role dropped, emphasis applied.
        assert_eq!(
            pipeline("[x-]+_em_+"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Start(Tag::Emphasis { id: None, roles: vec![] }),
                Event::Text("em".into()),
                Event::End(TagEnd::Emphasis),
                Event::End(TagEnd::Monospace),
            ],
        );
        // `[<attrs> x-]` keeps the leading role (here `method`), NORMAL_SUBS.
        assert_eq!(
            pipeline("[method x-]+save()+"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec!["method".into()] }),
                Event::Text("save()".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
        // regress: a non-`x-` attrlist is an ordinary monospace role, NOT a
        // marker — `*b*` inside is still literal under BASIC_SUBS? No: an ordinary
        // role keeps the content in the buffer, so the quotes passes DO apply.
        assert_eq!(
            pipeline("[x-y]`c`"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec!["x-y".into()] }),
                Event::Text("c".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
        assert_eq!(
            pipeline("[foo]`c`"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec!["foo".into()] }),
                Event::Text("c".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
    }

    /// The raw triple `+++…+++` passthrough survives compat mode (only `+`/`++`
    /// move to monospace), and outside compat mode `+`/`++` are unchanged.
    #[test]
    fn compat_mode_preserves_triple_plus_and_non_compat() {
        // triple-plus stays a raw passthrough even in compat mode
        assert_eq!(
            pipeline_compat("+++raw *x*+++"),
            vec![Event::InlinePassthrough("raw *x*".into())],
        );
        // non-compat: `+`/`++` are passthroughs, not monospace (regression guard)
        assert_eq!(pipeline("+text+"), vec![Event::Text("text".into())]);
        assert_eq!(pipeline("++mono++"), vec![Event::Text("mono".into())]);
    }

    /// In compat mode the AsciiDoc.py quote forms appear in `QUOTE_SUBS[true]`:
    /// `` ``…'' `` → curved double quotes, `'…'` → emphasis, `` `…' `` → curved
    /// single quotes (Asciidoctor `asciidoctor.rb:469-485`, order
    /// double → emphasis → single).
    #[test]
    fn compat_mode_curved_quotes_and_single_emphasis() {
        // ``double'' → curved double quotes (U+201C/U+201D); the curly leaves are
        // emitted as their own text events (like the modern smart-quote pass)
        assert_eq!(
            pipeline_compat("``page''"),
            vec![
                Event::Text("\u{201C}".into()),
                Event::Text("page".into()),
                Event::Text("\u{201D}".into()),
            ],
        );
        // `single' → curved single quotes (U+2018/U+2019)
        assert_eq!(
            pipeline_compat("the `chunk' here"),
            vec![
                Event::Text("the ".into()),
                Event::Text("\u{2018}".into()),
                Event::Text("chunk".into()),
                Event::Text("\u{2019}".into()),
                Event::Text(" here".into()),
            ],
        );
        // 'text' → emphasis (constrained single-quote)
        assert_eq!(
            pipeline_compat("'Give it a try!'"),
            vec![
                Event::Start(Tag::Emphasis { id: None, roles: vec![] }),
                Event::Text("Give it a try!".into()),
                Event::End(TagEnd::Emphasis),
            ],
        );
        // attrlist id/roles on a single-quote emphasis span
        assert_eq!(
            pipeline_compat("[.lead]'thrilled'"),
            vec![
                Event::Start(Tag::Emphasis { id: None, roles: vec!["lead".into()] }),
                Event::Text("thrilled".into()),
                Event::End(TagEnd::Emphasis),
            ],
        );
        // double runs before single: ``x'' and `y' coexist
        assert_eq!(
            pipeline_compat("``x'' and `y'"),
            vec![
                Event::Text("\u{201C}".into()),
                Event::Text("x".into()),
                Event::Text("\u{201D}".into()),
                Event::Text(" and ".into()),
                Event::Text("\u{2018}".into()),
                Event::Text("y".into()),
                Event::Text("\u{2019}".into()),
            ],
        );
    }

    /// Constrained boundaries protect apostrophes: an opening `'` after a word
    /// character does not open emphasis, so `don't`/`O'Reilly` stay plain (the
    /// apostrophe is later handled by replacements, not quotes).
    #[test]
    fn compat_mode_single_quote_respects_word_boundary() {
        // no emphasis span emitted (no Start/End Emphasis); content is one run
        let evs = pipeline_compat("don't and O'Reilly stay plain");
        assert!(
            !evs.iter().any(|e| matches!(
                e,
                Event::Start(Tag::Emphasis { .. }) | Event::End(TagEnd::Emphasis)
            )),
            "apostrophes must not open single-quote emphasis: {evs:?}"
        );
    }

    /// Outside compat mode the new compat quote forms are inert (regression
    /// guard): `'text'` is not emphasis and `` ``x'' `` is not curved.
    #[test]
    fn compat_quote_forms_inert_without_compat() {
        // single-quote emphasis only fires under compat
        assert!(
            !pipeline("'thrilled' to announce").iter().any(|e| matches!(
                e,
                Event::Start(Tag::Emphasis { .. })
            )),
            "single-quote emphasis must be gated on compat mode"
        );
    }

    /// The new pipeline must reproduce the legacy parser byte-for-byte on every
    /// input that involves only the implemented `quotes` constructs and no
    /// substitution the engine defers (replacements/macros/attributes/…).
    #[test]
    fn reproduces_legacy_on_quotes_only_inputs() {
        let cases = [
            // bare formatting, all markers, constrained + unconstrained
            "*bold*",
            "_em_",
            "`mono`",
            "#mark#",
            "^sup^",
            "~sub~",
            "**strong**",
            "__emph__",
            "``code``",
            "##hi##",
            // mixed siblings
            "_em_ and *strong* and `code`",
            "a*b*c",
            "**unc** then `m`",
            // nesting (both orderings)
            "*a _b_ c*",
            "_a *b* c_",
            "*a `b` c*",
            "`a *b* c`",
            // leading-edge cases reproduced by pass ordering, no edge flags
            "_`code`_",
            "_*b*_",
            "`*b*`",
            // attrlist spans
            "[.role]*x*",
            "[#id.cls]_y_",
            "[big]##O##",
            "[role]#span#",
            "word[role]#x# stays literal",
            "mid[x]##word##",
            // plain text and non-triggering punctuation
            "plain text no markup",
            "a lone . and ( stay literal",
            "trailing marker * alone",
            "unterminated *open and _open",
            // empty / doubled-edge
            "**",
            "``",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// Constrained-span close search: Asciidoctor's lazy `(\S|\S.*?\S)` content
    /// keeps scanning past a marker that cannot close (one preceded by a space —
    /// content would end in whitespace — or one whose trailing lookahead fails),
    /// absorbing it into the content via `.` and matching a *later* valid marker.
    /// The legacy parser stops at the first marker and abandons the span, so for
    /// these inputs the engine is more Asciidoctor-faithful; with the gate removed
    /// `try_parse` now ADOPTS the engine result (asserted below). This is the
    /// `outline.adoc` flip: `` `head` or `header; `foot` or `footer` ``.
    #[test]
    fn constrained_close_search_matches_asciidoctor() {
        // Raw engine (force-equivalent) matches the Asciidoctor reference: the
        // first inner marker is preceded by a space (or followed by a word char),
        // so the span closes at the next valid marker, the literal backtick living
        // inside the monospace content.
        for (input, expected) in [
            // space before the first candidate close → absorbed into content
            (
                "x `a; `b` y",
                vec![
                    Event::Text("x ".into()),
                    Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                    Event::Text("a; `b".into()),
                    Event::End(TagEnd::Monospace),
                    Event::Text(" y".into()),
                ],
            ),
            (
                "`a `b` c",
                vec![
                    Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                    Event::Text("a `b".into()),
                    Event::End(TagEnd::Monospace),
                    Event::Text(" c".into()),
                ],
            ),
            // first candidate close is followed by a word char (mono lookahead
            // `(?![\p{Word}"'`])` fails) → absorbed, closes at the trailing marker
            (
                "`a`b`",
                vec![
                    Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                    Event::Text("a`b".into()),
                    Event::End(TagEnd::Monospace),
                ],
            ),
        ] {
            assert_eq!(pipeline(input), expected, "force result for {input:?}");
        }
        // With the gate removed the engine ADOPTS these (the raw, Asciidoctor-
        // faithful result) instead of falling back to the more permissive legacy
        // parser, which leaves the leading marker literal and closes at the first
        // inner marker.
        for c in ["x `a; `b` y", "`a `b` c", "`a`b`"] {
            assert_eq!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()),
                Some(pipeline(c)),
                "engine should adopt the Asciidoctor-faithful result for {c:?}"
            );
        }
        // Regression guard: where the first marker is already a valid close (normal
        // spans, internal spaces, or no later marker to fall through to), the loop
        // returns the same position as before and the engine still reproduces
        // legacy byte-for-byte.
        for c in [
            "`code`",
            "a `b c` d",
            "`a *b* c`",
            "x `mono` y",
            "`foo`bar",   // close followed by word, no later marker → no span (both)
            "`trailing ", // no close at all → literal (both)
        ] {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the `replacements` pass ported, the pipeline must also reproduce
    /// legacy on inputs that mix quotes with typographic replacements
    /// (apostrophe, dashes, arrows, (C)/(R)/(TM), ellipsis). The whole-buffer
    /// replacement treats sentinel bytes as span boundaries, so an edge-anchored
    /// `--` inside a span stays literal while a top-level one becomes an em-dash.
    #[test]
    fn reproduces_legacy_on_replacement_inputs() {
        let cases = [
            // apostrophe (curly), inside and outside spans
            "don't worry",
            "*it's* fine",
            "`it's` code",
            "_a don't b_",
            // spaced em-dash: top-level vs span-internal
            "a -- b",
            "*--*",
            "`--`",
            "*a -- b*",
            "-- leading",
            // word--word em-dash, ellipsis, arrows, symbols
            "foo--bar",
            "wait... what",
            "a->b and c=>d",
            "x<-y and z<=w",
            "(C) (R) (TM) 2024",
            "rights (C)2024",
            // backtick-apostrophe closing smart quote
            "the `'90s",
            // mixed span + replacement siblings
            "*bold* and don't and `code`",
            "see -> *there*",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// An attribute reference (`{name}`) or inline set (`{set:…}`) is emitted as
    /// its own event by the legacy parser, which *splits* the surrounding text
    /// run — and a run-split edge counts as a line boundary for the spaced
    /// em-dash (`{empty}--{empty}` → ` — `). The new engine reproduces that by
    /// splitting the buffer at AttrRef/AttrSet sentinels before the replacements
    /// pass, while quote/passthrough sentinels stay *inside* a segment and keep
    /// their `<tag>`-like non-boundary treatment (`{empty}*--*{empty}` keeps the
    /// span-internal `--` literal). Replacements that do not depend on boundaries
    /// ((C)/ellipsis/arrows) are unaffected by the split.
    #[test]
    fn reproduces_legacy_on_attr_ref_emdash_boundary_inputs() {
        let cases = [
            // attr-ref-flanked em-dash → spaced em-dash at the split edges
            "{empty}--{empty}",
            "x{empty}--{empty}y",
            "{empty}--",
            "--{empty}",
            "a{empty}--b",
            "a--{empty}b",
            // real spaces around `--` near an attr-ref (already a spaced em-dash)
            "{empty} -- {empty}",
            // inline set is a boundary too
            "{set:foo:bar}--{set:foo:bar}",
            "{set:foo!}--end",
            // quote sentinel beside an attr-ref: the span-internal `--` stays literal
            "*--*{empty}",
            "{empty}*--*{empty}",
            "{empty}`--`{empty}",
            // boundary-independent replacements next to an attr-ref
            "{empty}...{empty}",
            "{empty}(C){empty}",
            "{empty}->{empty}",
            // apostrophe at a segment edge does not fire (needs flanking alnum)
            "a{empty}'s",
            // attr-ref with a trailing bracket segment beside `--`
            "{url}[x]--{url}[y]",
            // multiple attr-refs and dashes interleaved
            "{a}--{b}--{c}",
            // attr-ref far from any replacement (no spurious change)
            "see {version} for details",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With `post_replacements` ported, the pipeline must reproduce legacy on
    /// hard-break (` +`) inputs. The end-of-buffer break fires only at the true
    /// line edge: a ` +` inside a span is followed by its close sentinel and
    /// stays literal, with no `edges_are_line_boundaries` flag needed.
    #[test]
    fn reproduces_legacy_on_hard_break_inputs() {
        let cases = [
            // top-level trailing break
            "line one +",
            "see *there* +",
            "a +\nb",
            "a +\nb +\nc",
            // ` +` NOT a break (space-plus-space, mid-line)
            "a + b",
            "one + two + three",
            // ` +` inside a span stays literal (no break)
            "*x +*",
            "`code +`",
            "_em +_",
            // span then trailing break at the real edge
            "*x* +",
            "`m` +\nnext",
            // break combined with replacements
            "don't stop +\ngo",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With passthrough extraction ported, the pipeline must reproduce legacy on
    /// every `+…+`/`++…++`/`+++…+++`/`pass:[…]` form and its interaction with the
    /// surrounding quotes. Extraction runs first, so the protected content is
    /// opaque to the quote passes.
    #[test]
    fn reproduces_legacy_on_passthrough_inputs() {
        let cases = [
            // single / double / triple plus, bare
            "+single+",
            "++double++",
            "+++triple+++",
            // raw vs escaped content (renderer escapes Text, not InlinePassthrough)
            "+a <b> c+",
            "++a <b> c++",
            "+++a <b> c+++",
            // empty double-plus, and the surrounding text split it preserves
            "a++++b",
            "++++",
            // constrained open/close rules: word-before, space-after, inner '+'
            "C+a+ stays literal",
            "+ a+ stays literal",
            "+a+b+",
            "x + y (not a span)",
            // bare pass macro, raw verbatim, including empty
            "pass:[<raw>]",
            "pass:[]",
            "before pass:[x] after",
            // pass macro embedded in a single-plus span
            "+pass:[x]+",
            "+a pass:[<b>] c+",
            // passthrough inside / beside quote spans
            "`+mono pass+`",
            "*pass:[x]*",
            "+x+*y*",
            "a +b+ c and *d*",
            "see ++raw++ and _em_",
            // passthrough beside replacements (em-dash / apostrophe untouched inside)
            "++a -- b++ then c -- d",
            "+don't+ and don't",
            // hard-break `+` must NOT be claimed by single-plus (the legacy parser
            // consumes ` +\n` at the space before the `+` can open) — the image-ref
            // table-cell pattern that surfaced this
            "`id=x` +\n(or `+[[x]]+` or `[#x]` more)",
            "foo +\n+bar+ baz",
            "a +\nb +\nc",
            // but a `+` whose content starts with `\n` and has NO leading space is
            // a genuine single-plus span (no hard-break interception)
            "+\nfoo+",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the `attributes` pass ported, the pipeline must reproduce legacy on
    /// `{name}` references (unresolved leaf events), `{set:…}` assignments, the
    /// trailing-bracket capture, and their interaction with surrounding quotes.
    /// Extraction runs before quotes, so a captured `[brackets]` is protected
    /// from the quotes attrlist and the reference stays opaque inside a span.
    #[test]
    fn reproduces_legacy_on_attribute_inputs() {
        let cases = [
            // bare references, mixed with text
            "{name}",
            "{author}",
            "see {version} here",
            "{a}{b}{c}",
            // not references → stay literal
            "{n!}",
            "{counter:x}",
            "{}",
            "{ spaced }",
            "{-leading-dash}",
            // trailing brackets / path (renderer re-parses value + brackets)
            "{url}[text]",
            "{url}/issues[text]",
            "{a}[unclosed kept",
            // reference inside / beside quote spans
            "_{name}_",
            "*{name}*",
            "`{name}`",
            "before {name} *after*",
            // captured trailing bracket must not become a quotes attrlist
            "{a}[.role]*x*",
            "{a} [.role]*x*",
            // {set:…} inline assignment, all three forms
            "{set:foo:bar}",
            "{set:foo}",
            "{set:foo!}",
            "x {set:k:v} y",
            // reference beside replacements (apostrophe untouched between)
            "don't {name} and don't",
            "{name} -- dash",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// F-W. A backslash-escaped typographic replacement (`\...`, `\--`, `\(C)`, …)
    /// inside the path of an attribute reference with a trailing `[...]` is sealed
    /// by the `escape` pass into a `Literal` sentinel. `attributes::extract` captures
    /// the path as a raw buffer slice, so that sentinel used to leak into the
    /// emitted `AttributeReference.trailing_brackets` — and its index digit then
    /// surfaced as a spurious `0` when the renderer re-parsed it in a fresh table.
    /// The pass now desentinelizes the trailing, so the event carries clean literal
    /// text (the backslash already dropped, no control bytes). The engine
    /// intentionally diverges from legacy here (legacy keeps the raw `\...` in its
    /// trailing, which renders the wrong href) — these inputs are not in any
    /// `reproduces_legacy_on_*` set.
    #[test]
    fn attr_ref_trailing_desentinelizes_escaped_typographic() {
        fn trailing_of(text: &str) -> String {
            let evs = pipeline(text);
            let ev = evs
                .iter()
                .find(|e| matches!(e, Event::AttributeReference { .. }))
                .unwrap_or_else(|| panic!("no AttributeReference for {text:?}: {evs:?}"));
            match ev {
                Event::AttributeReference { trailing_brackets, .. } => trailing_brackets
                    .as_ref()
                    .unwrap_or_else(|| panic!("no trailing for {text:?}"))
                    .to_string(),
                _ => unreachable!(),
            }
        }

        // Flagship (CHANGELOG.adoc pattern): the `\...` Literal is resolved to a
        // bare `...` (backslash dropped), with no sentinel control bytes left.
        let t = trailing_of("{url-repo}/compare/v2.0.25\\...v2.0.26[full diff]");
        assert_eq!(t, "/compare/v2.0.25...v2.0.26[full diff]");

        // Every non-angle-bracket typographic escape resolves to its literal form.
        assert_eq!(trailing_of("{u}/a\\--b[d]"), "/a--b[d]");
        assert_eq!(trailing_of("{u}/a\\(C)b[d]"), "/a(C)b[d]");
        assert_eq!(trailing_of("{u}/a\\(R)b[d]"), "/a(R)b[d]");
        assert_eq!(trailing_of("{u}/a\\(TM)b[d]"), "/a(TM)b[d]");

        // No trailing ever contains a reserved control byte (the leak signature).
        for c in [
            "{url-repo}/compare/v2.0.25\\...v2.0.26[full diff]",
            "{u}/a\\--b[d]",
            "{u}/a\\(C)b[d]",
        ] {
            assert!(
                !trailing_of(c).bytes().any(|b| b == TAG_LEAD || b == TAG_TAIL),
                "sentinel byte leaked into trailing for {c:?}"
            );
        }

        // Regression guard: a sentinel-free trailing is untouched (desentinelize
        // fast-path), so the common `{url}[text]` link path still matches legacy.
        for c in ["{url}[text]", "{url}/issues[text]", "{a}[.role]*x*"] {
            assert_eq!(pipeline(c), legacy(c), "diverged from legacy for {c:?}");
        }
    }

    /// With the `:double`/`:single` curved smart quotes ported, the pipeline must
    /// reproduce legacy on every `"`…`"`/`'`…`'` form: the separate curly-quote
    /// `Text` events, the leading-edge suppression of constrained
    /// monospace/emphasis/mark (but not strong/superscript/subscript), the
    /// positional (leading-only) nature of that suppression, nesting, and the
    /// unclosed/empty non-matches.
    #[test]
    fn reproduces_legacy_on_smart_quote_inputs() {
        let cases = [
            // bare double / single, in isolation and in a sentence
            "\"`text`\"",
            "'`text`'",
            "He said \"`hello`\" to her",
            // strong opens at the leading edge (runs before :double)
            "\"`*bold* text`\"",
            // monospace/emphasis/mark suppressed at the leading edge
            "\"``end points``\"",
            "\"`_em_ x`\"",
            "\"`#mk# x`\"",
            // suppression is positional: a later span (after a space) still opens
            "\"`a `c` b`\"",
            // nested single-outer / double-inner
            "'`outer \"`inner`\" end`'",
            // unclosed and empty stay literal (no span)
            "\"`unclosed",
            "\"``\"",
            "'``'",
            // smart quotes beside other spans and replacements
            "*bold* and \"`quote`\" and `code`",
            "\"`don't worry`\"",
            "see \"`there`\" -> *go*",
            // a bare double-quote that is not a smart quote stays literal
            "say \"hello\" plainly",
            "it's a 'plain' word",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// A monospace span whose content starts with an apostrophe (`` `'a'` ``)
    /// must NOT let the modern single-smart-quote pass falsely open at that inner
    /// `'`+`` ` `` and swallow a following span. Asciidoctor's `:single`/`:double`
    /// `QUOTE_SUBS` are constrained — the opener needs a non-word left boundary —
    /// so `` `'a'` and `'b'` `` is two independent `<code>` spans with literal
    /// apostrophes, NOT one collapsed span with curly quotes (the
    /// `docs/modules/api/pages/index.adoc` frontier cascade). Matches asciidoctor
    /// 2.0.23. The positive smart-quote cases stay covered by
    /// [`reproduces_legacy_on_smart_quote_inputs`] and the HTML fixtures.
    #[test]
    fn monospace_apostrophe_does_not_leak_smart_quote() {
        let mono = || Event::Start(Tag::Monospace { id: None, roles: vec![] });
        // Core: two monospace spans, each literal `'a'`/`'b'`, plain text between.
        assert_eq!(
            pipeline("`'a'` and `'b'`"),
            vec![
                mono(),
                Event::Text("'a'".into()),
                Event::End(TagEnd::Monospace),
                Event::Text(" and ".into()),
                mono(),
                Event::Text("'b'".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
        // Same with surrounding text (positional: the bug was independent of the
        // line start).
        assert_eq!(
            pipeline("it `'a'` and `'b'` end"),
            vec![
                Event::Text("it ".into()),
                mono(),
                Event::Text("'a'".into()),
                Event::End(TagEnd::Monospace),
                Event::Text(" and ".into()),
                mono(),
                Event::Text("'b'".into()),
                Event::End(TagEnd::Monospace),
                Event::Text(" end".into()),
            ],
        );
        // A genuine smart quote beside such spans still forms (left boundary is a
        // space): the curly opener/closer wrap the words, the backtick span is
        // untouched. `'plain'` (no backtick) stays literal in modern mode.
        assert_eq!(
            pipeline("\"`q`\" and `'x'`"),
            vec![
                Event::Text("\u{201C}".into()),
                Event::Text("q".into()),
                Event::Text("\u{201D}".into()),
                Event::Text(" and ".into()),
                mono(),
                Event::Text("'x'".into()),
                Event::End(TagEnd::Monospace),
            ],
        );
    }

    /// With the `escape` pass ported, the pipeline must reproduce legacy on the
    /// NON-marker backslash escapes it handles: the `\{name}` attribute-reference
    /// escape, the `\"`/`\'` smart-quote opener escapes, and the generic
    /// single-character escapes (`\[`/`\<`/`\'`). Each drops the backslash and
    /// keeps the character literal, COALESCING with the following text into one
    /// `Text` event exactly as the legacy parser does. (Quote-marker escapes
    /// `\*`/`\_`/`` \` ``/`\#`/`\^`/`\~` and the passthrough escape `\+` are
    /// intentionally NOT handled here — see [`escape_marker_left_untouched`].)
    #[test]
    fn reproduces_legacy_on_escape_inputs() {
        let cases = [
            // attribute-reference escape (the unresolved-references win): the
            // escaped `{name}` stays literal while a later live one resolves
            "\\{name}",
            "\\{author} and {author}",
            "see \\{version} here",
            // escaped smart-quote openers (the escaped-smart-quote win)
            "\\\"`text`\"",
            "\\'`text`'",
            "say \\\"`q`\" and \"`real`\"",
            // generic single-character escapes (bracket / angle / apostrophe)
            "\\[x]",
            "\\<x",
            "it\\'s mine",
            // escape mixed with a live span / reference after it
            "\\{name} *bold*",
            // a backslash INSIDE a passthrough is verbatim content, NOT an escape:
            // passthrough is extracted first, so the `\{` never reaches this pass
            // (the `` `+\{name}+` `` monospace-around-passthrough regression)
            "+\\{name}+",
            "`+\\{name}+`",
            "pass:[\\{x}]",
            // a backslash before a non-escapable char stays literal
            "a\\b c",
            "path\\to\\file",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// The quote-marker escape is span-aware (folded into each quote pass), so it
    /// must never hide an *enclosing* span's closing marker: a `\` that is genuine
    /// span content (`` `\` ``) leaves the monospace intact rather than tearing it
    /// apart. This is the bug that ruled out an escape-FIRST pass.
    #[test]
    fn marker_escape_does_not_tear_spans() {
        // `\` inside a monospace span is literal content — the span still forms.
        assert_eq!(
            pipeline("`\\`"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("\\".into()),
                Event::End(TagEnd::Monospace),
            ]
        );
        // Two adjacent monospace spans around a literal backslash and bracket: the
        // first span must not swallow the second.
        assert_eq!(
            pipeline("(`\\`) (`]`)"),
            vec![
                Event::Text("(".into()),
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("\\".into()),
                Event::End(TagEnd::Monospace),
                Event::Text(") (".into()),
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("]".into()),
                Event::End(TagEnd::Monospace),
                Event::Text(")".into()),
            ]
        );
    }

    /// Quote-marker escapes (`\*`/`\_`/`` \` ``/`\#`/`\^`/`\~`) and the single-plus
    /// passthrough escape (`\+`) where Asciidoctor and the legacy parser AGREE:
    /// an escaped marker that *would* open a span drops the backslash and keeps
    /// the construct literal, while later passes still process the content
    /// (`\*_em_*` → `*<em>em</em>*`, `\+*b*+` → `+<strong>b</strong>+`). The
    /// `\*`/`\_`/`` \` `` no-close keeps match legacy too (its arm 1001).
    /// Quote-marker escapes (`\*`/`\_`/`` \` ``/`\#`/`\^`/`\~`) and the single-plus
    /// passthrough escape (`\+`) where the new engine reproduces legacy at the
    /// event level: an escaped marker that *would* open a span at the start of the
    /// run drops the backslash and keeps the construct literal, while later passes
    /// still process the content (`\*_em_*` → `*<em>em</em>*`,
    /// `\+*b*+` → `+<strong>b</strong>+`).
    #[test]
    fn reproduces_legacy_on_marker_escape_inputs() {
        let cases = [
            // escaped span → dropped backslash, literal markers
            "\\*bold*",
            "\\_em_",
            "\\`code`",
            "\\#mark#",
            "\\^sup^",
            "\\~sub~",
            // content between the escaped markers still gets substituted
            "\\*_em_*",
            "\\^*b*^",
            // monospace wrapping an escaped strong (cross-pass)
            "`\\*bold*`",
            // single-plus passthrough escape: literal `+`, content substituted
            "\\+x+",
            "\\+*b*+",
            "\\+a+ +b+",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// Marker escapes where the engine reproduces Asciidoctor but NOT the legacy
    /// event stream — either because the new engine fixes a legacy bug (Asciidoctor
    /// keeps `\#`/`\^`/`\~`/`\+` literal when no span/passthrough forms, but legacy
    /// wrongly dropped the backslash) or because the literal coalesces into a single
    /// `Text` where legacy split it (same HTML). Verified against the Asciidoctor
    /// reference, so these assert the exact event vector rather than `legacy`.
    #[test]
    fn marker_escape_matches_asciidoctor() {
        for (input, expected) in [
            // `*`/`_`/`` ` `` with no closing marker → keep (HTML matches legacy,
            // which splits the run; the engine coalesces it)
            ("\\* not bold", "\\* not bold"),
            ("\\_ not em", "\\_ not em"),
            ("\\` not code", "\\` not code"),
            // escape applies mid-word (the `\` is the boundary); drop + coalesce
            ("word\\*bold*", "word*bold*"),
            // `#`/`^`/`~`/`\+` no-span keeps — the legacy-bug fixes
            ("\\# no mark", "\\# no mark"),
            ("\\^ no sup", "\\^ no sup"),
            ("\\~ no sub", "\\~ no sub"),
            ("word\\#tag", "word\\#tag"),
            ("\\+nopass", "\\+nopass"),
            ("a\\+b+c", "a\\+b+c"),
        ] {
            assert_eq!(
                pipeline(input),
                vec![Event::Text(expected.into())],
                "expected {expected:?} for {input:?}"
            );
        }
        // `\#` inside a monospace span stays literal (legacy dropped it).
        assert_eq!(
            pipeline("`a \\# b`"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("a \\# b".into()),
                Event::End(TagEnd::Monospace),
            ]
        );
    }

    /// `\'` is an escape ONLY where the apostrophe replacement (`(\w)\\?'(?=\w)`)
    /// would fire — i.e. between two word characters. There the backslash drops for
    /// a literal apostrophe (`it\'s` → `it's`, NOT the curly `it&#8217;s`). Anywhere
    /// else the `\` has no replacement to escape, so Asciidoctor keeps `\'` literal
    /// (`\'.text'`, `\'word'`, `\'>'`). The legacy parser wrongly dropped the
    /// backslash in EVERY position, so the engine diverges from legacy here and is
    /// verified against the Asciidoctor 2.0.23 reference instead.
    #[test]
    fn escaped_apostrophe_matches_asciidoctor() {
        // NOT word-flanked → keep `\'` literal (the legacy-bug fix).
        for (input, expected) in [
            ("\\'.text'", "\\'.text'"),
            ("the \\'word' quote", "the \\'word' quote"),
            ("\\'word'", "\\'word'"),
            ("\\'>'", "\\'>'"),
        ] {
            assert_eq!(
                pipeline(input),
                vec![Event::Text(expected.into())],
                "expected {expected:?} for {input:?}"
            );
        }
        // Word-flanked → drop the backslash for a LITERAL apostrophe (no curly).
        // Matches legacy at the event level (also covered by
        // `reproduces_legacy_on_escape_inputs`); pinned here for the boundary contrast.
        assert_eq!(
            pipeline("it\\'s"),
            vec![Event::Text("it".into()), Event::Text("'s".into())]
        );
        assert_eq!(
            pipeline("a\\'b"),
            vec![Event::Text("a".into()), Event::Text("'b".into())]
        );
    }

    /// A `++`/`+++` run that fails to close as a double/triple passthrough is not
    /// a passthrough — Asciidoctor's `InlinePassRx` single-`+` form then claims it
    /// (its `InlinePassMacroRx` multi-plus phase having declined). The engine
    /// reproduces this by falling through to the single-plus form from the same
    /// `+`, with the constrained close allowing an adjacent `+` (`\S` before,
    /// non-word after). The legacy parser left these runs literal (`+++` → `+++`),
    /// so the engine diverges from legacy here and is pinned against the
    /// Asciidoctor 2.0.23 reference instead.
    #[test]
    fn unclosed_plus_run_reparses_as_single_plus_matches_asciidoctor() {
        // `+++` → single-plus with content `+` (asciidoctor renders `+`).
        assert_eq!(pipeline("+++"), vec![Event::Text("+".into())]);
        // `+x++` → `+x+` passthrough (`x`) then a literal trailing `+`.
        assert_eq!(
            pipeline("+x++"),
            vec![Event::Text("x".into()), Event::Text("+".into())]
        );
        // `+text++more` → `text` then `+more`.
        assert_eq!(
            pipeline("+text++more"),
            vec![Event::Text("text".into()), Event::Text("+more".into())]
        );
        // `note +++ here` → the run reparses to a single `+`.
        assert_eq!(
            pipeline("note +++ here"),
            vec![
                Event::Text("note ".into()),
                Event::Text("+".into()),
                Event::Text(" here".into()),
            ]
        );
        // The frontier mdbasics line: `+*+`/`+++`/`+-+` each a single-plus form
        // (asciidoctor `(*, +, and -)`).
        assert_eq!(
            pipeline("(+*+, +++, and +-+)"),
            vec![
                Event::Text("(".into()),
                Event::Text("*".into()),
                Event::Text(", ".into()),
                Event::Text("+".into()),
                Event::Text(", and ".into()),
                Event::Text("-".into()),
                Event::Text(")".into()),
            ]
        );
        // Regression guard: a single-plus span must NOT claim a `+` that belongs
        // to a real `++…++`/`+++…+++` passthrough — the leading `+x` stays literal.
        assert_eq!(
            pipeline("+x ++y++"),
            vec![Event::Text("+x ".into()), Event::Text("y".into())]
        );
    }

    /// With the character-reference survival pass (and the `\&#…;` escape) ported,
    /// the pipeline must reproduce legacy on valid references (kept as raw
    /// `InlinePassthrough`), invalid `&`s (left as escaped `Text`), references
    /// inside quote spans, and the escaped form (dropped backslash, escaped
    /// `Text`). Extraction runs before quotes, so a reference is opaque to the
    /// surrounding span — but the `#`-bearing decimal/hex forms are kept away from
    /// the `#` mark marker in these cases (where legacy and the engine agree).
    #[test]
    fn reproduces_legacy_on_char_ref_inputs() {
        let cases = [
            // bare valid references: named, decimal, hex (survival → passthrough)
            "&#167;",
            "&copy;",
            "&#x2026;",
            "&amp;",
            "&#8217;",
            // references mixed with surrounding text
            "see &#167; here",
            "a&#167;b",
            "&#167; and &copy; both",
            // invalid → stays escaped Text (too few digits/letters, no `;`, bare `&`)
            "&#1;",
            "&a;",
            "&foo",
            "Tom & Jerry",
            "plain & text",
            // references inside `*`/`` ` ``/`_` spans (markers that do not collide
            // with the `#` inside a decimal/hex reference)
            "*&#167;*",
            "`&#167;`",
            "_&copy;_",
            "see *&#167;* and `&#x2026;`",
            // escaped reference: backslash drops, reference becomes escaped Text
            "\\&#174;",
            "x\\&copy;y",
            "\\&#x2026;",
            "say \\&#167; not &#167;",
            // reference beside a replacement (em-dash / apostrophe untouched)
            "&#167; -- dash",
            "don't &copy; me",
            // a backslash before an INVALID reference stays literal
            "\\&foo",
            "\\& bare",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the cross-reference macro family ported (`xref:target[label]` and the
    /// `<<target>>` / `<<target,label>>` shorthand), the pipeline must reproduce
    /// legacy on every form: the bracket-less / empty-bracket target-as-text case,
    /// the explicit (re-parsed) label, the `#`-stripped and comma-trimmed
    /// shorthand, references mixed with surrounding text and other spans, and the
    /// invalid forms that stay literal. The macro leaf is opaque to the later
    /// quote/replacement passes, so a span wrapping it reparses its content the
    /// same way legacy does.
    #[test]
    fn reproduces_legacy_on_cross_reference_inputs() {
        let cases = [
            // xref macro: empty label → target as text; explicit label → re-parsed
            "xref:target[]",
            "xref:target.adoc[]",
            "xref:ROOT:comments.adoc[]",
            "xref:a.adoc#frag[]",
            "xref:target[Some Label]",
            "xref:target[*bold* label]",
            "xref:target[a -- b]",
            // attribute reference inside the label (re-parsed with attributes on)
            "xref:target[{name}]",
            // attribute reference inside the TARGET stays literal in the target
            "xref:{anchor}[]",
            // mixed with surrounding text and a sibling span
            "see xref:there[] now",
            "*before* xref:x[y] *after*",
            "xref:a[] and xref:b[c]",
            // a span wrapping a macro reparses the same way
            "*xref:x[]*",
            "`xref:x[]`",
            // <<...>> shorthand: bare, labelled, #-stripped, trimmed (the target
            // begins with a valid `[\p{Word}#/.:{]` char, so legacy and the engine
            // agree).
            "<<id>>",
            "<<id,label>>",
            "<<#id>>",
            "<<id , the label >>",
            "<<id>> and <<other,text>>",
            "text <<id>> more",
            // three `<` cannot open a cross reference: the inner `<<` target would
            // begin with `<`, so the whole run stays literal (both engines) — bare
            // and inside a `` `…` `` monospace span (the `page-breaks.adoc` case,
            // where the span must survive so `<<<` renders as `<code>&lt;&lt;&lt;`).
            "<<<",
            "`<<<`",
            // both macro forms together
            "<<a>> then xref:b[c]",
            // invalid → stay literal (no brackets, empty target, empty cross
            // reference)
            "xref:notarget",
            "xref:[]",
            "<<>>",
            "a < b << c",
            // a non-macro 'xref' substring mid-word
            "prefixref:x[]",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
        // Asciidoctor's `InlineXrefMacroRx` rejects a `<<target>>` whose target does
        // NOT begin with `[\p{Word}#/.:{]`. The legacy parser has no such guard and
        // links these (it only reaches a top-level `<<`, never one buried inside a
        // span). With the gate removed the engine ADOPTS the Asciidoctor-faithful
        // result (no link), which is what flips `page-breaks`.
        for c in [
            "<< id , the label >>", // leading space
            "<<-y>>",               // leading dash
            "<<\"a\">>",            // leading quote
            "a <<<b>>",             // inner `<<` matches at `b` → `<` literal + `#b`
            // reversed-looking brackets: the target is non-greedy up to the first
            // `[` (Asciidoctor `xref:([\w":./]...)\[`), so `a]b` is a valid target
            // and the close `]` is escape-aware — links to `#a]b` (legacy declined).
            "xref:a]b[c]",
        ] {
            assert_eq!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()),
                Some(pipeline(c)),
                "engine should adopt the Asciidoctor-faithful result for {c:?}"
            );
        }
    }

    /// With the link macro family ported (`link:url[attrs]`, `mailto:email[attrs]`,
    /// bare URL autolinks and email autolinks), the pipeline must reproduce legacy
    /// on every form: the bare/text/named-attr link, the `^` blank-window
    /// shorthand, the `mailto:` subject/body query encoding, the bare and
    /// `[label]` autolink forms (with trailing-punctuation stripping and the
    /// left-boundary rule), the backward-scanned email autolink, references mixed
    /// with surrounding text and spans, and the invalid forms that stay literal.
    #[test]
    fn reproduces_legacy_on_link_inputs() {
        let cases = [
            // link macro: bare (text = target), explicit text, relative target
            "link:http://example.com[]",
            "link:http://example.com[Example]",
            "link:/docs/intro.html[Intro]",
            // link macro: named attrs, role, window, nofollow, `^` blank-window
            "link:http://x.com[role=external]",
            "link:http://x.com[Open,window=_blank]",
            "link:http://x.com[Open,role=red]",
            "link:http://x.com[Site,opts=nofollow]",
            "link:http://x.com[New^]",
            // link macro: attribute reference inside the (re-parsed) label
            "link:http://x.com[{name}]",
            // link macro mixed with text and wrapped by a span
            "see link:http://x.com[here] now",
            "*link:http://x.com[a]*",
            // link macro: invalid → literal
            "link:[]",
            "link:noclose",
            // mailto macro: bare, explicit text, subject, subject+body
            "mailto:a@b.com[]",
            "mailto:a@b.com[Mail me]",
            "mailto:a@b.com[Mail,My Subject]",
            "mailto:a@b.com[Mail,Subject Line,Body Text]",
            "mailto:a@b.com[Mail,role=x]",
            // bare URL autolinks, all four schemes
            "http://example.com",
            "https://example.com",
            "ftp://example.com",
            "irc://example.com",
            // autolink mixed with text, trailing punctuation, parentheses
            "see https://example.com here",
            "https://example.com.",
            "(https://example.com)",
            "visit https://example.com, then leave",
            // autolink URL[text] form (keeps trailing punctuation in the URL)
            "https://example.com[Example]",
            "https://example.com[]",
            // angle-bracketed bare URL: closed → both brackets stripped (trailing
            // punctuation kept); unclosed → declined (literal); `<url[text]>` keeps
            // its brackets; `<email>` is on a separate path and keeps its brackets
            "<https://example.com>",
            "a<https://x.org/y>z",
            "<https://x.org/y> tail",
            "(<https://x.org/y>)",
            "<https://x.org/y.>",
            "<https://x.org/y now",
            "<https://example.com[the site]>",
            "<user@example.com>",
            // not at a boundary → stays literal (mid-word scheme)
            "ahttps://example.com",
            // too-short scheme-only → literal
            "http://",
            // email autolink: simple, embedded, decline (no dot / no local part)
            "user@example.com",
            "Contact user@example.com today",
            "a.b+c@sub.example.com",
            "user@example.com.",
            "a@b",
            "@example.com",
            // email autolink beside a span and a cross reference
            "*x* user@example.com",
            "user@example.com and xref:t[]",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// Angle-bracketed bare URL (`<https://…>`), asserted against the Asciidoctor
    /// reference (exact event vector, not just `pipeline == legacy`): a closed
    /// bracket strips BOTH `<`/`>` and links the URL bare while KEEPING trailing
    /// punctuation; an unclosed `<url` declines to link (stays literal); the
    /// `<url[text]>` macro form keeps its literal brackets around the link; and the
    /// `<email>` autolink keeps its brackets too (the strip is URL-only).
    #[test]
    fn angle_bracket_url_matches_asciidoctor() {
        fn bare_link(url: &str) -> Vec<Event<'static>> {
            vec![
                Event::Start(Tag::Link {
                    url: url.to_string().into(),
                    window: None,
                    nofollow: false,
                    is_bare: true,
                    role: None,
                }),
                Event::Text(url.to_string().into()),
                Event::End(TagEnd::Link),
            ]
        }
        // closed `<url>` → both brackets gone, bare link, no surrounding Text
        assert_eq!(pipeline("<https://example.com>"), bare_link("https://example.com"));
        // trailing punctuation is KEPT inside the brackets (the `>` is the boundary)
        assert_eq!(pipeline("<https://x.org/y.>"), bare_link("https://x.org/y."));
        // mid-word `<` and trailing text are preserved either side of the link
        assert_eq!(pipeline("a<https://x.org/y>z"), {
            let mut v = vec![Event::Text("a".into())];
            v.extend(bare_link("https://x.org/y"));
            v.push(Event::Text("z".into()));
            v
        });
        // unclosed `<url` → no link at all (the whole run stays one literal Text)
        assert_eq!(
            pipeline("<https://x.org/y now"),
            vec![Event::Text("<https://x.org/y now".into())]
        );
        // `<url[text]>` → brackets stay literal around the labelled link
        assert_eq!(pipeline("<https://example.com[the site]>"), vec![
            Event::Text("<".into()),
            Event::Start(Tag::Link {
                url: "https://example.com".to_string().into(),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text("the site".into()),
            Event::End(TagEnd::Link),
            Event::Text(">".into()),
        ]);
        // `<email>` keeps its brackets (URL-only strip); the email still links
        assert_eq!(pipeline("<user@example.com>"), vec![
            Event::Text("<".into()),
            Event::Start(Tag::Link {
                url: "mailto:user@example.com".to_string().into(),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text("user@example.com".into()),
            Event::End(TagEnd::Link),
            Event::Text(">".into()),
        ]);
    }

    #[test]
    fn escaped_ellipsis_in_url_target_keeps_literal_dots() {
        // The `escape` pass seals `\...` as a `Literal("...")` before the link is
        // detected; `reconstruct_link_target` splices that backslash-stripped
        // literal back into the target, so the URL keeps literal dots (no
        // ellipsis) exactly as Asciidoctor's `replacements` pass leaves it. Before
        // this, the sentinel forced a punt to legacy, which kept the raw `\...`.
        // URL[text] form.
        assert_eq!(pipeline("https://ex.com/a\\...b[t]"), vec![
            Event::Start(Tag::Link {
                url: "https://ex.com/a...b".to_string().into(),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text("t".into()),
            Event::End(TagEnd::Link),
        ]);
        // Bare form — both the href and the visible text are the literal target.
        assert_eq!(pipeline("https://ex.com/a\\...b"), vec![
            Event::Start(Tag::Link {
                url: "https://ex.com/a...b".to_string().into(),
                window: None,
                nofollow: false,
                is_bare: true,
                role: None,
            }),
            Event::Text("https://ex.com/a...b".into()),
            Event::End(TagEnd::Link),
        ]);
        // `link:` macro form.
        assert_eq!(pipeline("link:https://ex.com/a\\...b[t]"), vec![
            Event::Start(Tag::Link {
                url: "https://ex.com/a...b".to_string().into(),
                window: None,
                nofollow: false,
                is_bare: false,
                role: None,
            }),
            Event::Text("t".into()),
            Event::End(TagEnd::Link),
        ]);
    }

    #[test]
    fn escaped_macro_prefix_file_scheme_and_anchor_id() {
        // `file://` is an autolink scheme (Asciidoctor `(?:https?|file|ftp|irc)://`):
        // a bare `file://` URL links as `class="bare"`, exactly like `ftp`/`irc`.
        assert_eq!(pipeline("file:///root"), vec![
            Event::Start(Tag::Link {
                url: "file:///root".to_string().into(),
                window: None,
                nofollow: false,
                is_bare: true,
                role: None,
            }),
            Event::Text("file:///root".into()),
            Event::End(TagEnd::Link),
        ]);
        // Escaped `\file://…` drops the backslash to plain text (no link), where an
        // unescaped autolink could open. `\file:relative` has no `://`, so it is no
        // scheme at all and the backslash stays literal.
        assert_eq!(pipeline("\\file:///root"), vec![Event::Text("file:///root".into())]);
        assert_eq!(pipeline("\\file:relative"), vec![Event::Text("\\file:relative".into())]);

        // `anchor:` requires a valid id (`InlineAnchorRx`): a valid id is an anchor…
        assert_eq!(pipeline("anchor:my-id[t]"), vec![
            Event::Start(Tag::Anchor { id: "my-id".to_string().into(), label: Some("t".into()) }),
            Event::End(TagEnd::Anchor),
        ]);
        // …while an invalid id (`<id>`, a leading digit, or `#`) is no macro at all,
        // so the whole form stays literal text.
        assert_eq!(pipeline("anchor:<id>[t]"), vec![Event::Text("anchor:<id>[t]".into())]);
        assert_eq!(pipeline("anchor:1abc[t]"), vec![Event::Text("anchor:1abc[t]".into())]);
        // An escaped `\anchor:` with a VALID id drops the backslash (escaped macro)…
        assert_eq!(pipeline("\\anchor:myid[t]"), vec![Event::Text("anchor:myid[t]".into())]);
        // …but with an INVALID id the construct never matches, so the `\\?` capture
        // never engages and the backslash stays literal (Asciidoctor parity).
        assert_eq!(pipeline("\\anchor:<id>[t]"), vec![Event::Text("\\anchor:<id>[t]".into())]);

        // The default engine and the legacy fallback agree on every form above.
        for c in [
            "file:///root", "\\file:///root", "\\file:relative", "anchor:my-id[t]",
            "anchor:<id>[t]", "anchor:1abc[t]", "\\anchor:myid[t]", "\\anchor:<id>[t]",
        ] {
            assert_eq!(pipeline(c), legacy(c), "engine mismatch on {c:?}");
        }
    }

    /// With the inline-image macro ported, the pipeline must reproduce legacy on
    /// the `image:target[attrs]` forms: bare (filename-derived alt), explicit alt,
    /// positional width/height, named attrs (width/height/align/float/link/role/
    /// title), quoted alt, an attribute-reference target left literal (attributes
    /// run after macros), the `image::` block form left untouched, the invalid
    /// forms that stay literal, and mixes with surrounding text and spans.
    #[test]
    fn reproduces_legacy_on_image_inputs() {
        let cases = [
            // bare: alt auto-generated from the filename by the renderer
            "image:play.png[]",
            "Click image:play.png[] to get the party started.",
            // explicit alt (positional 0), quoted alt
            "image:play.png[Play]",
            "image:play.png[\"Play button\"]",
            // positional width/height
            "image:play.png[Play,200,100]",
            // named attrs
            "image:play.png[title=Pause]",
            "image:play.png[alt=Go,width=50]",
            "image:play.png[align=center]",
            "image:play.png[float=left]",
            "image:play.png[link=https://example.com]",
            "image:play.png[role=icon]",
            // path-style target (imagesdir-relative), left literal in the target
            "image:macros:play.png[]",
            "image:{imagesdir}/play.png[Go]",
            // block form must NOT be treated as an inline image
            "image::play.png[]",
            // invalid → literal
            "image:noclose",
            "image:[]",
            // mixed with text and wrapped by a span
            "see image:a.png[A] and image:b.png[B]",
            "*image:a.png[A]*",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// A passthrough inside a verbatim leaf macro's content (image alt/target,
    /// stem, kbd/btn label, index term) is no longer punted: [`macros`] restores
    /// the passthrough's protected content into the verbatim attribute, so the
    /// engine forms the macro natively. Asserted on the engine's own events here
    /// (legacy diverges — it kept the raw `++…++` markers); the rendered HTML
    /// matches Asciidoctor, covered in `adoc-html`.
    #[test]
    fn verbatim_macro_passthrough_reconstructed_natively() {
        // image alt: the protected `a b` (space intact) reaches the alt field.
        let img = pipeline("image:i.png[++a b++]");
        assert!(
            matches!(
                img.as_slice(),
                [Event::Start(Tag::InlineImage { alt, .. }), Event::End(TagEnd::InlineImage)]
                    if alt.as_ref() == "a b"
            ),
            "{img:?}"
        );
        // stem content restored verbatim (no `++` markers).
        assert_eq!(
            pipeline("stem:[++x++]"),
            vec![
                Event::Start(Tag::Stem { variant: std::borrow::Cow::Borrowed("stem") }),
                Event::Text(std::borrow::Cow::Borrowed("x")),
                Event::End(TagEnd::Stem),
            ]
        );
        // kbd content `Ctrl` is one verbatim key (legacy mangled `++Ctrl++` on `+`).
        assert_eq!(
            pipeline_exp("kbd:[++Ctrl++]"),
            vec![
                Event::Start(Tag::Keyboard),
                Event::Text(std::borrow::Cow::Borrowed("Ctrl")),
                Event::End(TagEnd::Keyboard),
            ]
        );
        // A SURVIVED char-ref inside a verbatim macro is now spliced natively (was
        // a punt): the reference reaches the content intact, and the engine
        // reproduces legacy exactly (legacy also keeps the literal reference here).
        assert_eq!(
            pipeline("stem:[caf&#233;]"),
            vec![
                Event::Start(Tag::Stem { variant: std::borrow::Cow::Borrowed("stem") }),
                Event::Text(std::borrow::Cow::Borrowed("caf&#233;")),
                Event::End(TagEnd::Stem),
            ]
        );
        assert_eq!(
            pipeline("stem:[caf&#233;]"),
            legacy("stem:[caf&#233;]"),
            "native char-ref splice must reproduce legacy events"
        );
        // An ESCAPED char-ref (`\&#…;`, raw:false) still punts (its verbatim-vs-
        // escaped treatment is family-specific — DEFERRED) → engine declines.
        assert!(
            try_parse("stem:[caf\\&#233;]", SubstitutionSet::NORMAL, InlineOptions::default())
                .is_none()
        );
    }

    /// A SURVIVED character reference in any verbatim macro content/target is now
    /// spliced natively rather than punted, and the engine reproduces legacy
    /// EXACTLY (both keep the literal reference in the verbatim string — the
    /// renderer decides per family whether to preserve or re-escape it, covered in
    /// `adoc-html`). Mirrors the per-construct differential of the other ported
    /// families. The escaped `\&#…;` form is left punting (DEFERRED), so it is not
    /// asserted here.
    #[test]
    fn native_char_ref_in_verbatim_macros_reproduces_legacy() {
        // Families recognised under NORMAL subs (no `:experimental:` needed).
        let normal = [
            "image:i.png[caf&#233;]",      // alt
            "image:a&#167;b.png[alt]",     // target
            "icon:tags[caf&#233;]",        // icon attrs → class
            "indexterm2:[caf&#233;]",      // flow term, rendered in place
            "stem:[caf&#233;]",            // math content
            "(((caf&#233;)))",             // concealed index term
        ];
        for c in normal {
            assert!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()).is_some(),
                "engine should form {c:?} natively (no char-ref punt)"
            );
            assert_eq!(pipeline(c), legacy(c), "engine diverged from legacy for {c:?}");
        }
        // UI macros need `:experimental:`.
        let experimental = [
            "kbd:[caf&#233;]",
            "btn:[caf&#233;]",
            "menu:File[Save As&#8230;]",
            "menu:F&#167;X[Item]",
        ];
        for c in experimental {
            assert_eq!(
                pipeline_exp(c),
                legacy_exp(c),
                "engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the leaf macros ported (`icon:name[attrs]` and the STEM family
    /// `stem:[…]` / `latexmath:[…]` / `asciimath:[…]`), the pipeline must reproduce
    /// legacy on every form: the bare and attrlist-bearing icon, the three STEM
    /// spellings (empty and non-empty content), the `\]` escape inside STEM
    /// content, the invalid forms that stay literal, and mixes with surrounding
    /// text and spans. Each leaf is opaque to the later quote/replacement passes,
    /// so neither the icon attrlist nor the math content is re-substituted.
    #[test]
    fn reproduces_legacy_on_leaf_macro_inputs() {
        let cases = [
            // icon: bare, attrlist (positional + named), mixed with text
            "icon:heart[]",
            "icon:heart[2x]",
            "icon:heart[2x,role=red]",
            "icon:tags[role=blue] ruby",
            "see icon:a[] and icon:b[fw] here",
            // icon: invalid → literal (no brackets, empty name)
            "icon:noclose",
            "icon:[]",
            // STEM: all three spellings, empty and non-empty content
            "stem:[]",
            "stem:[x^2]",
            "latexmath:[\\sqrt{a}]",
            "asciimath:[sqrt(b)]",
            // STEM: `\]` escape inside content (does not close, unescaped to `]`)
            "stem:[[a,b\\],[c,d\\]\\]]",
            "latexmath:[f(x\\])]",
            // STEM: no closing bracket → literal
            "stem:[unterminated",
            // leaf macros mixed with text and wrapped by a span
            "before stem:[x] after",
            "*icon:a[]*",
            "`stem:[x^2]`",
            // these macros have no left-boundary rule, so a prefix still matches
            // mid-word — the engine must match at the same offset legacy does
            "prefixicon:x[]",
            "myasciimath:[x]",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
        // Reversed-looking brackets: the icon name is non-greedy up to the first
        // `[` (Asciidoctor `i(?:mage|con):([^:\s\[]...)\[`, name may contain `]`)
        // and the close `]` is escape-aware, so `icon:a]b[c]` is an icon named
        // `a]b` with attr `c` (legacy declined → literal). The engine adopts the
        // Asciidoctor-faithful result.
        assert_eq!(
            try_parse("icon:a]b[c]", SubstitutionSet::NORMAL, InlineOptions::default()),
            Some(pipeline("icon:a]b[c]")),
            "engine should adopt the Asciidoctor-faithful icon for reversed brackets"
        );
    }

    /// With the anchor family ported (`[[id]]` / `[[id,label]]`, the `[[[id]]]`
    /// bibliography form, and the `anchor:id[label]` macro), the pipeline must
    /// reproduce legacy on every form: the bare and labelled anchor (comma-split,
    /// trimmed, empty-label dropped), the bibliography anchor (empty label kept as
    /// `Some`), the anchor macro (whitespace target rejected), references mixed with
    /// surrounding text and spans, and the invalid forms that stay literal. Each
    /// leaf is opaque to the later quote/replacement passes.
    #[test]
    fn reproduces_legacy_on_anchor_inputs() {
        let cases = [
            // plain anchor: bare, labelled (trimmed), empty-label dropped
            "[[anchor-id]]",
            "[[id,Reference Text]]",
            "[[id , spaced label ]]",
            "[[id,]]",
            "see [[here]] now",
            // bibliography anchor: bare, labelled, empty label kept
            "[[[biblio-ref]]]",
            "[[[ref, Display Label]]]",
            "[[[ref,]]]",
            "[[[ Knuth1984 ]]]",
            // anchor macro: bare, labelled, whitespace target → literal
            "anchor:my-id[]",
            "anchor:my-id[xreflabel]",
            "anchor:bad id[x]",
            // anchors beside / inside spans and mixed with text
            "*x* [[a]] and [[b,c]]",
            "before anchor:t[] after",
            "[[a]]text",
            // invalid → literal (unclosed, empty, single bracket left to quotes)
            "[[unclosed",
            "[[]]",
            "[[[unclosed]]",
            "anchor:noclose",
            "[single]",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the index-term family ported (the `((term))` flow / `(((p, s)))`
    /// concealed shorthand, the `indexterm:[…]` concealed macro, and the
    /// `indexterm2:[term]` flow macro), the pipeline must reproduce legacy on every
    /// form: the four shorthand shapes decided by enclosing parens (a one-sided
    /// paren splits off a literal `Text`), the non-greedy `))` close that slides
    /// over a trailing `)`, the comma-split concealed terms (up to three), and the
    /// invalid forms that stay literal.
    #[test]
    fn reproduces_legacy_on_index_term_inputs() {
        let cases = [
            // flow shorthand: plain term
            "((tigers))",
            "see ((big cats)) here",
            // concealed shorthand: both parens, comma-split
            "(((Big cats, Tigers)))",
            "(((a, b, c)))",
            "((( spaced , terms )))",
            // one-sided paren → literal paren beside a flow term
            "(((leading)",
            "((trailing)))",
            // non-greedy close slides over a trailing `)`
            "((a)))",
            // concealed macro and flow macro
            "indexterm:[Big cats, Tigers]",
            "indexterm:[single]",
            "indexterm2:[flow term]",
            // mixed with surrounding text
            "a ((term)) b",
            "x indexterm:[y] z",
            // invalid → literal (no close, empty, single paren left alone)
            "((unclosed",
            "(())",
            "indexterm:[]",
            "indexterm2:[]",
            "indexterm:noclose",
            "(single paren)",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the escaped inline-macro form ported (`\name:target[…]` for each of the
    /// twelve macro names), the pipeline must reproduce legacy: the backslash drops
    /// and the whole macro form stays literal as its OWN `Text` event (so trailing
    /// text does NOT coalesce with it), the `macros` pass never fires on it, the
    /// block-macro `\image::…` is rejected (backslash stays), an unbracketed form is
    /// not an escape, and the escape applies mid-word and inside a span.
    #[test]
    fn reproduces_legacy_on_macro_escape_inputs() {
        let cases = [
            // each macro name escaped → dropped backslash, literal macro form
            "\\link:http://x.com[Site]",
            "\\xref:target[label]",
            "\\mailto:a@b.com[Mail]",
            "\\image:play.png[Play]",
            "\\icon:heart[2x]",
            "\\stem:[x^2]",
            "\\latexmath:[\\sqrt a]",
            "\\asciimath:[sqrt b]",
            "\\indexterm2:[primary]",
            "\\indexterm:[primary, secondary, tertiary]",
            "\\footnote:[a note]",
            "\\anchor:my-id[]",
            // the escaped macro is its OWN Text event — trailing/leading text does
            // not coalesce with it (two/three separate Text events)
            "\\link:u[t] more",
            "before \\indexterm2:[primary] after",
            // bare (nothing after) → one Text event
            "\\link:u[t]",
            // content that looks like an attribute ref stays literal inside the form
            "\\link:u[{name}]",
            // mid-word: the backslash is the boundary, the macro form still matches
            "word\\link:u[t]",
            "see \\image:a.png[A] now",
            // escaped macro inside a span (only-content-in-span)
            "`\\indexterm2:[primary]`",
            "*\\link:u[t]*",
            // NOT an escape: block-macro `::` form (backslash stays literal)
            "\\image::play.png[]",
            // NOT an escape: no closing bracket (backslash stays, macro declines too)
            "\\link:noclose",
            "\\xref:notarget",
            // a non-macro name after the backslash is unaffected (blanket arm)
            "\\notamacro:x[y]",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the escaped pass macro `\pass:SPEC[…]` folded into the passthrough
    /// pass, the pipeline must reproduce legacy: the backslash drops and the
    /// `pass:SPEC[` prefix stays literal while the bracketed content flows through
    /// the remaining substitutions (it is NOT extracted as a verbatim
    /// passthrough). An unbracketed `\pass:` is not an escape (the backslash
    /// stays). The escape is what stops the bare-`pass:[…]` arm from lifting the
    /// whole macro into a sentinel and leaving the lone backslash behind.
    ///
    /// Cases keep the escape at a flush boundary (input start or a span edge),
    /// where the legacy parser's `flush_text` at the backslash produces no empty
    /// split, so the event vectors match exactly. A bare `\pass:` mid-run after
    /// other text (`before \pass:[x]`) renders identically but splits the text one
    /// event differently from legacy (the engine adopts its own, still-correct
    /// split), so it is excluded from these byte-equality cases. The `\\pass:`
    /// double-backslash form is likewise excluded.
    #[test]
    fn reproduces_legacy_on_pass_escape_inputs() {
        let cases = [
            // bare and spec'd, empty content → literal `pass:SPEC[]`
            "\\pass:[]",
            "\\pass:c[]",
            "\\pass:q[]",
            // content flows through the remaining subs (quotes / specialchars),
            // and the `pass:SPEC[` prefix shares the `*`/text flush boundary
            "\\pass:c[*b*]",
            "\\pass:[<raw>]",
            "\\pass:q[<x> & y]",
            "\\pass:[plain text]",
            // the corpus pattern: escaped pass macro inside a monospace span (the
            // span edge is the flush boundary, so the prefix matches legacy)
            "`\\pass:[]`",
            "`\\pass:c[]`",
            "the `\\pass:[]` macro",
            "shorthand for the `\\pass:c[]` enclosure",
            // NOT an escape: no opening bracket → backslash stays literal
            "\\pass:nobracket",
            "\\pass:c",
            // a non-`pass:` name after the backslash is unaffected here
            "\\passenger[x]",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the escaped autolink `\http://…` folded into the `macros` pass, the
    /// pipeline must reproduce legacy: the backslash drops and the URL stays
    /// literal text where an unescaped autolink could open — at a real boundary or
    /// immediately inside a constrained quote span that opens here. Without such a
    /// boundary (`word\http…`, `a*\http…`, `` a`\http… ``, `\httpx://…`) the
    /// backslash stays literal.
    ///
    /// Cases keep the escape at a flush boundary (input start or a span edge): the
    /// legacy parser flushes text AT the backslash, so a mid-run bare escape
    /// (`before \http://x`) splits the text one event differently (the URL flows
    /// into a fresh run) while the flat engine merges it into one Text — the engine
    /// adopts its own (HTML-identical) split, so that case is excluded from these
    /// byte-equality cases. The `\\http://…` double-backslash form (legacy drops
    /// one backslash, Asciidoctor keeps both) is likewise excluded.
    #[test]
    fn reproduces_legacy_on_autolink_escape_inputs() {
        let cases = [
            // bare at input start (flush boundary), every scheme + a query string
            "\\https://example.org",
            "\\http://example.org/x?source=home",
            "\\ftp://files.example.org",
            "\\irc://chat.example.org",
            // the corpus pattern: escaped autolink inside a monospace span (the
            // span edge is the flush boundary, so the drop matches legacy)
            "`\\https://example.org/x?source=home`",
            "the `\\https://example.org` link",
            // inside other constrained spans that open at the marker
            "*\\https://example.org*",
            "_\\https://example.org_",
            "#\\https://example.org#",
            // NOT a drop: a marker that opens no span keeps the backslash literal
            "a*\\https://example.org",
            "a`\\https://example.org",
            // NOT a drop: a word boundary before the backslash
            "word\\https://example.org",
            // NOT an autolink scheme → backslash stays literal (blanket arm)
            "\\httpx://example.org",
            "\\hello world",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// A bare URL immediately inside a constrained span autolinks, exactly as
    /// Asciidoctor links it after `quotes` materialises the `<code>`/`<strong>`/…
    /// wrapper and the `macros` pass sees the `>`/`<` tag boundaries. Because the
    /// engine runs `macros` *before* `quotes`, the still-literal opening marker is
    /// the left boundary ([`super::macros::autolink_url_limit`]) and the
    /// still-literal closing marker caps the URL scan (the pre-`quotes` stand-in
    /// for the `<` of `</code>`). The legacy parser reaches the same result by
    /// recursively re-parsing the span content, so the pipeline reproduces it.
    #[test]
    fn reproduces_legacy_on_bare_autolink_in_span_inputs() {
        let cases = [
            // the corpus pattern (monitoring.adoc): a URL is the whole monospace
            // span, trailing sentence punctuation outside the span
            "See `http://localhost:8080/actuator`.",
            // every constrained marker that opens a span: the URL links inside it
            "m `http://example.com/a` x",
            "b *http://example.com/b* x",
            "i _http://example.com/c_ x",
            "k #http://example.com/d# x",
            // superscript / subscript simple pairs
            "s ^http://example.com/e^ x",
            "z ~http://example.com/f~ x",
            // URL mid-span (preceded by a space → plain boundary, capped by close)
            "`see http://example.com/x here`",
            // trailing punctuation inside the span is stripped from the bare URL
            "`http://example.com/y.`",
            // NOT a span (marker mid-word opens nothing) → no autolink, literal
            "word`http://example.com/z` x",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// The spec'd pass macro `pass:SPEC[…]` re-runs exactly its spec'd
    /// substitutions over the bracketed content and seals the result as one opaque
    /// leaf (so the later passes cannot reach inside). The pipeline must reproduce
    /// the legacy `try_pass_macro` / `push_pass_spec_content`: `q`→quotes,
    /// `c`→specialchars (text stays `Text`, html-escaped), `a`→attributes
    /// (`{name}` → an `AttributeReference` leaf), `macros`→autolink, the full
    /// `quotes`/`normal` names, comma-combined specs, and the empty-content form
    /// (`pass:q[]` emits nothing). Unlike the escaped `\pass:` form, the spec'd
    /// macro inserts a sentinel where the legacy parser flushes text, so a mid-run
    /// macro splits the surrounding text identically — mid-run and in-span cases
    /// match too.
    #[test]
    fn reproduces_legacy_on_pass_spec_macro_inputs() {
        let cases = [
            // `q` (quotes): the corpus table-cols pattern, bare and in-monospace
            "pass:q[#e#]",
            "the `[cols=\">pass:q[#e#],.^3pass:q[#s#]\"]` style",
            "`[cols=\"pass:q[#h#],pass:q[#e#]\"]`",
            "`pass:q[#h#]`",
            "`[cols=\"2,pass:q[#^#]1\"]`",
            // `q` over raw HTML with an inner strong (no specialchars → raw text)
            "the text pass:q[<del>strike *this*</del>] is deleted",
            // `q,a`: quotes + attributes, an attribute reference inside
            "pass:q,a[<del>strike _{docname}_</del>]",
            // full name spec
            "pass:quotes[But I should contain *bold* text.]",
            // `macros`: an autolink, with `__` left literal (no quotes)
            "pass:macros[https://asciidoctor.org/now_this__link_works.html]",
            // `c,a`: specialchars keeps text as escaped `Text`, attributes active
            "pass:c,a[__<{email}>__]",
            // `r` / `n`: replacements / normal (spaced em-dash, not an edge case)
            "rep pass:r[a -- b (C) x...] end",
            "norm pass:n[*b* -- _i_] end",
            // empty content emits nothing (both specs)
            "pass:q[]",
            "pass:c[]",
            "before pass:q[] after",
            // mid-run flush boundary (the sentinel splits text like the flush)
            "the text pass:q[#x#] here",
            // the bare form is unchanged (verbatim leaf) — regression guard
            "pass:[verbatim *x*]",
            // no opening bracket → not a macro, the text stays literal
            "pass:q nobracket",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the passthrough-protected link target ported, the pipeline must
    /// reproduce legacy on the `link:++url++[…]` forms: the URL is extracted as a
    /// passthrough leaf first, then the link macro reconstructs it. Covers the
    /// corpus cases (special chars `[a b]` in the URL, repeating `__`), bare and
    /// explicit-label forms, a plain-link regression guard, and surrounding text.
    /// A passthrough/escape/char-ref sentinel in the *label* is no longer a punt:
    /// the label is re-parsed by a *seeded* sub-pipeline ([`run_pipeline_seeded`])
    /// so the sentinel resolves against the outer table, and the result matches
    /// legacy's `push_macro_label` byte-for-byte — the equality loop below covers
    /// those formerly-punted forms directly (gate-neutral by construction).
    #[test]
    fn reproduces_legacy_on_link_passthrough_url_inputs() {
        let cases = [
            // corpus: special characters in the URL, explicit label
            "link:++https://example.org/?q=[a b]++[URL with special characters]",
            // corpus: repeating underscores, bare (text defaults to the target)
            "link:++https://example.org/now_this__link_works.html++[]",
            "For example, link:++https://example.org/now_this__link_works.html++[].",
            // protected space, explicit label and bare
            "link:++http://x.com/a b++[Spaced]",
            "link:++http://x.com/a b++[]",
            // plain link with no passthrough → unchanged (regression guard)
            "link:http://example.com[Example]",
            "see link:http://x.com[here] now",
            // surrounded by a span
            "*link:++http://x.com/a b++[a]*",
            // NATIVE seeded label re-parse (formerly punted): a passthrough in the
            // LABEL of the link/mailto/autolink families now matches legacy's event
            // stream exactly (these tags carry no raw-text field, only label events).
            "link:http://x.com[++raw__text++]",
            "link:http://x.com[a ++raw__b++ c]",
            "link:http://x.com[*bold* ++raw++]",
            "mailto:a@b.com[++raw__text++]",
            "https://example.org/page[++raw__text++]",
            // escape + char-ref sentinels in the label re-parse natively too
            "link:http://x.com[\\*not bold* x]",
            "link:http://x.com[caf&#233; ++r++]",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// Replace every `CrossReference` label field with `Some("")` (presence
    /// preserved) so two event streams can be compared modulo that field. The
    /// renderer reads only `label.is_none()`, never the field's content, so a
    /// difference there is render-dead — see [`reproduces_legacy_on_xref_label_seeded`].
    fn strip_xref_label(mut events: Vec<Event<'_>>) -> Vec<Event<'_>> {
        for e in &mut events {
            if let Event::Start(Tag::CrossReference { label: Some(l), .. }) = e {
                *l = std::borrow::Cow::Borrowed("");
            }
        }
        events
    }

    /// A passthrough / escape / char-ref sentinel in a cross-reference label is
    /// re-parsed natively (seeded), like the link family. Unlike a link, the
    /// `CrossReference` tag also stores the label *text* in a field — legacy keeps
    /// the raw pre-substitution source (`++raw__text++`) there, while the engine,
    /// having already substituted, can only restore the passthrough *content*
    /// (`raw__text`; the `+` marker count is lost at extraction). That field is
    /// render-dead (the renderer reads only `label.is_none()`), so the streams are
    /// asserted equal modulo it — the label *events* (what actually renders) match
    /// legacy exactly, which the gate/frontier HTML comparison confirms end to end.
    #[test]
    fn reproduces_legacy_on_xref_label_seeded() {
        let cases = [
            "xref:tgt[++raw__text++]",
            "xref:tgt[lead ++raw++ tail]",
            "xref:tgt[*bold* ++raw++]",
            "<<tgt,++raw__text++>>",
            "<<tgt,lead ++raw++ tail>>",
            "xref:tgt[\\*not bold* x]",
            "xref:tgt[caf&#233; ++r++]",
        ];
        for c in cases {
            assert_eq!(
                strip_xref_label(pipeline(c)),
                strip_xref_label(legacy(c)),
                "new engine diverged from legacy (modulo render-dead label field) for {c:?}"
            );
        }
    }

    /// A survived character reference in a link/autolink URL (`link:a&#167;b[t]`,
    /// `http://a&#167;b.com`) is now reconstructed natively — the engine no longer
    /// punts to legacy. Asciidoctor keeps the reference as an already-formed entity
    /// in the `href`; the renderer's href escape (adoc-html) preserves it. The
    /// end-to-end `href`/`alt` parity is asserted in the html crate's
    /// `test_char_ref_in_link_url_href` &c.; this pins the parser behaviour.
    #[test]
    fn native_char_ref_in_link_url() {
        // No longer declines: every form is handled by the engine itself.
        for inp in [
            "link:a&#167;b[text]",
            "link:a&#167;b[]",
            "http://a&#167;b.com",
            "link:My&#32;Documents/r.pdf[Get]",
            "link:a&copy;b[t]",
        ] {
            assert!(
                try_parse(inp, SubstitutionSet::NORMAL, InlineOptions::default()).is_some(),
                "engine must handle a char-ref in the URL natively (no legacy punt): {inp:?}"
            );
        }

        // Explicit-text form: the URL string carries the entity exactly as legacy
        // produces it, so the event streams are identical — the visible divergence
        // is purely in the (shared) renderer's href escape.
        assert_eq!(pipeline("link:a&#167;b[text]"), legacy("link:a&#167;b[text]"));

        // Bare form: the engine INTENTIONALLY diverges from legacy, segmenting the
        // visible text so the reference rides through as its own `InlinePassthrough`
        // (renderer-verbatim, matching Asciidoctor); legacy emits one escaped `Text`.
        let bare = pipeline("link:a&#167;b[]");
        assert_ne!(bare, legacy("link:a&#167;b[]"));
        assert!(
            bare.iter()
                .any(|e| matches!(e, Event::InlinePassthrough(s) if s == "&#167;")),
            "bare-link visible text must carry the reference as a passthrough segment: {bare:?}"
        );

        // A bare `&` (not a valid reference) is NOT segmented — single `Text`, so the
        // renderer still escapes it to `&amp;`. The engine matches legacy here.
        assert_eq!(pipeline("link:a?x=1&y=2[]"), legacy("link:a?x=1&y=2[]"));
    }

    /// The `\((…))` index-term-shorthand escape and the `\\MM…MM` doubled-marker
    /// escape (subs.adoc lines 20 and 27): the engine reproduces legacy's event
    /// stream here and renders the literal `((…))` / `__…__` Asciidoctor emits.
    #[test]
    fn reproduces_legacy_on_index_and_doubled_marker_escape_inputs() {
        let cases = [
            // corpus: escaped non-concealed index term — whole `((…))` literal
            "\\((DD AND CC) OR (DD AND EE)) is not interpreted as a flow index term.",
            "\\((Two Words)) plain.",
            "pre \\((x)) post",
            // escaped concealed `\(((…)))` — literal parens around a flow term
            "\\(((primary, secondary))) text",
            // no closing `))` ahead → the backslash stays literal (no escape forms)
            "\\((no close here",
            "\\(( only one open ) here",
            // corpus: doubled-marker escape — `__func__` literal, marks kept
            "The text \\\\__func__ will appear with two underscores",
            "\\\\__func__",
            "lead \\\\**bold marks** tail",
            "\\\\##hi## there",
            "\\\\``code`` here",
            // doubled marker whose content still receives substitutions
            "\\\\__a*b*c__",
            "x \\\\__ *b* __ y",
            // regression guards: the unescaped forms are unchanged
            "((Two Words)) plain index term",
            "__func__ stays emphasised",
            "**bold** and ((term))",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
    }

    /// With the footnote macro ported, the pipeline must reproduce legacy on every
    /// form: the anonymous `footnote:[text]`, the named `footnote:id[text]`, the
    /// reference `footnote:id[]` (named + empty), the anonymous-but-empty
    /// `footnote:[]` (a definition with empty text, *not* a reference), the
    /// first-`]` content scan (a nested `[` stays literal in the text), footnotes
    /// mixed with surrounding text and wrapped by a span, raw (un-re-parsed) quote
    /// markers in the text, and the invalid forms that stay literal.
    #[test]
    fn reproduces_legacy_on_footnote_inputs() {
        let cases = [
            // anonymous / named definitions and the named reference
            "footnote:[A clarification.]",
            "footnote:disclaimer[Opinions are my own.]",
            "footnote:disclaimer[]",
            // anonymous + empty content is a definition with empty text, not a ref
            "footnote:[]",
            // first `]` ends the content; a nested `[` stays literal in the text
            "footnote:[a [nested bracket]",
            // raw quote markers in the text are NOT re-parsed (sealed before quotes)
            "footnote:[note with _em_ and *strong* markers]",
            // mixed with surrounding text and inside a span
            "A bold statement!footnote:disclaimer[Opinions are my own.]",
            "see footnote:[one] and footnote:two[x] done",
            "*footnote:[bold note]*",
            "`footnote:[mono note]`",
            // invalid → literal (no bracket, no close, empty rest, empty id)
            "footnote:noclose",
            "footnote:[unclosed",
            "footnote:",
            "footnote:[id with no bracket close",
            // a prefix still matches mid-word (no left-boundary rule), at legacy's offset
            "prefixfootnote:[x]",
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }

        // The leaf shape: a named empty macro is a `FootnoteRef`; everything else is
        // a `Footnote` (anonymous → `id: None`).
        assert_eq!(
            pipeline("footnote:[hi]"),
            vec![Event::Footnote { id: None, text: "hi".into() }]
        );
        assert_eq!(
            pipeline("footnote:fn1[hi]"),
            vec![Event::Footnote { id: Some("fn1".into()), text: "hi".into() }]
        );
        assert_eq!(
            pipeline("footnote:fn1[]"),
            vec![Event::FootnoteRef { id: "fn1".into() }]
        );
    }

    /// A footnote body that carried a passthrough/escape sentinel is parsed
    /// natively into [`Event::FootnoteParsed`] (pre-parsed events) instead of
    /// punting to legacy. The renderer re-parses the raw text of a plain
    /// [`Event::Footnote`], so a body whose passthrough markers were already lifted
    /// must be parsed *here* — before the markers are lost — and handed over as
    /// finished events. A sentinel-free body keeps the raw-text `Event::Footnote`
    /// (the common case is unchanged); a marker escape (`\*`) is deferred into the
    /// quotes pass and so leaves no sentinel, also staying on the common path.
    #[test]
    fn footnote_with_sentinel_body_parses_natively() {
        // Double-plus passthrough → escaped `Text` leaf; the renderer emits it
        // literally instead of re-substituting `__x__` into emphasis (the bug the
        // old punt avoided, now handled without falling back to legacy).
        assert_eq!(
            pipeline("footnote:[++__x__++]"),
            vec![Event::FootnoteParsed {
                id: None,
                events: vec![Event::Text("__x__".into())],
            }]
        );
        // Triple-plus passthrough → raw `InlinePassthrough` leaf (verbatim HTML).
        assert_eq!(
            pipeline("footnote:[+++<b>raw</b>+++]"),
            vec![Event::FootnoteParsed {
                id: None,
                events: vec![Event::InlinePassthrough("<b>raw</b>".into())],
            }]
        );
        // Named definition with a passthrough body.
        assert_eq!(
            pipeline("footnote:fn1[++raw++]"),
            vec![Event::FootnoteParsed {
                id: Some("fn1".into()),
                events: vec![Event::Text("raw".into())],
            }]
        );
        // A `pass:[…]` macro inside the body is extracted as a passthrough leaf
        // before the footnote forms; the seeded re-parse restores it as raw HTML
        // followed by the trailing text — what Asciidoctor's global restore yields.
        // (Legacy mangled this to a literal `pass:[…]`; this is a genuine fix.)
        assert_eq!(
            pipeline("footnote:[pass:[<b>x</b>] y]"),
            vec![Event::FootnoteParsed {
                id: None,
                events: vec![
                    Event::InlinePassthrough("<b>x</b>".into()),
                    Event::Text(" y".into()),
                ],
            }]
        );
        // A typographic escape (`\--`, `\(C)`) IS sealed by the escape pass into a
        // `Literal`, so it too travels as `FootnoteParsed`; the backslash is gone.
        match &pipeline("footnote:[a \\-- b \\(C) c]")[..] {
            [Event::FootnoteParsed { id: None, events }] => {
                let text: String = events
                    .iter()
                    .map(|e| match e {
                        Event::Text(t) | Event::InlinePassthrough(t) => t.as_ref(),
                        _ => "",
                    })
                    .collect();
                assert_eq!(text, "a -- b (C) c");
            }
            other => panic!("expected FootnoteParsed, got {other:?}"),
        }

        // Common case — a sentinel-free body — is unchanged: raw text on
        // `Event::Footnote`, re-parsed by the renderer.
        assert_eq!(
            pipeline("footnote:[plain _em_ text]"),
            vec![Event::Footnote { id: None, text: "plain _em_ text".into() }]
        );
        // A marker escape (`\*`) is deferred into the quotes pass — no sentinel —
        // so it also stays on the raw-text common path.
        assert_eq!(
            pipeline("footnote:[\\*x*]"),
            vec![Event::Footnote { id: None, text: "\\*x*".into() }]
        );
    }

    /// With the `:experimental:`-gated UI macros ported, the pipeline (run with
    /// `experimental` set) must reproduce the legacy parser on every form: the
    /// bracket-only `kbd:[keys]`/`btn:[label]` (incl. mid-word, inside a span, and
    /// the malformed/empty-content forms that stay literal), and the
    /// `menu:target[items]` macro (incl. empty items, a `>`-sequence, a nested `[`
    /// stopping at the first `]`, and the empty-target form that declines). Raw
    /// quote markers in the content are NOT re-parsed (the renderer owns the
    /// `+`/`,`/`>` splitting). Inputs whose brackets would hold a passthrough/
    /// escape/char-ref leaf are deliberately excluded: an earlier pass lifts those
    /// into a sentinel and the leaf-macro sentinel guard then declines
    /// (`flag_decline` → legacy fallback), so the engine never forms them itself.
    #[test]
    fn reproduces_legacy_on_ui_macro_inputs() {
        let cases = [
            // kbd: keys, single key, in a sentence, with modifiers/commas
            "kbd:[Ctrl+C]",
            "kbd:[F11]",
            "Press kbd:[Ctrl+T] to open a tab",
            "kbd:[Ctrl,T]",
            // btn: label, in a sentence
            "btn:[OK]",
            "click the btn:[Save] button",
            // menu: target with items, empty items, a `>` sub-menu sequence
            "menu:File[New]",
            "menu:File[]",
            "menu:View[Zoom > Reset]",
            "menu:File[Open Recent > Reopen]",
            // first `]` ends the items; a nested `[` stays literal in the items text
            "menu:File[a [b] c]",
            // raw quote markers in the content are NOT re-parsed (renderer-owned)
            "kbd:[*x*]",
            "btn:[_y_]",
            // inside a constrained span (the span re-parses the macro with the same
            // experimental option, exactly as legacy threads `self.options`)
            "*kbd:[X]*",
            "`btn:[Y]`",
            // mid-word prefix still matches at legacy's offset (no left boundary)
            "xkbd:[Y]",
            // mixed run of all three
            "kbd:[Esc] then menu:Edit[Undo] then btn:[Go]",
            // invalid / empty → literal (empty content, no bracket, no close)
            "kbd:[]",
            "btn:[]",
            "kbd:noclose",
            "btn:[unclosed",
            "menu:[x]",      // empty target → declines
            "menu:File",     // no bracket → literal
            "menu:File[a",   // no close → literal
            // a UI prefix with experimental still defers to the surrounding text
            "plain kbd text with no macro",
        ];
        for c in cases {
            assert_eq!(
                pipeline_exp(c),
                legacy_exp(c),
                "new engine diverged from legacy (experimental) for {c:?}"
            );
        }

        // The leaf shapes: kbd/btn carry the content as one raw `Text`; menu carries
        // its target on the tag and the items (when non-empty) as one raw `Text`.
        assert_eq!(
            pipeline_exp("kbd:[Ctrl+C]"),
            vec![
                Event::Start(Tag::Keyboard),
                Event::Text("Ctrl+C".into()),
                Event::End(TagEnd::Keyboard),
            ]
        );
        assert_eq!(
            pipeline_exp("btn:[Save]"),
            vec![
                Event::Start(Tag::Button),
                Event::Text("Save".into()),
                Event::End(TagEnd::Button),
            ]
        );
        assert_eq!(
            pipeline_exp("menu:File[New]"),
            vec![
                Event::Start(Tag::Menu { target: "File".into() }),
                Event::Text("New".into()),
                Event::End(TagEnd::Menu),
            ]
        );
        // Empty items → no `Text` between `Start` and `End`.
        assert_eq!(
            pipeline_exp("menu:File[]"),
            vec![
                Event::Start(Tag::Menu { target: "File".into() }),
                Event::End(TagEnd::Menu),
            ]
        );

        // The macros fire ONLY under `:experimental:`. With it unset the prefix is
        // not a macro: no UI event is produced (the bytes stay plain text, matching
        // Asciidoctor's default and `legacy` with experimental off).
        assert_eq!(pipeline("kbd:[Ctrl+C]"), legacy("kbd:[Ctrl+C]"));
        assert!(!pipeline("kbd:[Ctrl+C]")
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::Keyboard))));
        assert!(pipeline_exp("kbd:[Ctrl+C]")
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::Keyboard))));
    }

    /// Quoted inline menu (`"A > B"` under `:experimental:`, Asciidoctor
    /// `InlineMenuRx`): a double-quoted run whose content starts with `[\w&]` and
    /// holds a space-flanked `>` becomes a `MenuSeq` of one `MenuPart` per segment,
    /// each segment re-parsed (so `icon:`/`link:`/quotes render inside).
    #[test]
    fn quoted_inline_menu_matches_asciidoctor() {
        let menu = |role| Event::Start(Tag::MenuPart { role });
        // Two segments → menu + menuitem (no submenu).
        assert_eq!(
            pipeline_exp("\"File > Save\""),
            vec![
                Event::Start(Tag::MenuSeq),
                menu(MenuPart::Menu),
                Event::Text("File".into()),
                Event::End(TagEnd::MenuPart),
                menu(MenuPart::Item),
                Event::Text("Save".into()),
                Event::End(TagEnd::MenuPart),
                Event::End(TagEnd::MenuSeq),
            ]
        );
        // Three segments → middle is a submenu.
        assert_eq!(
            pipeline_exp("\"File > New > Tab\""),
            vec![
                Event::Start(Tag::MenuSeq),
                menu(MenuPart::Menu),
                Event::Text("File".into()),
                Event::End(TagEnd::MenuPart),
                menu(MenuPart::Submenu),
                Event::Text("New".into()),
                Event::End(TagEnd::MenuPart),
                menu(MenuPart::Item),
                Event::Text("Tab".into()),
                Event::End(TagEnd::MenuPart),
                Event::End(TagEnd::MenuSeq),
            ]
        );
        // The menu segment is re-parsed: an inner `icon:` becomes Icon events
        // (the corpus case `"icon:apple[] > Software Update"`).
        assert_eq!(
            pipeline_exp("\"icon:apple[] > Software Update\""),
            vec![
                Event::Start(Tag::MenuSeq),
                menu(MenuPart::Menu),
                Event::Start(Tag::Icon { name: "apple".into() }),
                Event::End(TagEnd::Icon),
                Event::End(TagEnd::MenuPart),
                menu(MenuPart::Item),
                Event::Text("Software Update".into()),
                Event::End(TagEnd::MenuPart),
                Event::End(TagEnd::MenuSeq),
            ]
        );
        // A macro also renders in a non-first segment (full-subs re-parse).
        assert_eq!(
            pipeline_exp("\"File > link:http://x[T]\""),
            vec![
                Event::Start(Tag::MenuSeq),
                menu(MenuPart::Menu),
                Event::Text("File".into()),
                Event::End(TagEnd::MenuPart),
                menu(MenuPart::Item),
                Event::Start(Tag::Link {
                    url: "http://x".into(),
                    window: None,
                    nofollow: false,
                    is_bare: false,
                    role: None,
                }),
                Event::Text("T".into()),
                Event::End(TagEnd::Link),
                Event::End(TagEnd::MenuPart),
                Event::End(TagEnd::MenuSeq),
            ]
        );

        // Non-matches: no `MenuSeq` is produced (stay literal / smart-quote pass).
        let no_menuseq = |t: &str| {
            assert!(
                !pipeline_exp(t)
                    .iter()
                    .any(|e| matches!(e, Event::Start(Tag::MenuSeq))),
                "unexpected MenuSeq for {t:?}"
            );
        };
        no_menuseq("\"a>b\""); // no whitespace around `>`
        no_menuseq("\"File >Save\""); // space only before `>`
        no_menuseq("\"File> Save\""); // space only after `>`
        no_menuseq("\"*bold* > x\""); // first content char `*` ∉ [\w&]
        no_menuseq("\"hello world\""); // no `>` at all

        // Leading `\` escapes the rule: the quoted run stays literal, no MenuSeq.
        no_menuseq("\\\"File > Save\"");
        assert!(
            pipeline_exp("\\\"File > Save\"")
                .iter()
                .any(|e| matches!(e, Event::Text(t) if t.contains("File > Save"))),
            "escaped quoted-menu should keep the literal text"
        );

        // The rule fires ONLY under `:experimental:`. Without it, no MenuSeq.
        assert!(!pipeline("\"File > Save\"")
            .iter()
            .any(|e| matches!(e, Event::Start(Tag::MenuSeq))));
    }

    /// The signature cross-span case: a constrained strong that opens inside one
    /// monospace region and closes inside the next produces *overlapping*,
    /// non-nested events — which the recursive legacy parser cannot. With the
    /// differential gate removed (Phase 3) the engine now adopts this overlap
    /// instead of falling back to legacy — the raw pipeline must produce it.
    #[test]
    fn produces_cross_span_overlap() {
        let events = pipeline("a *crosses `code* span`");
        assert_eq!(
            events,
            vec![
                Event::Text("a ".into()),
                Event::Start(Tag::Strong { id: None, roles: vec![] }),
                Event::Text("crosses ".into()),
                Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                Event::Text("code".into()),
                Event::End(TagEnd::Strong),
                Event::Text(" span".into()),
                Event::End(TagEnd::Monospace),
            ]
        );
        // And it genuinely differs from the legacy (nested) interpretation.
        assert_ne!(events, legacy("a *crosses `code* span`"));
    }

    #[test]
    fn try_parse_declines_without_quotes() {
        // Verbatim subs carry no inline-needing flag (`specialchars`/`callouts`
        // only), so `needs_inline_parsing()` is false → engine defers to legacy.
        assert!(try_parse("*x*", SubstitutionSet::VERBATIM, InlineOptions::default()).is_none());
        assert!(try_parse("*x*", SubstitutionSet::NONE, InlineOptions::default()).is_none());
        // But a non-QUOTES set that DOES need inline parsing (e.g. `[subs=macros]`,
        // `[subs=attributes]`) now runs the engine rather than deferring.
        let mut macros_only = SubstitutionSet::NONE;
        macros_only.add(SubstitutionSet::MACROS);
        assert!(try_parse("link:u[t]", macros_only, InlineOptions::default()).is_some());
        let mut attrs_only = SubstitutionSet::NONE;
        attrs_only.add(SubstitutionSet::ATTRIBUTES);
        assert!(try_parse("\\*x* {n}", attrs_only, InlineOptions::default()).is_some());
    }

    /// The engine must reproduce the legacy parser byte-for-byte on every
    /// inline-needing substitution set that does NOT include `quotes`
    /// (`[subs=attributes]`, `[subs=+macros]`, `[subs=attributes+]`, …). Removing
    /// the old `!subs.has(QUOTES)` gate routes those blocks through the engine, so
    /// this pins the parity the corpus gate (`gate_check.py`) also guards.
    ///
    /// Quote-marker escapes (`\*` `\_` `` \` `` `\#` `\^` `\~` `\'`) are the one
    /// construct that needed engine work: with `quotes` off no quotes pass runs to
    /// consume them, so [`escape::run`]'s `!quotes_on` arm drops the backslash here
    /// to match the legacy parser's unconditional escape catch-all.
    ///
    /// Excluded by design (engine intentionally diverges from legacy even at
    /// NORMAL, so they are not a non-`quotes` regression): the bare doubled
    /// backslash (`a\\b`, `\\x` — the engine keeps BOTH backslashes like
    /// Asciidoctor, the legacy parser drops one) and the deferred `\++`/`\+++`
    /// forms (which flag a decline and fall back to legacy anyway).
    #[test]
    fn reproduces_legacy_on_non_quotes_subs() {
        fn set(flags: &[u8]) -> SubstitutionSet {
            let mut s = SubstitutionSet::NONE;
            for &f in flags {
                s.add(f);
            }
            s
        }
        // Compare for HTML-equivalence, not raw event identity: the two parsers
        // legitimately differ in how they SPLIT a text run into `Text` events (the
        // legacy typographic-escape arm emits the sealed `\--`/`\(C)` as its own
        // `Text`, the engine coalesces it into the surrounding run — a pre-existing
        // structural difference, HTML-identical, present at NORMAL too). Two
        // adjacent `Text` events render exactly as their concatenation, so merging
        // them before comparison still catches every content/tag/structure
        // divergence while tolerating that HTML-neutral re-split. This is the
        // parity the corpus gate (`gate_check.py`) measures on the HTML directly.
        fn coalesce(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
            let mut out: Vec<Event> = Vec::with_capacity(events.len());
            for ev in events {
                match (&ev, out.last_mut()) {
                    (Event::Text(t), Some(Event::Text(prev))) => {
                        prev.to_mut().push_str(t);
                    }
                    _ => out.push(ev),
                }
            }
            out
        }
        let subs_sets = [
            set(&[SubstitutionSet::ATTRIBUTES]),
            set(&[SubstitutionSet::MACROS]),
            set(&[SubstitutionSet::REPLACEMENTS]),
            set(&[SubstitutionSet::POST_REPLACEMENTS]),
            set(&[SubstitutionSet::SPECIALCHARS, SubstitutionSet::ATTRIBUTES]),
            // `[subs=attributes+]` on a listing block (VERBATIM + attributes).
            set(&[
                SubstitutionSet::SPECIALCHARS,
                SubstitutionSet::CALLOUTS,
                SubstitutionSet::ATTRIBUTES,
            ]),
            set(&[SubstitutionSet::SPECIALCHARS, SubstitutionSet::MACROS]),
            set(&[SubstitutionSet::SPECIALCHARS, SubstitutionSet::REPLACEMENTS]),
            // Everything-but-quotes — the stress set.
            SubstitutionSet::NORMAL.without(SubstitutionSet::QUOTES),
        ];
        let inputs = [
            // Quote/super/sub marker escapes — the `!quotes_on` arm's target.
            "\\*bold*",
            "\\_em_",
            "\\`code`",
            "\\#mark#",
            "\\^sup^",
            "\\~sub~",
            "\\*nospan",
            "word\\*here",
            "a \\* b \\_ c \\` d",
            // Apostrophe escapes (word-flanked + non-word-flanked).
            "it\\'s",
            "\\'word'",
            "say \\'hi\\'",
            // Non-marker single escapes (already unconditional in both).
            "\\{name}",
            "\\[x]",
            "\\<tag",
            // Attribute references.
            "{author}",
            "{set:x:1}{x}",
            "\\{name} {name}",
            "before {undefined-attr} after",
            // Inline macros / autolinks (MACROS-gated identically in both).
            "link:http://x.com[text]",
            "xref:sec-id[label]",
            "image:pic.png[alt text]",
            "mailto:a@b.com[mail]",
            "\\link:u[t]",
            "\\link:u[t] more text",
            "((visible term))",
            "\\((term))",
            "see http://example.com now",
            // Character references.
            "&#167; section",
            "&copy; 2026",
            "\\&#174; mark",
            "caf&#233; au lait",
            // Replacements text + escapes.
            "em -- dash",
            "(C) and (R) and (TM)",
            "ellipsis ...",
            "\\-- kept",
            "\\(C) kept",
            "\\... kept",
            // Passthrough (extracted unconditionally in both).
            "+single+",
            "++double++",
            "pass:[\\{x}]",
            "pass:q[*bold*]",
            "pass:a[{author}]",
            "code +\\{name}+ here",
            "\\+plus+",
            "empty pass:[] here",
            // Mixed / edge.
            "trailing backslash abc\\",
            "plain text, nothing special",
        ];
        for subs in subs_sets {
            for input in inputs {
                assert_eq!(
                    coalesce(pipeline_subs(input, subs)),
                    coalesce(legacy_subs(input, subs)),
                    "engine diverged from legacy for {input:?} under subs {subs:?}",
                );
            }
        }
    }

    #[test]
    fn try_parse_declines_on_sentinel_bytes() {
        let with_lead = "a\u{01}b";
        assert!(try_parse(with_lead, SubstitutionSet::NORMAL, InlineOptions::default()).is_none());
    }

    #[test]
    fn try_parse_adopts_plain_quotes_result() {
        // A plain quotes input the engine reproduces exactly as legacy.
        let got = try_parse("*bold* and _em_", SubstitutionSet::NORMAL, InlineOptions::default());
        assert_eq!(got, Some(legacy("*bold* and _em_")));
    }

    /// The doubled-backslash escape also covers the macro / char-reference /
    /// index-term forms (whose single-backslash escape lives in the `escape` pass:
    /// `\\image:…` → `\image:…`, `\\&copy;` → `\&amp;copy;`, `\\((term))` →
    /// `\((term))`, `\\xref:id[t]` → `\xref:id[t]`) and the double-plus passthrough
    /// (`\\++pp++` → `++pp++`, `\\++*x*++` → `++<strong>x</strong>++`). Only the
    /// construct-adjacent backslash is consumed. Deliberately NOT covered (left
    /// deferred — Asciidoctor itself is pathological/inconsistent there): the
    /// URL-target `\\link:http://…[…]` (which Asciidoctor still renders as a link)
    /// and the triple-plus `\\+++…+++`. As with the markers, every covered form
    /// diverges from the legacy parser, and the engine now ADOPTS the
    /// Asciidoctor-faithful result (asserted below).
    #[test]
    fn handles_doubled_backslash_macro_index_and_double_plus() {
        for (input, expected) in [
            // macro escapes — one backslash dropped, the macro kept literal
            (
                "\\\\image:a.png[alt]",
                vec![Event::Text("\\".into()), Event::Text("image:a.png[alt]".into())],
            ),
            (
                "\\\\xref:id[t]",
                vec![Event::Text("\\".into()), Event::Text("xref:id[t]".into())],
            ),
            // char-reference escape (renders `\&amp;copy;`)
            (
                "\\\\&copy;",
                vec![Event::Text("\\".into()), Event::Text("&copy;".into())],
            ),
            // index-term shorthand escape
            (
                "\\\\((term))",
                vec![Event::Text("\\".into()), Event::Text("((term))".into())],
            ),
            // double-plus passthrough — `++` markers literal, content flows
            (
                "\\\\++pp++",
                vec![
                    Event::Text("++".into()),
                    Event::Text("pp".into()),
                    Event::Text("++".into()),
                ],
            ),
            (
                "\\\\++*x*++",
                vec![
                    Event::Text("++".into()),
                    Event::Start(Tag::Strong { id: None, roles: vec![] }),
                    Event::Text("x".into()),
                    Event::End(TagEnd::Strong),
                    Event::Text("++".into()),
                ],
            ),
        ] {
            assert_eq!(pipeline(input), expected, "pipeline result for {input:?}");
        }

        for c in ["\\\\image:a.png[alt]", "\\\\&copy;", "\\\\((term))", "\\\\++pp++"] {
            assert_eq!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()),
                Some(pipeline(c)),
                "engine should adopt the Asciidoctor-faithful result for {c:?}"
            );
        }
    }

    #[test]
    fn try_parse_adopts_cross_span_overlap() {
        // Cross-span overlap is the Asciidoctor-faithful result the legacy (nested)
        // parser cannot produce; with the gate removed `try_parse` adopts it.
        let input = "a *crosses `code* span`";
        assert_eq!(
            try_parse(input, SubstitutionSet::NORMAL, InlineOptions::default()),
            Some(pipeline(input))
        );
    }

    /// A doubled (or longer) backslash before a constrained quote marker, a
    /// super/sub marker, a `pass:` macro, or a single-plus passthrough consumes
    /// exactly the ONE backslash adjacent to the construct (Asciidoctor's `\\?`
    /// capture), keeping the construct literal and leaving every leading backslash
    /// as a literal boundary char. This DIVERGES from the legacy parser, which
    /// either still renders the span (`\\*bold*` → `\<strong>bold</strong>`) or
    /// emits a different event shape; with the gate removed the engine now ADOPTS
    /// every form (asserted below). Matches Asciidoctor byte-for-byte (verified
    /// against `asciidoctor -s` on each form).
    #[test]
    fn handles_doubled_backslash_escape() {
        // `pipeline` reproduces Asciidoctor: one backslash dropped, the construct
        // kept literal.
        for (input, expected) in [
            // constrained markers — one backslash dropped, span NOT formed
            ("\\\\*bold*", vec![Event::Text("\\*bold*".into())]),
            ("\\\\_em_", vec![Event::Text("\\_em_".into())]),
            ("\\\\`code`", vec![Event::Text("\\`code`".into())]),
            ("\\\\#mark#", vec![Event::Text("\\#mark#".into())]),
            // super/sub simple pairs
            ("\\\\^sup^", vec![Event::Text("\\^sup^".into())]),
            ("\\\\~sub~", vec![Event::Text("\\~sub~".into())]),
            // pass macro (bare + spec'd) — kept literal, content still flows but
            // forms no span here (the `*y*` is word-flanked)
            ("\\\\pass:[raw]", vec![Event::Text("\\pass:[raw]".into())]),
            ("\\\\pass:q[x*y*z]", vec![Event::Text("\\pass:q[x*y*z]".into())]),
            // single-plus passthrough
            ("\\\\+plus+", vec![Event::Text("\\+plus+".into())]),
            // mid-text and the multi-backslash cascade: only the marker-adjacent
            // `\` is consumed, leading ones stay literal
            ("a \\\\*b* c", vec![Event::Text("a \\*b* c".into())]),
            ("\\\\\\*a*", vec![Event::Text("\\\\*a*".into())]),
            // a doubled backslash inside a span whose own close is INVALID keeps
            // BOTH backslashes literal — the inner `*a*` cannot close before the
            // word char `_`, so no escape fires (Asciidoctor parity)
            (
                "_\\\\*a*_",
                vec![
                    Event::Start(Tag::Emphasis { id: None, roles: vec![] }),
                    Event::Text("\\\\*a*".into()),
                    Event::End(TagEnd::Emphasis),
                ],
            ),
            // …but inside `` `code` `` the inner `*a*` CAN close (before the
            // backtick), so one backslash is consumed
            (
                "`\\\\*a*`",
                vec![
                    Event::Start(Tag::Monospace { id: None, roles: vec![] }),
                    Event::Text("\\*a*".into()),
                    Event::End(TagEnd::Monospace),
                ],
            ),
        ] {
            assert_eq!(pipeline(input), expected, "pipeline result for {input:?}");
        }

        // The engine ADOPTS every form: the new (correct) result differs from the
        // legacy parser's (buggy span / split-event) output, and with the gate
        // removed `try_parse` returns the Asciidoctor-faithful pipeline result.
        for c in [
            "\\\\*bold*",
            "\\\\^sup^",
            "\\\\pass:q[x*y*z]",
            "\\\\pass:[raw]",
            "\\\\+plus+",
        ] {
            assert_eq!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()),
                Some(pipeline(c)),
                "engine should adopt the Asciidoctor-faithful result for {c:?}"
            );
        }
    }

    /// An attribute reference inside an inline `[attrlist]` (`[{role}]*x*`,
    /// `[.{role}]_y_`, `[#{id}]`z``) must survive into the captured role/id as the
    /// literal `{name}`, NOT as a raw extraction sentinel. The engine runs the
    /// `attributes` pass before `quotes`, so without `desentinelize` the quotes
    /// pass would capture the sentinel byte sequence verbatim and leak it into the
    /// rendered `class`/`id`. The renderer resolves the `{name}` afterwards
    /// (mirroring Asciidoctor's quotes-then-attributes ordering), so the parser's
    /// job is only to restore the literal source text here.
    #[test]
    fn force_resolves_attr_ref_sentinel_in_inline_attrlist() {
        fn strong(roles: Vec<&str>, id: Option<&str>, body: &str) -> Vec<Event<'static>> {
            vec![
                Event::Start(Tag::Strong {
                    id: id.map(|s| s.to_string().into()),
                    roles: roles.into_iter().map(|r| r.to_string().into()).collect(),
                }),
                Event::Text(body.to_string().into()),
                Event::End(TagEnd::Strong),
            ]
        }

        // Positional role, shorthand role, shorthand id, id+role combo,
        // unconstrained span, and the emphasis/monospace markers.
        assert_eq!(pipeline("[{a}]*x*"), strong(vec!["{a}"], None, "x"));
        assert_eq!(
            pipeline("[.{a}]_y_"),
            vec![
                Event::Start(Tag::Emphasis { id: None, roles: vec!["{a}".to_string().into()] }),
                Event::Text("y".to_string().into()),
                Event::End(TagEnd::Emphasis),
            ]
        );
        assert_eq!(
            pipeline("[#{a}]`z`"),
            vec![
                Event::Start(Tag::Monospace {
                    id: Some("{a}".to_string().into()),
                    roles: vec![],
                }),
                Event::Text("z".to_string().into()),
                Event::End(TagEnd::Monospace),
            ]
        );
        assert_eq!(
            pipeline("[#{a}.{b}]*c*"),
            strong(vec!["{b}"], Some("{a}"), "c")
        );
        assert_eq!(
            pipeline("[{a}]##u##"),
            vec![
                Event::Start(Tag::InlineSpan { id: None, roles: vec!["{a}".to_string().into()] }),
                Event::Text("u".to_string().into()),
                Event::End(TagEnd::InlineSpan),
            ]
        );

        // No raw sentinel may survive: the role string is exactly `{a}`.
        if let Event::Start(Tag::Strong { roles, .. }) = &pipeline("[{a}]*x*")[0] {
            assert_eq!(roles[0].as_ref(), "{a}");
            assert!(!roles[0].as_ref().bytes().any(|b| b == 0x01 || b == 0x02));
        } else {
            panic!("expected an attributed strong span");
        }

        // The SHORTHAND forms (`[.role]` / `[#id]`) are parsed identically by the
        // legacy parser, which also keeps the `{name}` literal in the role/id.
        // Restoring the literal here makes the new result byte-equal to legacy, so
        // the engine adopts them, finished off by the renderer's reference
        // resolution.
        for c in ["[.{a}]_y_", "[#{a}]`z`", "[#{a}.{b}]*c*"] {
            assert_eq!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()),
                Some(pipeline(c)),
                "engine should adopt the result for {c:?}"
            );
        }

        // The POSITIONAL form (`[role]`, no `.`/`#`) is NOT treated as a span
        // attrlist by the legacy parser (it emits the bracket and an
        // `AttributeReference` separately), so the new result diverges; with the
        // gate removed the engine ADOPTS the corrected span.
        for c in ["[{a}]*x*", "[{a}]##u##"] {
            assert_eq!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()),
                Some(pipeline(c)),
                "engine should adopt the corrected span for {c:?}"
            );
        }
    }

    /// An attrlist-prefixed *constrained* span whose content begins with the same
    /// marker (`[.path]__config_`) falls back from the unconstrained form to the
    /// constrained one, exactly like Asciidoctor's pass order: the unconstrained
    /// `__…__` pass finds no closing `__`, so the constrained `_(\S|\S.*?\S)_`
    /// pass matches with the second marker folded into the content. Verified
    /// against `asciidoctor 2.0.23` for every quote marker.
    #[test]
    fn attrlist_constrained_falls_back_from_doubled_marker() {
        // `[.path]__config/site.yml_` → <em class="path">_config/site.yml</em>
        assert_eq!(
            pipeline("[.path]__config/site.yml_"),
            vec![
                Event::Start(Tag::Emphasis {
                    id: None,
                    roles: vec!["path".to_string().into()],
                }),
                Event::Text("_config/site.yml".to_string().into()),
                Event::End(TagEnd::Emphasis),
            ]
        );
        // Same fall-back for strong / monospace / mark-span (generic over marker).
        assert_eq!(
            pipeline("[.r]**bold_x*"),
            vec![
                Event::Start(Tag::Strong { id: None, roles: vec!["r".to_string().into()] }),
                Event::Text("*bold_x".to_string().into()),
                Event::End(TagEnd::Strong),
            ]
        );
        assert_eq!(
            pipeline("[.r]``code_x`"),
            vec![
                Event::Start(Tag::Monospace { id: None, roles: vec!["r".to_string().into()] }),
                Event::Text("`code_x".to_string().into()),
                Event::End(TagEnd::Monospace),
            ]
        );
        assert_eq!(
            pipeline("[.r]##mark_x#"),
            vec![
                Event::Start(Tag::InlineSpan { id: None, roles: vec!["r".to_string().into()] }),
                Event::Text("#mark_x".to_string().into()),
                Event::End(TagEnd::InlineSpan),
            ]
        );
        // The genuinely-unconstrained form (`[.r]__closed__`, closing `__` present)
        // is still owned by the earlier unconstrained pass: content has NO leading
        // marker, so the fall-back must not double-process it.
        assert_eq!(
            pipeline("[.r]__closed__"),
            vec![
                Event::Start(Tag::Emphasis { id: None, roles: vec!["r".to_string().into()] }),
                Event::Text("closed".to_string().into()),
                Event::End(TagEnd::Emphasis),
            ]
        );
    }
}

