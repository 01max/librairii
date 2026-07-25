package main

import (
	"archive/zip"
	"bytes"
	"context"
	_ "embed"
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/platform"
	"github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/tagging"
	"github.com/google/uuid"
)

const matchedStoryUUID = "123e4567-e89b-42d3-a456-426614174000"

//go:embed testdata/catalog.json
var catalogFixture []byte

type smokeResult struct {
	ImportedStories int
	ExportScopes    int
	Recovered       bool
	OfflineRestart  bool
}

func main() {
	root := flag.String("root", "", "absolute isolated application-data root")
	flag.Parse()
	if *root == "" || !filepath.IsAbs(*root) {
		fmt.Fprintln(os.Stderr, "-root must be an absolute path")
		os.Exit(2)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	result, err := runReleaseSmoke(ctx, filepath.Clean(*root), catalogFixture)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fmt.Printf(
		"release smoke passed: imported=%d export_scopes=%d recovery=%t offline_restart=%t root=%s\n",
		result.ImportedStories,
		result.ExportScopes,
		result.Recovered,
		result.OfflineRestart,
		filepath.Clean(*root),
	)
}

func runReleaseSmoke(
	ctx context.Context,
	root string,
	catalogPayload []byte,
) (smokeResult, error) {
	if !filepath.IsAbs(root) {
		return smokeResult{}, errors.New("release smoke root must be absolute")
	}
	if len(catalogPayload) == 0 {
		return smokeResult{}, errors.New("release smoke catalog fixture is empty")
	}
	if err := os.MkdirAll(root, 0o700); err != nil {
		return smokeResult{}, fmt.Errorf("create release smoke root: %w", err)
	}
	fixtureRoot, err := os.MkdirTemp(
		filepath.Dir(root),
		".librairii-release-smoke-fixtures-",
	)
	if err != nil {
		return smokeResult{}, fmt.Errorf("create smoke fixture root: %w", err)
	}
	defer func() {
		_ = os.RemoveAll(fixtureRoot)
	}()

	sources, sourceBytes, err := writeSmokeArchives(fixtureRoot)
	if err != nil {
		return smokeResult{}, err
	}
	destinations, err := createExportDestinations(fixtureRoot, 4)
	if err != nil {
		return smokeResult{}, err
	}
	dialogs := &smokeDialogs{
		files:        sources,
		destinations: destinations,
	}
	events := newSmokeEvents()
	composition, err := newSmokeComposition(
		root,
		smokeCatalogFetcher{payload: catalogPayload},
		dialogs,
		events,
	)
	if err != nil {
		return smokeResult{}, err
	}
	running := false
	defer func() {
		if running {
			composition.application.Stop(context.Background())
		}
	}()
	if err := composition.start(ctx); err != nil {
		return smokeResult{}, err
	}
	running = true

	imported, err := importAndInspect(ctx, composition.application, events)
	if err != nil {
		return smokeResult{}, err
	}
	defaultStory, found := imported[testfixture.StoryUUID]
	if !found {
		return smokeResult{}, errors.New("default synthetic story was not imported")
	}
	matchedStory, found := imported[matchedStoryUUID]
	if !found {
		return smokeResult{}, errors.New("metadata-matched synthetic story was not imported")
	}

	taggedQuery, calmShelf, err := tagSearchFilterAndSaveShelf(
		ctx,
		composition.application,
		defaultStory.ID,
	)
	if err != nil {
		return smokeResult{}, err
	}
	officialQuery, err := refreshAndFilterOfficialMetadata(
		ctx,
		composition.application,
		events,
		matchedStory.ID,
	)
	if err != nil {
		return smokeResult{}, err
	}
	allShelf, officialShelf, err := createExportShelves(
		ctx,
		composition.application,
		officialQuery,
	)
	if err != nil {
		return smokeResult{}, err
	}

	exportRequests := []struct {
		request exporter.PreflightRequest
		count   int
	}{
		{
			request: exporter.PreflightRequest{
				SourceType: operations.ExportSourceSelection,
				StoryIDs:   []int64{defaultStory.ID},
			},
			count: 1,
		},
		{
			request: exporter.PreflightRequest{
				SourceType: operations.ExportSourceCurrentQuery,
				Query:      officialQuery,
			},
			count: 1,
		},
		{
			request: exporter.PreflightRequest{
				SourceType: operations.ExportSourceShelf,
				ShelfIDs:   []int64{calmShelf.ID},
			},
			count: 1,
		},
		{
			request: exporter.PreflightRequest{
				SourceType: operations.ExportSourceShelves,
				ShelfIDs:   []int64{allShelf.ID, officialShelf.ID},
			},
			count: 2,
		},
	}
	for index, exportCase := range exportRequests {
		if err := runExportScope(
			ctx,
			composition.application,
			events,
			exportCase.request,
			destinations[index],
			exportCase.count,
		); err != nil {
			return smokeResult{}, fmt.Errorf(
				"export scope %s: %w",
				exportCase.request.SourceType,
				err,
			)
		}
	}
	if err := verifyExportBytes(
		destinations,
		sources,
		sourceBytes,
	); err != nil {
		return smokeResult{}, err
	}

	removed := composition.application.RemoveStory(ctx, defaultStory.ID)
	if removed.Error != nil || removed.Result == nil {
		return smokeResult{}, fmt.Errorf("remove imported story: %#v", removed)
	}
	remaining := composition.application.QueryStories(
		ctx,
		library.StoryLibraryQuery{
			Page: 1, PageSize: 100, Sort: library.SortNameAscending,
		},
	)
	if remaining.Error != nil ||
		remaining.Page == nil ||
		remaining.Page.TotalItems != 1 ||
		remaining.Page.Stories[0].ID != matchedStory.ID {
		return smokeResult{}, fmt.Errorf(
			"collection after removal: %#v",
			remaining,
		)
	}
	trashFiles, err := regularFiles(composition.readiness.Layout().Trash)
	if err != nil {
		return smokeResult{}, fmt.Errorf("read managed trash: %w", err)
	}
	if len(trashFiles) != 1 {
		return smokeResult{}, fmt.Errorf(
			"managed trash after removal: files=%v",
			trashFiles,
		)
	}
	for index, source := range sources {
		after, readErr := os.ReadFile(source)
		if readErr != nil {
			return smokeResult{}, fmt.Errorf(
				"read import source %s after removal: %w",
				filepath.Base(source),
				readErr,
			)
		}
		if !bytes.Equal(after, sourceBytes[index]) {
			return smokeResult{}, fmt.Errorf(
				"import source %s changed",
				filepath.Base(source),
			)
		}
	}

	interruptedID, err := stageInterruptedWork(
		ctx,
		composition.readiness,
	)
	if err != nil {
		return smokeResult{}, err
	}
	composition.application.Stop(ctx)
	running = false

	offlineEvents := newSmokeEvents()
	offlineComposition, err := newSmokeComposition(
		root,
		smokeCatalogFetcher{err: errors.New("offline fixture")},
		&smokeDialogs{},
		offlineEvents,
	)
	if err != nil {
		return smokeResult{}, err
	}
	composition = offlineComposition
	if err := composition.start(ctx); err != nil {
		return smokeResult{}, err
	}
	running = true
	if err := verifyRecoveryAndOfflineRestart(
		ctx,
		composition,
		offlineEvents,
		interruptedID,
		matchedStory.ID,
		taggedQuery,
	); err != nil {
		return smokeResult{}, err
	}

	return smokeResult{
		ImportedStories: len(imported),
		ExportScopes:    len(exportRequests),
		Recovered:       true,
		OfflineRestart:  true,
	}, nil
}

type smokeComposition struct {
	application *coreapp.Application
	runtime     *coreapp.ImportRuntime
	readiness   *platform.StorageReadiness
}

func newSmokeComposition(
	root string,
	fetcher metadata.CatalogFetcher,
	dialogs coreapp.DialogPort,
	events coreapp.EventPort,
) (*smokeComposition, error) {
	readiness := platform.NewStorageReadiness(root)
	clock := platform.SystemClock{}
	runtime, err := coreapp.NewImportRuntime(
		readiness,
		clock,
		events,
		2,
		coreapp.WithMetadataFetcher(fetcher),
	)
	if err != nil {
		return nil, fmt.Errorf("construct release smoke runtime: %w", err)
	}
	application, err := coreapp.New(coreapp.Dependencies{
		Clock:       clock,
		Dialogs:     dialogs,
		Events:      events,
		Readiness:   readiness,
		Operations:  runtime,
		Library:     runtime,
		Removal:     runtime,
		Tags:        runtime,
		Shelves:     runtime,
		Diagnostics: runtime,
		Resources:   []coreapp.ResourcePort{readiness},
	})
	if err != nil {
		return nil, fmt.Errorf("construct release smoke application: %w", err)
	}
	return &smokeComposition{
		application: application,
		runtime:     runtime,
		readiness:   readiness,
	}, nil
}

func (c *smokeComposition) start(ctx context.Context) error {
	if err := c.application.Start(ctx); err != nil {
		return fmt.Errorf("start release smoke application: %w", err)
	}
	status := c.application.Status()
	if status.State != coreapp.StateReady || !status.MutationsAllowed {
		return fmt.Errorf("release smoke application is not ready: %#v", status)
	}
	return nil
}

func importAndInspect(
	ctx context.Context,
	application *coreapp.Application,
	events *smokeEvents,
) (map[string]library.StorySummary, error) {
	started := application.SelectAndStartImport(ctx)
	if started.Error != nil || started.Operation == nil || started.Cancelled {
		return nil, fmt.Errorf("start release smoke import: %#v", started)
	}
	terminal, err := events.waitTerminal(ctx, started.Operation.ID)
	if err != nil {
		return nil, err
	}
	if terminal.Status != operations.StatusSucceeded ||
		len(terminal.Items) != 2 {
		return nil, fmt.Errorf("release smoke import failed: %#v", terminal)
	}
	page := application.QueryStories(ctx, library.StoryLibraryQuery{
		Page: 1, PageSize: 100, Sort: library.SortNameAscending,
	})
	if page.Error != nil || page.Page == nil || page.Page.TotalItems != 2 {
		return nil, fmt.Errorf("query imported stories: %#v", page)
	}
	stories := make(map[string]library.StorySummary, len(page.Page.Stories))
	for _, story := range page.Page.Stories {
		detail := application.StoryDetail(ctx, story.ID)
		if detail.Error != nil ||
			detail.Detail == nil ||
			detail.Detail.Archive.SHA256 == "" ||
			detail.Detail.Archive.ByteSize <= 0 ||
			detail.Detail.Archive.Verification != library.CompatibilityCompatible {
			return nil, fmt.Errorf("inspect imported story %d: %#v", story.ID, detail)
		}
		stories[story.UUID] = story
	}
	return stories, nil
}

func tagSearchFilterAndSaveShelf(
	ctx context.Context,
	application *coreapp.Application,
	storyID int64,
) (library.StoryLibraryQuery, shelves.Shelf, error) {
	catalogResponse := application.TagCatalog(ctx)
	if catalogResponse.Error != nil || catalogResponse.Catalog == nil {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("load tag catalog: %#v", catalogResponse)
	}
	broken, found := findDefinition(
		catalogResponse.Catalog.Definitions,
		tagging.BrokenKey,
	)
	if !found {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			errors.New("protected broken tag is missing")
	}
	moodResponse := application.CreateTagDefinition(ctx, tagging.CreateDefinition{
		Key:   "smoke-mood",
		Label: "Smoke mood",
		Color: "#405CF5",
		Kind:  tagging.KindChoice,
	})
	if moodResponse.Error != nil || moodResponse.Definition == nil {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("create smoke tag: %#v", moodResponse)
	}
	calmResponse := application.CreateTagValue(ctx, tagging.CreateValue{
		DefinitionID: moodResponse.Definition.ID,
		Key:          "calm",
		Label:        "Calm",
	})
	if calmResponse.Error != nil || calmResponse.Value == nil {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("create smoke tag value: %#v", calmResponse)
	}
	if assigned := application.SetBooleanTag(
		ctx,
		[]int64{storyID},
		broken.ID,
		true,
	); assigned.Error != nil {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("assign broken tag: %#v", assigned)
	}
	if assigned := application.SetChoiceTagValue(
		ctx,
		[]int64{storyID},
		moodResponse.Definition.ID,
		calmResponse.Value.ID,
		true,
	); assigned.Error != nil {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("assign smoke choice: %#v", assigned)
	}

	nameSearch := application.QueryStories(ctx, library.StoryLibraryQuery{
		Name: "clockwork forest", Page: 1, PageSize: 100,
		Sort: library.SortNameAscending,
	})
	if nameSearch.Error != nil ||
		nameSearch.Page == nil ||
		nameSearch.Page.TotalItems != 1 ||
		nameSearch.Page.Stories[0].ID != storyID {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("name search smoke: %#v", nameSearch)
	}
	query := library.StoryLibraryQuery{
		BooleanFilters: []library.BooleanFilter{{
			DefinitionID: broken.ID,
			State:        library.BooleanTrue,
		}},
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: moodResponse.Definition.ID,
			ValueIDs:     []int64{calmResponse.Value.ID},
		}},
		Page:     1,
		PageSize: 100,
		Sort:     library.SortNameAscending,
	}
	filtered := application.QueryStories(ctx, query)
	if filtered.Error != nil ||
		filtered.Page == nil ||
		filtered.Page.TotalItems != 1 ||
		filtered.Page.Stories[0].ID != storyID {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("combined tag filter smoke: %#v", filtered)
	}
	shelfResponse := application.CreateShelf(ctx, "Calm and broken", query)
	if shelfResponse.Error != nil || shelfResponse.Shelf == nil {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("save smoke shelf: %#v", shelfResponse)
	}
	opened := application.OpenShelf(
		ctx,
		shelfResponse.Shelf.ID,
		library.ListRequest{
			Page: 1, PageSize: 100, Sort: library.SortNameAscending,
		},
	)
	if opened.Error != nil ||
		opened.Evaluation == nil ||
		opened.Evaluation.Page.TotalItems != 1 {
		return library.StoryLibraryQuery{}, shelves.Shelf{},
			fmt.Errorf("evaluate smoke shelf: %#v", opened)
	}
	return query, *shelfResponse.Shelf, nil
}

