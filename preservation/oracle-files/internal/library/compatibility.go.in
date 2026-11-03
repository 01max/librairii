package library

import (
	"context"
	"database/sql"
	"errors"
)

type CompatibilityInfo struct {
	LegacyBrokenTagID int64 `json:"legacyBrokenTagId"`
}

type CompatibilityReader interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func ReadLegacyBrokenTagID(ctx context.Context, reader CompatibilityReader) (int64, error) {
	var id int64
	err := reader.QueryRowContext(ctx, "SELECT definition_id FROM legacy_broken_tag WHERE singleton=1").Scan(&id)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, nil
	}
	return id, err
}

func (q *Query) CompatibilityInfo(ctx context.Context) (CompatibilityInfo, error) {
	var result CompatibilityInfo
	err := q.database.QueryRowContext(ctx, "SELECT definition_id FROM legacy_broken_tag WHERE singleton=1").Scan(&result.LegacyBrokenTagID)
	if errors.Is(err, sql.ErrNoRows) {
		return result, nil
	}
	return result, err
}
