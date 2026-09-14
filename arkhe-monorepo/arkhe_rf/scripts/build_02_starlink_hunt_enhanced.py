"""
Gerador do notebook `02_starlink_hunt_enhanced.ipynb`.

Re-execute este script sempre que o protocolo de caça mudar:

    python notebooks/build_02_starlink_hunt_enhanced.py

Também emite `02_starlink_hunt_enhanced.py` (versão script, executável
em CLI: `python notebooks/02_starlink_hunt_enhanced.py`).
"""

import json
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

NOTEBOOK = ROOT / "notebooks" / "02_starlink_hunt_enhanced.ipynb"
SCRIPT = ROOT / "notebooks" / "02_starlink_hunt_enhanced.py"

HEADER = textwrap.dedent(
    """\
    # Protocolo de Caça Starlink DTC — Versão Empírica v2.1

    Este protocolo **não** assume o Doppler a priori. Ele mede:

     1. SNR elevado (> 15 dB)
     2. Múltiplos picos espectrais (estrutura OFDM-like)
     3. Inclinação Doppler mensurável (0,5–5,0 kHz/s)
     4. Bondade do ajuste linear (R² > 0,7)

    A janela de busca de ±300 kHz cobre a estimativa teórica (~50 kHz a
    1,9 GHz) e valores relatados em documentos (~250 kHz — origem Ku).
    Os dados empíricos resolverão a discrepância.

    Alvos (FCC DA 23-338):
      - UPLINK_PCS   1912,5 MHz (1910–1915 MHz)
      - DOWNLINK_PCS 1992,5 MHz (1990–1995 MHz)

    > Sem hardware ainda, o backend opera em **modo simulação** com uma
    > passagem LEO determinística (S-curve Doppler). Quando o AntSDR
    > chegar, basta definir `SIMULATION_MODE=false` e o driver SoapySDR
    > no `arkhe_rf/config.py`.
    """
)

IMPORTS = textwrap.dedent(
    """\
    import json
    import sys
    import time
    from datetime import datetime, timezone
    from pathlib import Path

    # Resolve a raiz do monorepo independentemente de onde o notebook roda.
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

    import arkhe_rf
    from arkhe_rf.config import (
        DETECTION,
        DOPPLER_SLOPE_MAX_HZ_S,
        DOPPLER_SLOPE_MIN_HZ_S,
        DOPPLER_R_SQUARED_MIN,
        DWELL_TIME_PER_TARGET_S,
        STARLINK_DTC_TARGETS,
    )
    """
)

PHYSICS_CHECK = textwrap.dedent(
    """\
    from arkhe_rf.doppler import estimate_leo_doppler_max, validate_doppler_claim

    est = estimate_leo_doppler_max(1992.5e6)
    print(f"Doppler LEO teórico (downlink PCS): ")
    for k, v in est.items():
        print(f"  {k}: {v}")

    val_pcs = validate_doppler_claim(254.6, 1992.5e6)
    val_ku = validate_doppler_claim(254.6, 10.0e9)
    print(f"\\n254,6 kHz @ 1,99 GHz → {val_pcs['verdict']}")
    print(f"254,6 kHz @ 10 GHz (Ku) → {val_ku['verdict']}")
    """
)

