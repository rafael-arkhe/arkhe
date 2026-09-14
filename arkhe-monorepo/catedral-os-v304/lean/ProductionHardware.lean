/- ============================================================================
   Catedral OS v304.0 — ProductionHardware.lean
   Invariantes de hardware/produção (I358–I362), núcleo Lean 4 (sem Mathlib).
   ============================================================================ -/

/- Diferença absoluta de Nat (Nat.sub satura em zero). -/
def abs_nat (a b : Nat) : Nat :=
  if a < b then b - a else a - b

/- ============================================================================
   I358 — Leitura de phi do hardware é contínua e confiável:
   cada passo consecutivo de leitura dista menos (ou igual) de δ.
   ============================================================================ -/
def hardware_phi_continuous (readings : List Nat) (δ : Nat) : Prop :=
  ∀ i : Nat, i + 1 < readings.length → abs_nat (readings.getD i 0) (readings.getD (i + 1) 0) ≤ δ

/- ============================================================================
   I359 — Pool de provadores escala linearmente: vazão(n) = n · vazão(1).
   ============================================================================ -/
def prover_pool_scales (throughput : Nat → Nat) : Prop :=
  ∀ n : Nat, n > 0 → throughput n = n * throughput 1

/- ============================================================================
   I360 — Snapshots Raft são restauráveis: todo índice criado restaura.
   ============================================================================ -/
def raft_snapshot_restorable (create : Nat → Bool) (restore : Nat → Bool) : Prop :=
  ∀ idx : Nat, create idx = true → restore idx = true

/- ============================================================================
   I361 — Contratos WASM são isolados e seguros: nenhuma execução insegura
   é admitida no sandbox.
   ============================================================================ -/
def wasm_sandboxed (executes_unsafe : Prop) : Prop :=
  ¬ executes_unsafe

/- ============================================================================
   I362 — Validação de hardware preserva invariantes: se o modelo simulado
   implica todos os invariantes, a validação em hardware também os implica.
   ============================================================================ -/
def hardware_validation_preserves
    (invariants : List Prop) (simulated : Prop) (hardware : Prop) : Prop :=
  (∀ inv ∈ invariants, simulated → inv) → (∀ inv ∈ invariants, hardware → inv)