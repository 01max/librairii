package preferences

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"

	"github.com/01max/librairii/internal/database"
	"github.com/google/uuid"
)

func DatabasePath(root string) string { return filepath.Join(root, "db", "librairii.sqlite3") }
func Identity(root string) (string, error) {
	data, err := os.ReadFile(filepath.Join(root, IdentityFile))
	if err != nil {
		return "", err
	}
	id := strings.TrimSpace(string(data))
	if _, err := uuid.Parse(id); err != nil {
		return "", ErrInvalid
	}
	return id, nil
}
func EnsureIdentity(root string) (string, error) {
	id, err := Identity(root)
	if err == nil {
		return id, nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return "", err
	}
	id = uuid.NewString()
	file, err := os.OpenFile(filepath.Join(root, IdentityFile), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if errors.Is(err, os.ErrExist) {
		return Identity(root)
	}
	if err != nil {
		return "", err
	}
	_, err = file.WriteString(id + "\n")
	if err == nil {
		err = file.Sync()
	}
	err = errors.Join(err, file.Close())
	return id, err
}
func ValidateLibrary(ctx context.Context, root, id string) error {
	actual, err := Identity(root)
	if err != nil || actual != id {
		return ErrMissingLibrary
	}
	identity, err := (database.SchemaProbe{}).Inspect(ctx, DatabasePath(root))
	if err != nil {
		return err
	}
	if identity != database.IdentityCompatible {
		return ErrMissingLibrary
	}
	return nil
}

// Bootstrap observes the old root without creating directories. Ambiguous data
// is deliberately left for storage recovery rather than overwritten.
func Bootstrap(ctx context.Context, s *Store, legacy, language string) (Profile, error) {
	p, err := s.Load()
	if err == nil {
		return p, nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return p, err
	}
	p = Profile{Version: Version, Language: language}
	identity, err := (database.SchemaProbe{}).Inspect(ctx, DatabasePath(legacy))
	if err != nil {
		return p, err
	}
	if identity == database.IdentityAbsent {
		entries, readErr := os.ReadDir(legacy)
		if readErr != nil && !errors.Is(readErr, os.ErrNotExist) {
			return p, readErr
		}
		if len(entries) > 0 {
			return p, ErrInvalid
		}
		return p, ErrSetupRequired
	}
	if identity != database.IdentityCompatible {
		return p, database.ErrForeignSchema
	}
	id, err := EnsureIdentity(legacy)
	if err != nil {
		return p, err
	}
	return s.Update(func(next *Profile) error {
		*next = Profile{Version: Version, Language: language, Root: legacy, LibraryID: id, Complete: true}
		return nil
	})
}
