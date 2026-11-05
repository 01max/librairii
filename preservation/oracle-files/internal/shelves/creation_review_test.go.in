package shelves

import (
	"context"
	"errors"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestCreationReviewConfirmsOnlyChosenMembersAndProtectsManualOverlap(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	manual := "123e4567-e89b-42d3-a456-426614174000"
	sourceOnly := "223e4567-e89b-42d3-a456-426614174000"
	deferred := "323e4567-e89b-42d3-a456-426614174000"
	for id, title := range map[string]string{manual: "Moon forest", sourceOnly: "Moon", deferred: "Forest"} {
		seedStory(t, db, id, title)
	}
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	moon, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
	if err != nil {
		t.Fatal(err)
	}
	forest, err := dynamic.Create(ctx, "Forest", library.StoryLibraryQuery{Name: "forest"})
	if err != nil {
		t.Fatal(err)
	}
	preview, err := s.PreviewCreation(ctx, CreationRequest{Name: "Chosen", ManualUUIDs: []string{manual}, CollectionIDs: []int64{moon.ID, forest.ID}})
	if err != nil || len(preview.Additions) != 3 || preview.Additions[0].UUID != manual || !preview.Additions[0].Manual {
		t.Fatalf("creation preview: %#v %v", preview, err)
	}
	listed, err := s.List(ctx)
	if err != nil || len(listed) != 0 {
		t.Fatal("preview created a Shelf")
	}
	created, err := s.CreateFromReview(ctx, preview.Token, []string{sourceOnly, manual})
	if err != nil || created.Count != 2 || created.SourceCount != 2 {
		t.Fatalf("creation: %#v %v", created, err)
	}
	opened, err := s.Open(ctx, created.ID, library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	if opened.Members[0].UUID != manual || !opened.Members[0].Protected || len(opened.Members[0].Origins) != 2 || opened.Members[1].Protected || len(opened.Members[1].Origins) != 1 {
		t.Fatalf("seed decisions lost: %#v", opened.Members)
	}
	updates, err := s.Preview(ctx, created.ID)
	if err != nil || len(updates.Additions) != 1 || updates.Additions[0].UUID != deferred {
		t.Fatalf("unchecked story excluded or inserted: %#v %v", updates, err)
	}
}

func TestCreationReviewRejectsStaleSourceResultsWithoutCreatingShelf(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	collection, err := dynamic.Create(ctx, "All", library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	preview, err := s.PreviewCreation(ctx, CreationRequest{Name: "Empty seed", CollectionIDs: []int64{collection.ID}})
	if err != nil {
		t.Fatal(err)
	}
	seedStory(t, db, "123e4567-e89b-42d3-a456-426614174000", "New import")
	if _, err := s.CreateFromReview(ctx, preview.Token, nil); !errors.Is(err, ErrStaleReview) {
		t.Fatalf("stale creation accepted: %v", err)
	}
	listed, err := s.List(ctx)
	if err != nil || len(listed) != 0 {
		t.Fatal("stale creation left a Shelf")
	}
	fresh, err := s.PreviewCreation(ctx, CreationRequest{Name: "Empty seed", CollectionIDs: []int64{collection.ID}})
	if err != nil {
		t.Fatal(err)
	}
	created, err := s.CreateFromReview(ctx, fresh.Token, nil)
	if err != nil || created.Count != 0 || created.SourceCount != 1 {
		t.Fatalf("explicit empty seed invalid: %#v %v", created, err)
	}
}
