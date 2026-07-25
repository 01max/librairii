package operations

import (
	"context"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"path"
	"path/filepath"
	"strings"
	"time"

	"github.com/01max/librairii/internal/archive"
	"github.com/google/uuid"
)

var (
	ErrOperationNotActive = errors.New("operation is not active")
	ErrInvalidTransition  = errors.New("operation state transition is invalid")
)

type Repository struct {
	database *sql.DB
}

type newOperation struct {
	Kind         Kind
	ExportSource ExportSource
	Destination  string
	Items        []NewItem
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
	return r.create(ctx, id, newOperation{
		Kind:  KindImport,
		Items: items,
	}, now)
}

func (r *Repository) CreateMetadataSync(
	ctx context.Context,
	id string,
	locale string,
	now time.Time,
) (Snapshot, error) {
	if locale == "" {
		return Snapshot{}, fmt.Errorf("%w: empty metadata locale", ErrInvalidTransition)
	}
	return r.create(ctx, id, newOperation{
		Kind: KindMetadataSync,
		Items: []NewItem{{
			SourceName: locale,
			TotalBytes: 1,
		}},
	}, now)
}

func (r *Repository) CreateExport(
	ctx context.Context,
	id string,
	source ExportSource,
	destination string,
	items []NewItem,
	now time.Time,
) (Snapshot, error) {
	return r.create(ctx, id, newOperation{
		Kind:         KindExport,
		ExportSource: source,
		Destination:  destination,
		Items:        items,
	}, now)
}

func (r *Repository) create(
	ctx context.Context,
	id string,
	operation newOperation,
	now time.Time,
) (Snapshot, error) {
	if id == "" || len(operation.Items) == 0 {
		return Snapshot{}, fmt.Errorf("%w: empty operation", ErrInvalidTransition)
	}
	if operation.Kind != KindImport &&
		operation.Kind != KindMetadataSync &&
		operation.Kind != KindExport {
		return Snapshot{}, fmt.Errorf("%w: unsupported operation kind", ErrInvalidTransition)
	}
	totalBytes, err := validateNewItems(operation.Kind, operation.Items)
	if err != nil {
		return Snapshot{}, err
	}
	var sourceType any
	var sourceShelfIDs any
	var sourceShelfNames any
	var destination any
	if operation.Kind == KindExport {
		if err := validateExportSource(operation.ExportSource); err != nil {
			return Snapshot{}, err
		}
		if strings.TrimSpace(operation.Destination) == "" {
			return Snapshot{}, fmt.Errorf(
				"%w: empty export destination",
				ErrInvalidTransition,
			)
		}
		ids, err := json.Marshal(nonNilInt64s(operation.ExportSource.ShelfIDs))
		if err != nil {
			return Snapshot{}, fmt.Errorf("encode export shelf IDs: %w", err)
		}
		names, err := json.Marshal(nonNilStrings(operation.ExportSource.ShelfNames))
		if err != nil {
			return Snapshot{}, fmt.Errorf("encode export shelf names: %w", err)
		}
		sourceType = operation.ExportSource.Type
		sourceShelfIDs = string(ids)
		sourceShelfNames = string(names)
		destination = operation.Destination
	} else {
		totalBytes = 0
	}
	transaction, err := r.database.BeginTx(ctx, nil)
	if err != nil {
		return Snapshot{}, fmt.Errorf("begin operation creation: %w", err)
	}
	defer transaction.Rollback()

	if _, err := transaction.ExecContext(
		ctx,
		`INSERT INTO file_operations (
			id,
			kind,
			status,
			export_source_type,
			export_source_shelf_ids,
			export_source_shelf_names,
			export_destination,
			total_items,
			total_bytes,
			created_at
		) VALUES (?, ?, 'queued', ?, ?, ?, ?, ?, ?, ?)`,
		id,
		operation.Kind,
		sourceType,
		sourceShelfIDs,
		sourceShelfNames,
		destination,
		len(operation.Items),
		totalBytes,
		formatTime(now),
	); err != nil {
		return Snapshot{}, fmt.Errorf(
			"insert %s operation: %w",
			operation.Kind,
			err,
		)
	}
	for _, item := range operation.Items {
		var storyID any
		if item.StoryID != 0 {
			storyID = item.StoryID
		}
		if _, err := transaction.ExecContext(
			ctx,
			`INSERT INTO file_operation_items (
				operation_id,
				story_id,
				resolved_story_id,
				story_uuid,
				story_title,
				source_name,
				output_name,
				archive_relative_path,
				archive_sha256,
				status,
				total_bytes
			) VALUES (?, ?, ?, NULLIF(?, ''), NULLIF(?, ''), ?, NULLIF(?, ''),
				NULLIF(?, ''), NULLIF(?, ''), 'pending', ?)`,
			id,
			storyID,
			storyID,
			item.StoryUUID,
			item.StoryTitle,
			item.SourceName,
			item.OutputName,
			item.ArchiveRelativePath,
			item.ArchiveSHA256,
			item.TotalBytes,
		); err != nil {
			return Snapshot{}, fmt.Errorf("insert operation item: %w", err)
		}
	}
	if err := transaction.Commit(); err != nil {
		return Snapshot{}, fmt.Errorf("commit operation creation: %w", err)
	}
	return r.Snapshot(ctx, id)
}

