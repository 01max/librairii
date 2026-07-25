package operations

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"time"
)

var (
	ErrOperationNotActive = errors.New("operation is not active")
	ErrInvalidTransition  = errors.New("operation state transition is invalid")
)

type Repository struct {
	database *sql.DB
}

func NewRepository(database *sql.DB) *Repository {
	return &Repository{database: database}
}

func (r *Repository) CreateImport(
	ctx context.Context,
	id string,
	items []NewItem,
	now time.Time,
) (Snapshot, error) {
	if id == "" || len(items) == 0 {
		return Snapshot{}, fmt.Errorf("%w: empty operation", ErrInvalidTransition)
	}
	transaction, err := r.database.BeginTx(ctx, nil)
	if err != nil {
		return Snapshot{}, fmt.Errorf("begin operation creation: %w", err)
	}
	defer transaction.Rollback()

	if _, err := transaction.ExecContext(
		ctx,
		`INSERT INTO file_operations (
			id, kind, status, total_items, created_at
		) VALUES (?, 'import', 'queued', ?, ?)`,
		id,
		len(items),
		formatTime(now),
	); err != nil {
		return Snapshot{}, fmt.Errorf("insert import operation: %w", err)
	}
	for _, item := range items {
		if item.SourceName == "" || item.TotalBytes < 0 {
			return Snapshot{}, fmt.Errorf("%w: invalid operation item", ErrInvalidTransition)
		}
		if _, err := transaction.ExecContext(
			ctx,
			`INSERT INTO file_operation_items (
				operation_id, source_name, status, total_bytes
			) VALUES (?, ?, 'pending', ?)`,
			id,
			item.SourceName,
			item.TotalBytes,
		); err != nil {
			return Snapshot{}, fmt.Errorf("insert import operation item: %w", err)
		}
	}
	if err := transaction.Commit(); err != nil {
		return Snapshot{}, fmt.Errorf("commit operation creation: %w", err)
	}
	return r.Snapshot(ctx, id)
}

func (r *Repository) MarkRunning(ctx context.Context, id string, now time.Time) error {
	result, err := r.database.ExecContext(
		ctx,
		`UPDATE file_operations
		 SET status = 'running', started_at = COALESCE(started_at, ?)
		 WHERE id = ? AND status IN ('queued', 'running')`,
		formatTime(now),
		id,
	)
	return transitionResult(result, err)
}

func (r *Repository) MarkItemRunning(ctx context.Context, itemID int64) error {
	result, err := r.database.ExecContext(
		ctx,
		`UPDATE file_operation_items
		 SET status = 'running'
		 WHERE id = ? AND status = 'pending'`,
		itemID,
	)
	return transitionResult(result, err)
}

func (r *Repository) CompleteItem(
	ctx context.Context,
	operationID string,
	item ItemSnapshot,
) error {
	transaction, err := r.database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin operation item completion: %w", err)
	}
	defer transaction.Rollback()

	var storyID any
	if item.StoryID != 0 {
		storyID = item.StoryID
	}
	result, err := transaction.ExecContext(
		ctx,
		`UPDATE file_operation_items
		 SET story_id = ?,
		     status = ?,
		     outcome_code = NULLIF(?, ''),
		     outcome_message = NULLIF(?, ''),
		     completed_bytes = ?
		 WHERE id = ?
		   AND operation_id = ?
		   AND status IN ('pending', 'running')`,
		storyID,
		item.Status,
		item.OutcomeCode,
		item.OutcomeMessage,
		item.CompletedBytes,
		item.ID,
		operationID,
	)
	if err := transitionResult(result, err); err != nil {
		return err
	}
	result, err = transaction.ExecContext(
		ctx,
		`UPDATE file_operations
		 SET completed_items = completed_items + 1
		 WHERE id = ?
		   AND status IN ('queued', 'running')
		   AND completed_items < total_items`,
		operationID,
	)
	if err := transitionResult(result, err); err != nil {
		return err
	}
	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("commit operation item completion: %w", err)
	}
	return nil
}

