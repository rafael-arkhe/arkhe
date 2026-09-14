#!/usr/bin/env python3
"""
ARKHE-ZEITGEIST v4.2.1 — Módulo consolidado com todas as correções.
Executa suíte de testes sintéticos end-to-end.
"""
import numpy as np
from collections import deque
from scipy.linalg import eigvalsh
from scipy import stats


class IncrementalSVD:
    def __init__(self, n_rows, max_rank=20, reorth_every=1000):
        self.n_rows = n_rows
        self.max_rank = max_rank
        self.reorth_every = reorth_every
        self.k = 0
        self.U = np.zeros((n_rows, max_rank))
        self.s = np.zeros(max_rank)
        self.Vt = np.zeros((max_rank, 0))
        self._n_cols = 0
        self._steps_since_reorth = 0

    def add_column(self, col):
        col = np.asarray(col).flatten()
        assert len(col) == self.n_rows
        self._n_cols += 1
        self._steps_since_reorth += 1

        if self.k == 0:
            norm = np.linalg.norm(col)
            if norm > 1e-12:
                self.U[:, 0] = col / norm
                self.s[0] = norm
                self.Vt = np.zeros((self.max_rank, 1))
                self.Vt[0, 0] = 1.0
                self.k = 1
            return

        k = self.k
        proj = self.U[:, :k].T @ col
        residual = col - self.U[:, :k] @ proj
        norm_r = np.linalg.norm(residual)

        B = np.zeros((k + 1, k + 1))
        B[:k, :k] = np.diag(self.s[:k])
        B[:k, k] = proj
        B[k, k] = norm_r

        Ub, sb, Vtb = np.linalg.svd(B, full_matrices=False)
        new_k = min(k + 1, self.max_rank)

        # U: matriz aumentada
        U_aug = np.zeros((self.n_rows, k + 1))
        U_aug[:, :k] = self.U[:, :k]
        if norm_r > 1e-12:
            U_aug[:, k] = residual / norm_r
        elif new_k > k:
            q = np.random.randn(self.n_rows)
            q -= U_aug[:, :k] @ (U_aug[:, :k].T @ q)
            qn = np.linalg.norm(q)
            if qn > 1e-12:
                U_aug[:, k] = q / qn
        U_new = U_aug @ Ub[:, :new_k]

        # Vt: extended
        extended = np.zeros((k + 1, self._n_cols))
        if k > 0:
            extended[:k, :self._n_cols - 1] = self.Vt[:k, :]
        extended[k, self._n_cols - 1] = 1.0
        Vt_new = Vtb[:new_k, :] @ extended

        self.U[:, :new_k] = U_new
        self.s[:new_k] = sb[:new_k]
        if self.Vt.shape[1] != self._n_cols:
            self.Vt = np.zeros((self.max_rank, self._n_cols))
        self.Vt[:new_k, :] = Vt_new
        self.k = new_k

        # Truncamento
        limiar = max(1e-12, 1e-10 * self.s[0]) if self.s[0] > 0 else 1e-12
        keep = self.s[:self.k] > limiar
        if not np.any(keep):
            keep[0] = True
        if not np.all(keep):
            self.U = self.U[:, :self.k][:, keep]
            self.s = self.s[:self.k][keep]
            self.Vt = self.Vt[:self.k, :][keep, :]
            self.k = int(keep.sum())

        # Reortogonalização periódica
        if self.reorth_every is not None and self._steps_since_reorth >= self.reorth_every:
            self._reorthogonalize()

    def _reorthogonalize(self):
        if self.k < 2:
            self._steps_since_reorth = 0
            return
        Q, R = np.linalg.qr(self.U[:, :self.k])
        M = R @ np.diag(self.s[:self.k]) @ self.Vt[:self.k, :]
        Um, sm, Vtm = np.linalg.svd(M, full_matrices=False)
        new_k = min(len(sm), self.k)
        self.U[:, :new_k] = Q @ Um[:, :new_k]
        self.s[:new_k] = sm[:new_k]
        self.Vt[:new_k, :] = Vtm[:new_k, :]
        self.k = new_k
        self._steps_since_reorth = 0

    def get_entropy(self):
        if self.k == 0:
            return 0.0
        p = self.s[:self.k]**2 / (np.sum(self.s[:self.k]**2) + 1e-12)
        return float(-np.sum(p * np.log2(p + 1e-12)))

    def get_effective_rank(self):
        h = self.get_entropy()
        return 2.0 ** h if h > 0 else 1.0

    def spectral_state(self):
        if self.k == 0:
            return {'entropy': 0.0, 'effective_rank': 1.0,
                    'dominant_ratio': 1.0, 'eigenvalues': np.array([]),
                    'rank': 0, 'n_experiences': self._n_cols}
        p = self.s[:self.k]**2 / (np.sum(self.s[:self.k]**2) + 1e-12)
        h = float(-np.sum(p * np.log2(p + 1e-12)))
        return {
            'entropy': h,
            'effective_rank': 2.0 ** h,
            'dominant_ratio': float(np.max(p)),
            'eigenvalues': p,
            'rank': self.k,
            'n_experiences': self._n_cols,
        }

    def reconstruct(self):
        if self.k == 0:
            return np.zeros((self.n_rows, self._n_cols))
        return self.U[:, :self.k] @ np.diag(self.s[:self.k]) @ self.Vt[:self.k, :]


