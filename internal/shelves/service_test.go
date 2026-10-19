package shelves

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
)

func newShelfService(t *testing.T) (*Service, *database.Database) {
	t.Helper()
	db, err := database.Open(context.Background(), filepath.Join(t.TempDir(), "library.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	s, err := NewService(db.Writer(), library.NewQuery(db.SQL(), nil))
	if err != nil {
		t.Fatal(err)
	}
	return s, db
}

func TestManualShelfCRUDKeepsIndependentNamesAndUnavailableUUIDs(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	if _, err := db.Writer().Exec(`INSERT INTO collections(name,normalized_name,position,query_version,query_payload) VALUES('Same','same',0,5,'{}')`); err != nil {
		t.Fatal(err)
	}
	first, err := s.Create(ctx, "  Same ")
	if err != nil {
		t.Fatal(err)
	}
	second, err := s.Create(ctx, "Other")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := s.Create(ctx, "same"); err == nil {
		t.Fatal("duplicate name accepted")
	}
	if err := s.Reorder(ctx, []int64{second.ID, first.ID}); err != nil {
		t.Fatal(err)
	}
	if err := s.Reorder(ctx, []int64{second.ID}); err == nil {
		t.Fatal("stale reorder accepted")
	}
	if _, err := db.Writer().Exec(`INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title) VALUES(?,'123e4567-e89b-42d3-a456-426614174000',0,'Missing story')`, first.ID); err != nil {
		t.Fatal(err)
	}
	renamed, err := s.Rename(ctx, first.ID, "Kept")
	if err != nil {
		t.Fatal(err)
	}
	if renamed.Count != 1 || renamed.UnavailableCount != 1 || renamed.Revision != 2 {
		t.Fatalf("unexpected summary: %#v", renamed)
	}
	members, err := s.Members(ctx, first.ID)
	if err != nil || len(members) != 1 || members[0].CachedTitle != "Missing story" || !members[0].Protected {
		t.Fatalf("lost missing member: %#v %v", members, err)
	}
	if err := s.Delete(ctx, first.ID); err != nil {
		t.Fatal(err)
	}
	var remaining int
	if err := db.SQL().QueryRow(`SELECT count(*) FROM collections`).Scan(&remaining); err != nil || remaining != 1 {
		t.Fatalf("collection changed: %d %v", remaining, err)
	}
	listed, err := s.List(ctx)
	if err != nil || len(listed) != 1 || listed[0].ID != second.ID {
		t.Fatalf("unrelated shelf changed: %#v %v", listed, err)
	}
}
