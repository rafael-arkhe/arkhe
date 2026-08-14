package lmf

import (
	"math"
	"testing"
)

func TestGeodesyRoundTrip(t *testing.T) {
	lat, lon, alt := -23.55, -46.63, 800.0
	ecef := GeodeticToECEF(lat, lon, alt)
	glat, glon, galt := ECEFToGeodetic(ecef[0], ecef[1], ecef[2])
	if math.Abs(glat-lat) > 1e-9 {
		t.Errorf("lat: got %.12f want %.12f", glat, lat)
	}
	if math.Abs(glon-lon) > 1e-9 {
		t.Errorf("lon: got %.12f want %.12f", glon, lon)
	}
	if math.Abs(galt-alt) > 0.5 {
		t.Errorf("alt: got %.2f want %.2f", galt, alt)
	}
}

func TestEngineLocateECID(t *testing.T) {
	db := NewMemoryCellDB()
	db.Update("A", CellInfo{Lat: -23.55, Lon: -46.63, Alt: 600})
	db.Update("B", CellInfo{Lat: -23.55, Lon: -46.65, Alt: 500})
	db.Update("C", CellInfo{Lat: -23.54, Lon: -46.63, Alt: 700})

	p := NewPositioningEngine(db)
	lat, lon, alt, err := p.LocateECID([]CellMeasurement{
		{CellID: "A", Lat: -23.55, Lon: -46.63, Alt: 600, TA: 21},
		{CellID: "B", Lat: -23.55, Lon: -46.65, Alt: 500, TA: 32},
		{CellID: "C", Lat: -23.54, Lon: -46.63, Alt: 700, TA: 35},
	})
	if err != nil {
		t.Fatalf("LocateECID: %v", err)
	}
	if math.Abs(lat) < 10 {
		t.Errorf("unrealistic latitude: %v", lat)
	}
	if math.Abs(lon) < 10 {
		t.Errorf("unrealistic longitude: %v", lon)
	}
	_ = alt
}

func TestEngineEmpty(t *testing.T) {
	p := NewPositioningEngine(NewMemoryCellDB())
	if _, _, _, err := p.LocateECID(nil); err == nil {
		t.Fatal("expected error on empty measurements")
	}
}
