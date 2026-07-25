package metadata

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"
)

type SyncStatus string

const (
	SyncRunning   SyncStatus = "running"
	SyncSucceeded SyncStatus = "succeeded"
	SyncFailed    SyncStatus = "failed"
	SyncCancelled SyncStatus = "cancelled"
)

type SnapshotStatus string

const (
	SnapshotStaged     SnapshotStatus = "staged"
	SnapshotActive     SnapshotStatus = "active"
	SnapshotSuperseded SnapshotStatus = "superseded"
	SnapshotRejected   SnapshotStatus = "rejected"
)

const ProvenanceLuniiCatalog = "lunii_catalog"

var ErrInvalidTransition = errors.New("invalid catalog repository transition")

type CatalogSync struct {
	ID                string
	Locale            string
	Status            SyncStatus
	MatchedStoryCount int
	ErrorCode         string
	ErrorMessage      string
	StartedAt         string
	FinishedAt        string
}

type CatalogSnapshot struct {
	ID          int64
	SyncID      string
	Locale      string
	RawPath     string
	RawSHA256   string
	ByteSize    int64
	RecordCount int
	Status      SnapshotStatus
	FetchedAt   string
	ActivatedAt string
}

type CatalogArtwork struct {
	ID           string
	SourceURL    string
	ManagedPath  string
	ContentType  string
	SHA256       string
	ByteSize     int64
	ETag         string
	LastModified string
	CachedAt     string
}

type OfficialStoryMetadata struct {
	ID              int64
	SnapshotID      int64
	StoryUUID       string
	Locale          string
	Title           string
	Description     string
	Author          string
	Publisher       string
	Language        string
	DurationSeconds *int
	MinimumAge      *int
	MaximumAge      *int
	ArtworkID       string
	Provenance      string
	SourceRecordID  string
	SourceUpdatedAt string
	FetchedAt       string
	ActivatedAt     string
}

type NewCatalogSync struct {
	ID        string
	Locale    string
	StartedAt time.Time
}

type NewCatalogSnapshot struct {
	SyncID    string
	Locale    string
	RawPath   string
	RawSHA256 string
	ByteSize  int64
	FetchedAt time.Time
	Stories   []NewOfficialStoryMetadata
	Artworks  []NewCatalogArtwork
}

type NewCatalogArtwork struct {
	ID        string
	SourceURL string
}

type NewOfficialStoryMetadata struct {
	StoryUUID       string
	Title           string
	Description     string
	Author          string
	Publisher       string
	Language        string
	DurationSeconds *int
	MinimumAge      *int
	MaximumAge      *int
	ArtworkID       string
	Provenance      string
	SourceRecordID  string
	SourceUpdatedAt *time.Time
}

type Repository struct {
	database *sql.DB
}

func NewRepository(database *sql.DB) *Repository {
	return &Repository{database: database}
}

func (r *Repository) CreateSync(
	ctx context.Context,
	input NewCatalogSync,
) (CatalogSync, error) {
	_, err := r.database.ExecContext(
		ctx,
		`INSERT INTO catalog_syncs (id, locale, started_at)
		 VALUES (?, ?, ?)`,
		input.ID,
		input.Locale,
		formatTime(input.StartedAt),
	)
	if err != nil {
		return CatalogSync{}, fmt.Errorf("insert catalog sync: %w", err)
	}
	return r.Sync(ctx, input.ID)
}

func (r *Repository) Sync(ctx context.Context, id string) (CatalogSync, error) {
	var sync CatalogSync
	err := r.database.QueryRowContext(
		ctx,
		`SELECT
			id,
			locale,
			status,
			matched_story_count,
			COALESCE(error_code, ''),
			COALESCE(error_message, ''),
			started_at,
			COALESCE(finished_at, '')
		 FROM catalog_syncs
		 WHERE id = ?`,
		id,
	).Scan(
		&sync.ID,
		&sync.Locale,
		&sync.Status,
		&sync.MatchedStoryCount,
		&sync.ErrorCode,
		&sync.ErrorMessage,
		&sync.StartedAt,
		&sync.FinishedAt,
	)
	if err != nil {
		return CatalogSync{}, err
	}
	return sync, nil
}

