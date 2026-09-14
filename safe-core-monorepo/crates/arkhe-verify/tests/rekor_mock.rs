//! `RekorClient` contra um servidor HTTP **local** (mockito), para que o
//! contrato de fio — caminho, parâmetros de consulta, cabeçalho, mapeamento de
//! status e interpretação do corpo — seja exercitado em vez de afirmado.
//!
//! Mesma abordagem de `arkhe-orcid/tests/mock_server.rs`.
//!
//! **Nenhuma requisição sai desta máquina.** O único endereço contactado é o
//! `127.0.0.1` efêmero que o mockito abre, e a instância pública do Sigstore
//! nunca é consultada — inclusive porque a URL base é justamente o que estes
//! testes variam.
//!
//! # Uma armadilha do mockito, registrada
//!
//! Sem `match_query`, o mockito casa o **caminho e a consulta inteiros** como
//! uma string exata (`PathAndQueryMatcher::Unified` com `Matcher::Exact`). Como
//! toda requisição desta crate carrega consulta (`?logIndex=…`,
//! `?firstSize=…`), um mock declarado só com o caminho **nunca casa** — e a
//! resposta que chega é um `501 Not Implemented` do mockito, que na primeira
//! leitura parece um erro do cliente. Os testes abaixo ou casam a consulta
//! explicitamente, ou declaram `Matcher::Any`; nenhum depende do padrão.

use arkhe_verify::encoding::encode_hex;
use arkhe_verify::merkle_root_from_leaves;
use arkhe_verify::{ConsistencyProof, RekorClient, RekorError};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ct_merkle::mem_backed_tree::MemoryBackedTree;
use mockito::Matcher;
use sha2::Sha256;

/// Um corpo de resposta de `/api/v1/log/entries` com uma prova de inclusão
/// **de verdade**: 5 folhas, a folha 2 provada, montada com o mesmo pin
/// (`ct-merkle` 0.3.0) que o core usa.
struct Fixture {
    body_b64: String,
    root_hex: String,
    leaf_index: u64,
    tree_size: u64,
    hashes: Vec<String>,
}

fn fixture() -> Fixture {
    let leaves: Vec<Vec<u8>> = (0..5)
        .map(|index| format!("artifact-{index}").into_bytes())
        .collect();
    let leaf_index = 2usize;

    let mut tree = MemoryBackedTree::<Sha256, Vec<u8>>::new();
    for leaf in &leaves {
        tree.push(leaf.clone());
    }
    let proof = tree.prove_inclusion(leaf_index);

    Fixture {
        body_b64: BASE64.encode(&leaves[leaf_index]),
        root_hex: encode_hex(&merkle_root_from_leaves(&leaves)),
        leaf_index: leaf_index as u64,
        tree_size: leaves.len() as u64,
        hashes: proof.as_bytes().chunks(32).map(encode_hex).collect(),
    }
}

/// O corpo que o Rekor devolve: **um mapa** de UUID para entrada.
fn entries_body(fixture: &Fixture) -> String {
    serde_json::json!({
        "24296fb24b8ad77a0d4e1a3f6c1c9f2e5b8a7d6c4e3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f": {
            "logIndex": fixture.leaf_index,
            "body": fixture.body_b64,
            "integratedTime": 1_700_000_000u64,
            "logID": "c0d23d6ad406973f9559f3ba2d1ca01f84147d8ffc5b8445c224f98b9591801d",
            "verification": {
                "inclusionProof": {
                    "logIndex": fixture.leaf_index,
                    "rootHash": fixture.root_hex,
                    "treeSize": fixture.tree_size,
                    "hashes": fixture.hashes,
                    "checkpoint": "rekor.sigstore.dev - 26057336\n5\ncm9vdA==\n"
                },
                "signedEntryTimestamp": "c2lnbmVk"
            }
        }
    })
    .to_string()
}

// --- /api/v1/log/entries -----------------------------------------------

#[tokio::test]
async fn a_log_entry_is_fetched_from_the_indexed_endpoint_and_verifies_for_inclusion() {
    let fixture = fixture();
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v1/log/entries")
        .match_query(Matcher::UrlEncoded("logIndex".into(), "2".into()))
        .match_header("accept", "application/json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(entries_body(&fixture))
        .create_async()
        .await;

    let entry = RekorClient::with_base_url(server.url())
        .log_entry(2)
        .await
        .expect("a entrada é devolvida");

    assert_eq!(entry.log_index, 2);
    assert_eq!(entry.integrated_time, 1_700_000_000);
    assert_eq!(entry.body, fixture.body_b64);
    assert_eq!(
        entry
            .verification
            .as_ref()
            .and_then(|v| v.signed_entry_timestamp.as_deref()),
        Some("c2lnbmVk")
    );

    // O caminho Rekor → modelo → core, de ponta a ponta: a prova de inclusão
    // que veio no corpo da resposta verifica contra a raiz que veio no mesmo
    // corpo, pelo core do `arkhe-verify-wasm`.
    let report = entry.inclusion_report().expect("há prova para verificar");
    assert!(report.ok, "erro: {:?}", report.error);
    assert_eq!(report.leaf_index, fixture.leaf_index);
    assert_eq!(report.tree_size, fixture.tree_size);
    assert_eq!(report.root_hex, fixture.root_hex);

    // O mock só casa o caminho exato, o `logIndex` e o `Accept`, então chegar
    // às asserções acima já prova a forma da requisição; isto a fixa.
    mock.assert_async().await;
}

#[tokio::test]
async fn a_404_is_mapped_to_not_found_with_the_requested_index() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/entries")
        .match_query(Matcher::UrlEncoded("logIndex".into(), "7".into()))
        .with_status(404)
        .create_async()
        .await;

    assert_eq!(
        RekorClient::with_base_url(server.url()).log_entry(7).await,
        Err(RekorError::NotFound { log_index: 7 })
    );
}

