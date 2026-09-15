// @vitest-environment node
/**
 * Integração WASM (Fase 4) — o teste que prova que o alvo `web` do
 * `arkhe-verify-wasm` carrega e devolve um veredito **real**.
 *
 * Duas coisas tornam isto uma prova e não uma tautologia:
 *
 * 1. O módulo é inicializado **pelos bytes** lidos do `pkg-web` (`initSync`),
 *    pelo **mesmo** `verify-adapter` que a app usa — não há mock nem cópia da
 *    lógica de verificação no lado do JS.
 * 2. A atestação de teste é construída aqui, em JavaScript, com oráculos
 *    independentes do Rust: SHA-256 e Ed25519 vêm de `node:crypto` e a árvore
 *    Merkle segue o RFC 6962 §2.1/§2.1.1 implementado à mão. O WASM concordar
 *    com isto é concordância entre implementações, não consigo mesmo.
 *
 * `@vitest-environment node`: o ambiente jsdom substitui o `import.meta.url`
 * por um URL `http://`, e este ficheiro precisa de `file:` para localizar o
 * `.wasm` no disco (mesmo motivo que `test/contrast.test.ts`).
 */
import { createHash, createPrivateKey, createPublicKey, sign as signEd25519, type KeyObject } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { initVerifyFromBytes } from './verify-adapter';

// ---------------------------------------------------------------------------
// O artefacto: `pkg-web` (wasm-bindgen --target web), gerado a partir do crate
// ---------------------------------------------------------------------------
const WASM_PATH = fileURLToPath(
  new URL(
    '../../../safe-core-monorepo/crates/arkhe-verify-wasm/pkg-web/arkhe_verify_wasm_bg.wasm',
    import.meta.url,
  ),
);
const wasmBytes = readFileSync(WASM_PATH);

/** A app e o teste usam a mesma instância: `initSync` é idempotente. */
const verify = initVerifyFromBytes(wasmBytes);

// ---------------------------------------------------------------------------
// Oráculo independente — node:crypto + RFC 6962 escrito à mão
// ---------------------------------------------------------------------------
const sha256 = (bytes: Uint8Array): Buffer => createHash('sha256').update(bytes).digest();
const hex = (bytes: Uint8Array): string => Buffer.from(bytes).toString('hex');
const cat = (...parts: Uint8Array[]): Buffer =>
  Buffer.concat(parts.map((part) => Buffer.from(part)));

// MTH({}) = SHA-256(); MTH({d0}) = SHA-256(0x00 ∥ d0);
// MTH(D[n]) = SHA-256(0x01 ∥ MTH(D[0:k]) ∥ MTH(D[k:n])), k = maior potência de 2 < n
const leafHash = (data: Uint8Array): Buffer => sha256(cat(Buffer.from([0x00]), data));
const nodeHash = (left: Uint8Array, right: Uint8Array): Buffer =>
  sha256(cat(Buffer.from([0x01]), left, right));

function largestPowerOfTwoBelow(n: number): number {
  let k = 1;
  while (k * 2 < n) k *= 2;
  return k;
}

function merkleTreeHash(leaves: readonly Uint8Array[]): Buffer {
  if (leaves.length === 0) return sha256(Buffer.alloc(0));
  if (leaves.length === 1) return leafHash(leaves[0]);
  const k = largestPowerOfTwoBelow(leaves.length);
  return nodeHash(merkleTreeHash(leaves.slice(0, k)), merkleTreeHash(leaves.slice(k)));
}

/** PATH(m, D[n]) — RFC 6962 §2.1.1, de baixo para cima. */
function merklePath(index: number, leaves: readonly Uint8Array[]): Buffer[] {
  if (leaves.length === 1) return [];
  const k = largestPowerOfTwoBelow(leaves.length);
  if (index < k) return [...merklePath(index, leaves.slice(0, k)), merkleTreeHash(leaves.slice(k))];
  return [...merklePath(index - k, leaves.slice(k)), merkleTreeHash(leaves.slice(0, k))];
}

// Chaves Ed25519 determinísticas a partir de uma semente: PKCS#8 na entrada,
// SPKI na saída, para não depender de RNG nem de nada do crate.
const PKCS8_PREFIX = Buffer.from('302e020100300506032b657004220420', 'hex');

function keypairFromSeed(seed: number): { privateKey: KeyObject; publicKeyHex: string } {
  const privateKey = createPrivateKey({
    key: cat(PKCS8_PREFIX, Buffer.alloc(32, seed)),
    format: 'der',
    type: 'pkcs8',
  });
  const spki = createPublicKey(privateKey).export({ format: 'der', type: 'spki' }) as Buffer;
  return { privateKey, publicKeyHex: hex(spki.subarray(spki.length - 32)) };
}

