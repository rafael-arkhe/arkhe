//! Fixed unit-cell geometry of the metamaterial (from the article).

/// Geometry of the metamaterial unit cell.
///
/// Values extracted from the article: 20.5 × 20.5 × 14 µm³ with a 7 µm gold
/// disk.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UnitCellGeometry {
    /// Lattice period (µm).
    pub period: f64,
    /// Radius of the gold disk (µm).
    pub gold_radius: f64,
    /// Thickness of the SiO₂ dielectric (µm).
    pub dielectric_thickness: f64,
    /// Model thickness of the graphene layer (µm). Physical graphene is
    /// 0.34 nm; the layer is modelled as an effective sheet.
    pub graphene_thickness: f64,
}

impl Default for UnitCellGeometry {
    fn default() -> Self {
        Self {
            period: 20.5,
            gold_radius: 7.0,
            dielectric_thickness: 14.0,
            graphene_thickness: 0.01,
        }
    }
}
