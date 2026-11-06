package testfixture

import (
	"archive/zip"
	"bytes"
	"testing"

	"github.com/01max/librairii/internal/catalog"
	sevenzip "github.com/bodgit/sevenzip"
)

func TestSyntheticFixtureFamilies(t *testing.T) {
	t.Parallel()

	fixtures := []Archive{
		PlainPK(),
		V1PK(),
		V2PK(),
		GenericPK(),
		GenericZIP(),
		StudioZIP(),
	}
	formats := map[catalog.ArchiveFormat]bool{}
	for _, fixture := range fixtures {
		fixture := fixture
		t.Run(string(fixture.ExpectedFormat), func(t *testing.T) {
			t.Parallel()
			if fixture.ExpectedUUID != StoryUUID {
				t.Fatalf("ExpectedUUID = %q", fixture.ExpectedUUID)
			}
			archiveBytes, err := ZIPBytes(fixture)
			if err != nil {
				t.Fatalf("ZIPBytes() error = %v", err)
			}
			reader, err := zip.NewReader(bytes.NewReader(archiveBytes), int64(len(archiveBytes)))
			if err != nil {
				t.Fatalf("zip.NewReader() error = %v", err)
			}
			if len(reader.File) != len(fixture.Entries) {
				t.Fatalf("ZIP contains %d entries, want %d", len(reader.File), len(fixture.Entries))
			}
		})
		formats[fixture.ExpectedFormat] = true
	}

	for _, format := range []catalog.ArchiveFormat{
		catalog.FormatPlainPK,
		catalog.FormatV1PK,
		catalog.FormatV2PK,
		catalog.FormatGenericPK,
		catalog.FormatZIP,
		catalog.FormatStudioZIP,
	} {
		if !formats[format] {
			t.Fatalf("missing synthetic fixture for %q", format)
		}
	}

	sevenZip := GenericSevenZIP()
	if sevenZip.ExpectedFormat != catalog.FormatSevenZIP ||
		sevenZip.ExpectedUUID != StoryUUID ||
		len(sevenZip.Entries) == 0 {
		t.Fatalf("GenericSevenZIP() = %#v", sevenZip)
	}
	sevenZipBytes, err := SevenZIPBytes(SevenZIPGeneric)
	if err != nil {
		t.Fatalf("SevenZIPBytes() error = %v", err)
	}
	sevenZipReader, err := sevenzip.NewReader(
		bytes.NewReader(sevenZipBytes),
		int64(len(sevenZipBytes)),
	)
	if err != nil {
		t.Fatalf("sevenzip.NewReader() error = %v", err)
	}
	if len(sevenZipReader.File) != len(sevenZip.Entries) {
		t.Fatalf(
			"7z contains %d entries, want %d",
			len(sevenZipReader.File),
			len(sevenZip.Entries),
		)
	}

	studioSevenZip := StudioSevenZIP()
	if studioSevenZip.ExpectedFormat != catalog.FormatSevenZIP ||
		studioSevenZip.ExpectedUUID != StoryUUID {
		t.Fatalf("StudioSevenZIP() = %#v", studioSevenZip)
	}
}

func TestFixtureMutatorsDoNotChangeOriginal(t *testing.T) {
	t.Parallel()

	original := PlainPK()
	mutated := WithoutEntry(original, "uuid.bin")
	mutated = WithEntries(mutated, Entry{Name: "../unsafe", Bytes: []byte("unsafe")})
	if len(original.Entries) != 9 {
		t.Fatalf("original entry count = %d", len(original.Entries))
	}
	if len(mutated.Entries) != 9 {
		t.Fatalf("mutated entry count = %d", len(mutated.Entries))
	}
}