func validateNewItems(kind Kind, items []NewItem) (int64, error) {
	var totalBytes int64
	storyIDs := make(map[int64]struct{}, len(items))
	for _, item := range items {
		if strings.TrimSpace(item.SourceName) == "" || item.TotalBytes < 0 {
			return 0, fmt.Errorf(
				"%w: invalid operation item",
				ErrInvalidTransition,
			)
		}
		if item.TotalBytes > math.MaxInt64-totalBytes {
			return 0, fmt.Errorf(
				"%w: operation byte total overflow",
				ErrInvalidTransition,
			)
		}
		totalBytes += item.TotalBytes
		if kind != KindExport {
			continue
		}
		if item.StoryID <= 0 {
			return 0, fmt.Errorf(
				"%w: export story is missing",
				ErrInvalidTransition,
			)
		}
		if _, duplicate := storyIDs[item.StoryID]; duplicate {
			return 0, fmt.Errorf(
				"%w: duplicate export story",
				ErrInvalidTransition,
			)
		}
		storyIDs[item.StoryID] = struct{}{}
		if _, err := uuid.Parse(item.StoryUUID); err != nil ||
			strings.TrimSpace(item.StoryTitle) == "" ||
			!archive.ValidFilename(item.OutputName) ||
			!validRelativePath(item.ArchiveRelativePath) ||
			!validSHA256(item.ArchiveSHA256) {
			return 0, fmt.Errorf(
				"%w: invalid export operation item",
				ErrInvalidTransition,
			)
		}
	}
	return totalBytes, nil
}

func validateExportSource(source ExportSource) error {
	if len(source.ShelfIDs) != len(source.ShelfNames) {
		return fmt.Errorf(
			"%w: export shelf identity mismatch",
			ErrInvalidTransition,
		)
	}
	switch source.Type {
	case ExportSourceSelection, ExportSourceCurrentQuery:
		if len(source.ShelfIDs) != 0 {
			return fmt.Errorf(
				"%w: unexpected export shelves",
				ErrInvalidTransition,
			)
		}
	case ExportSourceShelf:
		if len(source.ShelfIDs) != 1 {
			return fmt.Errorf(
				"%w: single-shelf export identity is invalid",
				ErrInvalidTransition,
			)
		}
	case ExportSourceShelves:
		if len(source.ShelfIDs) < 2 {
			return fmt.Errorf(
				"%w: multi-shelf export identity is invalid",
				ErrInvalidTransition,
			)
		}
	default:
		return fmt.Errorf(
			"%w: export source type is invalid",
			ErrInvalidTransition,
		)
	}
	seen := make(map[int64]struct{}, len(source.ShelfIDs))
	for index, shelfID := range source.ShelfIDs {
		if shelfID <= 0 || strings.TrimSpace(source.ShelfNames[index]) == "" {
			return fmt.Errorf(
				"%w: export shelf identity is invalid",
				ErrInvalidTransition,
			)
		}
		if _, duplicate := seen[shelfID]; duplicate {
			return fmt.Errorf(
				"%w: duplicate export shelf",
				ErrInvalidTransition,
			)
		}
		seen[shelfID] = struct{}{}
	}
	return nil
}

func validRelativePath(value string) bool {
	return value != "" &&
		value == path.Clean(value) &&
		value != "." &&
		!strings.HasPrefix(value, "../") &&
		!strings.HasPrefix(value, "/")
}

