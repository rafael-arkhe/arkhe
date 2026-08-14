// ============================================================================
// arkhe-lcs :: Positioning Engine — v0.9.0
// ============================================================================
package lmf

import (
	"errors"
	"sync"

	"arkhe-lcs/internal/lmf/solver"
)

// PositioningEngine is the ECEF-based positioning engine. It is compatible
// with terrestrial 4G/5G and NTN (LEO) without code changes.
type PositioningEngine struct {
	mu     sync.RWMutex
	cellDB *MemoryCellDB
	hybrid *solver.HybridSolver
}

// NewPositioningEngine returns an engine bound to the given cell database.
func NewPositioningEngine(cellDB *MemoryCellDB) *PositioningEngine {
	return &PositioningEngine{
		cellDB: cellDB,
		hybrid: solver.NewHybridSolver(),
	}
}

// LocateECID computes a UE position from E-CID TA measurements. Returns
// WGS84 lat/lon/alt.
func (p *PositioningEngine) LocateECID(measurements []CellMeasurement) (float64, float64, float64, error) {
	p.mu.RLock()
	defer p.mu.RUnlock()
	if len(measurements) == 0 {
		return 0, 0, 0, errors.New("empty measurements")
	}
	// Use serving cell as initial guess.
	init := solver.GeodeticToECEF(measurements[0].Lat, measurements[0].Lon, measurements[0].Alt)
	refined := solver.RefineECIDWithTaylor(measurements, init)
	lat, lon, alt := solver.ECEFToGeodetic(refined[0], refined[1], refined[2])
	return lat, lon, alt, nil
}
