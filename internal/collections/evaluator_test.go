package collections_test

import (
	"context"
	"database/sql"
	"fmt"
	"github.com/01max/librairii/internal/testsupport"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/catalog"
	collectionstore "github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/tagging"
)

func TestEvaluatorReevaluatesImportsAndRemovalsThroughStoryLibraryQuery(
	t *testing.T,
) {
	t.Parallel()

	harness := newEvaluationHarness(t)
	collection, err := harness.collections.Create(
		context.Background(),
		"Moon stories",
		library.StoryLibraryQuery{Name: "moon"},
	)
	if err != nil {
		t.Fatal(err)
	}
	assertCollectionCount(t, harness.evaluator, collection.ID, 0)

	matching := createEvaluationStory(
		t,
		harness.catalog,
		"123e4567-e89b-42d3-a456-426614174000",
		"Moon workshop",
		"a",
	)
	createEvaluationStory(
		t,
		harness.catalog,
		"223e4567-e89b-42d3-a456-426614174001",
		"Forest train",
		"b",
	)
	evaluation, err := harness.evaluator.Evaluate(
		context.Background(),
		collection.ID,
		library.ListRequest{Page: 1, PageSize: 12},
	)
	if err != nil {
		t.Fatal(err)
	}
	if evaluation.Collection.ID != collection.ID ||
		evaluation.Page.TotalItems != 1 ||
		len(evaluation.Page.Stories) != 1 ||
		evaluation.Page.Stories[0].ID != matching.ID {
		t.Fatalf("Evaluate(after import) = %#v", evaluation)
	}

	if _, err := harness.database.Exec(
		"DELETE FROM stories WHERE id = ?",
		matching.ID,
	); err != nil {
		t.Fatal(err)
	}
	assertCollectionCount(t, harness.evaluator, collection.ID, 0)
}

