package storage

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

const DirectoryMode = 0o700

var (
	ErrDataRootNotAbsolute = errors.New("application data root must be absolute")
	ErrPlatformUnsupported = errors.New("platform application data root is unsupported")
)

type Layout struct {
	Root     string
	Database string
	Archives string
	Catalog  string
	Staging  string
	Trash    string
	Logs     string
}

func ResolveRoot(override string) (string, error) {
	if override != "" {
		return validateRoot(override)
	}

	root, err := defaultDataRoot()
	if err != nil {
		return "", err
	}
	return validateRoot(root)
}

func Initialize(root string) (Layout, error) {
	cleanRoot, err := validateRoot(root)
	if err != nil {
		return Layout{}, err
	}

	layout := Layout{
		Root:     cleanRoot,
		Database: filepath.Join(cleanRoot, "db"),
		Archives: filepath.Join(cleanRoot, "archives"),
		Catalog:  filepath.Join(cleanRoot, "catalog"),
		Staging:  filepath.Join(cleanRoot, "staging"),
		Trash:    filepath.Join(cleanRoot, "trash"),
		Logs:     filepath.Join(cleanRoot, "logs"),
	}

	for _, directory := range layout.Directories() {
		if err := os.MkdirAll(directory, DirectoryMode); err != nil {
			return Layout{}, fmt.Errorf("create application data directory: %w", err)
		}
	}

	return layout, nil
}

func (l Layout) Directories() []string {
	return []string{
		l.Root,
		l.Database,
		l.Archives,
		l.Catalog,
		l.Staging,
		l.Trash,
		l.Logs,
	}
}

func validateRoot(root string) (string, error) {
	if root == "" || !filepath.IsAbs(root) {
		return "", ErrDataRootNotAbsolute
	}
	return filepath.Clean(root), nil
}

func platformDataRoot(goos string, home string, getenv func(string) string) (string, error) {
	switch goos {
	case "darwin":
		if home == "" {
			return "", ErrDataRootNotAbsolute
		}
		return filepath.Join(home, "Library", "Application Support", "Librairii"), nil
	case "windows":
		localAppData := getenv("LOCALAPPDATA")
		if localAppData == "" {
			return "", fmt.Errorf("%w: LOCALAPPDATA is empty", ErrDataRootNotAbsolute)
		}
		return filepath.Join(localAppData, "Librairii"), nil
	case "linux":
		if dataHome := getenv("XDG_DATA_HOME"); dataHome != "" {
			if !filepath.IsAbs(dataHome) {
				return "", fmt.Errorf("%w: XDG_DATA_HOME", ErrDataRootNotAbsolute)
			}
			return filepath.Join(dataHome, "librairii"), nil
		}
		if home == "" {
			return "", ErrDataRootNotAbsolute
		}
		return filepath.Join(home, ".local", "share", "librairii"), nil
	default:
		return "", fmt.Errorf("%w: %s", ErrPlatformUnsupported, goos)
	}
}
