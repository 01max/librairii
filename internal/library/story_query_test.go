package library

import (
	"context"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/catalog"
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
