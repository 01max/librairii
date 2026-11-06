package metadata

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"
)

func TestCatalogStatusDistinguishesNeverSyncedFreshAndStaleCache(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	status, err := repository.CatalogStatus(ctx, "en_GB")
	if err != nil {
		t.Fatal(err)
	}
	if status.State != CatalogNeverSynced ||
		status.Locale != "en-GB" ||
		status.LastAttemptStatus != "" {
		t.Fatalf("CatalogStatus(never synced) = %#v", status)
	}

	storyUUID := "30000000-0000-4000-8000-000000000001"
	if _, err := connection.Exec(
		"INSERT INTO stories (uuid) VALUES (?)",
		storyUUID,
	); err != nil {
		t.Fatal(err)
	}
	activatedAt := time.Date(2026, time.July, 25, 16, 0, 0, 0, time.UTC)
	snapshot := stageStatusSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174130",
		storyUUID,
		activatedAt.Add(-time.Hour),
	)
	if err := repository.ActivateSnapshot(
		ctx,
		snapshot.ID,
		1,
		activatedAt,
	); err != nil {
		t.Fatal(err)
	}
	status, err = repository.CatalogStatus(ctx, "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	if status.State != CatalogFresh ||
		status.MatchedStoryCount != 1 ||
		status.ActivatedAt != formatTime(activatedAt) ||
		status.LastAttemptStatus != SyncSucceeded {
		t.Fatalf("CatalogStatus(fresh) = %#v", status)
	}

	failedID := "123e4567-e89b-42d3-a456-426614174131"
	if _, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        failedID,
		Locale:    "en-GB",
		StartedAt: activatedAt.Add(time.Hour),
	}); err != nil {
		t.Fatal(err)
	}
	if err := repository.FinishSyncFailure(
		ctx,
		failedID,
		0,
		SyncFailed,
		string(RefreshFetchFailed),
		refreshFailureMessage(RefreshFetchFailed),
		activatedAt.Add(2*time.Hour),
	); err != nil {
		t.Fatal(err)
	}
	status, err = repository.CatalogStatus(ctx, "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	if status.State != CatalogStaleCache ||
		status.MatchedStoryCount != 1 ||
		status.ErrorCode != string(RefreshFetchFailed) ||
		status.ErrorMessage == "" {
		t.Fatalf("CatalogStatus(stale cache) = %#v", status)
	}
}

func TestCountMatchingStoriesUsesCompleteLocalUUIDs(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	localUUID := "30000000-0000-4000-8000-000000000002"
	if _, err := connection.Exec(
		"INSERT INTO stories (uuid) VALUES (?)",
		localUUID,
	); err != nil {
		t.Fatal(err)
	}
	snapshot := stageStatusSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174132",
		localUUID,
		time.Date(2026, time.July, 25, 16, 0, 0, 0, time.UTC),
	)
	count, err := repository.CountMatchingStories(ctx, snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if count != 1 {
		t.Fatalf("CountMatchingStories() = %d", count)
	}
	if _, err := repository.CountMatchingStories(
		ctx,
		0,
	); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("CountMatchingStories(0) error = %v", err)
	}
}

func stageStatusSnapshot(
	t *testing.T,
	repository *Repository,
	syncID string,
	matchedUUID string,
	startedAt time.Time,
) CatalogSnapshot {
	t.Helper()

	ctx := context.Background()
	if _, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        syncID,
		Locale:    "en-GB",
		StartedAt: startedAt,
	}); err != nil {
		t.Fatal(err)
	}
	snapshot, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    syncID,
		Locale:    "en-GB",
		RawPath:   "catalog/" + syncID + "/catalog.json",
		RawSHA256: strings.Repeat("f", 64),
		ByteSize:  128,
		FetchedAt: startedAt,
		Stories: []NewOfficialStoryMetadata{
			{StoryUUID: matchedUUID},
			{StoryUUID: "30000000-0000-4000-8000-000000000099"},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	return snapshot
}
