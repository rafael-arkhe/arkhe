//! §2.1 — `verify_inclusion`: prova de inclusão Merkle segundo o RFC 6962.
//!
//! A construção da árvore e das provas usa `ct-merkle` 0.3.0, o pin do plano.
//! Note-se que **este é o único ponto do crate que depende de `ct-merkle`**:
//! a verificação e a construção da raiz são as duas únicas operações de
//! árvore de que o verificador precisa, e ambas cabem neste módulo.
//!
//! Os testes deste módulo não confiam no `ct-merkle` para decidir o que é
//! correto: eles reconstroem a raiz com uma implementação independente do
//! RFC 6962 §2.1 escrita no próprio módulo de teste (prefixos `0x00`/`0x01`,
//! promoção do nó ímpar) e conferem que as duas concordam.

use ct_merkle::mem_backed_tree::MemoryBackedTree;
use ct_merkle::{InclusionProof, RootHash};
use sha2::Sha256;

use crate::encoding::decode_fixed_hex;
use crate::error::VerifyError;
use crate::hash::SHA256_LEN;

/// Confere uma prova de inclusão contra uma raiz conhecida.
///
/// `proof` é a concatenação crua dos digests irmãos (n × 32 bytes), que é o
/// formato de `InclusionProof::as_bytes()` / `try_from_bytes` do `ct-merkle`.
/// O `leaf_index` é 0-based e `tree_size` é o número de folhas da árvore **a
/// que a raiz pertence** — o `num_leaves` que acompanha a raiz.
///
/// Uma raiz de árvore vazia nunca tem prova válida, e uma prova de
/// comprimento que não seja múltiplo de 32 é rejeitada antes de qualquer
/// hashing.
pub fn verify_inclusion_inner(
    leaf: &[u8],
    leaf_index: u64,
    tree_size: u64,
    proof: &[u8],
    root_hex: &str,
) -> Result<(), VerifyError> {
    if !proof.len().is_multiple_of(SHA256_LEN) {
        return Err(VerifyError::InvalidInclusionProof(format!(
            "comprimento da prova ({}) não é múltiplo de {SHA256_LEN}",
            proof.len()
        )));
    }
    let root_bytes = decode_fixed_hex::<SHA256_LEN>(root_hex)?;

    let proof = InclusionProof::<Sha256>::try_from_bytes(proof.to_vec())
        .map_err(|err| VerifyError::InvalidInclusionProof(err.to_string()))?;
    let root = RootHash::<Sha256>::new(root_bytes.into(), tree_size);

    root.verify_inclusion(&leaf.to_vec(), leaf_index, &proof)
        .map_err(|err| VerifyError::InvalidInclusionProof(err.to_string()))
}

