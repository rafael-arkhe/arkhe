//! Deterministic sentence splitting — **not** an LLM planner.
//!
//! [`decompose`] turns one input into an ordered [`Plan`] of steps, and
//! `AgiCoordinator::process_planned` runs each step through the full
//! `process()` cycle. The number of steps decides how many safety checks,
//! inference calls, evidence records, and memory entries an input produces,
//! so it is a decision worth being explicit about.
//!
//! # Why this is not the model's job
//!
//! The coordinator *has* an inference engine and could ask it to decompose
//! the input. It deliberately does not:
//!
//! - **The plan exists before any inference happens.** `process_planned`
//!   decomposes first, then processes each step; there is no turn to ask
//!   about yet, and asking would consume a turn of its own.
//! - **The plan decides how many records get written.** Handing that to a
//!   sampling engine would make the shape of the evidence chain depend on a
//!   non-deterministic call: the same input could produce one provenance
//!   record today and four tomorrow, and nothing in the chain would say
//!   which run it was.
//! - **A wrong plan is cheap to see and cheap to fix.** Every step is its
//!   own fully-attested turn with its own hash, so a bad split is visible in
//!   the provenance graph instead of being buried inside one opaque record.
//!
//! # What it actually does
//!
//! A step ends after a run of `.`, `!`, or `?` that is followed by
//! whitespace or by the end of the input. The punctuation stays attached to
//! the sentence it closes, so `"Really?!"` is one step ending `Really?!`,
//! and each step is trimmed of surrounding whitespace. An input with no
//! sentence-ending punctuation is a single step — one `process()` call,
//! indistinguishable from calling `process` directly — which is what makes
//! `"hello arkhe"` decompose to exactly one step.
//!
//! Two consequences of there being no abbreviation list and no corpus:
//!
//! - a `.` inside a token is not a boundary (it is not followed by
//!   whitespace), so `"1.0 is out"` and `"pi is 3.14"` are single steps;
//! - an abbreviation that ends a token *is* a boundary, so
//!   `"Dr. Smith arrived."` splits into `"Dr."` and `"Smith arrived."`. The
//!   fix for that is more steps, not a bigger abbreviation list: `decompose`
//!   is total and deterministic, and never fails.
//!
//! Empty input, and input that is nothing but whitespace, decompose to a
//! plan with **no steps**: there is nothing to ask, and `process_planned`
//! therefore returns `Ok(vec![])` having recorded no turn — rather than
//! fabricating a turn from an empty question. Punctuation by itself (`"..."`)
//! is not blank, so it does become a step; the rule is "no words, no step"
//! only for input that trims away completely.
//!
//! ```
//! use arkhe_agi::planner::decompose;
//!
//! let plan = decompose("What is X? What is Y?");
//! let texts: Vec<&str> = plan.steps.iter().map(|s| s.text.as_str()).collect();
//! assert_eq!(texts, ["What is X?", "What is Y?"]);
//!
//! // One sentence, one step — `process_planned` then behaves like `process`.
//! assert_eq!(decompose("hello arkhe").steps.len(), 1);
//!
//! // Nothing to ask.
//! assert!(decompose("   \n ").steps.is_empty());
//! ```

/// One step of a [`Plan`]: the text handed to `AgiCoordinator::process`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    /// The step's text with surrounding whitespace trimmed and its
    /// sentence-ending punctuation kept — the coordinator passes this
    /// verbatim as a `process()` input.
    pub text: String,
}

/// An ordered decomposition of one input.
///
/// `steps` may be empty (nothing to ask); it is never `None` and the order
/// is the order the steps must run in, because each step's turn is appended
/// to the same session history and evidence chain as the ones before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The steps, in the order they must run.
    pub steps: Vec<PlanStep>,
}

