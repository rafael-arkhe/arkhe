// ============================================================================
// arkhe-lcs :: Multi-RTT Solver (Sphere Intersection) — v0.9.0
// ============================================================================
package solver

import (
	"errors"
	"math"
)

// SolveMultiRTT solves the multi-cell RTT sphere-intersection problem via
// Taylor linearization over round-trip ranges.
func SolveMultiRTT(cells []MultiRTTMeasurement, guess [3]float64) ([3]float64, error) {
	if len(cells) < 3 {
		return [3]float64{}, errors.New("need >=3 cells")
	}
	pos := guess
	for iter := 0; iter < 10; iter++ {
		var H [][]float64
		var deltaY []float64
		for _, cell := range cells {
			p := GeodeticToECEF(cell.Lat, cell.Lon, cell.Alt)
			target := cell.RTTus * 1e-6 * C / 2.0
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
	return pos, nil
}
