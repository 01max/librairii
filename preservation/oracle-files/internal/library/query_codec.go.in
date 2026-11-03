package library

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/url"
	"sort"
	"strconv"
	"strings"
)

const (
	StoryLibraryQueryVersion = 3
	storyLibraryHashPath     = "#/library"
)

var ErrInvalidStoryLibraryHash = errors.New("story library hash is invalid")

func EncodeStoryLibraryQuery(query StoryLibraryQuery) (string, error) {
	query, err := canonicalStoryLibraryQuery(query)
	if err != nil {
		return "", err
	}
	values := url.Values{}
	if len(query.AnyOf) > 0 {
		data, err := json.Marshal(query.AnyOf)
		if err != nil {
			return "", err
		}
		values.Set("any", string(data))
	}
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

func DecodeStoryLibraryQuery(hash string, legacyBrokenID ...int64) (StoryLibraryQuery, error) {
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
		case "any", "v",
			"includeBroken", "brokenVisibility", "brokenState",
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
	if err != nil || (version != "1" && version != "2" && version != strconv.Itoa(StoryLibraryQueryVersion)) {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	var query StoryLibraryQuery
	if encoded, err := oneQueryValue(values, "any", false); err != nil {
		return StoryLibraryQuery{}, err
	} else if encoded != "" {
		decoder := json.NewDecoder(strings.NewReader(encoded))
		decoder.DisallowUnknownFields()
		if len(encoded) > 262144 || decoder.Decode(&query.AnyOf) != nil || decoder.Decode(&struct{}{}) != io.EOF {
			return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
		}
	}
	visibility, visibilityErr := oneQueryValue(values, "includeBroken", false)
	if visibilityErr != nil || (visibility != "" && visibility != "true") {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	query.IncludeBroken = visibility == "true"
	mode, modeErr := oneQueryValue(values, "brokenVisibility", false)
	state, stateErr := oneQueryValue(values, "brokenState", false)
	if modeErr != nil || stateErr != nil || (version == "1" && (mode != "" || state != "")) || (version != "1" && visibility != "") {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	query.BrokenVisibility = BrokenVisibility(mode)
	query.BrokenState = BooleanFilterState(state)
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
	// Validate non-flag predicates before removing retired tag references, so
	// malformed duplicate criteria cannot become valid through migration.
	if !validLegacyFlagFields(query, 50) {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	if _, err := canonicalStoryLibraryQuery(WithoutBrokenRestrictions(query)); err != nil {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
	}
	if version == "1" && len(legacyBrokenID) > 0 {
		query = TranslateLegacyBroken(query, legacyBrokenID[0])
	}
	if version != strconv.Itoa(StoryLibraryQueryVersion) {
		query = WithoutBrokenRestrictions(query)
	} else if HasBrokenRestrictions(query) {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryHash
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
	remaining := 50
	return canonicalMembershipNode(query, &remaining)
}

func canonicalMembershipNode(query StoryLibraryQuery, remaining *int) (StoryLibraryQuery, error) {
	*remaining--
	if *remaining < 0 {
		return StoryLibraryQuery{}, ErrInvalidStoryLibraryQuery
	}
	if len(query.AnyOf) > 0 {
		branches := make([]StoryLibraryQuery, len(query.AnyOf))
		for i, branch := range query.AnyOf {
			var err error
			branches[i], err = canonicalMembershipNode(branch, remaining)
			if err != nil {
				return StoryLibraryQuery{}, err
			}
		}
		query.AnyOf = branches
	}
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

// TranslateLegacyBroken is called only at a legacy-version boundary. Current
// user tags, including reused IDs, must never be interpreted as story flags.
func TranslateLegacyBroken(query StoryLibraryQuery, legacyID int64) StoryLibraryQuery {
	filters := make([]BooleanFilter, 0, len(query.BooleanFilters))
	for _, filter := range query.BooleanFilters {
		if legacyID > 0 && filter.DefinitionID == legacyID {
			if filter.State != BooleanIgnored {
				query.BrokenState = filter.State
			}
		} else {
			filters = append(filters, filter)
		}
	}
	query.BooleanFilters = filters
	for i, branch := range query.AnyOf {
		query.AnyOf[i] = TranslateLegacyBroken(branch, legacyID)
	}
	return query
}

// WithoutBrokenRestrictions applies the intentional policy migration only at
// legacy decoding boundaries. Ordinary user-tag predicates are left intact.
func WithoutBrokenRestrictions(query StoryLibraryQuery) StoryLibraryQuery {
	query.IncludeBroken = false
	query.BrokenVisibility = ""
	query.BrokenState = ""
	if query.AnyOf != nil {
		branches := make([]StoryLibraryQuery, len(query.AnyOf))
		for i, branch := range query.AnyOf {
			branches[i] = WithoutBrokenRestrictions(branch)
		}
		query.AnyOf = branches
	}
	return query
}

func HasBrokenRestrictions(query StoryLibraryQuery) bool {
	if query.IncludeBroken || query.BrokenVisibility != "" || query.BrokenState != "" {
		return true
	}
	for _, branch := range query.AnyOf {
		if HasBrokenRestrictions(branch) {
			return true
		}
	}
	return false
}

func validLegacyFlagFields(q StoryLibraryQuery, remaining int) bool {
	if remaining <= 0 {
		return false
	}
	if q.BrokenVisibility != "" && q.BrokenVisibility != BrokenHide && q.BrokenVisibility != BrokenInclude && q.BrokenVisibility != BrokenOnly {
		return false
	}
	if q.BrokenState != "" && q.BrokenState != BooleanIgnored && q.BrokenState != BooleanTrue && q.BrokenState != BooleanFalse {
		return false
	}
	for _, b := range q.AnyOf {
		if !validLegacyFlagFields(b, remaining-1) {
			return false
		}
	}
	return true
}
