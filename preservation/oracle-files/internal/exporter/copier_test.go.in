package exporter

import (
	"bytes"
	"context"
	"errors"
	"io"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
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
	resolvedDestination, err := ResolveDestination(destination)
	if err != nil {
		t.Fatal(err)
	}
	directorySynced := false
	copier.syncDirectory = func(path string) error {
		if path != resolvedDestination {
			t.Fatalf("sync directory = %q, want %q", path, resolvedDestination)
		}
		if _, err := os.Stat(filepath.Join(path, story.OriginalFilename)); err != nil {
			t.Fatalf("sync ran before publication: %v", err)
		}
		directorySynced = true
		return nil
	}
	var progress int64
	result, err := copier.Copy(
		context.Background(),
		exportNewItem(story),
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
		progress != story.ByteSize ||
		!directorySynced {
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
		exportNewItem(story),
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
		exportNewItem(story),
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
		exportNewItem(first),
		destination,
		nil,
	); err != nil {
		t.Fatal(err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	progressCalls := 0
	_, err = copier.Copy(ctx, exportNewItem(second), destination, func(int64) {
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

func TestCopierCleanupAbandonedRemovesOnlyOwnedRegularTemporaries(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	copier, err := NewCopier(layout)
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	resolvedDestination, err := ResolveDestination(destination)
	if err != nil {
		t.Fatal(err)
	}
	abandoned := filepath.Join(
		destination,
		temporaryNamePrefix+"abandoned.tmp",
	)
	if err := os.WriteFile(abandoned, []byte("partial"), 0o600); err != nil {
		t.Fatal(err)
	}
	keptPaths := []string{
		filepath.Join(destination, "keep.tmp"),
		filepath.Join(destination, temporaryNamePrefix+"completed.zip"),
	}
	for _, path := range keptPaths {
		if err := os.WriteFile(path, []byte("keep"), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	keptDirectory := filepath.Join(
		destination,
		temporaryNamePrefix+"directory.tmp",
	)
	if err := os.Mkdir(keptDirectory, 0o700); err != nil {
		t.Fatal(err)
	}
	keptSymlink := filepath.Join(
		destination,
		temporaryNamePrefix+"symlink.tmp",
	)
	if err := os.Symlink(keptPaths[0], keptSymlink); err != nil {
		t.Fatal(err)
	}

	syncCalls := 0
	copier.syncDirectory = func(path string) error {
		if path != resolvedDestination {
			t.Fatalf("sync directory = %q, want %q", path, resolvedDestination)
		}
		syncCalls++
		return nil
	}
	if err := copier.CleanupAbandoned(destination); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Lstat(abandoned); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("abandoned temporary still exists: %v", err)
	}
	for _, path := range append(keptPaths, keptDirectory, keptSymlink) {
		if _, err := os.Lstat(path); err != nil {
			t.Fatalf("preserved entry %q error = %v", path, err)
		}
	}
	if syncCalls != 1 {
		t.Fatalf("directory sync calls = %d, want 1", syncCalls)
	}

	if err := copier.CleanupAbandoned(destination); err != nil {
		t.Fatal(err)
	}
	if syncCalls != 1 {
		t.Fatalf("directory sync calls after no-op = %d, want 1", syncCalls)
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
	copier.publish = func(temporaryPath string, finalPath string) error {
		if err := os.WriteFile(finalPath, racingBytes, 0o600); err != nil {
			return err
		}
		return publishNoReplace(temporaryPath, finalPath)
	}
	destination := t.TempDir()
	if _, err := copier.Copy(
		context.Background(),
		exportNewItem(story),
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

func TestCopierMidCopyFailureCleansTemporaryAndPreservesCompletedFiles(t *testing.T) {
	t.Parallel()

	layout, err := storage.Initialize(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	first := writeManagedExportStory(t, layout, 1, "first.zip", []byte("first"))
	second := writeManagedExportStory(t, layout, 2, "second.zip", []byte("second"))
	copier, err := NewCopier(layout)
	if err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	if _, err := copier.Copy(
		context.Background(),
		exportNewItem(first),
		destination,
		nil,
	); err != nil {
		t.Fatal(err)
	}

	readErr := errors.New("injected read failure")
	copier.openSource = func(string) (io.ReadCloser, error) {
		return &failingReadCloser{
			reader: bytes.NewReader([]byte("partial")),
			err:    readErr,
		}, nil
	}
	if _, err := copier.Copy(
		context.Background(),
		exportNewItem(second),
		destination,
		nil,
	); !errors.Is(err, readErr) {
		t.Fatalf("Copy(mid-copy failure) error = %v", err)
	}
	got, err := os.ReadFile(filepath.Join(destination, first.OriginalFilename))
	if err != nil || string(got) != "first" {
		t.Fatalf("completed export = %q, %v", got, err)
	}
	entries, err := os.ReadDir(destination)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Name() != first.OriginalFilename {
		t.Fatalf("destination entries = %#v", entries)
	}
}

func exportNewItem(story library.ExportStory) operations.NewItem {
	return operations.NewItem{
		StoryID:             story.ID,
		StoryUUID:           story.UUID,
		StoryTitle:          story.Title,
		SourceName:          story.OriginalFilename,
		OutputName:          story.OriginalFilename,
		ArchiveRelativePath: story.ManagedRelativePath,
		ArchiveSHA256:       story.SHA256,
		TotalBytes:          story.ByteSize,
	}
}

type failingReadCloser struct {
	reader *bytes.Reader
	err    error
}

func (r *failingReadCloser) Read(buffer []byte) (int, error) {
	if r.reader.Len() > 0 {
		return r.reader.Read(buffer)
	}
	return 0, r.err
}

func (r *failingReadCloser) Close() error {
	return nil
}
