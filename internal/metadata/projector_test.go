package metadata

import (
	"context"
	"database/sql"
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/tagging"
)

func TestCatalogProjectorCreatesReadOnlyAgeFacetAndAssignsPreciseBands(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	ctx := context.Background()
	snapshot, storyIDs := stageAgeProjectionFixture(t, repository, connection)
	projector, err := NewCatalogProjector(
		connection,
		DefaultCatalogProjectionConfig(),
	)
	if err != nil {
		t.Fatal(err)
	}
	result, err := projector.Rebuild(ctx, snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if result.MatchedStories != 4 ||
		result.AssignedStories != 2 ||
		result.Unassigned != 2 ||
		result.DefinitionID <= 0 {
		t.Fatalf("Rebuild() = %#v", result)
	}

	rows, err := connection.Query(
		`SELECT assignments.story_id, tag_value_rows.normalized_key, assignments.source
		 FROM story_tag_assignments AS assignments
		 JOIN tag_values AS tag_value_rows
		   ON tag_value_rows.id = assignments.value_id
		 WHERE assignments.definition_id = ?
		 ORDER BY assignments.story_id`,
		result.DefinitionID,
	)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	type assignment struct {
		storyID int64
		key     string
		source  string
	}
	var assignments []assignment
	for rows.Next() {
		var item assignment
		if err := rows.Scan(&item.storyID, &item.key, &item.source); err != nil {
			t.Fatal(err)
		}
		assignments = append(assignments, item)
	}
	if err := rows.Err(); err != nil {
		t.Fatal(err)
	}
	if len(assignments) != 2 ||
		assignments[0] != (assignment{storyID: storyIDs[0], key: "3-5", source: "derived"}) ||
		assignments[1] != (assignment{storyID: storyIDs[1], key: "6-8", source: "derived"}) {
		t.Fatalf("derived assignments = %#v", assignments)
	}

	tagService, err := tagging.NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	catalog, err := tagService.Catalog(ctx)
	if err != nil {
		t.Fatal(err)
	}
	var age tagging.DefinitionWithValues
	for _, definition := range catalog.Definitions {
		if definition.Key == AgeDefinitionKey {
			age = definition
		}
	}
	if age.ID != result.DefinitionID ||
		age.Source != tagging.SourceDerived ||
		age.Kind != tagging.KindChoice ||
		age.Presentation != tagging.PresentationSystem ||
		!age.Protected ||
		len(age.Values) != 2 ||
		age.Values[0].Label != "3–5 years" ||
		age.Values[1].Label != "6–8 years" {
		t.Fatalf("derived age catalog = %#v", age)
	}
	if _, err := tagService.RenameDefinition(
		ctx,
		age.ID,
		"Ages",
	); !errors.Is(err, tagging.ErrProtectedDefinition) {
		t.Fatalf("RenameDefinition(age) error = %v", err)
	}
	if _, err := tagService.SetStoryChoiceValues(
		ctx,
		storyIDs[2],
		age.ID,
		[]int64{age.Values[0].ID},
	); !errors.Is(err, tagging.ErrDerivedAssignment) {
		t.Fatalf("SetStoryChoiceValues(age) error = %v", err)
	}
}

func TestCatalogProjectorReservesConfiguredFacetBeforeFirstRefresh(t *testing.T) {
	t.Parallel()

	_, connection := openMetadataRepository(t)
	projector, err := NewCatalogProjector(
		connection,
		DefaultCatalogProjectionConfig(),
	)
	if err != nil {
		t.Fatal(err)
	}
	firstID, err := projector.EnsureFacets(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	secondID, err := projector.EnsureFacets(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if firstID <= 0 || firstID != secondID {
		t.Fatalf("EnsureFacets() ids = %d, %d", firstID, secondID)
	}
	service, err := tagging.NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.CreateDefinition(
		context.Background(),
		tagging.CreateDefinition{
			Key:   "Age",
			Label: "Personal age",
			Color: "#405CF5",
			Kind:  tagging.KindChoice,
		},
	); !errors.Is(err, tagging.ErrDuplicateDefinition) {
		t.Fatalf("CreateDefinition(age) error = %v", err)
	}
}

func TestCatalogProjectorIsIdempotentAndRejectsFacetDrift(t *testing.T) {
	t.Parallel()

	repository, connection := openMetadataRepository(t)
	snapshot, _ := stageAgeProjectionFixture(t, repository, connection)
	projector, err := NewCatalogProjector(
		connection,
		DefaultCatalogProjectionConfig(),
	)
	if err != nil {
		t.Fatal(err)
	}
	first, err := projector.Rebuild(context.Background(), snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	second, err := projector.Rebuild(context.Background(), snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if first != second {
		t.Fatalf("projection results = %#v, %#v", first, second)
	}
	var definitions int
	var values int
	var assignments int
	if err := connection.QueryRow(
		"SELECT COUNT(*) FROM tag_definitions WHERE normalized_key = ?",
		AgeDefinitionKey,
	).Scan(&definitions); err != nil {
		t.Fatal(err)
	}
	if err := connection.QueryRow(
		"SELECT COUNT(*) FROM tag_values WHERE definition_id = ?",
		first.DefinitionID,
	).Scan(&values); err != nil {
		t.Fatal(err)
	}
	if err := connection.QueryRow(
		"SELECT COUNT(*) FROM story_tag_assignments WHERE definition_id = ?",
		first.DefinitionID,
	).Scan(&assignments); err != nil {
		t.Fatal(err)
	}
	if definitions != 1 || values != 2 || assignments != 2 {
		t.Fatalf(
			"projected counts = definitions %d, values %d, assignments %d",
			definitions,
			values,
			assignments,
		)
	}
	if _, err := connection.Exec(
		"UPDATE tag_definitions SET label = 'Drifted' WHERE id = ?",
		first.DefinitionID,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := projector.Rebuild(
		context.Background(),
		snapshot.ID,
	); !errors.Is(err, ErrDerivedFacetDrift) {
		t.Fatalf("Rebuild(drifted) error = %v", err)
	}
}

func TestCatalogProjectorRejectsInvalidBandConfiguration(t *testing.T) {
	t.Parallel()

	_, connection := openMetadataRepository(t)
	for _, config := range []CatalogProjectionConfig{
		{},
		{AgeBands: []AgeBand{{Key: "bad key", Label: "Bad", Minimum: 3, Maximum: 5}}},
		{AgeBands: []AgeBand{{Key: "5-3", Label: "Bad", Minimum: 5, Maximum: 3}}},
		{AgeBands: []AgeBand{
			{Key: "3-5", Label: "First", Minimum: 3, Maximum: 5},
			{Key: "5-7", Label: "Overlap", Minimum: 5, Maximum: 7},
		}},
	} {
		if _, err := NewCatalogProjector(
			connection,
			config,
		); !errors.Is(err, ErrInvalidProjectionConfig) {
			t.Fatalf("NewCatalogProjector(%#v) error = %v", config, err)
		}
	}
}

func stageAgeProjectionFixture(
	t *testing.T,
	repository *Repository,
	connection interface {
		Exec(string, ...any) (sql.Result, error)
	},
) (CatalogSnapshot, []int64) {
	t.Helper()

	ctx := context.Background()
	syncID := "123e4567-e89b-42d3-a456-426614174120"
	if _, err := repository.CreateSync(ctx, NewCatalogSync{
		ID:        syncID,
		Locale:    "en-GB",
		StartedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	uuids := []string{
		"10000000-0000-4000-8000-000000000001",
		"10000000-0000-4000-8000-000000000002",
		"10000000-0000-4000-8000-000000000003",
		"10000000-0000-4000-8000-000000000004",
	}
	storyIDs := make([]int64, 0, len(uuids))
	for _, storyUUID := range uuids {
		result, err := connection.Exec("INSERT INTO stories (uuid) VALUES (?)", storyUUID)
		if err != nil {
			t.Fatal(err)
		}
		storyID, err := result.LastInsertId()
		if err != nil {
			t.Fatal(err)
		}
		storyIDs = append(storyIDs, storyID)
	}
	three := 3
	five := 5
	six := 6
	eight := 8
	snapshot, err := repository.StageSnapshot(ctx, NewCatalogSnapshot{
		SyncID:    syncID,
		Locale:    "en-GB",
		RawPath:   "catalog/" + syncID + "/catalog.json",
		RawSHA256: strings.Repeat("d", 64),
		ByteSize:  128,
		FetchedAt: time.Now(),
		Stories: []NewOfficialStoryMetadata{
			{StoryUUID: uuids[0], MinimumAge: &three, MaximumAge: &five},
			{StoryUUID: uuids[1], MinimumAge: &six, MaximumAge: &eight},
			{StoryUUID: uuids[2], MinimumAge: &three},
			{StoryUUID: uuids[3], MinimumAge: &five, MaximumAge: &six},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	return snapshot, storyIDs
}
