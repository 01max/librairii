package searchtext

import (
	"strings"
	"unicode"

	"golang.org/x/text/cases"
	"golang.org/x/text/unicode/norm"
)

//go:generate go run ./cmd/generate-casefold -output ../../frontend/src/search-casefold.json

func Normalize(value string) string {
	var builder strings.Builder
	space := false
	for _, character := range norm.NFKD.String(cases.Fold().String(strings.TrimSpace(value))) {
		switch {
		case unicode.Is(unicode.Mn, character):
			continue
		case unicode.IsSpace(character):
			space = builder.Len() > 0
		default:
			if space {
				builder.WriteByte(' ')
				space = false
			}
			builder.WriteRune(character)
		}
	}
	return builder.String()
}
