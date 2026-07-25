//go:build windows

package storage

import "os"

func defaultDataRoot() (string, error) {
	return platformDataRoot("windows", "", os.Getenv)
}
