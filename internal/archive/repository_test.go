package archive

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/storage"
)

func TestStageAndPublishPreserveSourceBytes(t *testing.T) {
	t.Parallel()

	repository, layout := newTestRepository(t)
	source := filepath.Join(t.TempDir(), "story.plain.pk")
	content := []byte("synthetic archive bytes")
	if err := os.WriteFile(source, content, 0o640); err != nil {
		t.Fatal(err)
	}

	staged, err := repository.Stage(context.Background(), source)
	if err != nil {
		t.Fatalf("Stage() error = %v", err)
	}
	expectedHash := sha256.Sum256(content)
	if staged.SHA256 != hex.EncodeToString(expectedHash[:]) || staged.ByteSize != int64(len(content)) {
		t.Fatalf("Stage() = %#v", staged)
	}

	relative, err := repository.Publish(staged)
	if err != nil {
		t.Fatalf("Publish() error = %v", err)
	}
	expectedRelative := filepath.ToSlash(filepath.Join("archives", staged.SHA256, "story.plain.pk"))
	if relative != expectedRelative {
		t.Fatalf("Publish() path = %q, want %q", relative, expectedRelative)
	}

	managed, err := repository.Resolve(relative)
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}
	got, err := os.ReadFile(managed)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(content) {
		t.Fatalf("managed bytes = %q", got)
	}
	sourceBytes, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}
	if string(sourceBytes) != string(content) {
		t.Fatalf("source bytes = %q", sourceBytes)
	}
	if _, err := os.Stat(staged.Directory); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("staging directory still exists: %v", err)
	}
	if _, err := os.Stat(layout.Archives); err != nil {
		t.Fatal(err)
	}
}

func TestStageUsesUniqueDirectoriesAndCleansCancellation(t *testing.T) {
	t.Parallel()

	repository, _ := newTestRepository(t)
	source := filepath.Join(t.TempDir(), "story.pk")
	if err := os.WriteFile(source, []byte("story"), 0o600); err != nil {
		t.Fatal(err)
	}

	first, err := repository.Stage(context.Background(), source)
	if err != nil {
		t.Fatal(err)
	}
	second, err := repository.Stage(context.Background(), source)
	if err != nil {
		t.Fatal(err)
	}
	if first.Directory == second.Directory {
		t.Fatal("Stage() reused a staging directory")
	}
	if err := repository.Cleanup(first); err != nil {
		t.Fatalf("Cleanup() error = %v", err)
	}
	if _, err := os.Stat(first.Directory); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("Cleanup() left staging directory: %v", err)
	}
	if err := repository.Cleanup(second); err != nil {
		t.Fatalf("Cleanup(second) error = %v", err)
	}

	cancelled, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := repository.Stage(cancelled, source); !errors.Is(err, context.Canceled) {
		t.Fatalf("Stage(cancelled) error = %v", err)
	}
	entries, err := os.ReadDir(repository.layout.Staging)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("Stage(cancelled) left %d staging entries", len(entries))
	}
}

func TestPublishRejectsChangedStagingBytes(t *testing.T) {
	t.Parallel()

	repository, _ := newTestRepository(t)
	source := filepath.Join(t.TempDir(), "story.pk")
	if err := os.WriteFile(source, []byte("before"), 0o600); err != nil {
		t.Fatal(err)
	}
	staged, err := repository.Stage(context.Background(), source)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(staged.Path, []byte("after"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := repository.Publish(staged); !errors.Is(err, ErrStagedFileChanged) {
		t.Fatalf("Publish() error = %v", err)
	}
	if err := repository.Cleanup(staged); err != nil {
		t.Fatal(err)
	}
}

func TestManagedPathsCannotEscapeAndRemovalMovesToTrash(t *testing.T) {
	t.Parallel()

	repository, layout := newTestRepository(t)
	for _, invalid := range []string{"", "../story.pk", "/tmp/story.pk"} {
		if _, err := SafeJoin(layout.Root, invalid); !errors.Is(err, ErrInvalidManagedPath) {
			t.Fatalf("SafeJoin(%q) error = %v", invalid, err)
		}
	}

	source := filepath.Join(t.TempDir(), "story.pk")
	if err := os.WriteFile(source, []byte("story"), 0o600); err != nil {
		t.Fatal(err)
	}
	staged, err := repository.Stage(context.Background(), source)
	if err != nil {
		t.Fatal(err)
	}
	relative, err := repository.Publish(staged)
	if err != nil {
		t.Fatal(err)
	}
	trashRelative, err := repository.MoveToTrash(relative)
	if err != nil {
		t.Fatalf("MoveToTrash() error = %v", err)
	}
	trashed, err := repository.Resolve(trashRelative)
	if err != nil {
		t.Fatalf("Resolve(trash) error = %v", err)
	}
	if _, err := os.Stat(trashed); err != nil {
		t.Fatalf("trashed archive missing: %v", err)
	}
}

func newTestRepository(t *testing.T) (*Repository, storage.Layout) {
	t.Helper()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatalf("storage.Initialize() error = %v", err)
	}
	return NewRepository(layout), layout
}
