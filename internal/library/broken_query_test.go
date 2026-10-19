package library

import (
	"context"
	"testing"
)

func TestDedicatedBrokenQueryCompositionAndVisibility(t *testing.T) {
	ctx := context.Background()
	query, repository, db := newLibraryQuery(t, nil)
	damaged := createQueryableStory(t, repository, "00112233-4455-4677-8899-aabbccddeeff", "Damaged", "a")
	createQueryableStory(t, repository, "11112222-3333-4444-8555-666677778888", "Healthy", "b")
	if _, err := db.Exec("UPDATE stories SET is_broken=1 WHERE id=?", damaged.ID); err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		name  string
		query StoryLibraryQuery
		count int
	}{
		{"default", StoryLibraryQuery{}, 1},
		{"include", StoryLibraryQuery{BrokenVisibility: BrokenInclude}, 2},
		{"only", StoryLibraryQuery{BrokenVisibility: BrokenOnly}, 1},
		{"contradiction", StoryLibraryQuery{BrokenState: BooleanTrue}, 0},
		{"explicit false", StoryLibraryQuery{BrokenVisibility: BrokenInclude, BrokenState: BooleanFalse}, 1},
		{"root hide over branches", StoryLibraryQuery{AnyOf: []StoryLibraryQuery{{BrokenState: BooleanTrue}, {BrokenState: BooleanFalse}}}, 1},
		{"root only over branches", StoryLibraryQuery{BrokenVisibility: BrokenOnly, AnyOf: []StoryLibraryQuery{{Name: "dam"}, {Name: "healthy"}}}, 1},
		{"nested contradiction", StoryLibraryQuery{BrokenVisibility: BrokenInclude, BrokenState: BooleanFalse, AnyOf: []StoryLibraryQuery{{BrokenState: BooleanTrue}}}, 0},
	} {
		t.Run(test.name, func(t *testing.T) {
			test.query.PageSize = 1
			page, err := query.Search(ctx, test.query)
			if err != nil || page.TotalItems != test.count {
				t.Fatalf("page=%#v, err=%v", page, err)
			}
			count, err := query.Count(ctx, test.query)
			if err != nil || count != test.count {
				t.Fatalf("count=%d, err=%v", count, err)
			}
			exports, err := query.ExportQuery(ctx, test.query)
			if err != nil || len(exports) != test.count {
				t.Fatalf("exports=%d, err=%v", len(exports), err)
			}
		})
	}
	detail, err := query.Detail(ctx, damaged.ID)
	if err != nil || !detail.Story.Broken {
		t.Fatalf("detail=%#v, %v", detail, err)
	}
}

func TestLegacyBrokenURLTranslation(t *testing.T) {
	for _, state := range []string{"true", "false", "ignored"} {
		query, err := DecodeStoryLibraryQuery("#/library?v=1&includeBroken=true&bool=9:"+state, 9)
		if err != nil {
			t.Fatal(err)
		}
		expected := BooleanFilterState(state)
		if state == "ignored" {
			expected = ""
		}
		if query.BrokenState != expected || query.BrokenVisibility != BrokenInclude || len(query.BooleanFilters) != 0 {
			t.Fatalf("query=%#v", query)
		}
	}
	if _, err := DecodeStoryLibraryQuery("#/library?v=1&bool=9:true&bool=9:false", 9); err == nil {
		t.Fatal("duplicate legacy criteria accepted")
	}
}

func TestRetiredBrokenIDIsOnlyTranslatedForLegacyVersions(t *testing.T) {
	ctx := context.Background()
	query, repository, db := newLibraryQuery(t, nil)
	flagged := createQueryableStory(t, repository, "00112233-4455-4677-8899-aabbccddeeff", "Flagged", "a")
	tagged := createQueryableStory(t, repository, "11112222-3333-4444-8555-666677778888", "Tagged", "b")
	if _, err := db.Exec(`INSERT INTO legacy_broken_tag VALUES (1,9);
        INSERT INTO tag_definitions (id,key,normalized_key,label,color,kind,source,presentation,position,is_protected)
        VALUES (9,'broken','broken','My broken tag','#405CF5','boolean','user','default',0,0)`); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec("UPDATE stories SET is_broken=1 WHERE id=?", flagged.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec("INSERT INTO story_tag_assignments (story_id,definition_id,source) VALUES (?,9,'manual')", tagged.ID); err != nil {
		t.Fatal(err)
	}
	info, err := query.CompatibilityInfo(ctx)
	if err != nil || info.LegacyBrokenTagID != 9 {
		t.Fatalf("compatibility=%#v, %v", info, err)
	}
	for _, test := range []struct {
		hash string
		id   int64
	}{
		{"#/library?v=1&includeBroken=true&bool=9:true", flagged.ID},
		{"#/library?v=2&brokenVisibility=include&bool=9:true", tagged.ID},
	} {
		request, err := DecodeStoryLibraryQuery(test.hash, info.LegacyBrokenTagID)
		if err != nil {
			t.Fatal(err)
		}
		page, err := query.Search(ctx, request)
		if err != nil || page.TotalItems != 1 || page.Stories[0].ID != test.id {
			t.Fatalf("page=%#v, err=%v", page, err)
		}
	}
	for _, mode := range []BrokenVisibility{BrokenHide, BrokenOnly, BrokenInclude} {
		predicate, args := storyLibraryPredicate(StoryLibraryQuery{BrokenVisibility: mode}, "")
		statement := "EXPLAIN QUERY PLAN SELECT s.id FROM stories s JOIN story_archives a ON a.story_id=s.id"
		if predicate != "" {
			statement += " WHERE " + predicate
		}
		rows, err := db.Query(statement, args...)
		if err != nil {
			t.Fatal(err)
		}
		for rows.Next() {
			var id, parent, unused int
			var detail string
			if err := rows.Scan(&id, &parent, &unused, &detail); err != nil {
				t.Fatal(err)
			}
			t.Logf("%s: %s", mode, detail)
		}
		if err := rows.Err(); err != nil {
			t.Fatal(err)
		}
		rows.Close()
	}
}
