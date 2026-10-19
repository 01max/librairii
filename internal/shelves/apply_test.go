package shelves

import (
	"context"
	"errors"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestReviewApplyPersistsOriginsProtectionExclusionsAndDeferrals(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	manual := "123e4567-e89b-42d3-a456-426614174000"
	keep := "223e4567-e89b-42d3-a456-426614174000"
	remove := "323e4567-e89b-42d3-a456-426614174000"
	add := "423e4567-e89b-42d3-a456-426614174000"
	reject := "523e4567-e89b-42d3-a456-426614174000"
	later := "623e4567-e89b-42d3-a456-426614174000"
	for id, title := range map[string]string{manual: "Manual", keep: "Old keep", remove: "Old remove", add: "Moon forest", reject: "Moon reject", later: "Forest later"} {
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
	shelf, err := s.CreateManual(ctx, "Approved", []string{manual, keep, remove})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.AttachSources(ctx, shelf.ID, []int64{moon.ID, forest.ID}); err != nil {
		t.Fatal(err)
	}
	for _, id := range []string{keep, remove} {
		if _, err := s.SetProtected(ctx, shelf.ID, id, false); err != nil {
			t.Fatal(err)
		}
	}
	review, err := s.Preview(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	before, err := s.Get(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.Apply(ctx, shelf.ID, review.Token, []Decision{{UUID: add, Action: "add"}, {UUID: manual, Action: "remove"}}); !errors.Is(err, ErrInvalidInput) {
		t.Fatalf("out-of-preview batch accepted: %v", err)
	}
	after, err := s.Get(ctx, shelf.ID)
	if err != nil || after != before {
		t.Fatal("invalid batch partially applied")
	}
	result, err := s.Apply(ctx, shelf.ID, review.Token, []Decision{{UUID: add, Action: "add"}, {UUID: reject, Action: "exclude"}, {UUID: later, Action: "defer"}, {UUID: keep, Action: "keep"}, {UUID: remove, Action: "remove"}})
	if err != nil || result.Count != 3 || result.Revision != before.Revision+1 {
		t.Fatalf("apply: %#v %v", result, err)
	}
	opened, err := s.Open(ctx, shelf.ID, library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	byUUID := map[string]Member{}
	for _, member := range opened.Members {
		byUUID[member.UUID] = member
	}
	if !byUUID[manual].Protected || !byUUID[keep].Protected || byUUID[add].Protected || len(byUUID[add].Origins) != 2 || len(opened.Exclusions) != 2 {
		t.Fatalf("decisions lost: %#v", opened)
	}
	fresh, err := s.Preview(ctx, shelf.ID)
	if err != nil || len(fresh.Additions) != 1 || fresh.Additions[0].UUID != later || len(fresh.Removals) != 0 {
		t.Fatalf("repeated decisions: %#v %v", fresh, err)
	}
	if _, err := dynamic.ReplaceQuery(ctx, moon.ID, library.StoryLibraryQuery{Name: "none"}); err != nil {
		t.Fatal(err)
	}
	fresh, err = s.Preview(ctx, shelf.ID)
	if err != nil || len(fresh.Removals) != 0 {
		t.Fatalf("remaining source ignored: %#v %v", fresh, err)
	}
	if _, err := dynamic.ReplaceQuery(ctx, forest.ID, library.StoryLibraryQuery{Name: "none"}); err != nil {
		t.Fatal(err)
	}
	fresh, err = s.Preview(ctx, shelf.ID)
	if err != nil || len(fresh.Removals) != 1 || fresh.Removals[0].UUID != add || len(fresh.Removals[0].Origins) != 2 {
		t.Fatalf("acceptance origins lost: %#v %v", fresh, err)
	}
	if _, err := s.Apply(ctx, shelf.ID, fresh.Token, []Decision{{UUID: add, Action: "keep"}}); err != nil {
		t.Fatal(err)
	}
	noChanges, err := s.Preview(ctx, shelf.ID)
	if err != nil || !noChanges.UpToDate {
		t.Fatalf("kept story repeatedly proposed: %#v %v", noChanges, err)
	}
	before, err = s.Get(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	after, err = s.Apply(ctx, shelf.ID, noChanges.Token, nil)
	if err != nil || before.Revision != after.Revision {
		t.Fatalf("no-op changed revision: %#v %v", after, err)
	}
}

func TestReviewWriteFailureRollsBackAndKeepsPreviewRetryable(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	a := "123e4567-e89b-42d3-a456-426614174000"
	b := "223e4567-e89b-42d3-a456-426614174000"
	seedStory(t, db, a, "A moon")
	seedStory(t, db, b, "B moon")
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	collection, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
	if err != nil {
		t.Fatal(err)
	}
	shelf, err := s.Create(ctx, "Atomic")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.AttachSources(ctx, shelf.ID, []int64{collection.ID}); err != nil {
		t.Fatal(err)
	}
	review, err := s.Preview(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Writer().Exec(`CREATE TRIGGER fail_review BEFORE INSERT ON shelf_exclusions BEGIN SELECT RAISE(ABORT,'test failure'); END`); err != nil {
		t.Fatal(err)
	}
	decisions := []Decision{{UUID: a, Action: "add"}, {UUID: b, Action: "exclude"}}
	if _, err := s.Apply(ctx, shelf.ID, review.Token, decisions); err == nil {
		t.Fatal("write failure ignored")
	}
	current, err := s.Get(ctx, shelf.ID)
	if err != nil || current.Count != 0 || current.Revision != review.Revision {
		t.Fatalf("partial review applied: %#v %v", current, err)
	}
	var origins int
	if err := db.SQL().QueryRow(`SELECT count(*) FROM shelf_story_origins`).Scan(&origins); err != nil || origins != 0 {
		t.Fatalf("partial origins: %d %v", origins, err)
	}
	if _, err := db.Writer().Exec(`DROP TRIGGER fail_review`); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Apply(ctx, shelf.ID, review.Token, decisions); err != nil {
		t.Fatalf("rolled-back preview could not retry: %v", err)
	}
	if _, err := s.Apply(ctx, shelf.ID, review.Token, decisions); !errors.Is(err, ErrStaleReview) {
		t.Fatalf("consumed token replayed: %v", err)
	}
}
