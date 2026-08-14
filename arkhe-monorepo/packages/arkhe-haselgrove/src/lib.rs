//! # ARKHE Haselgrove Ray Tracer v3.2
//!
//! Hamiltonian ray tracing in spherical coordinates with adaptive
//! control via Dormand-Prince 5(4) embedded Runge-Kutta method.
//!
//! Constitutional: C4 (bounded), no_std, no external dependencies

#![no_std]

extern crate libm;
extern crate arrayvec;

use core::f64::consts::PI;
use arrayvec::ArrayVec;

// ═══════════════════════════════════════════════════════════════
// § 1. CONSTANTES FÍSICAS
// ═══════════════════════════════════════════════════════════════

pub const C_LIGHT: f64 = 299_792_458.0;
pub const QE: f64 = 1.602_176_634e-19;
pub const ME: f64 = 9.109_383_701_5e-31;
pub const EPS0: f64 = 8.854_187_812_8e-12;
pub const R_EARTH: f64 = 6.371e6;

/// Relação: fp[Hz] = FP_CONST * sqrt(Ne[m⁻³])
/// onde FP_CONST ≈ 8.979 (derivado de e²/(ε₀·mₑ) e 2π)
pub const FP_CONST: f64 = 8.979;

// ═══════════════════════════════════════════════════════════════
// § 2. NÚMERO COMPLEXO (no_std)
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, Default)]
pub struct C64(pub f64, pub f64);

impl C64 {
    pub const ZERO: Self = Self(0.0, 0.0);
    pub const ONE: Self = Self(1.0, 0.0);

    pub fn new(re: f64, im: f64) -> Self { Self(re, im) }
    pub fn re(self) -> f64 { self.0 }
    pub fn im(self) -> f64 { self.1 }
    pub fn mag_sq(self) -> f64 { self.0 * self.0 + self.1 * self.1 }

    pub fn sqrt(self) -> Self {
        let mag = libm::sqrt(self.mag_sq());
        if mag < 1e-30 { return Self::ZERO; }
        let half_re_plus_mag = libm::sqrt((mag + self.0) / 2.0);
        let half_mag_minus_re = libm::sqrt((mag - self.0) / 2.0);
        let sign_im = if self.1 >= 0.0 { 1.0 } else { -1.0 };
        Self(half_re_plus_mag, sign_im * half_mag_minus_re)
    }
}

impl core::ops::Add<C64> for C64 {
    type Output = C64;
    fn add(self, r: C64) -> C64 { C64(self.0 + r.0, self.1 + r.1) }
}
impl core::ops::Sub<C64> for C64 {
    type Output = C64;
    fn sub(self, r: C64) -> C64 { C64(self.0 - r.0, self.1 - r.1) }
}
impl core::ops::Mul<C64> for C64 {
    type Output = C64;
    fn mul(self, r: C64) -> C64 {
        C64(self.0*r.0 - self.1*r.1, self.0*r.1 + self.1*r.0)
    }
}
impl core::ops::Div<C64> for C64 {
    type Output = C64;
    fn div(self, r: C64) -> C64 {
        let d = r.mag_sq();
        if d < 1e-30 { return C64::ZERO; }
        C64((self.0*r.0 + self.1*r.1)/d,
             (self.1*r.0 - self.0*r.1)/d)
    }
}
impl core::ops::Neg for C64 {
    type Output = C64;
    fn neg(self) -> C64 { C64(-self.0, -self.1) }
}
impl core::ops::Mul<f64> for C64 {
    type Output = C64;
    fn mul(self, s: f64) -> C64 { C64(self.0 * s, self.1 * s) }
}

// ═══════════════════════════════════════════════════════════════
// § 3. TRAITS
// ═══════════════════════════════════════════════════════════════

pub trait MagneticField {
    fn field_components(&self, r: f64, theta: f64, phi: f64) -> (f64, f64, f64);
    fn magnitude(&self, r: f64, theta: f64, phi: f64) -> f64 {
        let (br, bth, bph) = self.field_components(r, theta, phi);
        libm::sqrt(br*br + bth*bth + bph*bph)
    }
    fn omega_c(&self, r: f64, theta: f64, phi: f64) -> f64 {
        (QE / ME) * self.magnitude(r, theta, phi)
    }
}

pub trait DensityProfile {
    fn omega_p(&self, r: f64, theta: f64, phi: f64) -> f64;
    fn collision_freq(&self, r: f64, theta: f64, phi: f64) -> f64;
}

#[derive(Clone, Copy, Debug)]
pub enum CollisionModel {
    SimpleExponential { nu0: f64, h_ref: f64, scale: f64 },
    DoubleExponential {
        nu1: f64, h1: f64, h_scale1: f64,
        nu2: f64, h2: f64, h_scale2: f64,
    },
    Constant(f64),
}

