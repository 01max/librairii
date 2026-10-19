package exporter

import (
	"context"
	"fmt"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
)

func TestCuratedExportUsesApprovedUUIDsAndRetainsUnavailableReferences(t *testing.T) {
	ctx := context.Background()
	db, err := database.Open(ctx, filepath.Join(t.TempDir(), "library.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	repo := catalog.NewRepository(db.Writer())
	uuids := []string{}
	for i, title := range []string{"First", "Second", "New import"} {
		id := fmt.Sprintf("123e4567-e89b-42d3-a456-%012d", i)
		uuids = append(uuids, id)
		archive := writeManagedExportStory(t, layout, int64(i+1), title+".zip", []byte(title))
		if _, _, err := repo.Create(ctx, catalog.CreateStory{UUID: id, EmbeddedTitle: title, OriginalFilename: title + ".zip", DetectedFormat: catalog.FormatZIP, SHA256: archive.SHA256, ByteSize: archive.ByteSize, ManagedPath: archive.ManagedRelativePath}); err != nil {
			t.Fatal(err)
		}
	}
	query := library.NewQuery(db.SQL(), nil)
	manual, err := shelves.NewService(db.Writer(), query)
	if err != nil {
		t.Fatal(err)
	}
	shelf, err := manual.CreateManual(ctx, "Approved", uuids[:2])
	if err != nil {
		t.Fatal(err)
	}
	copy, err := manual.Duplicate(ctx, shelf.ID, "Copy")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Writer().Exec(`DELETE FROM stories WHERE uuid=?`, uuids[0]); err != nil {
		t.Fatal(err)
	}
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	collection, err := dynamic.Create(ctx, "All", library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	resolver, err := NewResolver(query, dynamic, manual)
	if err != nil {
		t.Fatal(err)
	}
	scope, err := resolver.ResolveShelf(ctx, shelf.ID)
	if err != nil || scope.Source.Type != operations.ExportSourceCuratedShelf || len(scope.Stories) != 2 || scope.Stories[0].UUID != uuids[0] || scope.Stories[0].ID != 0 {
		t.Fatalf("approved scope: %#v %v", scope, err)
	}
	union, err := resolver.ResolveShelves(ctx, []int64{shelf.ID, copy.ID})
	if err != nil || len(union.Stories) != 2 || union.CollapsedOverlap != 2 {
		t.Fatalf("deduplicated scope: %#v %v", union, err)
	}
	current, err := resolver.ResolveCollection(ctx, collection.ID)
	if err != nil || current.Source.Type != operations.ExportSourceCollection || len(current.Stories) != 2 {
		t.Fatalf("dynamic scope: %#v %v", current, err)
	}
	refined, err := resolver.ResolveShelfResult(ctx, shelf.ID, library.StoryLibraryQuery{Name: "new import"})
	if err != nil || len(refined.Stories) != 0 {
		t.Fatalf("refinement escaped membership: %#v %v", refined, err)
	}
}
