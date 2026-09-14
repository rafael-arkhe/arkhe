"""
Dashboard RF — monitoramento em tempo real.

Modo CLI (sempre disponível): tabela textual frame-a-frame com SNR,
picos e confiança Doppler.
Modo Streamlit (se `streamlit` estiver instalado): waterfal/histórico.
"""

import argparse
import time

import numpy as np

from . import config as _config
from .capture import AntSDRCapture
from .doppler import DopplerTracker
from .elf_loop import ELFLoop
from .features import extract_features
from .gps_sync import GPSSync

__all__ = ["run_cli", "run_streamlit", "run_multimodal_cli", "main"]


def _header() -> str:
    return (
        f"{'frame':>6} | {'SNR(dB)':>8} | {'peaks':>5} | "
        f"{'offset(kHz)':>11} | {'slope(kHz/s)':>12} | {'R2':>6} | confiança"
    )


def _row(frame: int, feat: dict, state) -> str:
    return (
        f"{frame:>6} | {feat['snr_db']:>8.2f} | {feat['num_peaks']:>5} | "
        f"{feat['peak_freq_offset'] / 1e3:>+11.3f} | "
        f"{state.slope_khz_per_s:>+12.3f} | {state.r_squared:>6.3f} | "
        f"{state.confidence}"
    )


def run_cli(
    center_freq: float,
    sample_rate: float,
    duration: float,
    dwell_s: float,
    sim_mode: bool = True,
) -> None:
    """Monitora um alvo e imprime uma tabela viva de vigilância RF."""
    print(_header())
    print("-" * len(_header()))

    tracker = DopplerTracker(
        window_size=_config.DOPPLER_TRACKER_WINDOW,
        slope_min_hz=_config.DOPPLER_SLOPE_MIN_HZ_S,
        slope_max_hz=_config.DOPPLER_SLOPE_MAX_HZ_S,
        r_squared_min=_config.DOPPLER_R_SQUARED_MIN,
    )

    with AntSDRCapture(
        center_freq=center_freq,
        sample_rate=sample_rate,
        bandwidth=min(_config.BANDWIDTH_DEFAULT_HZ, sample_rate),
        duration=duration,
        sim_mode=sim_mode,
    ) as sdr:
        start = time.time()
        frame = 0
        while time.time() - start < dwell_s:
            iq = sdr.capture()
            feat = extract_features(
                iq, sample_rate=sdr.sample_rate, center_freq=sdr.center_freq
            )
            state = tracker.update(feat["peak_freq"] - sdr.center_freq, frame)
            frame += 1
            print(_row(frame, feat, state), flush=True)


def run_streamlit(**kwargs) -> None:
    """Dashboard Streamlit (requer `pip install streamlit`)."""
    try:
        import streamlit as st  # type: ignore
    except ImportError:
        raise ImportError(
            "Streamlit não instalado. Use `pip install streamlit` ou "
            "mode CLI (sem --streamlit)."
        ) from None

    st.set_page_config(page_title="ARKHE RF/ELF", layout="wide")
    st.title("ARKHE RF/ELF — Observatório Multi-Domínio")
    center = st.number_input("Center freq (MHz)", min_value=300.0, max_value=6000.0, value=1992.5)
    sr = st.number_input("Sample rate (MSPS)", min_value=0.5, max_value=61.44, value=20.0)
    dur = st.slider("Frame (s)", 0.1, 5.0, 0.5)

    snr_h, offset_h, sch_h, err_h = [], [], [], []
    chart_ph = st.empty()
    if st.button("Iniciar vigilância") or True:
        with AntSDRCapture(
            center_freq=center * 1e6,
            sample_rate=sr * 1e6,
            bandwidth=min(_config.BANDWIDTH_DEFAULT_HZ, sr * 1e6),
            duration=dur,
        ) as sdr, ELFLoop(seed=0) as elf, GPSSync() as gps:
            for idx in range(10_000):
                feat = extract_features(
                    sdr.capture(), sample_rate=sdr.sample_rate,
                    center_freq=sdr.center_freq,
                )
                gps_time = gps.get_pps_timestamp()
                elf_sig = elf.capture(duration=dur, gps_time=gps_time)
                sfeat = elf.extract_schumann_features(elf_sig)
                snr_h.append(feat["snr_db"])
                offset_h.append(feat["peak_freq_offset"] / 1e3)
                sch_h.append(sfeat["amp_fundamental"])
                with chart_ph.container():
                    st.write(
                        f"frame {idx}: RF SNR={feat['snr_db']:.1f} dB "
                        f"offset={offset_h[-1]:+.2f} kHz | "
                        f"ELF 7.83Hz amp={sfeat['amp_fundamental']:.3f} "
                        f"f_est={sfeat['frequency_est_hz']:.2f} Hz")
                    st.line_chart({"SNR": snr_h[-100:]})
                    st.line_chart({"Doppler": offset_h[-100:]})
                    st.line_chart({"Schumann_7.83": sch_h[-100:]})
                    if err_h:
                        st.line_chart({"AE err": err_h[-100:]})


