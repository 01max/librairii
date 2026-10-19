package library

import (
	"context"
	"database/sql"
	"errors"
)

type CompatibilityInfo struct {
	LegacyBrokenTagID int64 `json:"legacyBrokenTagId"`
}

func (q *Query) CompatibilityInfo(ctx context.Context) (CompatibilityInfo, error) {
	var result CompatibilityInfo
	err := q.database.QueryRowContext(ctx, "SELECT definition_id FROM legacy_broken_tag WHERE singleton=1").Scan(&result.LegacyBrokenTagID)
	if errors.Is(err, sql.ErrNoRows) {
		return result, nil
	}
	return result, err
}