func (r *Repository) StageSnapshot(
	ctx context.Context,
	input NewCatalogSnapshot,
) (CatalogSnapshot, error) {
	transaction, err := r.database.BeginTx(ctx, nil)
	if err != nil {
		return CatalogSnapshot{}, fmt.Errorf("begin catalog snapshot staging: %w", err)
	}
	defer transaction.Rollback()

	result, err := transaction.ExecContext(
		ctx,
		`INSERT INTO catalog_snapshots (
			sync_id,
			locale,
			raw_path,
			raw_sha256,
			byte_size,
			record_count,
			fetched_at
		 ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
		input.SyncID,
		input.Locale,
		input.RawPath,
		input.RawSHA256,
		input.ByteSize,
		len(input.Stories),
		formatTime(input.FetchedAt),
	)
	if err != nil {
		return CatalogSnapshot{}, fmt.Errorf("insert catalog snapshot: %w", err)
	}
	snapshotID, err := result.LastInsertId()
	if err != nil {
		return CatalogSnapshot{}, fmt.Errorf("read catalog snapshot id: %w", err)
	}

	for _, artwork := range input.Artworks {
		if _, err := transaction.ExecContext(
			ctx,
			`INSERT INTO catalog_artworks (id, source_url)
			 VALUES (?, ?)
			 ON CONFLICT(id) DO NOTHING`,
			artwork.ID,
			artwork.SourceURL,
		); err != nil {
			return CatalogSnapshot{}, fmt.Errorf("register catalog artwork %s: %w", artwork.ID, err)
		}
		var sourceURL string
		if err := transaction.QueryRowContext(
			ctx,
			"SELECT source_url FROM catalog_artworks WHERE id = ?",
			artwork.ID,
		).Scan(&sourceURL); err != nil {
			return CatalogSnapshot{}, fmt.Errorf("verify catalog artwork %s: %w", artwork.ID, err)
		}
		if sourceURL != artwork.SourceURL {
			return CatalogSnapshot{}, fmt.Errorf("catalog artwork identity collision")
		}
	}
	for _, story := range input.Stories {
		if err := insertOfficialMetadata(ctx, transaction, snapshotID, input.Locale, story); err != nil {
			return CatalogSnapshot{}, err
		}
	}

	if err := transaction.Commit(); err != nil {
		return CatalogSnapshot{}, fmt.Errorf("commit catalog snapshot staging: %w", err)
	}
	return r.Snapshot(ctx, snapshotID)
}

func (r *Repository) Artwork(ctx context.Context, id string) (CatalogArtwork, error) {
	var artwork CatalogArtwork
	err := r.database.QueryRowContext(
		ctx,
		`SELECT
			id,
			source_url,
			COALESCE(managed_path, ''),
			COALESCE(content_type, ''),
			COALESCE(sha256, ''),
			COALESCE(byte_size, 0),
			COALESCE(etag, ''),
			COALESCE(last_modified, ''),
			COALESCE(cached_at, '')
		 FROM catalog_artworks
		 WHERE id = ?`,
		id,
	).Scan(
		&artwork.ID,
		&artwork.SourceURL,
		&artwork.ManagedPath,
		&artwork.ContentType,
		&artwork.SHA256,
		&artwork.ByteSize,
		&artwork.ETag,
		&artwork.LastModified,
		&artwork.CachedAt,
	)
	if err != nil {
		return CatalogArtwork{}, err
	}
	return artwork, nil
}

func (r *Repository) CacheArtwork(
	ctx context.Context,
	id string,
	managedPath string,
	contentType string,
	sha256 string,
	byteSize int64,
	etag string,
	lastModified string,
	cachedAt time.Time,
) error {
	result, err := r.database.ExecContext(
		ctx,
		`UPDATE catalog_artworks
		 SET managed_path = ?,
		     content_type = ?,
		     sha256 = ?,
		     byte_size = ?,
		     etag = NULLIF(?, ''),
		     last_modified = NULLIF(?, ''),
		     cached_at = ?,
		     updated_at = ?
		 WHERE id = ?`,
		managedPath,
		contentType,
		sha256,
		byteSize,
		etag,
		lastModified,
		formatTime(cachedAt),
		formatTime(cachedAt),
		id,
	)
	return transitionResult(result, err)
}

func insertOfficialMetadata(
	ctx context.Context,
	transaction *sql.Tx,
	snapshotID int64,
	locale string,
	input NewOfficialStoryMetadata,
) error {
	provenance := input.Provenance
	if provenance == "" {
		provenance = ProvenanceLuniiCatalog
	}
	_, err := transaction.ExecContext(
		ctx,
		`INSERT INTO official_story_metadata (
			snapshot_id,
			story_uuid,
			locale,
			title,
			description,
			author,
			publisher,
			language,
			duration_seconds,
			minimum_age,
			maximum_age,
			artwork_id,
			provenance,
			source_record_id,
			source_updated_at
		 ) VALUES (
			?, ?, ?,
			NULLIF(?, ''), NULLIF(?, ''), NULLIF(?, ''),
			NULLIF(?, ''), NULLIF(?, ''), ?, ?, ?,
			NULLIF(?, ''), ?, NULLIF(?, ''), ?
		 )`,
		snapshotID,
		input.StoryUUID,
		locale,
		input.Title,
		input.Description,
		input.Author,
		input.Publisher,
		input.Language,
		input.DurationSeconds,
		input.MinimumAge,
		input.MaximumAge,
		input.ArtworkID,
		provenance,
		input.SourceRecordID,
		formatOptionalTime(input.SourceUpdatedAt),
	)
	if err != nil {
		return fmt.Errorf("insert official metadata for %s: %w", input.StoryUUID, err)
	}
	return nil
}

func (r *Repository) Snapshot(ctx context.Context, id int64) (CatalogSnapshot, error) {
	return scanSnapshot(r.database.QueryRowContext(
		ctx,
		`SELECT
			id,
			sync_id,
			locale,
			raw_path,
			raw_sha256,
			byte_size,
			record_count,
			status,
			fetched_at,
			COALESCE(activated_at, '')
		 FROM catalog_snapshots
		 WHERE id = ?`,
		id,
	))
}

func (r *Repository) ActiveSnapshot(
	ctx context.Context,
	locale string,
) (CatalogSnapshot, error) {
	return scanSnapshot(r.database.QueryRowContext(
		ctx,
		`SELECT
			id,
			sync_id,
			locale,
			raw_path,
			raw_sha256,
			byte_size,
			record_count,
			status,
			fetched_at,
			COALESCE(activated_at, '')
		 FROM catalog_snapshots
		 WHERE locale = ? AND status = 'active'`,
		locale,
	))
}

func (r *Repository) MetadataForSnapshot(
	ctx context.Context,
	snapshotID int64,
) ([]OfficialStoryMetadata, error) {
	rows, err := r.database.QueryContext(
		ctx,
		officialMetadataSelect+`
		 WHERE metadata.snapshot_id = ?
		 ORDER BY metadata.story_uuid`,
		snapshotID,
	)
	if err != nil {
		return nil, fmt.Errorf("query official metadata: %w", err)
	}
	defer rows.Close()

	metadata := make([]OfficialStoryMetadata, 0)
	for rows.Next() {
		story, err := scanOfficialMetadata(rows)
		if err != nil {
			return nil, err
		}
		metadata = append(metadata, story)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate official metadata: %w", err)
	}
	return metadata, nil
}

func (r *Repository) ActiveMetadataByUUID(
	ctx context.Context,
	locale string,
	storyUUID string,
) (OfficialStoryMetadata, error) {
	return scanOfficialMetadata(r.database.QueryRowContext(
		ctx,
		officialMetadataSelect+`
		 WHERE snapshot.locale = ?
		   AND snapshot.status = 'active'
		   AND metadata.story_uuid = ?`,
		locale,
		storyUUID,
	))
}

func (r *Repository) ActiveMetadataByUUIDs(
	ctx context.Context,
	locale string,
	storyUUIDs []string,
) (map[string]OfficialStoryMetadata, error) {
	if len(storyUUIDs) == 0 {
		return map[string]OfficialStoryMetadata{}, nil
	}
	unique := make([]string, 0, len(storyUUIDs))
	seen := make(map[string]struct{}, len(storyUUIDs))
	for _, storyUUID := range storyUUIDs {
		if _, exists := seen[storyUUID]; exists {
			continue
		}
		seen[storyUUID] = struct{}{}
		unique = append(unique, storyUUID)
	}
	placeholders := strings.TrimSuffix(strings.Repeat("?,", len(unique)), ",")
	arguments := make([]any, 0, len(unique)+1)
	arguments = append(arguments, locale)
	for _, storyUUID := range unique {
		arguments = append(arguments, storyUUID)
	}
	rows, err := r.database.QueryContext(
		ctx,
		officialMetadataSelect+`
		 WHERE snapshot.locale = ?
		   AND snapshot.status = 'active'
		   AND metadata.story_uuid IN (`+placeholders+`)
		 ORDER BY metadata.story_uuid`,
		arguments...,
	)
	if err != nil {
		return nil, fmt.Errorf("query active official metadata: %w", err)
	}
	defer rows.Close()

	result := make(map[string]OfficialStoryMetadata, len(unique))
	for rows.Next() {
		story, err := scanOfficialMetadata(rows)
		if err != nil {
			return nil, err
		}
		result[story.StoryUUID] = story
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate active official metadata: %w", err)
	}
	return result, nil
}

func (r *Repository) ActivateSnapshot(
	ctx context.Context,
	snapshotID int64,
	matchedStoryCount int,
	activatedAt time.Time,
) error {
	transaction, err := r.database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin catalog snapshot activation: %w", err)
	}
	defer transaction.Rollback()

	var syncID string
	var locale string
	var status SnapshotStatus
	err = transaction.QueryRowContext(
		ctx,
		`SELECT sync_id, locale, status
		 FROM catalog_snapshots
		 WHERE id = ?`,
		snapshotID,
	).Scan(&syncID, &locale, &status)
	if err != nil {
		return err
	}
	if status != SnapshotStaged || matchedStoryCount < 0 {
		return ErrInvalidTransition
	}

	if _, err := transaction.ExecContext(
		ctx,
		`UPDATE catalog_snapshots
		 SET status = 'superseded'
		 WHERE locale = ? AND status = 'active'`,
		locale,
	); err != nil {
		return fmt.Errorf("supersede active catalog snapshot: %w", err)
	}

	result, err := transaction.ExecContext(
		ctx,
		`UPDATE catalog_snapshots
		 SET status = 'active', activated_at = ?
		 WHERE id = ? AND status = 'staged'`,
		formatTime(activatedAt),
		snapshotID,
	)
	if err := transitionResult(result, err); err != nil {
		return err
	}

	result, err = transaction.ExecContext(
		ctx,
		`UPDATE catalog_syncs
		 SET status = 'succeeded',
		     matched_story_count = ?,
		     finished_at = ?
		 WHERE id = ? AND status = 'running'`,
		matchedStoryCount,
		formatTime(activatedAt),
		syncID,
	)
	if err := transitionResult(result, err); err != nil {
		return err
	}

	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("commit catalog snapshot activation: %w", err)
	}
	return nil
}

func (r *Repository) FinishSyncFailure(
	ctx context.Context,
	syncID string,
	snapshotID int64,
	status SyncStatus,
	errorCode string,
	errorMessage string,
	finishedAt time.Time,
) error {
	if (status != SyncFailed && status != SyncCancelled) ||
		errorCode == "" ||
		errorMessage == "" {
		return ErrInvalidTransition
	}
	transaction, err := r.database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin catalog sync failure: %w", err)
	}
	defer transaction.Rollback()

	if snapshotID != 0 {
		result, err := transaction.ExecContext(
			ctx,
			`UPDATE catalog_snapshots
			 SET status = 'rejected'
			 WHERE id = ? AND sync_id = ? AND status = 'staged'`,
			snapshotID,
			syncID,
		)
		if err := transitionResult(result, err); err != nil {
			return err
		}
	}
	result, err := transaction.ExecContext(
		ctx,
		`UPDATE catalog_syncs
		 SET status = ?,
		     error_code = ?,
		     error_message = ?,
		     finished_at = ?
		 WHERE id = ? AND status = 'running'`,
		status,
		errorCode,
		errorMessage,
		formatTime(finishedAt),
		syncID,
	)
	if err := transitionResult(result, err); err != nil {
		return err
	}
	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("commit catalog sync failure: %w", err)
	}
	return nil
}

const officialMetadataSelect = `
	SELECT
		metadata.id,
		metadata.snapshot_id,
		metadata.story_uuid,
		metadata.locale,
		COALESCE(metadata.title, ''),
		COALESCE(metadata.description, ''),
		COALESCE(metadata.author, ''),
		COALESCE(metadata.publisher, ''),
		COALESCE(metadata.language, ''),
		metadata.duration_seconds,
		metadata.minimum_age,
		metadata.maximum_age,
		COALESCE(metadata.artwork_id, ''),
		metadata.provenance,
		COALESCE(metadata.source_record_id, ''),
		COALESCE(metadata.source_updated_at, ''),
		snapshot.fetched_at,
		COALESCE(snapshot.activated_at, '')
	FROM official_story_metadata metadata
	JOIN catalog_snapshots snapshot ON snapshot.id = metadata.snapshot_id`

type scanner interface {
	Scan(dest ...any) error
}

func scanSnapshot(row scanner) (CatalogSnapshot, error) {
	var snapshot CatalogSnapshot
	err := row.Scan(
		&snapshot.ID,
		&snapshot.SyncID,
		&snapshot.Locale,
		&snapshot.RawPath,
		&snapshot.RawSHA256,
		&snapshot.ByteSize,
		&snapshot.RecordCount,
		&snapshot.Status,
		&snapshot.FetchedAt,
		&snapshot.ActivatedAt,
	)
	if err != nil {
		return CatalogSnapshot{}, err
	}
	return snapshot, nil
}

func scanOfficialMetadata(row scanner) (OfficialStoryMetadata, error) {
	var metadata OfficialStoryMetadata
	var duration sql.NullInt64
	var minimumAge sql.NullInt64
	var maximumAge sql.NullInt64
	err := row.Scan(
		&metadata.ID,
		&metadata.SnapshotID,
		&metadata.StoryUUID,
		&metadata.Locale,
		&metadata.Title,
		&metadata.Description,
		&metadata.Author,
		&metadata.Publisher,
		&metadata.Language,
		&duration,
		&minimumAge,
		&maximumAge,
		&metadata.ArtworkID,
		&metadata.Provenance,
		&metadata.SourceRecordID,
		&metadata.SourceUpdatedAt,
		&metadata.FetchedAt,
		&metadata.ActivatedAt,
	)
	if err != nil {
		return OfficialStoryMetadata{}, err
	}
	metadata.DurationSeconds = optionalInt(duration)
	metadata.MinimumAge = optionalInt(minimumAge)
	metadata.MaximumAge = optionalInt(maximumAge)
	return metadata, nil
}

func optionalInt(value sql.NullInt64) *int {
	if !value.Valid {
		return nil
	}
	converted := int(value.Int64)
	return &converted
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

func formatOptionalTime(value *time.Time) any {
	if value == nil {
		return nil
	}
	return formatTime(*value)
}
