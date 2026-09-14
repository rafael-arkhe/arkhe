"""# Fase 2 — Observatório Multi-Domínio (RF + ELF)

Pipeline demonstrado aqui, **sem hardware** (backends de simulação):

 1. **ELF**: loop sintético → filtro 5–35 Hz → ressonância de Schumann
    (7,83 Hz e harmónicos) → features 20-dim.
 2. **GPS/PPS**: mesmo pulso absoluto anota o bloco RF e o bloco ELF.
 3. **Autoencoder multi-modal** (68-dim RF + 20-dim ELF → latente 32):
    treinado em "normalidade"; erro de reconstrução > μ + kσ = anomalia.

Com hardware, os mesmos objetos operam sobre o ADC serial (ADS1256) e
o AntSDR — basta configurar portas e `SIMULATION_MODE=false`.
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
from arkhe_rf.capture import AntSDRCapture
from arkhe_rf.config import RF_FEATURE_DIM, ELF_FEATURE_DIM, AE_LATENT_DIM
from arkhe_rf.elf_loop import ELFLoop
from arkhe_rf.gps_sync import GPSSync
from arkhe_rf.features import extract_features
from arkhe_rf.autoencoder import ReconstructionAnomalyDetector


# 1) ELF — Schumann
with ELFLoop(seed=1) as elf:
    sig = elf.capture(duration=10.0, gps_time=1_753_000_000.0)
    sfeat = elf.extract_schumann_features(sig)

print("amp_7.83 =", round(sfeat["amp_fundamental"], 4))
print("f_est    =", round(sfeat["frequency_est_hz"], 2), "Hz")
print("snr_est  =", round(sfeat["snr_est"], 2))
print("harm     =", {k: round(v, 3) for k, v in sfeat["harmonics"].items()})

plt.figure(figsize=(12, 3))
n = len(sig)
freqs = np.fft.rfftfreq(n, 1.0 / elf.fs)
spec = np.abs(np.fft.rfft(sig)) / n
plt.plot(freqs[:60], spec[:60])
plt.title("PSD ELF — ressonâncias de Schumann (sim)")
plt.xlabel("Hz")
plt.grid(alpha=0.3)
plt.show()


# 2) GPS — mesmo PPS anota RF e ELF
with GPSSync() as gps:
    pps = gps.get_pps_timestamp()
    meta_rf = gps.annotate(pps, channel="RF")
    meta_elf = gps.annotate(pps, channel="ELF")

print("PPS", pps, "| RF", meta_rf["unix_epoch_s"], "| ELF",
      meta_elf["unix_epoch_s"], "| coerência < 1 ms:",
      gps.within_tolerance(meta_rf["unix_epoch_s"], meta_elf["unix_epoch_s"]))


# 3) Dataset multi-modal em "normalidade" (simulação)
N_FRAMES = 60
SAMPLE_RATE = 2e6
FRAME_DUR = 0.05          # RF frame
ELF_FRAME_DUR = 2.0       # ELF frame (filtro 5–35 Hz exige nº mínimo)

rf_feats, elf_feats = [], []
gps = GPSSync()
with AntSDRCapture(
    center_freq=1992.5e6, sample_rate=SAMPLE_RATE,
    duration=FRAME_DUR, seed=4,
) as sdr, ELFLoop(seed=4) as elf:
    for _ in range(N_FRAMES):
        pps = gps.get_pps_timestamp()
        rf = extract_features(sdr.capture(), sample_rate=sdr.sample_rate,
                              center_freq=sdr.center_freq)
        esig = elf.capture(duration=ELF_FRAME_DUR, gps_time=pps)
        ef = elf.extract_schumann_features(esig)
        rf_feats.append(rf["feature_vector"])
        elf_feats.append(ef["feature_vector"])

X_rf = np.vstack(rf_feats)
X_elf = np.vstack(elf_feats)
print("RF ", X_rf.shape, "| ELF", X_elf.shape)


# 4) Autoencoder multi-modal treinado em normalidade
det = ReconstructionAnomalyDetector(
    rf_dim=RF_FEATURE_DIM, elf_dim=ELF_FEATURE_DIM, latent_dim=AE_LATENT_DIM,
    sigma=3.0,
)
history = det.fit(X_rf, X_elf, epochs=150, batch_size=16, verbose=0)

ep = np.array(list(history.keys()))
loss = np.array(list(history.values()))
plt.figure(figsize=(12, 3))
plt.plot(ep, loss)
plt.title("Treino — perda de reconstrução (epoch)")
plt.xlabel("epoch"); plt.ylabel("MSE")
plt.yscale("log"); plt.grid(alpha=0.3)
plt.show()

print("limiar anomalia (μ+3σ) =", round(det.threshold(), 4))


# 5) Injeção de desvio + detecção
rng = np.random.default_rng(9)
normal_rf, normal_elf = X_rf[:1], X_elf[:1]
injected = normal_rf.copy()
injected[:, 0:20] += 40.0 * rng.standard_normal((1, 20))   # desvio forte

res = det.detect(injected, normal_elf)
print("err_total   =", round(float(res["err_total"][0]), 4))
print("threshold   =", round(float(res["threshold"]), 4))
print("is_anomaly  =", bool(res["is_anomaly"][0]))

events = det.to_arkhe_event(injected, normal_elf,
                            timestamps=np.array([pps]))
print("evento ARKHE:", events[0])
