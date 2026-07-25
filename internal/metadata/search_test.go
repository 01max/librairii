package metadata

import (
	"context"
	"strings"
	"testing"
	"time"
)

func TestBackfillNormalizedTitlesRepairsActiveSnapshotsWithoutWeakeningContentGuard(
	t *testing.T,
) {
	t.Parallel()

	repository, database := openMetadataRepository(t)
	ctx := context.Background()
	sync, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        "123e4567-e89b-42d3-a456-426614174120",
		Locale:    "en-GB",
		StartedAt: time.Now(),
	})
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    sync.ID,
		Locale:    sync.Locale,
		RawPath:   "catalog/search/catalog.json",
		RawSHA256: strings.Repeat("a", 64),
		ByteSize:  10,
		FetchedAt: time.Now(),
		Stories: []NewOfficialStoryMetadata{{
			StoryUUID: "123e4567-e89b-42d3-a456-426614174000",
			Title:     "L'École Magique",
			Language:  "en-GB",
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.Exec(
		"UPDATE official_story_metadata SET title_normalized = '' WHERE snapshot_id = ?",
		snapshot.ID,
	); err != nil {
		t.Fatal(err)
	}
	if err := repository.ActivateSnapshot(ctx, snapshot.ID, 0, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := BackfillNormalizedTitles(ctx, database); err != nil {
		t.Fatal(err)
	}
	var normalized string
	if err := database.QueryRow(
		"SELECT title_normalized FROM official_story_metadata WHERE snapshot_id = ?",
		snapshot.ID,
	).Scan(&normalized); err != nil {
		t.Fatal(err)
	}
	if normalized != "l'ecole magique" {
		t.Fatalf("title_normalized = %q", normalized)
	}
	if _, err := database.Exec(
		"UPDATE official_story_metadata SET title = 'Changed' WHERE snapshot_id = ?",
		snapshot.ID,
	); err == nil {
		t.Fatal("active official content mutation succeeded")
	}
}