class AdaptiveCUSUM:
    def __init__(self, window=30, target_fpr=0.005, burn_in=40,
                 min_threshold=0.5, max_threshold=5.0):
        self.window = window
        self.target_fpr = target_fpr
        self.burn_in = burn_in
        self.min_threshold = min_threshold
        self.max_threshold = max_threshold
        self._history = []
        self._cumsum = 0.0
        self._threshold = 3.0
        self._changes = []

    def update(self, value):
        self._history.append(value)
        if len(self._history) < self.burn_in + self.window:
            return False, self._threshold
        recent = np.array(self._history[-self.window:])
        mean, std = np.mean(recent), np.std(recent)
        if std < 1e-12:
            return False, self._threshold
        th = np.linspace(self.min_threshold, self.max_threshold, 50)
        fprs = 2 * (1 - stats.norm.cdf(th))
        best = th[np.argmin(np.abs(fprs - self.target_fpr))]
        self._threshold = float(np.clip(best, self.min_threshold, self.max_threshold))
        z = (value - mean) / std
        self._cumsum = max(0.0, self._cumsum + abs(z) - self._threshold * 0.4)
        if self._cumsum > self._threshold:
            self._changes.append(len(self._history) - 1)
            self._cumsum = 0.0
            return True, self._threshold
        return False, self._threshold

    def detect_changes(self, data):
        self._history.clear(); self._cumsum = 0.0; self._changes.clear()
        for i, v in enumerate(data):
            self.update(v)
        return list(self._changes)


class SafeCorePolynomialBridge:
    def __init__(self, n_dims=20, max_rank=20, max_history=1000):
        self.n_dims = n_dims
        self._svd = IncrementalSVD(n_dims, max_rank)
        self._metrics = deque(maxlen=max_history)
        self._states = deque(maxlen=max_history)
        self._detector = AdaptiveCUSUM()
        self._step = 0
        self._last_entropy = 0.0
        self._last_time = 0.0
        self._regime_changes = []

    def record_experience(self, vec, success):
        vec = np.asarray(vec).flatten()
        assert len(vec) == self.n_dims
        weighted = vec * (0.5 + 0.5 * success)
        self._svd.add_column(weighted)
        self._step += 1
        self._last_time += 1.0

        st = self._svd.spectral_state()
        self._states.append(st)
        entropy = st['entropy']
        dsdt = entropy - self._last_entropy if self._step > 1 else 0.0
        self._last_entropy = entropy

        det, _ = self._detector.update(dsdt)
        if det:
            self._regime_changes.append(self._step)

        m = {
            'step': self._step,
            'entropy': entropy,
            'effective_rank': st['effective_rank'],
            'dominant_ratio': st['dominant_ratio'],
            'dsdt': dsdt,
            'svd_rank': st['rank'],
            'regime_change': det,
        }
        self._metrics.append(m)
        return m

    def get_metric_history(self):
        return list(self._metrics)


