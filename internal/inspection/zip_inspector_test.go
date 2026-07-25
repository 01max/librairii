package inspection

import (
	"archive/zip"
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/inspection/testfixture"
)

func TestZIPInspectorRecognizesSupportedLuniiFamilies(t *testing.T) {
	t.Parallel()

	fixtures := []testfixture.Archive{
		testfixture.PlainPK(),
		testfixture.V1PK(),
		testfixture.V2PK(),
		testfixture.GenericPK(),
		testfixture.GenericZIP(),
	}
	for _, fixture := range fixtures {
		fixture := fixture
		t.Run(string(fixture.ExpectedFormat), func(t *testing.T) {
			t.Parallel()
			result, err := inspectFixture(t, fixture, DefaultLimits())
			if err != nil {
				t.Fatalf("Inspect() error = %v", err)
			}
			if result.UUID != fixture.ExpectedUUID || result.Format != fixture.ExpectedFormat {
				t.Fatalf("Inspect() = %#v", result)
			}
		})
	}
}

func TestZIPInspectorReadsPermittedPlainMetadata(t *testing.T) {
	t.Parallel()

	result, err := inspectFixture(t, testfixture.PlainPK(), DefaultLimits())
	if err != nil {
		t.Fatal(err)
	}
	if result.Metadata.Title != testfixture.StoryTitle ||
		result.Metadata.Description == "" ||
		result.Metadata.Artwork == nil ||
		result.Metadata.Artwork.MediaType != "image/png" {
		t.Fatalf("metadata = %#v", result.Metadata)
	}

	withoutOptional := testfixture.WithoutEntry(testfixture.PlainPK(), "_metadata.json")
	withoutOptional = testfixture.WithoutEntry(withoutOptional, "_thumbnail.png")
	result, err = inspectFixture(t, withoutOptional, DefaultLimits())
	if err != nil {
		t.Fatalf("Inspect(without optional metadata) error = %v", err)
	}
	if result.Metadata != (Metadata{}) {
		t.Fatalf("metadata = %#v", result.Metadata)
	}
}

func TestZIPInspectorRejectsInvalidContainerAndExtension(t *testing.T) {
	t.Parallel()

	directory := t.TempDir()
	notZIP := filepath.Join(directory, "story.zip")
	if err := os.WriteFile(notZIP, []byte("not a ZIP"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := NewZIPInspector().Inspect(context.Background(), Candidate{
		Path:             notZIP,
		OriginalFilename: "story.zip",
	}, DefaultLimits())
	if !ErrorHasCode(err, CodeInvalidContainer) {
		t.Fatalf("Inspect(not ZIP) error = %v", err)
	}

	validPath, err := testfixture.WriteZIP(directory, testfixture.GenericZIP())
	if err != nil {
		t.Fatal(err)
	}
	_, err = NewZIPInspector().Inspect(context.Background(), Candidate{
		Path:             validPath,
		OriginalFilename: "story.7z",
	}, DefaultLimits())
	if !ErrorHasCode(err, CodeUnsupportedFormat) {
		t.Fatalf("Inspect(extension mismatch) error = %v", err)
	}
}

func TestZIPInspectorRejectsMissingEntriesAndAmbiguousUUIDs(t *testing.T) {
	t.Parallel()

	missing := testfixture.WithoutEntry(testfixture.V2PK(), "00112233445546778899aabbccddeeff/si")
	if _, err := inspectFixture(t, missing, DefaultLimits()); !ErrorHasCode(err, CodeMissingEntry) {
		t.Fatalf("Inspect(missing entry) error = %v", err)
	}

	ambiguous := testfixture.WithEntries(
		testfixture.GenericZIP(),
		testfixture.Entry{
			Name:   "11112222-3333-4444-8555-666677778888/ni",
			Bytes:  []byte("another pack"),
			Method: zip.Store,
		},
	)
	if _, err := inspectFixture(t, ambiguous, DefaultLimits()); !ErrorHasCode(err, CodeAmbiguousStructure) {
		t.Fatalf("Inspect(ambiguous roots) error = %v", err)
	}
}

func TestZIPInspectorRejectsUnsafeAndDuplicatePaths(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		archive testfixture.Archive
		code    ErrorCode
	}{
		{
			name: "parent path",
			archive: testfixture.WithEntries(
				testfixture.GenericZIP(),
				testfixture.Entry{Name: "../outside", Bytes: []byte("escape"), Method: zip.Store},
			),
			code: CodeUnsafePath,
		},
		{
			name: "backslash path",
			archive: testfixture.WithEntries(
				testfixture.GenericZIP(),
				testfixture.Entry{Name: `..\outside`, Bytes: []byte("escape"), Method: zip.Store},
			),
			code: CodeUnsafePath,
		},
		{
			name: "symbolic link",
			archive: testfixture.WithEntries(
				testfixture.GenericZIP(),
				testfixture.Entry{
					Name:   "link",
					Bytes:  []byte("../outside"),
					Method: zip.Store,
					Mode:   os.ModeSymlink | 0o777,
				},
			),
			code: CodeLinkEntry,
		},
		{
			name: "nested archive",
			archive: testfixture.WithEntries(
				testfixture.GenericZIP(),
				testfixture.Entry{Name: "nested.zip", Bytes: []byte("nested"), Method: zip.Store},
			),
			code: CodeNestedArchive,
		},
		{
			name: "duplicate path",
			archive: testfixture.WithEntries(
				testfixture.GenericZIP(),
				testfixture.GenericZIP().Entries[0],
			),
			code: CodeAmbiguousStructure,
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			if _, err := inspectFixture(t, test.archive, DefaultLimits()); !ErrorHasCode(err, test.code) {
				t.Fatalf("Inspect() error = %v, want %q", err, test.code)
			}
		})
	}
}

