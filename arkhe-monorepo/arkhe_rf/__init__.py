"""
arkhe_rf — pipeline de inteligência RF/ELF da Catedral.

Captura AntSDR → espectro → features (68-dim) → rastreamento Doppler;
loop ELF → ressonância Schumann (20-dim); GPS/PPS → linha do tempo
absoluta; autoencoder multi-modal → anomalias temporais.

Módulos:
    config            Parâmetros, alvos Starlink DTC, ELF/GPS/autoencoder
    capture           AntSDRCapture (SoapySDR real ou modo simulação LEO)
    features          compute_spectrum / extract_features
    doppler           DopplerTracker, estimativas físicas e correção IQ
    elf_loop          ELFLoop + features de Schumann
    gps_sync          GPSSync (PPS) para coerência temporal RF/ELF
    autoencoder       MultiModalAutoencoder + detecção de anomalias (torch)
    quantum_pipeline  QKM/VQC (caminho paralelo Fase 3, qiskit)
    dpdm_reference    Benchmark DPDM (fóton escuro) — referência física
    qpe_estimator     QPE + ruído de frequência de plasma (saturação DPDM)
    elf_saturation_monitor  Monitor de saturação ionosférica (razão de modos)
    dashboard         Visualização (Streamlit quando disponível; CLI fallback)
"""

from . import config, doppler, features
from .capture import AntSDRCapture
from .doppler import (
    DopplerState,
    DopplerTracker,
    doppler_shift_iq,
    estimate_leo_doppler_max,
    validate_doppler_claim,
)
from .dpdm_reference import DPDMReference
from .elf_loop import ELFLoop, schumann_feature_vector_size
from .elf_saturation_monitor import IonosphericSaturationMonitor
from .features import compute_spectrum, extract_features, feature_vector_size
from .gps_sync import GPSSync, pps_next_second

try:  # PyTorch é opcional (extra "ml").
    from .autoencoder import (
        MultiModalAutoencoder,
        ReconstructionAnomalyDetector,
        TORCH_AVAILABLE,
        anomaly_event,
        train_autoencoder,
    )
    TORCH_OK = TORCH_AVAILABLE
except ImportError:  # pragma: no cover
    TORCH_OK = False

try:  # Qiskit é opcional (extra "quantum").
    from .quantum_pipeline import (
        QISKIT_AVAILABLE,
        QuantumKernel,
        QuantumSVMPipeline,
        VQClassifier,
        compare_quantum_classical,
    )
    from .qpe_estimator import run_qpe, run_qpe_with_plasma_noise
    QISKIT_OK = QISKIT_AVAILABLE
except ImportError:  # pragma: no cover
    QISKIT_OK = False

__all__ = [
    "AntSDRCapture",
    "DPDMReference",
    "DopplerState",
    "DopplerTracker",
    "ELFLoop",
    "GPSSync",
    "IonosphericSaturationMonitor",
    "compute_spectrum",
    "config",
    "doppler",
    "doppler_shift_iq",
    "estimate_leo_doppler_max",
    "extract_features",
    "feature_vector_size",
    "features",
    "pps_next_second",
    "schumann_feature_vector_size",
    "validate_doppler_claim",
]
if TORCH_OK:  # pragma: no cover
    __all__ += [
        "MultiModalAutoencoder",
        "ReconstructionAnomalyDetector",
        "anomaly_event",
        "train_autoencoder",
    ]
if QISKIT_OK:  # pragma: no cover
    __all__ += [
        "QuantumKernel",
        "QuantumSVMPipeline",
        "VQClassifier",
        "compare_quantum_classical",
        "run_qpe",
        "run_qpe_with_plasma_noise",
    ]

__version__ = "0.3.0"