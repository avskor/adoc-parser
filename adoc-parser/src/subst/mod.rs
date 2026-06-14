//! Sequential-pass inline substitution engine (Asciidoctor `Substitutors` model).
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
//! before monospace, …). Because an earlier pass splices literal `<strong>` text
//! into the string before a later pass wraps backticks in `<code>`, a quote span
//! can physically *overlap* a sibling span — output Asciidoctor itself emits as
//! invalid, non-nested HTML. A recursive/tree parser (the legacy engine) can
//! only ever produce *nested* tags, so it cannot reproduce this; replicating it
//! requires the string-rewriting pipeline this module will house.
//!
//! Phase 0 (current): the pipeline body is unimplemented; [`try_parse`] always
//! returns `None`, so even with the toggle on every input falls back to the
//! legacy engine and output is unchanged.

use std::sync::OnceLock;

use crate::event::{Event, SubstitutionSet};
use crate::inline::InlineOptions;

/// Whether the sequential-quotes engine is enabled for this process.
///
/// Read once from the `ADOC_QUOTES_SEQUENTIAL` env var (`1`/`true` enables).
/// This is a transition-only toggle: it disappears when the engine becomes the
/// default in the final phase. A process-global is acceptable because the
/// corpus harness (`blast_toggle.py`) runs each engine in a separate process.
pub(crate) fn enabled() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| {
        std::env::var("ADOC_QUOTES_SEQUENTIAL")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

/// Attempt to parse top-level inline `text` with the sequential-pass engine.
///
/// Returns `None` when the engine cannot (yet) handle the input, signalling the
/// caller to fall back to the legacy recursive parser. Only called for
/// top-level paragraph text (the unit a line-level pass operates on), never for
/// inner-span reparses — those belong to the legacy model exclusively.
///
/// Phase 0: always `None` (pipeline body not implemented).
pub(crate) fn try_parse<'a>(
    _text: &'a str,
    _subs: SubstitutionSet,
    _options: InlineOptions,
) -> Option<Vec<Event<'a>>> {
    None
}
