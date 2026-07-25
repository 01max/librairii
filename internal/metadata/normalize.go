package metadata

import (
	"encoding/json"
	"errors"
	"fmt"
	"sort"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/google/uuid"
	"golang.org/x/text/language"
)

var ErrInvalidCatalog = errors.New("official catalog failed schema or integrity validation")

const (
	maxCatalogRecords    = 50_000
	maxTitleBytes        = 1_024
	maxDescriptionBytes  = 128 * 1024
	maxAttributionBytes  = 4 * 1024
	maxSourceRecordBytes = 1_024
)

type rawCatalogEnvelope struct {
	Response map[string]json.RawMessage `json:"response"`
}

type rawCatalogRecord struct {
	UUID             string                     `json:"uuid"`
	LocalesAvailable map[string]json.RawMessage `json:"locales_available"`
	LocalizedInfos   map[string]json.RawMessage `json:"localized_infos"`
	Authors          json.RawMessage            `json:"authors"`
	Publisher        json.RawMessage            `json:"publisher"`
	Publishers       json.RawMessage            `json:"publishers"`
	UpdatedAt        json.RawMessage            `json:"updated_at"`
}

func NormalizeCatalog(
	payload []byte,
	requestedLocale string,
) ([]NewOfficialStoryMetadata, error) {
	locale, err := canonicalLocale(requestedLocale)
	if err != nil {
		return nil, catalogError("invalid requested locale")
	}

	var envelope rawCatalogEnvelope
	if err := json.Unmarshal(payload, &envelope); err != nil {
		return nil, catalogError("invalid response envelope")
	}
	if len(envelope.Response) == 0 || len(envelope.Response) > maxCatalogRecords {
		return nil, catalogError("catalog response has an invalid record count")
	}

	sourceIDs := make([]string, 0, len(envelope.Response))
	for sourceID := range envelope.Response {
		sourceIDs = append(sourceIDs, sourceID)
	}
	sort.Strings(sourceIDs)

	seenUUIDs := make(map[string]struct{})
	normalized := make([]NewOfficialStoryMetadata, 0, len(sourceIDs))
	for _, sourceID := range sourceIDs {
		if err := validateText(sourceID, maxSourceRecordBytes, true); err != nil {
			return nil, catalogError("invalid source record identifier")
		}
		var record rawCatalogRecord
		var recordValues map[string]json.RawMessage
		if err := json.Unmarshal(envelope.Response[sourceID], &record); err != nil {
			return nil, catalogError("invalid catalog record")
		}
		if err := json.Unmarshal(envelope.Response[sourceID], &recordValues); err != nil {
			return nil, catalogError("invalid catalog record")
		}
		storyUUID, err := canonicalUUID(record.UUID)
		if err != nil {
			return nil, catalogError("catalog record has an invalid complete UUID")
		}
		if len(record.LocalizedInfos) == 0 || len(record.LocalesAvailable) == 0 {
			return nil, catalogError("catalog record has no localized metadata")
		}

		localizedBody, found, err := localizedValue(record.LocalizedInfos, locale)
		if err != nil {
			return nil, err
		}
		available, err := localeAvailable(record.LocalesAvailable, locale)
		if err != nil {
			return nil, err
		}
		if !found {
			continue
		}
		if !available {
			return nil, catalogError("localized metadata is not declared available")
		}
		if _, exists := seenUUIDs[storyUUID]; exists {
			return nil, catalogError("catalog contains a duplicate story UUID")
		}

		story, err := normalizeStory(
			record,
			recordValues,
			localizedBody,
			sourceID,
			storyUUID,
			locale,
		)
		if err != nil {
			return nil, err
		}
		seenUUIDs[storyUUID] = struct{}{}
		normalized = append(normalized, story)
	}
	if len(normalized) == 0 {
		return nil, catalogError("catalog contains no records for the requested locale")
	}
	return normalized, nil
}

