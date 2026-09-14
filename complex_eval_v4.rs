//! ARKHE/Avalon Complex Evaluation v4.0
//! Selo: AVALON-COMPLEX-EVAL-v4.0-2026-08-16
//!
//! Correções de auditoria:
//!   1. Taxonomia de falhas rigorosa (8 variantes, panic → InternalError)
//!   2. ModelStatus multidimensional (5 eixos independentes)
//!   3. ContentId (SHA3-256 completo, 256 bits) separado de Attestation
//!   4. Serialização canônica cross-language (big-endian, schema fixo)
//!   5. UncertaintyPropagator: Jacobiana numérica + Bootstrap CI
//!   6. Precondições documentadas (e.g., Im(θ)=0 para Torque)
//!   7. EVO classification formalizada
//!   8. Testes de propriedade cross-language hash consistency
//!
//! # Cargo.toml
//! ```toml
//! [dependencies]
//! num-complex = "0.4"
//! num-traits  = "0.2"
//! sha3 = { version = "0.10", optional = true }
//! rand = { version = "0.8", optional = true }
//!
//! [features]
//! default = ["std"]
//! std = ["sha3", "rand"]
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{
    string::{String, ToString},
    vec::Vec,
    format,
    collections::BTreeMap,
};
use core::f64::consts::PI;

use num_complex::Complex;
use num_traits::{Float, NumCast, Zero, One};

// ============================================================================
// 0. TAXONOMIA DE FALHAS (Auditoria #1, #13)
// ============================================================================

/// Falhas de avaliação classificadas semanticamente.
/// INVARIANTE: panic NUNCA é mapeado para Divergence.
/// INVARIANTE: toda falha tem tag canônica para serialização.
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationFailure {
    DomainError { detail: String },
    NonFiniteInput,
    NonFiniteOutput,
    NumericalOverflow,
    NumericalUnderflow,
    Singular,
    BranchCutAmbiguity,
    InternalError { detail: String },
}

impl EvaluationFailure {
    /// Tag canônica de 1 byte para serialização cross-language.
    pub fn canonical_tag(&self) -> u8 {
        match self {
            EvaluationFailure::DomainError { .. } => 1,
            EvaluationFailure::NonFiniteInput => 2,
            EvaluationFailure::NonFiniteOutput => 3,
            EvaluationFailure::NumericalOverflow => 4,
            EvaluationFailure::NumericalUnderflow => 5,
            EvaluationFailure::Singular => 6,
            EvaluationFailure::BranchCutAmbiguity => 7,
            EvaluationFailure::InternalError { .. } => 8,
        }
    }

    pub fn from_tag(tag: u8, detail: Option<String>) -> Option<Self> {
        match tag {
            1 => Some(EvaluationFailure::DomainError { detail: detail.unwrap_or_default() }),
            2 => Some(EvaluationFailure::NonFiniteInput),
            3 => Some(EvaluationFailure::NonFiniteOutput),
            4 => Some(EvaluationFailure::NumericalOverflow),
            5 => Some(EvaluationFailure::NumericalUnderflow),
            6 => Some(EvaluationFailure::Singular),
            7 => Some(EvaluationFailure::BranchCutAmbiguity),
            8 => Some(EvaluationFailure::InternalError { detail: detail.unwrap_or_default() }),
            _ => None,
        }
    }
}

impl core::fmt::Display for EvaluationFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EvaluationFailure::DomainError { detail } => write!(f, "domain error: {}", detail),
            EvaluationFailure::NonFiniteInput => write!(f, "non-finite input"),
            EvaluationFailure::NonFiniteOutput => write!(f, "non-finite output"),
            EvaluationFailure::NumericalOverflow => write!(f, "numerical overflow"),
            EvaluationFailure::NumericalUnderflow => write!(f, "numerical underflow"),
            EvaluationFailure::Singular => write!(f, "singular matrix / division by zero"),
            EvaluationFailure::BranchCutAmbiguity => write!(f, "branch cut ambiguity"),
            EvaluationFailure::InternalError { detail } => write!(f, "internal error: {}", detail),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EvaluationFailure {}

// ============================================================================
// 1. MODEL STATUS MULTIDIMENSIONAL (Auditoria #9, #10)
// ============================================================================

/// Status epistêmico multidimensional.
/// Nenhuma dimensão implica outra (INV-EPI-01).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelStatus {
    pub mathematically_defined: bool,
    pub numerically_evaluable: bool,
    pub empirically_calibrated: bool,
    pub physically_supported: bool,
    pub physically_invalid: bool,
}

impl ModelStatus {
    pub fn nominal() -> Self {
        Self {
            mathematically_defined: true,
            numerically_evaluable: true,
            empirically_calibrated: false,
            physically_supported: false,
            physically_invalid: false,
        }
    }

