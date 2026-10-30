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

func TestEmptyExistingSetupFolderRemainsUsableAcrossRestartWithSharedControl(t *testing.T) {
	ctx := context.Background()
	directory, root := t.TempDir(), t.TempDir()
	first := NewConfiguredStorageReadiness(directory, root, "fr")
	if _, err := first.Check(ctx); !errors.Is(err, preferences.ErrSetupRequired) {
		t.Fatal(err)
	}
	if err := first.Close(); err != nil {
		t.Fatal(err)
	}
	next := NewConfiguredStorageReadiness(directory, root, "fr")
	defer next.Close()
	if _, err := next.Check(ctx); !errors.Is(err, preferences.ErrSetupRequired) {
		t.Fatal(err)
	}
	if err := next.InitializeLibrary(ctx, root); err != nil {
		t.Fatal(err)
	}
	if report, err := next.Check(ctx); err != nil || !report.MutationsAllowed {
		t.Fatal(report, err)
	}
}

func TestSetupKeepsLanguageSavedByAnotherRecoveryWindow(t *testing.T) {
	ctx := context.Background()
	directory, root := t.TempDir(), t.TempDir()
	owner := NewConfiguredStorageReadiness(directory, root, "en")
	defer owner.Close()
	if _, err := owner.Check(ctx); !errors.Is(err, preferences.ErrSetupRequired) {
		t.Fatal(err)
	}
	other := NewConfiguredStorageReadiness(directory, root, "en")
	defer other.Close()
	if err := other.SetLanguage("fr"); err != nil {
		t.Fatal(err)
	}
	if err := owner.InitializeLibrary(ctx, root); err != nil {
		t.Fatal(err)
	}
	actual, err := preferences.NewStore(directory).Load()
	if err != nil || actual.Language != "fr" || actual.Root != root || !actual.Complete {
		t.Fatal(actual, err)
	}
}

func TestFailedClosePermanentlyFencesReopenAndRetainsEveryLease(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	owner := NewStorageReadiness(root)
	// Test-only disposal after proving the terminal fence. Production retains
	// ownership until process exit; no normal callback can clear this state.
	defer owner.releaseOwnership()
	if _, err := owner.Check(ctx); err != nil {
		t.Fatal(err)
	}
	if err := owner.Writer().Close(); err != nil {
		t.Fatal(err)
	}
	if err := owner.Close(); err == nil {
		t.Fatal("failed checkpoint accepted")
	}
	if err := owner.Close(); err == nil {
		t.Fatal("second close cleared the fence")
	}
	if _, err := owner.Check(ctx); err == nil {
		t.Fatal("failed-close owner reopened")
	}
	if _, err := owner.RecoverSchemaConflict(ctx); err == nil {
		t.Fatal("failed-close owner entered recovery")
	}
	other := NewStorageReadiness(root)
	defer other.Close()
	if _, err := other.Check(ctx); !errors.Is(err, storage.ErrLibraryInUse) {
		t.Fatal("failed close released ownership", err)
	}
}

func TestNativeOwnedLibraryRefusesGoReopenAndRecoveryWithoutChangingItsRecords(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	initial := NewStorageReadiness(root)
	if _, err := initial.Check(ctx); err != nil {
		t.Fatal(err)
	}
	if err := initial.Close(); err != nil {
		t.Fatal(err)
	}
	marker := filepath.Join(root, ".librairii-native-state.json")
	if err := os.WriteFile(marker, []byte("future native recovery bytes"), 0600); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(filepath.Join(root, "db/librairii.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	other := NewStorageReadiness(root)
	defer other.Close()
	if _, err := other.Check(ctx); !errors.Is(err, storage.ErrNativeLibrary) {
		t.Fatal(err)
	}
	if _, err := other.RecoverSchemaConflict(ctx); !errors.Is(err, storage.ErrNativeLibrary) {
		t.Fatal(err)
	}
	after, err := os.ReadFile(filepath.Join(root, "db/librairii.sqlite3"))
	if err != nil || string(after) != string(before) {
		t.Fatal("native database changed", err)
	}
	if b, err := os.ReadFile(marker); err != nil || string(b) != "future native recovery bytes" {
		t.Fatal("native control changed", err)
	}
}

func TestRestartBeforeRootCommitClearsJournalAndRetainsCopyLease(t *testing.T) {
	for _, published := range []bool{false, true} {
		t.Run(map[bool]string{false: "copied", true: "published"}[published], func(t *testing.T) {
			ctx := context.Background()
			config, root := t.TempDir(), filepath.Join(t.TempDir(), "source")
			owner := NewConfiguredStorageReadiness(config, root, "en")
			defer owner.Close()
			if err := owner.InitializeLibrary(ctx, root); err != nil {
				t.Fatal(err)
			}
			if _, err := owner.Check(ctx); err != nil {
				t.Fatal(err)
			}
			profile := owner.Profile()
			if err := owner.Close(); err != nil {
				t.Fatal(err)
			}
			entries, err := preferences.Manifest(ctx, root)
			if err != nil {
				t.Fatal(err)
			}
			j := preferences.Journal{ID: "d0000000-0000-4000-8000-000000000001", LibraryID: profile.LibraryID, Source: root, Destination: filepath.Join(t.TempDir(), "destination"), Phase: "reserved", Entries: entries}
			if err := preferences.Reserve(j.Destination, j.ID, 0); err != nil {
				t.Fatal(err)
			}
			if err := preferences.CopyLibrary(ctx, j, nil); err != nil {
				t.Fatal(err)
			}
			copyRoot := j.Workspace()
			if published {
				if err := preferences.Publish(copyRoot, j.Destination, j.ID); err != nil {
					t.Fatal(err)
				}
				copyRoot = j.Destination
				j.Phase = "published"
			}
			if err := preferences.WriteJournal(config, j); err != nil {
				t.Fatal(err)
			}
			restarted := NewConfiguredStorageReadiness(config, root, "en")
			defer restarted.Close()
			if report, err := restarted.Check(ctx); err != nil || !report.MutationsAllowed {
				t.Fatal(report, err)
			}
			if restarted.Profile().Root != root {
				t.Fatal("uncommitted copy became authoritative")
			}
			if _, err := preferences.ReadJournal(config); !errors.Is(err, os.ErrNotExist) {
				t.Fatal("precommit recovery left a blocking journal", err)
			}
			remaining, err := os.ReadDir(copyRoot)
			if err != nil || len(remaining) != 1 || remaining[0].Name() != storage.LibraryLockFile {
				t.Fatal("copy payload remains", remaining, err)
			}
			if lease, err := storage.AcquireLibrary(copyRoot); !errors.Is(err, storage.ErrLibraryInUse) {
				if lease != nil {
					lease.Close()
				}
				t.Fatal("recovery released copy ownership", err)
			}
			if err := restarted.Close(); err != nil {
				t.Fatal(err)
			}
			lease, err := storage.AcquireLibrary(copyRoot)
			if err != nil {
				t.Fatal("final close did not release copy ownership", err)
			}
			defer lease.Close()
			if report, err := restarted.Check(ctx); err != nil || !report.MutationsAllowed {
				t.Fatal("later startup did not reopen the source", report, err)
			}
		})
	}
}
