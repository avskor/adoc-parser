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

    let candidate = run_pipeline(text, subs);

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
/// `\'` smart-quote openers, `\[`/`\<`, typographic, and the `\&#…;` character-
/// reference escape), passthrough extract/restore, character-reference survival
/// (`&#167;` / `&copy;` → `InlinePassthrough`), `attributes` (`{name}` /
/// `{set:…}`, unresolved leaf events mirroring legacy), `quotes` (including the
/// `:double`/`:single` curved smart quotes), `replacements`, `post_replacements`.
/// The remaining passes (macros) — and the quote-marker escapes `\*`/`\_`/
/// `` \` `` and `\+`, which belong inside the quote/passthrough passes — are not
/// yet ported; inputs that need them diverge from legacy and are rejected by the
/// gate (or surface as diffs under `force()`).
fn run_pipeline<'a>(text: &str, subs: SubstitutionSet) -> Vec<Event<'a>> {
    let mut work = Work::new(text);
    // Passthroughs are extracted FIRST so their content is verbatim — opaque to
    // every later pass INCLUDING `escape` (mirrors Asciidoctor's
    // `extract_passthroughs`, which runs before all substitutions). A backslash
    // inside `+…+`/`pass:[…]` is literal content, never an escape, so extracting
    // first is what stops `escape` from mangling it (`` `+\{name}+` `` →
    // `<code>\{name}</code>`). Unconditional: the legacy parser runs
    // `+…+`/`pass:[…]` regardless of the subs flags, and the engine only runs when
    // QUOTES is present anyway.
    passthrough::extract(&mut work, subs);
    // Escapes are neutralised next, before the attribute/quote passes: a
    // recognised `\x` drops the backslash and turns `x` into a literal leaf that
    // is opaque to those passes (mirrors Asciidoctor's per-substitution `\\?`
    // capture). Running after passthrough means a `\` left in the buffer is always
    // top-level (passthrough-internal backslashes are already inside a sentinel).
    escape::run(&mut work);
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
        run_pipeline(text, SubstitutionSet::NORMAL)
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
