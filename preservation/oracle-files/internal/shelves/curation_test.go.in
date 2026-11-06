package shelves

import (
	"context"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
)

func seedStory(t *testing.T, db *database.Database, storyUUID, title string) int64 {
	t.Helper()
	result, err := db.Writer().Exec(`INSERT INTO stories(uuid,embedded_title,display_name_normalized) VALUES(?,?,?)`, storyUUID, title, strings.ToLower(title))
	if err != nil {
		t.Fatal(err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	hash := strings.Repeat(strings.ReplaceAll(storyUUID, "-", ""), 2)
	if _, err := db.Writer().Exec(`INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path) VALUES(?,?,'zip',?,1,?)`, id, storyUUID+".zip", hash, "archives/"+storyUUID+".zip"); err != nil {
		t.Fatal(err)
	}
	return id
}

func TestCurationIsAtomicProtectedIdempotentAndReversible(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	first := "123e4567-e89b-42d3-a456-426614174000"
	second := "223e4567-e89b-42d3-a456-426614174000"
	missing := "323e4567-e89b-42d3-a456-426614174000"
	seedStory(t, db, first, "First")
	seedStory(t, db, second, "Second")
	shelf, err := s.Create(ctx, "Manual")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.AddStories(ctx, shelf.ID, []string{first, missing}); err == nil {
		t.Fatal("invalid batch accepted")
	}
	current, err := s.Get(ctx, shelf.ID)
	if err != nil || current.Count != 0 || current.Revision != 1 {
		t.Fatalf("partial add: %#v %v", current, err)
	}
	current, err = s.AddStories(ctx, shelf.ID, []string{first, first, second})
	if err != nil || current.Count != 2 || current.Revision != 2 {
		t.Fatalf("add: %#v %v", current, err)
	}
	current, err = s.AddStories(ctx, shelf.ID, []string{first})
	if err != nil || current.Revision != 2 {
		t.Fatalf("non-idempotent add: %#v %v", current, err)
	}
	if _, err := s.SetProtected(ctx, shelf.ID, first, false); err == nil {
		t.Fatal("source-less member could be unprotected")
	}
	if _, err := s.ReorderMembers(ctx, shelf.ID, []string{second, first}); err != nil {
		t.Fatal(err)
	}
	members, err := s.Members(ctx, shelf.ID)
	if err != nil || members[0].UUID != second || !members[1].Protected {
		t.Fatalf("order/protection: %#v %v", members, err)
	}
	if _, err := s.RemoveStories(ctx, shelf.ID, []string{first, missing}); err == nil {
		t.Fatal("unknown member batch accepted")
	}
	current, err = s.Get(ctx, shelf.ID)
	if err != nil || current.Count != 2 {
		t.Fatalf("partial removal: %#v %v", current, err)
	}
	if _, err := s.RemoveStories(ctx, shelf.ID, []string{first}); err != nil {
		t.Fatal(err)
	}
	exclusions, err := s.Exclusions(ctx, shelf.ID)
	if err != nil || len(exclusions) != 0 {
		t.Fatalf("ordinary removal created an exclusion: %#v %v", exclusions, err)
	}
	if _, err := s.ClearExclusion(ctx, shelf.ID, first); err != nil {
		t.Fatal(err)
	}
	current, err = s.Get(ctx, shelf.ID)
	if err != nil || current.Count != 1 {
		t.Fatal("clearing exclusion added membership")
	}
	if _, err := s.AddStories(ctx, shelf.ID, []string{first}); err != nil {
		t.Fatal(err)
	}
	members, err = s.Members(ctx, shelf.ID)
	if err != nil || !members[1].Protected {
		t.Fatalf("manual addition unprotected: %#v %v", members, err)
	}
}

func TestFailedBulkWriteRollsBackEveryMemberAndRevision(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	a := "123e4567-e89b-42d3-a456-426614174000"
	b := "223e4567-e89b-42d3-a456-426614174000"
	seedStory(t, db, a, "A")
	seedStory(t, db, b, "B")
	shelf, err := s.Create(ctx, "Atomic")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Writer().Exec(`CREATE TRIGGER fail_member BEFORE INSERT ON shelf_stories WHEN NEW.story_uuid='223e4567-e89b-42d3-a456-426614174000' BEGIN SELECT RAISE(ABORT,'test failure'); END`); err != nil {
		t.Fatal(err)
	}
	if _, err := s.AddStories(ctx, shelf.ID, []string{a, b}); err == nil {
		t.Fatal("write failure ignored")
	}
	current, err := s.Get(ctx, shelf.ID)
	if err != nil || current.Count != 0 || current.Revision != 1 {
		t.Fatalf("partial write: %#v %v", current, err)
	}
}

type shelfOfficialTitles map[string]library.OfficialMetadata

func (p shelfOfficialTitles) FindByUUIDs(_ context.Context, ids []string) (map[string]library.OfficialMetadata, error) {
	result := map[string]library.OfficialMetadata{}
	for _, id := range ids {
		if metadata, ok := p[id]; ok {
			result[id] = metadata
		}
	}
	return result, nil
}

func (shelfOfficialTitles) DisplayLocale() string { return "en-GB" }

func TestManualMembersCacheResolvedTitlesForUnavailableStories(t *testing.T) {
	for _, operation := range []string{"create", "add"} {
		t.Run(operation, func(t *testing.T) {
			s, db := newShelfService(t)
			ctx := context.Background()
			ids := []string{"123e4567-e89b-42d3-a456-426614174000", "223e4567-e89b-42d3-a456-426614174000", "323e4567-e89b-42d3-a456-426614174000", "423e4567-e89b-42d3-a456-426614174000"}
			for i, id := range ids {
				seedStory(t, db, id, []string{"", "Embedded", "", "Old embedded"}[i])
			}
			s.library = library.NewQuery(db.SQL(), shelfOfficialTitles{
				ids[0]: {Title: "Official"},
				ids[3]: {Title: "Official precedence"},
			})
			var shelf Shelf
			var err error
			if operation == "create" {
				shelf, err = s.CreateManual(ctx, "Titles", ids)
			} else {
				shelf, err = s.Create(ctx, "Titles")
				if err == nil {
					shelf, err = s.AddStories(ctx, shelf.ID, ids)
				}
			}
			if err != nil {
				t.Fatal(err)
			}
			titles := []string{"Official", "Embedded", "Story " + ids[2], "Official precedence"}
			view, err := s.Open(ctx, shelf.ID, library.StoryLibraryQuery{})
			if err != nil || len(view.Members) != len(ids) {
				t.Fatalf("open available members: %#v %v", view, err)
			}
			for i, member := range view.Members {
				if member.Story == nil || member.Story.Title != titles[i] || member.CachedTitle != titles[i] {
					t.Fatalf("member %d does not cache its display title: %#v", i, member)
				}
			}
			if _, err := db.Writer().Exec(`DELETE FROM story_archives`); err != nil {
				t.Fatal(err)
			}
			view, err = s.Open(ctx, shelf.ID, library.StoryLibraryQuery{})
			if err != nil || len(view.Members) != len(ids) || view.Shelf.UnavailableCount != len(ids) {
				t.Fatalf("open unavailable members: %#v %v", view, err)
			}
			for i, member := range view.Members {
				if member.Story != nil || member.CachedTitle != titles[i] {
					t.Fatalf("unavailable member %d lost its title: %#v", i, member)
				}
			}
		})
	}
}
