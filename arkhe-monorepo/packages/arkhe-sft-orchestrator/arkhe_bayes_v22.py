"""Pipeline bayesiano v2.2: ressonancia plasma-vacuo QED em magnetares.

Inferencia Bayesiana (dynesty) da resonancia plasma-vacuo QED no perfil de
polarizacao PD(E) de magnetares, usando como baseline o modelo M1
(magthomscatt_wrapper) ajustado aos dados reais do IXPE.

Modelos comparados:
    H1   : sem resonancia (somente baseline M1)
    H1.5 : dip gaussiano (dip_type='gaussian')
    H2   : dip Voigt (dip_type='voigt')

Melhorias v2.2 (ARKHE-PIPELINE-IMPROVEMENTS-2026-08-06):
    - f_FF (correcao de campo finito, Abbassi et al. 2026) propagado como
      parametro de nuisance com prior Gaussiano (incerteza real, nao pontual)
    - Escala de Jeffreys em log10 com valor numerico retornado
    - Seed fixo no dynesty (reproducibilidade total)
    - Normalizacao de nomes de fonte em VERIFIED_FF_DATA
    - Validacao de dominio na likelihood (evita erros numericos)
    - phase_averaged configuravel via Dataset (suporta dados fase-resolvidos)
    - dlogz configuravel (default 0.01)

Notas tecnicas detalhadas em NOTES.md (f_FF, Borel-Pade, Voigt, Jeffreys).

Dependencias:
    pip install dynesty numpy scipy
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Callable, Optional, Tuple

import numpy as np
from scipy.special import wofz
from scipy.stats import norm


# ---------------------------------------------------------------------------
# CONSTANTES FISICAS
# ---------------------------------------------------------------------------

ALPHA_QED = 1.0 / 137.035999
B_CRIT_GAUSS = 4.414e13


# ---------------------------------------------------------------------------
# 1. Correcao de campo finito — abordagem de tres camadas
# ---------------------------------------------------------------------------

# Dados VERIFICADOS de Abbassi et al. 2026 (arXiv:2607.06422)
VERIFIED_FF_DATA = {
    "1E1547.0-5408":  {"B": 3.2e14, "f_FF": 1.14},
    "1RXSJ1708-4009": {"B": 4.6e14, "f_FF": 1.37},
    "SGR1806-20":     {"B": 2.0e15, "f_FF": 2.13},
}

# Incertezas conservadoras adotadas (Seção 3 de Abbassi et al. 2026,
# dependencia suave em B/B_c): verificada / interpolada / extrapolada.
FF_SIGMA_BY_STATUS = {"verified": 0.01, "interpolated": 0.05, "extrapolated": 0.15}

# Modelo fenomenologico ajustado (polinomio em B/B_c)
# Ajustado aos 3 pontos verificados. Usado apenas para INTERPOLACAO.
_FFa, _FFb = 0.031190, -0.000137


def normalize_source_name(name: str) -> Optional[str]:
    """Normaliza o nome da fonte para as chaves de VERIFIED_FF_DATA.

    Tenta, nesta ordem: (1) match exato, (2) substring, (3) match sem
    espacos/hifens e em maiusculas. Retorna None se nao houver correspondencia.
    """
    if name in VERIFIED_FF_DATA:
        return name
    for key in VERIFIED_FF_DATA:
        if name in key or key in name:
            return key
    clean = name.replace(" ", "").replace("-", "").upper()
    for key in VERIFIED_FF_DATA:
        if clean in key.replace(" ", "").replace("-", "").upper():
            return key
    return None


def _ff_phenomenological(B_over_Bc: float) -> float:
    """Modelo fenomenologico para f_FF (INTERPOLACAO apenas)."""
    x = B_over_Bc
    return 1 + _FFa * x + _FFb * x**2


def get_f_FF(
    B_gauss: float,
    source_name: Optional[str] = None,
    user_f_FF: Optional[float] = None
) -> Tuple[float, str]:
    """Retorna fator de correcao de campo finito f_FF.

    Hierarquia de prioridade:
      1. user_f_FF: valor especificado pelo usuario (mais alta)
      2. VERIFIED_FF_DATA: valor tabulado de Abbassi et al.
      3. _ff_phenomenological: interpolacao/extrapolacao fenomenologica

    Retorna: (f_FF, status)
      status in {'user_specified', 'verified', 'interpolated', 'extrapolated'}
    """
    if user_f_FF is not None:
        return user_f_FF, "user_specified"

    if source_name:
        norm_name = normalize_source_name(source_name)
        if norm_name:
            entry = VERIFIED_FF_DATA[norm_name]
            if abs(B_gauss - entry["B"]) / entry["B"] < 0.2:
                return entry["f_FF"], "verified"

    B_bc = B_gauss / B_CRIT_GAUSS
    B_min = min(v["B"] for v in VERIFIED_FF_DATA.values()) / B_CRIT_GAUSS
    B_max = max(v["B"] for v in VERIFIED_FF_DATA.values()) / B_CRIT_GAUSS

    f = max(_ff_phenomenological(B_bc), 1.0)

    if B_bc < B_min or B_bc > B_max:
        status = "extrapolated"
    else:
        status = "interpolated"

    return f, status


def get_f_FF_with_uncertainty(
    B_gauss: float,
    source_name: Optional[str] = None,
    user_f_FF: Optional[float] = None,
    user_sigma: Optional[float] = None
) -> Tuple[float, float, str]:
    """f_FF com incerteza estimada (para propagacao Bayesiana).

    Retorna: (f_FF, sigma_f, status)
      sigma_f: user_sigma se dado; caso contrario da tabela FF_SIGMA_BY_STATUS.
    """
    f, status = get_f_FF(B_gauss, source_name, user_f_FF)
    if user_sigma is not None:
        sigma = user_sigma
    else:
        sigma = FF_SIGMA_BY_STATUS.get(status, 0.0)
    return f, sigma, status


def apply_finite_field_correction(
    delta_n_weak_field: float,
    B_gauss: float,
    source_name: Optional[str] = None,
    user_f_FF: Optional[float] = None
) -> Tuple[float, str]:
    """Aplica correcao de campo finito."""
    f_FF, status = get_f_FF(B_gauss, source_name, user_f_FF)
    return delta_n_weak_field * f_FF, status


# ---------------------------------------------------------------------------
# 2. Perfil de Voigt NORMALIZADO EM AREA
# ---------------------------------------------------------------------------

def voigt_profile(E: np.ndarray, E_res: float, sigma: float, gamma: float) -> np.ndarray:
    """Perfil de Voigt normalizado em area (∫V dE = 1)."""
    z = ((E - E_res) + 1j * gamma) / (sigma * np.sqrt(2))
    v = np.real(wofz(z))
    return v / (sigma * np.sqrt(2 * np.pi))


def gaussian_dip(E: np.ndarray, E_res: float, sigma: float) -> np.ndarray:
    """Dip gaussiano puro (H1.5). Normalizado em area."""
    return norm.pdf(E, loc=E_res, scale=sigma)


# ---------------------------------------------------------------------------
# 3. Geometria: RVM
# ---------------------------------------------------------------------------

def geometry_angle_theta(phi: np.ndarray, rvm_params: dict) -> np.ndarray:
    alpha = rvm_params["alpha"]
    beta = rvm_params["beta"]
    phi0 = rvm_params.get("phi0", 0.0)
    num = np.sin(alpha) * np.sin(phi - phi0)
    den = (np.sin(alpha + beta) * np.cos(alpha)
           - np.cos(alpha + beta) * np.sin(alpha) * np.cos(phi - phi0))
    return np.arctan2(num, den)


# ---------------------------------------------------------------------------
# 4. Modelo PD(E, phi)
# ---------------------------------------------------------------------------

def pd_model(
    E: np.ndarray,
    phi: np.ndarray,
    theta_params: dict,
    pd_base_fn: Callable[[np.ndarray], np.ndarray],
    eta: float,
    E_res: float,
    sigma: float,
    gamma: float,
    dPD_dip: float,
    with_resonance: bool,
    phase_averaged: bool = True,
    dip_type: str = "voigt",
    f_ff_scale: float = 1.0,
) -> np.ndarray:
    """Modelo PD(E, phi) com dip de resonancia opcional.

    f_ff_scale: razão f_FF/f_FF_ref que escala a profundidade do dip
    (correcao de campo finito acoplada a intensidade da resonancia).
    """
    pd_base = pd_base_fn(E)
    if not with_resonance:
        return pd_base

    if phase_averaged:
        phi_fine = np.linspace(0, 2 * np.pi, 200)
        theta_fine = geometry_angle_theta(phi_fine, theta_params)
        angular_factor = float(np.mean(np.sin(theta_fine) ** 2))
    else:
        theta = geometry_angle_theta(phi, theta_params)
        angular_factor = np.sin(theta) ** 2

    depth = dPD_dip * eta * angular_factor * f_ff_scale

    if dip_type == "voigt":
        dip = depth * voigt_profile(E, E_res, sigma, gamma)
    else:
        dip = depth * gaussian_dip(E, E_res, sigma)

    return pd_base - dip


# ---------------------------------------------------------------------------
# 5. Priors
# ---------------------------------------------------------------------------

class PriorSource(Enum):
    WIDE_UNINFORMATIVE = "wide_uninformative"
    NICER_RCS = "nicer_rcs_independent_window"
    RVM_OUT_OF_BAND = "rvm_polarization_out_of_band"
    FF_CALIB = "finite_field_calibration"


@dataclass
class Prior:
    name: str
    source: PriorSource
    transform: Callable[[float], float]


def build_priors(
    mode: str,
    calib: dict,
    with_f_sigma: bool = True,
    e_res_lo: float = 1.0,
    e_res_hi: float = 15.0,
    f_ff_ref: Optional[float] = None,
    f_ff_sigma: Optional[float] = None,
) -> dict[str, Prior]:
    """Priors para o modelo.

    Se f_ff_ref/f_ff_sigma forem fornecidos, adiciona f_FF como parametro de
    nuisance com prior Gaussiano (propagacao da incerteza de Abbassi et al.),
    escalando a profundidade do dip via f_FF/f_FF_ref.
    """
    if mode not in ("independent", "calibrated"):
        raise ValueError("mode deve ser 'independent' ou 'calibrated'")

    def uniform(lo, hi):
        return lambda u: lo + u * (hi - lo)

    def gaussian(mu, sigma):
        return lambda u: norm.ppf(u, loc=mu, scale=sigma)

    if mode == "independent":
        eta_prior = Prior("eta", PriorSource.WIDE_UNINFORMATIVE, uniform(0.0, 1.0))
        alpha_prior = Prior("alpha_rvm", PriorSource.WIDE_UNINFORMATIVE, uniform(0.0, np.pi))
        beta_prior = Prior("beta_rvm", PriorSource.WIDE_UNINFORMATIVE, uniform(-np.pi/4, np.pi/4))
    else:
        eta_prior = Prior("eta", PriorSource.NICER_RCS, gaussian(calib["eta_mu"], calib["eta_sigma"]))
        alpha_prior = Prior("alpha_rvm", PriorSource.RVM_OUT_OF_BAND, gaussian(calib["alpha_mu"], calib["alpha_sigma"]))
        beta_prior = Prior("beta_rvm", PriorSource.RVM_OUT_OF_BAND, gaussian(calib["beta_mu"], calib["beta_sigma"]))

    common = {
        "E_res": Prior("E_res", PriorSource.WIDE_UNINFORMATIVE, uniform(e_res_lo, e_res_hi)),
        "sigma": Prior("sigma", PriorSource.WIDE_UNINFORMATIVE, uniform(0.05, 3.0)),
        "gamma": Prior("gamma", PriorSource.WIDE_UNINFORMATIVE, uniform(0.01, 3.0)),
        "dPD_dip": Prior("dPD_dip", PriorSource.WIDE_UNINFORMATIVE, uniform(0.0, 0.5)),
        "phi0": Prior("phi0", PriorSource.WIDE_UNINFORMATIVE, uniform(0.0, 2*np.pi)),
    }
    if with_f_sigma:
        common["f_sigma"] = Prior("f_sigma", PriorSource.WIDE_UNINFORMATIVE, uniform(0.5, 3.0))
    if f_ff_ref is not None and f_ff_sigma is not None and f_ff_sigma > 0:
        common["f_FF"] = Prior("f_FF", PriorSource.FF_CALIB, gaussian(f_ff_ref, f_ff_sigma))

    return {"eta": eta_prior, "alpha_rvm": alpha_prior, "beta_rvm": beta_prior, **common}


# ---------------------------------------------------------------------------
# 6. Likelihood e dynesty
# ---------------------------------------------------------------------------

@dataclass
class Dataset:
    E: np.ndarray
    phi: np.ndarray
    PD_obs: np.ndarray
    PD_err: np.ndarray
    pd_base_fn: Callable[[np.ndarray], np.ndarray]
    phase_averaged: bool = True  # False habilita modelagem PD(E, phi) fase-resolvida


def make_prior_transform(priors: dict[str, Prior], with_resonance: bool,
                         dip_type: str = "voigt"):
    if with_resonance:
        keys = ["eta", "alpha_rvm", "beta_rvm", "E_res", "sigma", "dPD_dip", "phi0"]
        if dip_type == "voigt":
            keys.insert(keys.index("sigma") + 1, "gamma")
        if "f_sigma" in priors:
            keys.append("f_sigma")
        if "f_FF" in priors:
            keys.append("f_FF")
    else:
        keys = []

    def prior_transform(u: np.ndarray) -> np.ndarray:
        return np.array([priors[k].transform(ui) for k, ui in zip(keys, u)])

    return prior_transform, keys


def _domain_ok(params: dict, keys: list[str]) -> bool:
    """Validacao de dominio fisico antes de avaliar o modelo.

    So valida parametros presentes; parametros ausentes (ex.: H1 sem dip)
    nao sao restringidos.
    """
    eta = params.get("eta", None)
    if eta is not None and not (0.0 <= eta <= 1.0):
        return False
    e_res = params.get("E_res", None)
    if e_res is not None and (e_res <= 0.0 or e_res > 20.0):
        return False
    sigma = params.get("sigma", None)
    gamma = params.get("gamma", None)
    if sigma is not None and sigma <= 0.0:
        return False
    if gamma is not None and gamma <= 0.0:
        return False
    dpd = params.get("dPD_dip", None)
    if dpd is not None and not (0.0 <= dpd <= 0.5):
        return False
    f_sigma = params.get("f_sigma", None)
    if f_sigma is not None and f_sigma <= 0.0:
        return False
    f_ff = params.get("f_FF", None)
    if f_ff is not None and f_ff <= 0.0:
        return False
    alpha = params.get("alpha_rvm", None)
    beta = params.get("beta_rvm", None)
    if alpha is not None and not (0.0 < alpha < np.pi):
        return False
    if beta is not None and not (-np.pi/2 < beta < np.pi/2):
        return False
    return True


def make_loglikelihood(data: Dataset, with_resonance: bool, keys: list[str],
                       dip_type: str = "voigt", f_ff_ref: float = 1.0):
    def loglike(theta: np.ndarray) -> float:
        params = dict(zip(keys, theta))
        if not _domain_ok(params, keys):
            return -np.inf

        f_sigma = params.get("f_sigma", 1.0)
        f_ff = params.get("f_FF", None)
        f_ff_scale = (f_ff / f_ff_ref) if f_ff is not None else 1.0

        if with_resonance:
            gamma_val = params.get("gamma", 0.001)
            model = pd_model(
                data.E, data.phi,
                theta_params={"alpha": params["alpha_rvm"], "beta": params["beta_rvm"], "phi0": params["phi0"]},
                pd_base_fn=data.pd_base_fn,
                eta=params["eta"], E_res=params["E_res"], sigma=params["sigma"],
                gamma=gamma_val, dPD_dip=params["dPD_dip"],
                with_resonance=True, phase_averaged=data.phase_averaged,
                dip_type=dip_type, f_ff_scale=f_ff_scale,
            )
        else:
            model = data.pd_base_fn(data.E)

        err_scaled = f_sigma * data.PD_err
        resid = (data.PD_obs - model) / err_scaled
        return -0.5 * np.sum(resid**2 + np.log(2 * np.pi * err_scaled**2))

    return loglike


# ---------------------------------------------------------------------------
# 7. Runner dynesty
# ---------------------------------------------------------------------------

# Escala de Jeffreys (1961) em log10 (Kass & Raftery 1995)
JEFFREYS_SCALE = [(1.0, "inconclusivo"), (2.5, "evidencia fraca"), (5.0, "evidencia substancial"),
                  (10.0, "evidencia forte"), (float("inf"), "evidencia decisiva")]


def interpret_bayes_factor(ln_bf: float) -> Tuple[str, float]:
    """Classifica um ln(Fator de Bayes) na escala log10 de Jeffreys.

    Retorna: (rotulo, log10_BF)
    """
    log10_bf = abs(ln_bf) / np.log(10)
    for threshold, label in JEFFREYS_SCALE:
        if log10_bf < threshold:
            return label, log10_bf
    return "evidencia decisiva", log10_bf


def run_nested_sampling(data, priors, with_resonance, dip_type="voigt",
                        nlive=500, dlogz=0.01, seed=42, f_ff_ref=1.0,
                        print_progress=False):
    """Amostragem aninhada com semente fixa para reproducibilidade."""
    import dynesty
    prior_transform, keys = make_prior_transform(priors, with_resonance, dip_type)
    loglike = make_loglikelihood(data, with_resonance, keys, dip_type, f_ff_ref)
    ndim = len(keys)
    if ndim == 0:
        return loglike(np.array([])), 0.0, None
    rng = np.random.default_rng(seed)
    sampler = dynesty.NestedSampler(loglike, prior_transform, ndim, nlive=nlive,
                                    sample="rslice" if ndim > 4 else "unif",
                                    rstate=rng)
    sampler.run_nested(dlogz=dlogz, print_progress=print_progress)
    results = sampler.results
    return results.logz[-1], results.logzerr[-1], results


def bayes_factor_analysis(data, calib, source_name="4U0142+61", user_f_FF=None,
                          nlive=500, e_res_lo=1.0, e_res_hi=15.0,
                          seed=42, dlogz=0.01,
                          f_ff_ref=None, f_ff_sigma=None,
                          print_progress=False):
    """Comparacao H1 (sem dip) vs H1.5 (Gaussiano) vs H2 (Voigt), por modo.

    f_ff_ref/f_ff_sigma: propagam a correcao de campo finito como nuisance
    parametro (prior Gaussiano). Se nao fornecidos, usa os valores do calib
    (chaves 'f_FF_ref'/'f_FF_sigma').
    """
    if f_ff_ref is None:
        f_ff_ref = calib.get("f_FF_ref")
    if f_ff_sigma is None:
        f_ff_sigma = calib.get("f_FF_sigma")
    f_ff_ref = f_ff_ref or 1.0

    out = {}
    for mode in ("independent", "calibrated"):
        priors = build_priors(mode, calib, with_f_sigma=True,
                              e_res_lo=e_res_lo, e_res_hi=e_res_hi,
                              f_ff_ref=f_ff_ref, f_ff_sigma=f_ff_sigma)
        logz1, logz1e, _ = run_nested_sampling(data, priors, with_resonance=False,
                                               nlive=nlive, dlogz=dlogz, seed=seed,
                                               f_ff_ref=f_ff_ref,
                                               print_progress=print_progress)
        logz15, logz15e, _ = run_nested_sampling(data, priors, with_resonance=True,
                                                 dip_type="gaussian", nlive=nlive,
                                                 dlogz=dlogz, seed=seed + 1,
                                                 f_ff_ref=f_ff_ref,
                                                 print_progress=print_progress)
        logz2, logz2e, _ = run_nested_sampling(data, priors, with_resonance=True,
                                               dip_type="voigt", nlive=nlive,
                                               dlogz=dlogz, seed=seed + 2,
                                               f_ff_ref=f_ff_ref,
                                               print_progress=print_progress)
        label21, log10_21 = interpret_bayes_factor(logz2 - logz1)
        label215, log10_215 = interpret_bayes_factor(logz2 - logz15)
        out[mode] = {
            "logZ_H1": float(logz1), "logZ_H1_err": float(logz1e),
            "logZ_H1.5": float(logz15), "logZ_H1.5_err": float(logz15e),
            "logZ_H2": float(logz2), "logZ_H2_err": float(logz2e),
            "ln_BF_H2_H1": float(logz2 - logz1),
            "ln_BF_H2_H1.5": float(logz2 - logz15),
            "log10_BF_H2_H1": float(log10_21),
            "log10_BF_H2_H1.5": float(log10_215),
            "interpretation_H2_H1": label21,
            "interpretation_H2_H1.5": label215,
            "f_FF": {"ref": f_ff_ref, "sigma": f_ff_sigma},
        }
    return out


# ---------------------------------------------------------------------------
# 8. Exemplo de uso
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    rng = np.random.default_rng(42)
    E_grid = np.linspace(1.0, 15.0, 60)
    phi_grid = rng.uniform(0, 2 * np.pi, size=E_grid.size)

    def pd_base_example(E):
        return 0.6 + 0.01 * E

    true_model = pd_model(
        E_grid, phi_grid,
        theta_params={"alpha": 1.0, "beta": 0.05, "phi0": 0.0},
        pd_base_fn=pd_base_example,
        eta=0.5, E_res=5.9, sigma=0.5, gamma=0.3,
        dPD_dip=0.15, with_resonance=True, phase_averaged=True, dip_type="voigt"
    )
    pd_obs = true_model + rng.normal(0, 0.02, size=E_grid.size)
    pd_err = np.full_like(E_grid, 0.02)
    data = Dataset(E=E_grid, phi=phi_grid, PD_obs=pd_obs, PD_err=pd_err,
                   pd_base_fn=pd_base_example, phase_averaged=True)

    calib = {"eta_mu": 0.5, "eta_sigma": 0.1, "alpha_mu": 1.0, "alpha_sigma": 0.2,
             "beta_mu": 0.05, "beta_sigma": 0.05}

    print("=" * 60)
    print("PIPELINE BAYESIANO M2+ v2.2 — TESTE DE VALIDACAO")
    print("=" * 60)

    print("\n--- Teste de f_FF (com incerteza) ---")
    for name, B in [("1E1547", 3.2e14), ("4U0142+61", 1.3e14), ("SGR1806", 2.0e15)]:
        f, s, status = get_f_FF_with_uncertainty(B, name if name in VERIFIED_FF_DATA else None)
        print(f"  {name}: B={B:.2e} G -> f_FF={f:.3f} +- {s:.2f} ({status})")

    print("\n--- Normalizacao de nomes ---")
    for n in ["1E1547", "1e1547.0-5408", "SGR 1806-20", "4U0142+61"]:
        print(f"  '{n}' -> {normalize_source_name(n)}")

    print("\n--- Testes de sanidade ---")
    E_test = np.linspace(-10, 10, 10000)
    v_test = voigt_profile(E_test, 0, 1.0, 0.5)
    print(f"Integral Voigt (deve ser ~1.0): {np.trapezoid(v_test, E_test):.4f}")

    label, log10 = interpret_bayes_factor(6.3)
    print(f"ln_BF=6.3 -> log10_BF={log10:.2f} -> {label}")

    E_few = np.array([3.0, 4.5, 5.9, 7.0])
    pd_test = pd_model(E_few, np.array([0.0]), theta_params={"alpha": 1.0, "beta": 0.05, "phi0": 0.0},
                       pd_base_fn=pd_base_example, eta=0.5, E_res=5.9, sigma=0.5, gamma=0.3,
                       dPD_dip=0.15, with_resonance=True, phase_averaged=True)
    print(f"PD modelado em E={E_few}: {pd_test}")
    print(f"  -> Minimo em E={E_few[np.argmin(pd_test)]:.1f} keV")

    print("\n--- Rodando inferencia Bayesiana (seed=42, f_FF propagado) ---")
    try:
        results = bayes_factor_analysis(data, calib, source_name="4U0142+61",
                                        user_f_FF=1.32, f_ff_ref=1.32, f_ff_sigma=0.05,
                                        nlive=200, dlogz=0.5, seed=42)
        for mode, r in results.items():
            print(f"\n[{mode}]")
            print(f"  ln BF(H2/H1) = {r['ln_BF_H2_H1']:.2f} -> "
                  f"{r['interpretation_H2_H1']} (log10={r['log10_BF_H2_H1']:.2f})")
            print(f"  f_FF ref={r['f_FF']['ref']} +- {r['f_FF']['sigma']}")
    except ImportError:
        print("dynesty nao instalado — pip install dynesty")
    except Exception as e:
        print(f"Erro: {e}")
