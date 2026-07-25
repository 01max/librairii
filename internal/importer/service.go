package importer

import (
	"context"
	"database/sql"
	"errors"
	"fmt"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/inspection"
)

type OutcomeCode string

const (
	OutcomeImported          OutcomeCode = "imported"
	OutcomeDuplicateChecksum OutcomeCode = "duplicate_checksum"
	OutcomeUUIDConflict      OutcomeCode = "uuid_conflict"
)

type ErrorCode string

const (
	ErrorStage      ErrorCode = "stage_failed"
	ErrorLookup     ErrorCode = "lookup_failed"
	ErrorInspect    ErrorCode = "inspection_failed"
	ErrorPublish    ErrorCode = "publish_failed"
	ErrorArtwork    ErrorCode = "artwork_failed"
	ErrorCatalog    ErrorCode = "catalog_failed"
	ErrorCompensate ErrorCode = "compensation_failed"
	ErrorCleanup    ErrorCode = "cleanup_failed"
	ErrorCancelled  ErrorCode = "cancelled"
)

type Error struct {
	Code  ErrorCode
	Cause error
}

func (e *Error) Error() string {
	return fmt.Sprintf("%s: %v", e.Code, e.Cause)
}

func (e *Error) Unwrap() error {
	return e.Cause
}

type Outcome struct {
	Code            OutcomeCode
	UUID            string
	StoryID         int64
	ArchiveID       int64
	ExistingStoryID int64
	Checksum        string
}

type archiveStore interface {
	Stage(context.Context, string) (archive.StagedFile, error)
	Publish(archive.StagedFile) (string, error)
	Cleanup(archive.StagedFile) error
	MoveToTrash(string) (string, error)
}

type artworkStore interface {
	Publish(string, string, []byte) (string, error)
	Remove(string) error
}

type storyCatalog interface {
	Create(context.Context, catalog.CreateStory) (catalog.Story, catalog.StoryArchive, error)
	FindByUUID(context.Context, string) (catalog.Story, catalog.StoryArchive, error)
	FindByChecksum(context.Context, string) (catalog.Story, catalog.StoryArchive, error)
}

type Service struct {
	archives  archiveStore
	artwork   artworkStore
	catalog   storyCatalog
	inspector inspection.Inspector
	limits    inspection.Limits
}

func NewService(
	archives archiveStore,
	artwork artworkStore,
	catalog storyCatalog,
	inspector inspection.Inspector,
	limits inspection.Limits,
) (*Service, error) {
	if archives == nil || artwork == nil || catalog == nil || inspector == nil {
		return nil, fmt.Errorf("import service dependency is nil")
	}
	if err := limits.Validate(); err != nil {
		return nil, err
	}
	return &Service{
		archives:  archives,
		artwork:   artwork,
		catalog:   catalog,
		inspector: inspector,
		limits:    limits,
	}, nil
}