func validSHA256(value string) bool {
	if len(value) != 64 || value != strings.ToLower(value) {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func nonNilInt64s(values []int64) []int64 {
	if values == nil {
		return []int64{}
	}
	return values
}

func nonNilStrings(values []string) []string {
	if values == nil {
		return []string{}
	}
	return values
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

func (r *Repository) UpdateItemProgress(
	ctx context.Context,
	operationID string,
	itemID int64,
	completedBytes int64,
) error {
	if completedBytes < 0 {
		return ErrInvalidTransition
	}
	result, err := r.database.ExecContext(
		ctx,
		`UPDATE file_operation_items
		 SET completed_bytes = ?
		 WHERE id = ?
		   AND operation_id = ?
		   AND status = 'running'
		   AND completed_bytes <= ?
		   AND ? <= total_bytes`,
		completedBytes,
		itemID,
		operationID,
		completedBytes,
		completedBytes,
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
	var sourceShelfIDs string
	var sourceShelfNames string
	err := r.database.QueryRowContext(
		ctx,
		`SELECT
			id,
			kind,
			status,
			COALESCE(export_source_type, ''),
			COALESCE(export_source_shelf_ids, ''),
			COALESCE(export_source_shelf_names, ''),
			COALESCE(export_destination, ''),
			completed_items,
			total_items,
			total_bytes,
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
		&snapshot.ExportSourceType,
		&sourceShelfIDs,
		&sourceShelfNames,
		&snapshot.Destination,
		&snapshot.CompletedItems,
		&snapshot.TotalItems,
		&snapshot.TotalBytes,
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
	if snapshot.Destination != "" {
		snapshot.DestinationLabel = filepath.Base(snapshot.Destination)
	}
	snapshot.StartedAt = startedAt.String
	snapshot.FinishedAt = finishedAt.String
	if sourceShelfIDs != "" {
		if err := json.Unmarshal(
			[]byte(sourceShelfIDs),
			&snapshot.SourceShelfIDs,
		); err != nil {
			return Snapshot{}, fmt.Errorf("decode export shelf IDs: %w", err)
		}
	}
	if sourceShelfNames != "" {
		if err := json.Unmarshal(
			[]byte(sourceShelfNames),
			&snapshot.SourceShelfNames,
		); err != nil {
			return Snapshot{}, fmt.Errorf("decode export shelf names: %w", err)
		}
	}

	rows, err := r.database.QueryContext(
		ctx,
		`SELECT
			id,
			COALESCE(resolved_story_id, story_id, 0),
			COALESCE(story_uuid, ''),
			COALESCE(story_title, ''),
			source_name,
			COALESCE(output_name, ''),
			COALESCE(archive_relative_path, ''),
			COALESCE(archive_sha256, ''),
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
			&item.StoryUUID,
			&item.StoryTitle,
			&item.SourceName,
			&item.OutputName,
			&item.ArchiveRelativePath,
			&item.ArchiveSHA256,
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

func (r *Repository) ActiveSnapshots(ctx context.Context) ([]Snapshot, error) {
	rows, err := r.database.QueryContext(
		ctx,
		`SELECT id
		 FROM file_operations
		 WHERE status IN ('queued', 'running')
		 ORDER BY created_at DESC, id DESC`,
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
		snapshot, err := r.Snapshot(ctx, id)
		if err != nil {
			return nil, err
		}
		snapshots = append(snapshots, snapshot)
	}
	return snapshots, nil
}

func (r *Repository) LatestTerminalExport(
	ctx context.Context,
) (Snapshot, bool, error) {
	var id string
	err := r.database.QueryRowContext(
		ctx,
		`SELECT id
		 FROM file_operations
		 WHERE kind = 'export'
		   AND status IN (
		       'succeeded',
		       'partially_succeeded',
		       'failed',
		       'cancelled',
		       'interrupted'
		   )
		 ORDER BY COALESCE(finished_at, created_at) DESC, id DESC
		 LIMIT 1`,
	).Scan(&id)
	if errors.Is(err, sql.ErrNoRows) {
		return Snapshot{}, false, nil
	}
	if err != nil {
		return Snapshot{}, false, err
	}
	snapshot, err := r.Snapshot(ctx, id)
	if err != nil {
		return Snapshot{}, false, err
	}
	return snapshot, true, nil
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
		active, err := r.Snapshot(ctx, id)
		if err != nil {
			return nil, err
		}
		code, message := interruptionOutcome(active.Kind)
		snapshot, err := r.Interrupt(
			ctx,
			id,
			code,
			message,
			now,
		)
		if err != nil {
			return nil, err
		}
		snapshots = append(snapshots, snapshot)
	}
	return snapshots, nil
}

func interruptionOutcome(kind Kind) (string, string) {
	switch kind {
	case KindImport:
		return "interrupted",
			"Import stopped when the application closed. Select the archive again to retry."
	case KindMetadataSync:
		return "interrupted",
			"Metadata refresh stopped when the application closed. Start a new refresh to retry."
	case KindExport:
		return "interrupted",
			"Export stopped when the application closed. Completed files were preserved; start a new export to retry."
	default:
		return "interrupted",
			"The operation stopped when the application closed. Start it again to retry."
	}
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
