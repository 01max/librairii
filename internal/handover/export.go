package handover

import (
	"errors"
	"os"
	"path/filepath"
)

const Filename = "librairii-preferences.json"

// Export publishes only into the explicitly selected directory, without
// replacing a prior transfer. A destination-local temporary keeps it atomic.
func Export(directory string, body []byte) error {
	if !filepath.IsAbs(directory) || len(body) > MaxBytes {
		return ErrInvalid
	}
	file, err := os.CreateTemp(directory, ".librairii-preferences-*")
	if err != nil {
		return err
	}
	defer os.Remove(file.Name())
	if err = file.Chmod(0600); err == nil {
		_, err = file.Write(body)
	}
	if err == nil {
		err = file.Sync()
	}
	err = errors.Join(err, file.Close())
	if err != nil {
		return err
	}
	// Hard-link creation is atomic/no-clobber. The same-directory temporary is
	// removed before success, leaving one regular published preference file.
	if err = os.Link(file.Name(), filepath.Join(directory, Filename)); err != nil {
		return err
	}
	if err = os.Remove(file.Name()); err != nil {
		return err
	}
	dir, err := os.Open(directory)
	if err != nil {
		return err
	}
	return errors.Join(dir.Sync(), dir.Close())
}
