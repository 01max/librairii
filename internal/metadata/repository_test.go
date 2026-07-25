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
	"github.com/01max/librairii/internal/tagging"
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
		Artworks: []NewCatalogArtwork{{
			ID:        strings.Repeat("9", 64),
			SourceURL: "https://storage.googleapis.com/lunii-data-prod/fixture/little-prince.png",
		}},
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
			ArtworkID:       strings.Repeat("9", 64),
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

func TestRepositoryRegistersAndCachesOpaqueCatalogArtwork(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	artworkID := strings.Repeat("8", 64)
	syncID := "123e4567-e89b-42d3-a456-426614174110"
	if _, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        syncID,
		Locale:    "en-GB",
		StartedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	if _, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    syncID,
		Locale:    "en-GB",
		RawPath:   "catalog/" + syncID + "/catalog.json",
		RawSHA256: strings.Repeat("8", 64),
		ByteSize:  128,
		FetchedAt: time.Now(),
		Artworks: []NewCatalogArtwork{{
			ID:        artworkID,
			SourceURL: "https://storage.googleapis.com/lunii-data-prod/fixture/cover.png",
		}},
		Stories: []NewOfficialStoryMetadata{{
			StoryUUID: "123e4567-e89b-42d3-a456-426614174000",
			ArtworkID: artworkID,
		}},
	}); err != nil {
		t.Fatal(err)
	}

	record, err := repository.Artwork(ctx, artworkID)
	if err != nil {
		t.Fatal(err)
	}
	if record.ID != artworkID || record.ManagedPath != "" || record.CachedAt != "" {
		t.Fatalf("Artwork(before cache) = %#v", record)
	}
	cachedAt := time.Date(2026, time.July, 25, 14, 0, 0, 0, time.UTC)
	if err := repository.CacheArtwork(
		ctx,
		artworkID,
		"catalog/official/"+artworkID+"/"+strings.Repeat("a", 64)+".png",
		"image/png",
		strings.Repeat("a", 64),
		128,
		`"fixture"`,
		"Sat, 25 Jul 2026 10:00:00 GMT",
		cachedAt,
	); err != nil {
		t.Fatal(err)
	}
	record, err = repository.Artwork(ctx, artworkID)
	if err != nil {
		t.Fatal(err)
	}
	if record.ContentType != "image/png" ||
		record.ByteSize != 128 ||
		record.ETag != `"fixture"` ||
		record.CachedAt != formatTime(cachedAt) {
		t.Fatalf("Artwork(after cache) = %#v", record)
	}
	if _, err := connection.Exec(
		"UPDATE catalog_artworks SET source_url = ? WHERE id = ?",
		"https://example.test/replaced.png",
		artworkID,
	); err == nil {
		t.Fatal("catalog artwork identity mutation error = nil")
	}
	if err := repository.CacheArtwork(
		ctx,
		strings.Repeat("7", 64),
		"catalog/official/missing/fixture.png",
		"image/png",
		strings.Repeat("b", 64),
		1,
		"",
		"",
		cachedAt,
	); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("CacheArtwork(unregistered) error = %v", err)
	}
}

