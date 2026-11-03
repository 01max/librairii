package preferences

import (
	"github.com/01max/librairii/internal/metadata"
	"testing"
)

func TestInterfaceLanguageDoesNotChangeCatalogLocale(t *testing.T) {
	locale := metadata.DefaultLocale
	store := NewStore(t.TempDir())
	for _, language := range []string{"fr", "en"} {
		if _, err := store.Update(func(p *Profile) error { p.Language = language; return nil }); err != nil {
			t.Fatal(err)
		}
		if metadata.DefaultLocale != locale || metadata.DefaultLocale != "en-GB" {
			t.Fatal("metadata locale followed UI language")
		}
	}
}
