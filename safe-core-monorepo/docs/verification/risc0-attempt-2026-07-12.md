# RISC0 zkVM integration — attempted, blocked by a real upstream toolchain issue

## What was tried

```sh
cargo install cargo-risczero --locked
```

The intent: install `cargo-risczero`, then `rzup install` the
`riscv32im-risc0-zkvm-elf` guest toolchain, then write a minimal real guest
program (proving something concrete, e.g. a hash preimage or a plan
validation) plus a host program that generates and verifies a receipt.

## What happened

The install fails during compilation of a transitive C++ FFI dependency,
`risc0-circuit-keccak-sys` (pulled in by `risc0-sys` 1.5.0, itself pulled
in by `cargo-risczero` 3.0.5):

```
risc0-circuit-keccak-sys-4.0.2\kernels\cxx\layout.cpp.inc(75): error C7555:
  o uso de inicializadores designados requer pelo menos o '/std:c++20'
...
error occurred in cc-rs: command did not execute successfully
  (status code exit code: 2):
  "...\MSVC\14.44.35207\bin\HostX64\x64\cl.exe" ... "/std:c++17" ...
error: failed to compile `cargo-risczero v3.0.5`
```

`risc0-circuit-keccak-sys`'s C++ source (`kernels/cxx/layout.cpp.inc`) uses
C++20 designated initializers (`.field = value` struct init syntax), but
its `build.rs` (via the `cc` crate) invokes `cl.exe` with `/std:c++17`
hardcoded — a real incompatibility between what this crate's C++ code
requires and what its own build script asks the compiler to accept, on
MSVC specifically. This is not a flag this workspace's `Cargo.toml` or
this session's environment setup controls; it would require either a
patched/newer `risc0-circuit-keccak-sys` that raises its own `/std:` flag
on MSVC, or building on a different toolchain (GCC/Clang via WSL/Linux)
where this crate's `cc` invocation may pick a compiler with saner C++20
defaults.

`risc0-zkvm` (the crate actual guest/host programs would depend on
directly, rather than `cargo-risczero`, which is just the toolchain
installer CLI) pulls in the same STARK circuit crates
(`risc0-circuit-keccak-sys` and siblings) for its host-side prover, so
there is no reasonable expectation that depending on `risc0-zkvm` directly
in a new crate would avoid this exact failure — not independently
re-attempted, since the failure's root cause (an MSVC C++ standard version
mismatch inside a specific upstream crate's build script) is orthogonal to
which RISC0 crate is the entry point.

## Disposition

Not pursued further in this pass. This is a genuine, reproduced toolchain
incompatibility (Windows + this MSVC Build Tools version + this
`risc0-circuit-keccak-sys` version), consistent with this session's
established practice of disclosing real environment/toolchain limits
(compare the Kani model-checker situation, blocked on an exact nightly
toolchain pin, documented in `docs/verification/README.md`) rather than
fabricating a result. A real attempt at RISC0 integration would need
either: a Linux/WSL2 build environment, a `risc0-circuit-keccak-sys`
release that fixes its MSVC `/std:` flag, or building with a different C++
toolchain (e.g. `clang-cl`) that defaults to C++20 regardless of the
crate's requested flag.
