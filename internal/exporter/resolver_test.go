package exporter

import (
	"context"
	"errors"
	"reflect"
	"testing"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/shelves"
)

func TestResolverSnapshotsEverySupportedScope(t *testing.T) {
	t.Parallel()

	libraryQuery := &fakeLibraryResolver{
		pages: map[string]map[int][]int64{
			"current": {
				1: {1, 2},
				2: {3},
			},
			"moon": {
				1: {1, 2},
			},
			"forest": {
				1: {2, 3},
			},
		},
		stories: exportStories(1, 2, 3),
	}
	shelfService := fakeShelfResolver{
		7: openedShelf(7, "Bedtime", "moon"),
		8: openedShelf(8, "Adventures", "forest"),
	}
	resolver, err := NewResolver(libraryQuery, shelfService)
	if err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()

	selection, err := resolver.ResolveSelection(ctx, []int64{3, 1, 3})
	if err != nil {
		t.Fatal(err)
	}
	assertResolvedScope(
		t,
		selection,
		operations.ExportSourceSelection,
		[]int64{3, 1},
		nil,
		nil,
		0,
	)

	current, err := resolver.ResolveCurrentQuery(ctx, library.StoryLibraryQuery{
		Name:     "current",
		Page:     9,
		PageSize: 1,
		Sort:     library.SortImportedNewest,
	})
	if err != nil {
		t.Fatal(err)
	}
	assertResolvedScope(
		t,
		current,
		operations.ExportSourceCurrentQuery,
		[]int64{1, 2, 3},
		nil,
		nil,
		0,
	)
	if len(libraryQuery.exports) != 1 ||
		libraryQuery.exports[0].Name != "current" {
		t.Fatalf("current query exports = %#v", libraryQuery.exports)
	}

	single, err := resolver.ResolveShelf(ctx, 7)
	if err != nil {
		t.Fatal(err)
	}
	assertResolvedScope(
		t,
		single,
		operations.ExportSourceShelf,
		[]int64{1, 2},
		[]int64{7},
		[]string{"Bedtime"},
		0,
	)

	multiple, err := resolver.ResolveShelves(ctx, []int64{7, 8})
	if err != nil {
		t.Fatal(err)
	}
	assertResolvedScope(
		t,
		multiple,
		operations.ExportSourceShelves,
		[]int64{1, 2, 3},
		[]int64{7, 8},
		[]string{"Bedtime", "Adventures"},
		1,
	)
}

func TestResolverRejectsInvalidOrUnavailableScope(t *testing.T) {
	t.Parallel()

	libraryQuery := &fakeLibraryResolver{
		pages:   map[string]map[int][]int64{"moon": {1: {99}}},
		stories: map[int64]library.ExportStory{},
	}
	resolver, err := NewResolver(
		libraryQuery,
		fakeShelfResolver{7: openedShelf(7, "Bedtime", "moon")},
	)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.ResolveSelection(
		context.Background(),
		[]int64{0},
	); !errors.Is(err, ErrInvalidScope) {
		t.Fatalf("ResolveSelection(invalid) error = %v", err)
	}
	if _, err := resolver.ResolveShelves(
		context.Background(),
		[]int64{7, 7},
	); !errors.Is(err, ErrInvalidScope) {
		t.Fatalf("ResolveShelves(duplicate) error = %v", err)
	}
	if _, err := resolver.ResolveShelf(
		context.Background(),
		7,
	); !errors.Is(err, ErrStoryUnavailable) {
		t.Fatalf("ResolveShelf(unavailable story) error = %v", err)
	}
}

type fakeLibraryResolver struct {
	pages   map[string]map[int][]int64
	stories map[int64]library.ExportStory
	exports []library.StoryLibraryQuery
}

func (f *fakeLibraryResolver) ExportQuery(
	_ context.Context,
	query library.StoryLibraryQuery,
) ([]library.ExportStory, error) {
	f.exports = append(f.exports, query)
	pages := f.pages[query.Name]
	stories := make([]library.ExportStory, 0, len(f.stories))
	for page := 1; page <= len(pages); page++ {
		for _, id := range pages[page] {
			story, found := f.stories[id]
			if !found {
				return nil, errors.New("story missing")
			}
			stories = append(stories, story)
		}
	}
	return stories, nil
}

func (f *fakeLibraryResolver) ExportStory(
	_ context.Context,
	storyID int64,
) (library.ExportStory, error) {
	story, found := f.stories[storyID]
	if !found {
		return library.ExportStory{}, errors.New("story missing")
	}
	return story, nil
}

type fakeShelfResolver map[int64]shelves.OpenedShelf

func (f fakeShelfResolver) Open(
	_ context.Context,
	shelfID int64,
) (shelves.OpenedShelf, error) {
	opened, found := f[shelfID]
	if !found {
		return shelves.OpenedShelf{}, shelves.ErrShelfNotFound
	}
	return opened, nil
}

func openedShelf(id int64, name string, queryName string) shelves.OpenedShelf {
	return shelves.OpenedShelf{
		Shelf: shelves.Shelf{ID: id, Name: name},
		Query: shelves.SavedLibraryQuery{Name: queryName},
	}
}

func exportStories(ids ...int64) map[int64]library.ExportStory {
	stories := make(map[int64]library.ExportStory, len(ids))
	for _, id := range ids {
		stories[id] = library.ExportStory{
			ID:                  id,
			UUID:                "00112233-4455-4677-8899-aabbccddeeff",
			Title:               "Story",
			OriginalFilename:    "story.zip",
			DetectedFormat:      "zip",
			SHA256:              "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
			ByteSize:            42,
			ManagedRelativePath: "archives/aa/story.zip",
			Verification:        library.CompatibilityCompatible,
		}
	}
	return stories
}

func assertResolvedScope(
	t *testing.T,
	scope Scope,
	sourceType operations.ExportSourceType,
	storyIDs []int64,
	shelfIDs []int64,
	shelfNames []string,
	overlap int,
) {
	t.Helper()
	gotIDs := make([]int64, 0, len(scope.Stories))
	for _, story := range scope.Stories {
		gotIDs = append(gotIDs, story.ID)
	}
	if scope.Source.Type != sourceType ||
		!reflect.DeepEqual(gotIDs, storyIDs) ||
		!reflect.DeepEqual(scope.Source.ShelfIDs, shelfIDs) ||
		!reflect.DeepEqual(scope.Source.ShelfNames, shelfNames) ||
		scope.CollapsedOverlap != overlap {
		t.Fatalf("resolved scope = %#v, story IDs = %#v", scope, gotIDs)
	}
}
