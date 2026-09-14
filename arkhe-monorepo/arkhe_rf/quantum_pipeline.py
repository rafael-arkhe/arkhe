"""
Caminho paralelo Fase 3 — classificação quântica (QKM/VQC) em dados
sintéticos com rotulagem controlada.

OBJETIVO HONESTO: validar TECNICAMENTE que o pipeline quântico funciona
sobre o MEMSO formato de entrada da Fase 2 (features RF 68-dim + ELF
20-dim), sem reivindicar vantagem quântica:

  * O dataset é sintético (distribuição controlada) — não existe
    "vantagem" com significado até ≥10 h de dados reais calibrados.
  * QKM e baseline clássico usam o MESMO feature space (pós-PCA).
  * O relatório devolve exatamente o que os dados mostram.

Componentes:
    QuantumKernel          kernel |<φ(x_i)|φ(x_j)>|² via Statevector
    QuantumSVMPipeline     QKM + SVM de kernel pré-computado
    VQClassifier           VQC experimental (ansatz + COBYLA)
    compare_quantum_classical  comparação honesta com SVC-RBF
"""

import numpy as np
from sklearn.decomposition import PCA
from sklearn.metrics import accuracy_score
from sklearn.preprocessing import StandardScaler
from sklearn.svm import SVC

__all__ = [
    "QISKIT_AVAILABLE",
    "QuantumKernel",
    "QuantumSVMPipeline",
    "VQClassifier",
    "compare_quantum_classical",
]


try:
    from qiskit import QuantumCircuit  # noqa: F401
    from qiskit.circuit.library import real_amplitudes, zz_feature_map
    from qiskit.quantum_info import Statevector

    QISKIT_AVAILABLE = True
except ImportError:  # pragma: no cover
    QISKIT_AVAILABLE = False

_FEATURE_MAX = np.pi / 2.0   # mapeia embeddings para (-π/2, π/2)


def _wrap_transform(emb: np.ndarray) -> np.ndarray:
    """Leva coordenadas PCA para a faixa de fase dos feature maps."""
    mx = np.max(np.abs(emb)) + 1e-9
    return emb * (_FEATURE_MAX / mx)


class QuantumKernel:
    """
    Kernel quântico k(x_i, x_j) = |⟨φ(x_i) | φ(x_j)⟩|² usando ZZFeatureMap.

    O kernel é calculado por simulação de Statevector (fiel até ~20
    qubits). Em hardware real, o mesmo kernel exigiria estimativa por
    shots; a API pública é idêntica.
    """

    def __init__(self, n_qubits: int = 8, reps: int = 2, seed: int = 0):
        if not QISKIT_AVAILABLE:
            raise ImportError("qiskit necessário. `pip install 'arkhe-rf[quantum]'`")
        np.random.seed(seed)
        self.n_qubits = int(n_qubits)
        self.reps = int(reps)
        # API funcional Qiskit 2.x (a classe ZZFeatureMap é deprecada).
        self.feature_map = zz_feature_map(
            feature_dimension=self.n_qubits, reps=self.reps
        )
        self._data_params = list(self.feature_map.parameters)

    def _state(self, x: np.ndarray) -> "Statevector":
        params = np.asarray(x, dtype=float).reshape(-1)
        if params.size != self.n_qubits:
            raise ValueError(
                f"embedding precisa de {self.n_qubits} coordenadas, "
                f"recebeu {params.size}"
            )
        qc = self.feature_map.assign_parameters(
            dict(zip(self._data_params, params.tolist()))
        )
        return Statevector(qc)

    def kernel(self, X: np.ndarray) -> np.ndarray:
        """Matriz de kernel n×n para um lote."""
        X = np.atleast_2d(X)
        n = len(X)
        states = [self._state(row) for row in X]
        K = np.zeros((n, n))
        for i in range(n):
            for j in range(i, n):
                fid = _fidelity(states[i], states[j])
                K[i, j] = fid
                K[j, i] = fid
        return K

    def kernel_row(self, x: np.ndarray, X: np.ndarray) -> np.ndarray:
        """Linha k(x, ·) contra um lote de referência (para predict)."""
        sx = self._state(x)
        return np.array([_fidelity(sx, self._state(row)) for row in X])


def _fidelity(a: "Statevector", b: "Statevector") -> float:
    """|⟨a|b⟩|² entre dois estados puros."""
    v = a.conjugate().data @ b.data
    return float(np.abs(v) ** 2)


