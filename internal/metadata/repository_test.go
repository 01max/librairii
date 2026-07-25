package metadata

import (
	"context"
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
)

func TestRepositoryStagesAndActivatesLocalizedMetadata(t *testing.T) {
	t.Parallel()

	repository, _ := openMetadataRepository(t)
	ctx := context.Background()
	startedAt := time.Date(2026, time.July, 25, 8, 0, 0, 0, time.UTC)
	sync, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        "123e4567-e89b-42d3-a456-426614174100",
		Locale:    "en-GB",
		StartedAt: startedAt,
	})
	if err != nil {
		t.Fatal(err)
	}
	if sync.Status != SyncRunning || sync.StartedAt != formatTime(startedAt) {
		t.Fatalf("CreateSync() = %#v", sync)
	}

	fetchedAt := startedAt.Add(time.Second)
	sourceUpdatedAt := fetchedAt.Add(-time.Hour)
	duration := 3240
	minimumAge := 3
	maximumAge := 5
	snapshot, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    sync.ID,
		Locale:    sync.Locale,
		RawPath:   "catalog/123e4567-e89b-42d3-a456-426614174100/catalog.json",
		RawSHA256: strings.Repeat("a", 64),
		ByteSize:  1024,
		FetchedAt: fetchedAt,
		Stories: []NewOfficialStoryMetadata{{
			StoryUUID:       "123e4567-e89b-42d3-a456-426614174000",
			Title:           "The Little Prince",
			Description:     "A journey among the stars.",
			Author:          "Antoine de Saint-Exupéry",
			Publisher:       "Gallimard",
			Language:        "English",
			DurationSeconds: &duration,
			MinimumAge:      &minimumAge,
			MaximumAge:      &maximumAge,
			ArtworkID:       "artwork-little-prince",
			SourceRecordID:  "pack-001",
			SourceUpdatedAt: &sourceUpdatedAt,
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.Status != SnapshotStaged ||
		snapshot.RecordCount != 1 ||
		snapshot.FetchedAt != formatTime(fetchedAt) {
		t.Fatalf("StageSnapshot() = %#v", snapshot)
	}
	if _, err := repository.ActiveSnapshot(ctx, sync.Locale); !errors.Is(err, sql.ErrNoRows) {
		t.Fatalf("ActiveSnapshot(before activation) error = %v", err)
	}

	activatedAt := fetchedAt.Add(time.Second)
	if err := repository.ActivateSnapshot(ctx, snapshot.ID, 1, activatedAt); err != nil {
		t.Fatal(err)
	}
	active, err := repository.ActiveSnapshot(ctx, sync.Locale)
	if err != nil {
		t.Fatal(err)
	}
	if active.ID != snapshot.ID ||
		active.Status != SnapshotActive ||
		active.ActivatedAt != formatTime(activatedAt) {
		t.Fatalf("ActiveSnapshot() = %#v", active)
	}
	official, err := repository.ActiveMetadataByUUID(
		ctx,
		sync.Locale,
		"123e4567-e89b-42d3-a456-426614174000",
	)
	if err != nil {
		t.Fatal(err)
	}
	if official.Title != "The Little Prince" ||
		official.Provenance != ProvenanceLuniiCatalog ||
		official.FetchedAt != formatTime(fetchedAt) ||
		official.ActivatedAt != formatTime(activatedAt) ||
		official.MinimumAge == nil ||
		*official.MinimumAge != 3 {
		t.Fatalf("ActiveMetadataByUUID() = %#v", official)
	}
	sync, err = repository.Sync(ctx, sync.ID)
	if err != nil {
		t.Fatal(err)
	}
	if sync.Status != SyncSucceeded ||
		sync.MatchedStoryCount != 1 ||
		sync.FinishedAt != formatTime(activatedAt) {
		t.Fatalf("Sync(after activation) = %#v", sync)
	}
}

func TestRepositoryActivationSupersedesOnlySameLocale(t *testing.T) {
	t.Parallel()

	repository, _ := openMetadataRepository(t)
	ctx := context.Background()
	first := stageFixtureSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174101",
		"en-GB",
		"a",
	)
	if err := repository.ActivateSnapshot(ctx, first.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	french := stageFixtureSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174102",
		"fr-FR",
		"b",
	)
	if err := repository.ActivateSnapshot(ctx, french.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	second := stageFixtureSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174103",
		"en-GB",
		"c",
	)
	if err := repository.ActivateSnapshot(ctx, second.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}

	first, err := repository.Snapshot(ctx, first.ID)
	if err != nil {
		t.Fatal(err)
	}
	if first.Status != SnapshotSuperseded {
		t.Fatalf("first snapshot status = %q", first.Status)
	}
	activeEnglish, err := repository.ActiveSnapshot(ctx, "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	activeFrench, err := repository.ActiveSnapshot(ctx, "fr-FR")
	if err != nil {
		t.Fatal(err)
	}
	if activeEnglish.ID != second.ID || activeFrench.ID != french.ID {
		t.Fatalf("active English = %#v, active French = %#v", activeEnglish, activeFrench)
	}
}

func TestRepositoryRollsBackSupersessionWhenActivationFails(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	first := stageFixtureSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174108",
		"en-GB",
		"1",
	)
	if err := repository.ActivateSnapshot(ctx, first.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	second := stageFixtureSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174109",
		"en-GB",
		"2",
	)
	if _, err := connection.Exec(`
		CREATE TRIGGER test_reject_snapshot_activation
		BEFORE UPDATE OF status ON catalog_snapshots
		FOR EACH ROW
		WHEN (
			NEW.sync_id = '123e4567-e89b-42d3-a456-426614174109'
			AND NEW.status = 'active'
		)
		BEGIN
			SELECT RAISE(ABORT, 'injected activation failure');
		END
	`); err != nil {
		t.Fatal(err)
	}
	if err := repository.ActivateSnapshot(ctx, second.ID, 0, time.Now()); err == nil {
		t.Fatal("ActivateSnapshot(injected failure) error = nil")
	}
	active, err := repository.ActiveSnapshot(ctx, "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	second, err = repository.Snapshot(ctx, second.ID)
	if err != nil {
		t.Fatal(err)
	}
	if active.ID != first.ID || active.Status != SnapshotActive ||
		second.Status != SnapshotStaged {
		t.Fatalf("active = %#v, second = %#v", active, second)
	}
}

func TestMetadataSchemaRejectsInvalidIdentityPathsAndSnapshotWrites(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	sync, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        "123e4567-e89b-42d3-a456-426614174104",
		Locale:    "en-GB",
		StartedAt: time.Now(),
	})
	if err != nil {
		t.Fatal(err)
	}

	_, err = repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    sync.ID,
		Locale:    sync.Locale,
		RawPath:   "../catalog.json",
		RawSHA256: strings.Repeat("a", 64),
		FetchedAt: time.Now(),
	})
	if err == nil {
		t.Fatal("StageSnapshot(path escape) error = nil")
	}
	_, err = repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    sync.ID,
		Locale:    "fr-FR",
		RawPath:   "catalog/catalog.json",
		RawSHA256: strings.Repeat("a", 64),
		FetchedAt: time.Now(),
	})
	if err == nil {
		t.Fatal("StageSnapshot(locale mismatch) error = nil")
	}
	_, err = repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    sync.ID,
		Locale:    sync.Locale,
		RawPath:   "catalog/catalog.json",
		RawSHA256: strings.Repeat("a", 64),
		FetchedAt: time.Now(),
		Stories: []NewOfficialStoryMetadata{{
			StoryUUID: "not-a-complete-uuid",
		}},
	})
	if err == nil {
		t.Fatal("StageSnapshot(invalid UUID) error = nil")
	}

	snapshot := stageFixtureSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174105",
		"fr-FR",
		"d",
	)
	if err := repository.ActivateSnapshot(ctx, snapshot.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	_, err = connection.Exec(
		`INSERT INTO official_story_metadata (
			snapshot_id, story_uuid, locale, provenance
		 ) VALUES (?, ?, ?, ?)`,
		snapshot.ID,
		"123e4567-e89b-42d3-a456-426614174099",
		"fr-FR",
		ProvenanceLuniiCatalog,
	)
	if err == nil {
		t.Fatal("insert metadata into active snapshot error = nil")
	}
}

