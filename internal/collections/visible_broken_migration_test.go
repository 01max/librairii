package collections

import (
	"testing"

	"github.com/01max/librairii/internal/library"
)

func TestLegacyQueriesLoseOnlyBuiltInBrokenRestrictions(t *testing.T) {
	for _, version := range []int{1, 2, 3, 4} {
		payload := `{"brokenVisibility":"only","brokenState":"true","name":"moon","anyOf":[{"brokenState":"false"},{"name":"forest"}],"booleanFilters":[{"definitionId":9,"state":"true"}]}`
		if version < 4 {
			payload = `{"includeBroken":true,"name":"moon","anyOf":[{"booleanFilters":[{"definitionId":9,"state":"true"}]},{"name":"forest"}]}`
		}
		query, err := DecodeSavedLibraryQuery(version, payload, 9)
		if err != nil {
			t.Fatalf("v%d: %v", version, err)
		}
		if library.HasBrokenRestrictions(query.StoryLibraryQuery()) || query.Name != "moon" || len(query.AnyOf) != 2 || query.AnyOf[1].Name != "forest" {
			t.Fatalf("v%d: %#v", version, query)
		}
		if version == 4 && len(query.BooleanFilters) != 1 {
			t.Fatal("ordinary reused tag ID lost")
		}
	}
	if _, err := DecodeSavedLibraryQuery(5, `{"brokenState":"true"}`); err == nil {
		t.Fatal("new query accepted retired flag filtering")
	}
}
