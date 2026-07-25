package metadata

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"strings"
	"testing"
)

func TestNormalizeCatalogSelectsLocaleAndNormalizesSupportedFields(t *testing.T) {
	t.Parallel()

	catalog, err := NormalizeCatalogSnapshot(readCatalogFixture(t), "en_GB")
	if err != nil {
		t.Fatal(err)
	}
	stories := catalog.Stories
	if len(stories) != 1 {
		t.Fatalf("NormalizeCatalogSnapshot() story count = %d", len(stories))
	}
	story := stories[0]
	if story.StoryUUID != "123e4567-e89b-42d3-a456-426614174000" ||
		story.Title != "The Clockwork Mountain" ||
		story.Author != "A. Example" ||
		story.Publisher != "Fixture Press" ||
		story.Language != "en-GB" ||
		story.MinimumAge == nil ||
		*story.MinimumAge != 3 ||
		story.MaximumAge == nil ||
		*story.MaximumAge != 5 ||
		story.DurationSeconds == nil ||
		*story.DurationSeconds != 3240 ||
		story.SourceUpdatedAt == nil ||
		story.Provenance != ProvenanceLuniiCatalog ||
		len(catalog.Artworks) != 1 ||
		story.ArtworkID != catalog.Artworks[0].ID {
		t.Fatalf("NormalizeCatalogSnapshot() = %#v", catalog)
	}
	sourceURL := "https://storage.googleapis.com/lunii-data-prod/fixture/clockwork-mountain.png"
	digest := sha256.Sum256([]byte(sourceURL))
	if catalog.Artworks[0].SourceURL != sourceURL ||
		catalog.Artworks[0].ID != hex.EncodeToString(digest[:]) {
		t.Fatalf("NormalizeCatalogSnapshot() artwork = %#v", catalog.Artworks[0])
	}
}

func TestNormalizeCatalogRejectsCorruptOrInconsistentPayloads(t *testing.T) {
	t.Parallel()

	validRecord := map[string]any{
		"uuid": "123e4567-e89b-42d3-a456-426614174000",
		"locales_available": map[string]any{
			"en_GB": true,
		},
		"localized_infos": map[string]any{
			"en_GB": map[string]any{
				"title": "Fixture",
			},
		},
	}
	tests := []struct {
		name    string
		payload any
		locale  string
	}{
		{
			name:    "corrupt JSON",
			payload: json.RawMessage(`{"response":`),
			locale:  "en-GB",
		},
		{
			name:    "empty response",
			payload: map[string]any{"response": map[string]any{}},
			locale:  "en-GB",
		},
		{
			name: "incomplete UUID",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid":              "123e4567e89b42d3a456426614174000",
					"locales_available": validRecord["locales_available"],
					"localized_infos":   validRecord["localized_infos"],
				},
			}},
			locale: "en-GB",
		},
		{
			name: "duplicate UUID",
			payload: map[string]any{"response": map[string]any{
				"pack-a": validRecord,
				"pack-b": validRecord,
			}},
			locale: "en-GB",
		},
		{
			name: "undeclared locale",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid": "123e4567-e89b-42d3-a456-426614174000",
					"locales_available": map[string]any{
						"fr_FR": true,
					},
					"localized_infos": validRecord["localized_infos"],
				},
			}},
			locale: "en-GB",
		},
		{
			name: "locale explicitly unavailable",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid": "123e4567-e89b-42d3-a456-426614174000",
					"locales_available": map[string]any{
						"en_GB": false,
					},
					"localized_infos": validRecord["localized_infos"],
				},
			}},
			locale: "en-GB",
		},
		{
			name: "invalid locale availability declaration",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid": "123e4567-e89b-42d3-a456-426614174000",
					"locales_available": map[string]any{
						"en_GB": "yes",
					},
					"localized_infos": validRecord["localized_infos"],
				},
			}},
			locale: "en-GB",
		},
		{
			name: "missing title",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid":              "123e4567-e89b-42d3-a456-426614174000",
					"locales_available": validRecord["locales_available"],
					"localized_infos": map[string]any{
						"en_GB": map[string]any{"description": "Missing title"},
					},
				},
			}},
			locale: "en-GB",
		},
		{
			name: "ambiguous locale",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid":              "123e4567-e89b-42d3-a456-426614174000",
					"locales_available": validRecord["locales_available"],
					"localized_infos": map[string]any{
						"en_GB": map[string]any{"title": "One"},
						"en-GB": map[string]any{"title": "Two"},
					},
				},
			}},
			locale: "en-GB",
		},
		{
			name: "ambiguous age",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid":              "123e4567-e89b-42d3-a456-426614174000",
					"locales_available": validRecord["locales_available"],
					"localized_infos": map[string]any{
						"en_GB": map[string]any{
							"title":       "Fixture",
							"minimum_age": 8,
							"maximum_age": 5,
						},
					},
				},
			}},
			locale: "en-GB",
		},
		{
			name: "external artwork URL",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid":              "123e4567-e89b-42d3-a456-426614174000",
					"locales_available": validRecord["locales_available"],
					"localized_infos": map[string]any{
						"en_GB": map[string]any{
							"title": "Fixture",
							"image": map[string]any{
								"image_url": "https://example.test/secret.png",
							},
						},
					},
				},
			}},
			locale: "en-GB",
		},
		{
			name: "traversing artwork URL",
			payload: map[string]any{"response": map[string]any{
				"pack": map[string]any{
					"uuid":              "123e4567-e89b-42d3-a456-426614174000",
					"locales_available": validRecord["locales_available"],
					"localized_infos": map[string]any{
						"en_GB": map[string]any{
							"title": "Fixture",
							"image": map[string]any{
								"image_url": "/../secret.png",
							},
						},
					},
				},
			}},
			locale: "en-GB",
		},
		{
			name:    "unsupported requested locale",
			payload: map[string]any{"response": map[string]any{"pack": validRecord}},
			locale:  "fr-FR",
		},
	}

	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()

			var payload []byte
			if raw, ok := test.payload.(json.RawMessage); ok {
				payload = raw
			} else {
				var err error
				payload, err = json.Marshal(test.payload)
				if err != nil {
					t.Fatal(err)
				}
			}
			if _, err := NormalizeCatalog(payload, test.locale); !errors.Is(err, ErrInvalidCatalog) {
				t.Fatalf("NormalizeCatalog() error = %v", err)
			}
		})
	}
}

func TestNormalizeCatalogRejectsOversizedText(t *testing.T) {
	t.Parallel()

	payload := map[string]any{"response": map[string]any{
		"pack": map[string]any{
			"uuid": "123e4567-e89b-42d3-a456-426614174000",
			"locales_available": map[string]any{
				"en_GB": true,
			},
			"localized_infos": map[string]any{
				"en_GB": map[string]any{
					"title": strings.Repeat("x", maxTitleBytes+1),
				},
			},
		},
	}}
	body, err := json.Marshal(payload)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := NormalizeCatalog(body, "en-GB"); !errors.Is(err, ErrInvalidCatalog) {
		t.Fatalf("NormalizeCatalog() error = %v", err)
	}
}

func readCatalogFixture(t *testing.T) []byte {
	t.Helper()

	body, err := os.ReadFile("../lunii/testdata/catalog.json")
	if err != nil {
		t.Fatal(err)
	}
	return body
}
