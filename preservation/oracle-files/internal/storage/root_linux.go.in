//go:build linux

package storage

import "os"

func defaultDataRoot() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return platformDataRoot("linux", home, os.Getenv)
}