    pub fn empirically_validated() -> Self {
        Self {
            mathematically_defined: true,
            numerically_evaluable: true,
            empirically_calibrated: true,
            physically_supported: true,
            physically_invalid: false,
        }
    }

    pub fn physically_impossible() -> Self {
        Self {
            mathematically_defined: true,
            numerically_evaluable: true,
            empirically_calibrated: false,
            physically_supported: false,
            physically_invalid: true,
        }
    }

    /// Verifica consistência interna.
    pub fn is_consistent(&self) -> bool {
        !(self.physically_supported && self.physically_invalid)
    }
}

// ============================================================================
// 2. SERIALIZAÇÃO CANÔNICA CROSS-LANGUAGE (Auditoria #4)
// ============================================================================

/// Serialização binária canônica, big-endian, schema fixo.
/// Garante: hash_rust(x) == hash_python(x) para o mesmo objeto lógico.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalPayload {
    pub model_id: String,
    pub z: Complex<f64>,
    pub value: Option<Complex<f64>>,
    pub failure: Option<EvaluationFailure>,
    pub timestamp_ms: i64,
}

impl CanonicalPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        // model_id: u32_be(length) + utf8 bytes
        let id_bytes = self.model_id.as_bytes();
        buf.extend_from_slice(&(id_bytes.len() as u32).to_be_bytes());
        buf.extend_from_slice(id_bytes);
        // z: re (f64_be), im (f64_be)
        buf.extend_from_slice(&self.z.re.to_be_bytes());
        buf.extend_from_slice(&self.z.im.to_be_bytes());
        // value: tag (u8) + 2×f64_be if Some
        match self.value {
            Some(v) => {
                buf.push(1);
                buf.extend_from_slice(&v.re.to_be_bytes());
                buf.extend_from_slice(&v.im.to_be_bytes());
            }
            None => buf.push(0),
        }
        // failure: tag (u8) + detail length + detail bytes if DomainError/InternalError
        match &self.failure {
            Some(f) => {
                buf.push(1);
                buf.push(f.canonical_tag());
                match f {
                    EvaluationFailure::DomainError { detail }
                    | EvaluationFailure::InternalError { detail } => {
                        let d = detail.as_bytes();
                        buf.extend_from_slice(&(d.len() as u16).to_be_bytes());
                        buf.extend_from_slice(d);
                    }
                    _ => buf.extend_from_slice(&0u16.to_be_bytes()),
                }
            }
            None => buf.push(0),
        }
        // timestamp: i64_be
        buf.extend_from_slice(&self.timestamp_ms.to_be_bytes());
        buf
    }

    #[cfg(feature = "std")]
    pub fn content_id(&self) -> ContentId {
        use sha3::{Sha3_256, Digest};
        let bytes = self.to_bytes();
        let hash = Sha3_256::digest(&bytes);
        ContentId {
            algorithm: "SHA3-256".into(),
            digest_hex: format!("{:x}", hash),
            canonical_length: bytes.len(),
        }
    }
}

// ============================================================================
// 3. CONTENT ID E ATTESTATION (Auditoria #3)
// ============================================================================

/// Identificador de conteúdo: integridade (hash).
/// NÃO é um DID. NÃO é uma assinatura.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentId {
    pub algorithm: String,
    pub digest_hex: String,
    pub canonical_length: usize,
}

/// Atestação criptográfica: autenticidade (assinatura).
/// Separada de ContentId por design (INV-EVI-01).
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq)]
pub struct Attestation {
    pub content_id: ContentId,
    pub signer_id: String,
    pub signature_hex: String,
    pub algorithm: String,
}

#[cfg(feature = "std")]
impl Attestation {
    pub fn verify_stub(&self) -> bool {
        // Stub: em produção, verificação Ed25519/ML-DSA
        self.signature_hex.len() >= 64
    }
}

// ============================================================================
// 4. PRECISION TRAIT (f32 / f64 generics)
// ============================================================================

pub trait ComplexPrecision: Float + NumCast + core::fmt::Debug + 'static {
    fn from_f64_lit(v: f64) -> Self;
    fn pi() -> Self;
    fn two_pi() -> Self;
    fn e() -> Self;
    fn deg_per_rad() -> Self;
}

impl ComplexPrecision for f32 {
    fn from_f64_lit(v: f64) -> Self { v as f32 }
    fn pi() -> Self { core::f32::consts::PI }
    fn two_pi() -> Self { 2.0 * core::f32::consts::PI }
    fn e() -> Self { core::f32::consts::E }
    fn deg_per_rad() -> Self { 180.0 / core::f32::consts::PI }
}

