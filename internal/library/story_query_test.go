package library

import (
	"context"
	"database/sql"
	"strings"
	"testing"
	"time"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/searchtext"
	"github.com/01max/librairii/internal/tagging"
)

func TestStoryLibraryQueryNormalizesLiteralNameSearch(t *testing.T) {
	t.Parallel()

	query, repository, database := newLibraryQuery(t, nil)
	createQueryableStory(
		t,
		repository,
		"00112233-4455-4677-8899-aabbccddeeff",
		"Le Dragon des montagnes",
		"a",
	)
	createQueryableStory(
		t,
		repository,
		"11112222-3333-4444-8555-666677778888",
		"L'Été Magique",
		"b",
	)
	createQueryableStory(
		t,
		repository,
		"22223333-4444-4555-8666-777788889999",
		"100%_Fun",
		"c",
	)

	for _, testCase := range []struct {
		name  string
		count int
		title string
	}{
		{name: "DRAGON", count: 1, title: "Le Dragon des montagnes"},
		{name: "ete", count: 1, title: "L'Été Magique"},
		{name: "%_", count: 1, title: "100%_Fun"},
		{name: "missing", count: 0},
	} {
		page, err := query.Search(context.Background(), StoryLibraryQuery{
			Name: testCase.name,
		})
		if err != nil {
			t.Fatalf("Search(%q) error = %v", testCase.name, err)
		}
		if page.TotalItems != testCase.count || len(page.Stories) != testCase.count {
			t.Fatalf("Search(%q) = %#v", testCase.name, page)
		}
		if testCase.count == 1 && page.Stories[0].Title != testCase.title {
			t.Fatalf("Search(%q) story = %#v", testCase.name, page.Stories[0])
		}
	}

	var normalized []string
	rows, err := database.Query(
		"SELECT display_name_normalized FROM stories ORDER BY id",
	)
	if err != nil {
		t.Fatal(err)
	}
	for rows.Next() {
		var value string
		if err := rows.Scan(&value); err != nil {
			t.Fatal(err)
		}
		normalized = append(normalized, value)
	}
	if err := rows.Close(); err != nil {
		t.Fatal(err)
	}
	if strings.Join(normalized, "|") !=
		"le dragon des montagnes|l'ete magique|100%_fun" {
		t.Fatalf("normalized display names = %#v", normalized)
	}
}