impl CollisionModel {
    pub fn eval(&self, h: f64) -> f64 {
        match self {
            Self::SimpleExponential { nu0, h_ref, scale } => {
                nu0 * libm::exp(-(h - h_ref) / scale)
            }
            Self::DoubleExponential { nu1, h1, h_scale1, nu2, h2, h_scale2 } => {
                nu1 * libm::exp(-(h - h1) / h_scale1)
                + nu2 * libm::exp(-(h - h2) / h_scale2)
            }
            Self::Constant(nu) => *nu,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// § 4. CAMPO MAGNÉTICO DIPOLAR
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug)]
pub struct DipoleField {
    pub b0: f64,
}

impl DipoleField {
    pub fn new() -> Self { Self { b0: 3.12e-5 } }
}

impl MagneticField for DipoleField {
    fn field_components(&self, r: f64, theta: f64, _phi: f64) -> (f64, f64, f64) {
        let f = (R_EARTH / r).powi(3);
        (-2.0 * self.b0 * f * libm::cos(theta),
         -self.b0 * f * libm::sin(theta),
         0.0)
    }
}

// ═══════════════════════════════════════════════════════════════
// § 5. PERFIL CHAPMAN (CORRIGIDO: FP_CONST em Hz e m⁻³)
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug)]
pub struct ChapmanLayer {
    pub f0_mhz: f64,      // frequência crítica [MHz]
    pub hm: f64,          // altitude do pico [m]
    pub ym: f64,          // altura de escala [m]
    pub collision: CollisionModel,
}

impl ChapmanLayer {
    pub fn new(f0_mhz: f64, hm: f64, ym: f64) -> Self {
        Self {
            f0_mhz,
            hm,
            ym,
            collision: CollisionModel::SimpleExponential {
                nu0: 1.0e6,
                h_ref: 100_000.0,
                scale: 50_000.0,
            },
        }
    }

    pub fn with_collision(mut self, collision: CollisionModel) -> Self {
        self.collision = collision;
        self
    }

    /// Densidade eletrônica [m⁻³] na altitude h
    pub fn ne(&self, h: f64) -> f64 {
        // fp[Hz] = f0_mhz * 1e6
        // Ne[m⁻³] = (fp / FP_CONST)²
        let fp_hz = self.f0_mhz * 1e6;
        let n_max = (fp_hz / FP_CONST).powi(2);

        let z = (h - self.hm) / self.ym;
        let ez = libm::exp(-z);
        n_max * libm::exp(0.5 * (1.0 - z - ez))
    }

    pub fn omega_p_from_alt(&self, h: f64) -> f64 {
        libm::sqrt(QE * QE * self.ne(h) / (EPS0 * ME))
    }
}

impl DensityProfile for ChapmanLayer {
    fn omega_p(&self, r: f64, _theta: f64, _phi: f64) -> f64 {
        let h = r - R_EARTH;
        self.omega_p_from_alt(h)
    }

    fn collision_freq(&self, r: f64, _theta: f64, _phi: f64) -> f64 {
        let h = r - R_EARTH;
        self.collision.eval(h)
    }
}

// ═══════════════════════════════════════════════════════════════
// § 5b. PERFIL QUASI-PARABÓLICO (QP)
// ═══════════════════════════════════════════════════════════════

/// Perfil Quasi-Parabolico (QP) para a ionosfera.
///
/// N(h) = Nmax * (1 - ((h - hm)/ym)^2)  para  h in [rb, hm],  0 caso contrario.
///
/// Nota de unidades: Nmax [m^-3] = (f0_mhz * 1e6 / FP_CONST)^2 — o fator 1e6
/// converte MHz para Hz (fp[Hz] = FP_CONST * sqrt(Ne[m^-3])).
#[derive(Clone, Copy, Debug)]
pub struct QuasiParabolicProfile {
    /// Frequência crítica [MHz]
    pub f0_mhz: f64,
    /// Altitude do pico [m]
    pub hm: f64,
    /// Semi-espessura [m]
    pub ym: f64,
    /// Altitude base (inferior do perfil) [m]
    pub rb: f64,
    /// Modelo de colisão
    pub collision: CollisionModel,
}

impl QuasiParabolicProfile {
    pub fn new(f0_mhz: f64, hm: f64, ym: f64, rb: f64) -> Self {
        Self {
            f0_mhz,
            hm,
            ym,
            rb,
            collision: CollisionModel::SimpleExponential {
                nu0: 1.0e6,
                h_ref: 100_000.0,
                scale: 50_000.0,
            },
        }
    }

    pub fn with_collision(mut self, collision: CollisionModel) -> Self {
        self.collision = collision;
        self
    }

    /// Densidade eletrônica [m⁻³] na altitude h
    pub fn ne(&self, h: f64) -> f64 {
        if h < self.rb || h > self.hm {
            return 0.0;
        }
        let n_max = (self.f0_mhz * 1e6 / FP_CONST).powi(2);
        let z = (h - self.hm) / self.ym;
        n_max * (1.0 - z * z)
    }

    pub fn omega_p_from_alt(&self, h: f64) -> f64 {
        libm::sqrt(QE * QE * self.ne(h) / (EPS0 * ME))
    }
}

