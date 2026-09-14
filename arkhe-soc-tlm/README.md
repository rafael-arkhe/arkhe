# arkhe-soc-tlm

Transaction-level **golden model** for the Arkhe SoC — the Rust reference the RTL
is checked against. This is the "emulador Rust primeiro" deliverable: a real,
compiling, tested crate, not a spec doc.

```bash
cargo test          # 17 tests, clippy -D warnings clean
cargo run --release -- 20000   # benchmark -> JSON report
```

## Modules

| Module | What it is | Key tests |
|--------|------------|-----------|
| `smith`   | (I,Q) → coupling `1-|Γ|²` via magnitude-only CORDIC | 4-quadrant magnitude < 3 LSB; coupling 0.75/1.0/0.0 |
| `payload` | Canonical **136-byte** AOTB payload (ints LE, f64 BE) | length == 136; stable field offsets |
| `aotb`    | Ed25519 encoder/verifier, **soft sync** | roundtrip, replay rejected, dropped-frame accepted, tamper fails |
| `sram`    | SRAM D/X double buffer + IFS expand | inclusion, w=1 identity, w=0 neighbor, swap |
| `qpl`     | ring convolution `(l+c+r)/3` + perf counters | constant fixed-point, ring wrap |
| `soc`     | integrator: AFE → D → QPL → expand → emit → verify | end-to-end 8-frame cycle |

## Cross-language consistency (the point of this crate)

`smith.rs` is a **bit-for-bit port** of `../arkhe-soc-rtl/model/cordic_ref.py`
(same `N_ITER=12`, `INV_K=19898`, Q1.13→Q16.16 path). So the chain is:

```
cordic_ref.py  ≡  arkhe-soc-tlm::smith  →  (target) afe_smith_cordic.sv
   (runs)            (runs, tested)            (transcription, cocotb TB unrun)
```

The Python model and the Rust model agree by construction; the RTL is written to
match both and is checked by the cocotb TB in `../arkhe-soc-rtl/tb/` — which still
needs a simulator to actually execute (none in the authoring env).

## Honesty notes

- The benchmark measures **host-CPU software** timing (~68 µs/iter here). It is
  NOT silicon latency and does **not** validate arXiv:2607.16100 — the report
  carries `reference_verified: false` and says so.
- Soft sync (`sequence >= expected`, reject `<`) is the one policy that must be
  identical across TLM, RTL, and any on-chain verifier.