func refreshAndFilterOfficialMetadata(
	ctx context.Context,
	application *coreapp.Application,
	events *smokeEvents,
	matchedStoryID int64,
) (library.StoryLibraryQuery, error) {
	refresh := application.RefreshOfficialMetadata(ctx)
	if refresh.Error != nil || refresh.Operation == nil {
		return library.StoryLibraryQuery{},
			fmt.Errorf("start metadata fixture refresh: %#v", refresh)
	}
	terminal, err := events.waitTerminal(ctx, refresh.Operation.ID)
	if err != nil {
		return library.StoryLibraryQuery{}, err
	}
	if terminal.Status != operations.StatusSucceeded {
		return library.StoryLibraryQuery{},
			fmt.Errorf("metadata fixture refresh failed: %#v", terminal)
	}
	catalogResponse := application.TagCatalog(ctx)
	if catalogResponse.Error != nil || catalogResponse.Catalog == nil {
		return library.StoryLibraryQuery{},
			fmt.Errorf("reload derived facets: %#v", catalogResponse)
	}
	age, found := findDefinition(
		catalogResponse.Catalog.Definitions,
		metadata.AgeDefinitionKey,
	)
	if !found {
		return library.StoryLibraryQuery{}, errors.New("derived age facet is missing")
	}
	ageValue, found := findValue(age.Values, "3-5")
	if !found {
		return library.StoryLibraryQuery{}, errors.New("derived age value is missing")
	}
	query := library.StoryLibraryQuery{
		Name:      "clockwork mountain",
		Languages: []string{metadata.DefaultLocale},
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: age.ID,
			ValueIDs:     []int64{ageValue.ID},
		}},
		Page:     1,
		PageSize: 100,
		Sort:     library.SortNameAscending,
	}
	filtered := application.QueryStories(ctx, query)
	if filtered.Error != nil ||
		filtered.Page == nil ||
		filtered.Page.TotalItems != 1 ||
		filtered.Page.Stories[0].ID != matchedStoryID ||
		filtered.Page.Stories[0].Title != "The Clockwork Mountain" {
		return library.StoryLibraryQuery{},
			fmt.Errorf("official metadata filter smoke: %#v", filtered)
	}
	status := application.OfficialMetadataStatus(ctx)
	if status.Error != nil ||
		status.Status.State != metadata.CatalogFresh ||
		status.Status.MatchedStoryCount != 1 {
		return library.StoryLibraryQuery{},
			fmt.Errorf("metadata status after fixture refresh: %#v", status)
	}
	return query, nil
}

