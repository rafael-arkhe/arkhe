package store

import (
	"encoding/json"
	"errors"
)

func encodeLine(e Event) ([]byte, error) {
	b, err := json.Marshal(e)
	if err != nil {
		return nil, err
	}
	if len(b) == 0 {
		return nil, errors.New("store: empty event encoding")
	}
	return append(b, '\n'), nil
}