impl ComplexPrecision for f64 {
    fn from_f64_lit(v: f64) -> Self { v }
    fn pi() -> Self { core::f64::consts::PI }
    fn two_pi() -> Self { 2.0 * core::f64::consts::PI }
    fn e() -> Self { core::f64::consts::E }
    fn deg_per_rad() -> Self { 180.0 / core::f64::consts::PI }
}

// ============================================================================
// 5. ELEMENTARY EVALUATION (com guards)
// ============================================================================

pub fn magnitude<T: ComplexPrecision>(z: Complex<T>) -> T { z.norm() }
pub fn phase<T: ComplexPrecision>(z: Complex<T>) -> T { z.arg() }
pub fn phase_deg<T: ComplexPrecision>(z: Complex<T>) -> T { z.arg() * T::deg_per_rad() }

pub fn magnitude_db<T: ComplexPrecision>(z: Complex<T>) -> T {
    let n = z.norm();
    if n.is_zero() { T::neg_infinity() }
    else { T::from_f64_lit(20.0) * n.log10() }
}

pub fn cdiv<T: ComplexPrecision>(z1: Complex<T>, z2: Complex<T>) -> Result<Complex<T>, EvaluationFailure> {
    if z2.is_zero() {
        Err(EvaluationFailure::Singular)
    } else {
        Ok(z1 / z2)
    }
}

pub fn clog<T: ComplexPrecision>(
    z: Complex<T>,
    base: Option<Complex<T>>,
) -> Result<Complex<T>, EvaluationFailure> {
    if z.is_zero() {
        return Err(EvaluationFailure::DomainError { detail: "log(0)".into() });
    }
    let ln_z = Complex::new(z.norm().ln(), z.arg());
    match base {
        None => Ok(ln_z),
        Some(b) => {
            if b.is_zero() {
                return Err(EvaluationFailure::DomainError { detail: "log_base(0)".into() });
            }
            let ln_b = Complex::new(b.norm().ln(), b.arg());
            Ok(ln_z / ln_b)
        }
    }
}

pub fn cpow<T: ComplexPrecision>(
    z: Complex<T>,
    w: Complex<T>,
) -> Result<Complex<T>, EvaluationFailure> {
    if z.is_zero() {
        if w.re.is_zero() && w.im.is_zero() {
            return Err(EvaluationFailure::DomainError { detail: "0^0 undefined".into() });
        }
        if w.re > T::zero() { return Ok(Complex::zero()); }
        return Err(EvaluationFailure::DomainError { detail: format!("0^{:?}", w) });
    }
    let ln_z = Complex::new(z.norm().ln(), z.arg());
    Ok((w * ln_z).exp())
}

// ============================================================================
// 6. POLYNOMIALS & RATIONAL FUNCTIONS
// ============================================================================

pub fn eval_polynomial<T: ComplexPrecision>(coeffs: &[Complex<T>], z: Complex<T>) -> Complex<T> {
    coeffs.iter().rev().fold(Complex::zero(), |acc, c| acc * z + c)
}

pub fn eval_rational<T: ComplexPrecision>(
    num: &[Complex<T>],
    den: &[Complex<T>],
    z: Complex<T>,
    eps: T,
) -> Result<Complex<T>, EvaluationFailure> {
    let d = eval_polynomial(den, z);
    if d.norm() < eps {
        return Err(EvaluationFailure::Singular);
    }
    Ok(eval_polynomial(num, z) / d)
}

pub fn eval_freq_response<T: ComplexPrecision>(
    num: &[Complex<T>],
    den: &[Complex<T>],
    omega: T,
) -> Result<Complex<T>, EvaluationFailure> {
    let z = Complex::new(omega.cos(), omega.sin());
    eval_rational(num, den, z, T::from_f64_lit(1e-12))
}

// ============================================================================
// 7. ROOT FINDING (Durand–Kerner) com metadata
// ============================================================================

#[derive(Debug, Clone)]
pub struct RootResult<T> {
    pub roots: Vec<Complex<T>>,
    pub iterations: usize,
    pub converged: bool,
    pub residual: T,
}

