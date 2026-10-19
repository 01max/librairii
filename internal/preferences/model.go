// Package preferences owns the application profile independently of library data.
package preferences

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
)

const Version = 1
const IdentityFile = ".librairii-library"

var ErrInvalid = errors.New("settings.invalid")
var ErrMissingLibrary = errors.New("settings.library_missing")
var ErrSetupRequired = errors.New("settings.setup_required")

type Profile struct {
	Version   int    `json:"version"`
	Language  string `json:"language"`
	Root      string `json:"root"`
	LibraryID string `json:"libraryId"`
	Complete  bool   `json:"complete"`
}

func (p Profile) Validate() error {
	if p.Version != Version || (p.Language != "en" && p.Language != "fr") {
		return ErrInvalid
	}
	if p.Complete && (!filepath.IsAbs(p.Root) || p.LibraryID == "") {
		return ErrInvalid
	}
	return nil
}

func Directory() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return PlatformDirectory(runtime.GOOS, home, os.Getenv)
}

func PlatformDirectory(platform, home string, getenv func(string) string) (string, error) {
	switch platform {
	case "darwin":
		return filepath.Join(home, "Library", "Preferences", "io.github.librairii.app"), nil
	case "linux":
		base := getenv("XDG_CONFIG_HOME")
		if base == "" {
			base = filepath.Join(home, ".config")
		}
		if !filepath.IsAbs(base) {
			return "", ErrInvalid
		}
		return filepath.Join(base, "librairii"), nil
	default:
		return "", ErrInvalid
	}
}
