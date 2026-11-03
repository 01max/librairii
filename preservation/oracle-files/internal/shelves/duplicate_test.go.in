package shelves

import (
	"context"
	"testing"
)

func TestDuplicateCopiesDecisionsAndOriginsUnderIndependentIDs(t *testing.T) {
	s, db := newShelfService(t)
	ctx := context.Background()
	a := "123e4567-e89b-42d3-a456-426614174000"
	b := "223e4567-e89b-42d3-a456-426614174000"
	seedStory(t, db, a, "Original")
	seedStory(t, db, b, "New source match")
	original, err := s.CreateManual(ctx, "Original", []string{a})
	if err != nil {
		t.Fatal(err)
	}
	for _, statement := range []struct {
		sql  string
		args []any
	}{
		{`INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload) VALUES(1,'All','all',0,5,'{}')`, nil},
		{`INSERT INTO shelf_sources(id,shelf_id,collection_id,collection_name) VALUES(1,?,1,'All')`, []any{original.ID}},
		{`INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id) VALUES(?,?,1)`, []any{original.ID, a}},
		{`INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)`, []any{original.ID, b}},
	} {
		if _, err := db.Writer().Exec(statement.sql, statement.args...); err != nil {
			t.Fatal(err)
		}
	}
	copy, err := s.Duplicate(ctx, original.ID, "Copy")
	if err != nil || copy.ID == original.ID || copy.Count != 1 || copy.SourceCount != 1 {
		t.Fatalf("duplicate: %#v %v", copy, err)
	}
	var sourceID int64
	if err := db.SQL().QueryRow(`SELECT source_id FROM shelf_story_origins WHERE shelf_id=?`, copy.ID).Scan(&sourceID); err != nil || sourceID == 1 {
		t.Fatalf("shared origin: %d %v", sourceID, err)
	}
	if _, err := s.ClearExclusion(ctx, copy.ID, b); err != nil {
		t.Fatal(err)
	}
	if _, err := s.RemoveStories(ctx, copy.ID, []string{a}); err != nil {
		t.Fatal(err)
	}
	current, err := s.Get(ctx, original.ID)
	if err != nil || current.Count != 1 {
		t.Fatalf("original changed: %#v %v", current, err)
	}
	excluded, err := s.Exclusions(ctx, original.ID)
	if err != nil || len(excluded) != 1 || excluded[0].UUID != b {
		t.Fatalf("original decisions changed: %#v %v", excluded, err)
	}
}
