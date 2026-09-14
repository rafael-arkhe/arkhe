"""
Explosão de Coulomb 2D (PIC) com saturação não linear DPDM.

Íons em repulsão coulombiana são acelerados pelas forças de Coulomb (O(N²)
vetorizada) e pela força ponderomotriz do campo de ondas de Langmuir. Quando
a energia cinética média excede a energia térmica eletrônica, modos de maior
k (Langmuir / íon-acústicos) são excitados, gerando perturbação de densidade
δn/n e deslocando a frequência de plasma — a assinatura de saturação
prevista por arXiv:2510.13956 (Hook, Huang & Shalaby, 2025).

Integração Velocity Verlet. Exemplo:

    sim = CoulombExplosion2D_DPDM(n_ions=500, electron_temp_ev=0.05, seed=1)
    for _ in range(200):
        sim.step(dt=1e-14)
    print(sim.get_saturation_metrics())
"""

import numpy as np
from scipy.constants import e, epsilon_0

__all__ = ["CoulombExplosion2D_DPDM"]


class CoulombExplosion2D_DPDM:
    """
    Simulação PIC 2D da explosão de Coulomb com acoplamento de modos.

    Args:
        n_ions: Número de íons.
        r_max: Raio inicial do pacote (m).
        charge_state: Carga dos íons (unidades de e).
        mass_ion: Massa do íon (kg) — padrão deutério.
        electron_temp_ev: Temperatura eletrônica (eV) — define o limiar de
            energia térmica que dispara a excitação de modos de maior k.
        seed: Semente do RNG (determinismo em testes).
    """

    def __init__(
        self,
        n_ions: int = 500,
        r_max: float = 1e-6,
        charge_state: int = 1,
        mass_ion: float = 3.34e-27,
        electron_temp_ev: float = 10.0,
        seed: int | None = None,
    ):
        self.n_ions = int(n_ions)
        self.charge = float(charge_state) * e
        self.mass = float(mass_ion)
        self.softening = 1e-9
        self.electron_temp_ev = float(electron_temp_ev)
        self.thermal_energy_j = self.electron_temp_ev * 1.602e-19

        rng = np.random.default_rng(seed)
        theta = rng.uniform(0, 2 * np.pi, self.n_ions)
        r = r_max * np.sqrt(rng.uniform(0, 1, self.n_ions))
        self.x = r * np.cos(theta)
        self.y = r * np.sin(theta)
        self.vx = rng.normal(0, 1e3, self.n_ions)
        self.vy = rng.normal(0, 1e3, self.n_ions)

        # Campos e modos (saturação DPDM).
        self.E_field = np.zeros((self.n_ions, 2))
        self.density_perturb = np.zeros(self.n_ions)
        self.k_modes: list[dict] = []

        self.fx = np.zeros(self.n_ions)
        self.fy = np.zeros(self.n_ions)
        self.n_steps = 0
        self.r_max = float(r_max)
        self.omega_p = 1.0  # normalizado

    # ------------------------------------------------------------------
    def _mode_field(self):
        """Reconstrói o campo E a partir dos modos (potencial escalar)."""
        if not self.k_modes:
            self.E_field = np.zeros((self.n_ions, 2))
            self.density_perturb = np.zeros(self.n_ions)
            return

        phase = np.zeros(self.n_ions)
        ex = np.zeros(self.n_ions)
        ey = np.zeros(self.n_ions)
        dn = np.zeros(self.n_ions)
        for m in self.k_modes:
            arg = m["kx"] * self.x + m["ky"] * self.y
            amp = m["amplitude"]
            phase += amp * np.sin(arg)
            ex -= amp * m["kx"] * np.cos(arg)
            ey -= amp * m["ky"] * np.cos(arg)
            dn += amp * np.sin(arg)
        self.E_field = np.column_stack([ex, ey])
        self.density_perturb = dn
        self.omega_p = 1.0 * (1.0 + 0.5 * np.mean(dn))

    # ------------------------------------------------------------------
    def compute_ponderomotive_force(self):
        """Força ponderomotriz F_p = −(q²/(4 m ω²)) ∇|E|²."""
        Ex = self.E_field[:, 0]
        Ey = self.E_field[:, 1]
        E_sq = Ex**2 + Ey**2
        grad_x = np.gradient(E_sq, self.x)
        grad_y = np.gradient(E_sq, self.y)
        factor = self.charge**2 / (4.0 * self.mass * self.omega_p**2)
        return -factor * grad_x, -factor * grad_y

    # ------------------------------------------------------------------
    def compute_forces_vectorized(self):
        """Força de Coulomb O(N²) vetorizada + força ponderomotriz."""
        dx = self.x[:, np.newaxis] - self.x[np.newaxis, :]
        dy = self.y[:, np.newaxis] - self.y[np.newaxis, :]
        r_sq = dx**2 + dy**2 + self.softening**2
        r = np.sqrt(r_sq)
        force_mag = (1.0 / (4 * np.pi * epsilon_0)) * self.charge**2 / r_sq
        np.fill_diagonal(force_mag, 0.0)
        fx = np.sum(force_mag * (dx / r), axis=1)
        fy = np.sum(force_mag * (dy / r), axis=1)

        fx_p, fy_p = self.compute_ponderomotive_force()
        fx += fx_p
        fy += fy_p
        return fx, fy

    # ------------------------------------------------------------------
    def excite_higher_k_modes(self):
        """
        Excita modos de maior k quando a energia cinética excede a térmica.

        O excesso de energia é convertido em modos de Langmuir / íon-acústicos
        com k aleatório na faixa [0.5, 2.0]·k₀ (cascata), atualizando o campo
        E e a perturbação de densidade — e portanto ω_p efetiva.
        """
        kinetic_energy = 0.5 * self.mass * float(np.mean(self.vx**2 + self.vy**2))
        self._kinetic_energy = kinetic_energy

        if kinetic_energy > self.thermal_energy_j * 0.5:
            excess_ratio = (kinetic_energy / max(self.thermal_energy_j, 1e-30)) - 0.5
            n_modes = int(min(10 * excess_ratio, 64))

            rng = np.random.default_rng(self.n_steps)
            k0 = 2 * np.pi / self.r_max
            for _ in range(n_modes):
                kx = rng.uniform(0.5, 2.0) * k0
                ky = rng.uniform(0.5, 2.0) * k0
                amp = rng.uniform(0.01, 0.1) * excess_ratio
                self.k_modes.append(
                    {
                        "kx": kx,
                        "ky": ky,
                        "amplitude": amp,
                        "type": rng.choice(["Langmuir", "Ion_Acoustic"]),
                    }
                )
            self._mode_field()

    # ------------------------------------------------------------------
    def step(self, dt: float = 1e-14):
        """Integração Velocity Verlet com saturação DPDM."""
        self.excite_higher_k_modes()

        fx, fy = self.compute_forces_vectorized()
        self.x += self.vx * dt + 0.5 * fx * dt**2 / self.mass
        self.y += self.vy * dt + 0.5 * fy * dt**2 / self.mass

        fx_new, fy_new = self.compute_forces_vectorized()
        self.vx += 0.5 * (fx + fx_new) * dt / self.mass
        self.vy += 0.5 * (fy + fy_new) * dt / self.mass

        self.fx, self.fy = fx_new, fy_new
        self.n_steps += 1

    # ------------------------------------------------------------------
    def get_saturation_metrics(self) -> dict:
        """Métricas de saturação não linear (DPDM)."""
        kinetic = 0.5 * self.mass * float(np.mean(self.vx**2 + self.vy**2))
        saturation_fraction = min(kinetic / max(self.thermal_energy_j, 1e-30), 1.0)
        return {
            "saturation_fraction": float(saturation_fraction),
            "n_modes": len(self.k_modes),
            "max_density_perturb": float(np.max(np.abs(self.density_perturb)))
            if len(self.density_perturb)
            else 0.0,
            "omega_p_eff": float(self.omega_p),
            "kinetic_energy_j": float(kinetic),
            "thermal_energy_j": float(self.thermal_energy_j),
        }


if __name__ == "__main__":
    sim = CoulombExplosion2D_DPDM(n_ions=500, electron_temp_ev=0.05, seed=1)
    for _ in range(300):
        sim.step(dt=1e-14)
    print(sim.get_saturation_metrics())
