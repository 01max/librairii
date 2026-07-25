package shelves

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"

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
	payload, err := json.Marshal(saved)
	if err != nil {
		return SerializedQuery{}, fmt.Errorf(
			"%w: encode canonical query: %v",
			ErrInvalidSavedLibraryQuery,
			err,
		)
	}
	return SerializedQuery{
		Version: CurrentSavedLibraryQueryVersion,
		Payload: string(payload),
	}, nil
}

func DecodeSavedLibraryQuery(
	version int,
	payload string,
) (SavedLibraryQuery, error) {
	var query library.StoryLibraryQuery
	switch version {
	case 1:
		if err := decodeSavedPayload(payload, &query); err != nil {
			return SavedLibraryQuery{}, err
		}
	case CurrentSavedLibraryQueryVersion:
		var saved SavedLibraryQuery
		if err := decodeSavedPayload(payload, &saved); err != nil {
			return SavedLibraryQuery{}, err
		}
		query = saved.StoryLibraryQuery()
	default:
		return SavedLibraryQuery{}, ErrUnsupportedSavedQueryVersion
	}
	return canonicalSavedLibraryQuery(query)
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

func decodeSavedPayload(payload string, target any) error {
	if len(payload) < 2 || len(payload) > maxSavedQueryPayloadBytes ||
		strings.TrimSpace(payload) == "null" {
		return ErrInvalidSavedLibraryQuery
	}
	decoder := json.NewDecoder(strings.NewReader(payload))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return fmt.Errorf("%w: decode payload: %v", ErrInvalidSavedLibraryQuery, err)
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return ErrInvalidSavedLibraryQuery
	}
	return nil
}
