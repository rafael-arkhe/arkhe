"""
Configuração central do pipeline RF — Fase 1 (Caça Starlink DTC).

Todos os parâmetros são sobrescrevíveis por variáveis de ambiente
(nomes em `_ENV`). O pipeline não assume valor de Doppler a priori:
a janela de busca larga cobre a estimativa teórica (~50 kHz a 1,9 GHz)
e valores relatados em documentos que exigem validação empírica.
"""

import os

# ============================== Física ==============================

SPEED_OF_LIGHT_MS = 299792458.0
GRAVITATIONAL_CONSTANT = 6.674e-11      # m³/(kg·s²)
EARTH_MASS_KG = 5.972e24
EARTH_RADIUS_M = 6.371e6
LEO_REF_ALTITUDE_KM = 550.0

# ========================== Alvos DTC (Fase 1) ========================

# FCC DA 23-338: SpaceX autorizada a operar Direct-to-Cell em bandas PCS.
# Downlink é o sinal de espaço → Terra; Uplink é o smartphone → satélite.
STARLINK_DTC_TARGETS = [
    {
        "name": "UPLINK_PCS",
        "freq": 1912.5e6,       # Centro de 1910–1915 MHz
        "bw": 5e6,
        "source": "FCC DA 23-338",
    },
    {
        "name": "DOWNLINK_PCS",
        "freq": 1992.5e6,       # Centro de 1990–1995 MHz
        "bw": 5e6,
        "source": "FCC DA 23-338",
    },
]

# Alias retrocompatível com o briefing original (alvo único).
STARLINK_DTC_FREQ = 1910.0e6     # 1910 MHz (herdado)
STARLINK_DTC_BW = 5.0e6          # 5 MHz (bloco padrão LTE)
STARLINK_CAPTURE_DURATION = 1.0  # 1s por frame (necessário para ver rajadas TDD)

# ============================ Doppler ================================

# Janela de busca — cobre 50 kHz teórico E 250 kHz relatado (espírito Ku).
DOPPLER_SEARCH_RANGE_HZ = int(os.environ.get("DOPPLER_SEARCH_RANGE_HZ", "300000"))
DOPPLER_SLOPE_MIN_HZ_S = float(os.environ.get("DOPPLER_SLOPE_MIN_HZ_S", "500.0"))      # 0.5 kHz/s
DOPPLER_SLOPE_MAX_HZ_S = float(os.environ.get("DOPPLER_SLOPE_MAX_HZ_S", "5000.0"))     # 5.0 kHz/s
DOPPLER_R_SQUARED_MIN = float(os.environ.get("DOPPLER_R_SQUARED_MIN", "0.7"))
DOPPLER_TRACKER_WINDOW = int(os.environ.get("DOPPLER_TRACKER_WINDOW", "60"))

# ============================ Captura ================================

SAMPLE_RATE_DEFAULT = float(os.environ.get("ARKHE_RF_SAMPLE_RATE", "30e6"))   # 30 MSPS
BANDWIDTH_DEFAULT_HZ = float(os.environ.get("ARKHE_RF_BANDWIDTH", "10e6"))    # 10 MHz de visão
FFT_SIZE = int(os.environ.get("ARKHE_RF_FFT_SIZE", "4096"))

# Driver SoapySDR. Formato depende do backend: "driver=ant,serial=<id>".
SOAPY_DRIVER = os.environ.get("ARKHE_SOAPY_DRIVER", "driver=ant")
SIMULATION_MODE = os.environ.get("ARKHE_RF_SIM_MODE", "true").lower() == "true"

# ============================ Detecção ===============================

DETECTION = {
    "snr_threshold_db": float(os.environ.get("DETECTION_SNR_DB", "15.0")),
    "min_peaks": int(os.environ.get("DETECTION_MIN_PEAKS", "3")),
    "diffuse": {
        "snr_threshold_db": float(os.environ.get("DETECTION_DIFFUSE_SNR_DB", "12.0")),
        "frames_to_confirm": int(os.environ.get("DETECTION_FRAMES_TO_CONFIRM", "10")),
    },
}

# Dwell por alvo antes de rotacionar para o próximo.
DWELL_TIME_PER_TARGET_S = float(os.environ.get("DWELL_TIME_PER_TARGET_S", "30.0"))

# ========================== Simulação (fallback) =====================

# Envelope de passagem LEO sintética (usado apenas em modo simulação).
SIM_PASS_TOTAL_S = 360.0
SIM_PASS_TAU_S = 50.0                              # escala temporal do S-curve
SIM_PASS_DOPPLER_MAX_HZ = 50.0e3                   # Δf max no horizonte (~50 kHz)

# Potências relativas (dBc simulado). O satélite domina sobre o ruído.
SIM_NOISE_FLOOR_DBF = -118.0
SIM_TERRESTRIAL_POWER_DBF = -102.0
SIM_SATELLITE_POWER_DBF = -90.0   # tom central do cluster DTC

# ============================== Saída ================================

DETECTIONS_DIR = os.environ.get("ARKHE_RF_DETECTIONS_DIR", "data/detections")

# ============================ ELF / Schumann ==========================

# Ressonâncias de Schumann — fundamental 7,83 Hz e os modos de cavidade
# da Terra-ionosfera. IMPORTANTE: as frequências acima de 7,83 Hz são
# EIGENFREQUÊNCIAS da cavidade esférica, NÃO harmónicos inteiros de uma
# onda 1D (razões ≈ 1,83 / 2,66 / 3,49 / 4,32, e não 2 / 3 / 4 / 5).
SCHUMANN_FUNDAMENTAL_HZ = 7.83
SCHUMANN_EIGENMODES_HZ = (14.3, 20.8, 27.3, 33.8)
SCHUMANN_FREQS_HZ = (SCHUMANN_FUNDAMENTAL_HZ,) + SCHUMANN_EIGENMODES_HZ
SCHUMANN_HARMONICS = SCHUMANN_EIGENMODES_HZ  # alias retrocompatível

# Loop magnético + LNA: banda útil nominal 5–35 Hz.
ELF_SAMPLE_RATE_HZ = float(os.environ.get("ELF_SAMPLE_RATE_HZ", "250"))
ELF_BAND_HZ = (5.0, 35.0)
ELF_DEFAULT_DURATION_S = 10.0
ELF_PORT = os.environ.get("ELF_PORT", "")           # ex.: /dev/ttyUSB0 ou COM#
ELF_BAUD = 115200

# Dimensões do vetor de features ELF alimentando o autoencoder.
ELF_FEATURE_DIM = 20

# =========================== GPS Sync ================================

# Precisão exigida para coerência temporal RF/ELF (< 1 ms em desvio).
GPS_PPS_TOLERANCE_MS = 1.0

# ========================== Autoencoder ===============================

RF_FEATURE_DIM = 68          # = feature_vector_size() do modelo RF
AE_LATENT_DIM = 32
AE_HIDDEN_RF = 64            # encoder RF: 68 → 64 → 32
AE_HIDDEN_ELF = 32           # encoder ELF: 20 → 32 → 16
AE_DECODER_HIDDEN = 64
AE_SIGMA_ANOMALY = 3.0       # limiar de anomalia = erro médio + k·σ
AE_EPOCHS = 300
AE_BATCH_SIZE = 32
AE_LR = 1e-3
AE_ELF_LOSS_WEIGHT = 1.0

# ========================== Anomalia/ARKHE ============================

# Limiar de correlação para promover a anomalia a alerta de canal.
ANOMALY_T1_VIBRA2_CORRELATION_MIN = 0.7