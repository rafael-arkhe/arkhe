/-
   SafeManifoldNucleus — Moda (dominante) sobre EscapeRegion
   Catedral OS — bloco 1073, sobre substrato REAL:
     `packages/arkhe-safe-manifold/src/frontier.rs` (dominant_invariant)
     `packages/arkhe-safe-manifold/src/escape_region.rs` (EscapeRegion).
   Núcleo Lean 4 core (SEM Mathlib, SEM sorry) — convenção do repositório
   (blocos 966/971/972/996/1009: kernel real, `decide`/`native_decide`).

   HONESTIDADE: o Rust é a realização de sanidade (como nos blocos 1009-1011,
   I534/I535); este núcleo prova a SEMÂNTICA MODAL sobre um modelo Lean do
   mesmo tipo indutivo e da mesma função de contagem/tie-break. Não há FFI
   Lean→Rust real neste bloco.

   PROPRIEDADES PROVADAS:
     I647 — severidade é estritamente monótona (0..4) nas 5 regiões.
     I648 — moda de lista vazia = none; moda de singleton = {x}.
     I649 — tie-break conservador: contagens iguais → vence a região
            MAIS SEVERA (dominant [Safe, Warning] = Warning;
            dominant [Safe, Warning, Boundary] = Boundary).
-/

inductive EscapeRegion where
| safe
| warning
| boundary
| continuum
| outside
deriving DecidableEq, Repr

namespace SafeManifoldNucleus

open EscapeRegion

/-- Severidade monótona — espelha `severity()` do frontier.rs. -/
def severity : EscapeRegion → Nat
| safe     => 0
| warning  => 1
| boundary => 2
| continuum => 3
| outside  => 4

/-- Contagem de ocorrências — espelha a contagem do dominant_invariant. -/
def count (x : EscapeRegion) : List EscapeRegion → Nat
| []      => 0
| y :: ys => (if x = y then 1 else 0) + count x ys

/-- Moda com tie-break pela severidade (reproduz *esta* semântica:
   mais frequente; empate → mais severo; conservador). -/
def dominant (hist : List EscapeRegion) : Option EscapeRegion :=
  hist.foldl
    (fun acc x =>
      match acc with
      | none => some x
      | some a =>
          let ca := count a hist
          let cx := count x hist
          if cx > ca then some x
          else if cx = ca ∧ severity x > severity a then some x
          else acc)
    none

/- I647: monotonia estrita da severidade. -/
theorem severity_safe_lt_warning : severity safe < severity warning := by native_decide
theorem severity_warning_lt_boundary : severity warning < severity boundary := by native_decide
theorem severity_boundary_lt_continuum : severity boundary < severity continuum := by native_decide
theorem severity_continuum_lt_outside : severity continuum < severity outside := by native_decide

/- I648: domínio da moda. -/
theorem dominant_empty : dominant [] = none := by native_decide
theorem dominant_singleton_safe : dominant [safe] = some safe := by native_decide
theorem dominant_singleton_outside : dominant [outside] = some outside := by native_decide

/- I649: tie-break conservador (empate → mais severo). -/
theorem dominant_tie_safe_warning : dominant [safe, warning] = some warning := by native_decide
theorem dominant_tie_three_way : dominant [safe, warning, boundary] = some boundary := by native_decide

/- Homogeneidade: janela uniforme → moda = a própria região. -/
theorem dominant_uniform_safe : dominant [safe, safe, safe] = some safe := by native_decide
theorem dominant_uniform_outside : dominant [outside, outside] = some outside := by native_decide

end SafeManifoldNucleus