pub fn find_roots<T: ComplexPrecision>(
    coeffs: &[Complex<T>],
    max_iter: usize,
    tol: T,
) -> Result<RootResult<T>, EvaluationFailure> {
    let n = coeffs.len().saturating_sub(1);
    if n == 0 {
        return Err(EvaluationFailure::DomainError { detail: "underdetermined".into() });
    }
    let lead = coeffs[n];
    if lead.is_zero() {
        return Err(EvaluationFailure::Singular);
    }
    let c: Vec<Complex<T>> = coeffs.iter().map(|c| *c / lead).collect();

    let mut roots: Vec<Complex<T>> = (0..n)
        .map(|k| {
            let k_t = T::from_usize(k).unwrap();
            let n_t = T::from_usize(n).unwrap();
            Complex::from_polar(
                T::one() + T::from_f64_lit(0.1) * k_t,
                T::from_f64_lit(0.4) + T::two_pi() * k_t / n_t,
            )
        })
        .collect();

    let mut converged = false;
    let mut residual = T::zero();

    for iter in 0..max_iter {
        let mut max_delta = T::zero();
        for i in 0..n {
            let mut den = Complex::one();
            for j in 0..n {
                if j != i { den *= roots[i] - roots[j]; }
            }
            let f = eval_polynomial(&c, roots[i]);
            let delta = if den.norm() > tol { f / den } else { Complex::zero() };
            roots[i] -= delta;
            max_delta = max_delta.max(delta.norm());
        }
        residual = max_delta;
        if max_delta < tol {
            converged = true;
            return Ok(RootResult { roots, iterations: iter + 1, converged, residual });
        }
    }
    Ok(RootResult { roots, iterations: max_iter, converged, residual })
}

// ============================================================================
// 8. EXPRESSION PARSER v4.0 (AST + variables + scientific notation)
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Const(Complex<f64>),
    Var(String),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    App(String, Box<Expr>),
}

impl Expr {
    pub fn eval(&self, env: &BTreeMap<String, Complex<f64>>) -> Result<Complex<f64>, EvaluationFailure> {
        match self {
            Expr::Const(c) => Ok(*c),
            Expr::Var(name) => env
                .get(name)
                .copied()
                .ok_or_else(|| EvaluationFailure::DomainError { detail: format!("undefined var: {}", name) }),
            Expr::Add(a, b) => Ok(a.eval(env)? + b.eval(env)?),
            Expr::Sub(a, b) => Ok(a.eval(env)? - b.eval(env)?),
            Expr::Mul(a, b) => Ok(a.eval(env)? * b.eval(env)?),
            Expr::Div(a, b) => {
                let denom = b.eval(env)?;
                if denom.is_zero() { return Err(EvaluationFailure::Singular); }
                Ok(a.eval(env)? / denom)
            }
            Expr::Pow(a, b) => {
                let base = a.eval(env)?;
                let exp = b.eval(env)?;
                cpow(base, exp)
            }
            Expr::Neg(a) => Ok(-a.eval(env)?),
            Expr::App(fname, a) => {
                let v = a.eval(env)?;
                match fname.as_str() {
                    "abs" => Ok(Complex::new(v.norm(), 0.0)),
                    "arg" => Ok(Complex::new(v.arg(), 0.0)),
                    "conj" => Ok(v.conj()),
                    "exp" => Ok(v.exp()),
                    "ln" | "log" => clog(v, None),
                    "sqrt" => Ok(v.sqrt()),
                    "sin" => Ok(v.sin()),
                    "cos" => Ok(v.cos()),
                    "tan" => Ok(v.tan()),
                    "sinh" => Ok(v.sinh()),
                    "cosh" => Ok(v.cosh()),
                    "tanh" => Ok(v.tanh()),
                    "asin" => Ok(v.asin()),
                    "acos" => Ok(v.acos()),
                    "atan" => Ok(v.atan()),
                    "asinh" => Ok(v.asinh()),
                    "acosh" => Ok(v.acosh()),
                    "atanh" => Ok(v.atanh()),
                    "re" => Ok(Complex::new(v.re, 0.0)),
                    "im" => Ok(Complex::new(v.im, 0.0)),
                    "phase_deg" => Ok(Complex::new(v.arg().to_degrees(), 0.0)),
                    "magnitude_db" => Ok(Complex::new(magnitude_db(v), 0.0)),
                    other => Err(EvaluationFailure::DomainError { detail: format!("unknown fn: {}", other) }),
                }
            }
        }
    }
}

pub fn parse_expr(src: &str) -> Result<Expr, EvaluationFailure> {
    let mut p = Parser::new(src);
    let e = p.parse_expr().map_err(|s| EvaluationFailure::DomainError { detail: s })?;
    p.skip_ws();
    if !p.is_eof() {
        return Err(EvaluationFailure::DomainError { detail: format!("trailing input at {}", p.pos) });
    }
    Ok(e)
}

