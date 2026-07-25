package exporter

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"sort"
	"testing"

	"github.com/01max/librairii/internal/catalog"
	"github.com/01max/librairii/internal/inspection"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/storage"
)

func TestExportedSyntheticArchivesRemainLuniiQTCompatible(t *testing.T) {
	t.Parallel()

	fixtures := luniiQTCompatibilityFixtures(t)
	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	copier, err := NewCopier(layout)
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	inspector := inspection.NewStoryInspector()
	expectedFiles := make([]string, 0, len(fixtures))

	for index, fixture := range fixtures {
		fixture := fixture
		t.Run(string(fixture.archive.ExpectedFormat), func(t *testing.T) {
			story := writeManagedExportStory(
				t,
				layout,
				int64(index+1),
				fixture.archive.Filename,
				fixture.bytes,
			)
			story.DetectedFormat = string(fixture.archive.ExpectedFormat)
			result, err := copier.Copy(
				context.Background(),
				exportNewItem(story),
				destination,
				nil,
			)
			if err != nil {
				t.Fatalf("Copy() error = %v", err)
			}
			if result.OutputName != fixture.archive.Filename ||
				result.SHA256 != story.SHA256 ||
				result.ByteSize != story.ByteSize {
				t.Fatalf("Copy() = %#v, story = %#v", result, story)
			}

			exportedPath := filepath.Join(destination, fixture.archive.Filename)
			exportedBytes, err := os.ReadFile(exportedPath)
			if err != nil {
				t.Fatal(err)
			}
			sum := sha256.Sum256(exportedBytes)
			if hex.EncodeToString(sum[:]) != story.SHA256 {
				t.Fatalf("exported checksum differs from managed checksum")
			}

			// Native selection and drag-and-drop converge on this inspection
			// boundary, so exercise the exported path as both ingress modes.
			for _, ingress := range []string{"native-selection", "drag-and-drop"} {
				inspected, err := inspector.Inspect(
					context.Background(),
					inspection.Candidate{
						Path:             exportedPath,
						OriginalFilename: filepath.Base(exportedPath),
					},
					inspection.DefaultLimits(),
				)
				if err != nil {
					t.Fatalf("%s inspection error = %v", ingress, err)
				}
				if inspected.UUID != fixture.archive.ExpectedUUID ||
					inspected.Format != fixture.archive.ExpectedFormat {
					t.Fatalf("%s inspection = %#v", ingress, inspected)
				}
			}
		})
		expectedFiles = append(expectedFiles, fixture.archive.Filename)
	}

	entries, err := os.ReadDir(destination)
	if err != nil {
		t.Fatal(err)
	}
	actualFiles := make([]string, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() {
			t.Fatalf("export added directory %q", entry.Name())
		}
		actualFiles = append(actualFiles, entry.Name())
	}
	sort.Strings(actualFiles)
	sort.Strings(expectedFiles)
	if len(actualFiles) != len(expectedFiles) {
		t.Fatalf("destination files = %q, want %q", actualFiles, expectedFiles)
	}
	for index := range expectedFiles {
		if actualFiles[index] != expectedFiles[index] {
			t.Fatalf("destination files = %q, want %q", actualFiles, expectedFiles)
		}
	}
}

type luniiQTCompatibilityFixture struct {
	archive testfixture.Archive
	bytes   []byte
}

func luniiQTCompatibilityFixtures(t *testing.T) []luniiQTCompatibilityFixture {
	t.Helper()

	zipArchives := []testfixture.Archive{
		testfixture.PlainPK(),
		testfixture.V1PK(),
		testfixture.V2PK(),
		testfixture.GenericPK(),
		testfixture.GenericZIP(),
		testfixture.StudioZIP(),
	}
	fixtures := make([]luniiQTCompatibilityFixture, 0, len(zipArchives)+1)
	for _, archive := range zipArchives {
		bytes, err := testfixture.ZIPBytes(archive)
		if err != nil {
			t.Fatal(err)
		}
		fixtures = append(fixtures, luniiQTCompatibilityFixture{
			archive: archive,
			bytes:   bytes,
		})
	}
	sevenZIPArchive, err := testfixture.SevenZIPArchive(testfixture.SevenZIPGeneric)
	if err != nil {
		t.Fatal(err)
	}
	sevenZIPBytes, err := testfixture.SevenZIPBytes(testfixture.SevenZIPGeneric)
	if err != nil {
		t.Fatal(err)
	}
	fixtures = append(fixtures, luniiQTCompatibilityFixture{
		archive: sevenZIPArchive,
		bytes:   sevenZIPBytes,
	})

	formats := make(map[catalog.ArchiveFormat]struct{}, len(fixtures))
	for _, fixture := range fixtures {
		formats[fixture.archive.ExpectedFormat] = struct{}{}
	}
	if len(formats) != 7 {
		t.Fatalf("compatibility fixtures cover %d formats, want 7", len(formats))
	}
	return fixtures
}
