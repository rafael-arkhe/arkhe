//! # Registry canónico de invariantes — política de atestação por realização
//!
//! Bloco 1057 (ratificado): o registro canónico materializa a decisão do parecer
//! `ARKHE-BLOCO-1057-RATIFICADO-2026-09-09` e de `docs/namespace_policy.md`:
//!
//! 1. Um ID só é atestado se houver **realização testada** (código + teste +
//!    entrada no ledger).
//! 2. Referências sem substrato **não** geram renomeações; são rejeitadas e
//!    mantidas para auditoria (`ReferenciaSubstrateless`).
//! 3. IDs futuros tomam IDs frescos (I625+), nunca reutilizam IDs canónicos.
//!
//! ## I624 — apuração do bloco 1057
//!
//! O canónico `I624` é **ARKHE Chaves** (família I619/I622/I623/I624 do
//! `arkhe-blink-bridge`), implementado/testado desde o bloco 1052 e
//! cripto-vinculado no bloco 1057 (peer binding + signed message + anti-replay).
//! As referências a «I624 = MLPerf» e «I624 = KeyMgmt» têm **0 ocorrências** no
//! monorepo (grep) e foram rejeitadas nos pareceres v510/v510.1 — não reclamam
//! o ID. Conforme `bloco_1053.json` (correção epistémica), um ID só entra na
//! matriz de rastreabilidade se houver realização neste crate; registar o
//! contrário seria fabricar cobertura.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Domínio de um invariante atestado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dominio {
    P2P,
    HSM,
    GPU,
    Performance,
    PQC,
    LuzEstruturada,
    OpenResearch,
    Kernel,
}

/// Estado de atestação de um invariante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Especificado,
    Implementado,
    Testado,
    Ratificado,
}

/// Invariante atestado no registry canónico.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invariante {
    /// ID canónico (ex.: `I624`).
    pub id: String,
    /// Nome estável do invariante.
    pub nome: String,
    /// Descrição funcional.
    pub descricao: String,
    /// Domínio de realização.
    pub dominio: Dominio,
    /// Bloco que atestou/alterou o invariante (opcional).
    pub bloco: Option<u64>,
    /// Estado de atestação.
    pub status: Status,
    /// Evidências de realização testada (arquivo/linha/teste).
    pub evidencia: Vec<String>,
}

/// Referência sem substrato — registada apenas para auditoria, nunca atestada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenciaSubstrateless {
    /// ID referido (mesmo que o canónico, quando a referência é morta).
    pub id: String,
    /// Origem da referência (arquivo, parecer, plano).
    pub origem: String,
    /// Motivo da rejeição registada.
    pub motivo_rejeicao: String,
}

/// Erros do registry.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Duas implementações reais (com substrato) reclamam o mesmo ID.
    #[error("Colisão real (duas implementações para {0})")]
    ColisaoReal(String),
    /// Atestação reivindicada sem evidência de teste para status não-especificado.
    #[error("{0}: status sem evidência de teste")]
    SemEvidencia(String),
    /// ID não encontrado.
    #[error("Invariante não encontrado: {0}")]
    NaoEncontrado(String),
}

/// Registry canónico de invariantes (bloco 1057).
#[derive(Debug, Default)]
pub struct InvariantRegistry {
    invariantes: HashMap<String, Invariante>,
    substrateless: Vec<ReferenciaSubstrateless>,
}

impl InvariantRegistry {
    /// Cria um registry vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Atesta um invariante. Requer pelo menos uma evidência de teste para
    /// todo status acima de `Especificado`; rejeita colisão real (duas
    /// implementações com substrato para o mesmo ID).
    pub fn atestar(&mut self, inv: Invariante) -> Result<(), RegistryError> {
        if inv.evidencia.is_empty() && inv.status != Status::Especificado {
            return Err(RegistryError::SemEvidencia(inv.id.clone()));
        }
        if let Some(existente) = self.invariantes.get(&inv.id) {
            if !existente.evidencia.is_empty() && !inv.evidencia.is_empty() {
                return Err(RegistryError::ColisaoReal(inv.id.clone()));
            }
        }
        self.invariantes.insert(inv.id.clone(), inv);
        Ok(())
    }

    /// Registra uma referência sem substrato (não atestada).
    pub fn rejeitar_referencia(&mut self, r: ReferenciaSubstrateless) {
        self.substrateless.push(r);
    }

    /// Resolve um ID atestado.
    pub fn resolver(&self, id: &str) -> Option<&Invariante> {
        self.invariantes.get(id)
    }

