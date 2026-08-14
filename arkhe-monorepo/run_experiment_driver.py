#!/usr/bin/env python3
"""
run_experiment_driver.py
Executa o pipeline completo do protocolo v5.1:
1. FPGA threshold detection (lensing_threshold_detector_v5_1.py)
2. QPU emulator validation (emulator_s2_validation.py)
Consolida os relatorios JSON de ambos em um relatorio final.

Uso:
  python run_experiment_driver.py [--p-depol 0.0065] [--keep-json]
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time


def run_script(script_name, args, description):
    """Executa um script Python e captura a saida (stdout/stderr)."""
    print(f"\n{'=' * 70}")
    print(f"EXECUTANDO: {description}")
    print(f"Script: {script_name}")
    print('=' * 70)

    if not os.path.exists(script_name):
        print(f"ERRO: {script_name} nao encontrado. Abortando.")
        return None

    start = time.time()
    result = subprocess.run(
        [sys.executable, script_name] + args,
        capture_output=True,
        text=True,
    )
    elapsed = time.time() - start

    if result.returncode != 0:
        print(f"ERRO: {script_name} falhou (codigo {result.returncode})")
        print("STDERR:")
        print(result.stderr)
        return None

    print(f"OK: {script_name} concluido em {elapsed:.1f}s. stdout capturado.")
    return result.stdout


def load_json(path):
    try:
        with open(path, 'r', encoding='utf-8') as fh:
            return json.load(fh)
    except Exception as exc:  # noqa: BLE001
        print(f"ERRO ao ler JSON {path}: {exc}")
        return None


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--p-depol', type=float, default=0.0065,
                    help='erro depolarizante por qubit no emulador')
    ap.add_argument('--n', type=int, default=50000,
                    help='amostras/medicao do detector FPGA')
    ap.add_argument('--keep-json', action='store_true',
                    help='mantem os JSONs intermediarios no diretorio atual')
    args = ap.parse_args(argv)

    tmpdir = tempfile.mkdtemp(prefix='v51_driver_')
    fpga_json = os.path.join(tmpdir, 'fpga.json')
    emul_json = os.path.join(tmpdir, 'emulator.json')

    print("=" * 70)
    print("PROTOCOLO v5.1 — PIPELINE COMPLETO")
    print("FPGA + Emulador QPU")
    print("=" * 70)

    # 1. FPGA threshold detection
    fpga_out = run_script(
        'lensing_threshold_detector_v5_1.py',
        ['--n', str(args.n), '--json', fpga_json],
        'FPGA Threshold Detection',
    )

    # 2. Emulator validation
    emul_out = run_script(
        'emulator_s2_validation.py',
        ['--p-depol', str(args.p_depol), '--verify', '--json', emul_json],
        'QPU Emulator Validation',
    )

    fpga_data = load_json(fpga_json) if fpga_out else None
    emul_data = load_json(emul_json) if emul_out else None

    # 3. Relatorio consolidado
    print("\n" + "=" * 70)
    print("RELATORIO FINAL — PROTOCOLO v5.1")
    print("=" * 70)

    if fpga_data:
        ok = fpga_data.get('cross_check_ok', False)
        print("\n--- FPGA Threshold ---")
        print(f"  target D_KL        = {fpga_data['target_kl']:.1f} bit")
        print(f"  p_target  (A spline)= {fpga_data['p_target_a']:.4f}")
        print(f"  p_operate (A)       = {fpga_data['p_operate_a']:.4f}")
        print(f"  p_target  (B bis)   = {fpga_data['p_target_b']:.4f}")
        print(f"  p_operate (B)       = {fpga_data['p_operate_b']:.4f}")
        print(f"  KL no ponto B       = {fpga_data['kl_at_target']:.4f} "
              f"+/- {fpga_data['kl_std']:.4f} bit (ruido MC)")
        print(f"  cross-check         = "
              f"{'OK' if ok else 'DIVERGENCIA'} "
              f"(|dA-dB| = {fpga_data['cross_check_diff']:.4f})")
    else:
        print("\n--- FPGA Threshold: FALHA ---")

    if emul_data:
        print("\n--- QPU Emulator Validation ---")
        print(f"  ruido por qubit     = p={emul_data['p_depol']:.4f} "
              f"(fidelity 1q = {emul_data['fidelity_1q']:.4f})")
        all_pass = True
        for r in emul_data['rows']:
            print(f"  k={r['k']}: S2 = {r['S2_est']:.3f} +/- {r['S2_std']:.3f} "
                  f"(teo = {r['S2_theory']:.3f}) -> {r['status']}")
            if r['status'] != 'OK':
                all_pass = False
        print("\n" + ("EMULADOR: TODOS OS TESTES PASSARAM."
                      if all_pass else "EMULADOR: ALGUMAS METRICAS FALHARAM."))
    else:
        print("\n--- QPU Emulator: FALHA ---")
        all_pass = False

    # 4. Resumo final
    print("\n" + "=" * 70)
    print("STATUS FINAL")
    if fpga_data and fpga_data.get('cross_check_ok', False) and emul_data \
            and emul_data.get('all_ok', False):
        print("PROTOCOLO v5.1 VALIDADO COMPLETAMENTE.")
        print("   FPGA threshold definido e emulador QPU aprovado.")
        print("   Pronto para execucao em hardware real (sujeito a orcamento).")
    else:
        print("PROTOCOLO PARCIALMENTE VALIDADO.")
        print("   Verifique as falhas acima.")
    print("=" * 70)

    if not args.keep_json:
        import shutil
        shutil.rmtree(tmpdir, ignore_errors=True)
    else:
        print(f"JSONs intermediarios: {tmpdir}")

    ok = bool(fpga_data and fpga_data.get('cross_check_ok', False)
              and emul_data and emul_data.get('all_ok', False))
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main())
