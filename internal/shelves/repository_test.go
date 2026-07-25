package shelves

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/database"
)

func TestRepositoryPersistsCanonicalShelfFieldsInPositionOrder(t *testing.T) {
	t.Parallel()

	repository, connection := openShelfRepository(t)
	second, err := repository.Insert(context.Background(), NewShelf{
		Name:           "Bedtime",
		NormalizedName: "bedtime",
		Position:       1,
		QueryVersion:   1,
		QueryPayload:   `{"name":"moon","booleanFilters":[]}`,
	})
	if err != nil {
		t.Fatal(err)
	}
	first, err := repository.Insert(context.Background(), NewShelf{
		Name:           "All stories",
		NormalizedName: "all stories",
		Position:       0,
		QueryVersion:   1,
		QueryPayload:   `{}`,
		Validity:       ValidityNeedsAttention,
	})
	if err != nil {
		t.Fatal(err)
	}

	saved, err := repository.List(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(saved) != 2 ||
		saved[0] != first ||
		saved[1] != second ||
		first.Validity != ValidityNeedsAttention ||
		second.Validity != ValidityValid {
		t.Fatalf("List() = %#v", saved)
	}

	assertShelfIndexes(
		t,
		connection,
		"idx_shelves_normalized_name",
		"idx_shelves_position",
		"idx_shelves_validity_position",
	)
}

func TestRepositoryRejectsInvalidShelfIdentityQueryStateAndOrdering(t *testing.T) {
	t.Parallel()

	repository, _ := openShelfRepository(t)
	valid := NewShelf{
		Name:           "Bedtime",
		NormalizedName: "bedtime",
		Position:       0,
		QueryVersion:   1,
		QueryPayload:   `{}`,
	}
	if _, err := repository.Insert(context.Background(), valid); err != nil {
		t.Fatal(err)
	}

	tests := []struct {
		name  string
		input NewShelf
	}{
		{
			name: "duplicate normalized name",
			input: NewShelf{
				Name:           "BEDTIME",
				NormalizedName: "BEDTIME",
				Position:       1,
				QueryVersion:   1,
				QueryPayload:   `{}`,
			},
		},
		{
			name: "duplicate position",
			input: NewShelf{
				Name:           "Favorites",
				NormalizedName: "favorites",
				Position:       valid.Position,
				QueryVersion:   1,
				QueryPayload:   `{}`,
			},
		},
		{
			name: "untrimmed name",
			input: NewShelf{
				Name:           " Favorites ",
				NormalizedName: "favorites",
				Position:       2,
				QueryVersion:   1,
				QueryPayload:   `{}`,
			},
		},
		{
			name: "empty normalized name",
			input: NewShelf{
				Name:           "Favorites",
				NormalizedName: "",
				Position:       2,
				QueryVersion:   1,
				QueryPayload:   `{}`,
			},
		},
		{
			name: "negative position",
			input: NewShelf{
				Name:           "Favorites",
				NormalizedName: "favorites",
				Position:       -1,
				QueryVersion:   1,
				QueryPayload:   `{}`,
			},
		},
		{
			name: "invalid query version",
			input: NewShelf{
				Name:           "Favorites",
				NormalizedName: "favorites",
				Position:       2,
				QueryVersion:   0,
				QueryPayload:   `{}`,
			},
		},
		{
			name: "malformed query payload",
			input: NewShelf{
				Name:           "Favorites",
				NormalizedName: "favorites",
				Position:       2,
				QueryVersion:   1,
				QueryPayload:   `{"name":`,
			},
		},
		{
			name: "non-object query payload",
			input: NewShelf{
				Name:           "Favorites",
				NormalizedName: "favorites",
				Position:       2,
				QueryVersion:   1,
				QueryPayload:   `[]`,
			},
		},
		{
			name: "unsupported validity state",
			input: NewShelf{
				Name:           "Favorites",
				NormalizedName: "favorites",
				Position:       2,
				QueryVersion:   1,
				QueryPayload:   `{}`,
				Validity:       "invalid",
			},
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			if _, err := repository.Insert(
				context.Background(),
				test.input,
			); err == nil {
				t.Fatalf("Insert(%s) error = nil", test.name)
			}
		})
	}

	saved, err := repository.List(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(saved) != 1 || saved[0].NormalizedName != valid.NormalizedName {
		t.Fatalf("List(after rejected inserts) = %#v", saved)
	}
}

func TestNewRepositoryRequiresDatabase(t *testing.T) {
	t.Parallel()

	if _, err := NewRepository(nil); err != ErrMissingDatabase {
		t.Fatalf("NewRepository(nil) error = %v", err)
	}
}

func openShelfRepository(t *testing.T) (*Repository, *sql.DB) {
	t.Helper()

	opened, err := database.Open(
		context.Background(),
		filepath.Join(t.TempDir(), "librairii.sqlite3"),
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := opened.Close(); err != nil {
			t.Error(err)
		}
	})
	repository, err := NewRepository(opened.SQL())
	if err != nil {
		t.Fatal(err)
	}
	return repository, opened.SQL()
}

func assertShelfIndexes(t *testing.T, connection *sql.DB, expected ...string) {
	t.Helper()

	rows, err := connection.Query("PRAGMA index_list(shelves)")
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	found := make(map[string]bool)
	for rows.Next() {
		var sequence int
		var name string
		var unique int
		var origin string
		var partial int
		if err := rows.Scan(&sequence, &name, &unique, &origin, &partial); err != nil {
			t.Fatal(err)
		}
		found[name] = true
	}
	if err := rows.Err(); err != nil {
		t.Fatal(err)
	}
	for _, name := range expected {
		if !found[name] {
			t.Errorf("missing shelf index %q; found %#v", name, found)
		}
	}
}
