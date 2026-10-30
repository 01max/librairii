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

func TestClosedStorageReopensAtCommittedRoot(t *testing.T) {
	ctx := context.Background()
	r := NewConfiguredStorageReadiness(t.TempDir(), "/unused", "en")
	old := filepath.Join(t.TempDir(), "old")
	newRoot := filepath.Join(t.TempDir(), "new")
	if err := r.InitializeLibrary(ctx, old); err != nil {
		t.Fatal(err)
	}
	if _, err := r.Check(ctx); err != nil {
		t.Fatal(err)
	}
	if err := r.Close(); err != nil {
		t.Fatal(err)
	}
	if err := os.Rename(old, newRoot); err != nil {
		t.Fatal(err)
	}
	if err := r.CommitRoot(newRoot); err != nil {
		t.Fatal(err)
	}
	if _, err := r.Check(ctx); err != nil {
		t.Fatal(err)
	}
	defer r.Close()
	if r.Layout().Root != newRoot || r.SQL() == nil {
		t.Fatal(r.Layout())
	}
}

func TestMissingLibraryCanBeLocatedWithoutCreatingReplacement(t *testing.T) {
	ctx := context.Background()
	directory := t.TempDir()
	old := filepath.Join(t.TempDir(), "old")
	moved := filepath.Join(t.TempDir(), "moved")
	r := NewConfiguredStorageReadiness(directory, "/unused", "fr")
	if err := r.InitializeLibrary(ctx, old); err != nil {
		t.Fatal(err)
	}
	if _, err := r.Check(ctx); err != nil {
		t.Fatal(err)
	}
	r.Close()
	os.Rename(old, moved)
	next := NewConfiguredStorageReadiness(directory, "/unused", "en")
	if _, err := next.Check(ctx); err == nil {
		t.Fatal("missing library accepted")
	}
	if _, err := os.Stat(old); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("created replacement", err)
	}
	other := NewConfiguredStorageReadiness(t.TempDir(), "/unused", "en")
	wrong := filepath.Join(t.TempDir(), "wrong")
	if err := other.InitializeLibrary(ctx, wrong); err != nil {
		t.Fatal(err)
	}
	if err := next.LocateLibrary(ctx, wrong); err == nil {
		t.Fatal("accepted unrelated library")
	}
	if err := next.LocateLibrary(ctx, moved); err != nil {
		t.Fatal(err)
	}
	if _, err := next.Check(ctx); err != nil {
		t.Fatal(err)
	}
	defer next.Close()
	if next.Profile().Root != moved || next.Profile().Language != "fr" {
		t.Fatal(next.Profile())
	}
}

func TestInterruptedSetupRetriesConfirmedFolderAfterRestart(t *testing.T) {
	directory := t.TempDir()
	root := filepath.Join(t.TempDir(), "chosen")
	r := NewConfiguredStorageReadiness(directory, "/unused", "fr")
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := r.InitializeLibrary(ctx, root); err == nil {
		t.Fatal("cancelled initialization succeeded")
	}
	next := NewConfiguredStorageReadiness(directory, "/unused", "en")
	if _, err := next.Check(context.Background()); !errors.Is(err, preferences.ErrSetupRequired) {
		t.Fatal(err)
	}
	if next.SuggestedRoot() != root {
		t.Fatal("lost confirmed folder", next.SuggestedRoot())
	}
	if err := next.InitializeLibrary(context.Background(), root); err != nil {
		t.Fatal(err)
	}
	if _, err := next.Check(context.Background()); err != nil {
		t.Fatal(err)
	}
	defer next.Close()
	if next.Profile().Language != "fr" {
		t.Fatal(next.Profile())
	}
}

func TestSetupCreatesMissingDefaultParentsAfterConfirmation(t *testing.T) {
	for _, relative := range []string{".local/share/librairii", "custom/data/home/librairii", "Library/Application Support/Librairii"} {
		t.Run(relative, func(t *testing.T) {
			home := t.TempDir()
			root := filepath.Join(home, filepath.FromSlash(relative))
			r := NewConfiguredStorageReadiness(t.TempDir(), root, "fr")
			if _, err := r.Check(context.Background()); !errors.Is(err, preferences.ErrSetupRequired) {
				t.Fatal(err)
			}
			if _, err := os.Stat(filepath.Dir(root)); !errors.Is(err, os.ErrNotExist) {
				t.Fatal("created parents before confirmation", err)
			}
			if err := r.InitializeLibrary(context.Background(), root); err != nil {
				t.Fatal(err)
			}
			report, err := r.Check(context.Background())
			if err != nil || !report.MutationsAllowed {
				t.Fatal(report, err)
			}
			defer r.Close()
			if !r.Profile().Complete || r.Profile().Root != root {
				t.Fatal(r.Profile())
			}
		})
	}
}
