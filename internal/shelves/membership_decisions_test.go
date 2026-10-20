package shelves

import (
	"context"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestSourceMismatchRetainsMembershipUntilAnExplicitDecision(t *testing.T) {
	for _, action := range []string{"keep", "remove"} {
		t.Run(action, func(t *testing.T) {
			s, db := newShelfService(t)
			ctx := context.Background()
			sourced := "123e4567-e89b-42d3-a456-426614174000"
			manual := "223e4567-e89b-42d3-a456-426614174000"
			seedStory(t, db, sourced, "Moon")
			seedStory(t, db, manual, "Manual")
			dynamic, err := collections.NewService(db.Writer())
			if err != nil {
				t.Fatal(err)
			}
			collection, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
			if err != nil {
				t.Fatal(err)
			}
			shelf := createSourcedShelf(t, s, "Chosen", []int64{collection.ID}, []string{sourced})
			if _, err := s.AddStories(ctx, shelf.ID, []string{manual}); err != nil {
				t.Fatal(err)
			}
			if _, err := dynamic.ReplaceQuery(ctx, collection.ID, library.StoryLibraryQuery{Name: "no match"}); err != nil {
				t.Fatal(err)
			}
			before, err := s.Get(ctx, shelf.ID)
			if err != nil {
				t.Fatal(err)
			}
			review, err := s.Preview(ctx, shelf.ID)
			if err != nil || len(review.Removals) != 1 || review.Removals[0].UUID != sourced {
				t.Fatalf("expected only the source-derived story: %#v %v", review, err)
			}
			deferred, err := s.Apply(ctx, shelf.ID, review.Token, nil)
			if err != nil || deferred != before || deferred.Count != 2 {
				t.Fatalf("undecided membership changed: %#v %v", deferred, err)
			}
			review, err = s.Preview(ctx, shelf.ID)
			if err != nil || len(review.Removals) != 1 {
				t.Fatalf("deferred decision disappeared: %#v %v", review, err)
			}
			result, err := s.Apply(ctx, shelf.ID, review.Token, []Decision{{UUID: sourced, Action: action}})
			if err != nil {
				t.Fatal(err)
			}
			wantCount := 2
			if action == "remove" {
				wantCount = 1
			}
			if result.Count != wantCount {
				t.Fatalf("explicit %s: got %d members, want %d", action, result.Count, wantCount)
			}
			fresh, err := s.Preview(ctx, shelf.ID)
			if err != nil || len(fresh.Removals) != 0 {
				t.Fatalf("manual retention or approved decision proposed again: %#v %v", fresh, err)
			}
		})
	}
}