func normalizeStory(
	record rawCatalogRecord,
	recordValues map[string]json.RawMessage,
	localizedBody json.RawMessage,
	sourceID string,
	storyUUID string,
	locale string,
) (NewOfficialStoryMetadata, error) {
	var localized map[string]json.RawMessage
	if err := json.Unmarshal(localizedBody, &localized); err != nil {
		return NewOfficialStoryMetadata{}, catalogError("invalid localized metadata")
	}
	title, err := requiredString(localized, "title", maxTitleBytes)
	if err != nil {
		return NewOfficialStoryMetadata{}, err
	}
	description, err := optionalString(localized, "description", maxDescriptionBytes)
	if err != nil {
		return NewOfficialStoryMetadata{}, err
	}
	author, err := attribution(record.Authors)
	if err != nil {
		return NewOfficialStoryMetadata{}, err
	}
	publisher, err := firstNonemptyAttribution(record.Publisher, record.Publishers)
	if err != nil {
		return NewOfficialStoryMetadata{}, err
	}
	duration, err := optionalIntFromMaps(
		localized,
		recordValues,
		"duration_seconds", "durationSeconds",
	)
	if err != nil {
		return NewOfficialStoryMetadata{}, err
	}
	minimumAge, err := optionalIntFromMaps(
		localized,
		recordValues,
		"minimum_age", "minimumAge", "min_age", "age_min",
	)
	if err != nil {
		return NewOfficialStoryMetadata{}, err
	}
	maximumAge, err := optionalIntFromMaps(
		localized,
		recordValues,
		"maximum_age", "maximumAge", "max_age", "age_max",
	)
	if err != nil {
		return NewOfficialStoryMetadata{}, err
	}
	if duration != nil && *duration < 0 {
		return NewOfficialStoryMetadata{}, catalogError("negative story duration")
	}
	if minimumAge != nil && *minimumAge < 0 ||
		maximumAge != nil && *maximumAge < 0 ||
		minimumAge != nil && maximumAge != nil && *minimumAge > *maximumAge {
		return NewOfficialStoryMetadata{}, catalogError("invalid story age range")
	}
	sourceUpdatedAt, err := optionalTimestamp(record.UpdatedAt)
	if err != nil {
		return NewOfficialStoryMetadata{}, err
	}

	return NewOfficialStoryMetadata{
		StoryUUID:       storyUUID,
		Title:           title,
		Description:     description,
		Author:          author,
		Publisher:       publisher,
		Language:        locale,
		DurationSeconds: duration,
		MinimumAge:      minimumAge,
		MaximumAge:      maximumAge,
		Provenance:      ProvenanceLuniiCatalog,
		SourceRecordID:  sourceID,
		SourceUpdatedAt: sourceUpdatedAt,
	}, nil
}

func canonicalLocale(value string) (string, error) {
	value = strings.ReplaceAll(strings.TrimSpace(value), "_", "-")
	if value == "" || len(value) > 35 {
		return "", ErrInvalidCatalog
	}
	tag, err := language.Parse(value)
	if err != nil || tag == language.Und {
		return "", ErrInvalidCatalog
	}
	return tag.String(), nil
}

func canonicalUUID(value string) (string, error) {
	value = strings.TrimSpace(value)
	if len(value) != 36 {
		return "", ErrInvalidCatalog
	}
	parsed, err := uuid.Parse(value)
	if err != nil || parsed == uuid.Nil {
		return "", ErrInvalidCatalog
	}
	return parsed.String(), nil
}

func localizedValue(
	values map[string]json.RawMessage,
	locale string,
) (json.RawMessage, bool, error) {
	var selected json.RawMessage
	found := false
	for key, value := range values {
		canonical, err := canonicalLocale(key)
		if err != nil {
			return nil, false, catalogError("invalid localized metadata locale")
		}
		if canonical != locale {
			continue
		}
		if found {
			return nil, false, catalogError("ambiguous localized metadata locale")
		}
		selected = value
		found = true
	}
	return selected, found, nil
}

func localeAvailable(
	values map[string]json.RawMessage,
	locale string,
) (bool, error) {
	found := false
	for key := range values {
		canonical, err := canonicalLocale(key)
		if err != nil {
			return false, catalogError("invalid available locale")
		}
		if canonical != locale {
			continue
		}
		if found {
			return false, catalogError("ambiguous available locale")
		}
		found = true
	}
	return found, nil
}