func (r *Repository) RequestCancel(ctx context.Context, id string) error {
	result, err := r.database.ExecContext(
		ctx,
		`UPDATE file_operations
		 SET cancel_requested = 1
		 WHERE id = ? AND status IN ('queued', 'running')`,
		id,
	)
	if err != nil {
		return err
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if rows != 1 {
		return ErrOperationNotActive
	}
	return nil
}

func (r *Repository) Finish(
	ctx context.Context,
	id string,
	status Status,
	errorCode string,
	errorMessage string,
	now time.Time,
) error {
	if !(Snapshot{Status: status}).Terminal() || status == StatusInterrupted {
		return ErrInvalidTransition
	}
	result, err := r.database.ExecContext(
		ctx,
		`UPDATE file_operations
		 SET status = ?,
		     error_code = NULLIF(?, ''),
		     error_message = NULLIF(?, ''),
		     finished_at = ?
		 WHERE id = ?
		   AND status IN ('queued', 'running')
		   AND completed_items = total_items`,
		status,
		errorCode,
		errorMessage,
		formatTime(now),
		id,
	)
	return transitionResult(result, err)
}

func (r *Repository) Interrupt(
	ctx context.Context,
	id string,
	errorCode string,
	errorMessage string,
	now time.Time,
) (Snapshot, error) {
	if id == "" || errorCode == "" || errorMessage == "" {
		return Snapshot{}, ErrInvalidTransition
	}
	transaction, err := r.database.BeginTx(ctx, nil)
	if err != nil {
		return Snapshot{}, fmt.Errorf("begin operation interruption: %w", err)
	}
	defer transaction.Rollback()

	if _, err := transaction.ExecContext(
		ctx,
		`UPDATE file_operation_items
		 SET status = 'cancelled',
		     outcome_code = ?,
		     outcome_message = ?
		 WHERE operation_id = ?
		   AND status IN ('pending', 'running')`,
		errorCode,
		errorMessage,
		id,
	); err != nil {
		return Snapshot{}, fmt.Errorf("interrupt operation items: %w", err)
	}
	result, err := transaction.ExecContext(
		ctx,
		`UPDATE file_operations
		 SET status = 'interrupted',
		     completed_items = total_items,
		     error_code = ?,
		     error_message = ?,
		     finished_at = ?
		 WHERE id = ? AND status IN ('queued', 'running')`,
		errorCode,
		errorMessage,
		formatTime(now),
		id,
	)
	if err := transitionResult(result, err); err != nil {
		return Snapshot{}, err
	}
	if err := transaction.Commit(); err != nil {
		return Snapshot{}, fmt.Errorf("commit operation interruption: %w", err)
	}
	return r.Snapshot(ctx, id)
}

func (r *Repository) Snapshot(ctx context.Context, id string) (Snapshot, error) {
	var snapshot Snapshot
	var cancelRequested int
	var startedAt sql.NullString
	var finishedAt sql.NullString
	err := r.database.QueryRowContext(
		ctx,
		`SELECT
			id,
			kind,
			status,
			completed_items,
			total_items,
			cancel_requested,
			COALESCE(error_code, ''),
			COALESCE(error_message, ''),
			created_at,
			started_at,
			finished_at
		 FROM file_operations
		 WHERE id = ?`,
		id,
	).Scan(
		&snapshot.ID,
		&snapshot.Kind,
		&snapshot.Status,
		&snapshot.CompletedItems,
		&snapshot.TotalItems,
		&cancelRequested,
		&snapshot.ErrorCode,
		&snapshot.ErrorMessage,
		&snapshot.CreatedAt,
		&startedAt,
		&finishedAt,
	)
	if err != nil {
		return Snapshot{}, err
	}
	snapshot.CancelRequested = cancelRequested == 1
	snapshot.StartedAt = startedAt.String
	snapshot.FinishedAt = finishedAt.String

	rows, err := r.database.QueryContext(
		ctx,
		`SELECT
			id,
			COALESCE(story_id, 0),
			source_name,
			status,
			COALESCE(outcome_code, ''),
			COALESCE(outcome_message, ''),
			completed_bytes,
			total_bytes
		 FROM file_operation_items
		 WHERE operation_id = ?
		 ORDER BY id`,
		id,
	)
	if err != nil {
		return Snapshot{}, err
	}
	defer rows.Close()
	for rows.Next() {
		var item ItemSnapshot
		if err := rows.Scan(
			&item.ID,
			&item.StoryID,
			&item.SourceName,
			&item.Status,
			&item.OutcomeCode,
			&item.OutcomeMessage,
			&item.CompletedBytes,
			&item.TotalBytes,
		); err != nil {
			return Snapshot{}, err
		}
		snapshot.Items = append(snapshot.Items, item)
	}
	return snapshot, rows.Err()
}

func (r *Repository) InterruptActive(
	ctx context.Context,
	now time.Time,
) ([]Snapshot, error) {
	rows, err := r.database.QueryContext(
		ctx,
		`SELECT id
		 FROM file_operations
		 WHERE status IN ('queued', 'running')
		 ORDER BY created_at, id`,
	)
	if err != nil {
		return nil, err
	}
	var ids []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			_ = rows.Close()
			return nil, err
		}
		ids = append(ids, id)
	}
	if err := rows.Close(); err != nil {
		return nil, err
	}

	snapshots := make([]Snapshot, 0, len(ids))
	for _, id := range ids {
		snapshot, err := r.Interrupt(
			ctx,
			id,
			"interrupted",
			"The application stopped before this operation completed.",
			now,
		)
		if err != nil {
			return nil, err
		}
		snapshots = append(snapshots, snapshot)
	}
	return snapshots, nil
}

func transitionResult(result sql.Result, err error) error {
	if err != nil {
		return err
	}
	rows, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if rows != 1 {
		return ErrInvalidTransition
	}
	return nil
}

func formatTime(value time.Time) string {
	return value.UTC().Format(time.RFC3339Nano)
}
