//go:build darwin

package exporter

import (
	"errors"
	"os"

	"golang.org/x/sys/unix"
)

func publishNoReplace(temporaryPath string, finalPath string) error {
	err := unix.RenamexNp(temporaryPath, finalPath, unix.RENAME_EXCL)
	if errors.Is(err, unix.EEXIST) {
		return os.ErrExist
	}
	return err
}
