package metadata

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/google/uuid"
)

type RefreshErrorCode string

const (
	RefreshFetchFailed       RefreshErrorCode = "catalog_fetch_failed"
	RefreshInvalidCatalog    RefreshErrorCode = "catalog_invalid"
	RefreshStagingFailed     RefreshErrorCode = "catalog_stage_failed"
	RefreshPersistenceFailed RefreshErrorCode = "catalog_persist_failed"
	RefreshCancelled         RefreshErrorCode = "catalog_cancelled"
)

type RefreshError struct {
	Code  RefreshErrorCode
	Cause error
}

func (e *RefreshError) Error() string {
	return fmt.Sprintf("%s: %v", e.Code, e.Cause)
}

func (e *RefreshError) Unwrap() error {
	return e.Cause
}

type RefreshResult struct {
	Sync       CatalogSync
	Snapshot   CatalogSnapshot
	StoryCount int
}

type CatalogFetcher interface {
	FetchCatalog(context.Context) ([]byte, error)
}

type refreshRepository interface {
	CreateSync(context.Context, NewCatalogSync) (CatalogSync, error)
	StageSnapshot(context.Context, NewCatalogSnapshot) (CatalogSnapshot, error)
	CountMatchingStories(context.Context, int64) (int, error)
	ActivateSnapshot(context.Context, int64, int, time.Time) error
	FinishSyncFailure(
		context.Context,
		string,
		int64,
		SyncStatus,
		string,
		string,
		time.Time,
	) error
	Sync(context.Context, string) (CatalogSync, error)
	Snapshot(context.Context, int64) (CatalogSnapshot, error)
}

type rawSnapshotStore interface {
	Stage(context.Context, string, []byte) (StagedRawSnapshot, error)
	Publish(StagedRawSnapshot) (string, error)
	Cleanup(StagedRawSnapshot) error
	Remove(string) error
}

type RefreshService struct {
	fetcher    CatalogFetcher
	repository refreshRepository
	store      rawSnapshotStore
	now        func() time.Time
	newID      func() string
}

func NewRefreshService(
	fetcher CatalogFetcher,
	repository refreshRepository,
	store rawSnapshotStore,
) (*RefreshService, error) {
	if fetcher == nil || repository == nil || store == nil {
		return nil, errors.New("catalog refresh dependency is nil")
	}
	return &RefreshService{
		fetcher:    fetcher,
		repository: repository,
		store:      store,
		now:        time.Now,
		newID:      uuid.NewString,
	}, nil
}

