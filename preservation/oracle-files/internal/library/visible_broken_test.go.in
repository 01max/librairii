package library

import (
	"context"
	"testing"
)

func TestBrokenFlagsDoNotChangeQueryMembership(t *testing.T) {
	ctx := context.Background()
	query, repository, db := newLibraryQuery(t, nil)
	story := createQueryableStory(t, repository, "00112233-4455-4677-8899-aabbccddeeff", "Moon", "a")
	if _, err := db.Exec(`UPDATE stories SET is_broken=1 WHERE id=?`, story.ID); err != nil {
		t.Fatal(err)
	}
	page, err := query.Search(ctx, StoryLibraryQuery{Name: "moon", PageSize: 1})
	if err != nil || page.TotalItems != 1 || len(page.Stories) != 1 || !page.Stories[0].Broken {
		t.Fatalf("flagged story disappeared: %#v %v", page, err)
	}
	count, err := query.Count(ctx, StoryLibraryQuery{Name: "moon"})
	if err != nil || count != 1 {
		t.Fatalf("flag altered count: %d %v", count, err)
	}
	for _, restriction := range []StoryLibraryQuery{{BrokenState: BooleanTrue}, {BrokenVisibility: BrokenHide}, {IncludeBroken: true}, {AnyOf: []StoryLibraryQuery{{BrokenVisibility: BrokenOnly}}}} {
		if _, err := query.Search(ctx, restriction); err == nil {
			t.Fatalf("new query accepted retired restriction: %#v", restriction)
		}
	}
}