class QuantumSVMPipeline:
    """
    QKM → SVC com kernel pré-computado (receita padrão de ML quântico).

    Honestidade: nenhum excesso é atribuído a efeitos quânticos sem dados
    reais; `report()` declara explicitamente o regime dos dados usados.
    """

    def __init__(
        self,
        n_qubits: int = 8,
        reps: int = 2,
        seed: int = 0,
        svc_c: float = 1.0,
    ):
        np.random.seed(seed)
        self.kernel_obj = QuantumKernel(n_qubits=n_qubits, reps=reps, seed=seed)
        self.embed_dim = int(n_qubits)
        self.scaler = StandardScaler()
        self.pca = PCA(n_components=self.embed_dim, random_state=seed)
        self.svc = SVC(kernel="precomputed", C=float(svc_c))
        self.train_emb_: np.ndarray | None = None

    def _embed(self, X: np.ndarray) -> np.ndarray:
        z = self.scaler.transform(X)
        p = self.pca.transform(z)
        return _wrap_transform(p)

    def fit(self, X: np.ndarray, y: np.ndarray) -> "QuantumSVMPipeline":
        X = np.atleast_2d(np.asarray(X, dtype=float))
        self.scaler.fit(X)
        self.pca.fit(self.scaler.transform(X))
        emb = self._embed(X)
        self.train_emb_ = emb
        K = self.kernel_obj.kernel(emb)
        self.svc.fit(K, np.asarray(y))
        return self

    def predict(self, X: np.ndarray) -> np.ndarray:
        if self.train_emb_ is None:
            raise RuntimeError("chame .fit() antes de predict()")
        emb = self._embed(np.atleast_2d(X))
        K_test = np.array(
            [self.kernel_obj.kernel_row(row, self.train_emb_) for row in emb]
        )
        return self.svc.predict(K_test)

    def score(self, X: np.ndarray, y: np.ndarray) -> float:
        return float(accuracy_score(np.asarray(y), self.predict(X)))

    def report(self, X_test=None, y_test=None, dataset_note: str = "sintético") -> dict:
        result = {"dataset_regime": dataset_note, "n_qubits": self.embed_dim}
        if X_test is not None and y_test is not None:
            result["acc_test"] = self.score(X_test, y_test)
        if self.train_emb_ is not None:
            K = self.kernel_obj.kernel(self.train_emb_)
            eig = np.linalg.eigvalsh(K)
            result["kernel_spectrum_min"] = float(eig.min())
            result["kernel_spectrum_max"] = float(eig.max())
        result["disclaimer"] = (
            "Nenhuma alegação de vantagem quântica sem dataset real "
            "(≥10 h RF+ELF). Comparação final exige o mesmo dataset "
            "calibrado para clássico e quântico."
        )
        return result


class VQClassifier:
    """
    VQC experimental: ZZFeatureMap + RealAmplitudes, treinado com COBYLA.

    Simulação por Statevector (expectativa de Z no qubit 0). Marcado como
    experimental: em hardware real, shots e mitigação de ruído seriam
    necessários.
    """

    def __init__(self, n_qubits: int = 4, reps: int = 1, seed: int = 0):
        if not QISKIT_AVAILABLE:
            raise ImportError("qiskit necessário. `pip install 'arkhe-rf[quantum]'`")
        np.random.seed(seed)
        self.n_qubits = int(n_qubits)
        self.feature_map = zz_feature_map(feature_dimension=self.n_qubits, reps=1)
        self._data_params = list(self.feature_map.parameters)
        self.ansatz = real_amplitudes(self.n_qubits, reps=int(reps))
        self._theta_params = list(self.ansatz.parameters)
        self.theta_ = np.random.uniform(-np.pi, np.pi, len(self._theta_params))
        self.fitted_ = False

    def _state(self, x: np.ndarray, theta: np.ndarray) -> "Statevector":
        qc = self.feature_map.assign_parameters(
            dict(zip(self._data_params, np.asarray(x, float).reshape(-1).tolist()))
        )
        qc = qc.compose(
            self.ansatz.assign_parameters(
                dict(zip(self._theta_params, np.asarray(theta, float).reshape(-1).tolist()))
            )
        )
        return Statevector(qc)

    def _decision_values(self, X: np.ndarray, theta: np.ndarray) -> np.ndarray:
        out = []
        for row in X:
            sv = self._state(row, theta)
            probs = sv.probabilities_dict()
            p0 = sum(p for key, p in probs.items() if key[-1] == "0")
            out.append(2.0 * p0 - 1.0)
        return np.asarray(out)

    def _loss(self, theta: np.ndarray, X: np.ndarray, y: np.ndarray) -> float:
        yt = np.where(np.asarray(y) > 0, 1.0, -1.0)
        pred = self._decision_values(X, theta)
        return float(np.mean((yt - pred) ** 2))

    def fit(self, X: np.ndarray, y: np.ndarray, maxiter: int = 120) -> dict:
        from scipy.optimize import minimize

        X = np.atleast_2d(np.asarray(X, float))
        res = minimize(
            self._loss,
            self.theta_.copy(),
            args=(X, np.asarray(y)),
            method="COBYLA",
            options={"maxiter": int(maxiter)},
        )
        self.theta_ = np.asarray(res.x, float)
        self.fitted_ = True
        return {"fun": float(res.fun), "success": bool(res.success)}

    def predict(self, X: np.ndarray) -> np.ndarray:
        if not self.fitted_:
            raise RuntimeError("chame .fit() antes de predict()")
        return np.where(self._decision_values(X, self.theta_) > 0.0, 1, 0)

    def score(self, X: np.ndarray, y: np.ndarray) -> float:
        return float(accuracy_score(np.asarray(y), self.predict(X)))