func createExportShelves(
	ctx context.Context,
	application *coreapp.Application,
	officialQuery library.StoryLibraryQuery,
) (shelves.Shelf, shelves.Shelf, error) {
	allResponse := application.CreateShelf(
		ctx,
		"All smoke stories",
		library.StoryLibraryQuery{},
	)
	if allResponse.Error != nil || allResponse.Shelf == nil {
		return shelves.Shelf{}, shelves.Shelf{},
			fmt.Errorf("create all-stories shelf: %#v", allResponse)
	}
	officialResponse := application.CreateShelf(
		ctx,
		"Official smoke story",
		officialQuery,
	)
	if officialResponse.Error != nil || officialResponse.Shelf == nil {
		return shelves.Shelf{}, shelves.Shelf{},
			fmt.Errorf("create official shelf: %#v", officialResponse)
	}
	preview := application.PreviewShelves(
		ctx,
		[]int64{allResponse.Shelf.ID, officialResponse.Shelf.ID},
	)
	if preview.Error != nil ||
		preview.Preview == nil ||
		preview.Preview.UniqueStoryCount != 2 ||
		preview.Preview.OverlapCount != 1 {
		return shelves.Shelf{}, shelves.Shelf{},
			fmt.Errorf("preview overlapping smoke shelves: %#v", preview)
	}
	return *allResponse.Shelf, *officialResponse.Shelf, nil
}

