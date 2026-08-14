// ============================================================================
// arkhe-lcs :: LMF Types — v0.9.0
//
// Type aliases re-exported from the solver package so callers use lmf.* names.
// ============================================================================
package lmf

import "arkhe-lcs/internal/lmf/solver"

// CellMeasurement is a single serving/neighbor cell observation for E-CID.
type CellMeasurement = solver.CellMeasurement

// ECIDResult is the E-CID solution in ECEF.
type ECIDResult = solver.ECIDResult

// OTDOAMeasurement is an OTDOA neighbor observation.
type OTDOAMeasurement = solver.OTDOAMeasurement

// MultiRTTMeasurement is a multi-cell RTT observation.
type MultiRTTMeasurement = solver.MultiRTTMeasurement
