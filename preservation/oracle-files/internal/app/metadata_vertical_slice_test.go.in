package app

import (
	"bytes"
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/01max/librairii/internal/artwork"
	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/lunii"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/tagging"
)

type verticalArtworkFetcher func(
	context.Context,
	string,
	int64,
) (lunii.ArtworkPayload, error)

func (f verticalArtworkFetcher) FetchArtwork(
	ctx context.Context,
	sourceURL string,
	maximumBytes int64,
) (lunii.ArtworkPayload, error) {
	return f(ctx, sourceURL, maximumBytes)
}

func TestMetadataVerticalSliceKeepsOfficialAndLocalOrganizationAvailableOffline(
	t *testing.T,
) {
	t.Parallel()

	ctx := context.Background()
	provider := newRuntimeStorageProvider(t)
	payload, err := os.ReadFile("../lunii/testdata/catalog.json")
	if err != nil {
		t.Fatal(err)
	}
	fetcher := &runtimeCatalogFetcher{payload: payload}
	events := &runtimeEventRecorder{events: make(chan operations.Snapshot, 32)}
	runtime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Date(2026, time.July, 25, 17, 0, 0, 0, time.UTC)},
		events,
		1,
		WithMetadataFetcher(fetcher),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := runtime.Start(ctx); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := runtime.Close(); err != nil {
			t.Error(err)
		}
	})

	stories := catalog.NewRepository(provider.SQL())
	matched, _, err := stories.Create(ctx, catalog.CreateStory{
		UUID:                "123e4567-e89b-42d3-a456-426614174000",
		EmbeddedTitle:       "Embedded mountain",
		EmbeddedDescription: "Embedded matched description",
		OriginalFilename:    "matched.zip",
		DetectedFormat:      catalog.FormatZIP,
		SHA256:              strings.Repeat("a", 64),
		ByteSize:            100,
		ManagedPath:         "archives/a/matched.zip",
	})
	if err != nil {
		t.Fatal(err)
	}
	unmatched, _, err := stories.Create(ctx, catalog.CreateStory{
		UUID:                "00112233-4455-4677-8899-aabbccddeeff",
		EmbeddedTitle:       "Unmatched local story",
		EmbeddedDescription: "Embedded unmatched description",
		OriginalFilename:    "unmatched.zip",
		DetectedFormat:      catalog.FormatZIP,
		SHA256:              strings.Repeat("b", 64),
		ByteSize:            200,
		ManagedPath:         "archives/b/unmatched.zip",
	})
	if err != nil {
		t.Fatal(err)
	}
	mood, err := runtime.CreateDefinition(ctx, tagging.CreateDefinition{
		Key:   "mood",
		Label: "Mood",
		Color: "#405CF5",
		Kind:  tagging.KindChoice,
	})
	if err != nil {
		t.Fatal(err)
	}
	calm, err := runtime.CreateValue(ctx, tagging.CreateValue{
		DefinitionID: mood.ID,
		Key:          "calm",
		Label:        "Calm",
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := runtime.SetBulkChoiceValue(
		ctx,
		[]int64{matched.ID},
		mood.ID,
		calm.ID,
		true,
	); err != nil {
		t.Fatal(err)
	}

	refresh, err := runtime.StartMetadataRefresh(ctx, metadata.DefaultLocale)
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, refresh.ID)
	if terminal.Status != operations.StatusSucceeded {
		t.Fatalf("metadata refresh = %#v", terminal)
	}

	page, err := runtime.Search(ctx, library.StoryLibraryQuery{})
	if err != nil {
		t.Fatal(err)
	}
	matchedSummary := storySummaryByUUID(t, page, matched.UUID)
	unmatchedSummary := storySummaryByUUID(t, page, unmatched.UUID)
	if matchedSummary.Title != "The Clockwork Mountain" ||
		matchedSummary.Author != "A. Example" ||
		matchedSummary.Sources.Title != library.SourceOfficial ||
		matchedSummary.Sources.Description != library.SourceOfficial ||
		matchedSummary.Sources.Author != library.SourceOfficial ||
		matchedSummary.Sources.Artwork != library.SourceOfficial ||
		len(matchedSummary.ArtworkID) != 64 ||
		matchedSummary.Official == nil ||
		matchedSummary.Official.Provenance != metadata.ProvenanceLuniiCatalog ||
		matchedSummary.Official.Language != metadata.DefaultLocale ||
		matchedSummary.Official.MinimumAge == nil ||
		*matchedSummary.Official.MinimumAge != 3 ||
		matchedSummary.Official.MaximumAge == nil ||
		*matchedSummary.Official.MaximumAge != 5 ||
		matchedSummary.Official.FetchedAt == "" ||
		matchedSummary.Official.ActivatedAt == "" {
		t.Fatalf("matched story = %#v", matchedSummary)
	}
	if unmatchedSummary.Title != "Unmatched local story" ||
		unmatchedSummary.Sources.Title != library.SourceEmbedded ||
		unmatchedSummary.Sources.Description != library.SourceEmbedded ||
		unmatchedSummary.Official != nil {
		t.Fatalf("unmatched story = %#v", unmatchedSummary)
	}

	tagCatalog, err := runtime.Catalog(ctx)
	if err != nil {
		t.Fatal(err)
	}
	ageDefinition, ageValue := requiredAgeFacet(t, tagCatalog)
	combined, err := runtime.Search(ctx, library.StoryLibraryQuery{
		Name:            "clockwork",
		Languages:       []string{metadata.DefaultLocale},
		Compatibilities: []library.Compatibility{library.CompatibilityCompatible},
		ChoiceFilters: []library.ChoiceFilter{
			{DefinitionID: ageDefinition.ID, ValueIDs: []int64{ageValue.ID}},
			{DefinitionID: mood.ID, ValueIDs: []int64{calm.ID}},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if combined.TotalItems != 1 ||
		len(combined.Stories) != 1 ||
		combined.Stories[0].ID != matched.ID {
		t.Fatalf("combined metadata/user/search query = %#v", combined)
	}

	var artworkFetches atomic.Int32
	handler, err := artwork.NewAssetHandler(
		provider,
		verticalArtworkFetcher(func(
			_ context.Context,
			sourceURL string,
			maximumBytes int64,
		) (lunii.ArtworkPayload, error) {
			artworkFetches.Add(1)
			if !strings.HasSuffix(sourceURL, "/fixture/clockwork-mountain.png") ||
				maximumBytes != artwork.DefaultMaximumBytes {
				t.Fatalf("artwork fetch = %q, %d", sourceURL, maximumBytes)
			}
			return lunii.ArtworkPayload{
				Content:     testfixture.PNG(),
				ContentType: "image/png",
				ETag:        `"metadata-vertical-slice"`,
			}, nil
		}),
	)
	if err != nil {
		t.Fatal(err)
	}
	for iteration := 0; iteration < 2; iteration++ {
		request := httptest.NewRequest(
			http.MethodGet,
			"/artwork/"+matchedSummary.ArtworkID,
			nil,
		)
		response := httptest.NewRecorder()
		handler.ServeHTTP(response, request)
		if response.Code != http.StatusOK ||
			response.Header().Get("Content-Type") != "image/png" ||
			response.Body.Len() == 0 {
			t.Fatalf("artwork response = %d, %#v", response.Code, response.Header())
		}
	}
	if artworkFetches.Load() != 1 {
		t.Fatalf("artwork fetch count = %d", artworkFetches.Load())
	}

	fetcher.payload = nil
	fetcher.err = errors.New("offline")
	failedRefresh, err := runtime.StartMetadataRefresh(ctx, metadata.DefaultLocale)
	if err != nil {
		t.Fatal(err)
	}
	terminal = events.waitTerminal(t, failedRefresh.ID)
	if terminal.Status != operations.StatusFailed {
		t.Fatalf("offline metadata refresh = %#v", terminal)
	}
	status, err := runtime.MetadataStatus(ctx, metadata.DefaultLocale)
	if err != nil {
		t.Fatal(err)
	}
	if status.State != metadata.CatalogStaleCache ||
		status.MatchedStoryCount != 1 {
		t.Fatalf("offline metadata status = %#v", status)
	}
	offline, err := runtime.Search(ctx, library.StoryLibraryQuery{
		Name:      "clockwork",
		Languages: []string{metadata.DefaultLocale},
		ChoiceFilters: []library.ChoiceFilter{
			{DefinitionID: ageDefinition.ID, ValueIDs: []int64{ageValue.ID}},
			{DefinitionID: mood.ID, ValueIDs: []int64{calm.ID}},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if offline.TotalItems != 1 ||
		offline.Stories[0].Title != "The Clockwork Mountain" {
		t.Fatalf("offline last-known-good query = %#v", offline)
	}
}

func TestImportProjectsDerivedAgeFromAnExistingActiveCatalog(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	provider := newRuntimeStorageProvider(t)
	payload, err := os.ReadFile("../lunii/testdata/catalog.json")
	if err != nil {
		t.Fatal(err)
	}
	payload = bytes.ReplaceAll(
		payload,
		[]byte("123e4567-e89b-42d3-a456-426614174000"),
		[]byte(testfixture.StoryUUID),
	)
	events := &runtimeEventRecorder{events: make(chan operations.Snapshot, 32)}
	runtime, err := NewImportRuntime(
		provider,
		fixedClock{now: time.Date(2026, time.July, 25, 18, 0, 0, 0, time.UTC)},
		events,
		1,
		WithMetadataFetcher(&runtimeCatalogFetcher{payload: payload}),
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := runtime.Start(ctx); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := runtime.Close(); err != nil {
			t.Error(err)
		}
	})

	refresh, err := runtime.StartMetadataRefresh(ctx, metadata.DefaultLocale)
	if err != nil {
		t.Fatal(err)
	}
	if terminal := events.waitTerminal(t, refresh.ID); terminal.Status != operations.StatusSucceeded {
		t.Fatalf("metadata refresh = %#v", terminal)
	}
	source, err := testfixture.WriteZIP(t.TempDir(), testfixture.GenericZIP())
	if err != nil {
		t.Fatal(err)
	}
	created, err := runtime.StartImport(ctx, []string{source})
	if err != nil {
		t.Fatal(err)
	}
	terminal := events.waitTerminal(t, created.ID)
	if terminal.Status != operations.StatusSucceeded || terminal.Items[0].StoryID == 0 {
		t.Fatalf("import operation = %#v", terminal)
	}
	tagCatalog, err := runtime.Catalog(ctx)
	if err != nil {
		t.Fatal(err)
	}
	ageDefinition, ageValue := requiredAgeFacet(t, tagCatalog)
	page, err := runtime.Search(ctx, library.StoryLibraryQuery{
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: ageDefinition.ID,
			ValueIDs:     []int64{ageValue.ID},
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	if page.TotalItems != 1 ||
		page.Stories[0].UUID != testfixture.StoryUUID ||
		page.Stories[0].Title != "The Clockwork Mountain" {
		t.Fatalf("derived age after import = %#v", page)
	}
}

func storySummaryByUUID(
	t *testing.T,
	page library.Page,
	storyUUID string,
) library.StorySummary {
	t.Helper()
	for _, story := range page.Stories {
		if story.UUID == storyUUID {
			return story
		}
	}
	t.Fatalf("story %s missing from %#v", storyUUID, page)
	return library.StorySummary{}
}

func requiredAgeFacet(
	t *testing.T,
	catalog tagging.Catalog,
) (tagging.DefinitionWithValues, tagging.Value) {
	t.Helper()
	for _, definition := range catalog.Definitions {
		if definition.Key != metadata.AgeDefinitionKey {
			continue
		}
		for _, value := range definition.Values {
			if value.Key == "3-5" {
				return definition, value
			}
		}
	}
	t.Fatalf("derived age facet missing from %#v", catalog)
	return tagging.DefinitionWithValues{}, tagging.Value{}
}
