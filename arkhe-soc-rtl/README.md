# arkhe-soc-rtl — AFE Smith-Chart → Domain-D CORDIC

Real, reviewable implementation of the AFE mapper that turns 8 channels of
homodyne I/Q into the fundamental-domain vector `domain_d[0..7]`, where each
node is the physical power-coupling `1 - |Γ|²`.

This replaces the `afe_smith_cordic.sv` *sketch* from the v26.3 doc, which did
not work (dead 1-bit rotation direction, no sign handling, off-by-one scaling,
non-wrapping channel counter, incomplete AXI). See the checklist status below.

```
arkhe-soc-rtl/
├── rtl/afe_smith_cordic.sv      synthesizable module (magnitude-only CORDIC + AXI4-Lite)
├── model/cordic_ref.py          bit-accurate golden model + self-check (RUNS, PASSES)
├── tb/test_afe_smith_cordic.py  cocotb differential TB vs the model (needs a simulator)
├── tb/Makefile                  Icarus/Verilator runner
└── docs/NV_DIAMOND_RnD.md       quarantined speculation (item S9)
```

## What actually runs here

`model/cordic_ref.py` executes and self-checks with stock Python (no deps):

```bash
python3 model/cordic_ref.py
```

Result (measured, not asserted):

```
CORDIC gain K     = 1.6467601927
INV_K (Q0.15)     = 19898
worst |mag| error = 2.75e-4   (all four quadrants)
effective bits    = 11.8
coupling: Gamma=0 -> 1.00000 | |Gamma|=1 -> 0.00000 | |Gamma|=0.5 -> 0.74976
SELFCHECK PASSED
```

The RTL is a line-by-line transcription of this model; the cocotb TB
(`tb/`) checks the DUT against `smith_pipeline()` per channel. **The Verilog was
NOT simulated in the authoring environment — no iverilog/verilator/cocotb was
available.** To close that gap, run `make` in `tb/` on a machine with a
simulator; the pass/fail is the differential comparison, so it is objective.

## Q-format map (checklist M5)

| Signal                | Format   | 1.0 =  | Notes |
|-----------------------|----------|--------|-------|
| `adc_i`, `adc_q`      | Q2.13 (signed, 16b) | 8192 | Re/Im of Γ; Q2.13 gives ±4.0 headroom over the ±1 passive range |
| CORDIC `x`,`y`        | Q2.13 (signed, 24b) | 8192 | widened for CORDIC gain K≈1.6468 |
| magnitude (post-INV_K)| Q2.13 (signed)      | 8192 | `(x*INV_K) >> 15`, INV_K = round(2¹⁵/K) = 19898 |
| `mag_q16`             | Q16.16 (signed)     | 65536| magnitude rescaled `<< 3` (13→16 frac bits) |
| `gamma_sq`            | Q16.16              | 65536| `(mag_q16²) >> 16` |
| `domain_d[ch]`        | Q16.16 (unsigned)   | 65536| `1 - |Γ|²`, clipped to [0,1] |

Why magnitude-only: vectoring mode yields the magnitude in `x` without ever
needing the accumulated angle `z`, so the atan LUT (and the draft's LUT/scaling
bug) is deleted, not fixed.

## Throughput (checklist M7)

Per channel: `S_IDLE` accept (1) + `S_RUN` (N_ITER = 12) + `S_DONE` writeback (1)
= **14 cycles/channel**. Eight channels = **112 cycles**.

| Clock    | 8-channel latency |
|----------|-------------------|
| 100 MHz  | 1.12 µs           |
| 200 MHz  | 0.56 µs           |

Note: the honest number is **1.12 µs @ 100 MHz**, not the "960 ns" the checklist
guessed — that figure omitted the accept + writeback cycles (it assumed a flat
12 cycles/channel = 96 → 960 ns). Pipelining the accept/writeback into `S_RUN`
would recover it; the current design keeps them explicit for clarity.

## Checklist status

| Item | Description | Status |
|------|-------------|--------|
| **C1** | Sign extension in `x`,`y` | ✅ done — `abs_i/abs_q` first-quadrant fold, sign-extended to INT_W |
| **C2** | AXI4-Lite B channel (`bvalid`/`bresp`) | ✅ done — full AW/W/B write channel |
| **C3** | Rotation direction not a 1-bit `reg signed` | ✅ done — `dir_pos` combinational net |
| **M5** | Document Q-format ADC→CORDIC→out | ✅ done — table above + RTL header |
| **M6** | AR→R pipelined, VALID ⟂ READY | ✅ done — 1-cycle registered read, `rvalid` set independent of `rready` |
| **M7** | Document 8-channel throughput | ✅ done — 1.12 µs @100 MHz (corrected from 960 ns) |
| **S9** | Isolate NV-Diamond into R&D doc | ✅ done — `docs/NV_DIAMOND_RnD.md` |
| **TEST** | 4-quadrant magnitude | ✅ model-verified (worst 2.75e-4); ⏳ RTL pending a simulator |
| **TEST** | AXI protocol via VIP (cocotb) | ⏳ TB written (`test_axi_protocol`); ⏳ needs iverilog/verilator to run |

Two items are `⏳ pending a simulator` and I am not marking them ✅: the model
proves the arithmetic, but the RTL itself has not been executed here. That is the
one gap between this and "verified silicon-ready."
