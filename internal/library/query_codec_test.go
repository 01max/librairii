package library

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

type queryCodecFixture struct {
	Name  string            `json:"name"`
	Hash  string            `json:"hash"`
	Query StoryLibraryQuery `json:"query"`
}

func TestStoryLibraryQueryCodecMatchesSharedFixtures(t *testing.T) {
	t.Parallel()

	fixtures := loadQueryCodecFixtures(t)
	for _, fixture := range fixtures {
		fixture := fixture
		t.Run(fixture.Name, func(t *testing.T) {
			t.Parallel()

			encoded, err := EncodeStoryLibraryQuery(fixture.Query)
			if err != nil {
				t.Fatalf("EncodeStoryLibraryQuery() error = %v", err)
			}
			if encoded != fixture.Hash {
				t.Fatalf("EncodeStoryLibraryQuery() = %q, want %q", encoded, fixture.Hash)
			}
			decoded, err := DecodeStoryLibraryQuery(fixture.Hash)
			if err != nil {
				t.Fatalf("DecodeStoryLibraryQuery() error = %v", err)
			}
			if !reflect.DeepEqual(decoded, fixture.Query) {
				t.Fatalf("DecodeStoryLibraryQuery() = %#v, want %#v", decoded, fixture.Query)
			}
		})
	}
}

func TestStoryLibraryQueryCodecCanonicalizesAndRejectsInvalidHashes(t *testing.T) {
	t.Parallel()

	encoded, err := EncodeStoryLibraryQuery(StoryLibraryQuery{
		Languages: []string{"fr-FR", "en_gb", "en-GB"},
		Compatibilities: []Compatibility{
			CompatibilityInvalid,
			CompatibilityCompatible,
			CompatibilityInvalid,
		},
		BooleanFilters: []BooleanFilter{
			{DefinitionID: 8, State: BooleanFalse},
			{DefinitionID: 2, State: BooleanTrue},
			{DefinitionID: 12, State: BooleanIgnored},
		},
		ChoiceFilters: []ChoiceFilter{{
			DefinitionID: 4,
			ValueIDs:     []int64{6, 5, 6},
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	if encoded !=
		"#/library?bool=2%3Atrue&bool=8%3Afalse&choice=4%3A5%2C6&compatibility=compatible&compatibility=invalid&language=en-GB&language=fr-FR&v=1" {
		t.Fatalf("EncodeStoryLibraryQuery(canonical) = %q", encoded)
	}

	for _, hash := range []string{
		"#/other?v=1",
		"#/library",
		"#/library?v=2",
		"#/library?v=1&unknown=value",
		"#/library?v=1&page=zero",
		"#/library?v=1&bool=broken",
		"#/library?v=1&choice=1%3A",
		"#/library?v=1&language=not+a+locale",
		"#/library?v=1&compatibility=unknown",
		"#/library?v=1&v=1",
	} {
		if _, err := DecodeStoryLibraryQuery(hash); !errors.Is(
			err,
			ErrInvalidStoryLibraryHash,
		) {
			t.Fatalf("DecodeStoryLibraryQuery(%q) error = %v", hash, err)
		}
	}
}

func loadQueryCodecFixtures(t *testing.T) []queryCodecFixture {
	t.Helper()

	body, err := os.ReadFile(filepath.Join("testdata", "story_library_query_codec.json"))
	if err != nil {
		t.Fatal(err)
	}
	var fixtures []queryCodecFixture
	if err := json.Unmarshal(body, &fixtures); err != nil {
		t.Fatal(err)
	}
	return fixtures
}
