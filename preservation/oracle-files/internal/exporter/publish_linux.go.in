//go:build linux

package exporter

import (
	"errors"
	"os"

	"golang.org/x/sys/unix"
)

func publishNoReplace(temporaryPath string, finalPath string) error {
	err := unix.Renameat2(
		unix.AT_FDCWD,
		temporaryPath,
		unix.AT_FDCWD,
		finalPath,
		unix.RENAME_NOREPLACE,
	)
	if errors.Is(err, unix.EEXIST) {
		return os.ErrExist
	}
	return err
}
