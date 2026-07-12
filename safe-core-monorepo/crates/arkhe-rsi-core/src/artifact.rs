use crate::Digest;
use arkhe_core::hash::blake3_hash;
use serde::{Deserialize, Serialize};

/// Tipo de conteúdo carregado por um [`Artifact`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactKind {
    Code,
    Prompt,
    Config,
}

/// Um artefato versionado do loop RSI (baseline, candidato ou aplicado).
///
/// `id` é derivado do conteúdo via BLAKE3: dois artefatos com o mesmo
/// conteúdo têm sempre o mesmo `id`, o que torna a deduplicação e o
/// registro de iteração verificáveis por conteúdo, não por contador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: Digest,
    pub kind: ArtifactKind,
    pub content: String,
}

impl Artifact {
    pub fn new(kind: ArtifactKind, content: impl Into<String>) -> Self {
        let content = content.into();
        let id = blake3_hash(content.as_bytes());
        Self { id, kind, content }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_deterministic_and_content_sensitive() {
        let a = Artifact::new(ArtifactKind::Code, "fn main() {}");
        let b = Artifact::new(ArtifactKind::Code, "fn main() {}");
        let c = Artifact::new(ArtifactKind::Code, "fn main() { loop {} }");
        assert_eq!(a.id, b.id);
        assert_ne!(a.id, c.id);
    }
}
