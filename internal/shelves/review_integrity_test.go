package shelves

import (
	"context"
	"errors"
	"sync"
	"testing"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
)

func TestConcurrentReviewApplicationsCannotReplayTheSameDecision(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	id := "123e4567-e89b-42d3-a456-426614174000"
	seedStory(t, db, id, "Moon")
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	collection, err := dynamic.Create(ctx, "All", library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	shelf, err := s.Create(ctx, "Concurrent")
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
	start := make(chan struct{})
	results := make(chan error, 2)
	var workers sync.WaitGroup
	for i := 0; i < 2; i++ {
		workers.Add(1)
		go func() {
			defer workers.Done()
			<-start
			_, err := s.Apply(ctx, shelf.ID, review.Token, []Decision{{UUID: id, Action: "add"}})
			results <- err
		}()
	}
	close(start)
	workers.Wait()
	close(results)
	succeeded, stale := 0, 0
	for err := range results {
		if err == nil {
			succeeded++
		} else if errors.Is(err, ErrStaleReview) {
			stale++
		} else {
			t.Fatal(err)
		}
	}
	current, err := s.Get(ctx, shelf.ID)
	if err != nil || succeeded != 1 || stale != 1 || current.Count != 1 || current.Revision != review.Revision+1 {
		t.Fatalf("replayed review: %#v success=%d stale=%d err=%v", current, succeeded, stale, err)
	}
}

func TestNewLinkedShelfInvalidatesCollectionDeletionPlan(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	id := "123e4567-e89b-42d3-a456-426614174000"
	seedStory(t, db, id, "Moon")
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	collection, err := dynamic.Create(ctx, "All", library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	first := createSourcedShelf(t, s, "First", []int64{collection.ID}, []string{id})
	plan, err := s.PreviewCollectionDeletion(ctx, collection.ID)
	if err != nil {
		t.Fatal(err)
	}
	createSourcedShelf(t, s, "Late", []int64{collection.ID}, []string{id})
	if err := s.ApplyCollectionDeletion(ctx, plan.Token, true, nil); !errors.Is(err, ErrStaleReview) {
		t.Fatalf("new linked Shelf was silently omitted: %v", err)
	}
	current, err := s.Get(ctx, first.ID)
	if err != nil || current.SourceCount != 1 || current.Revision != first.Revision {
		t.Fatalf("stale deletion changed state: %#v %v", current, err)
	}
	if _, err := dynamic.Open(ctx, collection.ID); err != nil {
		t.Fatalf("stale plan deleted Collection: %v", err)
	}
}

func TestSourceEditInvalidatesDetachAndBrokenFlagsDoNotInvalidateKeep(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	id := "123e4567-e89b-42d3-a456-426614174000"
	storyID := seedStory(t, db, id, "Moon")
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	collection, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
	if err != nil {
		t.Fatal(err)
	}
	shelf := createSourcedShelf(t, s, "Approved", []int64{collection.ID}, []string{id})
	plan, err := s.PreviewSourceChange(ctx, shelf.ID, nil)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := dynamic.ReplaceQuery(ctx, collection.ID, library.StoryLibraryQuery{Name: "forest"}); err != nil {
		t.Fatal(err)
	}
	if _, err := s.ApplySourceChange(ctx, shelf.ID, plan.Token, true, nil); !errors.Is(err, ErrStaleReview) {
		t.Fatalf("stale detach applied: %v", err)
	}
	review, err := s.Preview(ctx, shelf.ID)
	if err != nil || len(review.Removals) != 1 {
		t.Fatalf("removal review: %#v %v", review, err)
	}
	if _, err := db.Writer().Exec(`UPDATE stories SET is_broken=1 WHERE id=?`, storyID); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Apply(ctx, shelf.ID, review.Token, []Decision{{UUID: id, Action: "keep"}}); err != nil {
		t.Fatalf("flag alone invalidated Keep: %v", err)
	}
	fresh, err := s.Preview(ctx, shelf.ID)
	if err != nil || !fresh.UpToDate {
		t.Fatalf("Keep repeated: %#v %v", fresh, err)
	}
}

func TestEvaluationFailurePausesRemovalsWithoutDroppingHealthyMatches(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	old := "123e4567-e89b-42d3-a456-426614174000"
	candidate := "223e4567-e89b-42d3-a456-426614174000"
	seedStory(t, db, old, "Old")
	seedStory(t, db, candidate, "Moon")
	if _, err := db.Writer().Exec(`INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,presentation,position,is_protected) VALUES(9,'favorite','favorite','Favorite','#405CF5','boolean','user','default',0,0)`); err != nil {
		t.Fatal(err)
	}
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	healthy, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
	if err != nil {
		t.Fatal(err)
	}
	failed, err := dynamic.Create(ctx, "Tagged", library.StoryLibraryQuery{BooleanFilters: []library.BooleanFilter{{DefinitionID: 9, State: library.BooleanTrue}}})
	if err != nil {
		t.Fatal(err)
	}
	shelf, err := s.CreateManual(ctx, "Approved", []string{old})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.AttachSources(ctx, shelf.ID, []int64{healthy.ID, failed.ID}); err != nil {
		t.Fatal(err)
	}
	if _, err := s.SetProtected(ctx, shelf.ID, old, false); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Writer().Exec(`DROP TABLE story_tag_assignments`); err != nil {
		t.Fatal(err)
	}
	review, err := s.Preview(ctx, shelf.ID)
	if err != nil || !review.Incomplete || len(review.Removals) != 0 || len(review.Additions) != 1 || review.Additions[0].UUID != candidate || review.Warnings[0].Code != "source_evaluation_failed" {
		t.Fatalf("failed source became empty: %#v %v", review, err)
	}
}

func TestApprovedDecisionsAndOriginsSurviveRestartAndUUIDReimport(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	a := "123e4567-e89b-42d3-a456-426614174000"
	b := "223e4567-e89b-42d3-a456-426614174000"
	originalID := seedStory(t, db, a, "A")
	seedStory(t, db, b, "B")
	dynamic, err := collections.NewService(db.Writer())
	if err != nil {
		t.Fatal(err)
	}
	collection, err := dynamic.Create(ctx, "All", library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	shelf := createSourcedShelf(t, s, "Approved", []int64{collection.ID}, []string{a})
	if _, err := s.SetProtected(ctx, shelf.ID, a, true); err != nil {
		t.Fatal(err)
	}
	review, err := s.Preview(ctx, shelf.ID)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.Apply(ctx, shelf.ID, review.Token, []Decision{{UUID: b, Action: "exclude"}}); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Writer().Exec(`DELETE FROM stories WHERE uuid=?`, a); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	reopened, err := database.Open(ctx, db.Path())
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	restored, err := NewService(reopened.Writer(), library.NewQuery(reopened.SQL(), nil))
	if err != nil {
		t.Fatal(err)
	}
	opened, err := restored.Open(ctx, shelf.ID, library.StoryLibraryQuery{})
	if err != nil || len(opened.Members) != 1 || !opened.Members[0].Protected || opened.Members[0].Story != nil || len(opened.Members[0].Origins) != 1 || len(opened.Exclusions) != 1 {
		t.Fatalf("decisions lost on restart: %#v %v", opened, err)
	}
	newID := seedStory(t, reopened, a, "Reimported")
	if newID == originalID {
		t.Fatal("fixture did not exercise changed numeric ID")
	}
	opened, err = restored.Open(ctx, shelf.ID, library.StoryLibraryQuery{})
	if err != nil || opened.Members[0].Story == nil || opened.Members[0].Story.ID != newID || !opened.Members[0].Protected {
		t.Fatalf("UUID not restored: %#v %v", opened, err)
	}
	fresh, err := restored.Preview(ctx, shelf.ID)
	if err != nil || len(fresh.Additions) != 0 || len(fresh.Removals) != 0 {
		t.Fatalf("persistent exclusion lost: %#v %v", fresh, err)
	}
}
