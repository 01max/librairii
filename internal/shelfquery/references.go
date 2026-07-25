package shelfquery

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
)

const (
	CurrentVersion  = 2
	MaxPayloadBytes = 262_144
)

var (
	ErrInvalidPayload     = errors.New("saved shelf query payload is invalid")
	ErrUnsupportedVersion = errors.New("saved shelf query version is unsupported")
)

type BooleanReference struct {
	DefinitionID int64  `json:"definitionId"`
	State        string `json:"state"`
}

type ChoiceReference struct {
	DefinitionID int64   `json:"definitionId"`
	ValueIDs     []int64 `json:"valueIds"`
}

type References struct {
	BooleanFilters []BooleanReference
	ChoiceFilters  []ChoiceReference
}

// Payload is the single versioned saved-query wire schema shared by shelf
// evaluation and tag-reference planning.
type Payload struct {
	Name            string             `json:"name,omitempty"`
	Languages       []string           `json:"languages,omitempty"`
	Compatibilities []string           `json:"compatibilities,omitempty"`
	BooleanFilters  []BooleanReference `json:"booleanFilters,omitempty"`
	ChoiceFilters   []ChoiceReference  `json:"choiceFilters,omitempty"`
}

type legacyPayload struct {
	Payload
	Page     int    `json:"page,omitempty"`
	PageSize int    `json:"pageSize,omitempty"`
	Sort     string `json:"sort,omitempty"`
}

func EncodePayload(payload Payload) (string, error) {
	if _, err := referencesFromPayload(payload); err != nil {
		return "", err
	}
	encoded, err := json.Marshal(payload)
	if err != nil || len(encoded) > MaxPayloadBytes {
		return "", ErrInvalidPayload
	}
	return string(encoded), nil
}

func DecodePayload(version int, payload string) (Payload, error) {
	var decoded Payload
	switch version {
	case 1:
		var legacy legacyPayload
		if err := decode(payload, &legacy); err != nil {
			return Payload{}, err
		}
		decoded = legacy.Payload
	case CurrentVersion:
		if err := decode(payload, &decoded); err != nil {
			return Payload{}, err
		}
	default:
		return Payload{}, ErrUnsupportedVersion
	}
	return decoded, nil
}

func DecodeReferences(version int, payload string) (References, error) {
	decoded, err := DecodePayload(version, payload)
	if err != nil {
		return References{}, err
	}
	return referencesFromPayload(decoded)
}

func referencesFromPayload(payload Payload) (References, error) {
	booleanFilters := make([]BooleanReference, 0, len(payload.BooleanFilters))
	choiceFilters := make([]ChoiceReference, 0, len(payload.ChoiceFilters))
	definitions := make(map[int64]struct{})
	for _, filter := range payload.BooleanFilters {
		if filter.DefinitionID <= 0 ||
			(filter.State != "ignored" &&
				filter.State != "true" &&
				filter.State != "false") {
			return References{}, ErrInvalidPayload
		}
		if filter.State == "ignored" {
			continue
		}
		if _, duplicate := definitions[filter.DefinitionID]; duplicate {
			return References{}, ErrInvalidPayload
		}
		definitions[filter.DefinitionID] = struct{}{}
		booleanFilters = append(booleanFilters, filter)
	}
	for _, filter := range payload.ChoiceFilters {
		if filter.DefinitionID <= 0 || len(filter.ValueIDs) == 0 {
			return References{}, ErrInvalidPayload
		}
		if _, duplicate := definitions[filter.DefinitionID]; duplicate {
			return References{}, ErrInvalidPayload
		}
		definitions[filter.DefinitionID] = struct{}{}
		values := make(map[int64]struct{}, len(filter.ValueIDs))
		for _, valueID := range filter.ValueIDs {
			if valueID <= 0 {
				return References{}, ErrInvalidPayload
			}
			if _, duplicate := values[valueID]; duplicate {
				continue
			}
			values[valueID] = struct{}{}
		}
		choiceFilters = append(choiceFilters, filter)
	}
	return References{
		BooleanFilters: booleanFilters,
		ChoiceFilters:  choiceFilters,
	}, nil
}

func decode(payload string, target any) error {
	if len(payload) < 2 ||
		len(payload) > MaxPayloadBytes ||
		strings.TrimSpace(payload) == "null" {
		return ErrInvalidPayload
	}
	if err := ValidateUniqueObjectKeys(payload); err != nil {
		return err
	}
	decoder := json.NewDecoder(strings.NewReader(payload))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidPayload, err)
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return ErrInvalidPayload
	}
	return nil
}
