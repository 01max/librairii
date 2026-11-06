package shelves

import (
	"context"
	"errors"
	"testing"
	"time"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestReviewSnapshotRejectsChangedInputsButIgnoresBrokenFlags(t *testing.T) {
	for _, change := range []string{"import", "definition", "membership", "availability", "matching metadata", "flag"} {
		t.Run(change, func(t *testing.T) {
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
			shelf, err := s.Create(ctx, "Review")
			if err != nil {
				t.Fatal(err)
			}
			if _, err := s.AttachSources(ctx, shelf.ID, []int64{collection.ID}); err != nil {
				t.Fatal(err)
			}
			review, err := s.Preview(ctx, shelf.ID)
			if err != nil || review.Token == "" {
				t.Fatalf("preview: %#v %v", review, err)
			}
			switch change {
			case "import":
				seedStory(t, db, "223e4567-e89b-42d3-a456-426614174000", "New moon")
			case "definition":
				_, err = dynamic.Rename(ctx, collection.ID, "Renamed")
			case "membership":
				_, err = s.AddStories(ctx, shelf.ID, []string{id})
			case "availability":
				_, err = db.Writer().Exec(`DELETE FROM stories WHERE id=?`, storyID)
			case "matching metadata":
				_, err = db.Writer().Exec(`UPDATE stories SET embedded_title='Forest',display_name_normalized='forest' WHERE id=?`, storyID)
			case "flag":
				_, err = db.Writer().Exec(`UPDATE stories SET is_broken=1 WHERE id=?`, storyID)
			}
			if err != nil {
				t.Fatal(err)
			}
			tx, err := db.Writer().BeginTx(ctx, nil)
			if err != nil {
				t.Fatal(err)
			}
			defer tx.Rollback()
			_, err = s.validatedReview(ctx, tx, shelf.ID, review.Token)
			if change == "flag" {
				if err != nil {
					t.Fatalf("flag alone invalidated membership review: %v", err)
				}
			} else if !errors.Is(err, ErrStaleReview) {
				t.Fatalf("changed %s accepted: %v", change, err)
			}
		})
	}
}

func TestReviewTicketsExpireAreBoundedAndBindTheirTarget(t *testing.T) {
	s, _ := newShelfService(t)
	now := time.Now()
	s.now = func() time.Time { return now }
	token := s.issueTicket("update", 7, [32]byte{}, reviewArguments{})
	if _, err := s.ticket(token, "update", 8); !errors.Is(err, ErrStaleReview) {
		t.Fatalf("wrong target accepted: %v", err)
	}
	if _, err := s.ticket("unknown", "update", 7); !errors.Is(err, ErrStaleReview) {
		t.Fatalf("unknown token accepted: %v", err)
	}
	now = now.Add(reviewLifetime + time.Second)
	if _, err := s.ticket(token, "update", 7); !errors.Is(err, ErrStaleReview) {
		t.Fatalf("expired token accepted: %v", err)
	}
	for i := 0; i < maxReviewTokens+5; i++ {
		s.issueTicket("update", 7, [32]byte{}, reviewArguments{})
	}
	if len(s.reviews) != maxReviewTokens {
		t.Fatalf("unbounded tickets: %d", len(s.reviews))
	}
}

func TestReviewDecisionsRejectConflictsAndOutOfPreviewActions(t *testing.T) {
	review := Review{Additions: []Candidate{{UUID: "a"}}, Removals: []Candidate{{UUID: "b"}}}
	for _, decisions := range [][]Decision{
		{{UUID: "a", Action: "add"}, {UUID: "a", Action: "exclude"}},
		{{UUID: "a", Action: "remove"}},
		{{UUID: "b", Action: "add"}},
		{{UUID: "unknown", Action: "defer"}},
	} {
		if _, err := validateDecisions(review, decisions); !errors.Is(err, ErrInvalidInput) {
			t.Fatalf("invalid decisions accepted: %#v %v", decisions, err)
		}
	}
	if actions, err := validateDecisions(review, nil); err != nil || len(actions) != 0 {
		t.Fatalf("empty decisions not a no-op: %#v %v", actions, err)
	}
}
