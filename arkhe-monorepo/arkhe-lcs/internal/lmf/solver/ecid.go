// ============================================================================
// arkhe-lcs :: E-CID Solver (TA-based) — v0.9.0
// ============================================================================
package solver

import (
	"errors"
	"math"
)

const (
	// TaUnitDistance is the distance per TA unit (c * 0.5µs / 2).
	TaUnitDistance = 78.4 // m por TA unit
	// Epsilon is the minimum uncertainty floor in meters.
	Epsilon = 5.0 // m
)

// SolveECID selects the highest-weight cell as the coarse E-CID solution.
func SolveECID(cells []CellMeasurement) (ECIDResult, error) {
	if len(cells) == 0 {
		return ECIDResult{}, errors.New("no cells")
	}
	var best ECIDResult
	bestWeight := 0.0
	for _, cell := range cells {
		pos := GeodeticToECEF(cell.Lat, cell.Lon, cell.Alt)
		dist := float64(cell.TA) * TaUnitDistance
		unc := dist * 0.1
		if unc < Epsilon {
			unc = Epsilon
		}
		w := 1.0 / unc
		if w > bestWeight {
			bestWeight = w
			best = ECIDResult{X: pos[0], Y: pos[1], Z: pos[2], Uncertainty: unc, Weight: w}
		}
	}
	return best, nil
}

// RefineECIDWithTaylor refines the E-CID solution with Taylor linearization
// over all cell range observations.
func RefineECIDWithTaylor(cells []CellMeasurement, guess [3]float64) [3]float64 {
	pos := guess
	for i := 0; i < 5; i++ {
		var H [][]float64
		var deltaY []float64
		for _, cell := range cells {
			p := GeodeticToECEF(cell.Lat, cell.Lon, cell.Alt)
			target := float64(cell.TA) * TaUnitDistance
			dx, dy, dz := pos[0]-p[0], pos[1]-p[1], pos[2]-p[2]
			d := math.Sqrt(dx*dx + dy*dy + dz*dz)
			if d < 1.0 {
				d = 1.0
			}
			H = append(H, []float64{dx / d, dy / d, dz / d})
			deltaY = append(deltaY, target-d)
		}
		delta := SolveLeastSquares(H, deltaY)
		pos[0] += delta[0]
		pos[1] += delta[1]
		pos[2] += delta[2]
		if math.Sqrt(delta[0]*delta[0]+delta[1]*delta[1]+delta[2]*delta[2]) < 1.0 {
			break
		}
	}
	return pos
}
