package preferences

import (
	"errors"
	"os"
	"path/filepath"

	"golang.org/x/sys/unix"
)

const reservationFile = ".librairii-reservation"

func Reserve(root, id string, bytes int64) error {
	if err := os.Mkdir(root, 0700); err != nil && !errors.Is(err, os.ErrExist) {
		return err
	}
	var stat unix.Statfs_t
	if err := unix.Statfs(root, &stat); err == nil && uint64(stat.Bavail)*uint64(stat.Bsize) < uint64(bytes) {
		return ErrDestination
	}
	file, err := os.OpenFile(filepath.Join(root, reservationFile), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	_, err = file.WriteString(id)
	if err == nil {
		err = file.Sync()
	}
	err = errors.Join(err, file.Close())
	if err != nil {
		return err
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return err
	}
	if len(entries) != 1 {
		return ErrDestination
	}
	return nil
}
func ReleaseReservation(root, id string) error {
	data, err := os.ReadFile(filepath.Join(root, reservationFile))
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if string(data) != id {
		return ErrDestination
	}
	if err := os.Remove(filepath.Join(root, reservationFile)); err != nil {
		return err
	}
	// Remove only an empty directory. Never remove concurrent destination files.
	return os.Remove(root)
}
func Publish(workspace, target, id string) error {
	data, err := os.ReadFile(filepath.Join(target, reservationFile))
	if err != nil || string(data) != id {
		return ErrDestination
	}
	entries, err := os.ReadDir(target)
	if err != nil || len(entries) != 1 {
		return ErrDestination
	}
	if err := os.Remove(filepath.Join(target, reservationFile)); err != nil {
		return err
	}
	// Removing the empty reservation fails if another process added files.
	if err := os.Remove(target); err != nil {
		return err
	}
	if err := os.Rename(workspace, target); err != nil {
		return err
	}
	return syncDirectory(filepath.Dir(target))
}
