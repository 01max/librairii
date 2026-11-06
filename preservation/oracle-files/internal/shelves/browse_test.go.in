package shelves

import (
	"context"
	"testing"

	"github.com/01max/librairii/internal/library"
)

func TestOpenReportsManualRanksAfterMembershipRemoval(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	ids := []string{"123e4567-e89b-42d3-a456-426614174000", "223e4567-e89b-42d3-a456-426614174000", "323e4567-e89b-42d3-a456-426614174000"}
	for i, id := range ids {
		seedStory(t, db, id, []string{"A", "B", "C"}[i])
	}
	shelf, err := s.CreateManual(ctx, "Reorder", ids)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.RemoveStories(ctx, shelf.ID, ids[:1]); err != nil {
		t.Fatal(err)
	}
	for page := 1; page <= 2; page++ {
		view, err := s.Open(ctx, shelf.ID, library.StoryLibraryQuery{Page: page, PageSize: 1})
		if err != nil || len(view.Members) != 1 {
			t.Fatalf("page %d: %#v %v", page, view, err)
		}
		if member := view.Members[0]; member.UUID != ids[page] || member.Position != page-1 || view.Shelf.Count != 2 {
			t.Fatalf("incorrect manual boundary on page %d: %#v", page, view)
		}
	}
	if _, err := s.ReorderMembers(ctx, shelf.ID, []string{ids[2], ids[1]}); err != nil {
		t.Fatal(err)
	}
	view, err := s.Open(ctx, shelf.ID, library.StoryLibraryQuery{})
	if err != nil || len(view.Members) != 2 || view.Members[0].UUID != ids[2] || view.Members[0].Position != 0 || view.Members[1].Position != 1 {
		t.Fatalf("reorder after removal: %#v %v", view, err)
	}
}