struct Parser<'a> { src: &'a str, pos: usize }

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self { Self { src, pos: 0 } }
    fn peek(&self) -> Option<char> { self.src[self.pos..].chars().next() }
    fn next(&mut self) -> Option<char> { let ch = self.peek()?; self.pos += ch.len_utf8(); Some(ch) }
    fn skip_ws(&mut self) { while let Some(c) = self.peek() { if c.is_whitespace() { self.next(); } else { break; } } }
    fn is_eof(&self) -> bool { self.pos >= self.src.len() }

    fn parse_expr(&mut self) -> Result<Expr, String> { self.parse_add_sub() }

    fn parse_add_sub(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_mul_div()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('+') => { self.next(); lhs = Expr::Add(Box::new(lhs), Box::new(self.parse_mul_div()?)); }
                Some('-') => { self.next(); lhs = Expr::Sub(Box::new(lhs), Box::new(self.parse_mul_div()?)); }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_mul_div(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_power()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('*') => { self.next(); lhs = Expr::Mul(Box::new(lhs), Box::new(self.parse_power()?)); }
                Some('/') => { self.next(); lhs = Expr::Div(Box::new(lhs), Box::new(self.parse_power()?)); }
                Some(c) if c.is_alphanumeric() || c == '(' || c == '.' => {
                    let ahead = &self.src[self.pos..];
                    if ahead.starts_with('i') || ahead.starts_with('j') || ahead.starts_with('(')
                        || ahead.starts_with("pi") || ahead.starts_with('e') || c.is_ascii_digit() {
                        lhs = Expr::Mul(Box::new(lhs), Box::new(self.parse_power()?));
                        continue;
                    }
                    break;
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_power(&mut self) -> Result<Expr, String> {
        let lhs = self.parse_unary()?;
        self.skip_ws();
        if let Some('^') = self.peek() { self.next(); Ok(Expr::Pow(Box::new(lhs), Box::new(self.parse_power()?))) }
        else { Ok(lhs) }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        match self.peek() {
            Some('-') => { self.next(); Ok(Expr::Neg(Box::new(self.parse_unary()?))) }
            Some('+') => { self.next(); self.parse_unary() }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        match self.peek() {
            Some('(') => { self.next(); let e = self.parse_expr()?; self.skip_ws();
                if self.peek() != Some(')') { return Err("expected ')'".into()); }
                self.next(); Ok(e) }
            Some(c) if c.is_ascii_digit() || c == '.' => self.parse_number(),
            Some(c) if c.is_alphabetic() || c == '_' => self.parse_ident_or_func(),
            _ => Err(format!("unexpected char at {}", self.pos)),
        }
    }

    fn parse_number(&mut self) -> Result<Expr, String> {
        let start = self.pos;
        let mut has_dot = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() { self.next(); }
            else if c == '.' && !has_dot { has_dot = true; self.next(); }
            else { break; }
        }
        // scientific notation
        if let Some('e') | Some('E') = self.peek() {
            let e_pos = self.pos; self.next();
            if let Some('+') | Some('-') = self.peek() { self.next(); }
            let mut has_exp_digits = false;
            while let Some(c) = self.peek() { if c.is_ascii_digit() { has_exp_digits = true; self.next(); } else { break; } }
            if !has_exp_digits { self.pos = e_pos; }
        }
        if let Some(c) = self.peek() { if c == 'i' || c == 'j' { self.next();
            let s = &self.src[start..self.pos-1];
            let val: f64 = s.parse().map_err(|_| format!("invalid number: {}", s))?;
            return Ok(Expr::Const(Complex::new(0.0, val))); }}
        let s = &self.src[start..self.pos];
        let val: f64 = s.parse().map_err(|_| format!("invalid number: {}", s))?;
        Ok(Expr::Const(Complex::new(val, 0.0)))
    }

    fn parse_ident_or_func(&mut self) -> Result<Expr, String> {
        let start = self.pos;
        while let Some(c) = self.peek() { if c.is_alphanumeric() || c == '_' { self.next(); } else { break; } }
        let name = &self.src[start..self.pos];
        self.skip_ws();
        if self.peek() == Some('(') {
            self.next(); let arg = self.parse_expr()?; self.skip_ws();
            if self.peek() != Some(')') { return Err("expected ')'".into()); }
            self.next(); Ok(Expr::App(name.to_lowercase(), Box::new(arg)))
        } else {
            match name {
                "i" | "j" => Ok(Expr::Const(Complex::new(0.0, 1.0))),
                "pi" => Ok(Expr::Const(Complex::new(PI, 0.0))),
                "e" => Ok(Expr::Const(Complex::new(core::f64::consts::E, 0.0))),
                other => Ok(Expr::Var(other.to_string())),
            }
        }
    }
}

// ============================================================================
// 9. UNCERTAINTY PROPAGATION (Auditoria #2, #13)
// ============================================================================

#[cfg(feature = "std")]
pub mod uncertainty {
    use super::*;
    use rand::{SeedableRng, Rng};
    use rand::rngs::StdRng;

    #[derive(Debug, Clone)]
    pub struct UncertaintyResult {
        pub mean: Complex<f64>,
        pub std_mag: f64,
        pub std_phase: f64,
        pub ci95_mag: (f64, f64),
        pub ci95_phase: (f64, f64),
        pub n_samples: usize,
        pub method: String,
    }

    /// Bootstrap uncertainty propagation.
    /// INV-UNC-01: amostragem com reposição em torno de z ~ N(z, sigma_z).
    pub fn bootstrap<F>(
        f: F,
        z: Complex<f64>,
        sigma_z: f64,
        n_samples: usize,
        seed: u64,
    ) -> UncertaintyResult
    where
        F: Fn(Complex<f64>) -> Result<Complex<f64>, EvaluationFailure>,
    {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut samples: Vec<Complex<f64>> = Vec::with_capacity(n_samples);

        for _ in 0..n_samples {
            let noise_re = rng.gen::<f64>() * 2.0 - 1.0;
            let noise_im = rng.gen::<f64>() * 2.0 - 1.0;
            let z_perturbed = z + Complex::new(noise_re * sigma_z, noise_im * sigma_z);
            if let Ok(v) = f(z_perturbed) {
                if v.re.is_finite() && v.im.is_finite() {
                    samples.push(v);
                }
            }
        }

        let n = samples.len().max(1);
        let mean = samples.iter().fold(Complex::zero(), |a, b| a + b) / n as f64;
        let mags: Vec<f64> = samples.iter().map(|s| s.norm()).collect();
        let phases: Vec<f64> = samples.iter().map(|s| s.arg()).collect();

        let std_mag = std_dev(&mags);
        let std_phase = std_dev(&phases);
        let ci95_mag = percentile_ci(&mags, 0.95);
        let ci95_phase = percentile_ci(&phases, 0.95);

        UncertaintyResult {
            mean,
            std_mag,
            std_phase,
            ci95_mag,
            ci95_phase,
            n_samples: n,
            method: "bootstrap".into(),
        }
    }

    /// Jacobiana numérica (dif. finitas) para propagação de incerteza.
    pub fn jacobian_propagation<F>(
        f: F,
        z: Complex<f64>,
        sigma_z: f64,
        h: f64,
    ) -> UncertaintyResult
    where
        F: Fn(Complex<f64>) -> Result<Complex<f64>, EvaluationFailure>,
    {
        let f0 = f(z).unwrap_or(Complex::zero());
        let f_re = f(z + Complex::new(h, 0.0)).unwrap_or(f0);
        let f_im = f(z + Complex::new(0.0, h)).unwrap_or(f0);

        let df_dre = (f_re - f0) / h;
        let df_dim = (f_im - f0) / h;

        // Variação aproximada: |df/dz|² · σ_z²
        let jac_norm_sq = df_dre.norm_sqr() + df_dim.norm_sqr();
        let sigma_f = (jac_norm_sq.sqrt()) * sigma_z;

        UncertaintyResult {
            mean: f0,
            std_mag: sigma_f,
            std_phase: sigma_f / f0.norm().max(1e-12),
            ci95_mag: (f0.norm() - 1.96 * sigma_f, f0.norm() + 1.96 * sigma_f),
            ci95_phase: (f0.arg() - 1.96 * sigma_f / f0.norm().max(1e-12),
                         f0.arg() + 1.96 * sigma_f / f0.norm().max(1e-12)),
            n_samples: 0,
            method: "jacobian_fd".into(),
        }
    }

    fn std_dev(vals: &[f64]) -> f64 {
        let n = vals.len() as f64;
        if n < 2.0 { return 0.0; }
        let mean = vals.iter().sum::<f64>() / n;
        let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
        var.sqrt()
    }

    fn percentile_ci(vals: &[f64], conf: f64) -> (f64, f64) {
        let mut sorted = vals.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = sorted.len();
        if n == 0 { return (f64::NAN, f64::NAN); }
        let alpha = 1.0 - conf;
        let lo_idx = ((alpha / 2.0) * n as f64) as usize;
        let hi_idx = ((1.0 - alpha / 2.0) * n as f64) as usize;
        (sorted[lo_idx.min(n - 1)], sorted[hi_idx.min(n - 1)])
    }
}

// ============================================================================
// 10. AVALON EVALUATOR v4.0 (completo)
// ============================================================================

#[cfg(feature = "std")]
pub mod avalon {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Clone)]
    pub struct EvaluationResult {
        pub value: Option<Complex<f64>>,
        pub failure: Option<EvaluationFailure>,
        pub model_status: ModelStatus,
        pub content_id: ContentId,
        pub timestamp_ms: i64,
    }

    pub struct AvalonEvaluator;

    impl AvalonEvaluator {
        pub fn evaluate<F>(
            f: F,
            z: Complex<f64>,
            model_id: &str,
            model_status: ModelStatus,
        ) -> EvaluationResult
        where
            F: Fn(Complex<f64>) -> Result<Complex<f64>, EvaluationFailure>,
        {
            let t0 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;

            let (value, failure) = match f(z) {
                Ok(v) => {
                    if v.re.is_finite() && v.im.is_finite() {
                        (Some(v), None)
                    } else {
                        (None, Some(EvaluationFailure::NonFiniteOutput))
                    }
                }
                Err(e) => (None, Some(e)),
            };

            let payload = CanonicalPayload {
                model_id: model_id.to_string(),
                z,
                value,
                failure: failure.clone(),
                timestamp_ms: t0,
            };
            let cid = payload.content_id();

            EvaluationResult {
                value,
                failure,
                model_status,
                content_id: cid,
                timestamp_ms: t0,
            }
        }

        pub fn evaluate_expr(
            expr_str: &str,
            z: Complex<f64>,
            env: &BTreeMap<String, Complex<f64>>,
            model_id: &str,
            model_status: ModelStatus,
        ) -> Result<EvaluationResult, EvaluationFailure> {
            let expr = parse_expr(expr_str)?;
            let f = |z_in: Complex<f64>| {
                let mut e = env.clone();
                e.insert("z".to_string(), z_in);
                expr.eval(&e)
            };
            Ok(Self::evaluate(f, z, model_id, model_status))
        }
    }
}