func TestExportQueryFreezesTheCompleteResultWithoutPagination(t *testing.T) {
	t.Parallel()

	query, repository, _ := newLibraryQuery(t, nil)
	first := createQueryableStory(
		t,
		repository,
		"00112233-4455-4677-8899-aabbccddeeff",
		"Alpha",
		"a",
	)
	second := createQueryableStory(
		t,
		repository,
		"11112222-3333-4444-8555-666677778888",
		"Beta",
		"b",
	)
	third := createQueryableStory(
		t,
		repository,
		"22223333-4444-4555-8666-777788889999",
		"Gamma",
		"c",
	)

	stories, err := query.ExportQuery(context.Background(), StoryLibraryQuery{
		Page:     2,
		PageSize: 1,
		Sort:     SortNameAscending,
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(stories) != 3 ||
		stories[0].ID != first.ID ||
		stories[1].ID != second.ID ||
		stories[2].ID != third.ID {
		t.Fatalf("ExportQuery() = %#v", stories)
	}
}

func TestStoryLibraryQueryComposesBooleanAndChoiceGroups(t *testing.T) {
	t.Parallel()

	query, repository, database := newLibraryQuery(t, nil)
	first := createQueryableStory(
		t,
		repository,
		"00112233-4455-4677-8899-aabbccddeeff",
		"Dragon",
		"a",
	)
	second := createQueryableStory(
		t,
		repository,
		"11112222-3333-4444-8555-666677778888",
		"Summer",
		"b",
	)
	third := createQueryableStory(
		t,
		repository,
		"22223333-4444-4555-8666-777788889999",
		"Forest",
		"c",
	)
	broken, err := tagging.SeedBuiltIns(context.Background(), database)
	if err != nil {
		t.Fatal(err)
	}
	tags, err := tagging.NewService(database)
	if err != nil {
		t.Fatal(err)
	}
	mood := createQueryDefinition(t, tags, "mood")
	calm := createQueryValue(t, tags, mood.ID, "calm")
	adventure := createQueryValue(t, tags, mood.ID, "adventure")
	theme := createQueryDefinition(t, tags, "theme")
	night := createQueryValue(t, tags, theme.ID, "night")
	day := createQueryValue(t, tags, theme.ID, "day")

	if _, err := tags.SetStoryBoolean(
		context.Background(),
		first.ID,
		broken.ID,
		true,
	); err != nil {
		t.Fatal(err)
	}
	setQueryChoices(t, tags, first.ID, mood.ID, calm.ID)
	setQueryChoices(t, tags, first.ID, theme.ID, night.ID)
	setQueryChoices(t, tags, second.ID, mood.ID, adventure.ID)
	setQueryChoices(t, tags, second.ID, theme.ID, night.ID)
	setQueryChoices(t, tags, third.ID, mood.ID, calm.ID)
	setQueryChoices(t, tags, third.ID, theme.ID, day.ID)

	assertQueryStoryIDs(
		t,
		query,
		StoryLibraryQuery{BooleanFilters: []BooleanFilter{{
			DefinitionID: broken.ID,
			State:        BooleanTrue,
		}}},
		first.ID,
	)
	assertQueryStoryIDs(
		t,
		query,
		StoryLibraryQuery{BooleanFilters: []BooleanFilter{{
			DefinitionID: broken.ID,
			State:        BooleanFalse,
		}}},
		third.ID,
		second.ID,
	)
	assertQueryStoryIDs(
		t,
		query,
		StoryLibraryQuery{
			ChoiceFilters: []ChoiceFilter{
				{DefinitionID: mood.ID, ValueIDs: []int64{calm.ID, adventure.ID}},
				{DefinitionID: theme.ID, ValueIDs: []int64{night.ID}},
			},
		},
		first.ID,
		second.ID,
	)
	assertQueryStoryIDs(
		t,
		query,
		StoryLibraryQuery{
			BooleanFilters: []BooleanFilter{{
				DefinitionID: broken.ID,
				State:        BooleanFalse,
			}},
			ChoiceFilters: []ChoiceFilter{
				{DefinitionID: mood.ID, ValueIDs: []int64{calm.ID, adventure.ID}},
				{DefinitionID: theme.ID, ValueIDs: []int64{night.ID}},
			},
		},
		second.ID,
	)
	assertQueryStoryIDs(
		t,
		query,
		StoryLibraryQuery{BooleanFilters: []BooleanFilter{{
			DefinitionID: broken.ID,
			State:        BooleanIgnored,
		}}},
		first.ID,
		third.ID,
		second.ID,
	)
}

func TestStoryLibraryQueryComposesOfficialLanguageArchiveAndDerivedFacets(t *testing.T) {
	t.Parallel()

	official := fixedOfficialProvider{
		"00112233-4455-4677-8899-aabbccddeeff": {
			UUID:  "00112233-4455-4677-8899-aabbccddeeff",
			Title: "L'École des dragons",
		},
		"11112222-3333-4444-8555-666677778888": {
			UUID:  "11112222-3333-4444-8555-666677778888",
			Title: "Night train",
		},
	}
	query, repository, database := newLibraryQuery(t, official)
	first := createQueryableStory(
		t,
		repository,
		"00112233-4455-4677-8899-aabbccddeeff",
		"Embedded first",
		"a",
	)
	second := createQueryableStory(
		t,
		repository,
		"11112222-3333-4444-8555-666677778888",
		"Embedded second",
		"b",
	)
	third := createQueryableStory(
		t,
		repository,
		"22223333-4444-4555-8666-777788889999",
		"Unmatched",
		"c",
	)
	seedOfficialQuerySnapshot(t, database, map[string]string{
		first.UUID:  official[first.UUID].Title,
		second.UUID: official[second.UUID].Title,
	})
	if _, err := database.Exec(
		"UPDATE story_archives SET validation_state = 'missing' WHERE story_id = ?",
		second.ID,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := database.Exec(
		"UPDATE story_archives SET validation_state = 'invalid' WHERE story_id = ?",
		third.ID,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := database.Exec(
		`INSERT INTO tag_definitions (
			key, normalized_key, label, color, kind, source,
			presentation, position, is_protected
		 ) VALUES ('age', 'age', 'Age', '#FF705C', 'choice', 'derived', 'system', 0, 1)`,
	); err != nil {
		t.Fatal(err)
	}
	result, err := database.Exec(
		`INSERT INTO tag_values (
			definition_id, key, normalized_key, label, position
		 ) SELECT id, '3-5', '3-5', '3–5 years', 0
		   FROM tag_definitions WHERE normalized_key = 'age'`,
	)
	if err != nil {
		t.Fatal(err)
	}
	ageValueID, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	var ageDefinitionID int64
	if err := database.QueryRow(
		"SELECT id FROM tag_definitions WHERE normalized_key = 'age'",
	).Scan(&ageDefinitionID); err != nil {
		t.Fatal(err)
	}
	if _, err := database.Exec(
		`INSERT INTO story_tag_assignments (
			story_id, definition_id, value_id, source
		 ) VALUES (?, ?, ?, 'derived')`,
		first.ID,
		ageDefinitionID,
		ageValueID,
	); err != nil {
		t.Fatal(err)
	}

	assertQueryStoryIDs(
		t,
		query,
		StoryLibraryQuery{Name: "ecole"},
		first.ID,
	)
	assertQueryStoryIDs(
		t,
		query,
		StoryLibraryQuery{
			Languages:       []string{"en_gb", "en-GB"},
			Compatibilities: []Compatibility{CompatibilityCompatible},
			ChoiceFilters: []ChoiceFilter{{
				DefinitionID: ageDefinitionID,
				ValueIDs:     []int64{ageValueID},
			}},
		},
		first.ID,
	)
	assertQueryStoryIDs(
		t,
		query,
		StoryLibraryQuery{
			Languages: []string{"en-GB"},
			Compatibilities: []Compatibility{
				CompatibilityCompatible,
				CompatibilityMissing,
			},
		},
		first.ID,
		second.ID,
	)
}

func TestStoryLibraryQuerySortsAndPagesByResolvedOfficialName(t *testing.T) {
	t.Parallel()

	official := fixedOfficialProvider{
		"00112233-4455-4677-8899-aabbccddeeff": {
			UUID:  "00112233-4455-4677-8899-aabbccddeeff",
			Title: "Zulu official",
		},
		"11112222-3333-4444-8555-666677778888": {
			UUID:  "11112222-3333-4444-8555-666677778888",
			Title: "Alpha official",
		},
	}
	query, repository, database := newLibraryQuery(t, official)
	first := createQueryableStory(
		t,
		repository,
		"00112233-4455-4677-8899-aabbccddeeff",
		"Alpha embedded",
		"a",
	)
	second := createQueryableStory(
		t,
		repository,
		"11112222-3333-4444-8555-666677778888",
		"Zulu embedded",
		"b",
	)
	seedOfficialQuerySnapshot(t, database, map[string]string{
		first.UUID:  official[first.UUID].Title,
		second.UUID: official[second.UUID].Title,
	})
	seedOfficialQuerySnapshotForLocale(
		t,
		database,
		"123e4567-e89b-42d3-a456-426614174001",
		"fr-FR",
		map[string]string{
			first.UUID:  "Alpha français",
			second.UUID: "Zulu français",
		},
	)

	firstPage, err := query.Search(context.Background(), StoryLibraryQuery{
		Page:     1,
		PageSize: 1,
		Sort:     SortNameAscending,
	})
	if err != nil {
		t.Fatal(err)
	}
	secondPage, err := query.Search(context.Background(), StoryLibraryQuery{
		Page:     2,
		PageSize: 1,
		Sort:     SortNameAscending,
	})
	if err != nil {
		t.Fatal(err)
	}
	if firstPage.TotalPages != 2 ||
		len(firstPage.Stories) != 1 ||
		firstPage.Stories[0].ID != second.ID ||
		firstPage.Stories[0].Title != "Alpha official" {
		t.Fatalf("first page = %#v", firstPage)
	}
	if len(secondPage.Stories) != 1 ||
		secondPage.Stories[0].ID != first.ID ||
		secondPage.Stories[0].Title != "Zulu official" {
		t.Fatalf("second page = %#v", secondPage)
	}
	searchPage, err := query.Search(context.Background(), StoryLibraryQuery{
		Name: "alpha",
		Sort: SortNameAscending,
	})
	if err != nil {
		t.Fatal(err)
	}
	if searchPage.TotalItems != 1 ||
		len(searchPage.Stories) != 1 ||
		searchPage.Stories[0].ID != second.ID ||
		searchPage.Stories[0].Title != "Alpha official" {
		t.Fatalf("English display-locale search = %#v", searchPage)
	}
}

func TestBackfillNormalizedDisplayNamesRepairsExistingRows(t *testing.T) {
	t.Parallel()

	query, repository, database := newLibraryQuery(t, nil)
	story := createQueryableStory(
		t,
		repository,
		"00112233-4455-4677-8899-aabbccddeeff",
		"Original",
		"a",
	)
	if _, err := database.Exec(
		`UPDATE stories
		 SET embedded_title = 'ÉCOLE des Rêves',
		     display_name_normalized = 'stale'
		 WHERE id = ?`,
		story.ID,
	); err != nil {
		t.Fatal(err)
	}
	if err := BackfillNormalizedDisplayNames(context.Background(), database); err != nil {
		t.Fatal(err)
	}
	page, err := query.Search(context.Background(), StoryLibraryQuery{Name: "ecole"})
	if err != nil {
		t.Fatal(err)
	}
	if page.TotalItems != 1 || page.Stories[0].Title != "ÉCOLE des Rêves" {
		t.Fatalf("Search(after backfill) = %#v", page)
	}
}

func createQueryableStory(
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

func seedOfficialQuerySnapshot(
	t *testing.T,
	database *sql.DB,
	stories map[string]string,
) {
	t.Helper()

	seedOfficialQuerySnapshotForLocale(
		t,
		database,
		"123e4567-e89b-42d3-a456-426614174000",
		"en-GB",
		stories,
	)
}

func seedOfficialQuerySnapshotForLocale(
	t *testing.T,
	database *sql.DB,
	syncID string,
	locale string,
	stories map[string]string,
) {
	t.Helper()

	now := time.Date(2026, time.July, 25, 16, 0, 0, 0, time.UTC).
		Format(time.RFC3339Nano)
	if _, err := database.Exec(
		`INSERT INTO catalog_syncs (id, locale, started_at)
		 VALUES (?, ?, ?)`,
		syncID,
		locale,
		now,
	); err != nil {
		t.Fatal(err)
	}
	result, err := database.Exec(
		`INSERT INTO catalog_snapshots (
			sync_id, locale, raw_path, raw_sha256, byte_size,
			record_count, fetched_at
		 ) VALUES (?, ?, ?, ?, 2, ?, ?)`,
		syncID,
		locale,
		"catalog/"+syncID+"/catalog.json",
		strings.Repeat("d", 64),
		len(stories),
		now,
	)
	if err != nil {
		t.Fatal(err)
	}
	snapshotID, err := result.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	for storyUUID, title := range stories {
		if _, err := database.Exec(
			`INSERT INTO official_story_metadata (
				snapshot_id, story_uuid, locale, title, title_normalized,
				language, provenance
			 ) VALUES (?, ?, ?, ?, ?, ?, 'lunii_catalog')`,
			snapshotID,
			storyUUID,
			locale,
			title,
			searchtext.Normalize(title),
			locale,
		); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := database.Exec(
		`UPDATE catalog_snapshots
		 SET status = 'active', activated_at = ?
		 WHERE id = ?`,
		now,
		snapshotID,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := database.Exec(
		`UPDATE catalog_syncs
		 SET status = 'succeeded',
		     matched_story_count = ?,
		     finished_at = ?
		 WHERE id = ?`,
		len(stories),
		now,
		syncID,
	); err != nil {
		t.Fatal(err)
	}
}

func createQueryDefinition(
	t *testing.T,
	service *tagging.Service,
	key string,
) tagging.Definition {
	t.Helper()

	definition, err := service.CreateDefinition(context.Background(), tagging.CreateDefinition{
		Key:   key,
		Label: key,
		Color: "#405CF5",
		Kind:  tagging.KindChoice,
	})
	if err != nil {
		t.Fatal(err)
	}
	return definition
}

func createQueryValue(
	t *testing.T,
	service *tagging.Service,
	definitionID int64,
	key string,
) tagging.Value {
	t.Helper()

	value, err := service.CreateValue(context.Background(), tagging.CreateValue{
		DefinitionID: definitionID,
		Key:          key,
		Label:        key,
	})
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func setQueryChoices(
	t *testing.T,
	service *tagging.Service,
	storyID int64,
	definitionID int64,
	valueIDs ...int64,
) {
	t.Helper()

	if _, err := service.SetStoryChoiceValues(
		context.Background(),
		storyID,
		definitionID,
		valueIDs,
	); err != nil {
		t.Fatal(err)
	}
}

func assertQueryStoryIDs(
	t *testing.T,
	query *Query,
	request StoryLibraryQuery,
	expected ...int64,
) {
	t.Helper()

	page, err := query.Search(context.Background(), request)
	if err != nil {
		t.Fatal(err)
	}
	got := make([]int64, 0, len(page.Stories))
	for _, story := range page.Stories {
		got = append(got, story.ID)
	}
	if len(got) != len(expected) {
		t.Fatalf("Search(%#v) ids = %#v, want %#v", request, got, expected)
	}
	for index := range expected {
		if got[index] != expected[index] {
			t.Fatalf("Search(%#v) ids = %#v, want %#v", request, got, expected)
		}
	}
}