func TestRepositoryRejectsUnregisteredOfficialArtwork(t *testing.T) {
	t.Parallel()

	repository, _ := openMetadataRepository(t)
	ctx := context.Background()
	syncID := "123e4567-e89b-42d3-a456-426614174111"
	if _, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        syncID,
		Locale:    "en-GB",
		StartedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	_, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    syncID,
		Locale:    "en-GB",
		RawPath:   "catalog/" + syncID + "/catalog.json",
		RawSHA256: strings.Repeat("7", 64),
		ByteSize:  128,
		FetchedAt: time.Now(),
		Stories: []NewOfficialStoryMetadata{{
			StoryUUID: "123e4567-e89b-42d3-a456-426614174000",
			ArtworkID: strings.Repeat("7", 64),
		}},
	})
	if err == nil {
		t.Fatal("StageSnapshot(unregistered artwork) error = nil")
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

func TestRepositoryActivationRebuildsOnlyDerivedAgeAssignments(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	storyUUID := "20000000-0000-4000-8000-000000000001"
	storyResult, err := connection.Exec(
		"INSERT INTO stories (uuid) VALUES (?)",
		storyUUID,
	)
	if err != nil {
		t.Fatal(err)
	}
	storyID, err := storyResult.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	broken, err := tagging.SeedBuiltIns(ctx, connection)
	if err != nil {
		t.Fatal(err)
	}
	tagService, err := tagging.NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	favorite, err := tagService.CreateDefinition(ctx, tagging.CreateDefinition{
		Key:   "favorite",
		Label: "Favorite",
		Color: "#405CF5",
		Kind:  tagging.KindBoolean,
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := tagService.SetStoryBoolean(ctx, storyID, broken.ID, true); err != nil {
		t.Fatal(err)
	}
	if _, err := tagService.SetStoryBoolean(ctx, storyID, favorite.ID, true); err != nil {
		t.Fatal(err)
	}

	first := stageAgeSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174121",
		storyUUID,
		3,
		5,
	)
	if err := repository.ActivateSnapshot(ctx, first.ID, 1, time.Now()); err != nil {
		t.Fatal(err)
	}
	if got := assignedDerivedAgeKey(t, connection, storyID); got != "3-5" {
		t.Fatalf("first derived age = %q", got)
	}

	second := stageAgeSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174122",
		storyUUID,
		6,
		8,
	)
	if err := repository.ActivateSnapshot(ctx, second.ID, 1, time.Now()); err != nil {
		t.Fatal(err)
	}
	if got := assignedDerivedAgeKey(t, connection, storyID); got != "6-8" {
		t.Fatalf("second derived age = %q", got)
	}

	var manualAssignments int
	if err := connection.QueryRow(
		`SELECT COUNT(*)
		 FROM story_tag_assignments
		 WHERE story_id = ? AND source = 'manual'`,
		storyID,
	).Scan(&manualAssignments); err != nil {
		t.Fatal(err)
	}
	definitions, err := tagService.ListDefinitions(ctx)
	if err != nil {
		t.Fatal(err)
	}
	var gotBroken tagging.Definition
	var gotFavorite tagging.Definition
	for _, definition := range definitions {
		switch definition.ID {
		case broken.ID:
			gotBroken = definition
		case favorite.ID:
			gotFavorite = definition
		}
	}
	if manualAssignments != 2 ||
		gotBroken != broken ||
		gotFavorite != favorite {
		t.Fatalf(
			"manual assignments = %d, broken = %#v, favorite = %#v",
			manualAssignments,
			gotBroken,
			gotFavorite,
		)
	}
	first, err = repository.Snapshot(ctx, first.ID)
	if err != nil {
		t.Fatal(err)
	}
	second, err = repository.Snapshot(ctx, second.ID)
	if err != nil {
		t.Fatal(err)
	}
	if first.Status != SnapshotSuperseded || second.Status != SnapshotActive {
		t.Fatalf("snapshots after rebuild = %#v, %#v", first, second)
	}
}

func TestRepositoryRollsBackActivationWhenDerivedProjectionFails(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	storyUUID := "20000000-0000-4000-8000-000000000002"
	storyResult, err := connection.Exec(
		"INSERT INTO stories (uuid) VALUES (?)",
		storyUUID,
	)
	if err != nil {
		t.Fatal(err)
	}
	storyID, err := storyResult.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	first := stageAgeSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174123",
		storyUUID,
		3,
		5,
	)
	if err := repository.ActivateSnapshot(ctx, first.ID, 1, time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := connection.Exec(
		`UPDATE tag_definitions
		 SET label = 'Drifted'
		 WHERE normalized_key = ?`,
		AgeDefinitionKey,
	); err != nil {
		t.Fatal(err)
	}
	second := stageAgeSnapshot(
		t,
		repository,
		"123e4567-e89b-42d3-a456-426614174124",
		storyUUID,
		6,
		8,
	)
	if err := repository.ActivateSnapshot(
		ctx,
		second.ID,
		1,
		time.Now(),
	); !errors.Is(err, ErrDerivedFacetDrift) {
		t.Fatalf("ActivateSnapshot(drifted projector) error = %v", err)
	}
	active, err := repository.ActiveSnapshot(ctx, "en-GB")
	if err != nil {
		t.Fatal(err)
	}
	second, err = repository.Snapshot(ctx, second.ID)
	if err != nil {
		t.Fatal(err)
	}
	if active.ID != first.ID ||
		active.Status != SnapshotActive ||
		second.Status != SnapshotStaged ||
		assignedDerivedAgeKey(t, connection, storyID) != "3-5" {
		t.Fatalf("activation rollback = active %#v, second %#v", active, second)
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

func stageAgeSnapshot(
	t *testing.T,
	repository *Repository,
	syncID string,
	storyUUID string,
	minimumAge int,
	maximumAge int,
) CatalogSnapshot {
	t.Helper()

	ctx := context.Background()
	if _, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        syncID,
		Locale:    "en-GB",
		StartedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	snapshot, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    syncID,
		Locale:    "en-GB",
		RawPath:   "catalog/" + syncID + "/catalog.json",
		RawSHA256: strings.Repeat("e", 64),
		ByteSize:  128,
		FetchedAt: time.Now(),
		Stories: []NewOfficialStoryMetadata{{
			StoryUUID:  storyUUID,
			MinimumAge: &minimumAge,
			MaximumAge: &maximumAge,
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	return snapshot
}

func assignedDerivedAgeKey(
	t *testing.T,
	connection *sql.DB,
	storyID int64,
) string {
	t.Helper()

	var key string
	if err := connection.QueryRow(
		`SELECT tag_values.normalized_key
		 FROM story_tag_assignments
		 JOIN tag_definitions
		   ON tag_definitions.id = story_tag_assignments.definition_id
		 JOIN tag_values
		   ON tag_values.id = story_tag_assignments.value_id
		 WHERE story_tag_assignments.story_id = ?
		   AND story_tag_assignments.source = 'derived'
		   AND tag_definitions.normalized_key = ?`,
		storyID,
		AgeDefinitionKey,
	).Scan(&key); err != nil {
		t.Fatal(err)
	}
	return key
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
