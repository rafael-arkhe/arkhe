// ============================================================================
// arkhe-lcs :: LMF Types — v0.9.0
// ============================================================================
package solver

// CellMeasurement is a single serving/neighbor cell observation for E-CID.
type CellMeasurement struct {
	CellID string
	Lat    float64
	Lon    float64
	Alt    float64
	TA     int32   // 3GPP TA units (0-3846)
	RSRP   float64 // dBm (optional)
}

// ECIDResult is the E-CID solution in ECEF.
type ECIDResult struct {
	X, Y, Z     float64
	Uncertainty float64 // metros
	Weight      float64 // 1/uncertainty
}

// OTDOAMeasurement is an OTDOA neighbor observation.
type OTDOAMeasurement struct {
	CellID string
	Lat    float64
	Lon    float64
	Alt    float64
	RSTD   float64 // Ts (1/30720000 s)
}

// MultiRTTMeasurement is a multi-cell RTT observation.
type MultiRTTMeasurement struct {
	CellID string
	Lat    float64
	Lon    float64
	Alt    float64
	RTTus  float64 // microssegundos
}