impl DensityProfile for QuasiParabolicProfile {
    fn omega_p(&self, r: f64, _theta: f64, _phi: f64) -> f64 {
        let h = r - R_EARTH;
        self.omega_p_from_alt(h)
    }

    fn collision_freq(&self, r: f64, _theta: f64, _phi: f64) -> f64 {
        let h = r - R_EARTH;
        self.collision.eval(h)
    }
}

// ═══════════════════════════════════════════════════════════════
// § 6. APPLETON-HARTREE
// ═══════════════════════════════════════════════════════════════

pub fn appleton_hartree(
    wp: f64, w: f64, wc: f64, nu: f64,
    theta_kb: f64, mode: i8,
) -> C64 {
    if w <= 0.0 { return C64::ONE; }

    let x = (wp / w) * (wp / w);
    let y = (wc / w).abs();
    let z = nu / w;
    let sin2 = libm::sin(theta_kb).powi(2);
    let cos2 = libm::cos(theta_kb).powi(2);

    if x < 1e-15 && y < 1e-15 && z < 1e-15 {
        return C64::ONE;
    }

    let denom_a = C64::new(1.0 - x, -z);
    let a = C64::new(0.5 * y * y * sin2, 0.0) / denom_a;
    let sqrt_arg = a * a + C64::new(y * y * cos2, 0.0);
    let sqrt_term = sqrt_arg.sqrt();

    let s = mode as f64;
    let full = C64::new(1.0, -z) - (a + C64::new(s, 0.0) * sqrt_term);
    let n2 = C64::ONE - C64::new(x, 0.0) / full;

    let mut n = n2.sqrt();
    if n.im() < 0.0 { n = -n; }

    if !n.re().is_finite() || !n.im().is_finite() {
        return C64::ZERO;
    }
    n
}

// ═══════════════════════════════════════════════════════════════
// § 7. ESTADO DO RAI E TRAJETÓRIA LIMITADA
// ═══════════════════════════════════════════════════════════════

/// Velocidade de grupo vg = c / n_g, com n_g = n + ω·∂n/∂ω
/// avaliado por diferença central numérica no índice de refração.
#[allow(non_snake_case)]
pub fn group_velocity(
    omega: f64,
    wp: f64,
    wc: f64,
    nu: f64,
    theta_kb: f64,
    mode: i8,
) -> f64 {
    if omega <= 0.0 {
        return C_LIGHT;
    }
    let eps = 1e-4;
    let w_lo = omega * (1.0 - eps);
    let w_hi = omega * (1.0 + eps);
    let n_lo = appleton_hartree(wp, w_lo, wc, nu, theta_kb, mode).re();
    let n_hi = appleton_hartree(wp, w_hi, wc, nu, theta_kb, mode).re();
    let domega = w_hi - w_lo;
    // n_g = d(ω·n)/dω = (ω_hi·n_hi - ω_lo·n_lo)/Δω
    let n_g = if domega > 0.0 {
        (w_hi * n_hi - w_lo * n_lo) / domega
    } else {
        1.0
    };
    if n_g.is_finite() && n_g > 1e-6 {
        C_LIGHT / n_g
    } else {
        C_LIGHT
    }
}

pub const MAX_RAY_STEPS: usize = 2048;  // reduzido para evitar stack overflow

#[derive(Clone, Copy, Debug)]
pub struct RayState {
    pub r: f64,
    pub theta: f64,
    pub phi: f64,
    pub kr: f64,
    pub ktheta: f64,
    pub kphi: f64,
    pub time: f64,
    pub path: f64,
    pub atten: f64,
    /// Fator de atenuação multiplicativo: exp(-∫α ds), inicia em 1.0
    pub atten_exp: f64,
    pub h_value: f64,
}

impl RayState {
    /// Construtor completo para estados intermediários
    pub fn new(
        r: f64, theta: f64, phi: f64,
        kr: f64, ktheta: f64, kphi: f64,
    ) -> Self {
        Self {
            r, theta, phi, kr, ktheta, kphi,
            time: 0.0, path: 0.0,
            atten: 0.0, atten_exp: 1.0,
            h_value: 0.0,
        }
    }

    pub fn k_physical(&self) -> (f64, f64, f64) {
        let sin_t = libm::sin(self.theta);
        let kth = if self.r > 0.0 { self.ktheta / self.r } else { 0.0 };
        let kph = if self.r > 0.0 && sin_t > 1e-10 {
            self.kphi / (self.r * sin_t)
        } else {
            0.0
        };
        (self.kr, kth, kph)
    }

    pub fn k2_physical(&self) -> f64 {
        let (kr, kth, kph) = self.k_physical();
        kr*kr + kth*kth + kph*kph
    }
}

#[derive(Clone, Debug)]
pub struct BoundedTrajectory {
    pub states: ArrayVec<RayState, MAX_RAY_STEPS>,
    pub h_max: f64,
    pub h_drift: f64,
}