func runExportScope(
	ctx context.Context,
	application *coreapp.Application,
	events *smokeEvents,
	request exporter.PreflightRequest,
	destination string,
	expectedCount int,
) error {
	preflight := application.SelectAndPreflightExport(ctx, request)
	if preflight.Error != nil ||
		preflight.Preflight == nil ||
		!preflight.Preflight.CanExport ||
		preflight.Preflight.ResolvedCount != expectedCount {
		return fmt.Errorf("preflight response: %#v", preflight)
	}
	started := application.StartPreparedExport(
		ctx,
		preflight.Preflight.PreparationID,
	)
	if started.Error != nil || started.Operation == nil {
		return fmt.Errorf("start response: %#v", started)
	}
	terminal, err := events.waitTerminal(ctx, started.Operation.ID)
	if err != nil {
		return err
	}
	if terminal.Status != operations.StatusSucceeded ||
		terminal.ExportSourceType != request.SourceType ||
		len(terminal.Items) != expectedCount {
		return fmt.Errorf("terminal response: %#v", terminal)
	}
	for _, item := range terminal.Items {
		if item.Status != operations.ItemSucceeded ||
			item.OutcomeCode != "exported" {
			return fmt.Errorf("export item failed: %#v", item)
		}
	}
	entries, err := os.ReadDir(destination)
	if err != nil {
		return fmt.Errorf("read export destination: %w", err)
	}
	if len(entries) != expectedCount {
		return fmt.Errorf(
			"export destination contains %d files, want %d",
			len(entries),
			expectedCount,
		)
	}
	return nil
}