    /// Referências rejeitadas (sem substrato), mantidas para auditoria.
    pub fn referencias_rejeitadas(&self) -> &[ReferenciaSubstrateless] {
        &self.substrateless
    }

    /// Todos os invariantes atestados.
    pub fn todos(&self) -> Vec<&Invariante> {
        self.invariantes.values().collect()
    }
}

/// Popula o registry canónico — SEM renomeações (bloco 1057).
pub fn registry_canonico() -> InvariantRegistry {
    let mut r = InvariantRegistry::new();

    // I624: ARKHE Chaves (canónico, atestado desde o bloco 1052,
    // cripto-vinculado no 1057).
    r.atestar(Invariante {
        id: "I624".into(),
        nome: "ARKHE Chaves (P2P Crypto-Binding)".into(),
        descricao: "Peer binding + signed message referenciada por chave registada em blink transactions".into(),
        dominio: Dominio::P2P,
        bloco: Some(1057),
        status: Status::Ratificado,
        evidencia: vec![
            "tests/i624_p2p_binding_test.rs (6 testes)".into(),
            "docs/triple_consolidation.md:44".into(),
            "bloco_1052.json".into(),
            "bloco_1053.json (correção epistémica)".into(),
            "bloco_1057.json (ratificado)".into(),
        ],
    })
    .expect("I624 com evidência e sem colisão real");

    // Referências mortas — registadas para auditoria, nunca atestadas.
    r.rejeitar_referencia(ReferenciaSubstrateless {
        id: "I624".into(),
        origem: "parecer_v510_substrato_fotonico.md:17".into(),
        motivo_rejeicao: "Prensas v508/v509 'I620, I624, I626-I640' explicitamente rejeitadas como 'Não são nós reais'.".into(),
    });
    r.rejeitar_referencia(ReferenciaSubstrateless {
        id: "I624".into(),
        origem: "parecer_v510_1_integracao_ml.md:33".into(),
        motivo_rejeicao: "'assinatura I624' no plano IA/ML marcada como 'Sem substrato de ML no monorepo'.".into(),
    });
    r.rejeitar_referencia(ReferenciaSubstrateless {
        id: "I624".into(),
        origem: "parecer ARKHE-BLOCO-1057-PARECER-CONDICIONAL (proposta) ".into(),
        motivo_rejeicao: "'I624 = MLPerf' renomeação proposta tinha 0 ocorrências de 'MLPerf' no monorepo; rejeitada sem gerar renomeação.".into(),
    });

    // I625+ ficam livres para invariantes futuros com substrato real.
    // (Nenhum registado no momento — IDs aguardam realização.)

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_sem_renomeacao() {
        let r = registry_canonico();
        let i624 = r.resolver("I624").expect("I624 atestado");
        assert_eq!(i624.nome, "ARKHE Chaves (P2P Crypto-Binding)");
        assert_eq!(i624.status, Status::Ratificado);
        assert!(i624.evidencia.len() >= 4);
        assert!(r.resolver("I625").is_none(), "I625 não pode existir sem realização");
        assert!(r.resolver("I626").is_none(), "I626 não pode existir sem realização");
    }

    #[test]
    fn test_referencias_rejeitadas() {
        let r = registry_canonico();
        let refs = r.referencias_rejeitadas();
        assert_eq!(refs.len(), 3);
        assert!(refs.iter().all(|f| f.id == "I624"));
    }

    #[test]
    fn test_colisao_real_detectada() {
        let mut r = InvariantRegistry::new();
        r.atestar(Invariante {
            id: "I700".into(),
            nome: "A".into(),
            descricao: "impl A".into(),
            dominio: Dominio::PQC,
            bloco: None,
            status: Status::Testado,
            evidencia: vec!["teste A".into()],
        })
        .expect("primeira atestação ok");

        let err = r.atestar(Invariante {
            id: "I700".into(),
            nome: "B".into(),
            descricao: "impl B".into(),
            dominio: Dominio::GPU,
            bloco: None,
            status: Status::Testado,
            evidencia: vec!["teste B".into()],
        });
        assert!(matches!(err, Err(RegistryError::ColisaoReal(id)) if id == "I700"));
    }

    #[test]
    fn test_atestacao_sem_evidencia_rejeitada() {
        let mut r = InvariantRegistry::new();
        let err = r.atestar(Invariante {
            id: "I999".into(),
            nome: "Fantasma".into(),
            descricao: "sem teste".into(),
            dominio: Dominio::Performance,
            bloco: None,
            status: Status::Testado,
            evidencia: vec![],
        });
        assert!(matches!(err, Err(RegistryError::SemEvidencia(_))));
    }
}