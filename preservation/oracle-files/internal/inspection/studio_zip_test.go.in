package inspection

import (
	"archive/zip"
	"encoding/json"
	"testing"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/inspection/testfixture"
)

func TestZIPInspectorRecognizesStudioArchive(t *testing.T) {
	t.Parallel()

	result, err := inspectFixture(t, testfixture.StudioZIP(), DefaultLimits())
	if err != nil {
		t.Fatalf("Inspect() error = %v", err)
	}
	if result.UUID != testfixture.StoryUUID ||
		result.Format != catalog.FormatStudioZIP ||
		result.Metadata.Title != testfixture.StoryTitle ||
		result.Metadata.Description == "" ||
		result.Metadata.Artwork == nil ||
		result.Metadata.Artwork.EntryName != "thumbnail.png" {
		t.Fatalf("Inspect() = %#v", result)
	}
}

func TestStudioInspectionRequiresStoryAndThumbnail(t *testing.T) {
	t.Parallel()

	withoutStory := testfixture.WithoutEntry(testfixture.StudioZIP(), "story.json")
	if _, err := inspectFixture(t, withoutStory, DefaultLimits()); !ErrorHasCode(err, CodeInvalidUUID) {
		t.Fatalf("Inspect(without story.json) error = %v", err)
	}

	withoutThumbnail := testfixture.WithoutEntry(testfixture.StudioZIP(), "thumbnail.png")
	if _, err := inspectFixture(t, withoutThumbnail, DefaultLimits()); !ErrorHasCode(err, CodeMissingEntry) {
		t.Fatalf("Inspect(without thumbnail.png) error = %v", err)
	}
}

func TestStudioInspectionValidatesUUIDAndJSON(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name  string
		story []byte
		code  ErrorCode
	}{
		{name: "malformed", story: []byte("{"), code: CodeMalformedMetadata},
		{
			name: "invalid UUID",
			story: studioJSON(t, map[string]any{
				"uuid":       "not-a-uuid",
				"stageNodes": validStudioNodes(),
			}),
			code: CodeInvalidUUID,
		},
		{
			name: "no stage nodes",
			story: studioJSON(t, map[string]any{
				"uuid":       testfixture.StoryUUID,
				"stageNodes": []any{},
			}),
			code: CodeMissingAsset,
		},
		{
			name: "no asset references",
			story: studioJSON(t, map[string]any{
				"uuid":       testfixture.StoryUUID,
				"stageNodes": []map[string]string{{}},
			}),
			code: CodeMissingAsset,
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			fixture := testfixture.ReplaceEntry(
				testfixture.StudioZIP(),
				testfixture.Entry{Name: "story.json", Bytes: test.story, Method: zip.Deflate},
			)
			if _, err := inspectFixture(t, fixture, DefaultLimits()); !ErrorHasCode(err, test.code) {
				t.Fatalf("Inspect() error = %v, want %q", err, test.code)
			}
		})
	}
}

func TestStudioInspectionValidatesEveryAssetReference(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name      string
		reference string
		code      ErrorCode
	}{
		{name: "missing", reference: "assets/missing.mp3", code: CodeMissingAsset},
		{name: "parent escape", reference: "../outside.mp3", code: CodeUnsafePath},
		{name: "absolute", reference: "/outside.mp3", code: CodeUnsafePath},
		{name: "outside assets", reference: "outside.mp3", code: CodeUnsafePath},
		{name: "backslash", reference: `assets\outside.mp3`, code: CodeUnsafePath},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			story := studioJSON(t, map[string]any{
				"uuid": testfixture.StoryUUID,
				"stageNodes": []map[string]string{
					{"audio": test.reference},
				},
			})
			fixture := testfixture.ReplaceEntry(
				testfixture.StudioZIP(),
				testfixture.Entry{Name: "story.json", Bytes: story, Method: zip.Deflate},
			)
			if _, err := inspectFixture(t, fixture, DefaultLimits()); !ErrorHasCode(err, test.code) {
				t.Fatalf("Inspect() error = %v, want %q", err, test.code)
			}
		})
	}
}

func TestStudioInspectionEnforcesMetadataAndArtworkLimits(t *testing.T) {
	t.Parallel()

	metadataLimits := DefaultLimits()
	metadataLimits.MaxMetadataBytes = 8
	if _, err := inspectFixture(
		t,
		testfixture.StudioZIP(),
		metadataLimits,
	); !ErrorHasCode(err, CodeMetadataLimit) {
		t.Fatalf("Inspect(metadata limit) error = %v", err)
	}

	artworkLimits := DefaultLimits()
	artworkLimits.MaxArtworkBytes = 8
	if _, err := inspectFixture(
		t,
		testfixture.StudioZIP(),
		artworkLimits,
	); !ErrorHasCode(err, CodeArtworkLimit) {
		t.Fatalf("Inspect(artwork limit) error = %v", err)
	}

	invalidThumbnail := testfixture.ReplaceEntry(
		testfixture.StudioZIP(),
		testfixture.Entry{
			Name:   "thumbnail.png",
			Bytes:  []byte("not a PNG"),
			Method: zip.Store,
		},
	)
	if _, err := inspectFixture(
		t,
		invalidThumbnail,
		DefaultLimits(),
	); !ErrorHasCode(err, CodeArtworkLimit) {
		t.Fatalf("Inspect(invalid thumbnail) error = %v", err)
	}
}

func TestStudioStructureRequiresZIPFilename(t *testing.T) {
	t.Parallel()

	fixture := testfixture.StudioZIP()
	fixture.Filename = "studio.pk"
	if _, err := inspectFixture(t, fixture, DefaultLimits()); !ErrorHasCode(err, CodeUnsupportedFormat) {
		t.Fatalf("Inspect(studio.pk) error = %v", err)
	}
}

func validStudioNodes() []map[string]string {
	return []map[string]string{
		{
			"image": "assets/cover.png",
			"audio": "assets/intro.mp3",
		},
	}
}

func studioJSON(t *testing.T, value any) []byte {
	t.Helper()
	bytes, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return bytes
}