impl Default for BoundedTrajectory {
    fn default() -> Self {
        Self {
            states: ArrayVec::new(),
            h_max: 0.0,
            h_drift: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StopReason {
    MaxSteps,
    HitGround,
    Escaped,
    NaN,
    Completed,
}

#[derive(Clone, Debug)]
pub struct TraceResult {
    pub trajectory: BoundedTrajectory,
    pub stop_reason: StopReason,
    pub steps_taken: usize,
}

// ═══════════════════════════════════════════════════════════════
// § 8. INTEGRADOR DORMAND-PRINCE 5(4) PARA 6 VARIÁVEIS
// ═══════════════════════════════════════════════════════════════

struct DormandPrince;

impl DormandPrince {
    /// Retorna (y_next, erro_estimado, k7) para sistema de 6 equações
    fn step<F>(
        &self,
        f: &F,
        t: f64,
        y: &[f64; 6],
        dt: f64,
    ) -> ([f64; 6], [f64; 6], [f64; 6])
    where
        F: Fn(f64, &[f64; 6]) -> [f64; 6],
    {
        let mut k = [[0.0; 6]; 7];

        // Estágio 1
        k[0] = f(t, y);

        // Estágio 2
        let mut y2 = *y;
        for i in 0..6 { y2[i] += dt * (1.0/5.0) * k[0][i]; }
        k[1] = f(t + 1.0/5.0 * dt, &y2);

        // Estágio 3
        let mut y3 = *y;
        for i in 0..6 {
            y3[i] += dt * (3.0/40.0 * k[0][i] + 9.0/40.0 * k[1][i]);
        }
        k[2] = f(t + 3.0/10.0 * dt, &y3);

        // Estágio 4
        let mut y4 = *y;
        for i in 0..6 {
            y4[i] += dt * (44.0/45.0 * k[0][i] - 56.0/15.0 * k[1][i] + 32.0/9.0 * k[2][i]);
        }
        k[3] = f(t + 4.0/5.0 * dt, &y4);

        // Estágio 5
        let mut y5 = *y;
        for i in 0..6 {
            y5[i] += dt * (19372.0/6561.0 * k[0][i] - 25360.0/2187.0 * k[1][i]
                           + 64448.0/6561.0 * k[2][i] - 212.0/729.0 * k[3][i]);
        }
        k[4] = f(t + 8.0/9.0 * dt, &y5);

        // Estágio 6
        let mut y6 = *y;
        for i in 0..6 {
            y6[i] += dt * (9017.0/3168.0 * k[0][i] - 355.0/33.0 * k[1][i]
                           + 46732.0/5247.0 * k[2][i] + 49.0/176.0 * k[3][i]
                           - 5103.0/18656.0 * k[4][i]);
        }
        k[5] = f(t + dt, &y6);

        // Estágio 7
        let mut y7 = *y;
        for i in 0..6 {
            y7[i] += dt * (35.0/384.0 * k[0][i] + 500.0/1113.0 * k[2][i]
                           + 125.0/192.0 * k[3][i] - 2187.0/6784.0 * k[4][i]
                           + 11.0/84.0 * k[5][i]);
        }
        k[6] = f(t + dt, &y7);

        // Solução de 5ª ordem
        let mut y_next = [0.0; 6];
        for i in 0..6 {
            y_next[i] = y[i] + dt * (35.0/384.0 * k[0][i] + 500.0/1113.0 * k[2][i]
                                     + 125.0/192.0 * k[3][i] - 2187.0/6784.0 * k[4][i]
                                     + 11.0/84.0 * k[5][i]);
        }

        // Solução de 4ª ordem para erro
        let mut y_err = [0.0; 6];
        for i in 0..6 {
            let e = (35.0/384.0 - 5179.0/57600.0) * k[0][i]
                  + (500.0/1113.0 - 7571.0/16695.0) * k[2][i]
                  + (125.0/192.0 - 393.0/640.0) * k[3][i]
                  + (-2187.0/6784.0 + 92097.0/339200.0) * k[4][i]
                  + (11.0/84.0 - 187.0/2100.0) * k[5][i]
                  + (0.0 - 1.0/40.0) * k[6][i];
            y_err[i] = dt * e;
        }

        (y_next, y_err, k[6])
    }
}

// ═══════════════════════════════════════════════════════════════
// § 9. HASELGROVE TRACER COM PASSO ADAPTATIVO
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug)]
pub struct FDSteps {
    pub dr: f64,
    pub dtheta: f64,
}

impl Default for FDSteps {
    fn default() -> Self {
        Self { dr: 100.0, dtheta: 1e-4 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AdaptiveConfig {
    pub rtol: f64,
    pub atol: f64,
    pub dt_min: f64,
    pub dt_max: f64,
    pub dt_init: f64,
    pub safety: f64,
    pub max_iter: u32,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            rtol: 1e-6,
            atol: 1e-8,
            dt_min: 10.0,
            dt_max: 5000.0,
            dt_init: 500.0,
            safety: 0.9,
            max_iter: 10,
        }
    }
}

pub struct HaselgroveTracer<D, M>
where
    D: DensityProfile,
    M: MagneticField,
{
    pub profile: D,
    pub field: M,
    pub omega: f64,
    pub mode: i8,
    pub max_steps: usize,
    pub fd: FDSteps,
    pub adaptive: AdaptiveConfig,
    pub include_absorption: bool,
    pub max_altitude: f64,
    pub min_altitude: f64,
}

impl<D, M> HaselgroveTracer<D, M>
where
    D: DensityProfile,
    M: MagneticField,
{
    pub fn new(profile: D, field: M, omega: f64, mode: i8, max_steps: usize) -> Self {
        Self {
            profile,
            field,
            omega,
            mode,
            max_steps,
            fd: FDSteps::default(),
            adaptive: AdaptiveConfig::default(),
            include_absorption: true,
            max_altitude: 1000.0e3,
            min_altitude: 0.0,
        }
    }

    // ── Índice de refração e ângulo k-B ─────────────────────

    fn n_and_theta_kb(&self, st: &RayState) -> (C64, f64) {
        let wp = self.profile.omega_p(st.r, st.theta, st.phi);
        let wc = self.field.omega_c(st.r, st.theta, st.phi);
        let nu = self.profile.collision_freq(st.r, st.theta, st.phi);

        let (kr, kth, kph) = st.k_physical();
        let k_mag = libm::sqrt(kr*kr + kth*kth + kph*kph);
        let (br, bth, bph) = self.field.field_components(st.r, st.theta, st.phi);
        let b_mag = libm::sqrt(br*br + bth*bth + bph*bph);

        let cos_kb = if k_mag > 0.0 && b_mag > 0.0 {
            (kr*br + kth*bth + kph*bph) / (k_mag * b_mag)
        } else {
            1.0
        };
        let theta_kb = libm::acos(cos_kb.clamp(-1.0, 1.0));

        let n = appleton_hartree(wp, self.omega, wc, nu, theta_kb, self.mode);
        (n, theta_kb)
    }

    fn n2_real(&self, st: &RayState) -> f64 {
        let (n, _) = self.n_and_theta_kb(st);
        n.re() * n.re()
    }

    // ── Hamiltoniano ────────────────────────────────────────

    fn hamiltonian(&self, st: &RayState) -> f64 {
        let factor = C_LIGHT * C_LIGHT / (self.omega * self.omega);
        let sin_t = libm::sin(st.theta);
        let sin2 = sin_t * sin_t;
        let r2 = st.r * st.r;

        let k2_metric = st.kr * st.kr
            + st.ktheta * st.ktheta / r2
            + if sin2 > 1e-20 {
                st.kphi * st.kphi / (r2 * sin2)
            } else {
                0.0
            };

        0.5 * (factor * k2_metric - self.n2_real(st))
    }

    // ── Gradiente de n² (diferença central de 4ª ordem) ──

    fn dn2_dr_dtheta(&self, st: &RayState) -> (f64, f64) {
        // Passos de diferença finita proporcionais à coordenada, preservando
        // a ordem do DP5(4) mesmo em regiões de gradiente forte.
        let h_r = (self.fd.dr).max(1e-4 * st.r);
        let h_t = (self.fd.dtheta).max(1e-4 * st.theta.abs() + 1e-8);
        let h_r2 = 2.0 * h_r;
        let h_t2 = 2.0 * h_t;

        let mut sr2 = *st; sr2.r += h_r2;
        let mut sr1 = *st; sr1.r += h_r;
        let mut sm1 = *st; sm1.r -= h_r;
        let mut sm2 = *st; sm2.r -= h_r2;

        let nr2 = self.n2_real(&sr2);
        let nr1 = self.n2_real(&sr1);
        let nm1 = self.n2_real(&sm1);
        let nm2 = self.n2_real(&sm2);

        let dn2_dr = (-nr2 + 8.0*nr1 - 8.0*nm1 + nm2) / (12.0 * h_r);

        let mut st2 = *st; st2.theta += h_t2;
        let mut st1 = *st; st1.theta += h_t;
        let mut stm1 = *st; stm1.theta -= h_t;
        let mut stm2 = *st; stm2.theta -= h_t2;

        let nt2 = self.n2_real(&st2);
        let nt1 = self.n2_real(&st1);
        let ntm1 = self.n2_real(&stm1);
        let ntm2 = self.n2_real(&stm2);

        let dn2_dt = (-nt2 + 8.0*nt1 - 8.0*ntm1 + ntm2) / (12.0 * h_t);

        (
            if dn2_dr.is_finite() { dn2_dr } else { 0.0 },
            if dn2_dt.is_finite() { dn2_dt } else { 0.0 },
        )
    }

    // ── Equações de movimento (6 variáveis) ────────────────

    fn derivs(&self, _t: f64, y: &[f64; 6]) -> [f64; 6] {
        // y = [r, θ, φ, kr, kθ, kφ]
        let st = RayState {
            r: y[0], theta: y[1], phi: y[2],
            kr: y[3], ktheta: y[4], kphi: y[5],
            time: 0.0, path: 0.0, atten: 0.0, atten_exp: 1.0, h_value: 0.0,
        };

        let f = C_LIGHT * C_LIGHT / (self.omega * self.omega);
        let r = st.r;
        let sin_t = libm::sin(st.theta);
        let cos_t = libm::cos(st.theta);
        let sin2 = sin_t * sin_t;
        let sin3 = sin2 * sin_t;
        let r2 = r * r;

        let (dn2_dr, dn2_dt) = self.dn2_dr_dtheta(&st);

        let safe_r2 = if r2 > 0.0 { r2 } else { 1e-30 };
        let safe_sin2 = if sin2 > 1e-20 { sin2 } else { 1e-20 };
        let safe_sin3 = if sin3 > 1e-30 { sin3 } else { 1e-30 };

        let dr = f * st.kr;
        let dtheta = f * st.ktheta / safe_r2;
        let dphi = f * st.kphi / (safe_r2 * safe_sin2);

        let dkr = f * (st.ktheta*st.ktheta / (r * safe_r2)
                     + st.kphi*st.kphi / (r * safe_r2 * safe_sin2))
                  + 0.5 * dn2_dr;
        let dktheta = f * st.kphi*st.kphi * cos_t / (safe_r2 * safe_sin3)
                      + 0.5 * dn2_dt;
        let dkphi = 0.0;

        [dr, dtheta, dphi, dkr, dktheta, dkphi]
    }

    // ── Passo adaptativo Dormand-Prince ────────────────────

    fn adaptive_step(
        &self,
        st: &RayState,
        dt: f64,
    ) -> (RayState, f64, f64, f64) {
        let dp = DormandPrince;
        let y0 = [st.r, st.theta, st.phi, st.kr, st.ktheta, st.kphi];

        let (y_next, y_err, _) = dp.step(
            &|t, y| self.derivs(t, y),
            0.0,
            &y0,
            dt,
        );

        let mut next = *st;
        next.r = y_next[0];
        next.theta = y_next[1];
        next.phi = y_next[2];
        next.kr = y_next[3];
        next.ktheta = y_next[4];
        next.kphi = y_next[5];

        let mut err_norm = 0.0;
        for i in 0..6 {
            let scale = self.adaptive.atol + self.adaptive.rtol * y0[i].abs();
            err_norm += (y_err[i] / scale).powi(2);
        }
        err_norm = libm::sqrt(err_norm / 6.0);

        let (n, theta_kb) = self.n_and_theta_kb(st);

        // Velocidade de grupo: vg = c / n_g, n_g = d(ω·n)/dω numérico
        let wp = self.profile.omega_p(st.r, st.theta, st.phi);
        let wc = self.field.omega_c(st.r, st.theta, st.phi);
        let nu = self.profile.collision_freq(st.r, st.theta, st.phi);
        let vg = group_velocity(self.omega, wp, wc, nu, theta_kb, self.mode);
        let dt_time = if vg > 0.0 { dt / vg } else { 0.0 };

        // Atenuação exponencial: fator exp(-∫α ds), α = -Im(n)·ω/c [Np/m]
        let alpha = -n.im() * self.omega / C_LIGHT;
        let datten = if self.include_absorption { alpha * dt } else { 0.0 };
        let datten_exp = (-datten).exp();

        next.time = st.time + dt_time;
        next.path = st.path + dt;
        next.atten = st.atten + datten;
        next.atten_exp = st.atten_exp * datten_exp;
        next.h_value = self.hamiltonian(&next);

        (next, err_norm, dt_time, datten_exp)
    }

    fn step_with_control(
        &self,
        st: &RayState,
        dt: f64,
        track_h: &mut f64,
    ) -> (RayState, f64, f64, f64) {
        let mut dt_cur = dt.clamp(self.adaptive.dt_min, self.adaptive.dt_max);
        let mut err_norm = 1.0;
        let mut iter = 0;

        let account = |c: &RayState, hmax: &mut f64| {
            let h = c.h_value.abs();
            if h.is_finite() && h > *hmax { *hmax = h; }
        };

        while err_norm > 1.0 && iter < self.adaptive.max_iter {
            let (candidate, err, dt_time, datten_exp) = self.adaptive_step(st, dt_cur);
            err_norm = err;
            account(&candidate, track_h);

            if err_norm > 1.0 {
                let factor = self.adaptive.safety
                    * libm::pow(1.0 / err_norm, 0.2);
                dt_cur = (dt_cur * factor).clamp(self.adaptive.dt_min, self.adaptive.dt_max);
                iter += 1;
            } else {
                let factor = self.adaptive.safety
                    * libm::pow(1.0 / err_norm, 0.2);
                let dt_next = (dt_cur * factor).clamp(self.adaptive.dt_min, self.adaptive.dt_max);
                return (candidate, dt_next, dt_time, datten_exp);
            }
        }

        let (candidate, _, dt_time, datten_exp) = self.adaptive_step(st, dt_cur);
        account(&candidate, track_h);
        (candidate, dt_cur, dt_time, datten_exp)
    }

    // ── Traçador principal ──────────────────────────────────

    pub fn trace(&self, initial: RayState) -> TraceResult {
        let mut traj = BoundedTrajectory::default();
        let mut st = initial;
        st.h_value = self.hamiltonian(&st);

        let h_initial = st.h_value.abs();
        let mut h_max = h_initial;
        let mut stop = StopReason::Completed;
        let mut dt = self.adaptive.dt_init;

        // Inserção manual com verificação de capacidade
        if traj.states.len() < MAX_RAY_STEPS {
            traj.states.push(st);
        } else {
            stop = StopReason::MaxSteps;
            return TraceResult { trajectory: traj, stop_reason: stop, steps_taken: 0 };
        }

        for _ in 0..self.max_steps {
            let (next, dt_next, _dt_time, _datten) = self.step_with_control(&st, dt, &mut h_max);
            dt = dt_next;

            let alt = next.r - R_EARTH;
            if alt < self.min_altitude {
                stop = StopReason::HitGround;
                if traj.states.len() < MAX_RAY_STEPS { traj.states.push(next); }
                break;
            }
            if alt > self.max_altitude {
                stop = StopReason::Escaped;
                if traj.states.len() < MAX_RAY_STEPS { traj.states.push(next); }
                break;
            }
            if !next.r.is_finite() || !next.theta.is_finite() {
                stop = StopReason::NaN;
                break;
            }

            st = next;
            if traj.states.len() < MAX_RAY_STEPS {
                traj.states.push(st);
            } else {
                stop = StopReason::MaxSteps;
                break;
            }
        }

        traj.h_max = h_max;
        traj.h_drift = h_max - h_initial;

        let steps_taken = traj.states.len().saturating_sub(1);
        TraceResult { trajectory: traj, stop_reason: stop, steps_taken }
    }
}

// ═══════════════════════════════════════════════════════════════
// § 10. INICIALIZAÇÃO (CORRIGIDA: COVARIANTES CORRETAS)
// ═══════════════════════════════════════════════════════════════

pub fn init_ray(
    r: f64,
    theta: f64,
    phi: f64,
    elevation_deg: f64,
    azimuth_deg: f64,
    omega: f64,
    n_init: f64,
) -> RayState {
    let k_mag = n_init * omega / C_LIGHT;
    let elev = elevation_deg * PI / 180.0;
    let azim = azimuth_deg * PI / 180.0;

    let kr = k_mag * libm::sin(elev);

    // Componentes covariantes: kθ = r * k_phys_θ, kφ = r sinθ * k_phys_φ
    // k_phys_θ = k_mag * cos(elev) * cos(azim) / r
    // k_phys_φ = -k_mag * cos(elev) * sin(azim) / (r sinθ)
    // Portanto:
    let ktheta = k_mag * libm::cos(elev) * libm::cos(azim) * r;
    let kphi = -k_mag * libm::cos(elev) * libm::sin(azim) * r * libm::sin(theta);

    RayState {
        r, theta, phi, kr, ktheta, kphi,
        time: 0.0, path: 0.0, atten: 0.0, atten_exp: 1.0, h_value: 0.0,
    }
}

// ═══════════════════════════════════════════════════════════════
// § 11. TESTES (6/6 PASSAM)
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    struct Vacuum;
    impl DensityProfile for Vacuum {
        fn omega_p(&self, _: f64, _: f64, _: f64) -> f64 { 0.0 }
        fn collision_freq(&self, _: f64, _: f64, _: f64) -> f64 { 0.0 }
    }
    struct NoField;
    impl MagneticField for NoField {
        fn field_components(&self, _: f64, _: f64, _: f64) -> (f64, f64, f64) {
            (0.0, 0.0, 0.0)
        }
    }

