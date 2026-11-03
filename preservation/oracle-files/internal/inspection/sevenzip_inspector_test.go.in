package inspection

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/inspection/testfixture"
)

func TestSevenZIPInspectorMatchesZIPValidationResults(t *testing.T) {
	t.Parallel()

	zipResult, err := inspectFixture(t, testfixture.GenericZIP(), DefaultLimits())
	if err != nil {
		t.Fatalf("ZIP Inspect() error = %v", err)
	}
	sevenResult, err := inspectSevenFixture(
		t,
		testfixture.SevenZIPGeneric,
		DefaultLimits(),
	)
	if err != nil {
		t.Fatalf("7z Inspect() error = %v", err)
	}
	if sevenResult.UUID != zipResult.UUID ||
		sevenResult.Format != catalog.FormatSevenZIP {
		t.Fatalf("ZIP result = %#v, 7z result = %#v", zipResult, sevenResult)
	}
}

func TestSevenZIPInspectorReadsStudioMetadata(t *testing.T) {
	t.Parallel()

	zipResult, err := inspectFixture(t, testfixture.StudioZIP(), DefaultLimits())
	if err != nil {
		t.Fatalf("ZIP Inspect() error = %v", err)
	}
	sevenResult, err := inspectSevenFixture(
		t,
		testfixture.SevenZIPStudio,
		DefaultLimits(),
	)
	if err != nil {
		t.Fatalf("7z Inspect() error = %v", err)
	}
	if sevenResult.UUID != zipResult.UUID ||
		sevenResult.Format != catalog.FormatSevenZIP ||
		sevenResult.Metadata.Title != zipResult.Metadata.Title ||
		sevenResult.Metadata.Description != zipResult.Metadata.Description ||
		sevenResult.Metadata.Artwork == nil {
		t.Fatalf("ZIP result = %#v, 7z result = %#v", zipResult, sevenResult)
	}
}

func TestSevenZIPInspectorRejectsInvalidContainerAndExtension(t *testing.T) {
	t.Parallel()

	directory := t.TempDir()
	notSevenZIP := filepath.Join(directory, "story.7z")
	if err := os.WriteFile(notSevenZIP, []byte("not a 7z"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := NewSevenZIPInspector().Inspect(context.Background(), Candidate{
		Path:             notSevenZIP,
		OriginalFilename: "story.7z",
	}, DefaultLimits())
	if !ErrorHasCode(err, CodeInvalidContainer) {
		t.Fatalf("Inspect(not 7z) error = %v", err)
	}

	validPath, err := testfixture.WriteSevenZIP(
		directory,
		testfixture.SevenZIPGeneric,
	)
	if err != nil {
		t.Fatal(err)
	}
	_, err = NewSevenZIPInspector().Inspect(context.Background(), Candidate{
		Path:             validPath,
		OriginalFilename: "story.zip",
	}, DefaultLimits())
	if !ErrorHasCode(err, CodeUnsupportedFormat) {
		t.Fatalf("Inspect(extension mismatch) error = %v", err)
	}
}

func TestSevenZIPInspectorRejectsUnsafeSpecialAndNestedEntries(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		fixture testfixture.SevenZIPFixture
		code    ErrorCode
	}{
		{
			name:    "deep path",
			fixture: testfixture.SevenZIPDeepPath,
			code:    CodeUnsafePath,
		},
		{
			name:    "symbolic link",
			fixture: testfixture.SevenZIPSymlink,
			code:    CodeLinkEntry,
		},
		{
			name:    "nested archive",
			fixture: testfixture.SevenZIPNested,
			code:    CodeNestedArchive,
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			if _, err := inspectSevenFixture(
				t,
				test.fixture,
				DefaultLimits(),
			); !ErrorHasCode(err, test.code) {
				t.Fatalf("Inspect() error = %v, want %q", err, test.code)
			}
		})
	}
}

func TestSevenZIPInspectorEnforcesEquivalentLimits(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		fixture testfixture.SevenZIPFixture
		limits  func() Limits
		code    ErrorCode
	}{
		{
			name:    "entry count",
			fixture: testfixture.SevenZIPGeneric,
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxEntries = 1
				return limits
			},
			code: CodeEntryLimit,
		},
		{
			name:    "expanded bytes",
			fixture: testfixture.SevenZIPGeneric,
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxExpandedBytes = 1
				return limits
			},
			code: CodeExpandedSizeLimit,
		},
		{
			name:    "compression ratio",
			fixture: testfixture.SevenZIPBomb,
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxCompressionRatio = 2
				return limits
			},
			code: CodeCompressionRatioLimit,
		},
		{
			name:    "metadata bytes",
			fixture: testfixture.SevenZIPStudio,
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxMetadataBytes = 8
				return limits
			},
			code: CodeMetadataLimit,
		},
		{
			name:    "artwork bytes",
			fixture: testfixture.SevenZIPStudio,
			limits: func() Limits {
				limits := DefaultLimits()
				limits.MaxArtworkBytes = 8
				return limits
			},
			code: CodeArtworkLimit,
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			if _, err := inspectSevenFixture(
				t,
				test.fixture,
				test.limits(),
			); !ErrorHasCode(err, test.code) {
				t.Fatalf("Inspect() error = %v, want %q", err, test.code)
			}
		})
	}
}

func TestSevenZIPInspectorValidatesRequiredStructureAndCancellation(t *testing.T) {
	t.Parallel()

	if _, err := inspectSevenFixture(
		t,
		testfixture.SevenZIPMissing,
		DefaultLimits(),
	); !ErrorHasCode(err, CodeMissingEntry) {
		t.Fatalf("Inspect(missing entry) error = %v", err)
	}

	path, err := testfixture.WriteSevenZIP(
		t.TempDir(),
		testfixture.SevenZIPGeneric,
	)
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err = NewSevenZIPInspector().Inspect(ctx, Candidate{
		Path:             path,
		OriginalFilename: filepath.Base(path),
	}, DefaultLimits())
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("Inspect(cancelled) error = %v", err)
	}
}

func TestSevenZIPInspectorDoesNotExtractEntries(t *testing.T) {
	t.Parallel()

	directory := t.TempDir()
	path, err := testfixture.WriteSevenZIP(
		directory,
		testfixture.SevenZIPGeneric,
	)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := NewStoryInspector().Inspect(context.Background(), Candidate{
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

func inspectSevenFixture(
	t *testing.T,
	fixture testfixture.SevenZIPFixture,
	limits Limits,
) (Result, error) {
	t.Helper()
	path, err := testfixture.WriteSevenZIP(t.TempDir(), fixture)
	if err != nil {
		t.Fatal(err)
	}
	archive, err := testfixture.SevenZIPArchive(fixture)
	if err != nil {
		t.Fatal(err)
	}
	return NewSevenZIPInspector().Inspect(context.Background(), Candidate{
		Path:             path,
		OriginalFilename: archive.Filename,
	}, limits)
}
