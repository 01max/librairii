package shelves

import (
	"errors"
	"fmt"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/shelfquery"
)

const (
	CurrentSavedLibraryQueryVersion = shelfquery.CurrentVersion
	maxSavedQueryPayloadBytes       = shelfquery.MaxPayloadBytes
)

var (
	ErrInvalidSavedLibraryQuery     = errors.New("saved library query is invalid")
	ErrUnsupportedSavedQueryVersion = errors.New("saved library query version is unsupported")
)

type SavedLibraryQuery struct {
	Name            string                  `json:"name,omitempty"`
	Languages       []string                `json:"languages,omitempty"`
	Compatibilities []library.Compatibility `json:"compatibilities,omitempty"`
	BooleanFilters  []library.BooleanFilter `json:"booleanFilters,omitempty"`
	ChoiceFilters   []library.ChoiceFilter  `json:"choiceFilters,omitempty"`
}

type SerializedQuery struct {
	Version int
	Payload string
}

func EncodeSavedLibraryQuery(
	query library.StoryLibraryQuery,
) (SerializedQuery, error) {
	saved, err := canonicalSavedLibraryQuery(query)
	if err != nil {
		return SerializedQuery{}, err
	}
	payload, err := shelfquery.EncodePayload(savedLibraryQueryPayload(saved))
	if err != nil {
		return SerializedQuery{}, fmt.Errorf(
			"%w: encode canonical query: %v",
			ErrInvalidSavedLibraryQuery,
			err,
		)
	}
	return SerializedQuery{
		Version: CurrentSavedLibraryQueryVersion,
		Payload: payload,
	}, nil
}

func DecodeSavedLibraryQuery(
	version int,
	payload string,
) (SavedLibraryQuery, error) {
	decoded, err := shelfquery.DecodePayload(version, payload)
	if errors.Is(err, shelfquery.ErrUnsupportedVersion) {
		return SavedLibraryQuery{}, ErrUnsupportedSavedQueryVersion
	}
	if err != nil {
		return SavedLibraryQuery{}, fmt.Errorf(
			"%w: %v",
			ErrInvalidSavedLibraryQuery,
			err,
		)
	}
	return canonicalSavedLibraryQuery(storyLibraryQueryFromPayload(decoded))
}

func MigrateSavedLibraryQuery(
	version int,
	payload string,
) (SerializedQuery, error) {
	saved, err := DecodeSavedLibraryQuery(version, payload)
	if err != nil {
		return SerializedQuery{}, err
	}
	return EncodeSavedLibraryQuery(saved.StoryLibraryQuery())
}

func (q SavedLibraryQuery) StoryLibraryQuery() library.StoryLibraryQuery {
	return library.StoryLibraryQuery{
		Name:            q.Name,
		Languages:       append([]string(nil), q.Languages...),
		Compatibilities: append([]library.Compatibility(nil), q.Compatibilities...),
		BooleanFilters:  append([]library.BooleanFilter(nil), q.BooleanFilters...),
		ChoiceFilters:   cloneChoiceFilters(q.ChoiceFilters),
	}
}

func canonicalSavedLibraryQuery(
	query library.StoryLibraryQuery,
) (SavedLibraryQuery, error) {
	query, err := library.CanonicalStoryLibraryMembershipQuery(query)
	if err != nil {
		return SavedLibraryQuery{}, fmt.Errorf(
			"%w: %v",
			ErrInvalidSavedLibraryQuery,
			err,
		)
	}
	return SavedLibraryQuery{
		Name:            query.Name,
		Languages:       append([]string(nil), query.Languages...),
		Compatibilities: append([]library.Compatibility(nil), query.Compatibilities...),
		BooleanFilters:  append([]library.BooleanFilter(nil), query.BooleanFilters...),
		ChoiceFilters:   cloneChoiceFilters(query.ChoiceFilters),
	}, nil
}

func cloneChoiceFilters(filters []library.ChoiceFilter) []library.ChoiceFilter {
	if len(filters) == 0 {
		return nil
	}
	cloned := make([]library.ChoiceFilter, 0, len(filters))
	for _, filter := range filters {
		cloned = append(cloned, library.ChoiceFilter{
			DefinitionID: filter.DefinitionID,
			ValueIDs:     append([]int64(nil), filter.ValueIDs...),
		})
	}
	return cloned
}

func savedLibraryQueryPayload(query SavedLibraryQuery) shelfquery.Payload {
	booleanFilters := make(
		[]shelfquery.BooleanReference,
		0,
		len(query.BooleanFilters),
	)
	for _, filter := range query.BooleanFilters {
		booleanFilters = append(booleanFilters, shelfquery.BooleanReference{
			DefinitionID: filter.DefinitionID,
			State:        string(filter.State),
		})
	}
	choiceFilters := make(
		[]shelfquery.ChoiceReference,
		0,
		len(query.ChoiceFilters),
	)
	for _, filter := range query.ChoiceFilters {
		choiceFilters = append(choiceFilters, shelfquery.ChoiceReference{
			DefinitionID: filter.DefinitionID,
			ValueIDs:     append([]int64(nil), filter.ValueIDs...),
		})
	}
	compatibilities := make([]string, 0, len(query.Compatibilities))
	for _, compatibility := range query.Compatibilities {
		compatibilities = append(compatibilities, string(compatibility))
	}
	return shelfquery.Payload{
		Name:            query.Name,
		Languages:       append([]string(nil), query.Languages...),
		Compatibilities: compatibilities,
		BooleanFilters:  booleanFilters,
		ChoiceFilters:   choiceFilters,
	}
}

func storyLibraryQueryFromPayload(payload shelfquery.Payload) library.StoryLibraryQuery {
	compatibilities := make(
		[]library.Compatibility,
		0,
		len(payload.Compatibilities),
	)
	for _, compatibility := range payload.Compatibilities {
		compatibilities = append(
			compatibilities,
			library.Compatibility(compatibility),
		)
	}
	booleanFilters := make(
		[]library.BooleanFilter,
		0,
		len(payload.BooleanFilters),
	)
	for _, filter := range payload.BooleanFilters {
		booleanFilters = append(booleanFilters, library.BooleanFilter{
			DefinitionID: filter.DefinitionID,
			State:        library.BooleanFilterState(filter.State),
		})
	}
	choiceFilters := make(
		[]library.ChoiceFilter,
		0,
		len(payload.ChoiceFilters),
	)
	for _, filter := range payload.ChoiceFilters {
		choiceFilters = append(choiceFilters, library.ChoiceFilter{
			DefinitionID: filter.DefinitionID,
			ValueIDs:     append([]int64(nil), filter.ValueIDs...),
		})
	}
	return library.StoryLibraryQuery{
		Name:            payload.Name,
		Languages:       append([]string(nil), payload.Languages...),
		Compatibilities: compatibilities,
		BooleanFilters:  booleanFilters,
		ChoiceFilters:   choiceFilters,
	}
}
