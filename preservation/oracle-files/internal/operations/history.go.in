package operations

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
)

const historyPageSize = 20

type HistoryPage struct {
	Operations []Snapshot `json:"operations"`
	NextBefore string     `json:"nextBefore,omitempty"`
}

// History uses a stable creation-time/ID cursor so new work cannot shift older pages.
func (r *Repository) History(ctx context.Context, before string) (HistoryPage, error) {
	query := `SELECT id FROM file_operations`
	args := []any{}
	if before != "" {
		var createdAt string
		err := r.database.QueryRowContext(ctx, `SELECT created_at FROM file_operations WHERE id = ?`, before).Scan(&createdAt)
		if errors.Is(err, sql.ErrNoRows) {
			return HistoryPage{}, fmt.Errorf("%w: history cursor", ErrInvalidRequest)
		}
		if err != nil {
			return HistoryPage{}, err
		}
		query += ` WHERE (created_at, id) < (?, ?)`
		args = append(args, createdAt, before)
	}
	query += ` ORDER BY created_at DESC, id DESC LIMIT ?`
	args = append(args, historyPageSize+1)
	rows, err := r.database.QueryContext(ctx, query, args...)
	if err != nil {
		return HistoryPage{}, err
	}
	ids := []string{}
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			_ = rows.Close()
			return HistoryPage{}, err
		}
		ids = append(ids, id)
	}
	if err := rows.Err(); err != nil {
		_ = rows.Close()
		return HistoryPage{}, err
	}
	if err := rows.Close(); err != nil {
		return HistoryPage{}, err
	}
	page := HistoryPage{Operations: make([]Snapshot, 0, historyPageSize)}
	if len(ids) > historyPageSize {
		ids = ids[:historyPageSize]
		page.NextBefore = ids[len(ids)-1]
	}
	for _, id := range ids {
		snapshot, err := r.Snapshot(ctx, id)
		if err != nil {
			return HistoryPage{}, err
		}
		page.Operations = append(page.Operations, snapshot)
	}
	return page, nil
}
