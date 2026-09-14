#!/usr/bin/env python3
"""
Substrato 238 — PLX Supersonic Jet (Registro de Solvers + Workflow PJMIF)

Registra os 10 solvers de plasma (referência LANL: FronTier, FLASH, OSIRIS,
HELIOS, LSP, ePLAS, Nautilus, USIM, HIGRAD, SPH) e orquestra o pipeline PJMIF
(3 fases). O workflow é um gerenciador de seleção/execução declarativo — não
executa códigos reais de plasmática, apenas modela o ciclo de vida.
"""

from typing import Dict, List, Optional


class PLXSimulator:
    """Gerenciador de solvers PLX + pipeline PJMIF."""

    # Tabela de solvers de referência (LANL PLX / sociable)
    SOLVERS = {
        "FronTier": "front_tracking",
        "FLASH": "mhd_radiation",
        "OSIRIS": "particle_in_cell",
        "HELIOS": "lagrangian_1d",
        "LSP": "hybrid_pic",
        "ePLAS": "multi_fluid",
        "Nautilus": "gas_dynamics",
        "USIM": "multi_fluid_3d",
        "HIGRAD": "les_compressible",
        "SPH": "smoothed_particle",
    }

    def __init__(self, active_solver: str = "FLASH"):
        self.solvers = dict(self.SOLVERS)
        if active_solver not in self.solvers:
            raise ValueError(f"Solvers conhecidos: {list(self.solvers)}")
        self.active = active_solver
        self.phases_run: List[Dict] = []

    def register_solver(self, name: str, solver_type: str) -> Dict:
        self.solvers[name] = solver_type
        return {"status": "registered", "name": name, "type": solver_type}

    def select_solver(self, name: str) -> Dict:
        if name not in self.solvers:
            return {"status": "error", "message": f"Solvers conhecidos: {list(self.solvers)}"}
        self.active = name
        return {"status": "ok", "name": name}

    def list_solvers(self) -> List[Dict]:
        return [{"name": n, "type": t} for n, t in self.solvers.items()]

    def _run_phase(self, phase: int, solver: str) -> Dict:
        record = {"phase": phase, "solver": solver, "status": "completed"}
        self.phases_run.append(record)
        return record

    def run_pjmif(self, solver: Optional[str] = None) -> Dict:
        """Executa o pipeline PJMIF de 3 fases sobre o solver ativo."""
        if solver is not None:
            if solver not in self.solvers:
                return {"status": "error", "message": f"Solvers conhecidos: {list(self.solvers)}"}
            self.active = solver

        self.phases_run = []
        for phase in (1, 2, 3):
            self._run_phase(phase, self.active)

        return {
            "status": "success",
            "solver": self.active,
            "solver_type": self.solvers[self.active],
            "phases": [1, 2, 3],
            "phase_records": list(self.phases_run),
        }


if __name__ == "__main__":
    plx = PLXSimulator()
    plx.select_solver("FLASH")
    res = plx.run_pjmif()
    print(f"PLX: solver={res['solver']} type={res['solver_type']} fases={res['phases']}")