const be64 = (value: bigint): Buffer => {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64BE(value);
  return buffer;
};

// ---------------------------------------------------------------------------
// A atestação de teste (fixture) — construída inteiramente do lado do JS
// ---------------------------------------------------------------------------
const LEAF_INDEX = 2n;
const TREE_SIZE = 5n;
const leaves = [0, 1, 2, 3, 4].map((i) => Buffer.from(`artifact-${i}`));
const root = merkleTreeHash(leaves);
const proof = Buffer.concat(merklePath(Number(LEAF_INDEX), leaves));
const payload = leaves[Number(LEAF_INDEX)];
const payloadDigest = sha256(payload);

// subject = "arkhe-attestation/v1" ∥ SHA-256(payload) ∥ root ∥ leaf_index ∥ tree_size
const subject = cat(
  Buffer.from('arkhe-attestation/v1'),
  payloadDigest,
  root,
  be64(LEAF_INDEX),
  be64(TREE_SIZE),
);

const signer = keypairFromSeed(1);
const witnesses = [keypairFromSeed(2), keypairFromSeed(3)];
const trustedKeys = [signer, ...witnesses].map((keypair) => keypair.publicKeyHex);
const trustRootJson = JSON.stringify(trustedKeys);

const attestation = {
  payload_b64: payload.toString('base64'),
  payload_sha256_hex: hex(payloadDigest),
  merkle_leaf_index: Number(LEAF_INDEX),
  merkle_tree_size: Number(TREE_SIZE),
  merkle_proof_hex: hex(proof),
  merkle_root_hex: hex(root),
  signer_public_key_hex: signer.publicKeyHex,
  signature_hex: hex(signEd25519(null, subject, signer.privateKey)),
  witnesses: witnesses.map((keypair) => ({
    public_key_hex: keypair.publicKeyHex,
    signature_hex: hex(signEd25519(null, subject, keypair.privateKey)),
  })),
  quorum_threshold: 2,
};
const attestationJson = JSON.stringify(attestation);

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------
describe('verify-adapter sobre o alvo `web` do arkhe-verify-wasm', () => {
  it('carrega o artefacto do pkg-web: bytes com o magic do WebAssembly', () => {
    expect(wasmBytes.length).toBeGreaterThan(0);
    expect([...wasmBytes.subarray(0, 4)]).toEqual([0x00, 0x61, 0x73, 0x6d]); // "\0asm"
  });

  it('POSITIVO: atestação válida -> ok, com os quatro estágios a concordar', () => {
    expect(verify.verifyAttestation(attestationJson, trustRootJson)).toEqual({
      ok: true,
      sha256: true,
      signature: true,
      inclusion: true,
      quorum: true,
      error: null,
    });
  });

  it('NEGATIVO (assinatura): assinatura zerada -> só o estágio `signature` falha', () => {
    const report = verify.verifyAttestation(
      JSON.stringify({ ...attestation, signature_hex: '00'.repeat(64) }),
      trustRootJson,
    );

    expect(report).toMatchObject({
      ok: false,
      sha256: true,
      signature: false,
      inclusion: true,
      quorum: true,
    });
    expect(report.error).toBeTypeOf('string');
  });

  it('NEGATIVO (quórum): limiar 3 com 2 testemunhas -> só o estágio `quorum` falha', () => {
    const report = verify.verifyAttestation(
      JSON.stringify({ ...attestation, quorum_threshold: 3 }),
      trustRootJson,
    );

    expect(report).toMatchObject({
      ok: false,
      sha256: true,
      signature: true,
      inclusion: true,
      quorum: false,
    });
  });

  it('NEGATIVO (inclusão): prova adulterada -> só o estágio `inclusion` falha', () => {
    const tampered = Buffer.from(proof);
    tampered[0] ^= 0x01;

    const report = verify.verifyAttestation(
      JSON.stringify({ ...attestation, merkle_proof_hex: hex(tampered) }),
      trustRootJson,
    );

    expect(report).toMatchObject({
      ok: false,
      sha256: true,
      signature: true,
      inclusion: false,
      quorum: true,
    });
  });

  it('entrada não confiável devolve um relatório, não uma exceção', () => {
    expect(() => verify.verifyAttestation('não é json', trustRootJson)).not.toThrow();

    const report = verify.verifyAttestation('não é json', trustRootJson);
    expect(report.ok).toBe(false);
    expect(report.error).toBeTypeOf('string');

    // Um trust root que não se interpreta não confia em nada.
    expect(verify.verifyAttestation(attestationJson, 'não é json')).toMatchObject({
      ok: false,
      sha256: false,
      signature: false,
      inclusion: false,
      quorum: false,
    });
  });
});
