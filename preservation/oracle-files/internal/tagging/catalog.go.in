package tagging

import (
	"context"
	"fmt"
)

type DefinitionWithValues struct {
	Definition
	Values []Value `json:"values"`
}

type Catalog struct {
	Definitions []DefinitionWithValues `json:"definitions"`
}

func (s *Service) Catalog(ctx context.Context) (Catalog, error) {
	definitions, err := s.ListDefinitions(ctx)
	if err != nil {
		return Catalog{}, err
	}
	catalog := Catalog{
		Definitions: make([]DefinitionWithValues, 0, len(definitions)),
	}
	for _, definition := range definitions {
		values, err := listValues(ctx, s.database, definition.ID)
		if err != nil {
			return Catalog{}, fmt.Errorf("load values for %s: %w", definition.Key, err)
		}
		if values == nil {
			values = []Value{}
		}
		catalog.Definitions = append(catalog.Definitions, DefinitionWithValues{
			Definition: definition,
			Values:     values,
		})
	}
	return catalog, nil
}
