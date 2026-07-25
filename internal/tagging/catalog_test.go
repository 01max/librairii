package tagging

import (
	"context"
	"testing"
)

func TestCatalogReturnsDefinitionsAndOrderedValues(t *testing.T) {
	t.Parallel()

	connection := openTaggingDatabase(t)
	if _, err := SeedBuiltIns(context.Background(), connection); err != nil {
		t.Fatal(err)
	}
	service, err := NewService(connection)
	if err != nil {
		t.Fatal(err)
	}
	definition := createUserDefinition(t, service, "mood", KindChoice)
	first := createChoiceValue(t, service, definition.ID, "calm")
	second := createChoiceValue(t, service, definition.ID, "adventure")
	if _, err := service.ReorderValues(
		context.Background(),
		definition.ID,
		[]int64{second.ID, first.ID},
	); err != nil {
		t.Fatal(err)
	}

	catalog, err := service.Catalog(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(catalog.Definitions) != 2 ||
		catalog.Definitions[0].Key != BrokenKey ||
		len(catalog.Definitions[0].Values) != 0 ||
		catalog.Definitions[1].ID != definition.ID ||
		catalog.Definitions[1].Values[0].ID != second.ID ||
		catalog.Definitions[1].Values[1].ID != first.ID {
		t.Fatalf("Catalog() = %#v", catalog)
	}
}
