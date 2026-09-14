use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpectralError {
    #[error("DAG vazio")]
    Empty,
    #[error("DAG não possui arestas")]
    NoEdges,
    #[error("Erro espectral: {0}")]
    EigenFailed(String),
}