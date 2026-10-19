package preferences

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/database"
	"github.com/google/uuid"
	"os"
	"path/filepath"
	"testing"
)

func moveFixture(t *testing.T) (Journal, Profile, string) {
	t.Helper()
	ctx := context.Background()
	base := t.TempDir()
	source := filepath.Join(base, "source")
	destination := filepath.Join(base, "destination")
	config := filepath.Join(base, "profile")
	os.MkdirAll(filepath.Join(source, "db"), 0700)
	db, err := database.Open(ctx, DatabasePath(source))
	if err != nil {
		t.Fatal(err)
	}
	if err = db.Close(); err != nil {
		t.Fatal(err)
	}
	id, err := EnsureIdentity(source)
	if err != nil {
		t.Fatal(err)
	}
	for _, dir := range []string{"archives", "catalog", "trash", "staging", "logs"} {
		os.MkdirAll(filepath.Join(source, dir), 0700)
		os.WriteFile(filepath.Join(source, dir, "example"), []byte("preserved "+dir), 0600)
	}
	entries, err := Manifest(ctx, source)
	if err != nil {
		t.Fatal(err)
	}
	j := Journal{ID: uuid.NewString(), LibraryID: id, Source: source, Destination: destination, Phase: "reserved", Entries: entries}
	p := Profile{Version: Version, Language: "fr", Root: source, LibraryID: id, Complete: true}
	return j, p, config
}
func TestRestartBeforeCommitRetainsSource(t *testing.T) {
	for _, phase := range []string{"journal", "reserved", "copied", "published"} {
		t.Run(phase, func(t *testing.T) {
			j, p, config := moveFixture(t)
			ctx := context.Background()
			if err := WriteJournal(config, j); err != nil {
				t.Fatal(err)
			}
			if phase != "journal" {
				if err := Reserve(j.Destination, j.ID, 0); err != nil {
					t.Fatal(err)
				}
			}
			if phase == "copied" || phase == "published" {
				if err := CopyLibrary(ctx, j, nil); err != nil {
					t.Fatal(err)
				}
			}
			if phase == "published" {
				if err := Publish(j.Workspace(), j.Destination, j.ID); err != nil {
					t.Fatal(err)
				}
			}
			if err := Reconcile(ctx, config, p); err != nil {
				t.Fatal(err)
			}
			if err := ValidateLibrary(ctx, p.Root, p.LibraryID); err != nil {
				t.Fatal(err)
			}
			if _, err := os.Stat(j.Destination); !errors.Is(err, os.ErrNotExist) {
				t.Fatal("destination not discarded", err)
			}
		})
	}
}
func TestPostCommitRecoveryNeverUsesOldSource(t *testing.T) {
	j, p, config := moveFixture(t)
	ctx := context.Background()
	Reserve(j.Destination, j.ID, 0)
	if err := CopyLibrary(ctx, j, nil); err != nil {
		t.Fatal(err)
	}
	if err := VerifyCopy(ctx, j, j.Workspace()); err != nil {
		t.Fatal(err)
	}
	if err := Publish(j.Workspace(), j.Destination, j.ID); err != nil {
		t.Fatal(err)
	}
	WriteJournal(config, j)
	p.Root = j.Destination
	os.WriteFile(filepath.Join(j.Source, "foreign"), []byte("keep"), 0600)
	if err := Reconcile(ctx, config, p); !errors.Is(err, ErrCleanupPending) {
		t.Fatal(err)
	}
	if data, err := os.ReadFile(filepath.Join(j.Source, "foreign")); err != nil || string(data) != "keep" {
		t.Fatal(err)
	}
	if err := ValidateLibrary(ctx, j.Destination, j.LibraryID); err != nil {
		t.Fatal(err)
	}
	os.Rename(j.Destination, j.Destination+"-offline")
	if err := Reconcile(ctx, config, p); err == nil {
		t.Fatal("accepted missing committed root")
	}
}
func TestCancellationCorruptionAndUnknownDestinationFiles(t *testing.T) {
	j, p, config := moveFixture(t)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := CopyLibrary(ctx, j, nil); !errors.Is(err, context.Canceled) {
		t.Fatal(err)
	}
	if err := DiscardOwned(context.Background(), j.Workspace(), j); err != nil {
		t.Fatal(err)
	}
	if err := CopyLibrary(context.Background(), j, nil); err != nil {
		t.Fatal(err)
	}
	os.WriteFile(filepath.Join(j.Workspace(), "archives", "example"), []byte("corrupt"), 0600)
	if err := VerifyCopy(context.Background(), j, j.Workspace()); err == nil {
		t.Fatal("accepted corruption")
	}
	WriteJournal(config, j)
	if err := Reconcile(context.Background(), config, p); !errors.Is(err, ErrCleanupPending) {
		t.Fatal(err)
	}
	if err := ValidateLibrary(context.Background(), j.Source, j.LibraryID); err != nil {
		t.Fatal(err)
	}
}
