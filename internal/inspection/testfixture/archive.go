// Package testfixture builds deterministic, copyright-free story archives.
package testfixture

import (
	"archive/zip"
	"bytes"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/01max/librairii/internal/catalog"
)

const (
	StoryUUID  = "00112233-4455-4677-8899-aabbccddeeff"
	StoryTitle = "The Clockwork Forest"
)

type Entry struct {
	Name   string
	Bytes  []byte
	Method uint16
	Mode   os.FileMode
}

type Archive struct {
	Filename       string
	ExpectedFormat catalog.ArchiveFormat
	ExpectedUUID   string
	Entries        []Entry
}

func PlainPK() Archive {
	metadata := mustJSON(map[string]string{
		"uuid":        StoryUUID,
		"title":       StoryTitle,
		"description": "A synthetic fixture made only for Librairii tests.",
	})
	return Archive{
		Filename:       "clockwork-forest.plain.pk",
		ExpectedFormat: catalog.FormatPlainPK,
		ExpectedUUID:   StoryUUID,
		Entries: []Entry{
			{Name: "_thumbnail.png", Bytes: PNG(), Method: zip.Deflate},
			{Name: "_metadata.json", Bytes: metadata, Method: zip.Deflate},
			{Name: "uuid.bin", Bytes: UUIDBytes(StoryUUID), Method: zip.Store},
			{Name: "ni", Bytes: []byte("synthetic ni"), Method: zip.Store},
			{Name: "li.plain", Bytes: []byte("synthetic li"), Method: zip.Store},
			{Name: "ri.plain", Bytes: []byte("synthetic ri"), Method: zip.Store},
			{Name: "si.plain", Bytes: []byte("synthetic si"), Method: zip.Store},
			{Name: "rf/000/00000000.bmp", Bytes: []byte("synthetic image asset"), Method: zip.Store},
			{Name: "sf/000/00000000.mp3", Bytes: []byte("synthetic audio asset"), Method: zip.Store},
		},
	}
}

func V1PK() Archive {
	return luniiPack("00112233445546778899aabbccddeeff.v1.pk", catalog.FormatV1PK, false)
}

func V2PK() Archive {
	return luniiPack("00112233445546778899aabbccddeeff.v2.pk", catalog.FormatV2PK, false)
}

func GenericPK() Archive {
	return luniiPack("clockwork-forest.pk", catalog.FormatGenericPK, false)
}

func GenericZIP() Archive {
	return luniiPack("clockwork-forest.zip", catalog.FormatZIP, true)
}

func GenericSevenZIP() Archive {
	archive := luniiPack("clockwork-forest.7z", catalog.FormatSevenZIP, true)
	return archive
}

func StudioZIP() Archive {
	story := mustJSON(map[string]any{
		"uuid":        StoryUUID,
		"title":       StoryTitle,
		"description": "A synthetic STUdio fixture.",
		"stageNodes": []map[string]string{
			{
				"uuid":  "11112222-3333-4444-8555-666677778888",
				"image": "assets/cover.png",
				"audio": "assets/intro.mp3",
			},
		},
	})
	return Archive{
		Filename:       "clockwork-forest-studio.zip",
		ExpectedFormat: catalog.FormatStudioZIP,
		ExpectedUUID:   StoryUUID,
		Entries: []Entry{
			{Name: "story.json", Bytes: story, Method: zip.Deflate},
			{Name: "thumbnail.png", Bytes: PNG(), Method: zip.Deflate},
			{Name: "assets/cover.png", Bytes: PNG(), Method: zip.Deflate},
			{Name: "assets/intro.mp3", Bytes: []byte("synthetic audio asset"), Method: zip.Store},
		},
	}
}