/// Raiz Merkle RFC 6962 de uma lista de folhas.
///
/// Árvore vazia devolve `SHA-256("")`, como manda o RFC 6962 §2.1
/// (`MTH({}) = SHA-256()`), e não zeros.
///
/// Os nomes `MemoryBackedTree`/`RootHash` vêm do `ct-merkle`. A construção
/// não pode falhar; a única condição de pânico dentro de
/// `MemoryBackedTree::push` é um número de folhas que não caberia em memória,
/// e a única dentro de `root()` é uma árvore malformada — que não é
/// alcançável construindo a árvore aqui, folha a folha, do zero.
pub fn merkle_root_from_leaves(leaves: &[Vec<u8>]) -> [u8; SHA256_LEN] {
    let mut tree = MemoryBackedTree::<Sha256, Vec<u8>>::new();
    for leaf in leaves {
        tree.push(leaf.clone());
    }
    let mut out = [0u8; SHA256_LEN];
    out.copy_from_slice(tree.root().as_bytes().as_slice());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{decode_hex, encode_hex};
    use sha2::{Digest, Sha256};

    /// Implementação independente do RFC 6962 §2.1, escrita só com `sha2`.
    ///
    /// Serve de oráculo: se o `ct-merkle` divergisse do RFC, os testes abaixo
    /// pegariam, porque comparam duas implementações escritas de forma
    /// independente em vez de afirmar um valor literal que eu teria copiado
    /// do próprio `ct-merkle`.
    mod rfc6962 {
        use super::Sha256;
        use sha2::Digest;

        fn leaf_hash(leaf: &[u8]) -> [u8; 32] {
            let mut hasher = Sha256::new();
            hasher.update([0x00]);
            hasher.update(leaf);
            hasher.finalize().into()
        }

        fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
            let mut hasher = Sha256::new();
            hasher.update([0x01]);
            hasher.update(left);
            hasher.update(right);
            hasher.finalize().into()
        }

        /// `MTH` do RFC 6962 §2.1.
        pub fn mth(leaves: &[Vec<u8>]) -> [u8; 32] {
            match leaves.len() {
                0 => Sha256::digest(b"").into(),
                1 => leaf_hash(&leaves[0]),
                n => {
                    // k = maior potência de 2 estritamente menor que n
                    let k = n.next_power_of_two() / 2;
                    node_hash(&mth(&leaves[..k]), &mth(&leaves[k..]))
                }
            }
        }
    }

    const EMPTY_TREE_ROOT: &str =
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn leaves(n: usize) -> Vec<Vec<u8>> {
        (0..n).map(|i| format!("leaf-{i}").into_bytes()).collect()
    }

    fn build(leaves: &[Vec<u8>]) -> MemoryBackedTree<Sha256, Vec<u8>> {
        let mut tree = MemoryBackedTree::<Sha256, Vec<u8>>::new();
        for leaf in leaves {
            tree.push(leaf.clone());
        }
        tree
    }

    // --- raiz: ct-merkle vs. RFC 6962 independente ------------------------

    #[test]
    fn empty_tree_root_matches_the_rfc_constant() {
        assert_eq!(encode_hex(&merkle_root_from_leaves(&[])), EMPTY_TREE_ROOT);
        let rfc_empty: [u8; 32] = Sha256::digest(b"").into();
        assert_eq!(rfc6962::mth(&[]), rfc_empty);
    }

    #[test]
    fn root_matches_the_independent_rfc6962_oracle_for_various_sizes() {
        // 1, 2, 3 e 5 folhas cobrem o caso ímpar, o nó promovido sem par, e a
        // divisão em k = maior potência de 2 menor que n.
        for n in [1usize, 2, 3, 4, 5, 7, 8, 9] {
            let batch = leaves(n);
            assert_eq!(
                encode_hex(&merkle_root_from_leaves(&batch)),
                encode_hex(&rfc6962::mth(&batch)),
                "divergência do RFC 6962 com {n} folhas"
            );
        }
    }

    #[test]
    fn single_leaf_root_is_the_prefixed_leaf_hash() {
        let batch = leaves(1);
        let mut hasher = Sha256::new();
        hasher.update([0x00]);
        hasher.update(&batch[0]);
        let expected: [u8; 32] = hasher.finalize().into();
        assert_eq!(merkle_root_from_leaves(&batch), expected);
    }

    // --- vetores conhecidos do RFC 6962 -----------------------------------

    /// O conjunto de folhas dos vetores de teste padrão do RFC 6962
    /// (§2.1.3 / `certificate-transparency-go`), cujos MTH aparecem como
    /// constantes em [`RFC6962_MTH`].
    fn rfc6962_leaves() -> Vec<Vec<u8>> {
        vec![
            vec![],
            vec![0x00],
            vec![0x10],
            vec![0x20, 0x21],
            vec![0x30, 0x31],
            vec![0x40, 0x41, 0x42, 0x43],
            vec![0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57],
            vec![
                0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d,
                0x6e, 0x6f,
            ],
        ]
    }

    /// `MTH(D[n])` para este conjunto de folhas, de n = 1 a 8.
    ///
    /// Estes valores **não** foram copiados do `ct-merkle` nem da minha
    /// memória: foram calculados de forma independente em Python
    /// (`hashlib.sha256` com os prefixos `0x00`/`0x01`) e conferidos contra a
    /// implementação independente em [`rfc6962`]. É um vetor conhecido
    /// externo, não uma tautologia sobre o código sob teste.
    const RFC6962_MTH: [&str; 8] = [
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d",
        "fac54203e7cc696cf0dfcb42c92a1d9dbaf70ad9e621f4bd8d98662f00e3c125",
        "aeb6bcfe274b70a14fb067a5e5578264db0fa9b51af5e0ba159158f329e06e77",
        "d37ee418976dd95753c1c73862b9398fa2a2cf9b4ff0fdfe8b30cd95209614b7",
        "4e3bbb1f7b478dcfe71fb631631519a3bca12c9aefca1612bfce4c13a86264d4",
        "76e67dadbcdf1e10e1b74ddc608abd2f98dfb16fbce75277b5232a127f2087ef",
        "ddb89be403809e325750d3d263cd78929c2942b7942a34b77e122c9594a74c8c",
        "5dc9da79a70659a9ad559cb701ded9a2ab9d823aad2f4960cfe370eff4604328",
    ];

    #[test]
    fn known_answer_roots_match_the_rfc6962_test_vectors() {
        let batch = rfc6962_leaves();
        for (index, expected) in RFC6962_MTH.iter().enumerate() {
            let n = index + 1;
            assert_eq!(
                &encode_hex(&merkle_root_from_leaves(&batch[..n])),
                expected,
                "MTH das primeiras {n} folhas não bate com o vetor do RFC 6962"
            );
        }
    }

    #[test]
    fn known_answer_proofs_verify_against_the_rfc6962_test_vector_roots() {
        let batch = rfc6962_leaves();
        for (index, expected_root) in RFC6962_MTH.iter().enumerate() {
            let n = index + 1;
            let slice = &batch[..n];
            let tree = build(slice);
            for (leaf_index, leaf) in slice.iter().enumerate() {
                let proof = tree.prove_inclusion(leaf_index).as_bytes().to_vec();
                assert_eq!(
                    verify_inclusion_inner(leaf, leaf_index as u64, n as u64, &proof, expected_root),
                    Ok(()),
                    "folha {leaf_index} de {n} não prova inclusão no vetor do RFC 6962"
                );
            }
        }
    }

    #[test]
    fn the_empty_leaf_has_a_known_leaf_hash() {
        // SHA-256(0x00) — a folha vazia do conjunto de teste do RFC 6962.
        // O erro que este teste teria pego: confundir o hash da folha vazia
        // (6e340b9c...) com o hash da folha "abc".
        assert_eq!(
            encode_hex(&merkle_root_from_leaves(&[Vec::new()])),
            "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
        );
        // SHA-256(0x00 ∥ "abc") — valor conferido independentemente em Python.
        assert_eq!(
            encode_hex(&merkle_root_from_leaves(&[b"abc".to_vec()])),
            "609f6e36d2405585188d5cfd761f407c7cc46a7d3f314c88270469dde315fcd1"
        );
    }

    #[test]
    fn tree_size_is_part_of_the_root_identity() {
        // A raiz de 2 folhas não é a raiz de 3: o `num_leaves` acompanha a
        // raiz, e uma prova de um tamanho não vale para o outro.
        assert_ne!(
            merkle_root_from_leaves(&leaves(2)),
            merkle_root_from_leaves(&leaves(3))
        );
    }

    // --- verify_inclusion: reconstrução da raiz ---------------------------

    fn proof_for(batch: &[Vec<u8>], index: usize) -> (Vec<u8>, String, u64) {
        let tree = build(batch);
        let proof = tree.prove_inclusion(index);
        let root = encode_hex(tree.root().as_bytes().as_slice());
        (proof.as_bytes().to_vec(), root, tree.len())
    }

    #[test]
    fn every_leaf_of_every_size_reconstructs_the_root() {
        for n in [1usize, 2, 3, 4, 5, 8, 9] {
            let batch = leaves(n);
            for index in 0..n {
                let (proof, root, size) = proof_for(&batch, index);
                assert_eq!(
                    verify_inclusion_inner(&batch[index], index as u64, size, &proof, &root),
                    Ok(()),
                    "folha {index} de {n} deveria provar inclusão"
                );
            }
        }
    }

    #[test]
    fn a_leaf_that_is_not_in_the_tree_is_rejected() {
        let batch = leaves(5);
        let (proof, root, size) = proof_for(&batch, 2);
        assert_eq!(
            verify_inclusion_inner(b"leaf-impostora", 2, size, &proof, &root),
            Err(VerifyError::InvalidInclusionProof(
                "this root hash doesn't match the proof's root hash".to_string()
            ))
        );
    }

    #[test]
    fn the_right_leaf_at_the_wrong_index_is_rejected() {
        let batch = leaves(5);
        let (proof, root, size) = proof_for(&batch, 2);
        assert!(verify_inclusion_inner(&batch[2], 3, size, &proof, &root).is_err());
    }

    #[test]
    fn a_proof_against_the_wrong_root_is_rejected() {
        let batch = leaves(5);
        let (proof, _root, size) = proof_for(&batch, 1);
        let other_root = encode_hex(&merkle_root_from_leaves(&leaves(6)));
        assert!(verify_inclusion_inner(&batch[1], 1, size, &proof, &other_root).is_err());
    }

    #[test]
    fn claiming_a_different_tree_size_than_the_proofs_origin_is_rejected() {
        // Com um tamanho declarado **menor** do que o real, o índice deixa de
        // existir e a prova é rejeitada.
        let batch = leaves(5);
        let (proof, root, _size) = proof_for(&batch, 4);
        assert!(verify_inclusion_inner(&batch[4], 4, 3, &proof, &root).is_err());
    }

    #[test]
    fn an_inclusion_proof_does_not_by_itself_pin_the_tree_size() {
        // Propriedade documentada do RFC 6962 (e do `RootHash::verify_inclusion`
        // do `ct-merkle`): uma prova de inclusão pode verificar para mais de um
        // tamanho de árvore. Aqui, a prova da folha 0 de uma árvore de 5 folhas
        // também verifica declarando 6.
        //
        // Isto **não** é um furo na atestação: é exatamente por isso que o
        // `tree_size` entra no subject assinado (§ `attestation`). A prova
        // sozinha não amarra o tamanho; a assinatura sobre o subject amarra.
        let batch = leaves(5);
        let (proof, root, size) = proof_for(&batch, 0);
        assert_eq!(size, 5);
        assert_eq!(
            verify_inclusion_inner(&batch[0], 0, size, &proof, &root),
            Ok(())
        );
        assert_eq!(
            verify_inclusion_inner(&batch[0], 0, 6, &proof, &root),
            Ok(()),
            "tamanho 6 também reconstrói esta raiz para a folha 0 — \
             é o subject assinado que fixa o tamanho, não a prova"
        );
    }

    #[test]
    fn an_empty_tree_root_has_no_valid_proof() {
        // `ct-merkle` checa o índice antes do vazio, e com 0 folhas todo
        // índice está fora do alcance — então o erro observável é
        // `IndexOutOfRange`, e o `TreeEmpty` da biblioteca é inalcançável por
        // este caminho. O que importa é que a prova não é aceita.
        assert_eq!(
            verify_inclusion_inner(b"", 0, 0, &[], EMPTY_TREE_ROOT),
            Err(VerifyError::InvalidInclusionProof(
                "the index of the leaf being verified exceeds the number of leaves in the tree"
                    .to_string()
            ))
        );
    }

    #[test]
    fn an_index_outside_the_tree_is_rejected() {
        let batch = leaves(3);
        let (proof, root, size) = proof_for(&batch, 0);
        assert_eq!(
            verify_inclusion_inner(&batch[0], 99, size, &proof, &root),
            Err(VerifyError::InvalidInclusionProof(
                "the index of the leaf being verified exceeds the number of leaves in the tree"
                    .to_string()
            ))
        );
    }

    #[test]
    fn a_malformed_proof_length_is_rejected_before_hashing() {
        let batch = leaves(4);
        let (mut proof, root, size) = proof_for(&batch, 0);
        proof.push(0xFF); // deixa de ser múltiplo de 32
        assert!(matches!(
            verify_inclusion_inner(&batch[0], 0, size, &proof, &root),
            Err(VerifyError::InvalidInclusionProof(_))
        ));
    }

    #[test]
    fn a_single_leaf_tree_verifies_with_an_empty_proof() {
        let batch = leaves(1);
        let (proof, root, size) = proof_for(&batch, 0);
        assert!(proof.is_empty());
        assert_eq!(
            verify_inclusion_inner(&batch[0], 0, size, &proof, &root),
            Ok(())
        );
    }

    #[test]
    fn rejects_a_malformed_root_hex() {
        let batch = leaves(2);
        let (proof, _root, size) = proof_for(&batch, 0);
        assert!(matches!(
            verify_inclusion_inner(&batch[0], 0, size, &proof, "não é hex"),
            Err(VerifyError::InvalidHex(_))
        ));
    }

    #[test]
    fn a_proof_is_the_concatenation_of_32_byte_digests() {
        let batch = leaves(6);
        let (proof, _root, _size) = proof_for(&batch, 4);
        assert_eq!(proof.len() % 32, 0);
        assert!(!proof.is_empty());
        // O formato consumido por `verify_inclusion_inner` é exatamente o que
        // o produtor da prova publica — confere o round-trip.
        assert_eq!(decode_hex(&encode_hex(&proof)).expect("decodifica"), proof);
    }
}
