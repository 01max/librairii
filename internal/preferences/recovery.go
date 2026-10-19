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
	switch p.Root {
	case j.Source:
		if err := DiscardOwned(j.Workspace(), j.ID); err != nil {
			return err
		}
		if err := DiscardOwned(j.Destination, j.ID); err != nil {
			return err
		}
		if err := ReleaseReservation(j.Destination, j.ID); err != nil && !errors.Is(err, os.ErrNotExist) {
			return err
		}
	case j.Destination:
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
func DiscardOwned(root, id string) error {
	data, err := os.ReadFile(filepath.Join(root, moveMarker))
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if string(data) != id {
		return ErrInvalid
	}
	return os.RemoveAll(root)
}
