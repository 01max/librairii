package exporter

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestResolveDestinationValidatesAndResolvesDirectory(t *testing.T) {
	t.Parallel()

	destination := t.TempDir()
	resolved, err := ResolveDestination(destination)
	if err != nil {
		t.Fatal(err)
	}
	expected, err := filepath.EvalSymlinks(destination)
	if err != nil {
		t.Fatal(err)
	}
	if resolved != expected {
		t.Fatalf("ResolveDestination() = %q, want %q", resolved, expected)
	}

	file := filepath.Join(destination, "story.zip")
	if err := os.WriteFile(file, []byte("story"), 0o600); err != nil {
		t.Fatal(err)
	}
	for name, path := range map[string]string{
		"empty":    "",
		"relative": "exports",
		"spaced":   " " + destination,
		"file":     file,
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := ResolveDestination(path); !errors.Is(
				err,
				ErrInvalidDestination,
			) {
				t.Fatalf("ResolveDestination(%q) error = %v", path, err)
			}
		})
	}
	if _, err := ResolveDestination(filepath.Join(destination, "missing")); !errors.Is(
		err,
		ErrDestinationUnavailable,
	) {
		t.Fatalf("ResolveDestination(missing) error = %v", err)
	}
}
