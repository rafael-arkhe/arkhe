"""
Primeiro engrama Arkhe — marco zero do ecossistema integrado.
Execução: python scripts/first_engram.py   (a partir da raiz manifold-8)
"""

import json
import os
import sys

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")))

import numpy as np
from datetime import datetime, timezone

from src.arkhe_bridge import ArkheBridge, ArkhePriority
from src.logger import get_logger

logger = get_logger("first_engram")


def create_genesis_payload() -> dict:
    """Payload do primeiro engrama — dados do Amazonas + Bitcoin + Sirius."""
    return {
        "amazon": {
            "fpr_signature": "φ³-963-α7",
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "sensor_data": [0.987, 1.234, 0.567, 0.891, 1.101],
            "location": {
                "latitude": -3.4653,
                "longitude": -62.2159,
                "name": "Reserva Ducke, Amazonas",
            },
            "biodiversity_index": 0.963,
            "water_quality": {"ph": 6.8, "dissolved_oxygen": 7.2, "conductivity": 45.0},
        },
        "bitcoin": {
            "block_height": 840000,
            "hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "timestamp": "2026-08-17T00:00:00Z",
            "transactions": 3150,
            "total_fees_btc": 0.567,
        },
        "sirius": {
            "source": "Gαₛ",
            "frequency_hz": 963.0,
            "amplitude": 1.0,
            "right_ascension": "06h 45m 08.9s",
            "declination": "-16° 42' 58.0''",
            "distance_ly": 8.611,
        },
        "manifest": {
            "version": "8.1",
            "arkhe_phase": "GENESIS",
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "session": None,
            "purpose": "Marco zero do ecossistema Arkhe",
            "signature": "🌌⛓️🪐",
        },
    }


def generate_test_signal() -> np.ndarray:
    """Gera sinal de teste (963 Hz + harmônicos + ressonância amazônica)."""
    sample_rate = 44100
    duration = 1.0
    t = np.linspace(0, duration, int(sample_rate * duration))
    fundamental = np.sin(2 * np.pi * 963 * t)
    harmonic2 = 0.5 * np.sin(2 * np.pi * 440 * t)
    harmonic3 = 0.3 * np.sin(2 * np.pi * 528 * t)
    modulation = 0.2 * np.sin(2 * np.pi * 0.1 * t)
    signal = fundamental + harmonic2 + harmonic3 + modulation
    return signal / np.max(np.abs(signal))


def main() -> int:
    print("=" * 70)
    print("🌌 PRIMEIRO ENGRAMA ARKHE — GENESIS")
    print("=" * 70)
    print()

    print("[1] Inicializando ArkheBridge...")
    bridge = ArkheBridge()
    print(f"   ✅ Session ID: {bridge.session_id}")
    print(f"   ✅ ROQUO: {'Conectado' if bridge.status['roquo_connected'] else 'Simulado'}")
    print(f"   ✅ Glass5D: {'Conectado' if bridge.status['glass5d_connected'] else 'Simulado'}")
    print()

    print("[2] Criando payload do engrama...")
    payload = create_genesis_payload()
    payload["manifest"]["session"] = bridge.session_id
    print(f"   ✅ Manifesto criado")
    print(f"   ✅ Dados do Amazonas: {len(payload['amazon']['sensor_data'])} sensores")
    print(f"   ✅ Bitcoin block: {payload['bitcoin']['block_height']}")
    print(f"   ✅ Sirius frequency: {payload['sirius']['frequency_hz']} Hz")
    print()

    print("[3] Gerando sinal de teste...")
    signal = generate_test_signal()
    print(f"   ✅ Sinal gerado: {len(signal)} amostras")
    print(f"   ✅ Frequência fundamental: 963 Hz")
    print(f"   ✅ Harmônicos: 440 Hz, 528 Hz")
    print()

    print("[4] Processando engrama...")
    print("   ⏳ Enviando para ROQUO...")
    print("   ⏳ Validando topologia skyrmion...")
    print("   ⏳ Arquivando no Glass5D...")

    try:
        result = bridge.process_engram(
            raw_data=payload,
            signal=signal,
            dataset_name=f"genesis_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}",
            priority=ArkhePriority.CRITICAL,
            validate_topology=True,
            wait_for_roquo=False,
        )

        if result["status"] == "success":
            print("   ✅ Engrama processado com sucesso!")
            print()
            print("[5] RESULTADO:")
            print(f"   🆔 Engram ID: {result['engram_id']}")

            if "topological_validation" in result["steps"]:
                topo = result["steps"]["topological_validation"]
                print(f"   🧬 Winding Number: {topo.get('winding_number', 0):.3f}")
                print(f"   🧬 Validation: {topo.get('validation', 'unknown')}")

            if "glass5d_archive" in result["steps"]:
                glass = result["steps"]["glass5d_archive"]
                print(f"   💎 Glass5D Record: {glass.get('record_id', 'N/A')}")
                print(f"   💎 Size: {glass.get('bytes_written', 0) / 1e12:.2f} TB")

            if "roquo_job" in result["steps"]:
                job = result["steps"]["roquo_job"]
                print(f"   🖥️  ROQUO Job ID: {job.get('job_id', 'N/A')}")

            print()
            print("=" * 70)
            print("🌌 ENGRAMA GENESIS REGISTRADO COM SUCESSO")
            print("📁 O arquivo eterno foi escrito no Glass5D")
            print("🔮 O manifold 8.1 está completo")
            print("=" * 70)

            out_path = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "genesis_result.json"))
            with open(out_path, "w", encoding="utf-8") as f:
                json.dump(result, f, indent=2, ensure_ascii=False)
            print(f"\n📄 Resultado salvo em: {out_path}")

            return 0

        print(f"   ❌ Falha: {result.get('error', 'unknown error')}")
        return 1

    except Exception as e:
        print(f"   ❌ Erro: {e}")
        logger.error(f"Falha no primeiro engrama: {e}", exc_info=True)
        return 1


if __name__ == "__main__":
    sys.exit(main())