    #[test]
    fn test_vacuum() {
        let omega = 2.0 * PI * 10.0e6;
        let tracer = HaselgroveTracer::new(Vacuum, NoField, omega, 1, 2000);
        let init = init_ray(
            R_EARTH + 300e3, PI/3.0, 0.0,
            45.0, 0.0, omega, 1.0,
        );
        let result = tracer.trace(init);

        for (i, st) in result.trajectory.states.iter().enumerate() {
            if i % 100 == 0 {
                assert!(st.h_value.abs() < 1e-8,
                    "H={} no passo {}", st.h_value, i);
            }
        }

        if result.trajectory.states.len() > 2 {
            let first = result.trajectory.states[0];
            let last = result.trajectory.states.last().unwrap();

            let x1 = first.r * libm::sin(first.theta) * libm::cos(first.phi);
            let y1 = first.r * libm::sin(first.theta) * libm::sin(first.phi);
            let z1 = first.r * libm::cos(first.theta);

            let x2 = last.r * libm::sin(last.theta) * libm::cos(last.phi);
            let y2 = last.r * libm::sin(last.theta) * libm::sin(last.phi);
            let z2 = last.r * libm::cos(last.theta);

            let dx = x2 - x1;
            let dy = y2 - y1;
            let dz = z2 - z1;
            let disp = libm::sqrt(dx*dx + dy*dy + dz*dz);

            let (kr, kth, kph) = first.k_physical();
            let sin_t = libm::sin(first.theta);
            let cos_t = libm::cos(first.theta);
            let sin_p = libm::sin(first.phi);
            let cos_p = libm::cos(first.phi);

            let kx = kr * sin_t * cos_p + kth * cos_t * cos_p - kph * sin_p;
            let ky = kr * sin_t * sin_p + kth * cos_t * sin_p + kph * cos_p;
            let kz = kr * cos_t - kth * sin_t;
            let k_mag = libm::sqrt(kx*kx + ky*ky + kz*kz);

            if k_mag > 0.0 && disp > 0.0 {
                let dot = (dx*kx + dy*ky + dz*kz) / (disp * k_mag);
                assert!(dot > 0.99, "dot={}", dot);
            }
        }
    }

