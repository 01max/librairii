package collections

import (
	"github.com/01max/librairii/internal/library"
	"testing"
)

func TestLegacyBrokenShelfCriteriaPreserveNestedComposition(t *testing.T) {
	payload := `{"includeBroken":false,"booleanFilters":[{"definitionId":9,"state":"true"}],"anyOf":[{"booleanFilters":[{"definitionId":9,"state":"false"}]},{"name":"moon","booleanFilters":[{"definitionId":9,"state":"ignored"}]}]}`
	for _, version := range []int{1, 2, 3} {
		saved, err := DecodeSavedLibraryQuery(version, payload, 9)
		if err != nil {
			t.Fatal(err)
		}
		if saved.BrokenVisibility != "" || saved.BrokenState != library.BooleanTrue || len(saved.BooleanFilters) != 0 || saved.AnyOf[0].BrokenState != library.BooleanFalse || saved.AnyOf[1].BrokenState != "" {
			t.Fatalf("saved=%#v", saved)
		}
		encoded, err := EncodeSavedLibraryQuery(saved.StoryLibraryQuery())
		if err != nil {
			t.Fatal(err)
		}
		if encoded.Version != 4 {
			t.Fatalf("version=%d", encoded.Version)
		}
	}
}
