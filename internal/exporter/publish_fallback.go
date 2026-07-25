//go:build !darwin && !linux && !windows

package exporter

import "os"

func publishNoReplace(temporaryPath string, finalPath string) error {
	if err := os.Link(temporaryPath, finalPath); err != nil {
		return err
	}
	return os.Remove(temporaryPath)
}
