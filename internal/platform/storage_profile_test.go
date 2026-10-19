package platform

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/preferences"
	"os"
	"path/filepath"
	"testing"
)

func TestConfiguredStorageWaitsForSetup(t *testing.T) {
	root := filepath.Join(t.TempDir(), "library")
	r := NewConfiguredStorageReadiness(t.TempDir(), root, "fr")
	if _, err := r.Check(context.Background()); !errors.Is(err, preferences.ErrSetupRequired) {
		t.Fatal(err)
	}
	if _, err := os.Stat(root); !errors.Is(err, os.ErrNotExist) {
		t.Fatal(err)
	}
}
func TestConfiguredMissingLibraryIsNotCreated(t *testing.T) {
	dir := t.TempDir()
	root := filepath.Join(t.TempDir(), "missing")
	s := preferences.NewStore(dir)
	_, err := s.Update(func(p *preferences.Profile) error {
		p.Root = root
		p.LibraryID = "missing"
		p.Complete = true
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	r := NewConfiguredStorageReadiness(dir, "/unused", "en")
	if _, err = r.Check(context.Background()); err == nil {
		t.Fatal("missing library opened")
	}
	if _, err = os.Stat(root); !errors.Is(err, os.ErrNotExist) {
		t.Fatal(err)
	}
}
