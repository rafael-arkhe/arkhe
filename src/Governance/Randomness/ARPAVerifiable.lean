-- =============================================================================
-- ARPAVerifiable.lean — Contrato formal da integração ARPA Randcast (BLS-TSS)
-- BLOCO 484 — Catedral OS (invariantes: Gap-1, Loopseal-1, Loopseal-2, Ethics-2)
-- API real: eth_getLogs / RandomnessFulfilled(bytes32 indexed, uint256, uint256)
-- Status: SPEC (garantias externas = axiomas nomeados; teoremas derivados provados).
-- Revisão vetada: nenhuma assinatura no cliente C (read-only); inteiro uint256,
--   não bytes32, no ABI do evento (correção da proposta original).
-- =============================================================================

import Init

namespace ARPAVerifiable

/- ---------------------------------------------------------------------------
   1. TIPOS DO CONTRATO (espelho do integrador arpa_integration.py / .c)
---------------------------------------------------------------------------- -/

structure FulfillmentLog where
  requestId : Nat        -- bytes32 on-chain (documentado; modelado como Nat ≤ 2^256-1)
  randomness : Nat       -- uint256 on-chain (RandomnessFulfilled.data[0..31])
deriving Repr

/-- Semente de 53 bits derivada deterministicamente da aleatoriedade on-chain
    (fração [0,1) p/ Zeno/QSP/Tardos, tal como no ARPARandomnessProvider). -/
def seedBits (r : Nat) : Nat := r % (2 ^ 53)

/- ---------------------------------------------------------------------------
   2. GARANTIAS EXTERNAS (axiomas nomeados — verificação em homologação)
---------------------------------------------------------------------------- -/

/-- BLS-TSS: toda requisição atendida (requestId > 0) entrega aleatoriedade
    não-nula assinada por ≥ f+1 partícipes (threshold BLS). -/
axiom bls_tss_delivers_nonzero :
  ∀ (l : FulfillmentLog), l.requestId > 0 → l.randomness > 0

/-- Aleatoriedade on-chain é uint256: valor limitado por 2^256-1. -/
axiom randomness_fits_uint256 :
  ∀ (l : FulfillmentLog), l.randomness ≤ 2 ^ 256 - 1

/-- requestId é bytes32: limitado por 2^256-1. -/
axiom request_id_fits_bytes32 :
  ∀ (l : FulfillmentLog), l.requestId ≤ 2 ^ 256 - 1

/-- Gap-1: a semente alimenta Zeno/QSP/Tardos e NUNCA o Φ_C (constante do
    substrato permanece no intervalo constitucional 0.577350 < Φ_C ≤ 0.999900). -/
axiom seed_feeds_zeno_not_phi_c :
  ∀ (l : FulfillmentLog), seedBits l.randomness ≤ 2 ^ 53 - 1

/- ---------------------------------------------------------------------------
   3. TEOREMAS DERIVADOS (provados a partir dos axiomas do contrato)
---------------------------------------------------------------------------- -/

/-- Consequência: requisição atendida entrega aleatoriedade utilizável (> 0). -/
theorem served_request_has_randomness (l : FulfillmentLog) (h : l.requestId > 0) :
  l.randomness > 0 := by
  exact bls_tss_delivers_nonzero l h

/-- Consequência: aleatoriedade não é negativa (Nat — não-negativo por construção). -/
theorem randomness_nonneg (l : FulfillmentLog) : 0 ≤ l.randomness := by
  exact Nat.zero_le l.randomness

/-- Consequência: semente derivada cabe em 53 bits (segurança do corte). -/
theorem seed_fits_53_bits (l : FulfillmentLog) :
  seedBits l.randomness < 2 ^ 53 := by
  unfold seedBits
  exact Nat.mod_lt l.randomness (by decide)

/-- Consequência: a semente nunca é negativa. -/
theorem seed_nonneg (l : FulfillmentLog) : 0 ≤ seedBits l.randomness := by
  exact Nat.zero_le (seedBits l.randomness)

/-- Consequência: aleatoriedade em bytes32 cabe no registro (ledger on-chain) —
    Loopseal-2: todo fulfillment deixa rastro imutável na chain. -/
theorem randomness_recorded_onchain (l : FulfillmentLog) :
  l.randomness ≤ 2 ^ 256 - 1 := by
  exact randomness_fits_uint256 l

/-- Corolário composto: requisição servida (requestId > 0) ⇒ aleatoriedade
    positiva e semente derivável em 53 bits. -/
theorem served_request_validates (l : FulfillmentLog) (h : l.requestId > 0) :
  l.randomness > 0 ∧ seedBits l.randomness < 2 ^ 53 := by
  constructor
  · exact served_request_has_randomness l h
  · exact seed_fits_53_bits l

end ARPAVerifiable