package preferences

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/database"
	"os"
	"path/filepath"
)

func VerifyCopy(ctx context.Context, j Journal, root string) error {
	for _, entry := range j.Entries {
		path := filepath.Join(root, entry.Path)
		info, err := os.Lstat(path)
		if err != nil {
			return err
		}
		if entry.Directory {
			if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
				return ErrInvalid
			}
			continue
		}
		if info.Size() != entry.Size {
			return ErrInvalid
		}
		hash, err := fileHash(ctx, path)
		if err != nil {
			return err
		}
		if hash != entry.SHA256 {
			return ErrInvalid
		}
	}
	if err := ValidateLibrary(ctx, root, j.LibraryID); err != nil {
		return err
	}
	db, err := database.Open(ctx, DatabasePath(root))
	if err != nil {
		return err
	}
	var integrity string
	err = db.SQL().QueryRowContext(ctx, "PRAGMA integrity_check").Scan(&integrity)
	if integrity != "ok" && err == nil {
		err = ErrInvalid
	}
	return errors.Join(err, db.Close())
}
