package removal

import (
	"context"
	"database/sql"
	"fmt"
)

type Intent struct {
	ID          string
	StoryID     int64
	ManagedPath string
	TrashPath   string
}

type Repository struct {
	database *sql.DB
}

func NewRepository(database *sql.DB) *Repository {
	return &Repository{database: database}
}

func (r *Repository) Create(ctx context.Context, intent Intent) error {
	_, err := r.database.ExecContext(
		ctx,
		`INSERT INTO removal_intents (id, story_id, managed_path, trash_path)
		 VALUES (?, ?, ?, ?)`,
		intent.ID,
		intent.StoryID,
		intent.ManagedPath,
		intent.TrashPath,
	)
	if err != nil {
		return fmt.Errorf("create removal intent: %w", err)
	}
	return nil
}

func (r *Repository) Delete(ctx context.Context, intentID string) error {
	result, err := r.database.ExecContext(
		ctx,
		"DELETE FROM removal_intents WHERE id = ?",
		intentID,
	)
	if err != nil {
		return fmt.Errorf("delete removal intent: %w", err)
	}
	deleted, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("read removal intent delete count: %w", err)
	}
	if deleted == 0 {
		return sql.ErrNoRows
	}
	return nil
}

func (r *Repository) List(ctx context.Context) ([]Intent, error) {
	rows, err := r.database.QueryContext(
		ctx,
		`SELECT id, story_id, managed_path, trash_path
		 FROM removal_intents
		 ORDER BY created_at, id`,
	)
	if err != nil {
		return nil, fmt.Errorf("list removal intents: %w", err)
	}
	defer rows.Close()

	var intents []Intent
	for rows.Next() {
		var intent Intent
		if err := rows.Scan(
			&intent.ID,
			&intent.StoryID,
			&intent.ManagedPath,
			&intent.TrashPath,
		); err != nil {
			return nil, err
		}
		intents = append(intents, intent)
	}
	return intents, rows.Err()
}