func TestMetadataSchemaRejectsDuplicateRowsAndInvalidTransitions(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	snapshot := stageFixtureSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174106",
		"en-GB",
		"e",
	)
	_, err := connection.Exec(
		`INSERT INTO official_story_metadata (
			snapshot_id, story_uuid, locale, provenance
		 ) VALUES (?, ?, ?, ?)`,
		snapshot.ID,
		"123e4567-e89b-42d3-a456-426614174000",
		"en-GB",
		ProvenanceLuniiCatalog,
	)
	if err == nil {
		t.Fatal("duplicate metadata row error = nil")
	}
	if err := repository.ActivateSnapshot(ctx, snapshot.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := repository.ActivateSnapshot(ctx, snapshot.ID, 0, time.Now()); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("ActivateSnapshot(active) error = %v", err)
	}
	if _, err := connection.Exec(
		"UPDATE catalog_snapshots SET status = 'rejected', activated_at = NULL WHERE id = ?",
		snapshot.ID,
	); err == nil {
		t.Fatal("active to rejected transition error = nil")
	}
	if _, err := connection.Exec(
		"UPDATE catalog_syncs SET status = 'running', finished_at = NULL WHERE id = ?",
		snapshot.SyncID,
	); err == nil {
		t.Fatal("terminal sync transition error = nil")
	}
}