func verifyExportBytes(
	destinations []string,
	sources []string,
	sourceBytes [][]byte,
) error {
	if len(destinations) != 4 || len(sources) != 2 || len(sourceBytes) != 2 {
		return errors.New("invalid export byte verification fixture")
	}
	expectations := [][]int{
		{0},
		{1},
		{0},
		{0, 1},
	}
	for destinationIndex, sourceIndexes := range expectations {
		for _, sourceIndex := range sourceIndexes {
			exported, err := os.ReadFile(filepath.Join(
				destinations[destinationIndex],
				filepath.Base(sources[sourceIndex]),
			))
			if err != nil {
				return fmt.Errorf("read exported smoke archive: %w", err)
			}
			if !bytes.Equal(exported, sourceBytes[sourceIndex]) {
				return fmt.Errorf(
					"exported smoke archive %s changed",
					filepath.Base(sources[sourceIndex]),
				)
			}
		}
	}
	return nil
}

func stageInterruptedWork(
	ctx context.Context,
	readiness *platform.StorageReadiness,
) (string, error) {
	repository := operations.NewRepository(readiness.Writer())
	operationID := uuid.NewString()
	created, err := repository.CreateImport(
		ctx,
		operationID,
		[]operations.NewItem{{SourceName: "abandoned-smoke.zip"}},
		time.Now(),
	)
	if err != nil {
		return "", fmt.Errorf("stage interrupted operation: %w", err)
	}
	if err := repository.MarkRunning(ctx, operationID, time.Now()); err != nil {
		return "", fmt.Errorf("mark interrupted operation running: %w", err)
	}
	if err := repository.MarkItemRunning(ctx, created.Items[0].ID); err != nil {
		return "", fmt.Errorf("mark interrupted item running: %w", err)
	}
	abandoned := filepath.Join(
		readiness.Layout().Staging,
		"import-abandoned-smoke",
	)
	if err := os.MkdirAll(abandoned, 0o700); err != nil {
		return "", fmt.Errorf("create abandoned staging directory: %w", err)
	}
	if err := os.WriteFile(
		filepath.Join(abandoned, "partial"),
		[]byte("partial smoke bytes"),
		0o600,
	); err != nil {
		return "", fmt.Errorf("write abandoned staging file: %w", err)
	}
	return operationID, nil
}