HUNT = textwrap.dedent(
    """\
    from arkhe_rf.capture import AntSDRCapture
    from arkhe_rf.doppler import DopplerTracker
    from arkhe_rf.features import extract_features

    CONFIG = {
        "sample_rate": 20e6,          # 20 MSPS (A/C — laptop-friendly)
        "capture_duration": 0.5,      # 0,5 s por frame (rajadas TDD visíveis)
        "dwell_time_per_target": float(DWELL_TIME_PER_TARGET_S),
    }

    DET = {
        "snr_threshold": DETECTION["snr_threshold_db"],
        "min_peaks": DETECTION["min_peaks"],
        "frames_to_confirm": DETECTION["diffuse"]["frames_to_confirm"],
        "slope_min_hz": DOPPLER_SLOPE_MIN_HZ_S,
        "slope_max_hz": DOPPLER_SLOPE_MAX_HZ_S,
        "r_squared_min": DOPPLER_R_SQUARED_MIN,
    }


    def hunt_target(sdr, target, duration):
        \"\"\"Monitora `target` por `duration` s e devolve o melhor candidato.\"\"\"
        tracker = DopplerTracker(
            window_size=60,
            slope_min_hz=DET["slope_min_hz"],
            slope_max_hz=DET["slope_max_hz"],
            r_squared_min=DET["r_squared_min"],
        )
        start = time.time()
        frame = 0
        consecutive = 0
        best = None
        hist = {"t": [], "snr": [], "offset_khz": [], "slope_khz_s": [], "r2": []}

        while time.time() - start < duration:
            iq = sdr.capture()
            feat = extract_features(iq, sample_rate=sdr.sample_rate,
                                    center_freq=sdr.center_freq)
            offset = feat["peak_freq"] - target["freq"]
            state = tracker.update(offset, time.time() - start)
            frame += 1

            hist["t"].append(time.time() - start)
            hist["snr"].append(feat["snr_db"])
            hist["offset_khz"].append(offset / 1e3)
            hist["slope_khz_s"].append(state.slope_khz_per_s)
            hist["r2"].append(state.r_squared)

            if frame % 5 == 0:
                print(f"    [{frame:3d}] SNR={feat['snr_db']:5.1f} dB | "
                      f"picos={feat['num_peaks']:2d} | "
                      f"slope={state.slope_khz_per_s:+6.2f} kHz/s | "
                      f"R²={state.r_squared:.3f} | {state.confidence}")

            if (feat["snr_db"] > DET["snr_threshold"]
                    and feat["num_peaks"] >= DET["min_peaks"]
                    and state.is_satellite):
                consecutive += 1
            else:
                consecutive = max(0, consecutive - 1)

            if consecutive >= DET["frames_to_confirm"]:
                candidate = {
                    "target": target["name"],
                    "freq_hz": target["freq"],
                    "timestamp": datetime.now(timezone.utc).isoformat(),
                    "snr_db": feat["snr_db"],
                    "slope_khz_per_s": state.slope_khz_per_s,
                    "r_squared": state.r_squared,
                    "confidence": state.confidence,
                    "num_peaks": feat["num_peaks"],
                    "all_peaks": feat["all_peaks"],
                    "theoretical_doppler_khz": est["delta_f_max_khz"],
                }
                print(f"\\n    *** DETECÇÃO CONFIRMADA [{target['name']}] ***")
                print(f"    SNR={candidate['snr_db']:.1f} dB | "
                      f"slope={candidate['slope_khz_per_s']:+.2f} kHz/s | "
                      f"R²={candidate['r_squared']:.4f} | "
                      f"{candidate['confidence']}")
                return candidate, hist, iq

        return best, hist, None


    def run_hunt():
        \"\"\"Loop de caça com rotação entre alvos PCS.\"\"\"
        print("=" * 62)
        print("ARKHE — PROTOCOLO DE CAÇA STARLINK DTC (Fase 1)")
        print(f"Janela Doppler: {DET['slope_min_hz']/1e3:.1f}–"
              f"{DET['slope_max_hz']/1e3:.1f} kHz/s | "
              f"R²>={DET['r_squared_min']}")
        print("=" * 62)

        detections = []
        try:
            with AntSDRCapture(
                center_freq=STARLINK_DTC_TARGETS[0]["freq"],
                sample_rate=CONFIG["sample_rate"],
                bandwidth=CONFIG["sample_rate"],
                duration=CONFIG["capture_duration"],
            ) as sdr:
                cycle = 0
                while True:
                    cycle += 1
                    print(f"\\nCICLO {cycle} — "
                          f"{datetime.now().strftime('%H:%M:%S')}")
                    for target in STARLINK_DTC_TARGETS:
                        sdr.center_freq = target["freq"]
                        candidate, hist, iq = hunt_target(
                            sdr, target, CONFIG["dwell_time_per_target"])
                        if candidate is not None:
                            detections.append(candidate)
                            sdr.snapshot(iq, candidate, candidate["target"])
                            print(f"  detecções: {len(detections)}")
        except KeyboardInterrupt:
            print("\\nInterrompido pelo operador.")
        return detections, hist


    detections, last_hist = run_hunt()
    """
)

LEGEND = textwrap.dedent(
    """\
    ### Leitura do rastro (tabela do briefing)

    | Padrão | Interpretação |
    |:---|:---|
    | Linha plana em 0 kHz, SNR constante | Torre terrestre local |
    | Curva em "S" no eixo Doppler + SNR em arco | **Satélite detectado** |
    | Spikes aleatórios de SNR, Doppler instável | Ruído urbano / interferência |
    """
)

PLOT = textwrap.dedent(
    """\
    import matplotlib.pyplot as plt

    if last_hist is not None and len(last_hist["t"]) > 1:
        fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(12, 6), sharex=True)
        ax1.plot(last_hist["t"], last_hist["snr"], color="cyan")
        ax1.set_ylabel("SNR (dB)")
        ax1.set_title("Rastreio RF — Starlink DTC (PCS)")
        ax1.grid(True, alpha=0.3)

        ax2.plot(last_hist["t"], last_hist["offset_khz"], color="orange")
        ax2.set_ylabel("Desvio Doppler (kHz)")
        ax2.set_xlabel("Tempo (s)")
        ax2.axhline(0, color="white", linestyle="--", alpha=0.5)
        ax2.grid(True, alpha=0.3)

        out = ROOT / "data" / "starlink_doppler_trace.png"
        out.parent.mkdir(parents=True, exist_ok=True)
        fig.savefig(out, dpi=150)
        print(f"Rastro salvo em {out}")
        plt.show()
    else:
        print("Sem histórico suficiente (candidato não confirmado nesta janela).")
    """
)


def cell(source: str, kind: str = "code") -> dict:
    return {
        "cell_type": kind,
        "metadata": {},
        "source": source,
        "outputs": [] if kind == "code" else None,
        "execution_count": None if kind == "code" else None,
    }


def build() -> None:
    cells = [
        cell(HEADER, "markdown"),
        cell(IMPORTS),
        cell(PHYSICS_CHECK),
        cell(HUNT),
        cell(LEGEND, "markdown"),
        cell(PLOT),
    ]
    nb = {
        "cells": cells,
        "metadata": {
            "kernelspec": {"display_name": "Python 3", "language": "python",
                           "name": "python3"},
            "language_info": {"name": "python"},
        },
        "nbformat": 4,
        "nbformat_minor": 5,
    }
    NOTEBOOK.parent.mkdir(parents=True, exist_ok=True)
    with open(NOTEBOOK, "w", encoding="utf-8") as fh:
        json.dump(nb, fh, ensure_ascii=False, indent=1)
    print(f"escrito: {NOTEBOOK}")

    script_body = '"""' + HEADER + '"""' + "\n\n" + "\n\n".join(
        c["source"] for c in cells if c["cell_type"] == "code"
    )
    with open(SCRIPT, "w", encoding="utf-8") as fh:
        fh.write(script_body)
    print(f"escrito: {SCRIPT}")


if __name__ == "__main__":
    build()