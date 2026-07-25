package shelfquery

import (
	"errors"
	"reflect"
	"testing"
)

func TestDecodeReferencesSupportsCurrentAndLegacyQueries(t *testing.T) {
	t.Parallel()

	current, err := DecodeReferences(CurrentVersion, `{
		"booleanFilters":[
			{"definitionId":2,"state":"false"},
			{"definitionId":9,"state":"true"}
		],
		"choiceFilters":[{"definitionId":5,"valueIds":[3,7]}]
	}`)
	if err != nil {
		t.Fatal(err)
	}
	legacy, err := DecodeReferences(1, `{
		"name":"moon",
		"booleanFilters":[{"definitionId":8,"state":"ignored"}],
		"choiceFilters":[{"definitionId":6,"valueIds":[4]}],
		"page":7,
		"pageSize":12,
		"sort":"imported_desc"
	}`)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(current.BooleanFilters, []BooleanReference{
		{DefinitionID: 2, State: "false"},
		{DefinitionID: 9, State: "true"},
	}) ||
		!reflect.DeepEqual(current.ChoiceFilters, []ChoiceReference{{
			DefinitionID: 5,
			ValueIDs:     []int64{3, 7},
		}}) ||
		!reflect.DeepEqual(legacy.BooleanFilters, []BooleanReference{{
			DefinitionID: 8,
			State:        "ignored",
		}}) ||
		!reflect.DeepEqual(legacy.ChoiceFilters, []ChoiceReference{{
			DefinitionID: 6,
			ValueIDs:     []int64{4},
		}}) {
		t.Fatalf("current = %#v, legacy = %#v", current, legacy)
	}
}

func TestDecodeReferencesRejectsUnsafePayloads(t *testing.T) {
	t.Parallel()

	for _, test := range []struct {
		name    string
		version int
		payload string
		want    error
	}{
		{
			name:    "unsupported version",
			version: CurrentVersion + 1,
			payload: `{}`,
			want:    ErrUnsupportedVersion,
		},
		{
			name:    "current navigation field",
			version: CurrentVersion,
			payload: `{"page":2}`,
			want:    ErrInvalidPayload,
		},
		{
			name:    "invalid state",
			version: CurrentVersion,
			payload: `{"booleanFilters":[{"definitionId":1,"state":"maybe"}]}`,
			want:    ErrInvalidPayload,
		},
		{
			name:    "duplicate definition",
			version: CurrentVersion,
			payload: `{"booleanFilters":[{"definitionId":1,"state":"true"}],` +
				`"choiceFilters":[{"definitionId":1,"valueIds":[2]}]}`,
			want: ErrInvalidPayload,
		},
		{
			name:    "trailing document",
			version: CurrentVersion,
			payload: `{} {}`,
			want:    ErrInvalidPayload,
		},
	} {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			if _, err := DecodeReferences(
				test.version,
				test.payload,
			); !errors.Is(err, test.want) {
				t.Fatalf("DecodeReferences() error = %v, want %v", err, test.want)
			}
		})
	}
}
