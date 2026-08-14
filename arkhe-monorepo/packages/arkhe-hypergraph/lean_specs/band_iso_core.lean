/- band_iso — abstract core (Mathlib-free, verified by `lean band_iso_core.lean`). -/
namespace Band

universe u v

/-- A bare bijection, to avoid pulling in Mathlib's `Equiv`. -/
structure Iso (A : Type u) (B : Type v) where
  toFun     : A -> B
  invFun    : B -> A
  left_inv  : forall a, invFun (toFun a) = a
  right_inv : forall b, toFun (invFun b) = b

variable {X : Type u} {D : Type v}

/--
  Fundamental-domain equivalence of quotients: if `inj : D -> X` covers every
  `X`-site modulo `Sx`, and the orbit relation restricted to `D` is exactly the
  seam `Sd`, then `Quotient Sd` and `Quotient Sx` are isomorphic.

  This is the "Möbius band site" lemma: a lattice quotient constructed from a
  covering seam rep is genuine (no `sorry`), given exactly two facts —
    hcov   : every ambient site is in the image orbit of the seam rep
    hfaith : the restricted orbit relation IS the seam relation.
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