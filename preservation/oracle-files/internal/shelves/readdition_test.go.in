package shelves

import (
	"context"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestRemovedMembersCanBeAddedAgainThroughACollection(t *testing.T) {
	for _, origin := range []string{"manual", "collection"} {
		t.Run(origin, func(t *testing.T) {
			s, db := newShelfService(t)
			ctx := context.Background()
			id := "123e4567-e89b-42d3-a456-426614174000"
			seedStory(t, db, id, "Moon")
			dynamic, err := collections.NewService(db.Writer())
			if err != nil {
				t.Fatal(err)
			}
			collection, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
			if err != nil {
				t.Fatal(err)
			}
			var shelf Shelf
			if origin == "manual" {
				shelf, err = s.CreateManual(ctx, "Chosen", []string{id})
				if err == nil {
					_, err = s.AttachSources(ctx, shelf.ID, []int64{collection.ID})
				}
			} else {
				shelf = createSourcedShelf(t, s, "Chosen", []int64{collection.ID}, []string{id})
			}
			if err != nil {
				t.Fatal(err)
			}
			removed, err := s.RemoveStories(ctx, shelf.ID, []string{id})
			if err != nil || removed.Count != 0 {
				t.Fatalf("remove: %#v %v", removed, err)
			}
			review, err := s.Preview(ctx, shelf.ID)
			if err != nil || len(review.Additions) != 1 || review.Additions[0].UUID != id {
				t.Fatalf("removed story missing from Collection diff: %#v %v", review, err)
			}
			readded, err := s.Apply(ctx, shelf.ID, review.Token, []Decision{{UUID: id, Action: "add"}})
			if err != nil || readded.Count != 1 {
				t.Fatalf("Collection re-addition: %#v %v", readded, err)
			}
			// Explicit exclusion remains a separate, reversible decision.
			if _, err := s.RemoveStories(ctx, shelf.ID, []string{id}); err != nil {
				t.Fatal(err)
			}
			review, err = s.Preview(ctx, shelf.ID)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := s.Apply(ctx, shelf.ID, review.Token, []Decision{{UUID: id, Action: "exclude"}}); err != nil {
				t.Fatal(err)
			}
			blocked, err := s.Preview(ctx, shelf.ID)
			if err != nil || len(blocked.Additions) != 0 {
				t.Fatalf("explicit exclusion ignored: %#v %v", blocked, err)
			}
			if _, err := s.ClearExclusion(ctx, shelf.ID, id); err != nil {
				t.Fatal(err)
			}
			allowed, err := s.Preview(ctx, shelf.ID)
			if err != nil || len(allowed.Additions) != 1 || allowed.Additions[0].UUID != id {
				t.Fatalf("allowed suggestion missing: %#v %v", allowed, err)
			}
		})
	}
}
