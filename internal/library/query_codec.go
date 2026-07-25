package library

import (
	"errors"
	"fmt"
	"net/url"
	"sort"
	"strconv"
	"strings"
)

const (
	StoryLibraryQueryVersion = 1
	storyLibraryHashPath     = "#/library"
)

var ErrInvalidStoryLibraryHash = errors.New("story library hash is invalid")

func EncodeStoryLibraryQuery(query StoryLibraryQuery) (string, error) {
	query, err := canonicalStoryLibraryQuery(query)
	if err != nil {
		return "", err
	}
	values := url.Values{}
	values.Set("v", strconv.Itoa(StoryLibraryQueryVersion))
	if query.Name != "" {
		values.Set("name", query.Name)
	}
	if query.Page != 1 {
		values.Set("page", strconv.Itoa(query.Page))
	}
	if query.PageSize != DefaultPageSize {
		values.Set("size", strconv.Itoa(query.PageSize))
	}
	if query.Sort != SortNameAscending {
		values.Set("sort", string(query.Sort))
	}
	for _, locale := range query.Languages {
		values.Add("language", locale)
	}
	for _, compatibility := range query.Compatibilities {
		values.Add("compatibility", string(compatibility))
	}
	for _, filter := range query.BooleanFilters {
		if filter.State == BooleanIgnored {
			continue
		}
		values.Add(
			"bool",
			strconv.FormatInt(filter.DefinitionID, 10)+":"+string(filter.State),
		)
	}
	for _, filter := range query.ChoiceFilters {
		valueIDs := make([]string, 0, len(filter.ValueIDs))
		for _, valueID := range filter.ValueIDs {
			valueIDs = append(valueIDs, strconv.FormatInt(valueID, 10))
		}
		values.Add(
			"choice",
			strconv.FormatInt(filter.DefinitionID, 10)+":"+strings.Join(valueIDs, ","),
		)
	}
	return storyLibraryHashPath + "?" + values.Encode(), nil
}

