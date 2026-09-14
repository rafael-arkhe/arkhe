#!/usr/bin/env python3
"""
AVALON TUT LINTER UNIFIED (tut_linter_unified.py)
===================================================
CI/CD module that validates all TUT-related claims in Avalon documents
and source files. Blocks publication if mathematical inconsistencies
are detected.

v2.1 Changes:
- Merged directory scanner (TUTLinter) with AST-based checks.
- validate_tut_consistency now requires explicit mode ('scalar' or 'ensemble').
- Added INV-SIGMA-01: blocks noise^2 entropy production formula.
- Added INV-DFT-01: flags unverified DFT assumptions.
- Added INV-OPT-01/02: blocks phi-forcing and temperature/horizon confusion.

Invariants: INV-TUT-01, INV-TUT-02, INV-PHI-01, INV-DATA-01, INV-MOCK-01,
           INV-SIGMA-01, INV-DFT-01, INV-OPT-01, INV-OPT-02
"""

import numpy as np
import re
import ast
from pathlib import Path
from typing import List, Dict, Optional


class TUTLinterUnified:
    """
    Mathematical linter for Thermodynamic Uncertainty Theorem claims.
    v2.1: Unified scanner + AST validator.
    """

    PHI = (1 + np.sqrt(5)) / 2
    ONE_OVER_PHI = 1.0 / PHI

    # Forbidden patterns
    FORBIDDEN_SIGMA_PATTERNS = [
        r"np\.sum\s*\(\s*noise\s*\*\*\s*2\s*\)\s*/\s*\(\s*2\s*\*\s*self\.T\s*\*\s*dt\s*\)",
        r"Sigma\s*\+\=\s*np\.sum\s*\(\s*noise\s*\*\*\s*2\s*\)",
        r"noise\s*\*\*\s*2\s*/\s*\(\s*2\s*\*\s*T\s*\*\s*dt\s*\)",
    ]

    FORBIDDEN_OPT_PATTERNS = [
        r"epsilon.*\-.*1/phi.*\*\*.*2",
        r"eps.*\-.*INV_PHI.*\*\*.*2",
        r"objective.*phi.*distance",
        r"min.*\|.*eps.*\-.*phi",
    ]

    # Tokens that indicate a forbidden pattern is being *described/negated*
    # rather than asserted (e.g. invariant definitions, lint messages,
    # bug demonstrations). Matches near these tokens are not violations.
    NEGATION_TOKENS = [
        r'\bforbidden\b', r'\bblock(?:ed|ing|s)?\b', r'\bviolat\w*\b',
        r'\bdetect\w*\b', r'\bfalse\b', r'\bbug\b', r'\bproblem\b',
        r'\bnot\b', r'\binvalid\b', r'\bincorrect\b', r'\bwrong\b',
    ]

    def __init__(self, tolerance: float = 1e-6):
        self.tolerance = tolerance
        self.violations: List[Dict] = []

    # ------------------------------------------------------------------
    # Context helpers
    # ------------------------------------------------------------------
    @staticmethod
    def _is_self_match(source: str) -> bool:
        """Return True if 'source' is this module's own file (skip self-lint)."""
        try:
            own = str(Path(__file__).resolve())
        except NameError:
            return False
        return source == own or Path(source).resolve() == Path(own)

    @staticmethod
    def _in_negated_context(text: str, span: tuple) -> bool:
        """
        Return True if the match at 'span' appears inside a context that
        negates or describes the pattern rather than asserting it.
        """
        lo = max(0, span[0] - 80)
        hi = min(len(text), span[1] + 80)
        context = text[lo:hi]
        return any(
            re.search(tok, context, re.IGNORECASE)
            for tok in TUTLinterUnified.NEGATION_TOKENS
        )

    @staticmethod
    def _first_unnegated_match(pattern: str, text: str) -> bool:
        """True if 'pattern' matches somewhere NOT in a negated context."""
        flags = re.IGNORECASE
        for m in re.finditer(pattern, text, flags):
            if not TUTLinterUnified._in_negated_context(text, m.span()):
                return True
        return False

    @staticmethod
    def _strip_python_strings(text: str) -> str:
        """
        Blank out string literals, docstrings, and comments in Python source.

        Text-based checks then see only real code, so a module's own
        docstrings and string-literal patterns cannot match themselves.
        """
        import tokenize
        import io

        def _idx(line: int, col: int) -> int:
            cum = 0
            for i, ln in enumerate(text.splitlines(keepends=True)):
                if i == line - 1:
                    return cum + col
                cum += len(ln)
            return len(text)

        chars = list(text)
        try:
            for tok in tokenize.generate_tokens(io.StringIO(text).readline):
                if tok.type in (tokenize.STRING, tokenize.COMMENT):
                    s = _idx(tok.start[0], tok.start[1])
                    e = _idx(tok.end[0], tok.end[1])
                    for k in range(s, min(e, len(chars))):
                        if chars[k] not in '\r\n':
                            chars[k] = ' '
        except (tokenize.TokenError, IndentationError):
            return text
        return ''.join(chars)

    # ------------------------------------------------------------------
    # Core TUT validation
    # ------------------------------------------------------------------
    def validate_tut_consistency(
        self,
        sigma: float,
        eps_sq: float,
        source: str = "unknown",
        mode: str = "scalar",
    ) -> bool:
        """
        Check if Sigma and eps_sq are consistent with TUT.

        mode='scalar':   Checks deterministic relation eps_sq(sigma) for a single value.
                         Used for plotting the theoretical curve.
        mode='ensemble': Checks ensemble bound eps_sq = 1/<tanh(Sigma/2)> - 1.
                         Requires array of Sigma values; this method should NOT be used
                         for single scalar pairs.
        """
        if mode not in ("scalar", "ensemble"):
            self.violations.append({
                'type': 'INVALID_MODE',
                'source': source,
                'message': f"mode must be 'scalar' or 'ensemble', got '{mode}'"
            })
            return False

        if sigma <= 0:
            self.violations.append({
                'type': 'INVALID_ENTROPY',
                'source': source,
                'message': f'Sigma={sigma} must be positive'
            })
            return False

        if mode == "scalar":
            # Deterministic curve: eps_sq(sigma) = 1/tanh(sigma/2) - 1
            eps_computed = 1.0 / np.tanh(sigma / 2.0) - 1.0
            if not np.isclose(eps_computed, eps_sq, rtol=self.tolerance):
                self.violations.append({
                    'type': 'TUT_SCALAR_INCONSISTENCY',
                    'source': source,
                    'sigma_claimed': sigma,
                    'eps_sq_claimed': eps_sq,
                    'eps_sq_computed': eps_computed,
                    'message': (
                        f"[scalar mode] Sigma={sigma:.6f} -> eps_sq={eps_computed:.6f}, "
                        f"but claimed {eps_sq:.6f}"
                    )
                })
                return False

        elif mode == "ensemble":
            # This method should NOT be called with scalar sigma for ensemble mode.
            # The caller should pass the FULL array to compute_mean_tanh_bound().
            self.violations.append({
                'type': 'ENSEMBLE_SCALAR_MISMATCH',
                'source': source,
                'message': (
                    "ensemble mode requires computing <tanh(Sigma/2)> over an array. "
                    "Use compute_mean_tanh_bound() instead of validate_tut_consistency()."
                )
            })
            return False

        return True

    def compute_mean_tanh_bound(self, sigma_array: np.ndarray, source: str = "unknown") -> Optional[float]:
        """Compute the ensemble TUT bound from an array of Sigma values."""
        sigma_array = np.asarray(sigma_array).flatten()
        if len(sigma_array) == 0:
            self.violations.append({
                'type': 'EMPTY_ENSEMBLE',
                'source': source,
                'message': 'Empty sigma array provided for ensemble bound'
            })
            return None

        if not np.all(sigma_array > 0):
            self.violations.append({
                'type': 'NONPOSITIVE_ENTROPY',
                'source': source,
                'message': 'All Sigma values must be positive for TUT bound'
            })
            return None

        avg_tanh = float(np.mean(np.tanh(sigma_array / 2.0)))
        if avg_tanh <= 0:
            self.violations.append({
                'type': 'UNDEFINED_BOUND',
                'source': source,
                'message': f'<tanh(Sigma/2)> = {avg_tanh} <= 0; bound undefined'
            })
            return None

        return 1.0 / avg_tanh - 1.0

    # ------------------------------------------------------------------
    # Text / regex checks
    # ------------------------------------------------------------------
    def check_ln3_claim(self, text: str, source: str) -> None:
        """Block S_opt = k_B ln(3) or equivalent (unless negated/described)."""
        if self._is_self_match(source):
            return
        patterns = [
            r'S[_\s]*opt\s*=\s*k[_\s]*B\s*ln\s*\(\s*3\s*\)',
            r'S_opt\s*=\s*k_B\s*ln\s*3',
            r'entropy.*optimal.*ln\s*\(\s*3\s*\)',
            r'ln\s*3.*entropy.*optimal',
        ]
        for pat in patterns:
            if self._first_unnegated_match(pat, text):
                self.violations.append({
                    'type': 'FORBIDDEN_LN3',
                    'source': source,
                    'message': 'Forbidden claim: S_opt = k_B ln(3) detected'
                })

    def check_phi_notation(self, text: str, source: str) -> None:
        """Block 1/phi - 1 = 0.618 (unless negated/described)."""
        pat = r'1/\s*phi\s*[-\u2212]\s*1\s*=\s*0\.618'
        if self._first_unnegated_match(pat, text):
            self.violations.append({
                'type': 'FORBIDDEN_PHI_NOTATION',
                'source': source,
                'message': (
                    f'Forbidden: 1/phi - 1 = 0.618. '
                    f'Correct: 1/phi = {self.ONE_OVER_PHI:.6f}; '
                    f'1/phi - 1 = {self.ONE_OVER_PHI - 1:.6f}'
                )
            })

    def check_hardcoded_arrays(self, text: str, source: str) -> None:
        """Flag suspicious hardcoded arrays that may be fabricated data."""
        suspicious = re.findall(
            r'(?:(?:computed|simulated|results?|data)[:]?\s*)?'
            r'\[\s*(\d+\.\d{3,}\s*,\s*){3,}\d+\.\d{3,}\s*\]',
            text, re.IGNORECASE
        )
        if suspicious:
            self.violations.append({
                'type': 'SUSPICIOUS_HARDCODED_ARRAY',
                'source': source,
                'message': f'Hardcoded array with many decimals may be fabricated: {suspicious[0][:80]}'
            })

    def check_mock_misrepresentation(self, text: str, source: str) -> None:
        """Flag mock functions presented without clear labeling (unless negated/described)."""
        has_mock = re.search(r'\bmock\b', text, re.IGNORECASE)
        has_results = re.search(r'\bresults?\b|\bfinding\b|\bevidence\b', text, re.IGNORECASE)
        has_placeholder = re.search(r'\bplaceholders?\b|\bsimulated\b|\bnot\s+physical\b', text, re.IGNORECASE)

        if has_mock and has_results and not has_placeholder:
            self.violations.append({
                'type': 'MOCK_MISREPRESENTATION',
                'source': source,
                'message': 'Mock function may be presented as physical result. Add explicit placeholder label.'
            })

    def check_sigma_formula(self, text: str, source: str) -> None:
        """INV-SIGMA-01: Block noise^2 entropy production (unless negated/described)."""
        for pat in self.FORBIDDEN_SIGMA_PATTERNS:
            if self._first_unnegated_match(pat, text):
                self.violations.append({
                    'type': 'FORBIDDEN_SIGMA_FORMULA',
                    'source': source,
                    'message': (
                        'INV-SIGMA-01 VIOLATION: Detected noise^2/(2T*dt) as entropy production. '
                        'Use Seifert medium entropy: dSigma = Sum F*dtheta / T.'
                    )
                })
                break

    def check_dft_assumption(self, text: str, source: str) -> None:
        """INV-DFT-01: Flag unverified DFT assumptions."""
        has_dft_claim = re.search(
            r'\b(TUT|thermodynamic uncertainty)\b.*\b(bound|theorem|holds|applies)\b',
            text, re.IGNORECASE
        )
        has_dft_verify = re.search(
            r'\b(dft_assumption_verified|DFT verified|detailed fluctuation theorem verified)\b',
            text, re.IGNORECASE
        )
        if has_dft_claim and not has_dft_verify and not self._is_self_match(source):
            self.violations.append({
                'type': 'UNVERIFIED_DFT_ASSUMPTION',
                'source': source,
                'message': (
                    'INV-DFT-01: TUT bound claimed without explicit DFT verification flag. '
                    'Set dft_assumption_verified=True only after independent audit.'
                )
            })

    def check_optimizer_objective(self, text: str, source: str) -> None:
        """INV-OPT-01/02: Block phi-forcing and temperature/horizon confusion."""
        for pat in self.FORBIDDEN_OPT_PATTERNS:
            if self._first_unnegated_match(pat, text):
                self.violations.append({
                    'type': 'FORBIDDEN_PHI_OBJECTIVE',
                    'source': source,
                    'message': (
                        'INV-OPT-01 VIOLATION: Optimizer appears to force result toward phi. '
                        'Objective must be eps_sq_TUT alone, NOT distance to 1/phi.'
                    )
                })
                break

        # Check for temperature/horizon confusion
        if re.search(r'temperature.*horizon|horizon.*temperature', text, re.IGNORECASE):
            if not re.search(r'distinct|separate|different', text, re.IGNORECASE):
                self.violations.append({
                    'type': 'TEMPERATURE_HORIZON_CONFUSION',
                    'source': source,
                    'message': (
                        'INV-OPT-02: temperature and horizon may be conflated. '
                        'They must be explicitly distinct parameters.'
                    )
                })

    # ------------------------------------------------------------------
    # AST-level forbidden pattern checks (audit CRITICAL: previously empty)
    # ------------------------------------------------------------------
    @staticmethod
    def _node_segment(raw_text: str, node: ast.AST) -> str:
        """Source text segment for an AST node ('' if unavailable)."""
        return ast.get_source_segment(raw_text, node) or ""

    @staticmethod
    def _is_pow2(node: ast.AST) -> bool:
        """True if node is an exponentiation with exponent ~2."""
        return (
            isinstance(node, ast.BinOp)
            and isinstance(node.op, ast.Pow)
            and isinstance(node.right, ast.Constant)
            and isinstance(node.right.value, (int, float))
            and abs(node.right.value - 2.0) < 1e-9
        )

    @classmethod
    def _node_refs_inv_phi(cls, raw_text: str, node: ast.AST) -> bool:
        """True if the node's source references INV_PHI / 1/phi / 0.618."""
        seg = cls._node_segment(raw_text, node)
        return bool(
            re.search(
                r'\b(INV_PHI|inv_?phi|phi_ref)\b|1\s*/\s*phi|0\.618',
                seg,
                re.IGNORECASE,
            )
        )

    @classmethod
    def _node_is_log3(cls, node: ast.AST) -> bool:
        """True if node structurally evaluates to log/ln(3)."""
        if isinstance(node, ast.Call):
            fn = node.func
            name = None
            if isinstance(fn, ast.Name):
                name = fn.id
            elif isinstance(fn, ast.Attribute):
                name = fn.attr
            if name.lower() in ('log', 'ln', 'log2', 'log10', 'logn'):
                args = node.args
                return (
                    len(args) >= 1
                    and isinstance(args[0], ast.Constant)
                    and isinstance(args[0].value, (int, float))
                    and abs(args[0].value - 3.0) < 1e-9
                )
        return False

    @staticmethod
    def _target_matches_entropy(node: ast.AST) -> bool:
        """Match assignment targets whose name suggests entropy/thermal capacity."""
        names = []
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for t in targets:
                name = getattr(t, 'id', '') if isinstance(t, ast.Name) else ''
                names.append(name)
        elif isinstance(node, ast.AugAssign):
            names.append(getattr(node.target, 'id', ''))
        return any(
            re.search(r'^(s|sigma|s_?opt|s_?max|s_?th|entropy|delta.?s)$', n, re.IGNORECASE)
            for n in names
        )

    def _check_ast_forbidden_patterns(self, raw_text: str, source: str) -> None:
        """
        AST-level invariant checks on Python source.

        Catches invSIGMA-01 (noise^2 entropy accumulation), INV-OPT-01
        (|eps - 1/phi|^2 objectives), and INV-TUT-02 (S_opt = k_B ln(3))
        even when formatting or line-wrapping hides the plain-text match.

        Own-file (self) and negated/descriptive contexts are skipped just
        like the text-based checks.
        """
        if self._is_self_match(source):
            return

        try:
            tree = ast.parse(raw_text)
        except SyntaxError:
            return

        for node in ast.walk(tree):
            # --- INV-OPT-01: phi-forcing squared objective ---------------
            if self._is_pow2(node):
                base = node.left
                # (X - INV_PHI)**2  |  abs(X - 1/phi)**2
                if self._node_refs_inv_phi(raw_text, base):
                    seg = self._node_segment(raw_text, node)
                    start = raw_text.find(seg)
                    span = (start, start + len(seg))
                    if (
                        start >= 0
                        and not self._in_negated_context(raw_text, span)
                    ):
                        self.violations.append({
                            'type': 'FORBIDDEN_PHI_OBJECTIVE',
                            'source': source,
                            'message': (
                                'INV-OPT-01 VIOLATION (AST): squared objective '
                                'references 1/phi (phi-forcing). Objective must be '
                                'eps_sq_TUT alone, NOT distance to 1/phi.'
                            )
                        })

            # --- INV-TUT-02: S_opt = k_B ln(3) ----------------------------
            elif isinstance(node, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
                value = None
                if isinstance(node, ast.AugAssign):
                    value = node.value
                else:
                    value = getattr(node, 'value', None)
                flag_ln3 = False
                if isinstance(value, ast.BinOp) and isinstance(value.op, ast.Mult):
                    left, right = value.left, value.right
                    # k_B * log(3)  or  log(3) * k_B
                    kname = (
                        (isinstance(left, ast.Name) and re.fullmatch(r'k_?b', left.id, re.IGNORECASE))
                        or (isinstance(right, ast.Name) and re.fullmatch(r'k_?b', right.id, re.IGNORECASE))
                    )
                    if kname and (
                        self._node_is_log3(left) or self._node_is_log3(right)
                    ):
                        flag_ln3 = True
                elif isinstance(value, ast.Call) and self._node_is_log3(value):
                    # plain log(3) assigned to an entropy symbol
                    flag_ln3 = True
                if flag_ln3 and self._target_matches_entropy(node):
                    seg = self._node_segment(raw_text, node)
                    start = raw_text.find(seg)
                    span = (start, start + len(seg))
                    if (
                        start >= 0
                        and not self._in_negated_context(raw_text, span)
                    ):
                        self.violations.append({
                            'type': 'FORBIDDEN_ENTROPY_OPTIMUM',
                            'source': source,
                            'message': (
                                'INV-TUT-02 VIOLATION (AST): S_opt = k_B ln(3) is '
                                'a forbidden hardcoded entropy optimum.'
                            )
                        })

            # --- INV-SIGMA-01: noise**2 entropy accumulation --------------
            if isinstance(node, ast.AugAssign):
                target = getattr(node.target, 'id', '')
                if re.match(r'^(sigma|Sigma|entropy|S)$', target) and isinstance(
                    node.value, ast.BinOp
                ):
                    v = node.value
                    if isinstance(v.op, (ast.Add, ast.Add)):
                        # Sigma += X ; X contains noise**2 / (2*T*dt)
                        lhs, rhs = v.left, v.right
                        seg = self._node_segment(raw_text, node)
                        if (
                            self._is_pow2(lhs) or self._is_pow2(rhs)
                        ) and re.search(r'noise', seg, re.IGNORECASE):
                            start = raw_text.find(seg)
                            span = (start, start + len(seg))
                            if (
                                start >= 0
                                and not self._in_negated_context(raw_text, span)
                            ):
                                self.violations.append({
                                    'type': 'FORBIDDEN_SIGMA_FORMULA',
                                    'source': source,
                                    'message': (
                                        'INV-SIGMA-01 VIOLATION (AST): noise^2 '
                                        'entropy production accumulation detected. '
                                        'Use Seifert medium entropy.'
                                    )
                                })

    # ------------------------------------------------------------------
    # File scanning
    # ------------------------------------------------------------------
    def scan_file(self, filepath: str) -> None:
        """Scan a single file for violations."""
        path = Path(filepath)
        if not path.exists():
            return

        raw_text = path.read_text(encoding='utf-8')
        source = str(path)

        # For Python sources, blank out strings/docstrings/comments so the
        # text checks match *code*, not prose or literal pattern definitions.
        text = self._strip_python_strings(raw_text) if path.suffix == '.py' else raw_text

        self.check_ln3_claim(text, source)
        self.check_phi_notation(text, source)
        self.check_hardcoded_arrays(text, source)
        self.check_mock_misrepresentation(text, source)
        self.check_sigma_formula(text, source)
        self.check_dft_assumption(text, source)
        self.check_optimizer_objective(text, source)

        # AST-level checks on the raw source (structure-aware, formatting-proof).
        if path.suffix == '.py':
            self._check_ast_forbidden_patterns(raw_text, source)

        # Scalar TUT pair checks (for deterministic curve documentation)
        pairs = re.findall(
            r'Sigma\s*=\s*(\d+\.?\d*)[,;:]?\s*eps(?:ilon)?[_\s]?sq\s*=\s*(\d+\.?\d*)',
            raw_text, re.IGNORECASE
        )
        for s, e in pairs:
            self.validate_tut_consistency(float(s), float(e), source, mode="scalar")

    def scan_directory(self, dirpath: str, pattern: str = "*.md") -> None:
        """Scan all matching files in directory."""
        for f in Path(dirpath).rglob(pattern):
            self.scan_file(str(f))

    def report(self) -> str:
        """Generate validation report."""
        if not self.violations:
            return "ALL CLAIMS VALIDATED - Publication approved"

        lines = [f"{len(self.violations)} VIOLATION(S) DETECTED - Publication BLOCKED", "=" * 60]
        for i, v in enumerate(self.violations, 1):
            lines.append(f"\n[{i}] {v['type']} in '{v['source']}':")
            lines.append(f"    {v['message']}")
        return "\n".join(lines)

    def exit_code(self) -> int:
        """Return 0 if clean, 1 if violations found."""
        return 1 if self.violations else 0


def main():
    """CLI entry point for CI integration."""
    import sys
    import argparse

    parser = argparse.ArgumentParser(description="Avalon TUT Linter Unified v2.1")
    parser.add_argument("paths", nargs="+", help="Files or directories to scan")
    parser.add_argument("--pattern", default="*.md", help="File pattern for directories")
    parser.add_argument("--py", action="store_true", help="Also scan *.py files")
    args = parser.parse_args()

    linter = TUTLinterUnified()
    for p in args.paths:
        path = Path(p)
        if path.is_dir():
            linter.scan_directory(str(path), args.pattern)
            if args.py:
                linter.scan_directory(str(path), "*.py")
        else:
            linter.scan_file(str(p))

    print(linter.report())
    sys.exit(linter.exit_code())


if __name__ == "__main__":
    main()