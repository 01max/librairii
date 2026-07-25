package exporter

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/01max/librairii/internal/storage"
)

func TestCopierPublishesVerifiedArchiveWithoutSidecars(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	content := []byte("byte-preserving story archive")
	story := writeManagedExportStory(
		t,
		layout,
		1,
		"clockwork-forest.zip",
		content,
	)
	copier, err := NewCopier(layout)
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	var progress int64
	result, err := copier.Copy(
		context.Background(),
		story,
		destination,
		func(delta int64) {
			progress += delta
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if result.OutputName != story.OriginalFilename ||
		result.ByteSize != story.ByteSize ||
		result.SHA256 != story.SHA256 ||
		progress != story.ByteSize {
		t.Fatalf("Copy() = %#v, progress = %d", result, progress)
	}
	exported, err := os.ReadFile(filepath.Join(destination, story.OriginalFilename))
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(exported, content) {
		t.Fatalf("exported bytes = %q", exported)
	}
	entries, err := os.ReadDir(destination)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Name() != story.OriginalFilename {
		t.Fatalf("destination entries = %#v", entries)
	}
}

func TestCopierNeverOverwritesExistingDestination(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	story := writeManagedExportStory(t, layout, 1, "story.zip", []byte("source"))
	copier, err := NewCopier(layout)
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	finalPath := filepath.Join(destination, story.OriginalFilename)
	existing := []byte("existing")
	if err := os.WriteFile(finalPath, existing, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := copier.Copy(
		context.Background(),
		story,
		destination,
		nil,
	); !errors.Is(err, ErrExportConflict) {
		t.Fatalf("Copy(existing) error = %v", err)
	}
	got, err := os.ReadFile(finalPath)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(got, existing) {
		t.Fatalf("existing bytes changed to %q", got)
	}
	entries, err := os.ReadDir(destination)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 {
		t.Fatalf("destination entries = %#v", entries)
	}
}

func TestCopierRejectsChangedSourceWithoutPublishing(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	story := writeManagedExportStory(t, layout, 1, "story.zip", []byte("source"))
	story.SHA256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	copier, err := NewCopier(layout)
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	if _, err := copier.Copy(
		context.Background(),
		story,
		destination,
		nil,
	); !errors.Is(err, ErrExportChecksumFailed) {
		t.Fatalf("Copy(changed) error = %v", err)
	}
	entries, err := os.ReadDir(destination)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("destination entries = %#v", entries)
	}
}
