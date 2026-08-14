//! RF-plane (spatial) lateral geometry.

/// Lateral RF geometry of the substrate.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LateralGeometry {
    /// Current azimuth of the RF plane in radians.
    pub azimuth: f64,
    /// Current elevation of the RF plane in radians.
    pub elevation: f64,
    /// Nominal azimuth the geometry should rest at.
    pub nominal_azimuth: f64,
    /// Nominal elevation the geometry should rest at.
    pub nominal_elevation: f64,
}

impl LateralGeometry {
    /// Geometry resting exactly at the nominal orientation.
    pub fn new() -> Self {
        Self {
            azimuth: 0.0,
            elevation: 0.0,
            nominal_azimuth: 0.0,
            nominal_elevation: 0.0,
        }
    }

    /// Geometry with an explicit nominal orientation.
    pub fn with_nominal(azimuth: f64, elevation: f64) -> Self {
        Self {
            azimuth,
            elevation,
            nominal_azimuth: azimuth,
            nominal_elevation: elevation,
        }
    }

    /// Normalized deviation of the current geometry from nominal, in `[0, 1]`.
    pub fn deviation(&self) -> f64 {
        let d = (self.azimuth - self.nominal_azimuth).abs() / std::f64::consts::PI
            + (self.elevation - self.nominal_elevation).abs() / std::f64::consts::PI;
        d.min(1.0)
    }

    /// Integrity of the lateral geometry, `1.0` at nominal, `0.0` at maximum
    /// deviation.
    pub fn integrity(&self) -> f64 {
        1.0 - self.deviation()
    }
}

impl Default for LateralGeometry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_geometry_has_full_integrity() {
        assert_eq!(LateralGeometry::new().integrity(), 1.0);
        assert_eq!(LateralGeometry::with_nominal(1.2, -0.4).integrity(), 1.0);
    }

    #[test]
    fn skewed_geometry_loses_integrity() {
        let mut g = LateralGeometry::with_nominal(0.0, 0.0);
        g.azimuth = std::f64::consts::PI;
        assert!(g.integrity() < 0.5);
    }
}
