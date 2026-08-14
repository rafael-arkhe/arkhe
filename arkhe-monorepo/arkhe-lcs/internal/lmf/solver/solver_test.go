package solver

import (
	"math"
	"testing"
)

// Place the UE ~1.5 km east of cell A. E-CID with TA consistent with that
// geometry should converge to within a few hundred meters.
func TestLocateECIDConverges(t *testing.T) {
	lat0, lon0 := -23.55, -46.63
	trueLat, trueLon := -23.55, -46.615 // ~1.5 km east
	meas := []CellMeasurement{
		{CellID: "A", Lat: lat0, Lon: lon0, Alt: 600, TA: 20},  // ~1.53 km
		{CellID: "B", Lat: lat0, Lon: lon0 - 0.02, Alt: 500, TA: 46}, // ~3.57 km
		{CellID: "C", Lat: lat0 + 0.015, Lon: lon0 + 0.01, Alt: 700, TA: 22}, // ~1.74 km
	}
	guess := GeodeticToECEF(lat0, lon0, 0)
	refined := RefineECIDWithTaylor(meas, guess)
	lat, lon, _ := ECEFToGeodetic(refined[0], refined[1], refined[2])
	dlat := lat - trueLat
	dlon := lon - trueLon
	// ~111 km per degree of latitude.
	if d := dlat * 111000; math.Abs(d) > 500 {
		t.Errorf("latitude error too large: %.0f m", d)
	}
	if d := dlon * 111000 * math.Cos(lat*math.Pi/180); math.Abs(d) > 700 {
		t.Errorf("longitude error too large: %.0f m", d)
	}
}

func TestSolveECIDWeighted(t *testing.T) {
	// Cell A at (0,0) with TA 0 has minimum uncertainty, so it dominates.
	aECEF := GeodeticToECEF(0, 0, 0)
	res, err := SolveECID([]CellMeasurement{
		{CellID: "A", Lat: 0, Lon: 0, Alt: 0, TA: 0},
		{CellID: "B", Lat: 1, Lon: 1, Alt: 0, TA: 100},
	})
	if err != nil {
		t.Fatalf("SolveECID: %v", err)
	}
	got := [3]float64{res.X, res.Y, res.Z}
	for i := range got {
		if math.Abs(got[i]-aECEF[i]) > 1e-3 {
			t.Errorf("axis %d: got %.3f want %.3f", i, got[i], aECEF[i])
		}
	}
}

func TestSolveLeastSquaresSingular(t *testing.T) {
	// All rows identical -> singular matrix -> zero vector, no panic.
	delta := SolveLeastSquares(
		[][]float64{{1, 0, 0}, {1, 0, 0}},
		[]float64{1, 1},
	)
	if delta[0] != 0 || delta[1] != 0 || delta[2] != 0 {
		t.Errorf("expected zero on singular system, got %v", delta)
	}
}

func TestSolveLeastSquaresKnown(t *testing.T) {
	// x = 2y - z, with solution [1,1,1]: rows [1,-2,1], rhs 0 with noise.
	H := [][]float64{{1, -2, 1}, {2, 1, 0}, {0, 1, 1}}
	y := []float64{0, 3, 2}
	got := SolveLeastSquares(H, y)
	// Exact solve: [1,1,1].
	for i, want := range []float64{1, 1, 1} {
		if math.Abs(got[i]-want) > 1e-6 {
			t.Errorf("solve[%d]: got %v want %v", i, got[i], want)
		}
	}
}

func TestSolveOTDOARequiresNeighbors(t *testing.T) {
	_, err := SolveOTDOA(
		OTDOAMeasurement{CellID: "S", Lat: 0, Lon: 0, Alt: 0, RSTD: 0},
		[]OTDOAMeasurement{{CellID: "N1", Lat: 1, Lon: 0, Alt: 0, RSTD: 1}},
		[3]float64{0, 0, 0},
	)
	if err == nil {
		t.Fatal("expected error for <2 neighbors")
	}
}

func TestSolveMultiRTTRequiresCells(t *testing.T) {
	_, err := SolveMultiRTT(
		[]MultiRTTMeasurement{{CellID: "A", Lat: 0, Lon: 0, Alt: 0, RTTus: 10}},
		[3]float64{0, 0, 0},
	)
	if err == nil {
		t.Fatal("expected error for <3 cells")
	}
}

func TestHybridWeightedAverage(t *testing.T) {
	hs := NewHybridSolver()
	got := hs.WeightedAverage(
		[][3]float64{{10, 0, 0}, {20, 0, 0}},
		[]float64{5, 20}, // first is 4x more reliable
	)
	if math.Abs(got[0]-12) > 1e-9 {
		t.Errorf("weighted avg: got %v", got[0])
	}
}
