package collections

import (
	"errors"
	"fmt"

	"github.com/01max/librairii/internal/collectionquery"
	"github.com/01max/librairii/internal/library"
)

const (
	CurrentSavedLibraryQueryVersion = collectionquery.CurrentVersion
	maxSavedQueryPayloadBytes       = collectionquery.MaxPayloadBytes
)

var (
	ErrInvalidSavedLibraryQuery     = errors.New("saved library query is invalid")
	ErrUnsupportedSavedQueryVersion = errors.New("saved library query version is unsupported")
)

type SavedLibraryQuery struct {
	BrokenVisibility library.BrokenVisibility   `json:"brokenVisibility,omitempty"`
	BrokenState      library.BooleanFilterState `json:"brokenState,omitempty"`
	AnyOf            []SavedLibraryQuery        `json:"anyOf,omitempty"`
	IncludeBroken    bool                       `json:"includeBroken,omitempty"`
	Name             string                     `json:"name,omitempty"`
	Languages        []string                   `json:"languages,omitempty"`
	Compatibilities  []library.Compatibility    `json:"compatibilities,omitempty"`
	BooleanFilters   []library.BooleanFilter    `json:"booleanFilters,omitempty"`
	ChoiceFilters    []library.ChoiceFilter     `json:"choiceFilters,omitempty"`
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
	payload, err := collectionquery.EncodePayload(savedLibraryQueryPayload(saved))
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
	legacyBrokenID ...int64,
) (SavedLibraryQuery, error) {
	decoded, err := collectionquery.DecodePayload(version, payload)
	if errors.Is(err, collectionquery.ErrUnsupportedVersion) {
		return SavedLibraryQuery{}, ErrUnsupportedSavedQueryVersion
	}
	if err != nil {
		return SavedLibraryQuery{}, fmt.Errorf(
			"%w: %v",
			ErrInvalidSavedLibraryQuery,
			err,
		)
	}
	query := storyLibraryQueryFromPayload(decoded)
	if version < 4 && len(legacyBrokenID) > 0 {
		query = library.TranslateLegacyBroken(query, legacyBrokenID[0])
	}
	if version < CurrentSavedLibraryQueryVersion {
		query = library.WithoutBrokenRestrictions(query)
	}
	return canonicalSavedLibraryQuery(query)
}

func MigrateSavedLibraryQuery(
	version int,
	payload string,
	legacyBrokenID ...int64,
) (SerializedQuery, error) {
	saved, err := DecodeSavedLibraryQuery(version, payload, legacyBrokenID...)
	if err != nil {
		return SerializedQuery{}, err
	}
	return EncodeSavedLibraryQuery(saved.StoryLibraryQuery())
}

func (q SavedLibraryQuery) StoryLibraryQuery() library.StoryLibraryQuery {
	var branches []library.StoryLibraryQuery
	for _, branch := range q.AnyOf {
		branches = append(branches, branch.StoryLibraryQuery())
	}
	return library.StoryLibraryQuery{
		IncludeBroken:    q.IncludeBroken,
		BrokenVisibility: q.BrokenVisibility,
		BrokenState:      q.BrokenState,
		AnyOf:            branches,
		Name:             q.Name,
		Languages:        append([]string(nil), q.Languages...),
		Compatibilities:  append([]library.Compatibility(nil), q.Compatibilities...),
		BooleanFilters:   append([]library.BooleanFilter(nil), q.BooleanFilters...),
		ChoiceFilters:    cloneChoiceFilters(q.ChoiceFilters),
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
	var branches []SavedLibraryQuery
	for _, branch := range query.AnyOf {
		saved, err := canonicalSavedLibraryQuery(branch)
		if err != nil {
			return SavedLibraryQuery{}, err
		}
		branches = append(branches, saved)
	}
	return SavedLibraryQuery{
		AnyOf:            branches,
		IncludeBroken:    query.IncludeBroken,
		BrokenVisibility: savedVisibility(query.BrokenVisibility),
		BrokenState:      query.BrokenState,
		Name:             query.Name,
		Languages:        append([]string(nil), query.Languages...),
		Compatibilities:  append([]library.Compatibility(nil), query.Compatibilities...),
		BooleanFilters:   append([]library.BooleanFilter(nil), query.BooleanFilters...),
		ChoiceFilters:    cloneChoiceFilters(query.ChoiceFilters),
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

func savedLibraryQueryPayload(query SavedLibraryQuery) collectionquery.Payload {
	var branches []collectionquery.Payload
	for _, branch := range query.AnyOf {
		branches = append(branches, savedLibraryQueryPayload(branch))
	}
	booleanFilters := make(
		[]collectionquery.BooleanReference,
		0,
		len(query.BooleanFilters),
	)
	for _, filter := range query.BooleanFilters {
		booleanFilters = append(booleanFilters, collectionquery.BooleanReference{
			DefinitionID: filter.DefinitionID,
			State:        string(filter.State),
		})
	}
	choiceFilters := make(
		[]collectionquery.ChoiceReference,
		0,
		len(query.ChoiceFilters),
	)
	for _, filter := range query.ChoiceFilters {
		choiceFilters = append(choiceFilters, collectionquery.ChoiceReference{
			DefinitionID: filter.DefinitionID,
			ValueIDs:     append([]int64(nil), filter.ValueIDs...),
		})
	}
	compatibilities := make([]string, 0, len(query.Compatibilities))
	for _, compatibility := range query.Compatibilities {
		compatibilities = append(compatibilities, string(compatibility))
	}
	return collectionquery.Payload{
		AnyOf:            branches,
		IncludeBroken:    query.IncludeBroken,
		BrokenVisibility: string(query.BrokenVisibility),
		BrokenState:      string(query.BrokenState),
		Name:             query.Name,
		Languages:        append([]string(nil), query.Languages...),
		Compatibilities:  compatibilities,
		BooleanFilters:   booleanFilters,
		ChoiceFilters:    choiceFilters,
	}
}

func storyLibraryQueryFromPayload(payload collectionquery.Payload) library.StoryLibraryQuery {
	var branches []library.StoryLibraryQuery
	for _, branch := range payload.AnyOf {
		branches = append(branches, storyLibraryQueryFromPayload(branch))
	}
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
		IncludeBroken:    payload.IncludeBroken,
		BrokenVisibility: library.BrokenVisibility(payload.BrokenVisibility),
		BrokenState:      library.BooleanFilterState(payload.BrokenState),
		AnyOf:            branches,
		Name:             payload.Name,
		Languages:        append([]string(nil), payload.Languages...),
		Compatibilities:  compatibilities,
		BooleanFilters:   booleanFilters,
		ChoiceFilters:    choiceFilters,
	}
}

func savedVisibility(mode library.BrokenVisibility) library.BrokenVisibility {
	if mode == library.BrokenHide {
		return ""
	}
	return mode
}