def run_multimodal_cli(
    center_freq: float,
    sample_rate: float,
    duration: float,
    dwell_s: float,
    sim_mode: bool = True,
    show_ae: bool = True,
    every: int = 5,
) -> None:
    """Monitora RF+ELF simultaneamente; alerta anomalias do autoencoder."""
    gps = GPSSync()
    detector = _build_demo_detector() if show_ae else None
    print(f"GPS: {gps.describe()}")
    print("grupo | RF SNR | picos | ELF 7.83 | f_est      | coerência | err AE")
    print("-" * 78)

    with AntSDRCapture(
        center_freq=center_freq,
        sample_rate=sample_rate,
        bandwidth=min(_config.BANDWIDTH_DEFAULT_HZ, sample_rate),
        duration=duration,
        sim_mode=sim_mode,
    ) as sdr, ELFLoop(seed=0) as elf:
        start = time.time()
        frame = 0
        while time.time() - start < dwell_s:
            frame += 1
            if frame % every:
                continue
            gps_time = gps.get_pps_timestamp()
            rfeat = extract_features(
                sdr.capture(), sample_rate=sdr.sample_rate,
                center_freq=sdr.center_freq,
            )
            esig = elf.capture(duration=duration, gps_time=gps_time)
            efeat = elf.extract_schumann_features(esig)
            coherent = gps.within_tolerance(gps_time, elf.last_timestamps[0])

            err_str, alert = "", ""
            if detector is not None:
                res = detector.detect(
                    rfeat["feature_vector"][None, :],
                    efeat["feature_vector"][None, :],
                )
                err_str = f"{res['err_total'][0]:.4f}"
                if res["is_anomaly"][0]:
                    alert = " *** ANOMALIA TEMPORAL ***"
            print(
                f"{frame:>5} | {rfeat['snr_db']:5.1f} | {rfeat['num_peaks']:4d} | "
                f"{efeat['amp_fundamental']:7.3f} | "
                f"{efeat['frequency_est_hz']:7.2f} Hz | "
                f"{'ok' if coherent else 'FORA':>9} | {err_str:>7}{alert}",
                flush=True,
            )


def _build_demo_detector(normal_n=240, epochs=160, seed=0):
    """Autoencoder de demonstração treinado sobre 'normalidade' sintética."""
    from .autoencoder import ReconstructionAnomalyDetector

    rng = np.random.default_rng(seed)
    rf = rng.standard_normal((normal_n, _config.RF_FEATURE_DIM))
    elf = rng.standard_normal((normal_n, _config.ELF_FEATURE_DIM))
    det = ReconstructionAnomalyDetector(
        rf_dim=_config.RF_FEATURE_DIM,
        elf_dim=_config.ELF_FEATURE_DIM,
        latent_dim=_config.AE_LATENT_DIM,
    )
    det.fit(rf, elf, epochs=epochs, batch_size=32, verbose=0)
    return det


def main(argv=None) -> int:
    """Entry point: `python -m arkhe_rf.dashboard`."""
    ap = argparse.ArgumentParser(prog="arkhe_rf.dashboard", description=__doc__)
    ap.add_argument("--target", choices=[t["name"] for t in _config.STARLINK_DTC_TARGETS],
                    default="DOWNLINK_PCS")
    ap.add_argument("--sample-rate", type=float, default=_config.SAMPLE_RATE_DEFAULT)
    ap.add_argument("--duration", type=float, default=0.5)
    ap.add_argument("--dwell", type=float, default=_config.DWELL_TIME_PER_TARGET_S)
    ap.add_argument("--real", action="store_true", help="força backend SoapySDR real")
    ap.add_argument("--sim", dest="sim", action="store_true")
    ap.add_argument("--streamlit", action="store_true")
    ap.add_argument("--elf", action="store_true",
                    help="modo multi-modal: RF + ELF + autoencoder")
    args = ap.parse_args(argv)

    target = next(t for t in _config.STARLINK_DTC_TARGETS if t["name"] == args.target)
    if args.streamlit:
        return run_streamlit()  # type: ignore[return-value]
    if args.elf:
        run_multimodal_cli(
            center_freq=target["freq"],
            sample_rate=args.sample_rate,
            duration=args.duration,
            dwell_s=args.dwell,
            sim_mode=not args.real,
        )
        return 0

    run_cli(
        center_freq=target["freq"],
        sample_rate=args.sample_rate,
        duration=args.duration,
        dwell_s=args.dwell,
        sim_mode=not args.real,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())