#[tokio::test]
async fn a_429_is_mapped_to_rate_limited() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/entries")
        .match_query(Matcher::Any)
        .with_status(429)
        .create_async()
        .await;

    assert_eq!(
        RekorClient::with_base_url(server.url()).log_entry(0).await,
        Err(RekorError::RateLimited)
    );
}

#[tokio::test]
async fn a_5xx_is_mapped_to_provider_with_the_status() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/entries")
        .match_query(Matcher::Any)
        .with_status(503)
        .create_async()
        .await;

    assert_eq!(
        RekorClient::with_base_url(server.url()).log_entry(0).await,
        Err(RekorError::Provider { status: 503 })
    );
}

#[tokio::test]
async fn an_unexpected_status_is_reported_with_the_url() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/entries")
        .match_query(Matcher::Any)
        .with_status(418)
        .create_async()
        .await;

    let error = RekorClient::with_base_url(server.url())
        .log_entry(0)
        .await
        .expect_err("418 não é 2xx");
    match error {
        RekorError::UnexpectedResponse { reason } => {
            assert!(reason.contains("418"), "razão: {reason}");
            assert!(reason.contains("/api/v1/log/entries"), "razão: {reason}");
        }
        other => panic!("esperava UnexpectedResponse, veio {other:?}"),
    }
}

#[tokio::test]
async fn a_body_that_is_not_json_is_a_malformed_log_entry() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/entries")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("isto não é json")
        .create_async()
        .await;

    assert!(matches!(
        RekorClient::with_base_url(server.url()).log_entry(0).await,
        Err(RekorError::MalformedLogEntry { .. })
    ));
}

#[tokio::test]
async fn an_empty_map_is_rejected_rather_than_treated_as_an_entry() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/entries")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("{}")
        .create_async()
        .await;

    assert!(matches!(
        RekorClient::with_base_url(server.url()).log_entry(0).await,
        Err(RekorError::MalformedLogEntry { .. })
    ));
}

#[tokio::test]
async fn a_map_with_two_entries_is_rejected_rather_than_arbitrarily_picking_one() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/entries")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"uuid-a":{"logIndex":0,"body":"","integratedTime":1,"logID":"aa"},
                "uuid-b":{"logIndex":1,"body":"","integratedTime":2,"logID":"bb"}}"#,
        )
        .create_async()
        .await;

    assert!(matches!(
        RekorClient::with_base_url(server.url()).log_entry(0).await,
        Err(RekorError::MalformedLogEntry { .. })
    ));
}

// --- /api/v1/log/proof -------------------------------------------------

#[tokio::test]
async fn a_consistency_proof_is_fetched_with_the_three_sizes() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/proof")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("firstSize".into(), "10".into()),
            Matcher::UrlEncoded("lastSize".into(), "20".into()),
            Matcher::UrlEncoded("treeSize".into(), "30".into()),
        ]))
        .match_header("accept", "application/json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"rootHash":"ab","hashes":["cd","ef"]}"#)
        .create_async()
        .await;

    let proof: ConsistencyProof = RekorClient::with_base_url(server.url())
        .consistency_proof(10, 20, 30)
        .await
        .expect("a prova é devolvida");

    assert_eq!(
        proof,
        ConsistencyProof {
            root_hash: "ab".to_string(),
            hashes: vec!["cd".to_string(), "ef".to_string()],
        }
    );
}

#[tokio::test]
async fn a_404_on_the_proof_endpoint_is_not_a_not_found_of_a_log_index() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/proof")
        .match_query(Matcher::Any)
        .with_status(404)
        .create_async()
        .await;

    // `NotFound` carrega um `log_index`; uma prova de consistência não tem um,
    // então o 404 aqui é uma resposta inesperada — não um índice que não existe.
    assert!(matches!(
        RekorClient::with_base_url(server.url())
            .consistency_proof(1, 2, 3)
            .await,
        Err(RekorError::UnexpectedResponse { .. })
    ));
}

#[tokio::test]
async fn a_proof_body_that_is_not_json_is_an_unexpected_response() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v1/log/proof")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("não é json")
        .create_async()
        .await;

    assert!(matches!(
        RekorClient::with_base_url(server.url())
            .consistency_proof(1, 2, 3)
            .await,
        Err(RekorError::UnexpectedResponse { .. })
    ));
}