func DecodeStoryLibraryQuery(hash string) (StoryLibraryQuery, error) {
	if hash == "" {
		return canonicalStoryLibraryQuery(StoryLibraryQuery{})
	}
	path, rawQuery, found := strings.Cut(hash, "?")
	if !found || path != storyLibraryHashPath {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	values, err := url.ParseQuery(rawQuery)
	if err != nil {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	for key := range values {
		switch key {
		case "v",
			"name",
			"page",
			"size",
			"sort",
			"language",
			"compatibility",
			"bool",
			"choice":
		default:
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
		}
	}
	version, err := oneQueryValue(values, "v", true)
	if err != nil || version != strconv.Itoa(StoryLibraryQueryVersion) {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	var query StoryLibraryQuery
	if query.Name, err = oneQueryValue(values, "name", false); err != nil {
		return StoryLibraryQuery{}, err
	}
	if page, valueErr := oneQueryValue(values, "page", false); valueErr != nil {
		return StoryLibraryQuery{}, valueErr
	} else if page != "" {
		query.Page, err = strconv.Atoi(page)
		if err != nil {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
		}
	}
	if size, valueErr := oneQueryValue(values, "size", false); valueErr != nil {
		return StoryLibraryQuery{}, valueErr
	} else if size != "" {
		query.PageSize, err = strconv.Atoi(size)
		if err != nil {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
		}
	}
	if sortValue, valueErr := oneQueryValue(values, "sort", false); valueErr != nil {
		return StoryLibraryQuery{}, valueErr
	} else {
		query.Sort = Sort(sortValue)
	}
	query.Languages = append(query.Languages, values["language"]...)
	for _, value := range values["compatibility"] {
		query.Compatibilities = append(query.Compatibilities, Compatibility(value))
	}
	for _, encoded := range values["bool"] {
		definition, state, ok := strings.Cut(encoded, ":")
		definitionID, parseErr := strconv.ParseInt(definition, 10, 64)
		if !ok || parseErr != nil {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
		}
		query.BooleanFilters = append(query.BooleanFilters, BooleanFilter{
			DefinitionID: definitionID,
			State:        BooleanFilterState(state),
		})
	}
	for _, encoded := range values["choice"] {
		definition, encodedValues, ok := strings.Cut(encoded, ":")
		definitionID, parseErr := strconv.ParseInt(definition, 10, 64)
		if !ok || parseErr != nil || encodedValues == "" {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
		}
		filter := ChoiceFilter{DefinitionID: definitionID}
		for _, encodedValue := range strings.Split(encodedValues, ",") {
			valueID, parseErr := strconv.ParseInt(encodedValue, 10, 64)
			if parseErr != nil {
				return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
			}
			filter.ValueIDs = append(filter.ValueIDs, valueID)
		}
		query.ChoiceFilters = append(query.ChoiceFilters, filter)
	}
	query, err = canonicalStoryLibraryQuery(query)
	if err != nil {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	return query, nil
}

func canonicalStoryLibraryQuery(
	query StoryLibraryQuery,
) (StoryLibraryQuery, error) {
	query, err := normalizeStoryLibraryQuery(query)
	if err != nil {
		return StoryLibraryQuery{}, err
	}
	if query.BooleanFilters == nil {
		query.BooleanFilters = []BooleanFilter{}
	}
	if query.ChoiceFilters == nil {
		query.ChoiceFilters = []ChoiceFilter{}
	}
	if query.Languages == nil {
		query.Languages = []string{}
	}
	if query.Compatibilities == nil {
		query.Compatibilities = []Compatibility{}
	}
	sort.Slice(query.BooleanFilters, func(i, j int) bool {
		return query.BooleanFilters[i].DefinitionID <
			query.BooleanFilters[j].DefinitionID
	})
	sort.Slice(query.ChoiceFilters, func(i, j int) bool {
		return query.ChoiceFilters[i].DefinitionID <
			query.ChoiceFilters[j].DefinitionID
	})
	for index := range query.ChoiceFilters {
		sort.Slice(query.ChoiceFilters[index].ValueIDs, func(i, j int) bool {
			return query.ChoiceFilters[index].ValueIDs[i] <
				query.ChoiceFilters[index].ValueIDs[j]
		})
	}
	return query, nil
}

// CanonicalStoryLibraryMembershipQuery returns only fields that can change
// which stories belong to a result. Navigation and presentation state are
// deliberately discarded so callers such as saved shelves and exports share
// the collection's exact normalization and validation rules.
func CanonicalStoryLibraryMembershipQuery(
	query StoryLibraryQuery,
) (StoryLibraryQuery, error) {
	query.Languages = append([]string(nil), query.Languages...)
	query.Compatibilities = append(
		[]Compatibility(nil),
		query.Compatibilities...,
	)
	booleanFilters := make([]BooleanFilter, 0, len(query.BooleanFilters))
	for _, filter := range query.BooleanFilters {
		if filter.State != BooleanIgnored {
			booleanFilters = append(booleanFilters, filter)
		}
	}
	query.BooleanFilters = booleanFilters
	choiceFilters := make([]ChoiceFilter, 0, len(query.ChoiceFilters))
	for _, filter := range query.ChoiceFilters {
		choiceFilters = append(choiceFilters, ChoiceFilter{
			DefinitionID: filter.DefinitionID,
			ValueIDs:     append([]int64(nil), filter.ValueIDs...),
		})
	}
	query.ChoiceFilters = choiceFilters
	query.Page = 1
	query.PageSize = DefaultPageSize
	query.Sort = SortNameAscending

	query, err := canonicalStoryLibraryQuery(query)
	if err != nil {
		return StoryLibraryQuery{}, err
	}
	query.Page = 0
	query.PageSize = 0
	query.Sort = ""
	return query, nil
}

func oneQueryValue(
	values url.Values,
	key string,
	required bool,
) (string, error) {
	found := values[key]
	if len(found) == 0 && !required {
		return "", nil
	}
	if len(found) != 1 || found[0] == "" {
		return "", fmt.Errorf("%w: %s", ErrInvalidStoryLibraryHash, key)
	}
	return found[0], nil
}