func (s *Service) Import(ctx context.Context, sourcePath string) (outcome Outcome, err error) {
	staged, err := s.archives.Stage(ctx, sourcePath)
	if err != nil {
		return Outcome{}, importError(ctx, ErrorStage, err)
	}
	stagingActive := true
	defer func() {
		if !stagingActive {
			return
		}
		if cleanupErr := s.archives.Cleanup(staged); cleanupErr != nil {
			err = errors.Join(err, &Error{Code: ErrorCleanup, Cause: cleanupErr})
		}
	}()

	existingStory, existingArchive, err := s.catalog.FindByChecksum(ctx, staged.SHA256)
	switch {
	case err == nil:
		return Outcome{
			Code:            OutcomeDuplicateChecksum,
			UUID:            existingStory.UUID,
			StoryID:         existingStory.ID,
			ArchiveID:       existingArchive.ID,
			ExistingStoryID: existingStory.ID,
			Checksum:        existingArchive.SHA256,
		}, nil
	case !errors.Is(err, sql.ErrNoRows):
		return Outcome{}, importError(ctx, ErrorLookup, err)
	}

	inspected, err := s.inspector.Inspect(ctx, inspection.Candidate{
		Path:             staged.Path,
		OriginalFilename: staged.OriginalFilename,
	}, s.limits)
	if err != nil {
		return Outcome{}, importError(ctx, ErrorInspect, err)
	}
	if err := ctx.Err(); err != nil {
		return Outcome{}, importError(ctx, ErrorCancelled, err)
	}

	existingStory, existingArchive, err = s.catalog.FindByUUID(ctx, inspected.UUID)
	switch {
	case err == nil:
		return Outcome{
			Code:            OutcomeUUIDConflict,
			UUID:            inspected.UUID,
			StoryID:         existingStory.ID,
			ArchiveID:       existingArchive.ID,
			ExistingStoryID: existingStory.ID,
			Checksum:        staged.SHA256,
		}, nil
	case !errors.Is(err, sql.ErrNoRows):
		return Outcome{}, importError(ctx, ErrorLookup, err)
	}

	managedPath, err := s.archives.Publish(staged)
	if err != nil {
		return Outcome{}, importError(ctx, ErrorPublish, err)
	}
	stagingActive = false

	artworkPath := ""
	if inspected.Metadata.Artwork != nil {
		artworkPath, err = s.artwork.Publish(
			inspected.UUID,
			inspected.Metadata.Artwork.MediaType,
			inspected.Metadata.Artwork.Bytes,
		)
		if err != nil {
			return Outcome{}, s.compensate(managedPath, "", ErrorArtwork, err)
		}
	}
	if err := ctx.Err(); err != nil {
		return Outcome{}, s.compensate(managedPath, artworkPath, ErrorCancelled, err)
	}

	story, storyArchive, err := s.catalog.Create(ctx, catalog.CreateStory{
		UUID:                inspected.UUID,
		EmbeddedTitle:       inspected.Metadata.Title,
		EmbeddedDescription: inspected.Metadata.Description,
		EmbeddedArtworkPath: artworkPath,
		OriginalFilename:    staged.OriginalFilename,
		DetectedFormat:      inspected.Format,
		SHA256:              staged.SHA256,
		ByteSize:            staged.ByteSize,
		ManagedPath:         managedPath,
	})
	if err != nil {
		return Outcome{}, s.compensate(managedPath, artworkPath, ErrorCatalog, err)
	}

	return Outcome{
		Code:      OutcomeImported,
		UUID:      story.UUID,
		StoryID:   story.ID,
		ArchiveID: storyArchive.ID,
		Checksum:  storyArchive.SHA256,
	}, nil
}

func (s *Service) compensate(
	managedPath string,
	artworkPath string,
	code ErrorCode,
	cause error,
) error {
	var compensationErrors []error
	if artworkPath != "" {
		if err := s.artwork.Remove(artworkPath); err != nil {
			compensationErrors = append(compensationErrors, err)
		}
	}
	if _, err := s.archives.MoveToTrash(managedPath); err != nil {
		compensationErrors = append(compensationErrors, err)
	}
	base := &Error{Code: code, Cause: cause}
	if len(compensationErrors) == 0 {
		return base
	}
	return errors.Join(
		base,
		&Error{Code: ErrorCompensate, Cause: errors.Join(compensationErrors...)},
	)
}

func importError(ctx context.Context, code ErrorCode, cause error) error {
	if ctxErr := ctx.Err(); ctxErr != nil && errors.Is(cause, ctxErr) {
		return &Error{Code: ErrorCancelled, Cause: ctxErr}
	}
	return &Error{Code: code, Cause: cause}
}

func ErrorHasCode(err error, code ErrorCode) bool {
	if err == nil {
		return false
	}
	var importError *Error
	if errors.As(err, &importError) && importError.Code == code {
		return true
	}
	if joined, ok := err.(interface{ Unwrap() []error }); ok {
		for _, child := range joined.Unwrap() {
			if ErrorHasCode(child, code) {
				return true
			}
		}
		return false
	}
	return ErrorHasCode(errors.Unwrap(err), code)
}
