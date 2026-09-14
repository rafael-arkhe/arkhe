//! FI-120 rule: no unredacted document number in a turn.
//!
//! [`NoUnredactedCpf`] rejects a turn whose text carries a CPF that should
//! have been redacted before it reached the agent. The turn is checked
//! *before* `AgiCoordinator::process` writes anything down, and all three
//! destinations are covered by that single position: the evidence chain
//! (whose `canonical_bytes` would preserve the number verbatim), working and
//! episodic memory, and the assistant reply returned to the caller.
//!
//! Both halves of the turn are checked — the user's input and the agent's
//! response — because a model that is handed a CPF tends to repeat it.
//!
//! # The heuristic, and its limits
//!
//! Two written shapes are recognised, and nothing else:
//!
//! | Shape | Example | Recognised |
//! |---|---|---|
//! | dotted | `123.456.789-00` | yes |
//! | bare | `12345678901` | yes |
//! | dotted, longer | `1123.456.789-001` | no — see below |
//! | bare, longer run | `123456789012` (12 digits) | no — see below |
//! | space-separated | `123 456 789 00` | no |
//! | redacted | `123.***.***-**`, `<redacted>` | yes (nothing to match) |
//!
//! The dotted shape matches only as a whole, delimited number: a run of 14
//! characters reading `NNN.NNN.NNN-NN` is not a match if the character
//! before it is another digit, `.`, or `-` (so the tail of a longer dotted
//! number is never mistaken for a CPF), nor if the character after it is a
//! digit or `-`. A `.` directly after the number is read as sentence
//! punctuation — `meu cpf é 123.456.789-00.` *is* caught — unless a digit
//! follows it.
//!
//! The bare shape matches a maximal run of ASCII digits whose length is
//! **exactly** 11. A substring rule ("any 11 consecutive digits") would flag
//! every turn carrying a millisecond timestamp or a longer account number,
//! which are common in ordinary text; requiring the run to be exactly 11
//! digits is what keeps the rule usable. The cost is a real blind spot: a
//! CPF embedded in a longer digit run is not detected.
//!
//! This is a **shape** check, not a CPF validation. No check-digit
//! arithmetic runs, so `000.000.000-00` and `111.111.111-11` are rejected
//! just like a real CPF, and so is any other 11-digit identifier — a
//! Brazilian mobile number (`11987654321`) has the same shape as a bare
//! CPF, and a constant with eleven digits after the point
//! (`3.14159265358`) has the digits of one; neither can be told apart from a
//! CPF here. That asymmetry is deliberate: a missed CPF ends up permanently
//! in the evidence chain and in memory, whereas a false positive only
//! rejects one turn, with a message naming the rule that rejected it.
//!
//! Deliberately not covered, in the same spirit of stating the limit rather
//! than implying completeness: a number split across the input/response
//! boundary (each half is checked on its own, so neither half sees a whole
//! number), any form with separators other than `.` and `-`, and numbers
//! written out as words.

use arkhe_geometric_verifier::{VerifiableRecord, VerificationRule};

/// Rejects a turn carrying an unredacted CPF in its input or its response.
///
/// Registered by `AgiCoordinator::new` after
/// [`NonEmptyResponse`](crate::geometry::NonEmptyResponse); see the
/// [module docs](self) for exactly which written shapes it detects and
/// which it does not.
///
/// ```
/// use arkhe_agi::compliance::NoUnredactedCpf;
/// use arkhe_agi::geometry::TurnRecord;
/// use arkhe_geometric_verifier::GeometricVerifier;
///
/// let mut verifier = GeometricVerifier::new();
/// verifier.register(NoUnredactedCpf);
///
/// let turn = TurnRecord {
///     user_input: "meu cpf é 123.456.789-00".to_string(),
///     response: "anotado".to_string(),
///     attested_by: None,
/// };
///
/// let err = verifier.verify(&turn).unwrap_err();
/// assert!(err.to_string().contains("NoUnredactedCpf"));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct NoUnredactedCpf;

impl VerificationRule for NoUnredactedCpf {
    fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
        if carries_unredacted_cpf(record.user_input()) || carries_unredacted_cpf(record.response())
        {
            Err(
                "the turn carries an unredacted document number (a CPF, in `NNN.NNN.NNN-NN` or \
                 11-digit form)"
                    .to_string(),
            )
        } else {
            Ok(())
        }
    }
}

/// How many characters the dotted shape occupies: `NNN.NNN.NNN-NN`.
const DOTTED_LEN: usize = 14;

/// How many digits a bare CPF has.
const CPF_DIGITS: usize = 11;

/// True when `text` contains either recognised shape.
fn carries_unredacted_cpf(text: &str) -> bool {
    // Collected once so the window scans below can index without caring
    // about UTF-8 boundaries; `text` is one turn's worth of characters.
    let chars: Vec<char> = text.chars().collect();
    contains_dotted_cpf(&chars) || contains_bare_cpf(&chars)
}

