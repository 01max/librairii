// Package testsupport provides small domain fixtures shared by integration tests.
package testsupport

import (
	"context"
	"database/sql"
	"github.com/01max/librairii/internal/tagging"
)

// SeedBooleanTag creates an ordinary manual tag; it has no story-flag semantics.
func SeedBooleanTag(ctx context.Context, db *sql.DB) (tagging.Definition, error) {
	service, err := tagging.NewService(db)
	if err != nil {
		return tagging.Definition{}, err
	}
	catalog, err := service.Catalog(ctx)
	if err != nil {
		return tagging.Definition{}, err
	}
	for _, definition := range catalog.Definitions {
		if definition.Key == "fixture-boolean" {
			return definition.Definition, nil
		}
	}
	return service.CreateDefinition(ctx, tagging.CreateDefinition{Key: "fixture-boolean", Label: "Favorite", Color: "#405CF5", Kind: tagging.KindBoolean})
}
