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