/// Looks for `NNN.NNN.NNN-NN` as a whole, delimited number.
fn contains_dotted_cpf(chars: &[char]) -> bool {
    if chars.len() < DOTTED_LEN {
        return false;
    }
    (0..=chars.len() - DOTTED_LEN).any(|start| {
        let end = start + DOTTED_LEN;
        is_dotted_cpf(&chars[start..end]) && delimited(chars, start, end)
    })
}

/// The shape alone: 11 digits in 3-3-3-2 layout with `.` and `-` separators.
fn is_dotted_cpf(window: &[char]) -> bool {
    window.iter().enumerate().all(|(i, c)| match i {
        3 | 7 => *c == '.',
        11 => *c == '-',
        _ => c.is_ascii_digit(),
    })
}

/// Is the window a stand-alone number rather than a slice of a longer one?
///
/// The leading side is the strict one: another digit, `.`, or `-` means the
/// match starts one character into a longer number. On the trailing side a
/// `.` is allowed (it is far more likely to be a sentence's full stop than
/// the start of a fourth digit group) unless a digit follows it.
fn delimited(chars: &[char], start: usize, end: usize) -> bool {
    if start > 0 && continues_a_number(chars[start - 1]) {
        return false;
    }
    match chars.get(end) {
        None => true,
        Some(c) if c.is_ascii_digit() || *c == '-' => false,
        Some('.') => !chars.get(end + 1).is_some_and(|c| c.is_ascii_digit()),
        Some(_) => true,
    }
}

/// Would this character read as part of the same written number?
fn continues_a_number(c: char) -> bool {
    c.is_ascii_digit() || c == '.' || c == '-'
}

/// Looks for a maximal digit run of exactly 11 digits.
fn contains_bare_cpf(chars: &[char]) -> bool {
    let mut run = 0;
    for c in chars {
        if c.is_ascii_digit() {
            run += 1;
        } else {
            if run == CPF_DIGITS {
                return true;
            }
            run = 0;
        }
    }
    run == CPF_DIGITS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carries(text: &str) -> bool {
        carries_unredacted_cpf(text)
    }

    #[test]
    fn the_dotted_shape_is_detected() {
        assert!(carries("meu cpf é 123.456.789-00"));
        assert!(carries("123.456.789-00"));
        assert!(carries("(123.456.789-00)"));
        // A sentence-ending period stays outside the number.
        assert!(carries("cpf: 123.456.789-00."));
    }

    #[test]
    fn the_bare_shape_is_detected() {
        assert!(carries("cpf 12345678901"));
        assert!(carries("12345678901"));
    }

    #[test]
    fn longer_numbers_are_not_a_cpf_with_a_digit_in_front() {
        assert!(!carries("1123.456.789-001"));
        assert!(!carries("123456789012"));
        assert!(!carries("1700000000000")); // a millisecond timestamp
    }

    #[test]
    fn a_full_stop_after_the_number_is_not_a_fourth_digit_group() {
        assert!(carries("meu cpf é 123.456.789-00."));
        // But a period with digits after it is a continuation of the number,
        // not punctuation.
        assert!(!carries("meu cpf é 123.456.789-00.5"));
    }

    #[test]
    fn shorter_numbers_are_not_cpfs() {
        assert!(!carries("1234567890"));
        assert!(!carries("123.456.789-0"));
    }

    #[test]
    fn decimal_points_do_not_split_a_cpf_in_half() {
        // "1.0" must not be read as the tail of a dotted CPF, nor the
        // leading "1" of one.
        assert!(!carries("version 1.0 is out"));
    }

    #[test]
    fn a_decimal_with_eleven_digits_is_indistinguishable_from_a_bare_cpf() {
        // Documented cost of a shape rule: the digits of a constant are
        // eleven digits. Asserted so the limitation cannot drift silently.
        assert!(carries("pi is 3.14159265358"));
        assert!(!carries("pi is 3.14159265")); // eight digits: not a CPF
    }

    #[test]
    fn redacted_text_passes() {
        assert!(!carries("cpf: ***.***.***-**"));
        assert!(!carries("cpf: <redacted>"));
        assert!(!carries("nothing numeric here"));
        assert!(!carries(""));
    }

    #[test]
    fn the_cpf_is_looked_for_in_both_halves_of_the_turn() {
        let rule = NoUnredactedCpf;
        let clean_input = || Turn("what is my balance?".to_string(), "…".to_string());

        let user_side = Turn("my cpf is 123.456.789-00".to_string(), "noted".to_string());
        let response_side = Turn(
            "what is my cpf?".to_string(),
            "your cpf is 12345678901".to_string(),
        );

        assert!(rule.check(&user_side).is_err());
        assert!(rule.check(&response_side).is_err());
        assert!(rule.check(&clean_input()).is_ok());
    }

    #[test]
    fn the_rejection_names_the_rule_and_the_reason() {
        let rule = NoUnredactedCpf;
        let err = rule.check(&Turn("12345678901".to_string(), "ok".to_string())).unwrap_err();
        assert!(err.contains("unredacted document number"), "{err}");
    }

    struct Turn(String, String);

    impl VerifiableRecord for Turn {
        fn user_input(&self) -> &str {
            &self.0
        }

        fn response(&self) -> &str {
            &self.1
        }
    }
}
