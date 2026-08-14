// Package solver implements the LMF positioning solvers (v0.9.0): least
// squares (Taylor), E-CID (TA-based), OTDOA (hyperbolic), Multi-RTT (sphere
// intersection) and the hybrid weighted-average combiner. All math operates in
// WGS84 ECEF so it works identically for terrestrial 4G/5G and NTN (LEO).
//
// The package is intentionally self-contained (no import of the parent lmf
// package) so that lmf can import solver without a circular dependency; lmf
// re-exports these types via aliases.
package solver
