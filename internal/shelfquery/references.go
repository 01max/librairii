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

type currentPayload struct {
	Name            string             `json:"name,omitempty"`
	Languages       []string           `json:"languages,omitempty"`
	Compatibilities []string           `json:"compatibilities,omitempty"`
	BooleanFilters  []BooleanReference `json:"booleanFilters,omitempty"`
	ChoiceFilters   []ChoiceReference  `json:"choiceFilters,omitempty"`
}

type legacyPayload struct {
	currentPayload
	Page     int    `json:"page,omitempty"`
	PageSize int    `json:"pageSize,omitempty"`
	Sort     string `json:"sort,omitempty"`
}

func DecodeReferences(version int, payload string) (References, error) {
	var decoded currentPayload
	switch version {
	case 1:
		var legacy legacyPayload
		if err := decode(payload, &legacy); err != nil {
			return References{}, err
		}
		decoded = legacy.currentPayload
	case CurrentVersion:
		if err := decode(payload, &decoded); err != nil {
			return References{}, err
		}
	default:
		return References{}, ErrUnsupportedVersion
	}
	if err := validateReferences(decoded); err != nil {
		return References{}, err
	}
	return References{
		BooleanFilters: decoded.BooleanFilters,
		ChoiceFilters:  decoded.ChoiceFilters,
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

func validateReferences(payload currentPayload) error {
	definitions := make(map[int64]struct{})
	for _, filter := range payload.BooleanFilters {
		if filter.DefinitionID <= 0 ||
			(filter.State != "ignored" &&
				filter.State != "true" &&
				filter.State != "false") {
			return ErrInvalidPayload
		}
		if _, duplicate := definitions[filter.DefinitionID]; duplicate {
			return ErrInvalidPayload
		}
		definitions[filter.DefinitionID] = struct{}{}
	}
	for _, filter := range payload.ChoiceFilters {
		if filter.DefinitionID <= 0 || len(filter.ValueIDs) == 0 {
			return ErrInvalidPayload
		}
		if _, duplicate := definitions[filter.DefinitionID]; duplicate {
			return ErrInvalidPayload
		}
		definitions[filter.DefinitionID] = struct{}{}
		values := make(map[int64]struct{}, len(filter.ValueIDs))
		for _, valueID := range filter.ValueIDs {
			if valueID <= 0 {
				return ErrInvalidPayload
			}
			if _, duplicate := values[valueID]; duplicate {
				continue
			}
			values[valueID] = struct{}{}
		}
	}
	return nil
}