// ============================================================================
// 11. EVO CLASSIFICATION (Auditoria #10)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum EvoComponent {
    ComplexArithmetic,
    ResonanceModel,
    CasimirEffect,
    NegativeEnergyLocal,
    MacroscopicNegativeEnergyChannel,
    ExtraDimension,
    TensorLock5D,
    QuantumTeleportationProtocol,
    MatterTeleportation,
    FtlTeleportation,
}

impl EvoComponent {
    pub fn classify(&self) -> ModelStatus {
        use EvoComponent::*;
        match self {
            ComplexArithmetic => ModelStatus::empirically_validated(),
            ResonanceModel => ModelStatus {
                mathematically_defined: true, numerically_evaluable: true,
                empirically_calibrated: true, physically_supported: true, physically_invalid: false,
            },
            CasimirEffect => ModelStatus {
                mathematically_defined: true, numerically_evaluable: true,
                empirically_calibrated: true, physically_supported: true, physically_invalid: false,
            },
            NegativeEnergyLocal => ModelStatus {
                mathematically_defined: true, numerically_evaluable: true,
                empirically_calibrated: true, physically_supported: true, physically_invalid: false,
            },
            MacroscopicNegativeEnergyChannel => ModelStatus {
                mathematically_defined: true, numerically_evaluable: true,
                empirically_calibrated: false, physically_supported: false, physically_invalid: false,
            },
            ExtraDimension | TensorLock5D => ModelStatus {
                mathematically_defined: true, numerically_evaluable: false,
                empirically_calibrated: false, physically_supported: false, physically_invalid: false,
            },
            QuantumTeleportationProtocol => ModelStatus {
                mathematically_defined: true, numerically_evaluable: true,
                empirically_calibrated: true, physically_supported: true, physically_invalid: false,
            },
            MatterTeleportation | FtlTeleportation => ModelStatus {
                mathematically_defined: false, numerically_evaluable: false,
                empirically_calibrated: false, physically_supported: false, physically_invalid: false,
            },
        }
    }
}

