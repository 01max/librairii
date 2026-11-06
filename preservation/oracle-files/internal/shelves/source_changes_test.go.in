package shelves

import (
	"context"
	"errors"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func createSourcedShelf(t *testing.T, s *Service, name string, sources []int64, members []string) Shelf {
	t.Helper()
	ctx := context.Background()
	preview, err := s.PreviewCreation(ctx, CreationRequest{Name: name, CollectionIDs: sources})
	if err != nil {
		t.Fatal(err)
	}
	created, err := s.CreateFromReview(ctx, preview.Token, members)
	if err != nil {
		t.Fatal(err)
	}
	return created
}

func TestDetachReviewsCoverageAndDoesNotUnpinOnReattach(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	a := "123e4567-e89b-42d3-a456-426614174000"
	b := "223e4567-e89b-42d3-a456-426614174000"
	manual := "323e4567-e89b-42d3-a456-426614174000"
	for id, title := range map[string]string{a: "Moon forest", b: "Moon", manual: "Ocean"} {
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
	shelf := createSourcedShelf(t, s, "Approved", []int64{moon.ID, forest.ID}, []string{a, b})
	if _, err := s.AddStories(ctx, shelf.ID, []string{manual}); err != nil {
		t.Fatal(err)
	}
	before, err := s.Get(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	plan, err := s.PreviewSourceChange(ctx, shelf.ID, []int64{forest.ID})
	if err != nil || len(plan.Affected) != 1 || plan.Affected[0].UUID != b {
		t.Fatalf("coverage plan: %#v %v", plan, err)
	}
	if _, err := s.ApplySourceChange(ctx, shelf.ID, plan.Token, false, []Decision{{UUID: b, Action: "defer"}}); !errors.Is(err, ErrInvalidInput) {
		t.Fatalf("deferred detach applied: %v", err)
	}
	after, err := s.Get(ctx, shelf.ID)
	if err != nil || after != before {
		t.Fatal("preview/defer changed state")
	}
	if _, err := s.ApplySourceChange(ctx, shelf.ID, plan.Token, true, nil); err != nil {
		t.Fatal(err)
	}
	opened, err := s.Open(ctx, shelf.ID, library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	byUUID := map[string]Member{}
	for _, member := range opened.Members {
		byUUID[member.UUID] = member
	}
	if opened.Shelf.Count != 3 || opened.Shelf.SourceCount != 1 || byUUID[a].Protected || !byUUID[b].Protected || !byUUID[manual].Protected {
		t.Fatalf("coverage/protection changed incorrectly: %#v", opened)
	}
	if len(byUUID[a].Origins) != 2 || byUUID[a].Origins[0].Active {
		t.Fatalf("detached origin lost: %#v", opened.Members[0].Origins)
	}
	if _, err := s.AttachSources(ctx, shelf.ID, []int64{moon.ID}); err != nil {
		t.Fatal(err)
	}
	members, err := s.Members(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	protected := false
	for _, member := range members {
		if member.UUID == b {
			protected = member.Protected
		}
	}
	if !protected {
		t.Fatal("reattachment silently removed protection")
	}
	last, err := s.PreviewSourceChange(ctx, shelf.ID, nil)
	if err != nil || len(last.Affected) != 1 || last.Affected[0].UUID != a {
		t.Fatalf("last source plan: %#v %v", last, err)
	}
	result, err := s.ApplySourceChange(ctx, shelf.ID, last.Token, false, []Decision{{UUID: a, Action: "remove"}})
	if err != nil || result.SourceCount != 0 || result.Count != 2 {
		t.Fatalf("last source detach: %#v %v", result, err)
	}
	updates, err := s.Preview(ctx, shelf.ID)
	if err != nil || !updates.UpToDate || len(updates.Additions) != 0 || len(updates.Removals) != 0 {
		t.Fatalf("manual Shelf has rule proposals: %#v %v", updates, err)
	}
}

func TestDetachCannotInferRemovalFromUnavailableOrUnknownCoverage(t *testing.T) {
	for _, kind := range []string{"unavailable", "invalid source"} {
		t.Run(kind, func(t *testing.T) {
			s, db := newShelfService(t)
			ctx := context.Background()
			id := "123e4567-e89b-42d3-a456-426614174000"
			seedStory(t, db, id, "Moon")
			dynamic, err := collections.NewService(db.Writer())
			if err != nil {
				t.Fatal(err)
			}
			moon, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
			if err != nil {
				t.Fatal(err)
			}
			other, err := dynamic.Create(ctx, "Other", library.StoryLibraryQuery{Name: "forest"})
			if err != nil {
				t.Fatal(err)
			}
			shelf := createSourcedShelf(t, s, "Approved", []int64{moon.ID, other.ID}, []string{id})
			desired := []int64{}
			if kind == "unavailable" {
				_, err = db.Writer().Exec(`DELETE FROM stories WHERE uuid=?`, id)
			} else {
				desired = []int64{other.ID}
				_, err = db.Writer().Exec(`UPDATE collections SET query_version=99 WHERE id=?`, other.ID)
			}
			if err != nil {
				t.Fatal(err)
			}
			plan, err := s.PreviewSourceChange(ctx, shelf.ID, desired)
			if err != nil || len(plan.Affected) != 1 {
				t.Fatalf("plan: %#v %v", plan, err)
			}
			if _, err := s.ApplySourceChange(ctx, shelf.ID, plan.Token, false, []Decision{{UUID: id, Action: "remove"}}); !errors.Is(err, ErrInvalidInput) {
				t.Fatalf("uncertain removal permitted: %v", err)
			}
			result, err := s.ApplySourceChange(ctx, shelf.ID, plan.Token, true, nil)
			if err != nil || result.Count != 1 {
				t.Fatalf("keep all: %#v %v", result, err)
			}
			members, err := s.Members(ctx, shelf.ID)
			if err != nil || !members[0].Protected {
				t.Fatal("retained orphan not protected")
			}
		})
	}
}

func TestCollectionDeletionReviewsAllShelvesAndRollsBackTogether(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	id := "123e4567-e89b-42d3-a456-426614174000"
	seedStory(t, db, id, "Moon")
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	all, err := dynamic.Create(ctx, "All", library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	moon, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
	if err != nil {
		t.Fatal(err)
	}
	first := createSourcedShelf(t, s, "First", []int64{all.ID}, []string{id})
	second := createSourcedShelf(t, s, "Second", []int64{all.ID, moon.ID}, []string{id})
	if err := dynamic.Delete(ctx, all.ID); !errors.Is(err, collections.ErrCollectionInUse) {
		t.Fatalf("linked deletion bypassed review: %v", err)
	}
	plan, err := s.PreviewCollectionDeletion(ctx, all.ID)
	if err != nil || len(plan.Shelves) != 2 || len(plan.Shelves[0].Affected) != 1 || len(plan.Shelves[1].Affected) != 0 {
		t.Fatalf("delete plan: %#v %v", plan, err)
	}
	if _, err := db.Writer().Exec(`CREATE TRIGGER fail_collection_delete BEFORE DELETE ON collections BEGIN SELECT RAISE(ABORT,'test failure'); END`); err != nil {
		t.Fatal(err)
	}
	if err := s.ApplyCollectionDeletion(ctx, plan.Token, true, nil); err == nil {
		t.Fatal("delete failure ignored")
	}
	for _, shelfID := range []int64{first.ID, second.ID} {
		members, err := s.Members(ctx, shelfID)
		if err != nil || len(members) != 1 || members[0].Protected {
			t.Fatal("partial detach/protection committed")
		}
	}
	if _, err := db.Writer().Exec(`DROP TRIGGER fail_collection_delete`); err != nil {
		t.Fatal(err)
	}
	if err := s.ApplyCollectionDeletion(ctx, plan.Token, false, []ShelfDecisions{{ShelfID: first.ID, Decisions: []Decision{{UUID: id, Action: "keep"}}}}); err != nil {
		t.Fatal(err)
	}
	opened, err := s.Open(ctx, first.ID, library.StoryLibraryQuery{})
	if err != nil || opened.Shelf.SourceCount != 0 || !opened.Members[0].Protected || len(opened.Members[0].Origins) != 1 || opened.Members[0].Origins[0].Name != "All" || opened.Members[0].Origins[0].CollectionID != 0 {
		t.Fatalf("deleted attribution lost: %#v %v", opened, err)
	}
	other, err := s.Open(ctx, second.ID, library.StoryLibraryQuery{})
	if err != nil || other.Shelf.SourceCount != 1 || other.Members[0].Protected {
		t.Fatalf("overlap protection changed: %#v %v", other, err)
	}
	if _, err := dynamic.Open(ctx, all.ID); !errors.Is(err, collections.ErrCollectionNotFound) {
		t.Fatalf("Collection not deleted: %v", err)
	}
}