func requiredString(
	values map[string]json.RawMessage,
	key string,
	maximum int,
) (string, error) {
	value, err := optionalString(values, key, maximum)
	if err != nil {
		return "", err
	}
	if value == "" {
		return "", catalogError("required localized string is missing")
	}
	return value, nil
}

func optionalString(
	values map[string]json.RawMessage,
	key string,
	maximum int,
) (string, error) {
	body, exists := values[key]
	if !exists || string(body) == "null" {
		return "", nil
	}
	var value string
	if err := json.Unmarshal(body, &value); err != nil {
		return "", catalogError("localized string has an invalid type")
	}
	value = strings.TrimSpace(value)
	if err := validateText(value, maximum, false); err != nil {
		return "", catalogError("localized string exceeds its limit")
	}
	return value, nil
}

func validateText(value string, maximum int, required bool) error {
	if !utf8.ValidString(value) ||
		len(value) > maximum ||
		required && strings.TrimSpace(value) == "" {
		return ErrInvalidCatalog
	}
	return nil
}

func attribution(body json.RawMessage) (string, error) {
	if len(body) == 0 || string(body) == "null" {
		return "", nil
	}
	var keyed map[string]struct {
		Name string `json:"name"`
	}
	if err := json.Unmarshal(body, &keyed); err == nil {
		keys := make([]string, 0, len(keyed))
		for key := range keyed {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		names := make([]string, 0, len(keys))
		for _, key := range keys {
			name := strings.TrimSpace(keyed[key].Name)
			if name != "" {
				names = append(names, name)
			}
		}
		return joinedAttribution(names)
	}
	var names []string
	if err := json.Unmarshal(body, &names); err == nil {
		return joinedAttribution(names)
	}
	var name string
	if err := json.Unmarshal(body, &name); err == nil {
		return joinedAttribution([]string{name})
	}
	return "", catalogError("invalid attribution metadata")
}

func firstNonemptyAttribution(bodies ...json.RawMessage) (string, error) {
	for _, body := range bodies {
		value, err := attribution(body)
		if err != nil {
			return "", err
		}
		if value != "" {
			return value, nil
		}
	}
	return "", nil
}

func joinedAttribution(names []string) (string, error) {
	normalized := make([]string, 0, len(names))
	for _, name := range names {
		name = strings.TrimSpace(name)
		if name != "" {
			normalized = append(normalized, name)
		}
	}
	value := strings.Join(normalized, ", ")
	if err := validateText(value, maxAttributionBytes, false); err != nil {
		return "", catalogError("attribution exceeds its limit")
	}
	return value, nil
}

func optionalIntFromMaps(
	primary map[string]json.RawMessage,
	secondary map[string]json.RawMessage,
	keys ...string,
) (*int, error) {
	for _, values := range []map[string]json.RawMessage{primary, secondary} {
		for _, key := range keys {
			body, exists := values[key]
			if !exists || string(body) == "null" {
				continue
			}
			var value int
			if err := json.Unmarshal(body, &value); err == nil {
				return &value, nil
			}
			var text string
			if err := json.Unmarshal(body, &text); err == nil {
				parsed, err := strconv.Atoi(strings.TrimSpace(text))
				if err == nil {
					return &parsed, nil
				}
			}
			return nil, catalogError("numeric metadata has an invalid type")
		}
	}
	return nil, nil
}

func optionalTimestamp(body json.RawMessage) (*time.Time, error) {
	if len(body) == 0 || string(body) == "null" {
		return nil, nil
	}
	var text string
	if err := json.Unmarshal(body, &text); err != nil {
		return nil, catalogError("source timestamp has an invalid type")
	}
	value, err := time.Parse(time.RFC3339, text)
	if err != nil {
		return nil, catalogError("source timestamp is invalid")
	}
	value = value.UTC()
	return &value, nil
}

func catalogError(message string) error {
	return fmt.Errorf("%w: %s", ErrInvalidCatalog, message)
}
