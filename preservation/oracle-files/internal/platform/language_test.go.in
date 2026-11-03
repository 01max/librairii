package platform

import "testing"

func TestPreferredLanguage(t *testing.T) {
	for input, want := range map[string]string{"fr_FR.UTF-8": "fr", "fr-CA": "fr", "en-GB": "en", "de_DE": "en", "": "en"} {
		if got := PreferredLanguage(input); got != want {
			t.Fatal(input, got, want)
		}
	}
}