func ZIPBytes(archive Archive) ([]byte, error) {
	var output bytes.Buffer
	writer := zip.NewWriter(&output)
	for _, entry := range archive.Entries {
		header := &zip.FileHeader{
			Name:     entry.Name,
			Method:   entry.Method,
			Modified: time.Date(2000, time.January, 1, 0, 0, 0, 0, time.UTC),
		}
		if entry.Mode != 0 {
			header.SetMode(entry.Mode)
		}
		target, err := writer.CreateHeader(header)
		if err != nil {
			return nil, fmt.Errorf("create synthetic ZIP entry %q: %w", entry.Name, err)
		}
		if _, err := target.Write(entry.Bytes); err != nil {
			return nil, fmt.Errorf("write synthetic ZIP entry %q: %w", entry.Name, err)
		}
	}
	if err := writer.Close(); err != nil {
		return nil, fmt.Errorf("close synthetic ZIP: %w", err)
	}
	return output.Bytes(), nil
}

func WriteZIP(directory string, archive Archive) (string, error) {
	bytes, err := ZIPBytes(archive)
	if err != nil {
		return "", err
	}
	path := filepath.Join(directory, archive.Filename)
	if err := os.WriteFile(path, bytes, 0o600); err != nil {
		return "", fmt.Errorf("write synthetic ZIP fixture: %w", err)
	}
	return path, nil
}

func WithEntries(archive Archive, entries ...Entry) Archive {
	archive.Entries = append(append([]Entry(nil), archive.Entries...), entries...)
	return archive
}

func WithoutEntry(archive Archive, name string) Archive {
	entries := make([]Entry, 0, len(archive.Entries))
	for _, entry := range archive.Entries {
		if entry.Name != name {
			entries = append(entries, entry)
		}
	}
	archive.Entries = entries
	return archive
}

func ReplaceEntry(archive Archive, replacement Entry) Archive {
	archive = WithoutEntry(archive, replacement.Name)
	archive.Entries = append(archive.Entries, replacement)
	return archive
}

func PNG() []byte {
	var output bytes.Buffer
	picture := image.NewNRGBA(image.Rect(0, 0, 2, 2))
	picture.SetNRGBA(0, 0, color.NRGBA{R: 0xE7, G: 0xA4, B: 0x39, A: 0xFF})
	picture.SetNRGBA(1, 0, color.NRGBA{R: 0x1D, G: 0x35, B: 0x32, A: 0xFF})
	picture.SetNRGBA(0, 1, color.NRGBA{R: 0x42, G: 0x75, B: 0xAA, A: 0xFF})
	picture.SetNRGBA(1, 1, color.NRGBA{R: 0xF6, G: 0xEF, B: 0xDD, A: 0xFF})
	if err := png.Encode(&output, picture); err != nil {
		panic(err)
	}
	return output.Bytes()
}

func UUIDBytes(uuid string) []byte {
	decoded, err := hex.DecodeString(strings.ReplaceAll(uuid, "-", ""))
	if err != nil {
		panic(err)
	}
	return decoded
}

func luniiPack(filename string, format catalog.ArchiveFormat, dashedDirectory bool) Archive {
	directory := strings.ReplaceAll(StoryUUID, "-", "")
	if dashedDirectory {
		directory = StoryUUID
	}
	entry := func(name string) string {
		return directory + "/" + name
	}
	return Archive{
		Filename:       filename,
		ExpectedFormat: format,
		ExpectedUUID:   StoryUUID,
		Entries: []Entry{
			{Name: entry("ni"), Bytes: []byte("synthetic ni"), Method: zip.Store},
			{Name: entry("li"), Bytes: []byte("synthetic li"), Method: zip.Store},
			{Name: entry("ri"), Bytes: []byte("synthetic ri"), Method: zip.Store},
			{Name: entry("si"), Bytes: []byte("synthetic si"), Method: zip.Store},
			{Name: entry("rf/000/00000000"), Bytes: []byte("synthetic image asset"), Method: zip.Store},
			{Name: entry("sf/000/00000000"), Bytes: []byte("synthetic audio asset"), Method: zip.Store},
		},
	}
}

func mustJSON(value any) []byte {
	bytes, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return bytes
}
