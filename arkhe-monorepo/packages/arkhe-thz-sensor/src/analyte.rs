//! Analyte target deposited on the sensor.

/// Analyte layer deposited on the sensor surface.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Analyte {
    /// Refractive index (RI) of the analyte (1.32–1.40 in the article).
    pub refractive_index: f64,
    /// Analyte thickness in µm (1–5 µm in the article).
    pub thickness_um: f64,
}

impl Analyte {
    /// Build an analyte with physically bounded parameters.
    ///
    /// RI is clamped to `[1.0, 2.0]`; thickness to a minimum of `0.1 µm` (a
    /// zero-thickness layer models clean air as `Analyte::air()`).
    pub fn new(ri: f64, thickness: f64) -> Self {
        Self {
            refractive_index: ri.clamp(1.0, 2.0),
            thickness_um: thickness.max(0.1),
        }
    }

    /// Clean-air reference layer (no analyte deposited).
    pub fn air() -> Self {
        Self::new(1.0, 0.0)
    }

    /// Effective RI contrast against air, `n - 1`.
    pub fn delta_ri(&self) -> f64 {
        self.refractive_index - 1.0
    }

    /// Thickness factor relative to the 1 µm calibration thickness, capped at
    /// the article's 5 µm max.
    pub fn thickness_factor(&self) -> f64 {
        (self.thickness_um / 1.0).min(5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_has_zero_contrast() {
        let air = Analyte::air();
        assert_eq!(air.delta_ri(), 0.0);
        assert_eq!(air.thickness_factor(), 0.1);
    }

    #[test]
    fn parameters_are_bounded() {
        let a = Analyte::new(3.0, 8.0);
        assert_eq!(a.refractive_index, 2.0);
        assert_eq!(a.thickness_factor(), 5.0);
    }

    #[test]
    fn thickness_factor_scales_with_um() {
        assert_eq!(Analyte::new(1.4, 2.0).thickness_factor(), 2.0);
    }
}
