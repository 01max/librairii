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

func TestExplicitRootIgnoresProductionProfile(t *testing.T) {
	root := t.TempDir()
	r := NewStorageReadiness(root)
	report, err := r.Check(context.Background())
	if err != nil || !report.MutationsAllowed {
		t.Fatalf("%+v %v", report, err)
	}
	defer r.Close()
	p := r.Profile()
	if !r.Isolated() || p.Language != "en" || p.Root != root {
		t.Fatalf("%+v", p)
	}
	if _, err = os.Stat(filepath.Join(root, "settings.json")); !errors.Is(err, os.ErrNotExist) {
		t.Fatal(err)
	}
}

func TestSetupInitializesConfirmedFolderOnly(t *testing.T) {
	ctx := context.Background()
	root := filepath.Join(t.TempDir(), "bibliothèque choisie")
	r := NewConfiguredStorageReadiness(t.TempDir(), "/unused", "fr")
	if err := r.InitializeLibrary(ctx, root); err != nil {
		t.Fatal(err)
	}
	if !r.Profile().Complete || r.Profile().Root != root {
		t.Fatal(r.Profile())
	}
	if _, err := r.Check(ctx); err != nil {
		t.Fatal(err)
	}
	r.Close()
	next := NewConfiguredStorageReadiness(r.profileStore.Directory(), "/unused", "en")
	if _, err := next.Check(ctx); err != nil {
		t.Fatal(err)
	}
	defer next.Close()
	if next.Profile().Language != "fr" {
		t.Fatal(next.Profile())
	}
}
