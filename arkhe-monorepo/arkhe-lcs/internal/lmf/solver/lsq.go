// ============================================================================
// arkhe-lcs :: Least Squares Solver (Taylor Linearization) — v0.9.0
// ============================================================================
package solver

import "math"

// SolveLeastSquares solves the normal equations (H^T H) x = H^T deltaY for a
// 3-vector (ECEF deltas). Returns a zero vector if the system is singular.
func SolveLeastSquares(H [][]float64, deltaY []float64) [3]float64 {
	var HtH [3][3]float64
	var HtY [3]float64
	m := len(deltaY)
	for i := 0; i < m; i++ {
		for j := 0; j < 3; j++ {
			HtY[j] += H[i][j] * deltaY[i]
			for k := 0; k < 3; k++ {
				HtH[j][k] += H[i][j] * H[i][k]
			}
		}
	}
	inv := invert3x3(HtH)
	if inv == nil {
		return [3]float64{0, 0, 0}
	}
	return [3]float64{
		inv[0][0]*HtY[0] + inv[0][1]*HtY[1] + inv[0][2]*HtY[2],
		inv[1][0]*HtY[0] + inv[1][1]*HtY[1] + inv[1][2]*HtY[2],
		inv[2][0]*HtY[0] + inv[2][1]*HtY[1] + inv[2][2]*HtY[2],
	}
}

func invert3x3(m [3][3]float64) *[3][3]float64 {
	det := m[0][0]*(m[1][1]*m[2][2]-m[1][2]*m[2][1]) -
		m[0][1]*(m[1][0]*m[2][2]-m[1][2]*m[2][0]) +
		m[0][2]*(m[1][0]*m[2][1]-m[1][1]*m[2][0])
	if math.Abs(det) < 1e-10 {
		return nil
	}
	inv := [3][3]float64{}
	detInv := 1.0 / det
	inv[0][0] = (m[1][1]*m[2][2] - m[1][2]*m[2][1]) * detInv
	inv[0][1] = (m[0][2]*m[2][1] - m[0][1]*m[2][2]) * detInv
	inv[0][2] = (m[0][1]*m[1][2] - m[0][2]*m[1][1]) * detInv
	inv[1][0] = (m[1][2]*m[2][0] - m[1][0]*m[2][2]) * detInv
	inv[1][1] = (m[0][0]*m[2][2] - m[0][2]*m[2][0]) * detInv
	inv[1][2] = (m[0][2]*m[1][0] - m[0][0]*m[1][2]) * detInv
	inv[2][0] = (m[1][0]*m[2][1] - m[1][1]*m[2][0]) * detInv
	inv[2][1] = (m[0][1]*m[2][0] - m[0][0]*m[2][1]) * detInv
	inv[2][2] = (m[0][0]*m[1][1] - m[0][1]*m[1][0]) * detInv
	return &inv
}
