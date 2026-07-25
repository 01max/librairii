package shelves_test

import (
	"context"
	"database/sql"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/metadata"
	shelfstore "github.com/01max/librairii/internal/shelves"
	"github.com/01max/librairii/internal/tagging"
)

func TestEvaluatorReevaluatesImportsAndRemovalsThroughStoryLibraryQuery(
	t *testing.T,
) {
	t.Parallel()

	harness := newEvaluationHarness(t)
	shelf, err := harness.shelves.Create(
		context.Background(),
		"Moon stories",
		library.StoryLibraryQuery{Name: "moon"},
	)
	if err != nil {
		t.Fatal(err)
	}
	assertShelfCount(t, harness.evaluator, shelf.ID, 0)

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
		shelf.ID,
		library.ListRequest{Page: 1, PageSize: 12},
	)
	if err != nil {
		t.Fatal(err)
	}
	if evaluation.Shelf.ID != shelf.ID ||
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
	assertShelfCount(t, harness.evaluator, shelf.ID, 0)
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
	broken, err := tagging.SeedBuiltIns(context.Background(), harness.database)
	if err != nil {
		t.Fatal(err)
	}
	tags, err := tagging.NewService(harness.database)
	if err != nil {
		t.Fatal(err)
	}
	shelf, err := harness.shelves.Create(
		context.Background(),
		"Broken stories",
		library.StoryLibraryQuery{
			BooleanFilters: []library.BooleanFilter{{
				DefinitionID: broken.ID,
				State:        library.BooleanTrue,
			}},
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	assertShelfCount(t, harness.evaluator, shelf.ID, 0)

	if _, err := tags.SetStoryBoolean(
		context.Background(),
		story.ID,
		broken.ID,
		true,
	); err != nil {
		t.Fatal(err)
	}
	assertShelfCount(t, harness.evaluator, shelf.ID, 1)

	if _, err := tags.SetStoryBoolean(
		context.Background(),
		story.ID,
		broken.ID,
		false,
	); err != nil {
		t.Fatal(err)
	}
	assertShelfCount(t, harness.evaluator, shelf.ID, 0)
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
	shelf, err := harness.shelves.Create(
		context.Background(),
		"Official match",
		library.StoryLibraryQuery{Name: "official"},
	)
	if err != nil {
		t.Fatal(err)
	}
	assertShelfCount(t, harness.evaluator, shelf.ID, 0)

	activateEvaluationMetadata(
		t,
		harness.metadata,
		"123e4567-e89b-42d3-a456-426614174100",
		story.UUID,
		"Official Moon",
		1,
	)
	assertShelfCount(t, harness.evaluator, shelf.ID, 1)

	activateEvaluationMetadata(
		t,
		harness.metadata,
		"123e4567-e89b-42d3-a456-426614174101",
		story.UUID,
		"Renamed catalog title",
		2,
	)
	assertShelfCount(t, harness.evaluator, shelf.ID, 0)
}

func TestEvaluatorReturnsOrderedCountsAndRequiresDependencies(t *testing.T) {
	t.Parallel()

	harness := newEvaluationHarness(t)
	first, err := harness.shelves.Create(
		context.Background(),
		"All",
		library.StoryLibraryQuery{},
	)
	if err != nil {
		t.Fatal(err)
	}
	second, err := harness.shelves.Create(
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
		counts[0] != (shelfstore.ShelfCount{ShelfID: first.ID, Count: 1}) ||
		counts[1] != (shelfstore.ShelfCount{ShelfID: second.ID, Count: 0}) {
		t.Fatalf("Counts() = %#v", counts)
	}
	if _, err := harness.database.Exec(
		"UPDATE shelves SET query_version = ? WHERE id = ?",
		shelfstore.CurrentSavedLibraryQueryVersion+10,
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
		summaries[1].Validity != shelfstore.ValidityNeedsAttention ||
		summaries[1].AttentionReason != shelfstore.AttentionUnmigratableQuery ||
		summaries[1].Count != 0 {
		t.Fatalf("Summaries() = %#v", summaries)
	}
	if _, err := shelfstore.NewEvaluator(
		nil,
		harness.library,
	); err != shelfstore.ErrMissingDatabase {
		t.Fatalf("NewEvaluator(nil service) error = %v", err)
	}
	if _, err := shelfstore.NewEvaluator(
		harness.shelves,
		nil,
	); err != shelfstore.ErrMissingLibraryQuery {
		t.Fatalf("NewEvaluator(nil query) error = %v", err)
	}
}

type evaluationHarness struct {
	database  *sql.DB
	catalog   *catalog.Repository
	metadata  *metadata.Repository
	library   *library.Query
	shelves   *shelfstore.Service
	evaluator *shelfstore.Evaluator
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
	shelfService, err := shelfstore.NewService(opened.SQL())
	if err != nil {
		t.Fatal(err)
	}
	libraryQuery := library.NewQuery(opened.SQL(), provider)
	evaluator, err := shelfstore.NewEvaluator(shelfService, libraryQuery)
	if err != nil {
		t.Fatal(err)
	}
	return &evaluationHarness{
		database:  opened.SQL(),
		catalog:   catalog.NewRepository(opened.SQL()),
		metadata:  metadataRepository,
		library:   libraryQuery,
		shelves:   shelfService,
		evaluator: evaluator,
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

func assertShelfCount(
	t *testing.T,
	evaluator *shelfstore.Evaluator,
	shelfID int64,
	expected int,
) {
	t.Helper()

	count, err := evaluator.Count(context.Background(), shelfID)
	if err != nil {
		t.Fatal(err)
	}
	if count.ShelfID != shelfID || count.Count != expected {
		t.Fatalf("Count(%d) = %#v, want %d", shelfID, count, expected)
	}
}