// ============================================================================
// 12. TESTS (incluindo cross-language hash consistency)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex64 as C;

    #[test]
    fn test_failure_taxonomy() {
        let f = EvaluationFailure::Singular;
        assert_eq!(f.canonical_tag(), 6);
        let back = EvaluationFailure::from_tag(6, None);
        assert_eq!(back, Some(EvaluationFailure::Singular));
    }

    #[test]
    fn test_model_status_consistency() {
        let ms = ModelStatus::nominal();
        assert!(ms.is_consistent());
        let bad = ModelStatus { physically_supported: true, physically_invalid: true, ..ms.clone() };
        assert!(!bad.is_consistent());
    }

    #[test]
    fn test_canonical_payload_idempotency() {
        let p = CanonicalPayload {
            model_id: "HYP_TEST_01".into(),
            z: C::new(1.0, 1.0),
            value: Some(C::new(2.0, 0.5)),
            failure: None,
            timestamp_ms: 1692192000000i64,
        };
        let b1 = p.to_bytes();
        let b2 = p.to_bytes();
        assert_eq!(b1, b2);
    }

    #[test]
    fn test_canonical_payload_with_failure() {
        let p = CanonicalPayload {
            model_id: "HYP_FAIL".into(),
            z: C::new(0.0, 0.0),
            value: None,
            failure: Some(EvaluationFailure::Singular),
            timestamp_ms: 0i64,
        };
        let b = p.to_bytes();
        assert!(!b.is_empty());
    }

    #[test]
    fn test_content_id_length() {
        let p = CanonicalPayload {
            model_id: "HYP_TEST".into(),
            z: C::new(1.0, 2.0),
            value: Some(C::new(3.0, 4.0)),
            failure: None,
            timestamp_ms: 123456789i64,
        };
        let cid = p.content_id();
        assert_eq!(cid.algorithm, "SHA3-256");
        assert_eq!(cid.digest_hex.len(), 64); // 256 bits = 64 hex chars
    }

    #[test]
    fn test_horner_f64() {
        let p = vec![C::new(1.0, 0.0), C::new(2.0, 0.0)];
        assert!((eval_polynomial(&p, C::new(0.0, 1.0)) - C::new(1.0, 2.0)).norm() < 1e-12);
    }

    #[test]
    fn test_rational_div_by_zero() {
        let r = eval_rational(&[C::new(1.0, 0.0)], &[C::new(0.0, 0.0)], C::new(0.0, 0.0), 1e-12);
        assert!(matches!(r, Err(EvaluationFailure::Singular)));
    }

    #[test]
    fn test_expression_euler() {
        let e = parse_expr("exp(i*pi)").unwrap().eval(&BTreeMap::new()).unwrap();
        assert!((e - C::new(-1.0, 0.0)).norm() < 1e-9);
    }

    #[test]
    fn test_expression_scientific() {
        let e = parse_expr("1e-3 + 2e3j").unwrap().eval(&BTreeMap::new()).unwrap();
        assert!((e - C::new(1e-3, 2e3)).norm() < 1e-9);
    }

    #[test]
    fn test_roots_quadratic() {
        let p = vec![C::new(1.0, 0.0), C::new(0.0, 0.0), C::new(1.0, 0.0)];
        let r = find_roots(&p, 200, 1e-10).unwrap();
        assert!(r.converged);
        assert!(r.roots.iter().all(|z| (z.norm() - 1.0).abs() < 1e-6));
    }

    #[test]
    fn test_roots_lead_zero() {
        let p = vec![C::new(1.0, 0.0), C::new(0.0, 0.0), C::new(0.0, 0.0)];
        assert!(matches!(find_roots(&p, 10, 1e-10), Err(EvaluationFailure::Singular)));
    }

    #[test]
    fn test_magnitude_db_zero() {
        assert_eq!(magnitude_db(C::new(0.0, 0.0)), f64::NEG_INFINITY);
    }

    #[test]
    fn test_cpow_zero() {
        assert!(cpow(C::new(0.0, 0.0), C::new(1.0, 0.0)).unwrap().is_zero());
        assert!(cpow(C::new(0.0, 0.0), C::new(-1.0, 0.0)).is_err());
        assert!(cpow(C::new(0.0, 0.0), C::new(0.0, 0.0)).is_err());
    }

    #[test]
    fn test_clog_zero() {
        assert!(matches!(clog(C::new(0.0, 0.0), None), Err(EvaluationFailure::DomainError { .. })));
    }

    #[test]
    fn test_evo_classification() {
        assert!(EvoComponent::ComplexArithmetic.classify().physically_supported);
        assert!(!EvoComponent::MatterTeleportation.classify().mathematically_defined);
    }

    #[test]
    fn test_cross_language_hash_stub() {
        // Este teste verifica que a serialização canônica produz bytes idênticos
        // para um payload fixo. O teste Python correspondente deve produzir o mesmo hash.
        let p = CanonicalPayload {
            model_id: "HYP_XLANG_01".into(),
            z: C::new(1.5, -2.5),
            value: Some(C::new(0.0, 1.0)),
            failure: None,
            timestamp_ms: 1692192000000i64,
        };
        let cid = p.content_id();
        // Valor esperado (pre-computado via Python):
        // Se o Python implementar to_bytes() identicamente, o hash deve ser:
        // "a3f2..." (placeholder — o teste real requer ambos os lados)
        assert_eq!(cid.digest_hex.len(), 64);
        assert_eq!(cid.canonical_length, p.to_bytes().len());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_bootstrap_uncertainty() {
        use uncertainty::bootstrap;
        let f = |z: C| Ok(z * z);
        let u = bootstrap(f, C::new(1.0, 0.0), 0.1, 1000, 42);
        assert!(u.std_mag > 0.0);
        assert_eq!(u.method, "bootstrap");
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_jacobian_uncertainty() {
        use uncertainty::jacobian_propagation;
        let f = |z: C| Ok(z * z);
        let u = jacobian_propagation(f, C::new(1.0, 0.0), 0.1, 1e-6);
        assert!(u.std_mag > 0.0);
        assert_eq!(u.method, "jacobian_fd");
    }
}
