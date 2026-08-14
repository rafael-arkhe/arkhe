// ============================================================================
// arkhe-lcs :: Geodesy Re-exports — v0.9.0
//
// The WGS84 ECEF conversions live in the solver package; this file re-exports
// them so callers can use lmf.GeodeticToECEF without importing solver.
// ============================================================================
package lmf

import "arkhe-lcs/internal/lmf/solver"

// GeodeticToECEF converts WGS84 geodetic coordinates to ECEF meters.
func GeodeticToECEF(latDeg, lonDeg, altM float64) [3]float64 {
	return solver.GeodeticToECEF(latDeg, lonDeg, altM)
}

// ECEFToGeodetic converts ECEF meters back to WGS84 geodetic coordinates.
func ECEFToGeodetic(x, y, z float64) (float64, float64, float64) {
	return solver.ECEFToGeodetic(x, y, z)
}
