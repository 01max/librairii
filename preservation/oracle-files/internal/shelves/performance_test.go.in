package shelves

import (
	"context"
	"fmt"
	"sort"
	"testing"
	"time"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
)

func TestLargeLibraryReviewUnionAndApplyBudget(t *testing.T) {
	service, db := newShelfService(t)
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	tx, err := db.Writer().BeginTx(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()
	for index := range 5000 {
		uuid := fmt.Sprintf("123e4567-e89b-42d3-a456-%012d", index)
		title := fmt.Sprintf("Story %05d shared %s", index, []string{"moon", "forest"}[index%2])
		result, err := tx.ExecContext(ctx, `INSERT INTO stories(uuid,embedded_title,display_name_normalized,is_broken) VALUES(?,?,?,?)`, uuid, title, title, index%5 == 0)
		if err != nil {
			t.Fatal(err)
		}
		id, _ := result.LastInsertId()
		if _, err := tx.ExecContext(ctx, `INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path) VALUES(?,?,'zip',?,1,?)`, id, uuid+".zip", fmt.Sprintf("%064d", index), "archives/"+uuid+".zip"); err != nil {
			t.Fatal(err)
		}
	}
	if err := tx.Commit(); err != nil {
		t.Fatal(err)
	}
	dynamic, _ := collections.NewService(db.Writer())
	all, err := dynamic.Create(ctx, "All shared", library.StoryLibraryQuery{Name: "shared"})
	if err != nil {
		t.Fatal(err)
	}
	moon, err := dynamic.Create(ctx, "Moon", library.StoryLibraryQuery{Name: "moon"})
	if err != nil {
		t.Fatal(err)
	}
	shelf, err := service.Create(ctx, "Review")
	if err != nil {
		t.Fatal(err)
	}
	if _, err = service.AttachSources(ctx, shelf.ID, []int64{all.ID, moon.ID}); err != nil {
		t.Fatal(err)
	}
	samples := []time.Duration{}
	var review Review
	for range 4 {
		started := time.Now()
		review, err = service.Preview(ctx, shelf.ID)
		samples = append(samples, time.Since(started))
		if err != nil || len(review.Additions) != 5000 || len(review.Removals) != 0 {
			t.Fatalf("large union: additions=%d removals=%d err=%v", len(review.Additions), len(review.Removals), err)
		}
	}
	decisions := make([]Decision, 50)
	for i := range decisions {
		decisions[i] = Decision{UUID: review.Additions[i].UUID, Action: "add"}
	}
	started := time.Now()
	if _, err = service.Apply(ctx, shelf.ID, review.Token, decisions); err != nil {
		t.Fatal(err)
	}
	applyTime := time.Since(started)
	after, err := service.Preview(ctx, shelf.ID)
	if err != nil || len(after.Additions) != 4950 {
		t.Fatalf("post-apply additions=%d err=%v", len(after.Additions), err)
	}
	opened, err := service.Open(ctx, shelf.ID, library.StoryLibraryQuery{PageSize: 100})
	members := opened.Members
	if err != nil || len(members) != 50 || len(members[0].Origins) == 0 {
		t.Fatalf("large apply lost decisions: %v", err)
	}
	sort.Slice(samples, func(i, j int) bool { return samples[i] < samples[j] })
	t.Logf("5000 stories, 2 overlapping sources: preview_max_ms=%.2f apply_50_ms=%.2f", float64(samples[len(samples)-1])/float64(time.Millisecond), float64(applyTime)/float64(time.Millisecond))
	if samples[len(samples)-1] > 5*time.Second || applyTime > 5*time.Second {
		t.Fatal("review exceeded 5s regression ceiling")
	}
}
