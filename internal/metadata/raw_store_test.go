package metadata

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

func TestRawSnapshotStoreStagesAndPublishesExactBytes(t *testing.T) {
	t.Parallel()

	store, layout := openRawSnapshotStore(t)
	syncID := "123e4567-e89b-42d3-a456-426614174200"
	payload := []byte(`{"response":{"fixture":{}}}`)
	staged, err := store.Stage(context.Background(), syncID, payload)
	if err != nil {
		t.Fatal(err)
	}
	hash := sha256.Sum256(payload)
	if staged.SHA256 != hex.EncodeToString(hash[:]) ||
		staged.ByteSize != int64(len(payload)) {
		t.Fatalf("Stage() = %#v", staged)
	}

	relativePath, err := store.Publish(staged)
	if err != nil {
		t.Fatal(err)
	}
	if relativePath != "catalog/"+syncID+"/catalog.json" {
		t.Fatalf("Publish() = %q", relativePath)
	}
	path, err := store.Resolve(relativePath)
	if err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(payload) {
		t.Fatalf("published bytes = %q", got)
	}
	if _, err := os.Stat(staged.Directory); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("staging directory still exists: %v", err)
	}
	if err := store.Remove(relativePath); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(layout.Catalog, syncID)); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("catalog directory still exists: %v", err)
	}
}

func TestRawSnapshotStoreCleansCancellationAndRejectsPathSubstitution(t *testing.T) {
	t.Parallel()

	store, layout := openRawSnapshotStore(t)
	cancelled, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := store.Stage(
		cancelled,
		"123e4567-e89b-42d3-a456-426614174201",
		[]byte("{}"),
	); !errors.Is(err, context.Canceled) {
		t.Fatalf("Stage(cancelled) error = %v", err)
	}
	if entries, err := os.ReadDir(layout.Staging); err != nil || len(entries) != 0 {
		t.Fatalf("staging entries = %#v, %v", entries, err)
	}
	if _, err := store.Stage(context.Background(), "../escape", []byte("{}")); !errors.Is(err, ErrInvalidSnapshotID) {
		t.Fatalf("Stage(path escape) error = %v", err)
	}
	if err := store.Remove("../catalog.json"); !errors.Is(err, ErrInvalidSnapshotPath) {
		t.Fatalf("Remove(path escape) error = %v", err)
	}
}

func TestRawSnapshotStoreDoesNotFollowSnapshotDirectorySymlink(t *testing.T) {
	t.Parallel()

	store, layout := openRawSnapshotStore(t)
	syncID := "123e4567-e89b-42d3-a456-426614174202"
	staged, err := store.Stage(context.Background(), syncID, []byte("{}"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = store.Cleanup(staged)
	})
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(layout.Catalog, syncID)); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}
	if _, err := store.Publish(staged); !errors.Is(err, ErrSnapshotExists) {
		t.Fatalf("Publish(symlink) error = %v", err)
	}
	if _, err := os.Stat(filepath.Join(outside, "catalog.json")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("snapshot escaped through symlink: %v", err)
	}
}

func openRawSnapshotStore(t *testing.T) (*RawSnapshotStore, storage.Layout) {
	t.Helper()

	layout, err := storage.Initialize(filepath.Join(t.TempDir(), "librairii"))
	if err != nil {
		t.Fatal(err)
	}
	return NewRawSnapshotStore(layout), layout
}
