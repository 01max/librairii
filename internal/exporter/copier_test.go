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

func TestCopierCancellationCleansTemporaryAndPreservesCompletedFiles(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	first := writeManagedExportStory(t, layout, 1, "first.zip", []byte("first"))
	secondBytes := make([]byte, 256*1024)
	for index := range secondBytes {
		secondBytes[index] = byte(index % 251)
	}
	second := writeManagedExportStory(t, layout, 2, "second.zip", secondBytes)
	copier, err := NewCopier(layout)
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	if _, err := copier.Copy(
		context.Background(),
		first,
		destination,
		nil,
	); err != nil {
		t.Fatal(err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	progressCalls := 0
	_, err = copier.Copy(ctx, second, destination, func(int64) {
		progressCalls++
		cancel()
	})
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("Copy(cancelled) error = %v", err)
	}
	if progressCalls == 0 {
		t.Fatal("copy did not reach a cancellation boundary")
	}
	if got, err := os.ReadFile(filepath.Join(destination, first.OriginalFilename)); err != nil {
		t.Fatal(err)
	} else if string(got) != "first" {
		t.Fatalf("completed export changed to %q", got)
	}
	if _, err := os.Lstat(
		filepath.Join(destination, second.OriginalFilename),
	); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("cancelled final path error = %v", err)
	}
	entries, err := os.ReadDir(destination)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Name() != first.OriginalFilename {
		t.Fatalf("destination entries = %#v", entries)
	}
}

func TestCopierDestinationRacePreservesRacingFile(t *testing.T) {
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
	racingBytes := []byte("created after preflight")
	copier.link = func(temporaryPath string, finalPath string) error {
		if err := os.WriteFile(finalPath, racingBytes, 0o600); err != nil {
			return err
		}
		return os.Link(temporaryPath, finalPath)
	}
	destination := t.TempDir()
	if _, err := copier.Copy(
		context.Background(),
		story,
		destination,
		nil,
	); !errors.Is(err, ErrExportConflict) {
		t.Fatalf("Copy(racing destination) error = %v", err)
	}
	got, err := os.ReadFile(filepath.Join(destination, story.OriginalFilename))
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(got, racingBytes) {
		t.Fatalf("racing destination changed to %q", got)
	}
	entries, err := os.ReadDir(destination)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Name() != story.OriginalFilename {
		t.Fatalf("destination entries = %#v", entries)
	}
}
