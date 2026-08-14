// ============================================================================
// arkhe-lcs :: In-Memory Cell Database — v0.9.0
// ============================================================================
package lmf

import "sync"

// CellInfo holds the geodetic position of a cell.
type CellInfo struct {
	Lat, Lon, Alt float64
}

// MemoryCellDB is a thread-safe in-memory cell database.
type MemoryCellDB struct {
	mu    sync.RWMutex
	cells map[string]CellInfo
}

// NewMemoryCellDB returns an empty cell database.
func NewMemoryCellDB() *MemoryCellDB {
	return &MemoryCellDB{cells: make(map[string]CellInfo)}
}

// Update upserts a cell position.
func (db *MemoryCellDB) Update(cellID string, info CellInfo) {
	db.mu.Lock()
	defer db.mu.Unlock()
	db.cells[cellID] = info
}

// Get returns a cell position.
func (db *MemoryCellDB) Get(cellID string) (CellInfo, bool) {
	db.mu.RLock()
	defer db.mu.RUnlock()
	info, ok := db.cells[cellID]
	return info, ok
}