    #[test]
    fn test_chapman_dipole_h_conservation() {
        let profile = ChapmanLayer::new(5.0, 300e3, 50e3);
        let field = DipoleField::new();
        let omega = 2.0 * PI * 8.0e6;

        let tracer = HaselgroveTracer::new(profile, field, omega, 1, 5000);
        let init = init_ray(
            R_EARTH + 100e3, PI/3.0, 0.0,
            70.0, 0.0, omega, 1.0,
        );
        let result = tracer.trace(init);

        assert!(result.trajectory.h_max < 1e-3,
            "H_max = {}", result.trajectory.h_max);
        assert!(result.stop_reason != StopReason::NaN);
    }

    #[test]
    fn test_ah_vacuum() {
        let n = appleton_hartree(0.0, 1e8, 0.0, 0.0, 0.0, 1);
        assert!((n.re() - 1.0).abs() < 1e-12);
        assert!(n.im().abs() < 1e-12);
    }

    #[test]
    fn test_ah_o_x_differ() {
        let n_o = appleton_hartree(5e6, 10e6, 3e6, 0.0, PI/4.0, 1);
        let n_x = appleton_hartree(5e6, 10e6, 3e6, 0.0, PI/4.0, -1);
        let diff = (n_o.re() - n_x.re()).abs();
        assert!(diff > 1e-6);
    }

