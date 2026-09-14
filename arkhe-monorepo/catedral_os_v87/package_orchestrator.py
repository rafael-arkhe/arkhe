#!/usr/bin/env python3
"""
Catedral OS — Orquestrador de Pacotes (Substrato 210)
=====================================================
Gerencia clonagem, compilação e integração dos repositórios mapeados.
"""

import subprocess
import os
import json
import sys
from pathlib import Path
from typing import Dict, List, Optional

PACKAGE_MAP = {
    "prolog": {
        "swipl": "https://github.com/SWI-Prolog/swipl-devel.git",
        "prolog-mcp": "https://github.com/umuro/prolog-mcp.git",
        "prolog-ai": "https://github.com/ai-university-aiu/PrologAI.git",
        "nexus_agi": "https://github.com/DOUGLASDAVIS08161978/nexus_agi.git",
    },
    "robotics": {
        "knowrob": "https://github.com/knowrob/knowrob.git",
        "rosprolog": "https://github.com/knowrob/rosprolog.git",
        "rclswi": "https://github.com/guillaumeautran/rclswi.git",
        "prolog_planner": "https://github.com/davidedema/prolog_planner.git",
    },
    "nanophotonics": {
        "cgan": "https://github.com/metaphotonics/Inverse-metasurface-design-CGAN.git",
        "frictional_metasurfaces": "https://github.com/JBil8/frictional_metasurfaces_inverse_design.git",
        "psat": "https://github.com/taehun-k/psat.git",
        "inverse_design": "https://github.com/yaya722/inverse_design.git",
        "beamz": "https://github.com/beamzorg/beamz.git",
        "autophotonic": "https://github.com/flexcompute/autophotonicdesign.git",
    },
    "6g": {
        "deterministic": "https://github.com/ustutt-ipvs-vs/6GDetCom_MKFirm.git",
        "nextgsim": "https://github.com/NextgCoreLab/nextgsim.git",
        "iodsim": "https://github.com/telematics-lab/IoD_Sim.git",
        "rust6g": "https://github.com/j143/6g.git",
    },
    "qkd": {
        "qosst": "https://github.com/QOSST/qosst.git",
        "quantumshield": "https://github.com/punyamodi/QuantumShield-QKD-Platform.git",
        "quantum_guard": "https://github.com/hallucinaut/quantum-guard.git",
        "bb84": "https://github.com/lupetenazzi/quantum-cryptography-bb84.git",
        "qkd_contingency": "https://github.com/naya-nagy/QKD-Contingency-NoClassicalChannel.git",
    },
    "formal": {
        "symbiyosys": "https://github.com/YosysHQ/SymbiYosys.git",
        "yosys": "https://github.com/YosysHQ/yosys.git",
        "lean4": "https://github.com/leanprover/lean4.git",
    },
}


class PackageOrchestrator:
    def __init__(self, base_dir: str = "./catedral_deps"):
        self.base_dir = Path(base_dir)
        self.base_dir.mkdir(exist_ok=True)

    def clone_all(self, shallow: bool = True, only: Optional[List[str]] = None):
        """Clona todos os repositórios do mapa."""
        results = {}
        for category, repos in PACKAGE_MAP.items():
            results[category] = {}
            for name, url in repos.items():
                if only is not None and name not in only:
                    continue
                target = self.base_dir / name
                if target.exists() and (target / ".git").exists():
                    print(f"✅ {name} já existe.")
                    results[category][name] = "exists"
                    continue
                print(f"📦 Clonando {name} ({url})...")
                cmd = ["git", "clone"]
                if shallow:
                    cmd.append("--depth")
                    cmd.append("1")
                cmd.append(url)
                cmd.append(str(target))
                proc = subprocess.run(cmd, capture_output=True, text=True)
                if proc.returncode == 0:
                    print(f"   ✅ {name} concluído.")
                    results[category][name] = "ok"
                else:
                    print(f"   ⚠️  Falha: {name}")
                    if proc.stderr:
                        print(f"      {proc.stderr.strip().splitlines()[-1] if proc.stderr.strip() else ''}")
                    results[category][name] = "failed"
        return results

    def generate_manifest(self) -> Dict:
        """Gera um manifesto com os hashes e versões dos repositórios."""
        manifest = {"generated": True, "repositories": {}}
        for category, repos in PACKAGE_MAP.items():
            manifest["repositories"][category] = {}
            for name, url in repos.items():
                target = self.base_dir / name
                if target.exists():
                    result = subprocess.run(
                        ["git", "-C", str(target), "rev-parse", "HEAD"],
                        capture_output=True, text=True
                    )
                    branch = subprocess.run(
                        ["git", "-C", str(target), "rev-parse", "--abbrev-ref", "HEAD"],
                        capture_output=True, text=True
                    )
                    manifest["repositories"][category][name] = {
                        "url": url,
                        "hash": result.stdout.strip(),
                        "branch": branch.stdout.strip() or "detached",
                        "path": str(target)
                    }
        return manifest


def main():
    depth = 1 if "--shallow" in sys.argv else None
    only = None
    if "--only" in sys.argv:
        idx = sys.argv.index("--only")
        only = sys.argv[idx + 1].split(",")

    orchestrator = PackageOrchestrator()
    print(f"Base de dependências: {orchestrator.base_dir.resolve()}")
    results = orchestrator.clone_all(shallow=(depth is not None), only=only)

    manifest = orchestrator.generate_manifest()
    manifest_file = orchestrator.base_dir / "manifest.json"
    with open(manifest_file, "w") as f:
        json.dump(manifest, f, indent=2)
    print(f"📋 Manifesto gerado em {manifest_file}")

    # Resumo
    total = sum(len(v) for v in results.values())
    ok = sum(1 for v in results.values() for s in v.values() if s == "ok")
    exists = sum(1 for v in results.values() for s in v.values() if s == "exists")
    failed = sum(1 for v in results.values() for s in v.values() if s == "failed")
    print(f"\nResumo: {ok} clonados, {exists} já existentes, {failed} falhas (de {total} tentativas)")


if __name__ == "__main__":
    main()
