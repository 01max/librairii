package app

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/tagging"
)

func TestSavedShelvesVerticalSliceThroughApplication(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	provider := newRuntimeStorageProvider(t)
	events := &runtimeEventRecorder{events: make(chan operations.Snapshot, 32)}
	importRuntime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Date(2026, time.July, 25, 10, 0, 0, 0, time.UTC)},
		events,
		2,
	)
	if err != nil {
		t.Fatal(err)
	}
	fixtureDirectory := t.TempDir()
	sources := make([]string, 0, 3)
	for _, fixture := range []struct {
		uuid     string
		filename string
	}{
		{
			uuid:     testfixture.StoryUUID,
			filename: "clockwork-one.zip",
		},
		{
			uuid:     "11112222-3333-4444-8555-666677778888",
			filename: "clockwork-two.zip",
		},
		{
			uuid:     "99992222-3333-4444-8555-666677778888",
			filename: "clockwork-three.zip",
		},
	} {
		source, err := testfixture.WriteZIP(
			fixtureDirectory,
			storyFixtureWithUUID(
				testfixture.GenericZIP(),
				fixture.uuid,
				fixture.filename,
			),
		)
		if err != nil {
			t.Fatal(err)
		}
		sources = append(sources, source)
	}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    &recordingDialogs{paths: sources},
		Events:     events,
		Readiness:  fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations: importRuntime,
		Library:    importRuntime,
		Removal:    importRuntime,
		Tags:       importRuntime,
		Shelves:    importRuntime,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := application.Start(ctx); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		application.Stop(ctx)
	})
	started := application.SelectAndStartImport(ctx)
	if started.Error != nil || started.Operation == nil {
		t.Fatalf("SelectAndStartImport() = %#v", started)
	}
	if terminal := events.waitTerminal(t, started.Operation.ID); terminal.Status !=
		operations.StatusSucceeded {
		t.Fatalf("terminal import = %#v", terminal)
	}
	page := application.QueryStories(ctx, library.StoryLibraryQuery{
		Page:     1,
		PageSize: 12,
		Sort:     library.SortImportedNewest,
	})
	if page.Error != nil || page.Page == nil || page.Page.TotalItems != 3 {
		t.Fatalf("QueryStories() = %#v", page)
	}
	storyIDs := []int64{
		page.Page.Stories[0].ID,
		page.Page.Stories[1].ID,
		page.Page.Stories[2].ID,
	}
	searchName := page.Page.Stories[0].Title
	searchPage := application.QueryStories(ctx, library.StoryLibraryQuery{
		Name:     searchName,
		Page:     1,
		PageSize: 12,
		Sort:     library.SortNameAscending,
	})
	if searchPage.Error != nil ||
		searchPage.Page == nil ||
		searchPage.Page.TotalItems == 0 {
		t.Fatalf("QueryStories(name search) = %#v", searchPage)
	}

	moodResponse := application.CreateTagDefinition(ctx, tagging.CreateDefinition{
		Key: "mood", Label: "Mood", Color: "#405CF5", Kind: tagging.KindChoice,
	})
	if moodResponse.Error != nil || moodResponse.Definition == nil {
		t.Fatalf("CreateTagDefinition() = %#v", moodResponse)
	}
	calm := createVerticalShelfValue(t, application, moodResponse.Definition.ID, "calm")
	adventure := createVerticalShelfValue(
		t,
		application,
		moodResponse.Definition.ID,
		"adventure",
	)
	if response := application.SetChoiceTagValue(
		ctx,
		storyIDs[:2],
		moodResponse.Definition.ID,
		calm.ID,
		true,
	); response.Error != nil {
		t.Fatalf("SetChoiceTagValue(calm) = %#v", response)
	}
	if response := application.SetChoiceTagValue(
		ctx,
		storyIDs[1:],
		moodResponse.Definition.ID,
		adventure.ID,
		true,
	); response.Error != nil {
		t.Fatalf("SetChoiceTagValue(adventure) = %#v", response)
	}

	calmShelf := createVerticalShelf(t, application, "Calm", library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: moodResponse.Definition.ID,
			ValueIDs:     []int64{calm.ID},
		}},
	})
	adventureShelf := createVerticalShelf(
		t,
		application,
		"Adventure",
		library.StoryLibraryQuery{ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: moodResponse.Definition.ID,
			ValueIDs:     []int64{adventure.ID},
		}}},
	)
	searchShelf := createVerticalShelf(
		t,
		application,
		"Named search",
		library.StoryLibraryQuery{Name: searchName},
	)
	emptyShelf := createVerticalShelf(
		t,
		application,
		"Empty",
		library.StoryLibraryQuery{Name: "never-matches"},
	)

	listed := application.ListShelves(ctx)
	if listed.Error != nil ||
		verticalShelfSummary(t, listed, calmShelf.ID).Count != 2 ||
		verticalShelfSummary(t, listed, adventureShelf.ID).Count != 2 ||
		verticalShelfSummary(t, listed, searchShelf.ID).Count !=
			searchPage.Page.TotalItems ||
		verticalShelfSummary(t, listed, emptyShelf.ID).Count != 0 {
		t.Fatalf("ListShelves(initial) = %#v", listed)
	}
	savedSearch, err := shelves.DecodeSavedLibraryQuery(
		searchShelf.QueryVersion,
		searchShelf.QueryPayload,
	)
	if err != nil {
		t.Fatal(err)
	}
	legacyPayload, err := json.Marshal(library.StoryLibraryQuery{
		Name:     searchName,
		Page:     8,
		PageSize: 1,
		Sort:     library.SortNameAscending,
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := provider.SQL().ExecContext(
		ctx,
		`UPDATE shelves
		 SET query_version = 1,
		     query_payload = ?
		 WHERE id = ?`,
		string(legacyPayload),
		searchShelf.ID,
	); err != nil {
		t.Fatal(err)
	}
	openedSearch := application.OpenShelf(
		ctx,
		searchShelf.ID,
		library.ListRequest{
			Page: 1, PageSize: 12, Sort: library.SortImportedNewest,
		},
	)
	if openedSearch.Error != nil ||
		openedSearch.Evaluation == nil ||
		openedSearch.Evaluation.Query.Name != savedSearch.Name ||
		openedSearch.Evaluation.Page.TotalItems != searchPage.Page.TotalItems {
		t.Fatalf("OpenShelf(migrated search) = %#v", openedSearch)
	}

	preview := application.PreviewShelves(
		ctx,
		[]int64{calmShelf.ID, adventureShelf.ID},
	)
	if preview.Error != nil ||
		preview.Preview == nil ||
		preview.Preview.UniqueStoryCount != 3 ||
		preview.Preview.OverlapCount != 1 ||
		len(preview.Preview.SourceShelfNames) != 2 ||
		preview.Preview.SourceShelfNames[0] != "Calm" ||
		preview.Preview.SourceShelfNames[1] != "Adventure" {
		t.Fatalf("PreviewShelves() = %#v", preview)
	}

	if response := application.SetChoiceTagValue(
		ctx,
		storyIDs[:1],
		moodResponse.Definition.ID,
		calm.ID,
		false,
	); response.Error != nil {
		t.Fatalf("SetChoiceTagValue(remove calm) = %#v", response)
	}
	listed = application.ListShelves(ctx)
	if listed.Error != nil ||
		verticalShelfSummary(t, listed, calmShelf.ID).Count != 1 {
		t.Fatalf("ListShelves(after dynamic tag change) = %#v", listed)
	}

	plan := application.PlanTagValueDeletion(ctx, calm.ID)
	if plan.Error != nil ||
		plan.Plan == nil ||
		plan.Plan.AffectedShelfCount != 1 ||
		len(plan.Plan.AffectedShelfIDs) != 1 ||
		plan.Plan.AffectedShelfIDs[0] != calmShelf.ID {
		t.Fatalf("PlanTagValueDeletion() = %#v", plan)
	}
	deletedValue := application.DeleteTagValue(ctx, *plan.Plan)
	if deletedValue.Error != nil || !deletedValue.Success {
		t.Fatalf("DeleteTagValue() = %#v", deletedValue)
	}
	listed = application.ListShelves(ctx)
	if listed.Error != nil ||
		verticalShelfSummary(t, listed, calmShelf.ID).Validity !=
			shelves.ValidityNeedsAttention ||
		verticalShelfSummary(t, listed, adventureShelf.ID).Validity !=
			shelves.ValidityValid {
		t.Fatalf("ListShelves(after criterion deletion) = %#v", listed)
	}
	blocked := application.OpenShelf(ctx, calmShelf.ID, library.ListRequest{})
	if blocked.Error == nil || blocked.Error.Code != ErrorConflict {
		t.Fatalf("OpenShelf(needs attention) = %#v", blocked)
	}
	blockedPreview := application.PreviewShelves(
		ctx,
		[]int64{calmShelf.ID, adventureShelf.ID},
	)
	if blockedPreview.Error == nil || blockedPreview.Error.Code != ErrorConflict {
		t.Fatalf("PreviewShelves(needs attention) = %#v", blockedPreview)
	}

	repaired := application.ReplaceShelfQuery(
		ctx,
		calmShelf.ID,
		library.StoryLibraryQuery{Name: "never-matches"},
	)
	if repaired.Error != nil ||
		repaired.Shelf == nil ||
		repaired.Shelf.Validity != shelves.ValidityValid {
		t.Fatalf("ReplaceShelfQuery(repair) = %#v", repaired)
	}
	openedEmpty := application.OpenShelf(ctx, calmShelf.ID, library.ListRequest{})
	if openedEmpty.Error != nil ||
		openedEmpty.Evaluation == nil ||
		openedEmpty.Evaluation.Page.TotalItems != 0 {
		t.Fatalf("OpenShelf(valid empty repair) = %#v", openedEmpty)
	}

	renamed := application.RenameShelf(ctx, adventureShelf.ID, "Quest")
	if renamed.Error != nil || renamed.Shelf == nil || renamed.Shelf.Name != "Quest" {
		t.Fatalf("RenameShelf() = %#v", renamed)
	}
	duplicated := application.DuplicateShelf(ctx, searchShelf.ID, "Named search copy")
	if duplicated.Error != nil || duplicated.Shelf == nil {
		t.Fatalf("DuplicateShelf() = %#v", duplicated)
	}
	listed = application.ListShelves(ctx)
	orderedIDs := make([]int64, 0, len(listed.Shelves))
	for index := len(listed.Shelves) - 1; index >= 0; index-- {
		orderedIDs = append(orderedIDs, listed.Shelves[index].ID)
	}
	reordered := application.ReorderShelves(ctx, orderedIDs)
	if reordered.Error != nil ||
		len(reordered.Shelves) != len(orderedIDs) ||
		reordered.Shelves[0].ID != orderedIDs[0] {
		t.Fatalf("ReorderShelves() = %#v", reordered)
	}

	if _, err := provider.SQL().ExecContext(
		ctx,
		"UPDATE shelves SET query_version = 99 WHERE id = ?",
		emptyShelf.ID,
	); err != nil {
		t.Fatal(err)
	}
	listed = application.ListShelves(ctx)
	if listed.Error != nil ||
		verticalShelfSummary(t, listed, emptyShelf.ID).AttentionReason !=
			shelves.AttentionUnmigratableQuery {
		t.Fatalf("ListShelves(unmigratable) = %#v", listed)
	}
	if deleted := application.DeleteShelf(ctx, emptyShelf.ID); deleted.Error != nil ||
		!deleted.Success {
		t.Fatalf("DeleteShelf(invalid) = %#v", deleted)
	}
	if deleted := application.DeleteShelf(
		ctx,
		duplicated.Shelf.ID,
	); deleted.Error != nil || !deleted.Success {
		t.Fatalf("DeleteShelf(duplicate) = %#v", deleted)
	}
}

func createVerticalShelfValue(
	t *testing.T,
	application *Application,
	definitionID int64,
	key string,
) tagging.Value {
	t.Helper()

	response := application.CreateTagValue(context.Background(), tagging.CreateValue{
		DefinitionID: definitionID,
		Key:          key,
		Label:        key,
	})
	if response.Error != nil || response.Value == nil {
		t.Fatalf("CreateTagValue(%s) = %#v", key, response)
	}
	return *response.Value
}

func createVerticalShelf(
	t *testing.T,
	application *Application,
	name string,
	query library.StoryLibraryQuery,
) shelves.Shelf {
	t.Helper()

	response := application.CreateShelf(context.Background(), name, query)
	if response.Error != nil || response.Shelf == nil {
		t.Fatalf("CreateShelf(%s) = %#v", name, response)
	}
	return *response.Shelf
}

func verticalShelfSummary(
	t *testing.T,
	response ShelfListResponse,
	shelfID int64,
) shelves.Summary {
	t.Helper()

	for _, summary := range response.Shelves {
		if summary.ID == shelfID {
			return summary
		}
	}
	t.Fatalf("shelf %d missing from %#v", shelfID, response)
	return shelves.Summary{}
}
