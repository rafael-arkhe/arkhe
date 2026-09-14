"""# Fase 3 — Caminho Paralelo: Classificação Quântica (QKM/VQC)

**Disclaimer (honestidade científica).** Este notebook valida
*tecnicamente* o pipeline quântico sobre o ambiente da Fase 2
(features 68-dim RF + 20-dim ELF). O dataset é **sintético** com
rotulagem controlada — **nenhuma** vantagem quântica é reivindicada.
Qualquer alegação exige ≥10 h de dados reais calibrados (mesmo
conjunto para clássico e quântico).

Pipeline aqui demonstrado:

 1. Dataset rotulado (regime interno/sat, sim).
 2. **QKM**: kernel |⟨φ(x_i)|φ(x_j)⟩|² via ZZFeatureMap/Statevector;
    verificação PSD/simetria; SVM de kernel pré-computado.
 3. **VQC** experimental: ZZFeatureMap + ansatz, treino COBYLA.
 4. **Comparação honesta** QKM vs. SVC-RBF no MESMO feature space.
"""

import matplotlib
matplotlib.use('Agg')  # headless-safe (o notebook mantém o backend interativo)
import matplotlib.pyplot as _plt
_plt.show = lambda *a, **k: None   # sem janela em CLI

import sys
from pathlib import Path

for _base in (Path.cwd(), Path.cwd().parent, Path.cwd().parents[1]):
    if (Path(_base) / "arkhe_rf").is_dir():
        _b = str(Path(_base))
        if _b not in sys.path:
            sys.path.insert(0, _b)
        ROOT = Path(_b)
        break
else:
    ROOT = Path.cwd()

import numpy as np
import matplotlib.pyplot as plt

import arkhe_rf
from arkhe_rf.quantum_pipeline import (
    QuantumKernel,
    QuantumSVMPipeline,
    VQClassifier,
    compare_quantum_classical,
)
from arkhe_rf.config import RF_FEATURE_DIM, ELF_FEATURE_DIM

print("arkhe_rf", arkhe_rf.__version__,
      "| qiskit ok:", arkhe_rf.QISKIT_OK and __import__("qiskit").__version__)


# 1) Dataset sintético rotulado (formato RF+ELF da Fase 2)
DIM = RF_FEATURE_DIM + ELF_FEATURE_DIM
rng = np.random.default_rng(11)

def synthetic_binary(n):
    X = rng.standard_normal((n, DIM))
    # Regime "interno" vs "transiente": mistura linear + ruído de rótulo.
    score = X[:, 0] + 0.9 * X[:, 1] - 0.7 * X[:, 3] + 0.2 * X[:, 87]
    y = (score > 0).astype(int)
    return X, y

X_all, y_all = synthetic_binary(240)
print("X", X_all.shape, "| y classe 1:", y_all.mean().round(3))

plt.figure(figsize=(12, 3))
plt.subplot(1, 2, 1)
plt.hist(X_all[y_all == 0, 0], bins=24, alpha=0.5, label="classe 0")
plt.hist(X_all[y_all == 1, 0], bins=24, alpha=0.5, label="classe 1")
plt.title("feature[0] por classe (sintético)"); plt.legend(); plt.grid(alpha=0.3)
plt.subplot(1, 2, 2)
plt.scatter(X_all[:120, 0], X_all[:120, 1], c=y_all[:120], cmap="coolwarm", s=8)
plt.title("X[0] vs X[1]"); plt.grid(alpha=0.3)
plt.tight_layout(); plt.show()


# 2) QKM — kernel e diagnóstico (simetria/PSD)
N_Q = 24
qk = QuantumKernel(n_qubits=4, reps=1, seed=1)
K = qk.kernel(X_all[:N_Q, :4])   # embedding direto p/ demo do kernel

sym = np.allclose(K, K.T, atol=1e-9)
eig = np.linalg.eigvalsh(K)
print("kernel simétrico:", sym, "| autovalor min:", eig.min().round(6))
print("diag = 1:", np.allclose(np.diag(K), 1.0, atol=1e-8))

plt.figure(figsize=(6, 5))
im = plt.imshow(K, cmap="viridis", vmin=0, vmax=1)
plt.colorbar(im, fraction=0.046)
plt.title(f"QKM K = |<φ|φ>|² — {N_Q} amostras"); plt.grid(alpha=0.2)
plt.show()


# 3) QKM + SVM pré-computado (pipeline completo)
X_train, y_train = X_all[:180], y_all[:180]
X_test, y_test = X_all[180:], y_all[180:]

q = QuantumSVMPipeline(n_qubits=4, reps=1, seed=2)
q.fit(X_train, y_train)
acc_tr = q.score(X_train, y_train)
acc_te = q.score(X_test, y_test)
rep = q.report(X_test, y_test)

print("acc train =", round(acc_tr, 4))
print("acc test  =", round(acc_te, 4))
print("espectro kernel [min,max] =",
      round(rep["kernel_spectrum_min"], 5),
      round(rep["kernel_spectrum_max"], 3))
print("disclaimer:", rep["disclaimer"])


# 4) VQC experimental (ansatz + COBYLA)
# PCA p/ n_qubits + wrap, igual ao pipeline QKM.
from sklearn.decomposition import PCA
from sklearn.preprocessing import StandardScaler
from arkhe_rf.quantum_pipeline import _wrap_transform

N_QV = 72
scaler = StandardScaler().fit(X_train[:N_QV])
pca = PCA(n_components=3, random_state=3).fit(scaler.transform(X_train[:N_QV]))
emb = _wrap_transform(pca.transform(scaler.transform(X_train[:N_QV])))
y_qv = y_train[:N_QV]

v = VQClassifier(n_qubits=3, reps=1, seed=4)
res = v.fit(emb, y_qv, maxiter=80)
print("otimização:", res, "| acc =", round(v.score(emb, y_qv), 4))


# 5) Comparação honesta — QKM vs SVC-RBF (mesmo feature space)
cmp = compare_quantum_classical(X_all, y_all, n_qubits=4, seed=5)

print("acc quantum (QKM) :", round(cmp["acc_quantum_qkm"], 4))
print("acc clássico (RBF):", round(cmp["acc_classical_rbf"], 4))
print("delta             :", round(cmp["delta"], 4))
print("regime dos dados  :", cmp["dataset_regime"])
print("conclusão         :", cmp["conclusion"])

labels = ["Quântico (QKM)", "Clássico (RBF)"]
vals = [cmp["acc_quantum_qkm"], cmp["acc_classical_rbf"]]
plt.figure(figsize=(6, 4))
bars = plt.bar(labels, vals, color=["#3b82f6", "#f97316"], width=0.55)
plt.ylim(0, 1)
for b, v in zip(bars, vals):
    plt.text(b.get_x() + b.get_width() / 2, v + 0.02, f"{v:.3f}",
             ha="center")
plt.title("Comparação no MESMO feature space — dados sintéticos")
plt.ylabel("acurácia de hold-out"); plt.grid(axis="y", alpha=0.3)
plt.show()
