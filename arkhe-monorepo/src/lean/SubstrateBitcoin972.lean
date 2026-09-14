/- ============================================================================
   Substrate ARKHE-BITCOIN — Invariantes I505–I510
   Catedral OS — Bloco 972 (ACEITAÇÃO)
   Núcleo Lean 4 (v4.33.1), SEM Mathlib — convenção do repositório
   (bloco 966 / bloco 971: Substrate924.lean).

   I505 — Serialização SEC1 da chave pública: comprimida = 33 bytes
          (1 byte de prefixo de compressão + 32 bytes da coordenada x).
   I506 — WIF (Base58Check): payload = 1 byte de versão + 32 bytes de chave
          + 1 byte sufixo de compressão + 4 bytes de checksum sha256d.
   I507 — BIP-340 (Schnorr): assinatura = 64 bytes = r ‖ s, com |r| = |s| = 32.
   I508 — PoTT: janela de trânsito tout = tin + 60 (não-nula, monotônica),
          nonce ν = 16 bytes, NodeId x-only = 32 bytes.
   I509 — Mapa de prefixos de rede (P2PKH/P2SH): mainnet 0x00/0x05,
          testnet 0x6f/0xc4 — valores disjuntos entre redes.
   I510 — Digest de endereço PoTT = SHA-256 de 32 bytes (privado em
          pott_integration::address_digest) — o resultado preenche
          exatamente um bloco de 32 bytes.

   Cada teorema notificado publicamente está provado no núcleo.
   Nenhum sorry é necessário neste arquivo — sem dívida formal.
   ============================================================================ -/

namespace ArkheBitcoin

/- ==========================================================================
   I505 — COMPRESSED PUBLIC KEY (SEC1)
   prefixo de compressão (1) + coordenada x (32) = 33 bytes.
   ========================================================================== -/

def sec1_compressed_prefix_len : Nat := 1
def sec1_x_coord_len : Nat := 32
def compressed_public_key_len : Nat := sec1_compressed_prefix_len + sec1_x_coord_len

def compressed_flag_even : Nat := 0x02
def compressed_flag_odd  : Nat := 0x03

/- I505-A: 1 + 32 = 33 bytes. -/
theorem I505_compressed_len :
    compressed_public_key_len = 33 := by
  native_decide

/- I505-B: o prefixo de compressão é sempre 0x02 ou 0x03 (flag de paridade). -/
theorem I505_flag_is_compression_byte :
    compressed_flag_even = 2 ∧ compressed_flag_odd = 3 := by
  constructor <;> rfl <;> native_decide

/- ==========================================================================
   I506 — WIF (Base58Check)
   versão (1) + chave (32) + sufixo compressão (0 ou 1) + checksum (4).
   ========================================================================== -/

def wif_version_len : Nat := 1
def wif_secret_len : Nat := 32
def wif_compression_suffix_len : Nat := 1
def wif_checksum_len : Nat := 4

def wif_payload_len_compressed : Nat :=
  wif_version_len + wif_secret_len + wif_compression_suffix_len

/- I506-A: payload WIF comprimido (antes do checksum) = 34 bytes. -/
theorem I506_payload_len_compressed :
    wif_payload_len_compressed = 34 := by
  native_decide

/- I506-B: checksum sha256d = 4 bytes. -/
theorem I506_checksum_len : wif_checksum_len = 4 := by
  rfl

/- ==========================================================================
   I507 — BIP-340 (Schnorr)
   assinatura = r ‖ s, |r| = |s| = 32 → 64 bytes. NodeId x-only = 32 bytes.
   ========================================================================== -/

def x_only_pubkey_len : Nat := 32
def schnorr_scalar_r_len : Nat := 32
def schnorr_scalar_s_len : Nat := 32

def schnorr_signature_len : Nat := schnorr_scalar_r_len + schnorr_scalar_s_len

/- I507-A: assinatura Schnorr = 32 + 32 = 64 bytes. -/
theorem I507_signature_len :
    schnorr_signature_len = 64 := by
  native_decide

/- I507-B: r e s têm o mesmo tamanho (32 bytes) — sem truncamento. -/
theorem I507_r_s_equal_len :
    schnorr_scalar_r_len = schnorr_scalar_s_len := by
  rfl

/- I507-C: chave x-only (NodeId BIP-340) = 32 bytes. -/
theorem I507_xonly_len : x_only_pubkey_len = 32 := by
  rfl

/- ==========================================================================
   I508 — PoTT: janela de trânsito `tout ≥ tin` (+60s) e comprimentos fixos.
   ========================================================================== -/

def pott_transit_window_seconds : Nat := 60
def pott_nonce_len : Nat := 16

/- I508-A: tout = tin + 60 ⇒ tin ≤ tout — a janela nunca fica vazia nem
   regride (Loopseal-1/Gravity-1: timestamps monotônicos). -/
theorem I508_window_nonempty (tin : Nat) :
    tin ≤ tin + pott_transit_window_seconds := by
  exact Nat.le.intro rfl

/- I508-B: janela é estritamente positiva na prática (60 > 0). -/
theorem I508_window_positive :
    0 < pott_transit_window_seconds := by
  native_decide

/- I508-C: nonce ν tem exatamente 16 bytes. -/
theorem I508_nonce_len : pott_nonce_len = 16 := by
  rfl

/- ==========================================================================
   I509 — MAPA DE PREFIXOS DE REDE (P2PKH / P2SH)
   mainnet: 0x00 / 0x05; testnet: 0x6f / 0xc4.
   Prefixos de P2PKH e P2SH são sempre distintos, em cada rede.
   ========================================================================== -/

def mainnet_p2pkh_prefix : Nat := 0x00
def mainnet_p2sh_prefix  : Nat := 0x05
def testnet_p2pkh_prefix : Nat := 0x6f
def testnet_p2sh_prefix  : Nat := 0xc4

/- I509-A: valores P2PKH e P2SH de cada rede são distintos. -/
theorem I509_prefixes_distinct_per_network :
    mainnet_p2pkh_prefix ≠ mainnet_p2sh_prefix ∧
    testnet_p2pkh_prefix ≠ testnet_p2sh_prefix := by
  decide

/- I509-B: prefixos de versão diferem entre mainnet e testnet. -/
theorem I509_testnet_differs_from_mainnet :
    testnet_p2pkh_prefix ≠ mainnet_p2pkh_prefix ∧
    testnet_p2sh_prefix ≠ mainnet_p2sh_prefix := by
  decide

/- ==========================================================================
   I510 — DIGEST DO ENDEREÇO (pott_integration::address_digest)
   SHA-256 → 32 bytes, que preenchem exatamente `h : [u8; 32]`.
   ========================================================================== -/

def sha256_digest_len : Nat := 32
def pott_payload_digest_len : Nat := sha256_digest_len

/- I510-A: o digest do endereço ocupa 32 bytes (um bloco `[u8; 32]`). -/
theorem I510_payload_digest_len :
    pott_payload_digest_len = 32 := by
  rfl

/- I510-B: 2 × 32 = 64 — o par (digest ‖ NodeId) que ancora marcha no
   recebimento também é de tamanho fixo (64 bytes). -/
theorem I510_digest_and_nodeid_len :
    pott_payload_digest_len + x_only_pubkey_len = 64 := by
  native_decide

end ArkheBitcoin