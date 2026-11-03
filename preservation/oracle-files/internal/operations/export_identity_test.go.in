package operations

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	"github.com/01max/librairii/internal/database"
	"github.com/google/uuid"
)

func TestExportHistoryPreservesDistinctGroupIdentitiesAndSkippedUUIDs(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "history.sqlite3")
	db, err := database.Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	repository := NewRepository(db.Writer())
	ids := []string{}
	sources := []ExportSource{
		{Type: ExportSourceShelf, ShelfIDs: []int64{7}, ShelfNames: []string{"Legacy search"}},
		{Type: ExportSourceCollection, CollectionIDs: []int64{7}, CollectionNames: []string{"Collection"}},
		{Type: ExportSourceCuratedShelf, ShelfIDs: []int64{7}, ShelfNames: []string{"Curated"}},
	}
	for _, source := range sources {
		id := uuid.NewString()
		ids = append(ids, id)
		created, err := repository.CreateExport(ctx, id, source, t.TempDir(), []NewItem{{StoryUUID: "123e4567-e89b-42d3-a456-426614174000", StoryTitle: "Unavailable", SourceName: "Unavailable", ExportSkipCode: "story_unavailable"}}, time.Now())
		if err != nil {
			t.Fatal(err)
		}
		item := created.Items[0]
		item.Status = ItemSkipped
		item.OutcomeCode = "story_unavailable"
		item.OutcomeMessage = "This approved story is unavailable in the library."
		if err := repository.CompleteItem(ctx, id, item); err != nil {
			t.Fatal(err)
		}
		if err := repository.Finish(ctx, id, StatusPartiallySucceeded, "", "", time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	db, err = database.Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	repository = NewRepository(db.SQL())
	for i, id := range ids {
		snapshot, err := repository.Snapshot(ctx, id)
		if err != nil {
			t.Fatal(err)
		}
		if snapshot.ExportSourceType != sources[i].Type || len(snapshot.Items) != 1 || snapshot.Items[0].StoryID != 0 || snapshot.Items[0].StoryUUID != "123e4567-e89b-42d3-a456-426614174000" || snapshot.Items[0].OutcomeCode != "story_unavailable" {
			t.Fatalf("history changed: %#v", snapshot)
		}
		if i < 2 {
			if len(snapshot.SourceCollectionNames) != 1 || snapshot.SourceCollectionNames[0] != sources[i].GroupNames()[0] {
				t.Fatalf("dynamic identity lost: %#v", snapshot)
			}
		} else if len(snapshot.SourceCollectionIDs) != 0 || snapshot.SourceShelfNames[0] != "Curated" {
			t.Fatalf("curated identity confused: %#v", snapshot)
		}
	}
}
