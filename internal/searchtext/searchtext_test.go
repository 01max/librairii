package searchtext

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"unicode/utf8"

	"golang.org/x/text/cases"
)

func TestNormalizeFoldsCaseAccentsAndWhitespaceButKeepsLiteralWildcards(t *testing.T) {
	t.Parallel()

	got := Normalize("  ÉTÉ\t100%_Magique  ")
	if got != "ete 100%_magique" {
		t.Fatalf("Normalize() = %q", got)
	}
}

func TestNormalizeMatchesSharedUnicodeFixtures(t *testing.T) {
	t.Parallel()
	data, err := os.ReadFile("testdata/normalization.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixtures []struct {
		Input      string `json:"input"`
		Normalized string `json:"normalized"`
	}
	if err := json.Unmarshal(data, &fixtures); err != nil {
		t.Fatal(err)
	}
	for _, fixture := range fixtures {
		if got := Normalize(fixture.Input); got != fixture.Normalized {
			t.Errorf("Normalize(%q) = %q, want %q", fixture.Input, got, fixture.Normalized)
		}
	}
}

func TestFrontendCaseFoldMapMatchesGo(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "frontend", "src", "search-casefold.json"))
	if err != nil {
		t.Fatal(err)
	}
	var mapping map[string]string
	if err := json.Unmarshal(data, &mapping); err != nil {
		t.Fatal(err)
	}
	fold := cases.Fold()
	for character := rune(0); character <= utf8.MaxRune; character++ {
		if character >= 0xd800 && character <= 0xdfff {
			continue
		}
		original := string(character)
		want := fold.String(original)
		got, mapped := mapping[original]
		if want == original {
			if mapped {
				t.Fatalf("unneeded fold for %q", original)
			}
		} else if !mapped || got != want {
			t.Fatalf("fold for %q = %q, want %q", original, got, want)
		}
	}
}
