//! Pure helpers around the verification hand-off.
//!
//! `build_check_script` renders the instruction we pipe to the external solver.
//! `parse_verdict` reads the solver's answer back. Keeping the *shape* of the
//! solver protocol here (and nothing about how the solver runs) means the
//! orchestrator and both bridge implementations agree on one contract.

use crate::types::Hypothesis;

/// Render the check request handed to the external solver.
///
/// This is deliberately plain text: the solver is expected to parse it, run a
/// symbolic identity check, and answer with a `VERDICT:` line (see
/// [`parse_verdict`]). Rust never evaluates the equation.
pub fn build_check_script(h: &Hypothesis) -> String {
    let mut s = String::new();
    s.push_str("# ARKHE-SATURON symbolic identity check\n");
    s.push_str(&format!("# hypothesis: {}\n", h.id));
    s.push_str(&format!("# source: {}\n", h.arxiv_toon_id));
    for v in &h.variables {
        s.push_str(&format!(
            "var {} = symbol({:?})  # {} [{}]\n",
            v.symbol, v.symbol, v.name, v.unit
        ));
    }
    s.push_str(&format!("check_identity: {}\n", h.math_syntax));
    s.push_str("# expected reply: `VERDICT:PASS <detail>` or `VERDICT:FAIL:<reason>`\n");
    s
}

/// Parse a solver reply. The solver must emit a line of the form
/// `VERDICT:PASS [detail]` or `VERDICT:FAIL:<reason>` (or bare `VERDICT:FAIL`).
///
/// Returns `(passed, detail)` for the first `VERDICT:` line found, or an error
/// string if no well-formed verdict is present.
pub fn parse_verdict(output: &str) -> Result<(bool, String), String> {
    for raw in output.lines() {
        let line = raw.trim();
        let Some(rest) = line.strip_prefix("VERDICT:") else {
            continue;
        };
        if let Some(reason) = rest.strip_prefix("FAIL:") {
            return Ok((false, reason.trim().to_string()));
        }
        if rest == "FAIL" {
            return Ok((false, "identity does not hold".to_string()));
        }
        if let Some(detail) = rest.strip_prefix("PASS") {
            let detail = detail.trim();
            let detail = if detail.is_empty() {
                "identity holds".to_string()
            } else {
                detail.to_string()
            };
            return Ok((true, detail));
        }
        return Err(format!("malformed verdict: {line:?}"));
    }
    Err("no VERDICT line found in solver output".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{HypothesisStatus, PhysicalVariable};

    fn hyp() -> Hypothesis {
        Hypothesis {
            id: "HYP-9".to_string(),
            arxiv_toon_id: "TOON-A".to_string(),
            text: "t".to_string(),
            math_syntax: "S = k_B*c^3*A/(4*G*hbar)".to_string(),
            variables: vec![PhysicalVariable {
                name: "area".to_string(),
                symbol: "A".to_string(),
                unit: "m^2".to_string(),
                description: "horizon area".to_string(),
            }],
            status: HypothesisStatus::Proposed,
        }
    }

    #[test]
    fn script_contains_equation_and_vars() {
        let s = build_check_script(&hyp());
        assert!(s.contains("HYP-9"));
        assert!(s.contains("S = k_B*c^3*A/(4*G*hbar)"));
        assert!(s.contains("var A"));
    }

    #[test]
    fn parse_pass_with_detail() {
        assert_eq!(
            parse_verdict("noise\nVERDICT:PASS residual=0\nmore"),
            Ok((true, "residual=0".to_string()))
        );
    }

    #[test]
    fn parse_pass_bare() {
        assert_eq!(
            parse_verdict("VERDICT:PASS"),
            Ok((true, "identity holds".to_string()))
        );
    }

    #[test]
    fn parse_fail_with_reason() {
        assert_eq!(
            parse_verdict("VERDICT:FAIL:residual=1e-3"),
            Ok((false, "residual=1e-3".to_string()))
        );
    }

    #[test]
    fn parse_fail_bare() {
        assert_eq!(
            parse_verdict("VERDICT:FAIL"),
            Ok((false, "identity does not hold".to_string()))
        );
    }

    #[test]
    fn parse_missing_verdict_errors() {
        assert!(parse_verdict("just some output\n").is_err());
    }

    #[test]
    fn parse_malformed_verdict_errors() {
        assert!(parse_verdict("VERDICT:MAYBE").is_err());
    }
}
