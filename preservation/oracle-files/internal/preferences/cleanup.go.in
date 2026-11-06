package preferences

import (
	"context"
	"errors"
	"os"
	"path/filepath"
)

var ErrCleanupPending = errors.New("settings.cleanup_pending")

// CleanupSource only removes manifest files whose bytes still match the closed
// source. Concurrently added or changed files are retained for explicit cleanup.
func CleanupSource(ctx context.Context, j Journal, authority string) error {
	if authority != j.Destination {
		return ErrInvalid
	}
	if err := ValidateLibrary(ctx, j.Destination, j.LibraryID); err != nil {
		return err
	}
	if _, err := os.Stat(j.Source); errors.Is(err, os.ErrNotExist) {
		return nil
	}
	id, err := Identity(j.Source)
	if err != nil || id != j.LibraryID {
		return ErrCleanupPending
	}
	var failures []error
	for _, entry := range j.Entries {
		if entry.Directory || entry.Path == IdentityFile {
			continue
		}
		path := filepath.Join(j.Source, entry.Path)
		if _, err := os.Lstat(path); errors.Is(err, os.ErrNotExist) {
			continue
		}
		resolved, err := Canonical(path)
		if err != nil || !containedResolved(j.Source, resolved) {
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
		if !entry.Directory {
			continue
		}
		path := filepath.Join(j.Source, entry.Path)
		info, statErr := os.Lstat(path)
		if errors.Is(statErr, os.ErrNotExist) {
			continue
		}
		resolved, resolveErr := Canonical(path)
		if statErr != nil || !info.IsDir() || resolveErr != nil || !containedResolved(j.Source, resolved) {
			failures = append(failures, ErrCleanupPending)
			continue
		}
		if err := os.Remove(filepath.Join(j.Source, entry.Path)); err != nil && !errors.Is(err, os.ErrNotExist) {
			failures = append(failures, err)
		}
	}
	if len(failures) > 0 {
		return errors.Join(append(failures, ErrCleanupPending)...)
	}
	entries, err := os.ReadDir(j.Source)
	if err != nil {
		return err
	}
	if len(entries) != 1 || entries[0].Name() != IdentityFile {
		return ErrCleanupPending
	}
	if err := os.Remove(filepath.Join(j.Source, IdentityFile)); err != nil {
		return err
	}
	return os.Remove(j.Source)
}
func RemoveMoveMarker(root, id string) error {
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
	return os.Remove(filepath.Join(root, moveMarker))
}
