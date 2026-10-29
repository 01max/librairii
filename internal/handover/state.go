// Package handover exports only explicitly supported library-bound preferences.
package handover

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"net/url"
	"strconv"
	"strings"

	"github.com/01max/librairii/internal/collections"
	"github.com/01max/librairii/internal/library"
	"github.com/google/uuid"
)

const MaxBytes = 8 * 1024 * 1024
const MaxHistory = 100
const MaxNotices = 4096
const maxSafeID = 9007199254740991

var ErrInvalid = errors.New("Preference transfer is invalid or exceeds its limits.")

type Frame struct {
	Hash        string `json:"hash"`
	Kind        string `json:"kind"`
	ID          *int64 `json:"id,omitempty"`
	ManualOrder *bool  `json:"manualOrder,omitempty"`
}
type State struct {
	Presentation        string   `json:"presentation"`
	DismissedOperations []string `json:"dismissedOperations"`
	SyncPanelHidden     bool     `json:"syncPanelHidden"`
	Frames              []Frame  `json:"frames"`
	Position            int      `json:"position"`
}
type Document struct {
	Format            string `json:"format"`
	Version           int    `json:"version"`
	LibraryID         string `json:"libraryId"`
	LegacyBrokenTagID int64  `json:"legacyBrokenTagId"`
	State             State  `json:"state"`
}

func DecodeState(input string) (State, error) {
	var state State
	if len(input) > MaxBytes {
		return state, ErrInvalid
	}
	if err := uniqueKeys(input); err != nil {
		return state, err
	}
	decoder := json.NewDecoder(strings.NewReader(input))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&state) != nil {
		return state, ErrInvalid
	}
	if err := decoder.Decode(new(any)); err != io.EOF {
		return state, ErrInvalid
	}
	if err := state.Validate(); err != nil {
		return state, err
	}
	return state, nil
}

func (s State) Validate() error {
	if s.Presentation != "list" && s.Presentation != "small_covers" && s.Presentation != "large_covers" {
		return ErrInvalid
	}
	if len(s.Frames) == 0 || len(s.Frames) > MaxHistory || s.Position < 0 || s.Position >= len(s.Frames) || len(s.DismissedOperations) > MaxNotices {
		return ErrInvalid
	}
	seen := map[string]bool{}
	for _, id := range s.DismissedOperations {
		parsed, err := uuid.Parse(id)
		if err != nil || parsed.String() != id || seen[id] {
			return ErrInvalid
		}
		seen[id] = true
	}
	for _, frame := range s.Frames {
		switch frame.Kind {
		case "library", "custom":
			if frame.ID != nil || frame.ManualOrder != nil {
				return ErrInvalid
			}
		case "collection":
			if frame.ID == nil || *frame.ID <= 0 || *frame.ID > maxSafeID || frame.ManualOrder != nil {
				return ErrInvalid
			}
		case "shelf":
			if frame.ID == nil || *frame.ID <= 0 || *frame.ID > maxSafeID || frame.ManualOrder == nil {
				return ErrInvalid
			}
		default:
			return ErrInvalid
		}
		if err := validateHash(frame.Hash); err != nil {
			return err
		}
	}
	return nil
}

func Encode(libraryID string, legacyBrokenTagID int64, state State) ([]byte, error) {
	id, err := uuid.Parse(libraryID)
	if err != nil || id.String() != libraryID || legacyBrokenTagID < 0 || legacyBrokenTagID > maxSafeID {
		return nil, ErrInvalid
	}
	if err := state.Validate(); err != nil {
		return nil, err
	}
	// Build the document ourselves: callers cannot supply roots, paths, tokens,
	// raw browser history or browser storage databases through this boundary.
	if state.DismissedOperations == nil {
		state.DismissedOperations = []string{}
	}
	body, err := json.Marshal(Document{"librairii.preference-handover", 1, libraryID, legacyBrokenTagID, state})
	if err != nil || len(body)+1 > MaxBytes {
		return nil, ErrInvalid
	}
	return append(body, '\n'), nil
}

