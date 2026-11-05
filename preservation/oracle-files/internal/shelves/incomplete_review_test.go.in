package shelves

import (
	"context"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestIncompleteSourcesAllowHealthyAdditionsAndSuspendAllRemovals(t *testing.T) {
	for _, corruption := range []struct {
		name    string
		version int
		payload string
	}{
		{"unsupported", 99, `{}`},
		{"missing criterion", 5, `{"booleanFilters":[{"definitionId":999,"state":"true"}]}`},
	} {
		t.Run(corruption.name, func(t *testing.T) {
			s, db := newShelfService(t)
			ctx := context.Background()
			old := "123e4567-e89b-42d3-a456-426614174000"
			candidate := "223e4567-e89b-42d3-a456-426614174000"
			seedStory(t, db, old, "Old")
			seedStory(t, db, candidate, "New moon")
			dynamic, err := collections.NewService(db.Writer())
			if err != nil {
				t.Fatal(err)
			}
			healthy, err := dynamic.Create(ctx, "Healthy", library.StoryLibraryQuery{Name: "moon"})
			if err != nil {
				t.Fatal(err)
			}
			bad, err := dynamic.Create(ctx, "Needs attention", library.StoryLibraryQuery{Name: "forest"})
			if err != nil {
				t.Fatal(err)
			}
			shelf, err := s.CreateManual(ctx, "Approved", []string{old})
			if err != nil {
				t.Fatal(err)
			}
			if _, err := s.AttachSources(ctx, shelf.ID, []int64{healthy.ID, bad.ID}); err != nil {
				t.Fatal(err)
			}
			if _, err := s.SetProtected(ctx, shelf.ID, old, false); err != nil {
				t.Fatal(err)
			}
			if _, err := db.Writer().Exec(`UPDATE collections SET query_version=?,query_payload=? WHERE id=?`, corruption.version, corruption.payload, bad.ID); err != nil {
				t.Fatal(err)
			}
			review, err := s.Preview(ctx, shelf.ID)
			if err != nil {
				t.Fatal(err)
			}
			if !review.Incomplete || review.UpToDate || len(review.Removals) != 0 || len(review.Additions) != 1 || review.Additions[0].UUID != candidate || len(review.Warnings) != 1 || review.Warnings[0].CollectionID != bad.ID {
				t.Fatalf("unsafe incomplete review: %#v", review)
			}
			if _, err := dynamic.ReplaceQuery(ctx, bad.ID, library.StoryLibraryQuery{Name: "forest"}); err != nil {
				t.Fatal(err)
			}
			fresh, err := s.Preview(ctx, shelf.ID)
			if err != nil || fresh.Incomplete || len(fresh.Removals) != 1 || fresh.Removals[0].UUID != old {
				t.Fatalf("repair not reevaluated: %#v %v", fresh, err)
			}
		})
	}
}
