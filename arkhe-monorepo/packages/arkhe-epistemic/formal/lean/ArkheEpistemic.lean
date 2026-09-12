import Lean

/-!
## ArkheEpistemic.lean

Verificação formal de `a(p) ≠ p³ − p²` para p ≥ 3 (em particular para primos
ímpares) — núcleo epistémico do ARKHE (bloco 1060, sequência A051193).

Núcleo Lean 4 core (kernel real v4.33.1, SEM Mathlib, SEM sorry) — convenção
do repositório (blocos 966/971/972/996/1009/1073/1074: kernel real;
`by`/`rfl`/`simp`/`omega`).

HONESTIDADE: prova-se o enunciado aritmético para TODO p ≥ 3
(`3 ≤ p`), do qual a restrição a primos ímpares é corolário imediato
(um primo ímpar satisfaz `p ≠ 2 ∧ IsPrime p`, logo `3 ≤ p`). A definição
`IsPrime` incluída é um subconjunto da definição clássica — suficiente para
derivar `3 ≤ p`; nenhuma afirmação de extensionalidade é feita.
-/

namespace ArkheEpistemic

/-- `a(n) = n² + 1` (entrada A051193 usada na barreira epistémica). -/
def a (n : Nat) : Nat := n^2 + 1

/-- Primalidade (subconjunto): n ≥ 2 e nenhum divisor próprio. -/
def IsPrime (n : Nat) : Prop :=
  2 ≤ n ∧ ∀ d : Nat, 2 ≤ d → d < n → ¬ d ∣ n

/-- Um primo que não é 2 (primo ímpar) satisfaz `3 ≤ p`. -/
theorem prime_odd_ge_three {p : Nat} (hp : IsPrime p) (h2 : p ≠ 2) : 3 ≤ p := by
  rcases hp with ⟨h2le, _⟩
  omega

/-! ## A refutação estrutural: `a(p) ≠ p³ − p²`

Estratégia (apenas `Nat.pow_succ`, `Nat.sub_mul`, `Nat.mul_le_mul_right`,
`Nat.pow_le_pow_left`, `simp`, `omega`):

1. `p³ − p² = (p − 1)·p²`  (factorização por sub-mul).
2. Para p ≥ 3: `p² + 1 < p² + p² ≤ (p − 1)·p²`, logo `a(p) < p³ − p²`.
3. Portanto `a(p) ≠ p³ − p²`.
-/

/-- Factorização: `p³ − p² = (p − 1)·p²` — sub-mul sobre a potência. -/
theorem p3_minus_p2_factor (p : Nat) : p^3 - p^2 = (p - 1) * p^2 := by
  rw [Nat.pow_succ]
  rw [Nat.mul_comm]
  have h3 : p * p^2 - p^2 = p * p^2 - 1 * p^2 := by
    rw [show 1 * p^2 = p^2 by simp]
  rw [h3]
  rw [Nat.sub_mul]

/-- Para p ≥ 3, `a(p) = p² + 1` é estritamente menor que `p³ − p²`. -/
theorem a_lt_p3_minus_p2 (p : Nat) (hp : 3 ≤ p) : a p < p^3 - p^2 := by
  unfold a
  have h2 : 2 ≤ p - 1 := by omega
  have hge : 2 * p^2 ≤ (p - 1) * p^2 := Nat.mul_le_mul_right (p^2) h2
  have hp2 : 9 ≤ p^2 := by
    have hp2' : 3^2 ≤ p^2 := Nat.pow_le_pow_left hp 2
    simpa using hp2'
  have hlt : p^2 + 1 < 2 * p^2 := by omega
  exact Nat.lt_of_lt_of_le hlt (Nat.le_trans hge (Nat.le_of_eq (p3_minus_p2_factor p).symm))

/-- Teorema central: para todo `p ≥ 3`, `a(p) ≠ p³ − p²`. -/
theorem a_neq_p3_minus_p2 (p : Nat) (hp : 3 ≤ p) : a p ≠ p^3 - p^2 := by
  intro h
  have hlt := a_lt_p3_minus_p2 p hp
  rw [h] at hlt
  omega

/-- Corolário (primos ímpares): `IsPrime p`, `p ≠ 2` ⇒ `a(p) ≠ p³ − p²`. -/
theorem a_neq_p3_minus_p2_for_prime {p : Nat} (hp : IsPrime p) (h2 : p ≠ 2) :
    a p ≠ p^3 - p^2 := by
  exact a_neq_p3_minus_p2 p (prime_odd_ge_three hp h2)

/-! ## Instantiações computacionais (fecho de `native_decide`) -/

/-- p=3: 10 ≠ 18. -/
example : a 3 ≠ 3^3 - 3^2 := by
  native_decide

/-- p=5: 26 ≠ 100. -/
example : a 5 ≠ 5^3 - 5^2 := by
  native_decide

/-- p=7: 50 ≠ 294. -/
example : a 7 ≠ 7^3 - 7^2 := by
  native_decide

end ArkheEpistemic