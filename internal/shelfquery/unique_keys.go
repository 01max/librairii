package shelfquery

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
)

func ValidateUniqueObjectKeys(payload string) error {
	decoder := json.NewDecoder(strings.NewReader(payload))
	if err := validateJSONValueKeys(decoder); err != nil {
		return err
	}
	if _, err := decoder.Token(); !errors.Is(err, io.EOF) {
		return ErrInvalidPayload
	}
	return nil
}

func validateJSONValueKeys(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidPayload, err)
	}
	delimiter, composite := token.(json.Delim)
	if !composite {
		return nil
	}
	switch delimiter {
	case '{':
		keys := make(map[string]struct{})
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return fmt.Errorf("%w: %v", ErrInvalidPayload, err)
			}
			key, ok := keyToken.(string)
			if !ok {
				return ErrInvalidPayload
			}
			foldedKey := strings.ToLower(key)
			if _, duplicate := keys[foldedKey]; duplicate {
				return fmt.Errorf("%w: duplicate object key %q", ErrInvalidPayload, key)
			}
			keys[foldedKey] = struct{}{}
			if err := validateJSONValueKeys(decoder); err != nil {
				return err
			}
		}
		end, err := decoder.Token()
		if err != nil || end != json.Delim('}') {
			return ErrInvalidPayload
		}
	case '[':
		for decoder.More() {
			if err := validateJSONValueKeys(decoder); err != nil {
				return err
			}
		}
		end, err := decoder.Token()
		if err != nil || end != json.Delim(']') {
			return ErrInvalidPayload
		}
	default:
		return ErrInvalidPayload
	}
	return nil
}