func validateHash(hash string) error {
	if hash == "" {
		return nil
	}
	if len(hash) > 262144 || !strings.HasPrefix(hash, "#/library?") {
		return ErrInvalid
	}
	values, err := url.ParseQuery(strings.TrimPrefix(hash, "#/library?"))
	version := values.Get("v")
	if err != nil || (version != "1" && version != "2" && version != "3") {
		return ErrInvalid
	}
	for key, entries := range values {
		switch key {
		case "v", "name", "page", "size", "sort", "any", "includeBroken", "brokenVisibility", "brokenState":
			if len(entries) != 1 {
				return ErrInvalid
			}
		case "language", "compatibility", "bool", "choice":
		default:
			return ErrInvalid
		}
	}
	query := library.StoryLibraryQuery{Name: values.Get("name")}
	if value, ok := values["includeBroken"]; ok && (version != "1" || value[0] != "true") {
		return ErrInvalid
	}
	if value, ok := values["brokenVisibility"]; ok && (version != "2" || (value[0] != "hide" && value[0] != "include" && value[0] != "only")) {
		return ErrInvalid
	}
	if value, ok := values["brokenState"]; ok && (version != "2" || (value[0] != "true" && value[0] != "false")) {
		return ErrInvalid
	}
	if text := values.Get("sort"); text != "" && text != "name_asc" && text != "imported_desc" {
		return ErrInvalid
	}
	for _, entry := range []struct {
		key string
		max int64
	}{{"page", maxSafeID}, {"size", 100}} {
		if text, ok := values[entry.key]; ok {
			n, err := strconv.ParseInt(text[0], 10, 64)
			if err != nil || n <= 0 || n > entry.max {
				return ErrInvalid
			}
		}
	}
	query.Languages = values["language"]
	for _, value := range values["compatibility"] {
		query.Compatibilities = append(query.Compatibilities, library.Compatibility(value))
	}
	for _, value := range values["bool"] {
		id, state, ok := strings.Cut(value, ":")
		n, err := strconv.ParseInt(id, 10, 64)
		if !ok || err != nil {
			return ErrInvalid
		}
		query.BooleanFilters = append(query.BooleanFilters, library.BooleanFilter{DefinitionID: n, State: library.BooleanFilterState(state)})
	}
	for _, value := range values["choice"] {
		id, items, ok := strings.Cut(value, ":")
		n, err := strconv.ParseInt(id, 10, 64)
		if !ok || err != nil {
			return ErrInvalid
		}
		filter := library.ChoiceFilter{DefinitionID: n}
		for _, item := range strings.Split(items, ",") {
			v, err := strconv.ParseInt(item, 10, 64)
			if err != nil {
				return ErrInvalid
			}
			filter.ValueIDs = append(filter.ValueIDs, v)
		}
		query.ChoiceFilters = append(query.ChoiceFilters, filter)
	}
	if value := values.Get("any"); value != "" {
		if err := uniqueKeys(value); err != nil {
			return err
		}
		decoder := json.NewDecoder(bytes.NewBufferString(value))
		decoder.DisallowUnknownFields()
		if decoder.Decode(&query.AnyOf) != nil {
			return ErrInvalid
		}
		if err := decoder.Decode(new(any)); err != io.EOF {
			return ErrInvalid
		}
	}
	if version != "3" {
		query = library.WithoutBrokenRestrictions(query)
	}
	if _, err := collections.EncodeSavedLibraryQuery(query); err != nil {
		return ErrInvalid
	}
	return nil
}

func uniqueKeys(input string) error {
	decoder := json.NewDecoder(strings.NewReader(input))
	var visit func(int) error
	visit = func(depth int) error {
		if depth > 64 {
			return ErrInvalid
		}
		token, err := decoder.Token()
		if err != nil {
			return ErrInvalid
		}
		delimiter, container := token.(json.Delim)
		if !container {
			return nil
		}
		seen := map[string]bool{}
		if delimiter != '{' && delimiter != '[' {
			return ErrInvalid
		}
		for decoder.More() {
			if delimiter == '{' {
				key, err := decoder.Token()
				if err != nil {
					return ErrInvalid
				}
				text, ok := key.(string)
				if !ok || seen[text] {
					return ErrInvalid
				}
				seen[text] = true
			}
			if err := visit(depth + 1); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		if err != nil {
			return ErrInvalid
		}
		return nil
	}
	if err := visit(0); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return ErrInvalid
	}
	return nil
}