func TestEvaluatorReevaluatesTagAssignmentsThroughStoryLibraryQuery(t *testing.T) {
	t.Parallel()

	harness := newEvaluationHarness(t)
	story := createEvaluationStory(
		t,
		harness.catalog,
		"123e4567-e89b-42d3-a456-426614174000",
		"Tag story",
		"a",
	)
	broken, err := testsupport.SeedBooleanTag(context.Background(), harness.database)
	if err != nil {
		t.Fatal(err)
	}
	tags, err := tagging.NewService(harness.database)
	if err != nil {
		t.Fatal(err)
	}
	collection, err := harness.collections.Create(
		context.Background(),
		"Broken stories",
		library.StoryLibraryQuery{IncludeBroken: true,
			BooleanFilters: []library.BooleanFilter{{
				DefinitionID: broken.ID,
				State:        library.BooleanTrue,
			}},
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	assertCollectionCount(t, harness.evaluator, collection.ID, 0)

	if _, err := tags.SetStoryBoolean(
		context.Background(),
		story.ID,
		broken.ID,
		true,
	); err != nil {
		t.Fatal(err)
	}
	assertCollectionCount(t, harness.evaluator, collection.ID, 1)

	if _, err := tags.SetStoryBoolean(
		context.Background(),
		story.ID,
		broken.ID,
		false,
	); err != nil {
		t.Fatal(err)
	}
	assertCollectionCount(t, harness.evaluator, collection.ID, 0)
}

func TestEvaluatorReevaluatesActiveOfficialMetadataThroughStoryLibraryQuery(
	t *testing.T,
) {
	t.Parallel()

	harness := newEvaluationHarness(t)
	story := createEvaluationStory(
		t,
		harness.catalog,
		"123e4567-e89b-42d3-a456-426614174000",
		"Embedded title",
		"a",
	)
	collection, err := harness.collections.Create(
		context.Background(),
		"Official match",
		library.StoryLibraryQuery{Name: "official"},
	)
	if err != nil {
		t.Fatal(err)
	}
	assertCollectionCount(t, harness.evaluator, collection.ID, 0)

	activateEvaluationMetadata(
		t,
		harness.metadata,
		"123e4567-e89b-42d3-a456-426614174100",
		story.UUID,
		"Official Moon",
		1,
	)
	assertCollectionCount(t, harness.evaluator, collection.ID, 1)

	activateEvaluationMetadata(
		t,
		harness.metadata,
		"123e4567-e89b-42d3-a456-426614174101",
		story.UUID,
		"Renamed catalog title",
		2,
	)
	assertCollectionCount(t, harness.evaluator, collection.ID, 0)
}

func TestEvaluatorPreviewsDeduplicatedMultiCollectionMembership(t *testing.T) {
	t.Parallel()

	harness := newEvaluationHarness(t)
	createEvaluationStory(
		t,
		harness.catalog,
		"123e4567-e89b-42d3-a456-426614174000",
		"Moon forest",
		"a",
	)
	createEvaluationStory(
		t,
		harness.catalog,
		"223e4567-e89b-42d3-a456-426614174001",
		"Moon train",
		"b",
	)
	createEvaluationStory(
		t,
		harness.catalog,
		"323e4567-e89b-42d3-a456-426614174002",
		"Forest walk",
		"c",
	)
	moon, err := harness.collections.Create(
		context.Background(),
		"Moon",
		library.StoryLibraryQuery{Name: "moon"},
	)
	if err != nil {
		t.Fatal(err)
	}
	forest, err := harness.collections.Create(
		context.Background(),
		"Forest",
		library.StoryLibraryQuery{Name: "forest"},
	)
	if err != nil {
		t.Fatal(err)
	}

	preview, err := harness.evaluator.PreviewSelection(
		context.Background(),
		[]int64{forest.ID, moon.ID},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(preview.Collections) != 2 ||
		preview.Collections[0] != (collectionstore.SelectedCollectionPreview{
			ID: forest.ID, Name: "Forest", Count: 2,
		}) ||
		preview.Collections[1] != (collectionstore.SelectedCollectionPreview{
			ID: moon.ID, Name: "Moon", Count: 2,
		}) ||
		len(preview.SourceCollectionNames) != 2 ||
		preview.SourceCollectionNames[0] != "Forest" ||
		preview.SourceCollectionNames[1] != "Moon" ||
		preview.UniqueStoryCount != 3 ||
		preview.OverlapCount != 1 {
		t.Fatalf("PreviewSelection() = %#v", preview)
	}
	if _, err := harness.evaluator.PreviewSelection(
		context.Background(),
		[]int64{moon.ID, moon.ID},
	); err != collectionstore.ErrInvalidCollectionSelection {
		t.Fatalf("PreviewSelection(duplicate) error = %v", err)
	}
	if _, err := harness.database.Exec(
		"UPDATE collections SET validity_state = 'needs_attention' WHERE id = ?",
		forest.ID,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := harness.evaluator.PreviewSelection(
		context.Background(),
		[]int64{forest.ID},
	); err != collectionstore.ErrCollectionNeedsAttention {
		t.Fatalf("PreviewSelection(invalid) error = %v", err)
	}
}

func TestEvaluatorReturnsOrderedCountsAndRequiresDependencies(t *testing.T) {
	t.Parallel()

	harness := newEvaluationHarness(t)
	first, err := harness.collections.Create(
		context.Background(),
		"All",
		library.StoryLibraryQuery{},
	)
	if err != nil {
		t.Fatal(err)
	}
	second, err := harness.collections.Create(
		context.Background(),
		"Missing",
		library.StoryLibraryQuery{Name: "missing"},
	)
	if err != nil {
		t.Fatal(err)
	}
	createEvaluationStory(
		t,
		harness.catalog,
		"123e4567-e89b-42d3-a456-426614174000",
		"One story",
		"a",
	)

	counts, err := harness.evaluator.Counts(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(counts) != 2 ||
		counts[0] != (collectionstore.CollectionCount{CollectionID: first.ID, Count: 1}) ||
		counts[1] != (collectionstore.CollectionCount{CollectionID: second.ID, Count: 0}) {
		t.Fatalf("Counts() = %#v", counts)
	}
	if _, err := harness.database.Exec(
		"UPDATE collections SET query_version = ? WHERE id = ?",
		collectionstore.CurrentSavedLibraryQueryVersion+10,
		second.ID,
	); err != nil {
		t.Fatal(err)
	}
	summaries, err := harness.evaluator.Summaries(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(summaries) != 2 ||
		summaries[0].Count != 1 ||
		summaries[1].Validity != collectionstore.ValidityNeedsAttention ||
		summaries[1].AttentionReason != collectionstore.AttentionUnmigratableQuery ||
		summaries[1].Count != 0 {
		t.Fatalf("Summaries() = %#v", summaries)
	}
	if _, err := collectionstore.NewEvaluator(
		nil,
		harness.library,
	); err != collectionstore.ErrMissingDatabase {
		t.Fatalf("NewEvaluator(nil service) error = %v", err)
	}
	if _, err := collectionstore.NewEvaluator(
		harness.collections,
		nil,
	); err != collectionstore.ErrMissingLibraryQuery {
		t.Fatalf("NewEvaluator(nil query) error = %v", err)
	}
}

type evaluationHarness struct {
	database    *sql.DB
	catalog     *catalog.Repository
	metadata    *metadata.Repository
	library     *library.Query
	collections *collectionstore.Service
	evaluator   *collectionstore.Evaluator
}

func newEvaluationHarness(t *testing.T) *evaluationHarness {
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
	metadataRepository := metadata.NewRepository(opened.SQL())
	provider, err := metadata.NewLibraryProvider(
		metadataRepository,
		metadata.DefaultLocale,
	)
	if err != nil {
		t.Fatal(err)
	}
	collectionService, err := collectionstore.NewService(opened.SQL())
	if err != nil {
		t.Fatal(err)
	}
	libraryQuery := library.NewQuery(opened.SQL(), provider)
	evaluator, err := collectionstore.NewEvaluator(collectionService, libraryQuery)
	if err != nil {
		t.Fatal(err)
	}
	return &evaluationHarness{
		database:    opened.SQL(),
		catalog:     catalog.NewRepository(opened.SQL()),
		metadata:    metadataRepository,
		library:     libraryQuery,
		collections: collectionService,
		evaluator:   evaluator,
	}
}

func createEvaluationStory(
	t *testing.T,
	repository *catalog.Repository,
	uuid string,
	title string,
	hashCharacter string,
) catalog.Story {
	t.Helper()

	story, _, err := repository.Create(context.Background(), catalog.CreateStory{
		UUID:             uuid,
		EmbeddedTitle:    title,
		OriginalFilename: title + ".zip",
		DetectedFormat:   catalog.FormatZIP,
		SHA256:           strings.Repeat(hashCharacter, 64),
		ByteSize:         100,
		ManagedPath:      "archives/" + hashCharacter + "/" + hashCharacter + ".zip",
	})
	if err != nil {
		t.Fatal(err)
	}
	return story
}

func activateEvaluationMetadata(
	t *testing.T,
	repository *metadata.Repository,
	syncID string,
	storyUUID string,
	title string,
	second int,
) {
	t.Helper()

	instant := time.Date(2026, time.July, 25, 10, 0, second, 0, time.UTC)
	sync, err := repository.CreateSync(context.Background(), metadata.NewCatalogSync{
		ID:        syncID,
		Locale:    metadata.DefaultLocale,
		StartedAt: instant,
	})
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := repository.StageSnapshot(
		context.Background(),
		metadata.NewCatalogSnapshot{
			SyncID:    sync.ID,
			Locale:    sync.Locale,
			RawPath:   "catalog/" + sync.ID + "/catalog.json",
			RawSHA256: strings.Repeat("d", 64),
			ByteSize:  2,
			FetchedAt: instant,
			Stories: []metadata.NewOfficialStoryMetadata{{
				StoryUUID: storyUUID,
				Title:     title,
				Language:  metadata.DefaultLocale,
			}},
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	matched, err := repository.CountMatchingStories(
		context.Background(),
		snapshot.ID,
	)
	if err != nil {
		t.Fatal(err)
	}
	if err := repository.ActivateSnapshot(
		context.Background(),
		snapshot.ID,
		matched,
		instant,
	); err != nil {
		t.Fatal(err)
	}
}

func assertCollectionCount(
	t *testing.T,
	evaluator *collectionstore.Evaluator,
	collectionID int64,
	expected int,
) {
	t.Helper()

	count, err := evaluator.Count(context.Background(), collectionID)
	if err != nil {
		t.Fatal(err)
	}
	if count.CollectionID != collectionID || count.Count != expected {
		t.Fatalf("Count(%d) = %#v, want %d", collectionID, count, expected)
	}
}

func TestCombinedCollectionPersistsDynamicUnionAcrossBrowsingCountsAndExport(t *testing.T) {
	h := newEvaluationHarness(t)
	ctx := context.Background()
	for i, title := range []string{"Moon workshop", "Forest train", "Moon forest", "Ocean"} {
		createEvaluationStory(t, h.catalog, fmt.Sprintf("123e4567-e89b-42d3-a456-%012d", i), title, string(rune('a'+i)))
	}
	combined, err := h.collections.Create(ctx, "arc + aqua", library.StoryLibraryQuery{AnyOf: []library.StoryLibraryQuery{
		{Name: "moon"}, {Name: "forest"},
	}})
	if err != nil {
		t.Fatal(err)
	}
	assertCollectionCount(t, h.evaluator, combined.ID, 3)
	page, err := h.evaluator.Evaluate(ctx, combined.ID, library.ListRequest{PageSize: 1})
	if err != nil || page.Page.TotalItems != 3 || len(page.Page.Stories) != 1 {
		t.Fatal(page, err)
	}
	reopened, err := h.collections.Open(ctx, combined.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(reopened.Query.AnyOf) != 2 {
		t.Fatal("lost union", reopened.Query)
	}
	exported, err := h.library.ExportQuery(ctx, reopened.Query.StoryLibraryQuery())
	if err != nil || len(exported) != 3 {
		t.Fatal("export union", len(exported), err)
	}
	createEvaluationStory(t, h.catalog, "123e4567-e89b-42d3-a456-000000000009", "Another forest", "e")
	assertCollectionCount(t, h.evaluator, combined.ID, 4)
	refined := reopened.Query.StoryLibraryQuery()
	refined.Name = "workshop"
	result, err := h.library.Search(ctx, refined)
	if err != nil || result.TotalItems != 1 {
		t.Fatal("refined union", result, err)
	}
}
