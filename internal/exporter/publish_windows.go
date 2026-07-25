//go:build windows

package exporter

import (
	"errors"
	"os"

	"golang.org/x/sys/windows"
)

func publishNoReplace(temporaryPath string, finalPath string) error {
	temporaryPointer, err := windows.UTF16PtrFromString(temporaryPath)
	if err != nil {
		return err
	}
	finalPointer, err := windows.UTF16PtrFromString(finalPath)
	if err != nil {
		return err
	}
	err = windows.MoveFileEx(
		temporaryPointer,
		finalPointer,
		windows.MOVEFILE_WRITE_THROUGH,
	)
	if errors.Is(err, windows.ERROR_FILE_EXISTS) ||
		errors.Is(err, windows.ERROR_ALREADY_EXISTS) {
		return os.ErrExist
	}
	return err
}
