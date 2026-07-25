package catalog

import (
	"context"
	"database/sql"
	"fmt"
)

type ArchiveFormat string

const (
	FormatPlainPK   ArchiveFormat = "plain_pk"
	FormatV1PK      ArchiveFormat = "v1_pk"
	FormatV2PK      ArchiveFormat = "v2_pk"
	FormatGenericPK ArchiveFormat = "generic_pk"
	FormatZIP       ArchiveFormat = "zip"
	FormatSevenZIP  ArchiveFormat = "seven_zip"
	FormatStudioZIP ArchiveFormat = "studio_zip"
)

type Story struct {
	ID                  int64
	UUID                string
	EmbeddedTitle       string
	EmbeddedDescription string
	EmbeddedArtworkPath string
}

type StoryArchive struct {
	ID               int64
	StoryID          int64
	OriginalFilename string
	DetectedFormat   ArchiveFormat
	SHA256           string
	ByteSize         int64
	ManagedPath      string
	ValidationState  string
}

type CreateStory struct {
	UUID                string
	EmbeddedTitle       string
	EmbeddedDescription string
	EmbeddedArtworkPath string
	OriginalFilename    string
	DetectedFormat      ArchiveFormat
	SHA256              string
	ByteSize            int64
	ManagedPath         string
}

type Repository struct {
	database *sql.DB
}

func NewRepository(database *sql.DB) *Repository {
	return &Repository{database: database}
}

func (r *Repository) Create(ctx context.Context, input CreateStory) (Story, StoryArchive, error) {
	transaction, err := r.database.BeginTx(ctx, nil)
	if err != nil {
		return Story{}, StoryArchive{}, fmt.Errorf("begin story creation: %w", err)
	}
	defer transaction.Rollback()

	result, err := transaction.ExecContext(
		ctx,
		`INSERT INTO stories (
			uuid, embedded_title, embedded_description, embedded_artwork_path
		) VALUES (?, NULLIF(?, ''), NULLIF(?, ''), NULLIF(?, ''))`,
		input.UUID,
		input.EmbeddedTitle,
		input.EmbeddedDescription,
		input.EmbeddedArtworkPath,
	)
	if err != nil {
		return Story{}, StoryArchive{}, fmt.Errorf("insert story: %w", err)
	}
	storyID, err := result.LastInsertId()
	if err != nil {
		return Story{}, StoryArchive{}, fmt.Errorf("read story id: %w", err)
	}

	result, err = transaction.ExecContext(
		ctx,
		`INSERT INTO story_archives (
			story_id, original_filename, detected_format, sha256, byte_size, managed_path
		) VALUES (?, ?, ?, ?, ?, ?)`,
		storyID,
		input.OriginalFilename,
		input.DetectedFormat,
		input.SHA256,
		input.ByteSize,
		input.ManagedPath,
	)
	if err != nil {
		return Story{}, StoryArchive{}, fmt.Errorf("insert story archive: %w", err)
	}
	archiveID, err := result.LastInsertId()
	if err != nil {
		return Story{}, StoryArchive{}, fmt.Errorf("read story archive id: %w", err)
	}

	if err := transaction.Commit(); err != nil {
		return Story{}, StoryArchive{}, fmt.Errorf("commit story creation: %w", err)
	}

	return Story{
			ID:                  storyID,
			UUID:                input.UUID,
			EmbeddedTitle:       input.EmbeddedTitle,
			EmbeddedDescription: input.EmbeddedDescription,
			EmbeddedArtworkPath: input.EmbeddedArtworkPath,
		}, StoryArchive{
			ID:               archiveID,
			StoryID:          storyID,
			OriginalFilename: input.OriginalFilename,
			DetectedFormat:   input.DetectedFormat,
			SHA256:           input.SHA256,
			ByteSize:         input.ByteSize,
			ManagedPath:      input.ManagedPath,
			ValidationState:  "valid",
		}, nil
}

func (r *Repository) FindByUUID(ctx context.Context, uuid string) (Story, StoryArchive, error) {
	return r.find(ctx, "s.uuid = ?", uuid)
}

func (r *Repository) FindByChecksum(ctx context.Context, checksum string) (Story, StoryArchive, error) {
	return r.find(ctx, "a.sha256 = ?", checksum)
}

func (r *Repository) find(ctx context.Context, predicate string, value string) (Story, StoryArchive, error) {
	var story Story
	var archive StoryArchive
	err := r.database.QueryRowContext(
		ctx,
		`SELECT
			s.id,
			s.uuid,
			COALESCE(s.embedded_title, ''),
			COALESCE(s.embedded_description, ''),
			COALESCE(s.embedded_artwork_path, ''),
			a.id,
			a.original_filename,
			a.detected_format,
			a.sha256,
			a.byte_size,
			a.managed_path,
			a.validation_state
		FROM stories s
		JOIN story_archives a ON a.story_id = s.id
		WHERE `+predicate,
		value,
	).Scan(
		&story.ID,
		&story.UUID,
		&story.EmbeddedTitle,
		&story.EmbeddedDescription,
		&story.EmbeddedArtworkPath,
		&archive.ID,
		&archive.OriginalFilename,
		&archive.DetectedFormat,
		&archive.SHA256,
		&archive.ByteSize,
		&archive.ManagedPath,
		&archive.ValidationState,
	)
	archive.StoryID = story.ID
	if err != nil {
		return Story{}, StoryArchive{}, err
	}
	return story, archive, nil
}
