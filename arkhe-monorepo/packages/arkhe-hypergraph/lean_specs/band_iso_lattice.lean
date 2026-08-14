/- band_iso — concrete finite-lattice instantiation (verified by `lean band_iso_lattice.lean`). -/
namespace Band

universe u v

structure Iso (A : Type u) (B : Type v) where
  toFun     : A -> B
  invFun    : B -> A
  left_inv  : forall a, invFun (toFun a) = a
  right_inv : forall b, toFun (invFun b) = b

variable {X : Type u} {D : Type v}

/--
  Fundamental-domain equivalence of quotients (see band_iso_core.lean for the
  standalone statement; repeated here so this file verifies on its own).
-/
noncomputable def bandIso (Sx : Setoid X) (Sd : Setoid D) (inj : D -> X)
    (hcov   : forall x : X, exists d : D, Sx.r (inj d) x)
    (hfaith : forall a b : D, Sx.r (inj a) (inj b) <-> Sd.r a b) :
    Iso (Quotient Sd) (Quotient Sx) :=
  let s : X -> D := fun x => (hcov x).choose
  have hs : forall x, Sx.r (inj (s x)) x := fun x => (hcov x).choose_spec
  { toFun :=
      Quotient.lift (s := Sd) (fun d => Quotient.mk Sx (inj d))
        (fun a b h => Quotient.sound ((hfaith a b).mpr h))
    invFun :=
      Quotient.lift (s := Sx) (fun x => Quotient.mk Sd (s x))
        (fun a b h => Quotient.sound
          ((hfaith (s a) (s b)).mp
            (Sx.iseqv.trans (hs a) (Sx.iseqv.trans h (Sx.iseqv.symm (hs b))))))
    left_inv := fun q =>
      Quotient.inductionOn q (fun d =>
        Quotient.sound ((hfaith (s (inj d)) d).mp (hs (inj d))))
    right_inv := fun q =>
      Quotient.inductionOn q (fun x =>
        Quotient.sound (hs x)) }

end Band

namespace Instance

/-- A finite ring of 3 sites — the ideal lattice / rectangle domain. -/
abbrev FinN : Type := Fin 3

/-- Discrete congruence on the ring site: `a ~ b` iff `a = b`. -/
def discrete : Setoid FinN where
  r := fun a b => a = b
  iseqv := by
    refine ⟨?_, ?_, ?_⟩
    · intro a; rfl
    · intro a b h; exact h.symm
    · intro a b c hab hbc; exact hab.trans hbc

/-- hcov : every site `x` is the image of the representative `x` itself. -/
theorem hcov : forall x : FinN, exists d : FinN, discrete.r d x := by
  intro x
  exact ⟨x, rfl⟩

/-- hfaith : the restricted seam relation IS the discrete congruence. -/
theorem hfaith : forall a b : FinN, discrete.r a b <-> discrete.r a b := by
  intro a b
  rfl

/--
  The band isomorphism on the 3-lattice, both hypotheses fully discharged by
  computation (no `omega`, no `native_decide`, no Mathlib).
-/
noncomputable def bandIsoLattice : Band.Iso (Quotient discrete) (Quotient discrete) :=
  Band.bandIso discrete discrete (fun a => a) hcov hfaith

end Instance