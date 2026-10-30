package preferences

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/01max/librairii/internal/storage"
)

func TestCleanupNeverUnlinksRetainedSharedOwnershipEvenFromEarlierManifest(t *testing.T) {
	for _, foreign := range []bool{false, true} {
		t.Run(map[bool]string{false: "control-shell", true: "foreign-retained"}[foreign], func(t *testing.T) {
			j, _, _ := moveFixture(t)
			ctx := context.Background()
			lease, err := storage.AcquireLibrary(j.Source)
			if err != nil {
				t.Fatal(err)
			}
			defer lease.Close()
			before, err := os.Stat(filepath.Join(j.Source, storage.LibraryLockFile))
			if err != nil {
				t.Fatal(err)
			}
			entries, err := Manifest(ctx, j.Source)
			if err != nil {
				t.Fatal(err)
			}
			for _, entry := range entries {
				if entry.Path == storage.LibraryLockFile {
					t.Fatal("control copied as payload")
				}
			}
			if err := Reserve(j.Destination, j.ID, 0); err != nil {
				t.Fatal(err)
			}
			if err := CopyLibrary(ctx, j, nil); err != nil {
				t.Fatal(err)
			}
			if err := Publish(j.Workspace(), j.Destination, j.ID); err != nil {
				t.Fatal(err)
			}
			j.Entries = append(j.Entries, Entry{Path: storage.LibraryLockFile, Size: 0, SHA256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"})
			if foreign {
				if err := os.WriteFile(filepath.Join(j.Source, "foreign"), []byte("keep"), 0600); err != nil {
					t.Fatal(err)
				}
			}
			err = CleanupSource(ctx, j, j.Destination)
			if foreign && !errors.Is(err, ErrCleanupPending) || !foreign && err != nil {
				t.Fatal(err)
			}
			after, err := os.Stat(filepath.Join(j.Source, storage.LibraryLockFile))
			if err != nil || !os.SameFile(before, after) {
				t.Fatal("lease inode removed", err)
			}
			if err := lease.Revalidate(); err != nil {
				t.Fatal(err)
			}
			other, err := storage.AcquireLibrary(j.Source)
			if other != nil {
				other.Close()
			}
			if !errors.Is(err, storage.ErrLibraryInUse) {
				t.Fatal("split ownership", err)
			}
		})
	}
}

func TestAbortedMoveReconcilesWithRetainedLease(t *testing.T) {
	for _, published := range []bool{false, true} {
		for _, earlierManifest := range []bool{false, true} {
			name := map[bool]string{false: "workspace", true: "published"}[published]
			if earlierManifest {
				name += "/earlier-manifest"
			}
			t.Run(name, func(t *testing.T) {
				j, p, config := moveFixture(t)
				ctx := context.Background()
				if err := Reserve(j.Destination, j.ID, 0); err != nil {
					t.Fatal(err)
				}
				if err := CopyLibrary(ctx, j, nil); err != nil {
					t.Fatal(err)
				}
				root := j.Workspace()
				lease, err := storage.AcquireLibrary(root)
				if err != nil {
					t.Fatal(err)
				}
				defer lease.Close()
				before, err := os.Stat(filepath.Join(root, storage.LibraryLockFile))
				if err != nil {
					t.Fatal(err)
				}
				if published {
					if err := Publish(root, j.Destination, j.ID); err != nil {
						t.Fatal(err)
					}
					root = j.Destination
					if err := lease.Retarget(root); err != nil {
						t.Fatal(err)
					}
				} else {
					cancelled, cancel := context.WithCancel(ctx)
					cancel()
					if err := VerifyCopy(cancelled, j, root); !errors.Is(err, context.Canceled) {
						t.Fatal("verification was not cancelled", err)
					}
				}
				payload := j.Entries
				if earlierManifest {
					j.Entries = append(j.Entries, Entry{Path: storage.LibraryLockFile, Size: 0, SHA256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"})
				}
				if err := WriteJournal(config, j); err != nil {
					t.Fatal(err)
				}
				if err := Reconcile(ctx, config, p); err != nil {
					t.Fatal(err)
				}
				if _, err := ReadJournal(config); !errors.Is(err, os.ErrNotExist) {
					t.Fatal("aborted move journal remains", err)
				}
				entries, err := os.ReadDir(root)
				if err != nil || len(entries) != 1 || entries[0].Name() != storage.LibraryLockFile {
					t.Fatal("aborted copy retained payload", entries, err)
				}
				after, err := os.Stat(filepath.Join(root, storage.LibraryLockFile))
				if err != nil || !os.SameFile(before, after) {
					t.Fatal("lease inode removed", err)
				}
				if err := lease.Revalidate(); err != nil {
					t.Fatal(err)
				}
				if other, err := storage.AcquireLibrary(root); !errors.Is(err, storage.ErrLibraryInUse) {
					if other != nil {
						other.Close()
					}
					t.Fatal("split ownership after cleanup", err)
				}
				if err := Reconcile(ctx, config, p); err != nil {
					t.Fatal("cleanup is not idempotent", err)
				}
				// A later move can create and clear its own journal normally.
				j.ID = "d0000000-0000-4000-8000-000000000001"
				j.Destination = filepath.Join(t.TempDir(), "retry")
				j.Entries = payload
				if err := WriteJournal(config, j); err != nil {
					t.Fatal(err)
				}
				if err := Reserve(j.Destination, j.ID, 0); err != nil {
					t.Fatal(err)
				}
				if err := CopyLibrary(ctx, j, nil); err != nil {
					t.Fatal(err)
				}
				if err := VerifyCopy(ctx, j, j.Workspace()); err != nil {
					t.Fatal("source changed during abort cleanup", err)
				}
				if err := Reconcile(ctx, config, p); err != nil {
					t.Fatal(err)
				}
			})
		}
	}
}

func TestReleaseReservationRetainsSharedOwnership(t *testing.T) {
	root := t.TempDir()
	const id = "d0000000-0000-4000-8000-000000000001"
	if err := Reserve(root, id, 0); err != nil {
		t.Fatal(err)
	}
	lease, err := storage.AcquireLibrary(root)
	if err != nil {
		t.Fatal(err)
	}
	defer lease.Close()
	if err := ReleaseReservation(root, id); err != nil {
		t.Fatal(err)
	}
	if err := lease.Revalidate(); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(root, reservationFile)); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("reservation remains", err)
	}
}
