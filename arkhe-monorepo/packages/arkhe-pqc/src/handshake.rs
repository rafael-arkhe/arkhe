//! Three-way authenticated PQC handshake (initiator / responder / confirm).
//! Built on the authenticated KEM: the responder proves possession of its
//! signing key while establishing a shared secret with the initiator.

use crate::auth_kem::{
    auth_decapsulate, auth_encapsulate, AuthCiphertext, AuthError, PqcIdentity, PqcKeyMaterial,
};
use crate::sign::QuantumSignature;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct Hello {
    pub initiator: PqcIdentity,
    pub nonce: [u8; 32],
    pub version: u32,
}

/// The initiator builds a hello with its identity and a fresh nonce.
pub fn initiator_hello(initiator: &PqcKeyMaterial, version: u32) -> Hello {
    Hello { initiator: initiator.identity.clone(), nonce: nonce_on(&initiator.identity), version }
}

#[derive(Debug, Clone)]
pub struct Confirm {
    pub digest: [u8; 32],
}

// nonce derived from identity to keep tests deterministic; still includes
// transport of the identity so it cannot be replayed across parties.
fn nonce_on(id: &PqcIdentity) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"arkhe-handshake");
    h.update(&id.dsa_public);
    let d = h.finalize();
    let mut n = [0u8; 32];
    n.copy_from_slice(&d[..32]);
    n
}

/// The responder receives the hello, establishes a session with the
/// initiator via authenticated KEM, and acknowledges.
pub fn responder_ack(
    responder: &PqcKeyMaterial,
    hello: &Hello,
) -> Result<(Response, Vec<u8>), AuthError> {
    let label = b"arkhe:3wh1";
    let (encap, ss) = auth_encapsulate(&responder.signer, &hello.initiator, label)?;
    let proof_msg = [&hello.nonce[..], &encap.ciphertext[..]].concat();
    let nonce_proof = responder.signer.sign(&proof_msg);
    let ack = Response {
        responder: responder.identity.clone(),
        encap,
        nonce_proof,
    };
    Ok((ack, ss))
}

#[derive(Debug, Clone)]
pub struct Response {
    pub responder: PqcIdentity,
    pub encap: AuthCiphertext,
    pub nonce_proof: QuantumSignature,
}

/// The initiator verifies the response and finalizes the session.
pub fn initiator_finalize_ss(
    initiator: &PqcKeyMaterial,
    hello: &Hello,
    response: &Response,
) -> Result<Vec<u8>, AuthError> {
    let proof_msg = [&hello.nonce[..], &response.encap.ciphertext[..]].concat();
    crate::sign::quantum_verify(
        &response.responder.dsa_public,
        &proof_msg,
        &response.nonce_proof.signature,
    )
    .map_err(AuthError::Signature)?;
    // The capsule was sent by the responder to the initiator, so the
    // sender identity is the responder, and the initiator decapsulates.
    auth_decapsulate(
        &response.responder,
        &initiator.kem.decapsulation,
        &response.encap,
    )
}

/// Confirm computes a digest of the session secret for both parties.
pub fn confirm_ss(ss: &[u8]) -> Confirm {
    let mut h = Sha256::new();
    h.update(ss);
    let d = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&d[..32]);
    Confirm { digest: out }
}

pub fn verify_confirm(ss: &[u8], c: &Confirm) -> bool {
    confirm_ss(ss).digest == c.digest
}