package preferences

import (
	"context"
	"errors"
	"os"
	"path/filepath"
)

// Reconcile never selects a root: the durable profile is the sole authority.
func Reconcile(ctx context.Context, directory string, p Profile) error {
	j, err := ReadJournal(directory)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if j.LibraryID != p.LibraryID {
		return ErrInvalid
	}
	if Contains(j.Source, directory) || Contains(j.Destination, directory) {
		return ErrInvalid
	}
	if err := ValidateLibrary(ctx, p.Root, p.LibraryID); err != nil {
		return err
	}
	authority, _ := Canonical(p.Root)
	source, _ := Canonical(j.Source)
	destination, _ := Canonical(j.Destination)
	switch authority {
	case source:
		if err := DiscardOwned(ctx, j.Workspace(), j); err != nil {
			return err
		}
		if err := DiscardOwned(ctx, j.Destination, j); err != nil {
			return err
		}
		if err := ReleaseReservation(j.Destination, j.ID); err != nil && !errors.Is(err, os.ErrNotExist) {
			return err
		}
	case destination:
		if err := CleanupSource(ctx, j, p.Root); err != nil {
			return errors.Join(ErrCleanupPending, err)
		}
		if err := RemoveMoveMarker(j.Destination, j.ID); err != nil {
			return err
		}
	default:
		return ErrInvalid
	}
	return ClearJournal(directory)
}

// DiscardOwned is restricted to unique, unpublished move-owned directories.
// An unrelated or changed destination is never recursively removed.
func DiscardOwned(ctx context.Context, root string, j Journal) error {
	data, err := os.ReadFile(filepath.Join(root, moveMarker))
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if string(data) != j.ID {
		return ErrInvalid
	}
	var failures []error
	for _, entry := range j.Entries {
		if entry.Directory {
			continue
		}
		path := filepath.Join(root, entry.Path)
		if _, err := os.Lstat(path); errors.Is(err, os.ErrNotExist) {
			continue
		}
		resolved, err := Canonical(path)
		if err != nil || !containedResolved(root, resolved) {
			failures = append(failures, ErrCleanupPending)
			continue
		}
		hash, err := fileHash(ctx, path)
		if err != nil || hash != entry.SHA256 {
			failures = append(failures, ErrCleanupPending)
			continue
		}
		if err := os.Remove(path); err != nil {
			failures = append(failures, err)
		}
	}
	for i := len(j.Entries) - 1; i >= 0; i-- {
		entry := j.Entries[i]
		if entry.Directory {
			path := filepath.Join(root, entry.Path)
			info, statErr := os.Lstat(path)
			if errors.Is(statErr, os.ErrNotExist) {
				continue
			}
			resolved, resolveErr := Canonical(path)
			if statErr != nil || !info.IsDir() || resolveErr != nil || !containedResolved(root, resolved) {
				failures = append(failures, ErrCleanupPending)
				continue
			}
			if err := os.Remove(filepath.Join(root, entry.Path)); err != nil && !errors.Is(err, os.ErrNotExist) {
				failures = append(failures, err)
			}
		}
	}
	if len(failures) > 0 {
		return errors.Join(append(failures, ErrCleanupPending)...)
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return err
	}
	if len(entries) != 1 || entries[0].Name() != moveMarker {
		return ErrCleanupPending
	}
	if err := os.Remove(filepath.Join(root, moveMarker)); err != nil {
		return err
	}
	return os.Remove(root)
}
