package app

import (
	"archive/zip"
	"context"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/tagging"
)

func TestTaggingAndSharedQueryVerticalSlice(t *testing.T) {
	t.Parallel()

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
	firstSource, err := testfixture.WriteZIP(
		fixtureDirectory,
		testfixture.GenericZIP(),
	)
	if err != nil {
		t.Fatal(err)
	}
	secondSource, err := testfixture.WriteZIP(
		fixtureDirectory,
		storyFixtureWithUUID(
			testfixture.GenericZIP(),
			"11112222-3333-4444-8555-666677778888",
			"moonlit-workshop.zip",
		),
	)
	if err != nil {
		t.Fatal(err)
	}
	dialogs := &recordingDialogs{paths: []string{firstSource, secondSource}}
	application, err := New(Dependencies{
		Clock:      fixedClock{now: time.Now()},
		Dialogs:    dialogs,
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
	if err := application.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		application.Stop(context.Background())
	})

	started := application.SelectAndStartImport(context.Background())
	if started.Error != nil || started.Operation == nil {
		t.Fatalf("SelectAndStartImport() = %#v", started)
	}
	if terminal := events.waitTerminal(t, started.Operation.ID); terminal.Status != operations.StatusSucceeded {
		t.Fatalf("terminal import = %#v", terminal)
	}
	page := application.QueryStories(context.Background(), library.StoryLibraryQuery{
		Page:     1,
		PageSize: 12,
		Sort:     library.SortImportedNewest,
	})
	if page.Error != nil || page.Page == nil || page.Page.TotalItems != 2 {
		t.Fatalf("QueryStories() = %#v", page)
	}
	storyIDs := []int64{page.Page.Stories[0].ID, page.Page.Stories[1].ID}

	catalog := application.TagCatalog(context.Background())
	if catalog.Error != nil ||
		catalog.Catalog == nil ||
		len(catalog.Catalog.Definitions) != 2 ||
		catalog.Catalog.Definitions[0].Key != tagging.BrokenKey ||
		catalog.Catalog.Definitions[1].Key != "age" ||
		catalog.Catalog.Definitions[1].Source != tagging.SourceDerived {
		t.Fatalf("TagCatalog() = %#v", catalog)
	}
	broken := catalog.Catalog.Definitions[0]
	if renamed := application.RenameTagDefinition(
		context.Background(),
		broken.ID,
		"Unsafe",
	); renamed.Error == nil || renamed.Error.Code != ErrorConflict {
		t.Fatalf("RenameTagDefinition(broken) = %#v", renamed)
	}

	moodResponse := application.CreateTagDefinition(
		context.Background(),
		tagging.CreateDefinition{
			Key:   "mood",
			Label: "Mood",
			Color: "#405CF5",
			Kind:  tagging.KindChoice,
		},
	)
	if moodResponse.Error != nil || moodResponse.Definition == nil {
		t.Fatalf("CreateTagDefinition() = %#v", moodResponse)
	}
	mood := moodResponse.Definition
	calmResponse := application.CreateTagValue(
		context.Background(),
		tagging.CreateValue{
			DefinitionID: mood.ID,
			Key:          "calm",
			Label:        "Calm",
		},
	)
	if calmResponse.Error != nil || calmResponse.Value == nil {
		t.Fatalf("CreateTagValue() = %#v", calmResponse)
	}
	calm := calmResponse.Value

	assignedBroken := application.SetBooleanTag(
		context.Background(),
		storyIDs,
		broken.ID,
		true,
	)
	if assignedBroken.Error != nil ||
		assignedBroken.Result == nil ||
		assignedBroken.Result.ChangedStories != 2 {
		t.Fatalf("SetBooleanTag() = %#v", assignedBroken)
	}
	idempotent := application.SetBooleanTag(
		context.Background(),
		storyIDs,
		broken.ID,
		true,
	)
	if idempotent.Error != nil ||
		idempotent.Result == nil ||
		idempotent.Result.ChangedStories != 0 {
		t.Fatalf("SetBooleanTag(idempotent) = %#v", idempotent)
	}
	assignedCalm := application.SetChoiceTagValue(
		context.Background(),
		storyIDs[:1],
		mood.ID,
		calm.ID,
		true,
	)
	if assignedCalm.Error != nil ||
		assignedCalm.Result == nil ||
		assignedCalm.Result.ChangedStories != 1 {
		t.Fatalf("SetChoiceTagValue() = %#v", assignedCalm)
	}

	workspace := application.TagAssignmentWorkspace(
		context.Background(),
		storyIDs,
	)
	if workspace.Error != nil ||
		workspace.Workspace == nil ||
		workspace.Workspace.States[0].AssignedStories != 2 ||
		workspace.Workspace.States[1].Values[0].AssignedStories != 1 {
		t.Fatalf("TagAssignmentWorkspace() = %#v", workspace)
	}
	filtered := application.QueryStories(
		context.Background(),
		library.StoryLibraryQuery{
			BooleanFilters: []library.BooleanFilter{{
				DefinitionID: broken.ID,
				State:        library.BooleanTrue,
			}},
			ChoiceFilters: []library.ChoiceFilter{{
				DefinitionID: mood.ID,
				ValueIDs:     []int64{calm.ID},
			}},
			Page:     1,
			PageSize: 12,
			Sort:     library.SortImportedNewest,
		},
	)
	if filtered.Error != nil ||
		filtered.Page == nil ||
		filtered.Page.TotalItems != 1 ||
		filtered.Page.Stories[0].ID != storyIDs[0] {
		t.Fatalf("QueryStories(filtered) = %#v", filtered)
	}

	if renamed := application.RenameTagDefinition(
		context.Background(),
		mood.ID,
		"Emotions",
	); renamed.Error != nil || renamed.Definition == nil {
		t.Fatalf("RenameTagDefinition() = %#v", renamed)
	}
	if recolored := application.RecolorTagDefinition(
		context.Background(),
		mood.ID,
		"#263A8B",
	); recolored.Error != nil ||
		recolored.Definition == nil ||
		recolored.Definition.Color != "#263A8B" {
		t.Fatalf("RecolorTagDefinition() = %#v", recolored)
	}
	if renamed := application.RenameTagValue(
		context.Background(),
		calm.ID,
		"Peaceful",
	); renamed.Error != nil ||
		renamed.Value == nil ||
		renamed.Value.Label != "Peaceful" {
		t.Fatalf("RenameTagValue() = %#v", renamed)
	}
	valuePlan := application.PlanTagValueDeletion(context.Background(), calm.ID)
	if valuePlan.Error != nil ||
		valuePlan.Plan == nil ||
		valuePlan.Plan.AssignmentCount != 1 {
		t.Fatalf("PlanTagValueDeletion() = %#v", valuePlan)
	}
	if deleted := application.DeleteTagValue(
		context.Background(),
		*valuePlan.Plan,
	); deleted.Error != nil || !deleted.Success {
		t.Fatalf("DeleteTagValue() = %#v", deleted)
	}
	definitionPlan := application.PlanTagDefinitionDeletion(context.Background(), mood.ID)
	if definitionPlan.Error != nil ||
		definitionPlan.Plan == nil ||
		definitionPlan.Plan.AssignmentCount != 0 {
		t.Fatalf("PlanTagDefinitionDeletion() = %#v", definitionPlan)
	}
	if deleted := application.DeleteTagDefinition(
		context.Background(),
		*definitionPlan.Plan,
	); deleted.Error != nil || !deleted.Success {
		t.Fatalf("DeleteTagDefinition() = %#v", deleted)
	}
}

func storyFixtureWithUUID(
	archive testfixture.Archive,
	uuid string,
	filename string,
) testfixture.Archive {
	archive.Filename = filename
	archive.ExpectedUUID = uuid
	for index := range archive.Entries {
		archive.Entries[index].Name = strings.Replace(
			archive.Entries[index].Name,
			testfixture.StoryUUID,
			uuid,
			1,
		)
		archive.Entries[index].Method = zip.Store
	}
	return archive
}
