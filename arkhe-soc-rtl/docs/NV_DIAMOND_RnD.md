# NV-Diamond / UEP — Speculative R&D (NOT part of the RTL deliverable)

**Status:** L3 speculation. Quarantined here on purpose (checklist item **S9**) so it
does not leak into the synthesizable AFE/CORDIC engineering, which is L0.

## Why this is separated

The "NV Diamond ↔ Arkhe" mapping (SUBSTRATE↔diamond lattice, UEP↔exceptional
point, `band_iso`↔spin dynamics, `hfaith`↔T₂ coherence) is **analogy**, not
engineering. None of it is checkable against a testbench, a datasheet, or a
measurement, and none of it produces a bit that the SoC computes. Mixing it into
`afe_smith_cordic.sv` or the `domain_d` contract would make a physically
meaningful signal (`1 - |Γ|²`) look like it inherits validation it does not have.

## What would make any of it real (bar to clear before promotion to L1)

1. A concrete, measured quantity from an NV device (e.g. contrast, T₂*, ODMR
   linewidth) with units and an instrument, not a metaphor.
2. A defined map from that quantity to a specific SoC register with a Q-format,
   the same way `afe_smith_cordic` maps I/Q → `domain_d[i]` in Q16.16.
3. A falsifiable prediction: "if X changes, register Y changes by Z," testable
   without appealing to the analogy.

Until all three exist, this document is a notebook, not a spec. Do not cite it as
support for the AFE datapath.

## Salvageable kernel (if anything)

The only non-fictional thread is generic: *if* a future revision adds a real
magnetometry front-end, its digitized output could be routed through the same
CORDIC/`domain_d` plumbing. That is a plumbing statement about the existing RTL,
not a claim about diamond physics.
