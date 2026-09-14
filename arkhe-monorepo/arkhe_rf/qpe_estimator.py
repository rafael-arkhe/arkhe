"""
Estimador de fase quântico (QPE) com ruído de frequência de plasma (DPDM).

A conversão ressonante no plasma gera inomogeneidades δn/n que variam a
frequência de plasma: δω_p = ω_p·(sqrt(1 + δn/n) − 1). O ruído de fase
resultante degrada a fase estimada pelo QPE — o alargamento espectral é a
assinatura de saturação medida pelo pipeline quântico.

O circuito QPE usa a API do Qiskit 2.x (StatevectorSampler). Sem qiskit
instalado, apenas o modelo de ruído (numpy puro) fica disponível.
"""

import numpy as np

__all__ = [
    "QISKIT_AVAILABLE",
    "add_plasma_phase_noise",
    "run_qpe",
    "run_qpe_with_plasma_noise",
]

try:
    from qiskit import ClassicalRegister, QuantumCircuit, QuantumRegister
    from qiskit.primitives import StatevectorSampler

    QISKIT_AVAILABLE = True
except ImportError:  # pragma: no cover
    QISKIT_AVAILABLE = False


def add_plasma_phase_noise(
    phi_estimated: float,
    density_perturb: float,
    n_est: int,
) -> dict:
    """
    Aplica ruído de fase devido a inomogeneidades de densidade do plasma.

    δω_p = ω_p·(sqrt(1 + δn/n) − 1);  δφ = δω_p·Δt/(2π).

    Returns:
        `phi_noisy`, `k_noisy` (inteiro no registrador), `confidence`
        (degradada pelo ruído), `phase_noise` e `density_perturb`.
    """
    density_perturb = float(density_perturb)
    omega_p = 2 * np.pi * 1e6  # 1 MHz (ex.: ionosfera terrestre)
    delta_t = 12.5e-9  # 12.5 ns
    delta_omega_p = omega_p * (np.sqrt(max(1.0 + density_perturb, 0.0)) - 1.0)
    phase_noise = float(delta_omega_p * delta_t / (2 * np.pi))

    phi_noisy = float(phi_estimated) + phase_noise
    k_noisy = int(round(phi_noisy * (2 ** int(n_est)))) % (2 ** int(n_est))
    confidence = float(1.0 / (1.0 + abs(phase_noise) * 10.0))

    return {
        "phi_noisy": phi_noisy,
        "k_noisy": k_noisy,
        "confidence": confidence,
        "phase_noise": phase_noise,
        "density_perturb": density_perturb,
    }


def run_qpe(
    unitary_phase: float,
    n_est: int = 6,
    shots: int = 4096,
    seed: int = 0,
) -> dict:
    """
    QPE de um unitário diagonal U = diag(1, e^{2πiφ}) por phase kickback.

    Returns:
        `phi_estimated` ∈ [0, 1), `k_decimal` (inteiro no registrador),
        `confidence`, `counts` (topo) e `n_est`.
    """
    if not QISKIT_AVAILABLE:
        raise ImportError("qiskit necessário. `pip install 'arkhe-rf[quantum]'`")

    n_est = int(n_est)
    phase = float(unitary_phase) % 1.0

    q_est = QuantumRegister(n_est, "q_est")
    q_aux = QuantumRegister(1, "q_aux")
    c_out = ClassicalRegister(n_est, "c")
    qc = QuantumCircuit(q_est, q_aux, c_out)

    qc.h(q_est)
    qc.x(q_aux)

    # Controlled-U^(2^i): phase kickback acumula 2πφ·2^i no qubit de estimativa.
    for i in range(n_est):
        qc.cp(2 * np.pi * phase * (2**i), q_est[i], q_aux)

    # QFT† — H e fases controladas.
    for i in range(n_est - 1, -1, -1):
        qc.h(q_est[i])
        for j in range(i):
            qc.cp(-np.pi / (2 ** (i - j)), q_est[i], q_est[j])
    for i in range(n_est // 2):  # swap → ordem natural dos bits
        qc.swap(q_est[i], q_est[n_est - 1 - i])

    qc.measure(q_est, c_out)

    sampler = StatevectorSampler(seed=int(seed))
    job = sampler.run([qc], shots=int(shots))
    counts = job.result()[0].data.c.get_counts()

    top_bitstring = max(counts, key=counts.get)
    k_decimal = int(top_bitstring, 2)
    phi_estimated = k_decimal / (2**n_est)

    return {
        "phi_estimated": float(phi_estimated),
        "k_decimal": int(k_decimal),
        "confidence": 1.0,
        "counts": counts,
        "n_est": n_est,
    }


def run_qpe_with_plasma_noise(
    unitary_phase: float,
    n_est: int = 6,
    shots: int = 4096,
    density_perturb: float = 0.0,
    seed: int = 0,
) -> dict:
    """
    Executa o QPE e aplica o ruído de plasma DPDM sobre a fase estimada.

    Quando `density_perturb > 0`, `phi_estimated`, `k_decimal` e
    `confidence` são degradados pelo modelo de saturação, e o resultado
    ganha os campos `plasma_noise` e `density_perturb`.
    """
    result = run_qpe(unitary_phase, n_est, shots, seed)

    if float(density_perturb) > 0.0:
        noise = add_plasma_phase_noise(
            result["phi_estimated"], float(density_perturb), n_est
        )
        result["phi_estimated"] = noise["phi_noisy"]
        result["k_decimal"] = noise["k_noisy"]
        result["confidence"] *= noise["confidence"]
        result["plasma_noise"] = noise["phase_noise"]
        result["density_perturb"] = float(density_perturb)

    return result


if __name__ == "__main__":
    if QISKIT_AVAILABLE:
        r = run_qpe_with_plasma_noise(0.25, n_est=6, density_perturb=10.0)
        print(f"phi={r['phi_estimated']:.4f} conf={r['confidence']:.3f}")
        print(f"plasma_noise={r.get('plasma_noise', 0.0):.2e}")
    else:
        n = add_plasma_phase_noise(0.25, 10.0, 6)
        print(f"phi_noisy={n['phi_noisy']:.4f} conf={n['confidence']:.3f}")
