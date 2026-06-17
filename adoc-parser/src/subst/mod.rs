//! Sequential-pass inline substitution engine (Asciidoctor `Substitutors`
//! model).
//!
//! TRANSITIONAL / DEV scaffolding. The legacy recursive inline parser
//! ([`crate::inline`]) remains the default; this engine is built in parallel
//! behind the `ADOC_QUOTES_SEQUENTIAL=1` environment toggle until it reaches
//! byte-for-byte corpus parity, at which point it becomes the default and the
//! legacy quotes path is removed (see plan `greedy-yawning-pumpkin`).
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
//! requires the string-rewriting pipeline this module houses.
//!
//! ## Phase 1: quotes pipeline behind a differential-equality gate
//!
//! This phase implements the `quotes` pass family ([`quotes`]) plus the
//! sentinel tokenizer ([`tokenize`]). The other passes (passthrough,
//! specialchars, attributes, replacements, macros, post-replacements) are NOT
//! yet implemented.
//!
//! To make partial coverage provably **zero-regression**, [`try_parse`] runs
//! the new pipeline AND the legacy parser and only returns the new result when
//! the two event streams are byte-identical; on any difference it returns
//! `None` and the caller falls back to legacy. So with the toggle on, corpus
//! output is unchanged (the gate). This gate is a Phase-1-only scaffold: Phase 2
//! removes it precisely so the divergent (overlapping) cases can flip
//! `outline.adoc`.
//!
//! `ADOC_SUBST_FORCE=1` (diagnostic, requires the toggle on) skips the gate and
//! returns the raw new-engine result, so a `blast` run measures how faithfully
//! the new engine reproduces the legacy output (divergences show up as diffs).

mod attributes;
mod char_refs;
mod escape;
mod macros;
mod passthrough;
mod post_replacements;
mod quotes;
mod replacements;
mod tokenize;

use std::sync::OnceLock;

use crate::event::{Event, SubstitutionSet};
use crate::inline::InlineOptions;
use tokenize::{Work, TAG_LEAD, TAG_TAIL};

/// Whether the sequential-quotes engine is enabled for this process.
///
/// Read once from the `ADOC_QUOTES_SEQUENTIAL` env var (`1`/`true` enables).
/// This is a transition-only toggle: it disappears when the engine becomes the
/// default in the final phase. A process-global is acceptable because the
/// corpus harness (`blast_toggle.py`) runs each engine in a separate process.
pub(crate) fn enabled() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| env_true("ADOC_QUOTES_SEQUENTIAL"))
}

/// Diagnostic: when set (and the engine is enabled), bypass the
/// differential-equality gate and return the raw new-engine result. Used to
/// measure reproduction fidelity via `blast`; NOT the safety gate.
fn force() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| env_true("ADOC_SUBST_FORCE"))
}