func (s *RefreshService) Refresh(
	ctx context.Context,
	locale string,
) (RefreshResult, error) {
	canonical, err := canonicalLocale(locale)
	if err != nil {
		return RefreshResult{}, &RefreshError{
			Code:  RefreshInvalidCatalog,
			Cause: err,
		}
	}
	locale = canonical
	startedAt := s.now().UTC()
	sync, err := s.repository.CreateSync(ctx, NewCatalogSync{
		ID:        s.newID(),
		Locale:    locale,
		StartedAt: startedAt,
	})
	if err != nil {
		return RefreshResult{}, &RefreshError{
			Code:  refreshCode(ctx, RefreshPersistenceFailed),
			Cause: err,
		}
	}

	payload, err := s.fetcher.FetchCatalog(ctx)
	if err != nil {
		return RefreshResult{}, s.fail(ctx, sync.ID, 0, refreshCode(ctx, RefreshFetchFailed), err)
	}
	staged, err := s.store.Stage(ctx, sync.ID, payload)
	if err != nil {
		return RefreshResult{}, s.fail(ctx, sync.ID, 0, refreshCode(ctx, RefreshStagingFailed), err)
	}
	stagingActive := true
	defer func() {
		if stagingActive {
			_ = s.store.Cleanup(staged)
		}
	}()

	normalized, err := NormalizeCatalogSnapshot(payload, locale)
	if err != nil {
		return RefreshResult{}, s.fail(ctx, sync.ID, 0, RefreshInvalidCatalog, err)
	}
	if err := ctx.Err(); err != nil {
		return RefreshResult{}, s.fail(ctx, sync.ID, 0, RefreshCancelled, err)
	}

	rawPath, err := s.store.Publish(staged)
	if err != nil {
		return RefreshResult{}, s.fail(ctx, sync.ID, 0, RefreshStagingFailed, err)
	}
	stagingActive = false
	snapshot, err := s.repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    sync.ID,
		Locale:    sync.Locale,
		RawPath:   rawPath,
		RawSHA256: staged.SHA256,
		ByteSize:  staged.ByteSize,
		FetchedAt: s.now().UTC(),
		Stories:   normalized.Stories,
		Artworks:  normalized.Artworks,
	})
	if err != nil {
		removeErr := s.store.Remove(rawPath)
		if removeErr != nil {
			err = errors.Join(err, removeErr)
		}
		return RefreshResult{}, s.fail(ctx, sync.ID, 0, RefreshPersistenceFailed, err)
	}

	matchedStoryCount, err := s.repository.CountMatchingStories(ctx, snapshot.ID)
	if err != nil {
		return RefreshResult{}, s.fail(
			ctx,
			sync.ID,
			snapshot.ID,
			refreshCode(ctx, RefreshPersistenceFailed),
			err,
		)
	}
	if err := ctx.Err(); err != nil {
		return RefreshResult{}, s.fail(
			ctx,
			sync.ID,
			snapshot.ID,
			RefreshCancelled,
			err,
		)
	}

	// Activation is the publication boundary: once it starts, finish reporting
	// the committed snapshot even if the caller subsequently cancels.
	publicationContext := context.WithoutCancel(ctx)
	activatedAt := s.now().UTC()
	if err := s.repository.ActivateSnapshot(
		publicationContext,
		snapshot.ID,
		matchedStoryCount,
		activatedAt,
	); err != nil {
		return RefreshResult{}, s.fail(
			publicationContext,
			sync.ID,
			snapshot.ID,
			RefreshPersistenceFailed,
			err,
		)
	}
	sync, err = s.repository.Sync(publicationContext, sync.ID)
	if err != nil {
		return RefreshResult{}, &RefreshError{
			Code:  RefreshPersistenceFailed,
			Cause: err,
		}
	}
	snapshot, err = s.repository.Snapshot(publicationContext, snapshot.ID)
	if err != nil {
		return RefreshResult{}, &RefreshError{
			Code:  RefreshPersistenceFailed,
			Cause: err,
		}
	}
	return RefreshResult{
		Sync:       sync,
		Snapshot:   snapshot,
		StoryCount: len(normalized.Stories),
	}, nil
}

func (s *RefreshService) fail(
	ctx context.Context,
	syncID string,
	snapshotID int64,
	code RefreshErrorCode,
	cause error,
) error {
	status := SyncFailed
	if code == RefreshCancelled {
		status = SyncCancelled
	}
	persistErr := s.repository.FinishSyncFailure(
		context.WithoutCancel(ctx),
		syncID,
		snapshotID,
		status,
		string(code),
		refreshFailureMessage(code),
		s.now().UTC(),
	)
	refreshErr := &RefreshError{Code: code, Cause: cause}
	if persistErr != nil {
		return errors.Join(refreshErr, persistErr)
	}
	return refreshErr
}

func refreshCode(ctx context.Context, fallback RefreshErrorCode) RefreshErrorCode {
	if ctx.Err() != nil {
		return RefreshCancelled
	}
	return fallback
}

func refreshFailureMessage(code RefreshErrorCode) string {
	switch code {
	case RefreshFetchFailed:
		return "Official metadata could not be downloaded."
	case RefreshInvalidCatalog:
		return "The downloaded official catalog was invalid."
	case RefreshStagingFailed:
		return "The downloaded official catalog could not be staged."
	case RefreshCancelled:
		return "Official metadata refresh was cancelled."
	default:
		return "Official metadata could not be saved."
	}
}

func RefreshErrorHasCode(err error, code RefreshErrorCode) bool {
	var refreshErr *RefreshError
	return errors.As(err, &refreshErr) && refreshErr.Code == code
}