func TestZIPInspectorEnforcesConfiguredResourceLimits(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		archive testfixture.Archive
		limits  func() Limits
		code    ErrorCode
	}{
		{
			name:    "entry count",
			archive: testfixture.PlainPK(),
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxEntries = 1
				return limits
			},
			code: CodeEntryLimit,
		},
		{
			name:    "expanded bytes",
			archive: testfixture.PlainPK(),
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxExpandedBytes = 1
				return limits
			},
			code: CodeExpandedSizeLimit,
		},
		{
			name: "compression ratio",
			archive: testfixture.WithEntries(
				testfixture.GenericZIP(),
				testfixture.Entry{
					Name:   "padding.bin",
					Bytes:  []byte(strings.Repeat("0", 64<<10)),
					Method: zip.Deflate,
				},
			),
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxCompressionRatio = 2
				return limits
			},
			code: CodeCompressionRatioLimit,
		},
		{
			name:    "metadata bytes",
			archive: testfixture.PlainPK(),
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxMetadataBytes = 4
				return limits
			},
			code: CodeMetadataLimit,
		},
		{
			name:    "artwork bytes",
			archive: testfixture.PlainPK(),
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxArtworkBytes = 4
				return limits
			},
			code: CodeArtworkLimit,
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			if _, err := inspectFixture(t, test.archive, test.limits()); !ErrorHasCode(err, test.code) {
				t.Fatalf("Inspect() error = %v, want %q", err, test.code)
			}
		})
	}
}

func TestZIPInspectorRejectsInvalidUUIDAndMetadata(t *testing.T) {
	t.Parallel()

	invalidUUID := testfixture.ReplaceEntry(
		testfixture.PlainPK(),
		testfixture.Entry{Name: "uuid.bin", Bytes: []byte("short"), Method: zip.Store},
	)
	if _, err := inspectFixture(t, invalidUUID, DefaultLimits()); !ErrorHasCode(err, CodeInvalidUUID) {
		t.Fatalf("Inspect(invalid UUID) error = %v", err)
	}

	mismatchedMetadata, err := json.Marshal(map[string]string{
		"uuid": "11112222-3333-4444-8555-666677778888",
	})
	if err != nil {
		t.Fatal(err)
	}
	mismatch := testfixture.ReplaceEntry(
		testfixture.PlainPK(),
		testfixture.Entry{Name: "_metadata.json", Bytes: mismatchedMetadata, Method: zip.Deflate},
	)
	if _, err := inspectFixture(t, mismatch, DefaultLimits()); !ErrorHasCode(err, CodeInvalidUUID) {
		t.Fatalf("Inspect(mismatched metadata) error = %v", err)
	}

	malformed := testfixture.ReplaceEntry(
		testfixture.PlainPK(),
		testfixture.Entry{Name: "_metadata.json", Bytes: []byte("{"), Method: zip.Deflate},
	)
	if _, err := inspectFixture(t, malformed, DefaultLimits()); !ErrorHasCode(err, CodeMalformedMetadata) {
		t.Fatalf("Inspect(malformed metadata) error = %v", err)
	}
}

func TestZIPInspectorHonorsCancellation(t *testing.T) {
	t.Parallel()

	path, err := testfixture.WriteZIP(t.TempDir(), testfixture.PlainPK())
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err = NewZIPInspector().Inspect(ctx, Candidate{
		Path:             path,
		OriginalFilename: filepath.Base(path),
	}, DefaultLimits())
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("Inspect(cancelled) error = %v", err)
	}
}

func TestZIPInspectorDoesNotExtractEntries(t *testing.T) {
	t.Parallel()

	directory := t.TempDir()
	path, err := testfixture.WriteZIP(directory, testfixture.GenericZIP())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := NewZIPInspector().Inspect(context.Background(), Candidate{
		Path:             path,
		OriginalFilename: filepath.Base(path),
	}, DefaultLimits()); err != nil {
		t.Fatal(err)
	}
	entries, err := os.ReadDir(directory)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Name() != filepath.Base(path) {
		t.Fatalf("inspection created filesystem entries: %#v", entries)
	}
}

func inspectFixture(
	t *testing.T,
	fixture testfixture.Archive,
	limits Limits,
) (Result, error) {
	t.Helper()
	path, err := testfixture.WriteZIP(t.TempDir(), fixture)
	if err != nil {
		t.Fatal(err)
	}
	return NewZIPInspector().Inspect(context.Background(), Candidate{
		Path:             path,
		OriginalFilename: fixture.Filename,
	}, limits)
}
