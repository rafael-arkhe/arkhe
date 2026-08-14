// ============================================================================
// arkhe-lcs :: Geodesy (WGS84 ECEF Conversion) — v0.9.0
// ============================================================================
package solver

import "math"

const (
	wgs84A  = 6378137.0
	wgs84B  = 6356752.314245
	wgs84E2 = 1 - (wgs84B*wgs84B)/(wgs84A*wgs84A)
)

// GeodeticToECEF converts WGS84 geodetic coordinates to ECEF meters.
func GeodeticToECEF(latDeg, lonDeg, altM float64) [3]float64 {
	lat := latDeg * math.Pi / 180
	lon := lonDeg * math.Pi / 180
	N := wgs84A / math.Sqrt(1-wgs84E2*math.Sin(lat)*math.Sin(lat))
	return [3]float64{
		(N + altM) * math.Cos(lat) * math.Cos(lon),
		(N + altM) * math.Cos(lat) * math.Sin(lon),
		(N*(1-wgs84E2) + altM) * math.Sin(lat),
	}
}

// ECEFToGeodetic converts ECEF meters back to WGS84 geodetic coordinates.
func ECEFToGeodetic(x, y, z float64) (float64, float64, float64) {
	lon := math.Atan2(y, x)
	p := math.Sqrt(x*x + y*y)
	lat := math.Atan2(z, p*(1-wgs84E2))
	for i := 0; i < 10; i++ {
		N := wgs84A / math.Sqrt(1-wgs84E2*math.Sin(lat)*math.Sin(lat))
		lat = math.Atan2(z+wgs84E2*N*math.Sin(lat), p)
	}
	N := wgs84A / math.Sqrt(1-wgs84E2*math.Sin(lat)*math.Sin(lat))
	alt := p/math.Cos(lat) - N
	return lat * 180 / math.Pi, lon * 180 / math.Pi, alt
}