func verifyRecoveryAndOfflineRestart(
	ctx context.Context,
	composition *smokeComposition,
	events *smokeEvents,
	interruptedID string,
	matchedStoryID int64,
	taggedQuery library.StoryLibraryQuery,
) error {
	reconciled := composition.application.OperationSnapshot(ctx, interruptedID)
	if reconciled.Error != nil ||
		reconciled.Operation == nil ||
		reconciled.Operation.Status != operations.StatusInterrupted ||
		reconciled.Operation.ErrorCode != "interrupted" {
		return fmt.Errorf("interrupted operation recovery: %#v", reconciled)
	}
	stagingEntries, err := os.ReadDir(composition.readiness.Layout().Staging)
	if err != nil {
		return fmt.Errorf("read staging after recovery: %w", err)
	}
	if len(stagingEntries) != 0 {
		return fmt.Errorf("abandoned staging remains: %#v", stagingEntries)
	}
	page := composition.application.QueryStories(ctx, library.StoryLibraryQuery{
		Name: "clockwork mountain", Page: 1, PageSize: 100,
		Sort: library.SortNameAscending,
	})
	if page.Error != nil ||
		page.Page == nil ||
		page.Page.TotalItems != 1 ||
		page.Page.Stories[0].ID != matchedStoryID ||
		page.Page.Stories[0].Title != "The Clockwork Mountain" {
		return fmt.Errorf("offline last-known-good collection: %#v", page)
	}
	tagged := composition.application.QueryStories(ctx, taggedQuery)
	if tagged.Error != nil || tagged.Page == nil || tagged.Page.TotalItems != 0 {
		return fmt.Errorf("dynamic shelf/tag state after removal: %#v", tagged)
	}
	shelfList := composition.application.ListShelves(ctx)
	if shelfList.Error != nil || len(shelfList.Shelves) != 3 {
		return fmt.Errorf("persisted shelves after restart: %#v", shelfList)
	}

	refresh := composition.application.RefreshOfficialMetadata(ctx)
	if refresh.Error != nil || refresh.Operation == nil {
		return fmt.Errorf("start offline metadata refresh: %#v", refresh)
	}
	terminal, err := events.waitTerminal(ctx, refresh.Operation.ID)
	if err != nil {
		return err
	}
	if terminal.Status != operations.StatusFailed {
		return fmt.Errorf("offline metadata refresh terminal: %#v", terminal)
	}
	status := composition.application.OfficialMetadataStatus(ctx)
	if status.Error != nil ||
		status.Status.State != metadata.CatalogStaleCache ||
		status.Status.MatchedStoryCount != 1 {
		return fmt.Errorf("offline metadata cache status: %#v", status)
	}
	return nil
}

type smokeCatalogFetcher struct {
	payload []byte
	err     error
}

func (f smokeCatalogFetcher) FetchCatalog(context.Context) ([]byte, error) {
	if f.err != nil {
		return nil, f.err
	}
	return append([]byte(nil), f.payload...), nil
}

type smokeDialogs struct {
	mu           sync.Mutex
	files        []string
	destinations []string
	revealed     string
}

func (d *smokeDialogs) OpenFiles(
	context.Context,
	coreapp.FileDialogRequest,
) ([]string, error) {
	d.mu.Lock()
	defer d.mu.Unlock()
	return append([]string(nil), d.files...), nil
}

