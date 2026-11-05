package exporter

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/storage"
	"path/filepath"
	"testing"
)

func TestBrokenStoriesAreSkippedInEveryExportMode(t *testing.T) {
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
        INSERT INTO collections (id,name,normalized_name,position,query_version,query_payload)
        VALUES (20,'Legacy broken','legacy broken',1,3,'{"includeBroken":true,"anyOf":[{"booleanFilters":[{"definitionId":9,"state":"true"}]}]}');`); err != nil {
		t.Fatal(err)
	}
	query := library.NewQuery(opened.SQL(), nil)
	manual, err := shelves.NewService(db, query)
	if err != nil {
		t.Fatal(err)
	}
	shelf, err := manual.CreateManual(ctx, "Approved", []string{"00112233-4455-4677-8899-aabbccddeeff", "11112222-3333-4444-8555-666677778888"})
	if err != nil {
		t.Fatal(err)
	}
	copy, err := manual.Duplicate(ctx, shelf.ID, "Copy")
	if err != nil {
		t.Fatal(err)
	}
	resolver, err := NewResolver(query, shelfService, manual)
	if err != nil {
		t.Fatal(err)
	}
	preflight, err := NewPreflightService(resolver, layout)
	if err != nil {
		t.Fatal(err)
	}
	requests := []PreflightRequest{
		{SourceType: operations.ExportSourceSelection, StoryIDs: ids},
		{SourceType: operations.ExportSourceCurrentQuery, Query: library.StoryLibraryQuery{PageSize: 1}},
		{SourceType: operations.ExportSourceCollection, CollectionIDs: []int64{all.ID}},
		{SourceType: operations.ExportSourceCollections, CollectionIDs: []int64{all.ID, 20}},
		{SourceType: operations.ExportSourceCuratedShelf, ShelfIDs: []int64{shelf.ID}},
		{SourceType: operations.ExportSourceCuratedShelves, ShelfIDs: []int64{shelf.ID, copy.ID}},
		{SourceType: operations.ExportSourceShelf, ShelfIDs: []int64{all.ID}, IncludeBroken: true},
	}
	var prepared PreflightReport
	for _, request := range requests {
		report, err := preflight.Plan(ctx, request, t.TempDir())
		if err != nil || !report.CanExport || report.ResolvedCount != 2 || report.ReadyCount != 1 {
			t.Fatalf("%s: %#v %v", request.SourceType, report, err)
		}
		skipped := 0
		for _, item := range report.Items {
			if item.Disposition == DispositionSkipped && item.Issue != nil && item.Issue.Code == IssueStoryMarkedBroken {
				skipped++
			}
		}
		if skipped != 1 {
			t.Fatalf("%s: flagged story was not skipped: %#v", request.SourceType, report.Items)
		}
		if request.SourceType == operations.ExportSourceSelection {
			prepared = report
		}
	}
	if _, err := flags.Set(ctx, ids, false); err != nil {
		t.Fatal(err)
	}
	if err := preflight.ValidatePrepared(ctx, prepared); !errors.Is(err, ErrPreparedExportStale) {
		t.Fatalf("changed flags did not invalidate preparation: %v", err)
	}
	refreshed, err := preflight.Plan(ctx, requests[0], t.TempDir())
	if err != nil || refreshed.ReadyCount != 2 {
		t.Fatalf("unflagged eligibility: %#v %v", refreshed, err)
	}
	if _, err := flags.Set(ctx, ids, true); err != nil {
		t.Fatal(err)
	}
	blocked, err := preflight.Plan(ctx, requests[0], t.TempDir())
	if err != nil || blocked.CanExport || blocked.ReadyCount != 0 || !blocked.Blocked {
		t.Fatalf("all-broken export enabled: %#v %v", blocked, err)
	}
}
