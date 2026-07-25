package shelves

import (
	"errors"
	"reflect"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/library"
)

func TestEncodeSavedLibraryQueryCanonicalizesMembershipAndExcludesNavigation(
	t *testing.T,
) {
	t.Parallel()

	input := library.StoryLibraryQuery{
		Name:            "  L'ÉCOLE ",
		Languages:       []string{"fr_fr", "en-GB", "fr-FR"},
		Compatibilities: []library.Compatibility{"missing", "compatible", "missing"},
		BooleanFilters: []library.BooleanFilter{
			{DefinitionID: 9, State: library.BooleanTrue},
			{DefinitionID: 7, State: library.BooleanIgnored},
			{DefinitionID: 2, State: library.BooleanFalse},
		},
		ChoiceFilters: []library.ChoiceFilter{
			{DefinitionID: 5, ValueIDs: []int64{7, 2, 7}},
			{DefinitionID: 3, ValueIDs: []int64{8, 4}},
		},
		Page:     7,
		PageSize: 12,
		Sort:     library.SortImportedNewest,
	}
	originalChoiceValues := append([]int64(nil), input.ChoiceFilters[0].ValueIDs...)
	encoded, err := EncodeSavedLibraryQuery(input)
	if err != nil {
		t.Fatal(err)
	}
	const expected = `{"name":"l'ecole","languages":["en-GB","fr-FR"],` +
		`"compatibilities":["compatible","missing"],` +
		`"booleanFilters":[{"definitionId":2,"state":"false"},` +
		`{"definitionId":9,"state":"true"}],` +
		`"choiceFilters":[{"definitionId":3,"valueIds":[4,8]},` +
		`{"definitionId":5,"valueIds":[2,7]}]}`
	if encoded.Version != CurrentSavedLibraryQueryVersion ||
		encoded.Payload != expected {
		t.Fatalf("EncodeSavedLibraryQuery() = %#v", encoded)
	}
	if !reflect.DeepEqual(input.ChoiceFilters[0].ValueIDs, originalChoiceValues) ||
		input.BooleanFilters[0].DefinitionID != 9 {
		t.Fatalf("EncodeSavedLibraryQuery() mutated input: %#v", input)
	}

	decoded, err := DecodeSavedLibraryQuery(encoded.Version, encoded.Payload)
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Name != "l'ecole" ||
		len(decoded.BooleanFilters) != 2 ||
		len(decoded.ChoiceFilters) != 2 {
		t.Fatalf("DecodeSavedLibraryQuery() = %#v", decoded)
	}
	query := decoded.StoryLibraryQuery()
	if query.Page != 0 || query.PageSize != 0 || query.Sort != "" {
		t.Fatalf("saved query retained navigation state: %#v", query)
	}
}

func TestEncodeSavedLibraryQueryRepresentsAllStoriesWithEmptyObject(t *testing.T) {
	t.Parallel()

	encoded, err := EncodeSavedLibraryQuery(library.StoryLibraryQuery{
		BooleanFilters: []library.BooleanFilter{{
			DefinitionID: 1,
			State:        library.BooleanIgnored,
		}},
		Page:     42,
		PageSize: 1,
		Sort:     library.SortImportedNewest,
	})
	if err != nil {
		t.Fatal(err)
	}
	if encoded.Payload != `{}` {
		t.Fatalf("empty saved query payload = %q", encoded.Payload)
	}
	decoded, err := DecodeSavedLibraryQuery(encoded.Version, encoded.Payload)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded, SavedLibraryQuery{}) {
		t.Fatalf("empty decoded query = %#v", decoded)
	}
}

func TestMigrateSavedLibraryQueryPreservesV1MembershipAndDropsNavigation(
	t *testing.T,
) {
	t.Parallel()

	const legacy = `{
		"name":" ÉTÉ ",
		"languages":["fr_FR","en-GB"],
		"compatibilities":["missing","compatible"],
		"booleanFilters":[
			{"definitionId":8,"state":"true"},
			{"definitionId":4,"state":"false"}
		],
		"choiceFilters":[
			{"definitionId":6,"valueIds":[9,3]}
		],
		"page":9,
		"pageSize":12,
		"sort":"imported_desc"
	}`
	migrated, err := MigrateSavedLibraryQuery(1, legacy)
	if err != nil {
		t.Fatal(err)
	}
	const expected = `{"name":"ete","languages":["en-GB","fr-FR"],` +
		`"compatibilities":["compatible","missing"],` +
		`"booleanFilters":[{"definitionId":4,"state":"false"},` +
		`{"definitionId":8,"state":"true"}],` +
		`"choiceFilters":[{"definitionId":6,"valueIds":[3,9]}]}`
	if migrated.Version != CurrentSavedLibraryQueryVersion ||
		migrated.Payload != expected {
		t.Fatalf("MigrateSavedLibraryQuery() = %#v", migrated)
	}
}

func TestSavedLibraryQueryRejectsUnsupportedCorruptAndBroadeningPayloads(
	t *testing.T,
) {
	t.Parallel()

	if _, err := DecodeSavedLibraryQuery(99, `{}`); !errors.Is(
		err,
		ErrUnsupportedSavedQueryVersion,
	) {
		t.Fatalf("DecodeSavedLibraryQuery(unknown version) error = %v", err)
	}
	for _, test := range []struct {
		name    string
		version int
		payload string
	}{
		{name: "null", version: 2, payload: `null`},
		{name: "malformed", version: 2, payload: `{"name":`},
		{name: "trailing JSON", version: 2, payload: `{} {}`},
		{name: "unknown current field", version: 2, payload: `{"page":2}`},
		{name: "unknown legacy field", version: 1, payload: `{"selection":[1]}`},
		{
			name:    "invalid filter",
			version: 2,
			payload: `{"booleanFilters":[{"definitionId":1,"state":"sometimes"}]}`,
		},
		{
			name:    "duplicate definition",
			version: 2,
			payload: `{"booleanFilters":[{"definitionId":1,"state":"true"}],` +
				`"choiceFilters":[{"definitionId":1,"valueIds":[2]}]}`,
		},
		{
			name:    "oversized",
			version: 2,
			payload: `{"name":"` + strings.Repeat("x", maxSavedQueryPayloadBytes) + `"}`,
		},
	} {
		test := test
		t.Run(test.name, func(t *testing.T) {
			if _, err := DecodeSavedLibraryQuery(
				test.version,
				test.payload,
			); !errors.Is(err, ErrInvalidSavedLibraryQuery) {
				t.Fatalf("DecodeSavedLibraryQuery(%s) error = %v", test.name, err)
			}
		})
	}
}

func TestSavedLibraryQueryConversionDoesNotAliasCallerSlices(t *testing.T) {
	t.Parallel()

	saved := SavedLibraryQuery{
		Languages: []string{"en-GB"},
		ChoiceFilters: []library.ChoiceFilter{{
			DefinitionID: 2,
			ValueIDs:     []int64{3},
		}},
	}
	query := saved.StoryLibraryQuery()
	query.Languages[0] = "fr-FR"
	query.ChoiceFilters[0].ValueIDs[0] = 99
	if saved.Languages[0] != "en-GB" ||
		saved.ChoiceFilters[0].ValueIDs[0] != 3 {
		t.Fatalf("StoryLibraryQuery() aliased saved data: %#v", saved)
	}
}
