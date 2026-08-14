// ============================================================================
// arkhe-lcs :: OTDOA Solver (Hyperbolic) — v0.9.0
// ============================================================================
package solver

import (
	"errors"
	"math"
)

const (
	// Ts is the 3GPP LTE sampling period.
	Ts = 1.0 / 30720000.0
	// C is the speed of light in m/s.
	C = 299792458.0
)

// SolveOTDOA solves the hyperbolic TDOA system using Taylor linearization
// with a Levenberg-style step scaling. Returns the UE position in ECEF.
func SolveOTDOA(serving OTDOAMeasurement, neighbors []OTDOAMeasurement, guess [3]float64) ([3]float64, error) {
	if len(neighbors) < 2 {
		return [3]float64{}, errors.New("need >=2 neighbors")
	}
	pos := guess
	servingECEF := GeodeticToECEF(serving.Lat, serving.Lon, serving.Alt)
	lambda := 0.001
	for iter := 0; iter < 15; iter++ {
		dx0, dy0, dz0 := pos[0]-servingECEF[0], pos[1]-servingECEF[1], pos[2]-servingECEF[2]
		d0 := math.Sqrt(dx0*dx0 + dy0*dy0 + dz0*dz0)
		if d0 < 1.0 {
			d0 = 1.0
		}
		var H [][]float64
		var deltaY []float64
		for _, nb := range neighbors {
			p := GeodeticToECEF(nb.Lat, nb.Lon, nb.Alt)
			dx, dy, dz := pos[0]-p[0], pos[1]-p[1], pos[2]-p[2]
			d := math.Sqrt(dx*dx + dy*dy + dz*dz)
			if d < 1.0 {
				d = 1.0
			}
			targetDiff := nb.RSTD * Ts * C
			residual := (d - d0) - targetDiff
			H = append(H, []float64{
				dx/d - dx0/d0,
				dy/d - dy0/d0,
				dz/d - dz0/d0,
			})
			deltaY = append(deltaY, residual)
		}
		delta := SolveLeastSquares(H, deltaY)
		pos[0] += delta[0] * lambda
		pos[1] += delta[1] * lambda
		pos[2] += delta[2] * lambda
		ssr := 0.0
		for _, r := range deltaY {
			ssr += r * r
		}
		if math.Sqrt(ssr) < 10.0 {
			break
		}
		if ssr < 0.1 {
			lambda *= 0.5
		} else {
			lambda *= 1.5
		}
	}
	return pos, nil
}
