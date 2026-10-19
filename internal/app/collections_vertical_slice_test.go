package app

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/tagging"
)

func TestSavedCollectionsVerticalSliceThroughApplication(t *testing.T) {
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
		Clock:       fixedClock{now: time.Now()},
		Dialogs:     &recordingDialogs{paths: sources},
		Events:      events,
		Readiness:   fakeReadiness{report: ReadinessReport{MutationsAllowed: true}},
		Operations:  importRuntime,
		Library:     importRuntime,
		Removal:     importRuntime,
		Tags:        importRuntime,
		Collections: importRuntime,
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
	calm := createVerticalCollectionValue(t, application, moodResponse.Definition.ID, "calm")
	adventure := createVerticalCollectionValue(
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

	calmCollection := createVerticalCollection(t, application, "Calm", library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: moodResponse.Definition.ID,
			ValueIDs:     []int64{calm.ID},
		}},
	})
	adventureCollection := createVerticalCollection(
		t,
		application,
		"Adventure",
		library.StoryLibraryQuery{ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: moodResponse.Definition.ID,
			ValueIDs:     []int64{adventure.ID},
		}}},
	)
	searchCollection := createVerticalCollection(
		t,
		application,
		"Named search",
		library.StoryLibraryQuery{Name: searchName},
	)
	emptyCollection := createVerticalCollection(
		t,
		application,
		"Empty",
		library.StoryLibraryQuery{Name: "never-matches"},
	)

	listed := application.ListCollections(ctx)
	if listed.Error != nil ||
		verticalCollectionSummary(t, listed, calmCollection.ID).Count != 2 ||
		verticalCollectionSummary(t, listed, adventureCollection.ID).Count != 2 ||
		verticalCollectionSummary(t, listed, searchCollection.ID).Count !=
			searchPage.Page.TotalItems ||
		verticalCollectionSummary(t, listed, emptyCollection.ID).Count != 0 {
		t.Fatalf("ListCollections(initial) = %#v", listed)
	}
	savedSearch, err := collections.DecodeSavedLibraryQuery(
		searchCollection.QueryVersion,
		searchCollection.QueryPayload,
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
		searchCollection.ID,
	); err != nil {
		t.Fatal(err)
	}
	openedSearch := application.OpenCollection(
		ctx,
		searchCollection.ID,
		library.ListRequest{
			Page: 1, PageSize: 12, Sort: library.SortImportedNewest,
		},
	)
	if openedSearch.Error != nil ||
		openedSearch.Evaluation == nil ||
		openedSearch.Evaluation.Query.Name != savedSearch.Name ||
		openedSearch.Evaluation.Page.TotalItems != searchPage.Page.TotalItems {
		t.Fatalf("OpenCollection(migrated search) = %#v", openedSearch)
	}

	preview := application.PreviewCollections(
		ctx,
		[]int64{calmCollection.ID, adventureCollection.ID},
	)
	if preview.Error != nil ||
		preview.Preview == nil ||
		preview.Preview.UniqueStoryCount != 3 ||
		preview.Preview.OverlapCount != 1 ||
		len(preview.Preview.SourceCollectionNames) != 2 ||
		preview.Preview.SourceCollectionNames[0] != "Calm" ||
		preview.Preview.SourceCollectionNames[1] != "Adventure" {
		t.Fatalf("PreviewCollections() = %#v", preview)
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
	listed = application.ListCollections(ctx)
	if listed.Error != nil ||
		verticalCollectionSummary(t, listed, calmCollection.ID).Count != 1 {
		t.Fatalf("ListCollections(after dynamic tag change) = %#v", listed)
	}

	plan := application.PlanTagValueDeletion(ctx, calm.ID)
	if plan.Error != nil ||
		plan.Plan == nil ||
		plan.Plan.AffectedCollectionCount != 1 ||
		len(plan.Plan.AffectedCollectionIDs) != 1 ||
		plan.Plan.AffectedCollectionIDs[0] != calmCollection.ID {
		t.Fatalf("PlanTagValueDeletion() = %#v", plan)
	}
	deletedValue := application.DeleteTagValue(ctx, *plan.Plan)
	if deletedValue.Error != nil || !deletedValue.Success {
		t.Fatalf("DeleteTagValue() = %#v", deletedValue)
	}
	listed = application.ListCollections(ctx)
	if listed.Error != nil ||
		verticalCollectionSummary(t, listed, calmCollection.ID).Validity !=
			collections.ValidityNeedsAttention ||
		verticalCollectionSummary(t, listed, adventureCollection.ID).Validity !=
			collections.ValidityValid {
		t.Fatalf("ListCollections(after criterion deletion) = %#v", listed)
	}
	blocked := application.OpenCollection(ctx, calmCollection.ID, library.ListRequest{})
	if blocked.Error == nil || blocked.Error.Code != ErrorConflict {
		t.Fatalf("OpenCollection(needs attention) = %#v", blocked)
	}
	blockedPreview := application.PreviewCollections(
		ctx,
		[]int64{calmCollection.ID, adventureCollection.ID},
	)
	if blockedPreview.Error == nil || blockedPreview.Error.Code != ErrorConflict {
		t.Fatalf("PreviewCollections(needs attention) = %#v", blockedPreview)
	}

	repaired := application.ReplaceCollectionQuery(
		ctx,
		calmCollection.ID,
		library.StoryLibraryQuery{Name: "never-matches"},
	)
	if repaired.Error != nil ||
		repaired.Collection == nil ||
		repaired.Collection.Validity != collections.ValidityValid {
		t.Fatalf("ReplaceCollectionQuery(repair) = %#v", repaired)
	}
	openedEmpty := application.OpenCollection(ctx, calmCollection.ID, library.ListRequest{})
	if openedEmpty.Error != nil ||
		openedEmpty.Evaluation == nil ||
		openedEmpty.Evaluation.Page.TotalItems != 0 {
		t.Fatalf("OpenCollection(valid empty repair) = %#v", openedEmpty)
	}

	renamed := application.RenameCollection(ctx, adventureCollection.ID, "Quest")
	if renamed.Error != nil || renamed.Collection == nil || renamed.Collection.Name != "Quest" {
		t.Fatalf("RenameCollection() = %#v", renamed)
	}
	duplicated := application.DuplicateCollection(ctx, searchCollection.ID, "Named search copy")
	if duplicated.Error != nil || duplicated.Collection == nil {
		t.Fatalf("DuplicateCollection() = %#v", duplicated)
	}
	listed = application.ListCollections(ctx)
	orderedIDs := make([]int64, 0, len(listed.Collections))
	for index := len(listed.Collections) - 1; index >= 0; index-- {
		orderedIDs = append(orderedIDs, listed.Collections[index].ID)
	}
	reordered := application.ReorderCollections(ctx, orderedIDs)
	if reordered.Error != nil ||
		len(reordered.Collections) != len(orderedIDs) ||
		reordered.Collections[0].ID != orderedIDs[0] {
		t.Fatalf("ReorderCollections() = %#v", reordered)
	}

	if _, err := provider.SQL().ExecContext(
		ctx,
		"UPDATE shelves SET query_version = 99 WHERE id = ?",
		emptyCollection.ID,
	); err != nil {
		t.Fatal(err)
	}
	listed = application.ListCollections(ctx)
	if listed.Error != nil ||
		verticalCollectionSummary(t, listed, emptyCollection.ID).AttentionReason !=
			collections.AttentionUnmigratableQuery {
		t.Fatalf("ListCollections(unmigratable) = %#v", listed)
	}
	if deleted := application.DeleteCollection(ctx, emptyCollection.ID); deleted.Error != nil ||
		!deleted.Success {
		t.Fatalf("DeleteCollection(invalid) = %#v", deleted)
	}
	if deleted := application.DeleteCollection(
		ctx,
		duplicated.Collection.ID,
	); deleted.Error != nil || !deleted.Success {
		t.Fatalf("DeleteCollection(duplicate) = %#v", deleted)
	}
}

func createVerticalCollectionValue(
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

func createVerticalCollection(
	t *testing.T,
	application *Application,
	name string,
	query library.StoryLibraryQuery,
) collections.Collection {
	t.Helper()

	response := application.CreateCollection(context.Background(), name, query)
	if response.Error != nil || response.Collection == nil {
		t.Fatalf("CreateCollection(%s) = %#v", name, response)
	}
	return *response.Collection
}

func verticalCollectionSummary(
	t *testing.T,
	response CollectionListResponse,
	shelfID int64,
) collections.Summary {
	t.Helper()

	for _, summary := range response.Collections {
		if summary.ID == shelfID {
			return summary
		}
	}
	t.Fatalf("shelf %d missing from %#v", shelfID, response)
	return collections.Summary{}
}