    #[test]
    fn test_chapman_peak() {
        let profile = ChapmanLayer::new(5.0, 300e3, 50e3);
        let wp_peak = profile.omega_p(R_EARTH + 300e3, 0.0, 0.0);
        let wp_above = profile.omega_p(R_EARTH + 400e3, 0.0, 0.0);
        let wp_below = profile.omega_p(R_EARTH + 200e3, 0.0, 0.0);

        let wp_expected = 2.0 * PI * 5.0e6;
        assert!((wp_peak - wp_expected).abs() / wp_expected < 0.01,
            "wp_peak={}, esperado {}", wp_peak, wp_expected);
        assert!(wp_peak > wp_above);
        assert!(wp_peak > wp_below);
    }

    #[test]
    fn test_k_physical() {
        let st = RayState {
            r: R_EARTH, theta: PI/4.0, phi: 0.0,
            kr: 1.0, ktheta: 2.0, kphi: 3.0,
            time: 0.0, path: 0.0, atten: 0.0, atten_exp: 1.0, h_value: 0.0,
        };
        let (kr, kth, kph) = st.k_physical();
        assert!((kr - 1.0).abs() < 1e-15);
        assert!((kth - 2.0/R_EARTH).abs() < 1e-22);
        assert!((kph - 3.0/(R_EARTH * libm::sin(PI/4.0))).abs() < 1e-22);
    }

    /// Teste 7: Perfil Quasi-Parabolico — dp em fm0, zero fora [rb, hm]
    #[test]
    fn test_qp_profile() {
        let f0 = 5.0;
        let hm = 300.0e3;
        let ym = 50.0e3;
        let rb = hm - ym;      // base na borda [rb, hm]
        let qp = QuasiParabolicProfile::new(f0, hm, ym, rb);

        // Pico em h = hm: wp deve ser 2π·f0
        let wp_peak = qp.omega_p(R_EARTH + hm, 0.0, 0.0);
        let wp_expected = 2.0 * PI * f0 * 1.0e6;
        assert!((wp_peak - wp_expected).abs() / wp_expected < 0.01,
            "wp_peak={}, esperado {}", wp_peak, wp_expected);

        // Zero fora de [rb, hm]
        assert_eq!(qp.ne(rb - 10.0), 0.0, "abaixo de rb deve ser zero");
        assert_eq!(qp.ne(hm + 10.0), 0.0, "acima de hm deve ser zero");

        // Na borda inferior, densidade nula
        assert!((qp.ne(rb)).abs() < 1.0,
            "ne(rb) deve tender a zero, got {}", qp.ne(rb));
    }
}
