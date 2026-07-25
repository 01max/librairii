package library

import (
	"context"
	"errors"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/tagging"
)

func TestStoryLibraryQueryPaginatesSortsAndCountsInSQLite(t *testing.T) {
	t.Parallel()

	query, repository, _ := newLibraryQuery(t, nil)
	for index, title := range []string{
		"Story Echo",
		"Story Alpha",
		"Story Delta",
		"Story Bravo",
		"Story Charlie",
	} {
		createQueryableStory(
			t,
			repository,
			[]string{
				"00112233-4455-4677-8899-aabbccddeeff",
				"11112222-3333-4444-8555-666677778888",
				"22223333-4444-4555-8666-777788889999",
				"33334444-5555-4666-8777-888899990000",
				"44445555-6666-4777-8888-999900001111",
			}[index],
			title,
			string(rune('a'+index)),
		)
	}

	page, err := query.Search(context.Background(), StoryLibraryQuery{
		Name:     "story",
		Page:     2,
		PageSize: 2,
		Sort:     SortNameAscending,
	})
	if err != nil {
		t.Fatal(err)
	}
	if page.TotalItems != 5 ||
		page.TotalPages != 3 ||
		page.Page != 2 ||
		len(page.Stories) != 2 ||
		page.Stories[0].Title != "Story Charlie" ||
		page.Stories[1].Title != "Story Delta" {
		t.Fatalf("Search(name page) = %#v", page)
	}
	count, err := query.Count(context.Background(), StoryLibraryQuery{
		Name: "story",
	})
	if err != nil {
		t.Fatal(err)
	}
	if count != page.TotalItems {
		t.Fatalf("Count(name) = %d, want %d", count, page.TotalItems)
	}

	newest, err := query.Search(context.Background(), StoryLibraryQuery{
		Page:     1,
		PageSize: 2,
		Sort:     SortImportedNewest,
	})
	if err != nil {
		t.Fatal(err)
	}
	if newest.TotalItems != 5 ||
		len(newest.Stories) != 2 ||
		newest.Stories[0].ID <= newest.Stories[1].ID {
		t.Fatalf("Search(newest) = %#v", newest)
	}

	beyond, err := query.Search(context.Background(), StoryLibraryQuery{
		Page:     5,
		PageSize: 2,
	})
	if err != nil {
		t.Fatal(err)
	}
	if beyond.TotalItems != 5 || beyond.TotalPages != 3 || len(beyond.Stories) != 0 {
		t.Fatalf("Search(beyond) = %#v", beyond)
	}
}

func TestStoryLibraryQueryRejectsInvalidBoundedInputsAndSQLText(t *testing.T) {
	t.Parallel()

	query, repository, database := newLibraryQuery(t, nil)
	createQueryableStory(
		t,
		repository,
		"00112233-4455-4677-8899-aabbccddeeff",
		"Dragon",
		"a",
	)
	broken, err := tagging.SeedBuiltIns(context.Background(), database)
	if err != nil {
		t.Fatal(err)
	}
	tags, err := tagging.NewService(database)
	if err != nil {
		t.Fatal(err)
	}
	choice := createQueryDefinition(t, tags, "mood")
	value := createQueryValue(t, tags, choice.ID, "calm")

	tooManyFilters := make([]BooleanFilter, maxFilterGroups+1)
	for index := range tooManyFilters {
		tooManyFilters[index] = BooleanFilter{
			DefinitionID: int64(index + 1),
			State:        BooleanTrue,
		}
	}
	for _, request := range []StoryLibraryQuery{
		{Page: -1},
		{PageSize: MaxPageSize + 1},
		{Sort: "unknown"},
		{Name: strings.Repeat("x", maxNameSearchRunes+1)},
		{Languages: []string{"not a locale"}},
		{Compatibilities: []Compatibility{"unknown"}},
		{BooleanFilters: []BooleanFilter{{
			DefinitionID: broken.ID,
			State:        "sometimes",
		}}},
		{BooleanFilters: tooManyFilters},
		{ChoiceFilters: []ChoiceFilter{{
			DefinitionID: choice.ID,
		}}},
		{ChoiceFilters: []ChoiceFilter{{
			DefinitionID: choice.ID,
			ValueIDs:     []int64{999_999},
		}}},
		{BooleanFilters: []BooleanFilter{{
			DefinitionID: choice.ID,
			State:        BooleanTrue,
		}}},
		{BooleanFilters: []BooleanFilter{{
			DefinitionID: broken.ID,
			State:        BooleanTrue,
		}}, ChoiceFilters: []ChoiceFilter{{
			DefinitionID: broken.ID,
			ValueIDs:     []int64{value.ID},
		}}},
	} {
		if _, err := query.Search(
			context.Background(),
			request,
		); !errors.Is(err, ErrInvalidStoryLibraryQuery) {
			t.Fatalf("Search(%#v) error = %v", request, err)
		}
	}

	injection, err := query.Search(context.Background(), StoryLibraryQuery{
		Name: "dragon') OR 1=1 --",
	})
	if err != nil {
		t.Fatalf("Search(SQL text) error = %v", err)
	}
	if injection.TotalItems != 0 {
		t.Fatalf("Search(SQL text) = %#v", injection)
	}
}

func TestStoryLibraryQueryPlanUsesAssignmentIndexes(t *testing.T) {
	t.Parallel()

	query, repository, database := newLibraryQuery(t, nil)
	createQueryableStory(
		t,
		repository,
		"00112233-4455-4677-8899-aabbccddeeff",
		"Dragon",
		"a",
	)
	broken, err := tagging.SeedBuiltIns(context.Background(), database)
	if err != nil {
		t.Fatal(err)
	}
	tags, err := tagging.NewService(database)
	if err != nil {
		t.Fatal(err)
	}
	choice := createQueryDefinition(t, tags, "mood")
	value := createQueryValue(t, tags, choice.ID, "calm")
	request, err := normalizeStoryLibraryQuery(StoryLibraryQuery{
		BooleanFilters: []BooleanFilter{{
			DefinitionID: broken.ID,
			State:        BooleanFalse,
		}},
		ChoiceFilters: []ChoiceFilter{{
			DefinitionID: choice.ID,
			ValueIDs:     []int64{value.ID},
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	predicate, arguments := storyLibraryPredicate(request, "en-GB")
	rows, err := database.QueryContext(
		context.Background(),
		`EXPLAIN QUERY PLAN
		 SELECT s.id
		 FROM stories s
		 JOIN story_archives a ON a.story_id = s.id
		 WHERE `+predicate,
		arguments...,
	)
	if err != nil {
		t.Fatal(err)
	}
	var details []string
	for rows.Next() {
		var id int
		var parent int
		var unused int
		var detail string
		if err := rows.Scan(&id, &parent, &unused, &detail); err != nil {
			t.Fatal(err)
		}
		details = append(details, detail)
	}
	if err := rows.Close(); err != nil {
		t.Fatal(err)
	}
	plan := strings.Join(details, "\n")
	if !(strings.Contains(plan, "idx_story_tag_boolean_assignment") ||
		strings.Contains(plan, "idx_story_tag_assignments_filter")) ||
		!strings.Contains(plan, "idx_story_tag_choice_assignment") {
		t.Fatalf("query plan did not use assignment indexes:\n%s", plan)
	}
	_ = query
}
