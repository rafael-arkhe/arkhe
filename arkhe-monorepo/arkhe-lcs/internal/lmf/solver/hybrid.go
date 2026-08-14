// ============================================================================
// arkhe-lcs :: Hybrid Solver — v0.9.0
// ============================================================================
package solver

// HybridSolver combines multiple position solutions by inverse-uncertainty
// weighted averaging.
type HybridSolver struct {
	lambda float64
}

// NewHybridSolver returns a hybrid combiner.
func NewHybridSolver() *HybridSolver {
	return &HybridSolver{lambda: 0.001}
}

// WeightedAverage combines ECEF solutions using 1/uncertainty weights.
func (hs *HybridSolver) WeightedAverage(results [][3]float64, uncertainties []float64) [3]float64 {
	var x, y, z, total float64
	for i, pos := range results {
		u := uncertainties[i]
		if u < Epsilon {
			u = Epsilon
		}
		w := 1.0 / u
		x += pos[0] * w
		y += pos[1] * w
		z += pos[2] * w
		total += w
	}
	if total == 0 {
		return results[0]
	}
	return [3]float64{x / total, y / total, z / total}
}