func (d *smokeDialogs) OpenDirectory(context.Context, string) (string, error) {
	d.mu.Lock()
	defer d.mu.Unlock()
	if len(d.destinations) == 0 {
		return "", errors.New("smoke destination queue is empty")
	}
	destination := d.destinations[0]
	d.destinations = d.destinations[1:]
	return destination, nil
}

func (d *smokeDialogs) RevealDirectory(_ context.Context, destination string) error {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.revealed = destination
	return nil
}

type smokeEvents struct {
	mu        sync.RWMutex
	snapshots map[string]operations.Snapshot
	changed   chan struct{}
}

func newSmokeEvents() *smokeEvents {
	return &smokeEvents{
		snapshots: make(map[string]operations.Snapshot),
		changed:   make(chan struct{}, 256),
	}
}

func (e *smokeEvents) Emit(_ context.Context, name string, payload any) {
	if name != operations.EventChanged {
		return
	}
	snapshot, ok := payload.(operations.Snapshot)
	if !ok {
		return
	}
	e.mu.Lock()
	e.snapshots[snapshot.ID] = snapshot
	e.mu.Unlock()
	select {
	case e.changed <- struct{}{}:
	default:
	}
}

func (e *smokeEvents) waitTerminal(
	ctx context.Context,
	operationID string,
) (operations.Snapshot, error) {
	for {
		e.mu.RLock()
		snapshot, found := e.snapshots[operationID]
		e.mu.RUnlock()
		if found && snapshot.Terminal() {
			return snapshot, nil
		}
		select {
		case <-ctx.Done():
			return operations.Snapshot{}, fmt.Errorf(
				"wait for operation %s: %w",
				operationID,
				ctx.Err(),
			)
		case <-e.changed:
		}
	}
}

func writeSmokeArchives(
	directory string,
) ([]string, [][]byte, error) {
	defaultSource, err := testfixture.WriteZIP(
		directory,
		testfixture.PlainPK(),
	)
	if err != nil {
		return nil, nil, fmt.Errorf("write embedded-metadata fixture: %w", err)
	}
	matchedArchive := testfixture.GenericZIP()
	matchedArchive.Filename = "clockwork-mountain.zip"
	matchedArchive.ExpectedUUID = matchedStoryUUID
	for index := range matchedArchive.Entries {
		matchedArchive.Entries[index].Name = strings.ReplaceAll(
			matchedArchive.Entries[index].Name,
			testfixture.StoryUUID,
			matchedStoryUUID,
		)
		matchedArchive.Entries[index].Method = zip.Store
	}
	matchedSource, err := testfixture.WriteZIP(directory, matchedArchive)
	if err != nil {
		return nil, nil, fmt.Errorf("write metadata-matched fixture: %w", err)
	}
	sources := []string{defaultSource, matchedSource}
	sourceBytes := make([][]byte, 0, len(sources))
	for _, source := range sources {
		content, err := os.ReadFile(source)
		if err != nil {
			return nil, nil, fmt.Errorf("read smoke fixture: %w", err)
		}
		sourceBytes = append(sourceBytes, content)
	}
	return sources, sourceBytes, nil
}

func createExportDestinations(
	root string,
	count int,
) ([]string, error) {
	destinations := make([]string, 0, count)
	for index := range count {
		destination := filepath.Join(root, fmt.Sprintf("export-%d", index+1))
		if err := os.Mkdir(destination, 0o700); err != nil {
			return nil, fmt.Errorf("create export destination: %w", err)
		}
		destinations = append(destinations, destination)
	}
	return destinations, nil
}

func regularFiles(root string) ([]string, error) {
	var files []string
	err := filepath.WalkDir(
		root,
		func(path string, entry os.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if entry.Type().IsRegular() {
				files = append(files, path)
			}
			return nil
		},
	)
	return files, err
}

func findDefinition(
	definitions []tagging.DefinitionWithValues,
	key string,
) (tagging.DefinitionWithValues, bool) {
	for _, definition := range definitions {
		if definition.Key == key {
			return definition, true
		}
	}
	return tagging.DefinitionWithValues{}, false
}

func findValue(values []tagging.Value, key string) (tagging.Value, bool) {
	for _, value := range values {
		if value.Key == key {
			return value, true
		}
	}
	return tagging.Value{}, false
}
