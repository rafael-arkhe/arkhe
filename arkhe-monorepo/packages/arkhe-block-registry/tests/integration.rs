//! Integração: formato canónico JSON + invariantes estruturais via API pública.
//!
//! O formato aqui é o mesmo que `tools/generate_test_blocks.py` produz e que
//! `tools/verify-blocks.ps1` valida — o contrato inter-artefacto do v582.0:
//! `hash` é sempre hex string de 64 caracteres.

use arkhe_block_registry::{compute_block_hash, BlockRecord, BlockRegistry, Hash, RegistryError};

fn write_block(dir: &std::path::Path, numero: u32, tipo: &str, parent: Option<&str>) -> String {
    let hash = match parent {
        Some(p) => compute_block_hash(numero, tipo, Some(&Hash::from_hex(p).unwrap())),
        None => compute_block_hash(numero, tipo, None),
    };
    let block = BlockRecord {
        numero,
        tipo: tipo.into(),
        hash,
        parent_hash: parent.map(|p| Hash::from_hex(p).unwrap()),
    };
    let json = serde_json::to_string_pretty(&block).unwrap();
    let name = format!("bloco_{numero:04}.json");
    std::fs::write(dir.join(name), json).unwrap();
    // arquivo deve conter o hash como string hex, nao array
    let raw = std::fs::read_to_string(dir.join(format!("bloco_{numero:04}.json"))).unwrap();
    assert!(raw.contains(&hash.to_hex()), "hash hex presente no JSON");
    assert!(!raw.contains("[1,2,3"), "array binário ausente");
    hash.to_hex()
}

#[test]
fn full_pipeline_valid_chain() {
    let dir = tempfile::tempdir().unwrap();
    let h1 = write_block(dir.path(), 1, "ROOT", None);
    let h2 = write_block(dir.path(), 2, "CHILD", Some(&h1));
    write_block(dir.path(), 3, "GRANDCHILD", Some(&h2));

    let mut reg = BlockRegistry::new();
    for (numero, tipo, parent) in [
        (1u32, "ROOT", None),
        (2, "CHILD", Some(h1.clone())),
        (3, "GRANDCHILD", Some(h2.clone())),
    ] {
        let hash = match &parent {
            Some(p) => compute_block_hash(numero, tipo, Some(&Hash::from_hex(p).unwrap())),
            None => compute_block_hash(numero, tipo, None),
        };
        reg.insert(BlockRecord {
            numero,
            tipo: tipo.into(),
            hash,
            parent_hash: parent.map(|p| Hash::from_hex(&p).unwrap()),
        })
        .unwrap();
    }
    assert_eq!(reg.len(), 3);
    assert!(reg.verify_chain().is_ok());
}

#[test]
fn canonical_json_is_hex_string_roundtrip() {
    let hash = compute_block_hash(42, "ROOT", None);
    let block = BlockRecord {
        numero: 42,
        tipo: "ROOT".into(),
        hash,
        parent_hash: None,
    };
    let json = serde_json::to_string(&block).unwrap();

    // Contrato canónico: { "numero", "tipo", "hash" } com hash como string.
    assert!(json.contains("\"numero\":42"));
    assert!(json.contains("\"tipo\":\"ROOT\""));
    assert!(json.contains(&format!("\"hash\":\"{}\"", hash.to_hex())));

    let parsed: BlockRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, block);
}

#[test]
fn collision_and_reuse_detected_structural_only() {
    let mut reg = BlockRegistry::new();

    let h1 = compute_block_hash(5, "ROOT", None);
    reg.insert(BlockRecord { numero: 5, tipo: "ROOT".into(), hash: h1, parent_hash: None })
        .unwrap();

    // NumberCollision
    let h_collision = compute_block_hash(5, "AUDITORIA", None);
    let err = reg
        .insert(BlockRecord { numero: 5, tipo: "AUDITORIA".into(), hash: h_collision, parent_hash: None })
        .unwrap_err();
    assert_eq!(err, RegistryError::NumberCollision(5));

    // HashReuse
    let err = reg
        .insert(BlockRecord { numero: 6, tipo: "GHOST".into(), hash: h1, parent_hash: None })
        .unwrap_err();
    assert_eq!(err, RegistryError::HashReuse(h1.to_hex()));

    // OrphanParent
    let orphan_parent = compute_block_hash(999, "FAKE", None);
    let err = reg
        .insert(BlockRecord { numero: 7, tipo: "CHILD".into(), hash: compute_block_hash(7, "CHILD", Some(&orphan_parent)), parent_hash: Some(orphan_parent) })
        .unwrap_err();
    assert_eq!(err, RegistryError::OrphanParent(compute_block_hash(999, "FAKE", None).to_hex()));
}