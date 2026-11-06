package shelves

import (
	"context"
	"testing"
	"time"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestReviewUsesOrUnionWithoutChangingApprovedMembership(t *testing.T) {
	s, db := newShelfService(t)
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	old := "123e4567-e89b-42d3-a456-426614174000"
	still := "223e4567-e89b-42d3-a456-426614174000"
	manual := "323e4567-e89b-42d3-a456-426614174000"
	candidate := "423e4567-e89b-42d3-a456-426614174000"
	missing := "523e4567-e89b-42d3-a456-426614174000"
	excluded := "623e4567-e89b-42d3-a456-426614174000"
	for id, title := range map[string]string{old: "Old", still: "Still moon", manual: "Manual ocean", candidate: "Moon forest", excluded: "Moon excluded"} {
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
	shelf, err := s.CreateManual(ctx, "Approved", []string{old, still, manual})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.AttachSources(ctx, shelf.ID, []int64{moon.ID, forest.ID}); err != nil {
		t.Fatal(err)
	}
	for _, id := range []string{old, still} {
		if _, err := s.SetProtected(ctx, shelf.ID, id, false); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := db.Writer().Exec(`INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(?,?,3,0,'Unavailable')`, shelf.ID, missing); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Writer().Exec(`INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)`, shelf.ID, excluded); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Writer().Exec(`UPDATE stories SET is_broken=1 WHERE uuid=?`, candidate); err != nil {
		t.Fatal(err)
	}
	before, err := s.Get(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	review, err := s.Preview(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(review.Additions) != 1 || review.Additions[0].UUID != candidate || len(review.Additions[0].Sources) != 2 || review.Additions[0].Story == nil || !review.Additions[0].Story.Broken {
		t.Fatalf("bad union addition: %#v", review)
	}
	if len(review.Removals) != 1 || review.Removals[0].UUID != old || len(review.Warnings) != 1 || review.Warnings[0].StoryUUID != missing {
		t.Fatalf("unsafe removals: %#v", review)
	}
	after, err := s.Get(ctx, shelf.ID)
	if err != nil || after != before {
		t.Fatalf("preview mutated state: before=%#v after=%#v err=%v", before, after, err)
	}
}
