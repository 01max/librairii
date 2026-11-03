package storage

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestResolveRootAcceptsOnlyAbsoluteOverrides(t *testing.T) {
	t.Parallel()

	root := filepath.Join(t.TempDir(), "data")
	got, err := ResolveRoot(root)
	if err != nil {
		t.Fatalf("ResolveRoot() error = %v", err)
	}
	if got != root {
		t.Fatalf("ResolveRoot() = %q, want %q", got, root)
	}

	if _, err := ResolveRoot("relative/data"); !errors.Is(err, ErrDataRootNotAbsolute) {
		t.Fatalf("ResolveRoot(relative) error = %v", err)
	}
}

func TestInitializeCreatesIsolatedLayout(t *testing.T) {
	t.Parallel()

	root := filepath.Join(t.TempDir(), "librairii")
	layout, err := Initialize(root)
	if err != nil {
		t.Fatalf("Initialize() error = %v", err)
	}

	for _, directory := range layout.Directories() {
		info, statErr := os.Stat(directory)
		if statErr != nil {
			t.Fatalf("Stat(%q) error = %v", directory, statErr)
		}
		if !info.IsDir() {
			t.Fatalf("%q is not a directory", directory)
		}
		if info.Mode().Perm()&0o077 != 0 {
			t.Fatalf("%q permissions = %o, want private", directory, info.Mode().Perm())
		}
	}
}

func TestPlatformDataRoots(t *testing.T) {
	t.Parallel()

	env := map[string]string{
		"XDG_DATA_HOME": filepath.Join(string(filepath.Separator), "users", "reader", "data"),
	}
	getenv := func(name string) string { return env[name] }
	home := filepath.Join(string(filepath.Separator), "users", "reader")

	tests := map[string]string{
		"darwin": filepath.Join(home, "Library", "Application Support", "Librairii"),
		"linux":  filepath.Join(env["XDG_DATA_HOME"], "librairii"),
	}

	for goos, expected := range tests {
		goos, expected := goos, expected
		t.Run(goos, func(t *testing.T) {
			t.Parallel()
			got, err := platformDataRoot(goos, home, getenv)
			if err != nil {
				t.Fatalf("platformDataRoot() error = %v", err)
			}
			if got != expected {
				t.Fatalf("platformDataRoot() = %q, want %q", got, expected)
			}
		})
	}
}

func TestLinuxDataRootFallsBackToHome(t *testing.T) {
	t.Parallel()

	home := filepath.Join(string(filepath.Separator), "users", "reader")
	got, err := platformDataRoot("linux", home, func(string) string { return "" })
	if err != nil {
		t.Fatalf("platformDataRoot() error = %v", err)
	}
	expected := filepath.Join(home, ".local", "share", "librairii")
	if got != expected {
		t.Fatalf("platformDataRoot() = %q, want %q", got, expected)
	}
}