func TestRepositoryFinishesFailedSyncAndRejectsItsStagedSnapshot(t *testing.T) {
	t.Parallel()

	repository, _ := openMetadataRepository(t)
	ctx := context.Background()
	snapshot := stageFixtureSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174107",
		"en-GB",
		"f",
	)
	finishedAt := time.Date(2026, time.July, 25, 12, 0, 0, 0, time.UTC)
	if err := repository.FinishSyncFailure(
		ctx,
		snapshot.SyncID,
		snapshot.ID,
		SyncFailed,
		"catalog_invalid",
		"The downloaded official catalog was invalid.",
		finishedAt,
	); err != nil {
		t.Fatal(err)
	}
	sync, err := repository.Sync(ctx, snapshot.SyncID)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err = repository.Snapshot(ctx, snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if sync.Status != SyncFailed ||
		sync.ErrorCode != "catalog_invalid" ||
		sync.FinishedAt != formatTime(finishedAt) ||
		snapshot.Status != SnapshotRejected {
		t.Fatalf("sync = %#v, snapshot = %#v", sync, snapshot)
	}
	if err := repository.FinishSyncFailure(
		ctx,
		sync.ID,
		snapshot.ID,
		SyncFailed,
		"again",
		"Again.",
		finishedAt,
	); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("FinishSyncFailure(terminal) error = %v", err)
	}
}

func stageFixtureSnapshot(
	t *testing.T,
	repository *Repository,
	syncID string,
	locale string,
	hashCharacter string,
) CatalogSnapshot {
	t.Helper()

	ctx := context.Background()
	if _, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        syncID,
		Locale:    locale,
		StartedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	snapshot, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    syncID,
		Locale:    locale,
		RawPath:   "catalog/" + syncID + "/catalog.json",
		RawSHA256: strings.Repeat(hashCharacter, 64),
		ByteSize:  128,
		FetchedAt: time.Now(),
		Stories: []NewOfficialStoryMetadata{{
			StoryUUID: "123e4567-e89b-42d3-a456-426614174000",
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	return snapshot
}

func openMetadataRepository(t *testing.T) (*Repository, *sql.DB) {
	t.Helper()

	path := filepath.Join(t.TempDir(), "db", "librairii.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	opened, err := database.Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := opened.Close(); err != nil {
			t.Error(err)
		}
	})
	return NewRepository(opened.SQL()), opened.SQL()
}