# ============================================================
# TESTES
# ============================================================

def test_svd_reconstruction():
    np.random.seed(42)
    # max_rank = n_dim (rank completo): dados randômicos 20-dim têm rank 20,
    # truncar para 15 remove σ₁₆..σ₂₀ e a reconstrução é impossível < 1e-8.
    svd = IncrementalSVD(20, 20)
    cols = []
    for i in range(50):
        c = np.random.randn(20) * (1 + 0.5 * np.sin(i / 10))
        cols.append(c)
        svd.add_column(c)
    A = np.column_stack(cols)
    err = np.linalg.norm(A - svd.reconstruct())
    orth_u = np.linalg.norm(svd.U[:, :svd.k].T @ svd.U[:, :svd.k] - np.eye(svd.k))
    orth_v = np.linalg.norm(svd.Vt[:svd.k, :] @ svd.Vt[:svd.k, :].T - np.eye(svd.k))
    ok = err < 1e-8 and orth_u < 1e-8 and orth_v < 1e-8
    print(f"TEST SVD: err={err:.2e}, orth_u={orth_u:.2e}, orth_v={orth_v:.2e} -> {'PASS' if ok else 'FAIL'}")
    return ok


def test_regime_detection():
    n_sim, n_steps, cps, tol = 100, 300, [80, 160, 240], 15
    tp = fp = fn = 0
    for sim in range(n_sim):
        np.random.seed(sim)
        data = np.concatenate([
            np.random.normal(0.01, 0.03, cps[0]),
            np.random.normal(0.05, 0.03, cps[1] - cps[0]),
            np.random.normal(-0.03, 0.03, cps[2] - cps[1]),
            np.random.normal(0.08, 0.03, n_steps - cps[2]),
        ])
        det = AdaptiveCUSUM().detect_changes(data)
        for cp in cps:
            if any(abs(d - cp) < tol for d in det):
                tp += 1
            else:
                fn += 1
        for d in det:
            if all(abs(d - cp) >= tol for cp in cps):
                fp += 1
    prec = tp / (tp + fp) if (tp + fp) else 0
    rec = tp / (tp + fn) if (tp + fn) else 0
    f1 = 2 * prec * rec / (prec + rec) if (prec + rec) else 0
    print(f"TEST REGIME: P={prec:.3f}, R={rec:.3f}, F1={f1:.3f}, TP={tp}, FP={fp}, FN={fn}")
    return prec >= 0.75 and rec >= 0.70 and f1 >= 0.72


def test_memory_bounded():
    b = SafeCorePolynomialBridge(n_dims=10, max_rank=5, max_history=50)
    for i in range(200):
        b.record_experience(np.random.randn(10), 0.5)
    ok = len(b._metrics) == 50 and len(b._states) == 50
    print(f"TEST MEMORY: len(metrics)={len(b._metrics)}, len(states)={len(b._states)} -> {'PASS' if ok else 'FAIL'}")
    return ok


def test_rank_base2():
    svd = IncrementalSVD(10, 8)
    for i in range(8):
        c = np.zeros(10); c[i] = 1.0
        svd.add_column(c)
    er = svd.get_effective_rank()
    ok = abs(er - 8.0) < 0.5
    print(f"TEST RANK: effective_rank={er:.4f} (esperado ~8.0) -> {'PASS' if ok else 'FAIL'}")
    return ok


if __name__ == "__main__":
    print("=" * 60)
    print("ARKHE-ZEITGEIST v4.2.1 -- SUITE DE TESTES")
    print("=" * 60)
    results = [
        test_svd_reconstruction(),
        test_regime_detection(),
        test_memory_bounded(),
        test_rank_base2(),
    ]
    passed = sum(results)
    print(f"\nTotal: {passed}/{len(results)} passaram")