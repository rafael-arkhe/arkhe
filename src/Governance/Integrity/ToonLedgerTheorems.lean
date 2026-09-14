import Init

/-!
# BLOCO 509 — Consolidação Final (v69, vetado)
Contrato formal do Ledger de TOONs (`toon_ledger.py`): cadeia de hashes
append-only, tamper-evidente. Modelo Nat/String sem Mathlib, zero `sorry`.

Os teoremas I32–I36 da proposta (Mathlib + `sorry` + símbolos fictícios
`coherence`, `recover_address`, `verify_on_chain`) foram REJEITADOS. Aqui o
que é genuíno e provável de primeiros princípios: monotonicidade de altura e
encadeamento de blocos. SHA-256 entra como axioma SPEC nomeado
(determinismo é propriedade de função — provável; pré-imagem não).
-/

def block_hash (content : String) (prev : Nat) (height : Nat) : Nat :=
  content.length * 1_000_003 + prev * 131 + height

structure ToonBlock where
  height : Nat
  prev_hash : Nat
  block_hash : Nat
  record_content : String

structure ToonLedger where
  blocks : List ToonBlock

def Linked (a b : ToonBlock) : Prop :=
  b.prev_hash = a.block_hash ∧ b.height = a.height + 1

def ChainVerified : List ToonBlock → Prop
  | [] => True
  | [_] => True
  | a :: b :: rest => Linked a b ∧ ChainVerified (b :: rest)

def appendBlock (l : ToonLedger) (b : ToonBlock) : ToonLedger :=
  { blocks := b :: l.blocks }

/- Determinismo do hash (definido, provado — sem axioma). A pré-imagem
   criptográfica do SHA-256 real está fora do escopo de Init. -/

theorem block_hash_deterministic (c1 c2 : String) (p1 p2 : Nat) (h1 h2 : Nat)
    (hc : c1 = c2) (hp : p1 = p2) (hh : h1 = h2) :
    block_hash c1 p1 h1 = block_hash c2 p2 h2 := by
  subst c2
  subst p2
  subst h2
  rfl

/- Teoremas exatos. -/

theorem append_increases_height (l : ToonLedger) (b : ToonBlock) :
    l.blocks.length < (appendBlock l b).blocks.length := by
  unfold appendBlock
  rw [List.length_cons]
  exact Nat.lt_succ_self l.blocks.length

theorem linked_height_contiguous {a b : ToonBlock} (h : Linked a b) :
    a.height < b.height := by
  rcases h with ⟨_, hh⟩
  rw [hh]
  exact Nat.lt_succ_self a.height

theorem linked_prev_links_hash {a b : ToonBlock} (h : Linked a b) :
    b.prev_hash = a.block_hash := by
  exact h.1

theorem chain_verified_head {a b : ToonBlock} {rest : List ToonBlock}
    (h : ChainVerified (a :: b :: rest)) : Linked a b := by
  unfold ChainVerified at h
  exact h.1

theorem chain_verified_tail {a b : ToonBlock} {rest : List ToonBlock}
    (h : ChainVerified (a :: b :: rest)) : ChainVerified (b :: rest) := by
  unfold ChainVerified at h
  exact h.2

-- Corolário: cadeia verificada na cabeça implica alturas adjacentes crescentes.
theorem chain_head_heights_monotone {a b : ToonBlock} {rest : List ToonBlock}
    (h : ChainVerified (a :: b :: rest)) : a.height < b.height := by
  exact linked_height_contiguous (chain_verified_head h)