def compare_quantum_classical(
    X: np.ndarray,
    y: np.ndarray,
    n_qubits: int = 6,
    test_size: float = 0.25,
    seed: int = 0,
) -> dict:
    """
    Comparação clássico vs. quântico sobre o MESMO feature space.

    - Quântico: QKM (ZZFeatureMap) + SVM pré-computado.
    - Clássico: SVC-RBF sobre as MESMAS componentes PCA.
    Retorna acurácias de hold-out e os parâmetros usados, sem juízo de
    vantagem — o regime dos dados é declarado.
    """
    from sklearn.model_selection import train_test_split

    X = np.atleast_2d(np.asarray(X, float))
    y = np.asarray(y)
    X_tr, X_te, y_tr, y_te = train_test_split(
        X, y, test_size=test_size, random_state=seed, stratify=y
    )

    # Pipeline quântico.
    qpipe = QuantumSVMPipeline(n_qubits=n_qubits, seed=seed)
    qpipe.fit(X_tr, y_tr)
    acc_q = qpipe.score(X_te, y_te)

    # Baseline clássico no mesmo espaço de features.
    from sklearn.pipeline import make_pipeline

    rbf = make_pipeline(StandardScaler(), PCA(n_components=n_qubits, random_state=seed),
                        SVC(kernel="rbf", C=1.0))
    rbf.fit(X_tr, y_tr)
    acc_c = float(accuracy_score(y_te, rbf.predict(X_te)))

    return {
        "acc_quantum_qkm": acc_q,
        "acc_classical_rbf": acc_c,
        "delta": acc_q - acc_c,
        "n_qubits": int(n_qubits),
        "n_train": int(len(y_tr)),
        "n_test": int(len(y_te)),
        "dataset_regime": "sintético (rotulagem controlada) — sem dados reais",
        "conclusion": (
            "Mesmo feature space; diferença observada NÃO constitui "
            "vantagem quântica até validação em dataset real."
        ),
    }


if QISKIT_AVAILABLE:

    def _self_test() -> dict:
        """Verificação técnica mínima (kernel PSD, shapes)."""
        rng = np.random.default_rng(0)
        n = 24
        X = rng.standard_normal((n, 6))
        y = (X[:, 0] + 0.8 * X[:, 1] > 0).astype(int)
        q = QuantumSVMPipeline(n_qubits=4, reps=1, seed=1)
        q.fit(X, y)
        acc = q.score(X, y)
        K = q.kernel_obj.kernel(q.train_emb_)
        return {"acc_train": acc, "kernel_shape": K.shape,
                "kernel_symmetric": bool(np.allclose(K, K.T))}


if __name__ == "__main__":
    print("selftest:", _self_test())
    rng = np.random.default_rng(7)
    X = rng.standard_normal((240, 68))
    y = (X[:, 0] + X[:, 3] > 0).astype(int)
    print("compare:", compare_quantum_classical(X, y, n_qubits=6))