fn env_true(var: &str) -> bool {
    std::env::var(var)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Attempt to parse top-level inline `text` with the sequential-pass engine.
///
/// Returns `None` when the engine declines (no quotes substitution requested,
/// the input contains a reserved sentinel byte, or — under the Phase 1 gate —
/// the result would differ from the legacy parser), signalling the caller to
/// fall back to the legacy recursive parser. Only called for top-level
/// paragraph text, never for inner-span reparses.
pub(crate) fn try_parse<'a>(
    text: &'a str,
    subs: SubstitutionSet,
    options: InlineOptions,
) -> Option<Vec<Event<'a>>> {
    // The engine only performs the `quotes` substitution; without it there is
    // nothing to do (e.g. verbatim blocks) — defer to legacy.
    if !subs.has(SubstitutionSet::QUOTES) {
        return None;
    }
    // A sentinel byte in the source would be indistinguishable from an
    // engine-inserted sentinel; refuse such input outright.
    if text
        .bytes()
        .any(|b| b == TAG_LEAD || b == TAG_TAIL)
    {
        return None;
    }

    let candidate = run_pipeline(text, subs, options);

    if force() {
        return Some(candidate);
    }

    // Phase 1 gate: only adopt the new result when it exactly reproduces legacy.
    let legacy = crate::inline::parse_legacy(text, subs, options);
    if candidate == legacy {
        Some(candidate)
    } else {
        None
    }
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
/// `options.experimental`), `replacements`, `post_replacements`. The remaining
/// escape forms (the `\pass:`/`\https://` autolink escapes, and the bare `\\`
/// escaped backslash) — and the quote-marker escapes `\*`/`\_`/`` \` `` and
/// `\+`, which belong inside the quote/passthrough passes — are not yet ported;
/// inputs that need them diverge from legacy and are rejected by the gate (or
/// surface as diffs under `force()`). `options` is threaded through so the
/// experimental flag reaches the macros pass and every inner reparse (label,
/// cross-reference label, passthrough spec content), mirroring the legacy
/// parser's `self.options` propagation into nested `InlineState`s.
fn run_pipeline<'a>(text: &str, subs: SubstitutionSet, options: InlineOptions) -> Vec<Event<'a>> {
    let mut work = Work::new(text);
    // Passthroughs are extracted FIRST so their content is verbatim — opaque to
    // every later pass INCLUDING `escape` (mirrors Asciidoctor's
    // `extract_passthroughs`, which runs before all substitutions). A backslash
    // inside `+…+`/`pass:[…]` is literal content, never an escape, so extracting
    // first is what stops `escape` from mangling it (`` `+\{name}+` `` →
    // `<code>\{name}</code>`). Unconditional: the legacy parser runs
    // `+…+`/`pass:[…]` regardless of the subs flags, and the engine only runs when
    // QUOTES is present anyway.
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
    // `push_macro_label`). Ported so far: the cross-reference family
    // (`xref:`/`<<>>`), the link family (`link:`/`mailto:`, bare URL/email
    // autolinks), the inline image (`image:`), the leaf macros `icon:` and the
    // STEM family (`stem:`/`latexmath:`/`asciimath:`), and the anchor
    // (`[[id]]`/`[[[id]]]`/`anchor:`) and index-term (`((…))`/`indexterm:`/
    // `indexterm2:`) families; the remaining macros are not, so their inputs
    // diverge from legacy and the gate rejects them.
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
        quotes::run_all(&mut work);
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
    use crate::event::{Tag, TagEnd};

    fn legacy(text: &str) -> Vec<Event<'_>> {
        crate::inline::parse_legacy(text, SubstitutionSet::NORMAL, InlineOptions::default())
    }

    fn pipeline(text: &str) -> Vec<Event<'_>> {
        run_pipeline(text, SubstitutionSet::NORMAL, InlineOptions::default())
    }

    /// Like [`legacy`], but with `:experimental:` set so the legacy parser
    /// recognises the `kbd:`/`btn:`/`menu:` UI macros — the reference the ported
    /// new-engine arms must reproduce.
    fn legacy_exp(text: &str) -> Vec<Event<'_>> {
        crate::inline::parse_legacy(
            text,
            SubstitutionSet::NORMAL,
            InlineOptions { experimental: true },
        )
    }

    /// Like [`pipeline`], but with `:experimental:` set so the macros pass
    /// dispatches the UI macros.
    fn pipeline_exp(text: &str) -> Vec<Event<'_>> {
        run_pipeline(
            text,
            SubstitutionSet::NORMAL,
            InlineOptions { experimental: true },
        )
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
    /// these inputs the engine is more Asciidoctor-faithful and the gate falls
    /// back (asserted via `try_parse` returning `None`); under `force()` the
    /// engine matches Asciidoctor (this is the `outline.adoc` flip:
    /// `` `head` or `header; `foot` or `footer` ``).
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
        // The gate declines these (the raw engine diverges from the more permissive
        // legacy parser, which leaves the leading marker literal and closes at the
        // first inner marker).
        for c in ["x `a; `b` y", "`a `b` c", "`a`b`"] {
            assert!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()).is_none(),
                "gate should decline (diverge from legacy) for {c:?}"
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
            // invalid → stay literal (no brackets, empty target, reversed brackets,
            // empty cross reference)
            "xref:notarget",
            "xref:[]",
            "xref:a]b[c]",
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
        // span), so the engine now diverges and the gate falls back to legacy —
        // asserted via `try_parse` returning `None`. Under `force()` the engine
        // instead matches Asciidoctor (no link), which is what flips `page-breaks`.
        for c in [
            "<< id , the label >>", // leading space
            "<<-y>>",               // leading dash
            "<<\"a\">>",            // leading quote
            "a <<<b>>",             // inner `<<` matches at `b` → `<` literal + `#b`
        ] {
            assert!(
                try_parse(c, SubstitutionSet::NORMAL, InlineOptions::default()).is_none(),
                "gate should decline (diverge from legacy) for {c:?}"
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
            // icon: invalid → literal (no brackets, empty name, reversed brackets)
            "icon:noclose",
            "icon:[]",
            "icon:a]b[c]",
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
    /// event differently in legacy; the gate declines it and falls back (still
    /// correct), so it is excluded here. The `\\pass:` double-backslash form is
    /// likewise deferred.
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
    /// into a fresh run) while the flat engine merges it into one Text — the gate
    /// declines that and falls back (HTML identical). The `\\http://…`
    /// double-backslash form (legacy drops one backslash, Asciidoctor keeps both)
    /// is likewise excluded.
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
    /// A passthrough in the *label* (not the URL) is a decline — the engine cannot
    /// re-parse a label that already holds a sentinel, so the gate falls back to
    /// legacy; that is asserted separately below.
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
        ];
        for c in cases {
            assert_eq!(
                pipeline(c),
                legacy(c),
                "new engine diverged from legacy for {c:?}"
            );
        }
        // A passthrough sentinel in the LABEL declines (the engine can't re-parse a
        // label that already holds a sentinel): the gate falls back to legacy.
        assert!(
            try_parse(
                "link:http://x.com[++raw__text++]",
                SubstitutionSet::NORMAL,
                InlineOptions::default()
            )
            .is_none()
        );
    }

    /// The `\((…))` index-term-shorthand escape and the `\\MM…MM` doubled-marker
    /// escape (subs.adoc lines 20 and 27): the engine must reproduce legacy's
    /// event stream so the gate adopts them, and (under FORCE) render the literal
    /// `((…))` / `__…__` Asciidoctor emits.
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

    /// With the `:experimental:`-gated UI macros ported, the pipeline (run with
    /// `experimental` set) must reproduce the legacy parser on every form: the
    /// bracket-only `kbd:[keys]`/`btn:[label]` (incl. mid-word, inside a span, and
    /// the malformed/empty-content forms that stay literal), and the
    /// `menu:target[items]` macro (incl. empty items, a `>`-sequence, a nested `[`
    /// stopping at the first `]`, and the empty-target form that declines). Raw
    /// quote markers in the content are NOT re-parsed (the renderer owns the
    /// `+`/`,`/`>` splitting). Inputs whose brackets would hold a passthrough/
    /// escape/char-ref leaf are deliberately excluded: an earlier pass lifts those
    /// into a sentinel and the leaf-macro sentinel guard then declines (gate
    /// fallback), so they are not byte-equal to legacy.
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

    /// The signature cross-span case: a constrained strong that opens inside one
    /// monospace region and closes inside the next produces *overlapping*,
    /// non-nested events — which the recursive legacy parser cannot. The Phase 1
    /// gate therefore declines this input (falls back), but the raw pipeline
    /// must produce the overlap.
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
        // Verbatim subs (no QUOTES) → engine defers to legacy.
        assert!(try_parse("*x*", SubstitutionSet::VERBATIM, InlineOptions::default()).is_none());
    }

    #[test]
    fn try_parse_declines_on_sentinel_bytes() {
        let with_lead = "a\u{01}b";
        assert!(try_parse(with_lead, SubstitutionSet::NORMAL, InlineOptions::default()).is_none());
    }

    #[test]
    fn try_parse_gate_adopts_matching_result() {
        // A plain quotes input the engine reproduces exactly → gate adopts it.
        let got = try_parse("*bold* and _em_", SubstitutionSet::NORMAL, InlineOptions::default());
        assert_eq!(got, Some(legacy("*bold* and _em_")));
    }

    #[test]
    fn try_parse_gate_declines_divergent_result() {
        // Cross-span overlap differs from legacy → gate declines (None).
        assert!(
            try_parse(
                "a *crosses `code* span`",
                SubstitutionSet::NORMAL,
                InlineOptions::default()
            )
            .is_none()
        );
    }

}
