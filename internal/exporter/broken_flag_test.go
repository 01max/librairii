package exporter

import (
	"context"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/storage"
	"path/filepath"
	"testing"
)

func TestBrokenFlagsPreserveExportScopesAndPreparedSelections(t *testing.T) {
	ctx := context.Background()
	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	opened, err := database.Open(ctx, filepath.Join(t.TempDir(), "library.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	db := opened.Writer()
	repo := catalog.NewRepository(db)
	ids := []int64{}
	for i, uuid := range []string{"00112233-4455-4677-8899-aabbccddeeff", "11112222-3333-4444-8555-666677778888"} {
		name := []string{"first.zip", "second.zip"}[i]
		archive := writeManagedExportStory(t, layout, int64(i+1), name, []byte(name))
		story, _, err := repo.Create(ctx, catalog.CreateStory{UUID: uuid, EmbeddedTitle: name, OriginalFilename: name, DetectedFormat: catalog.FormatZIP, SHA256: archive.SHA256, ByteSize: archive.ByteSize, ManagedPath: archive.ManagedRelativePath})
		if err != nil {
			t.Fatal(err)
		}
		ids = append(ids, story.ID)
	}
	flags := library.NewBrokenFlags(db)
	if _, err := flags.Set(ctx, ids[:1], true); err != nil {
		t.Fatal(err)
	}
	shelfService, err := collections.NewService(db)
	if err != nil {
		t.Fatal(err)
	}
	all, err := shelfService.Create(ctx, "All", library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`INSERT INTO legacy_broken_tag VALUES (1,9);
        INSERT INTO shelves (id,name,normalized_name,position,query_version,query_payload)
        VALUES (20,'Legacy broken','legacy broken',1,3,'{"includeBroken":true,"anyOf":[{"booleanFilters":[{"definitionId":9,"state":"true"}]}]}');`); err != nil {
		t.Fatal(err)
	}
	resolver, err := NewResolver(library.NewQuery(opened.SQL(), nil), shelfService)
	if err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		mode                   library.BrokenVisibility
		count, broken, overlap int
	}{
		{library.BrokenHide, 1, 0, 0}, {library.BrokenInclude, 2, 1, 1}, {library.BrokenOnly, 1, 1, 1},
	} {
		current, err := resolver.ResolveCurrentQuery(ctx, library.StoryLibraryQuery{BrokenVisibility: test.mode, PageSize: 1})
		if err != nil || len(current.Stories) != test.count {
			t.Fatalf("current %s: %#v, %v", test.mode, current, err)
		}
		single, err := resolver.ResolveShelf(ctx, all.ID, test.mode)
		if err != nil || len(single.Stories) != test.count {
			t.Fatalf("single %s: %#v, %v", test.mode, single, err)
		}
		migrated, err := resolver.ResolveShelf(ctx, 20, test.mode)
		if err != nil || len(migrated.Stories) != test.broken {
			t.Fatalf("migrated %s: %#v, %v", test.mode, migrated, err)
		}
		union, err := resolver.ResolveShelves(ctx, []int64{all.ID, 20}, test.mode)
		if err != nil || len(union.Stories) != test.count || union.CollapsedOverlap != test.overlap {
			t.Fatalf("union %s: %#v, %v", test.mode, union, err)
		}
	}
	preflight, err := NewPreflightService(resolver, layout)
	if err != nil {
		t.Fatal(err)
	}
	prepared, err := preflight.Plan(ctx, PreflightRequest{SourceType: operations.ExportSourceSelection, StoryIDs: ids[:1]}, t.TempDir())
	if err != nil || !prepared.CanExport || prepared.ReadyCount != 1 || len(prepared.Issues) != 0 {
		t.Fatalf("flagged selection preflight=%#v, %v", prepared, err)
	}
	if _, err := flags.Set(ctx, ids, false); err != nil {
		t.Fatal(err)
	}
	if len(prepared.Scope.Stories) != 1 || prepared.Scope.Stories[0].ID != ids[0] || len(prepared.OperationItems()) != 1 {
		t.Fatal("flag change altered prepared snapshot")
	}
}
