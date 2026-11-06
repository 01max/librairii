package exporter

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

var (
	ErrInvalidDestination     = errors.New("export destination is invalid")
	ErrDestinationUnavailable = errors.New("export destination is unavailable")
)

func ResolveDestination(path string) (string, error) {
	if path == "" ||
		strings.TrimSpace(path) != path ||
		strings.ContainsRune(path, '\x00') ||
		!filepath.IsAbs(path) {
		return "", ErrInvalidDestination
	}
	resolved, err := filepath.EvalSymlinks(filepath.Clean(path))
	if err != nil {
		return "", fmt.Errorf("%w: %v", ErrDestinationUnavailable, err)
	}
	info, err := os.Stat(resolved)
	if err != nil {
		return "", fmt.Errorf("%w: %v", ErrDestinationUnavailable, err)
	}
	if !info.IsDir() {
		return "", ErrInvalidDestination
	}
	return resolved, nil
}
