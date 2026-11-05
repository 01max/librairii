//go:build darwin

package storage

import "os"

func defaultDataRoot() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return platformDataRoot("darwin", home, os.Getenv)
}
