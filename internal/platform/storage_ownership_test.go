package platform

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/preferences"
	"github.com/01max/librairii/internal/storage"
)

func TestSharedOwnershipPrecedesRecoveryAndAllowsIndependentLanguage(t *testing.T) {
	ctx := context.Background()
	directory, root := t.TempDir(), filepath.Join(t.TempDir(), "library")
	owner := NewConfiguredStorageReadiness(directory, root, "en")
	defer owner.Close()
	if err := owner.InitializeLibrary(ctx, root); err != nil {
		t.Fatal(err)
	}
	if _, err := owner.Check(ctx); err != nil {
		t.Fatal(err)
	}
	journal := filepath.Join(directory, "relocation.json")
	if err := os.WriteFile(journal, []byte("uninterpreted sentinel"), 0600); err != nil {
		t.Fatal(err)
	}
	second := NewConfiguredStorageReadiness(directory, root, "en")
	defer second.Close()
	if _, err := second.Check(ctx); !errors.Is(err, storage.ErrLibraryInUse) {
		t.Fatalf("second entered recovery: %v", err)
	}
	if err := second.SetLanguage("fr"); err != nil {
		t.Fatalf("language unavailable during recovery: %v", err)
	}
	actual, err := preferences.NewStore(directory).Load()
	if err != nil || actual.Language != "fr" || actual.Root != root || actual.LibraryID != owner.Profile().LibraryID {
		t.Fatalf("language changed authority: %+v %v", actual, err)
	}
	bytes, err := os.ReadFile(journal)
	if err != nil || string(bytes) != "uninterpreted sentinel" {
		t.Fatalf("second launch interpreted journal: %q %v", bytes, err)
	}
	isolated := NewStorageReadiness(root)
	defer isolated.Close()
	if _, err := isolated.Check(ctx); !errors.Is(err, storage.ErrLibraryInUse) {
		t.Fatalf("override bypassed shared lock: %v", err)
	}
	if _, err := isolated.RecoverSchemaConflict(ctx); !errors.Is(err, storage.ErrLibraryInUse) {
		t.Fatalf("explicit recovery bypassed shared lock: %v", err)
	}
}

func TestRelocationRetainsOwnershipAcrossDatabaseClosure(t *testing.T) {
	ctx := context.Background()
	directory, root := t.TempDir(), filepath.Join(t.TempDir(), "library")
	owner := NewConfiguredStorageReadiness(directory, root, "en")
	defer owner.Close()
	if err := owner.InitializeLibrary(ctx, root); err != nil {
		t.Fatal(err)
	}
	if _, err := owner.Check(ctx); err != nil {
		t.Fatal(err)
	}
	if err := owner.BeginRelocation(); err != nil {
		t.Fatal(err)
	}
	if err := owner.Close(); err != nil {
		t.Fatal(err)
	}
	if owner.SQL() != nil {
		t.Fatal("DB stayed open")
	}
	other := NewStorageReadiness(root)
	defer other.Close()
	if _, err := other.Check(ctx); !errors.Is(err, storage.ErrLibraryInUse) {
		t.Fatalf("closed DB released live move ownership: %v", err)
	}
	owner.EndRelocation()
	if err := owner.Close(); err != nil {
		t.Fatal(err)
	}
	if report, err := other.Check(ctx); err != nil || !report.MutationsAllowed {
		t.Fatalf("actual close did not release: %+v %v", report, err)
	}
}
