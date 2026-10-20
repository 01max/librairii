package shelves

import (
	"context"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestSourceAttachmentDoesNotPopulateOrDuplicateMembership(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	seedStory(t, db, "123e4567-e89b-42d3-a456-426614174000", "Candidate")
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	collection, err := dynamic.Create(ctx, "All", library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	shelf, err := s.Create(ctx, "Empty")
	if err != nil {
		t.Fatal(err)
	}
	attached, err := s.AttachSources(ctx, shelf.ID, []int64{collection.ID, collection.ID})
	if err != nil || attached.Count != 0 || attached.SourceCount != 1 || attached.Revision != 2 {
		t.Fatalf("implicit population: %#v %v", attached, err)
	}
	noOp, err := s.AttachSources(ctx, shelf.ID, []int64{collection.ID})
	if err != nil || noOp.Revision != 2 {
		t.Fatalf("non-idempotent attach: %#v %v", noOp, err)
	}
	if _, err := s.AttachSources(ctx, shelf.ID, []int64{999}); err == nil {
		t.Fatal("non-Collection source accepted")
	}
	if _, err := dynamic.Rename(ctx, collection.ID, "Renamed"); err != nil {
		t.Fatal(err)
	}
	sources, err := s.Sources(ctx, shelf.ID)
	if err != nil || len(sources) != 1 || sources[0].Name != "Renamed" {
		t.Fatalf("source identity lost: %#v %v", sources, err)
	}
}