/// Splits `input` into steps at sentence boundaries.
///
/// Synchronous, total, and deterministic: the same input always produces the
/// same plan, and no input is an error. See the [module docs](self) for the
/// exact rule and its known limits.
pub fn decompose(input: &str) -> Plan {
    let chars: Vec<char> = input.chars().collect();
    let mut steps = Vec::new();
    let mut start = 0;

    let mut i = 0;
    while i < chars.len() {
        if !ends_a_sentence(chars[i]) {
            i += 1;
            continue;
        }

        // Take the whole run: "?!" is one sentence end, not two.
        let mut end = i;
        while end + 1 < chars.len() && ends_a_sentence(chars[end + 1]) {
            end += 1;
        }

        let boundary = end + 1;
        if boundary == chars.len() || chars[boundary].is_whitespace() {
            push_step(&mut steps, &chars[start..boundary]);
            start = boundary;
        }
        i = boundary;
    }

    push_step(&mut steps, &chars[start..]);
    Plan { steps }
}

/// Is this one of the punctuation marks that can end a sentence?
fn ends_a_sentence(c: char) -> bool {
    matches!(c, '.' | '!' | '?')
}

/// Appends `slice` as a step unless it is blank once trimmed.
///
/// The slice is not weighted by length, wording, or any other content: a
/// step is a step.
fn push_step(steps: &mut Vec<PlanStep>, slice: &[char]) {
    let text: String = slice.iter().collect();
    let text = text.trim();
    if !text.is_empty() {
        steps.push(PlanStep {
            text: text.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(input: &str) -> Vec<String> {
        decompose(input).steps.into_iter().map(|s| s.text).collect()
    }

    #[test]
    fn a_single_sentence_is_a_single_step() {
        assert_eq!(texts("hello arkhe"), ["hello arkhe"]);
    }

    #[test]
    fn each_sentence_ending_is_a_boundary() {
        assert_eq!(texts("One. Two! Three?"), ["One.", "Two!", "Three?"]);
    }

    #[test]
    fn the_punctuation_stays_with_its_sentence() {
        // The coordinator's `process_planned` test depends on this: a step's
        // text is what the engine sees, so the "?" has to survive.
        assert_eq!(texts("What is X? What is Y?"), ["What is X?", "What is Y?"]);
    }

    #[test]
    fn a_run_of_punctuation_is_one_boundary() {
        assert_eq!(texts("Really?! Yes."), ["Really?!", "Yes."]);
    }

    #[test]
    fn whitespace_is_trimmed_and_never_becomes_a_step() {
        assert_eq!(texts("  Hello.  World.  "), ["Hello.", "World."]);
        assert_eq!(texts("Hello.\nWorld.\n"), ["Hello.", "World."]);
    }

    #[test]
    fn nothing_to_ask_decomposes_to_no_steps() {
        assert!(decompose("").steps.is_empty());
        assert!(decompose("   \n\t ").steps.is_empty());
    }

    #[test]
    fn bare_punctuation_is_not_blank_so_it_is_a_step() {
        // "..." has no words, but it is not blank either, and deciding
        // otherwise would silently drop a "?" a caller did type.
        assert_eq!(texts("..."), ["..."]);
        assert_eq!(texts("?!"), ["?!"]);
    }

    #[test]
    fn a_full_stop_inside_a_token_is_not_a_boundary() {
        assert_eq!(texts("Version 1.0 is out."), ["Version 1.0 is out."]);
        assert_eq!(texts("pi is 3.14"), ["pi is 3.14"]);
    }

    #[test]
    fn an_abbreviation_that_ends_a_token_does_split() {
        // Documented limit, asserted so it cannot change silently.
        assert_eq!(texts("Dr. Smith arrived."), ["Dr.", "Smith arrived."]);
    }

    #[test]
    fn no_sentence_ending_means_one_step() {
        assert_eq!(texts("no punctuation at all"), ["no punctuation at all"]);
    }

    #[test]
    fn the_same_input_always_gives_the_same_plan() {
        let input = "Alpha beta. Gamma?! Delta";
        assert_eq!(decompose(input), decompose(input));
    }
}
