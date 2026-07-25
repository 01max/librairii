package shelves

import (
	"context"
	"database/sql"
	"errors"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/library"
)

func TestServiceRunsShelfCRUDWithoutChangingStories(t *testing.T) {
	t.Parallel()

	repository, connection := openShelfRepository(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := connection.Exec(
		`INSERT INTO stories (uuid)
		 VALUES ('123e4567-e89b-42d3-a456-426614174000')`,
	); err != nil {
		t.Fatal(err)
	}

	allStories, err := service.Create(
		context.Background(),
		"  All\tStories ",
		library.StoryLibraryQuery{
			Page:     9,
			PageSize: 1,
			Sort:     library.SortImportedNewest,
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if allStories.Name != "All Stories" ||
		allStories.NormalizedName != "all stories" ||
		allStories.Position != 0 ||
		allStories.QueryVersion != CurrentSavedLibraryQueryVersion ||
		allStories.QueryPayload != `{}` ||
		allStories.Validity != ValidityValid {
		t.Fatalf("Create(empty) = %#v", allStories)
	}
	bedtime, err := service.Create(
		context.Background(),
		"Bedtime",
		library.StoryLibraryQuery{Name: "moon"},
	)
	if err != nil {
		t.Fatal(err)
	}
	duplicate, err := service.Duplicate(
		context.Background(),
		bedtime.ID,
		"Bedtime copy",
	)
	if err != nil {
		t.Fatal(err)
	}
	if duplicate.ID == bedtime.ID ||
		duplicate.Position != 2 ||
		duplicate.QueryPayload != bedtime.QueryPayload {
		t.Fatalf("Duplicate() = %#v", duplicate)
	}

	renamed, err := service.Rename(
		context.Background(),
		bedtime.ID,
		"  Histoires d'ÉTÉ  ",
	)
	if err != nil {
		t.Fatal(err)
	}
	if renamed.Name != "Histoires d'ÉTÉ" ||
		renamed.NormalizedName != "histoires d'ete" {
		t.Fatalf("Rename() = %#v", renamed)
	}
	replaced, err := service.ReplaceQuery(
		context.Background(),
		bedtime.ID,
		library.StoryLibraryQuery{
			BooleanFilters: []library.BooleanFilter{{
				DefinitionID: 4,
				State:        library.BooleanTrue,
			}},
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if replaced.QueryPayload == duplicate.QueryPayload ||
		duplicate.QueryPayload != `{"name":"moon"}` {
		t.Fatalf("ReplaceQuery() changed duplicate: %#v / %#v", replaced, duplicate)
	}
	opened, err := service.Open(context.Background(), bedtime.ID)
	if err != nil {
		t.Fatal(err)
	}
	if opened.Shelf != replaced ||
		len(opened.Query.BooleanFilters) != 1 ||
		opened.Query.BooleanFilters[0].DefinitionID != 4 {
		t.Fatalf("Open() = %#v", opened)
	}

	reordered, err := service.Reorder(
		context.Background(),
		[]int64{duplicate.ID, bedtime.ID, allStories.ID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(reordered) != 3 ||
		reordered[0].ID != duplicate.ID ||
		reordered[0].Position != 0 ||
		reordered[1].ID != bedtime.ID ||
		reordered[2].ID != allStories.ID ||
		reordered[2].Position != 2 {
		t.Fatalf("Reorder() = %#v", reordered)
	}

	if err := service.Delete(context.Background(), bedtime.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := repository.Shelf(
		context.Background(),
		bedtime.ID,
	); err == nil {
		t.Fatal("deleted shelf still exists")
	}
	var stories int
	if err := connection.QueryRow("SELECT COUNT(*) FROM stories").Scan(&stories); err != nil {
		t.Fatal(err)
	}
	if stories != 1 {
		t.Fatalf("story count after shelf delete = %d", stories)
	}
}

func TestServiceMigratesSupportedShelfQueryWhenOpened(t *testing.T) {
	t.Parallel()

	repository, connection := openShelfRepository(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	legacy, err := repository.Insert(context.Background(), NewShelf{
		Name:           "Legacy",
		NormalizedName: "legacy",
		Position:       0,
		QueryVersion:   1,
		QueryPayload: `{
			"name":" ÉTÉ ",
			"booleanFilters":[{"definitionId":5,"state":"true"}],
			"page":7,
			"pageSize":12,
			"sort":"imported_desc"
		}`,
	})
	if err != nil {
		t.Fatal(err)
	}

	opened, err := service.Open(context.Background(), legacy.ID)
	if err != nil {
		t.Fatal(err)
	}
	const expected = `{"name":"ete","booleanFilters":[{"definitionId":5,"state":"true"}]}`
	if opened.Shelf.QueryVersion != CurrentSavedLibraryQueryVersion ||
		opened.Shelf.QueryPayload != expected ||
		opened.Query.Name != "ete" {
		t.Fatalf("Open(legacy) = %#v", opened)
	}
	persisted, err := repository.Shelf(context.Background(), legacy.ID)
	if err != nil {
		t.Fatal(err)
	}
	if persisted.QueryVersion != CurrentSavedLibraryQueryVersion ||
		persisted.QueryPayload != expected {
		t.Fatalf("persisted migration = %#v", persisted)
	}
}

func TestServiceRejectsDuplicateAndInvalidShelfNamesWithoutChangingRecords(
	t *testing.T,
) {
	t.Parallel()

	service, err := NewService(openShelfDatabase(t))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.Create(
		context.Background(),
		"Été",
		library.StoryLibraryQuery{},
	); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Create(
		context.Background(),
		" ETE ",
		library.StoryLibraryQuery{},
	); !errors.Is(err, ErrDuplicateShelfName) {
		t.Fatalf("Create(duplicate normalized name) error = %v", err)
	}
	if _, err := service.Create(
		context.Background(),
		"Other",
		library.StoryLibraryQuery{},
	); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Rename(
		context.Background(),
		2,
		"été",
	); !errors.Is(err, ErrDuplicateShelfName) {
		t.Fatalf("Rename(duplicate normalized name) error = %v", err)
	}
	for _, name := range []string{
		"",
		"   ",
		"bad\x00name",
		strings.Repeat("x", maxShelfNameRunes+1),
	} {
		if _, err := service.Create(
			context.Background(),
			name,
			library.StoryLibraryQuery{},
		); !errors.Is(err, ErrInvalidShelfName) {
			t.Fatalf("Create(%q) error = %v", name, err)
		}
	}
	shelves, err := service.List(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(shelves) != 2 ||
		shelves[0].Name != "Été" ||
		shelves[1].Name != "Other" {
		t.Fatalf("List(after rejected names) = %#v", shelves)
	}
}

func TestServiceRejectsStaleOrdersMissingShelvesAndAttentionSources(t *testing.T) {
	t.Parallel()

	repository, connection := openShelfRepository(t)
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	first, err := service.Create(
		context.Background(),
		"First",
		library.StoryLibraryQuery{},
	)
	if err != nil {
		t.Fatal(err)
	}
	second, err := service.Create(
		context.Background(),
		"Second",
		library.StoryLibraryQuery{},
	)
	if err != nil {
		t.Fatal(err)
	}
	for _, order := range [][]int64{
		{first.ID},
		{first.ID, first.ID},
		{first.ID, 999},
	} {
		if _, err := service.Reorder(
			context.Background(),
			order,
		); !errors.Is(err, ErrInvalidShelfOrder) {
			t.Fatalf("Reorder(%v) error = %v", order, err)
		}
	}
	if _, err := connection.Exec(
		"UPDATE shelves SET validity_state = 'needs_attention' WHERE id = ?",
		second.ID,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Open(
		context.Background(),
		second.ID,
	); !errors.Is(err, ErrShelfNeedsAttention) {
		t.Fatalf("Open(needs attention) error = %v", err)
	}
	if _, err := service.Duplicate(
		context.Background(),
		second.ID,
		"Blocked copy",
	); !errors.Is(err, ErrShelfNeedsAttention) {
		t.Fatalf("Duplicate(needs attention) error = %v", err)
	}
	for _, operation := range []func() error{
		func() error {
			_, err := service.Open(context.Background(), 999)
			return err
		},
		func() error {
			_, err := service.Rename(context.Background(), 999, "Missing")
			return err
		},
		func() error {
			_, err := service.Duplicate(context.Background(), 999, "Missing copy")
			return err
		},
		func() error {
			_, err := service.ReplaceQuery(
				context.Background(),
				999,
				library.StoryLibraryQuery{},
			)
			return err
		},
		func() error {
			return service.Delete(context.Background(), 999)
		},
	} {
		if err := operation(); !errors.Is(err, ErrShelfNotFound) {
			t.Fatalf("missing shelf operation error = %v", err)
		}
	}

	persisted, err := repository.List(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(persisted) != 2 ||
		persisted[0].ID != first.ID ||
		persisted[1].ID != second.ID {
		t.Fatalf("records changed after rejected operations: %#v", persisted)
	}
}

func openShelfDatabase(t *testing.T) *sql.DB {
	t.Helper()

	_, connection := openShelfRepository(t)
	return connection
}
