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
