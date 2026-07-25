package metadata

import (
	"context"
	"database/sql"
	"fmt"

	"github.com/01max/librairii/internal/searchtext"
)

func BackfillNormalizedTitles(ctx context.Context, database *sql.DB) error {
	if database == nil {
		return fmt.Errorf("metadata database is required")
	}
	rows, err := database.QueryContext(
		ctx,
		`SELECT id, COALESCE(title, '')
		 FROM official_story_metadata
		 WHERE title_normalized = ''
		 ORDER BY id`,
	)
	if err != nil {
		return fmt.Errorf("list official titles for normalization: %w", err)
	}
	type update struct {
		id    int64
		title string
	}
	var updates []update
	for rows.Next() {
		var item update
		if err := rows.Scan(&item.id, &item.title); err != nil {
			_ = rows.Close()
			return fmt.Errorf("scan official title for normalization: %w", err)
		}
		item.title = searchtext.Normalize(item.title)
		updates = append(updates, item)
	}
	if err := rows.Close(); err != nil {
		return fmt.Errorf("close official title rows: %w", err)
	}

	transaction, err := database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin official title normalization: %w", err)
	}
	defer transaction.Rollback()
	for _, item := range updates {
		if _, err := transaction.ExecContext(
			ctx,
			`UPDATE official_story_metadata
			 SET title_normalized = ?
			 WHERE id = ?`,
			item.title,
			item.id,
		); err != nil {
			return fmt.Errorf("normalize official title: %w", err)
		}
	}
	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("commit official title normalization: %w", err)
	}
	return nil
}
