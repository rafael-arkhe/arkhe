//! Núcleo espectral do Arkhe: SVD incremental, análise de DAG, Procrustes, ML-DSA.

pub mod error;
pub mod svd;
pub mod laplacian;
pub mod procrustes;
pub mod ml_dsa_guard;

pub use error::SpectralError;
pub use svd::IncrementalSVD;
pub use laplacian::{
    analyze_laplacian, build_laplacian_from_map, build_symmetrized_laplacian,
    check_dag_health, fiedler_partition, SpectralAnalysis,
};
pub use procrustes::{align_keys, key_compatibility_error, ProcrustesError};
pub use ml_dsa_guard::ml_dsa_65_offsets;