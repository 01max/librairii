package preferences

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/database"
	"os"
	"path/filepath"
	"testing"
)

func TestBootstrapAdoptsAndDoesNotReplace(t *testing.T) {
	ctx := context.Background()
	legacy := filepath.Join(t.TempDir(), "old")
	os.MkdirAll(filepath.Join(legacy, "db"), 0700)
	db, err := database.Open(ctx, DatabasePath(legacy))
	if err != nil {
		t.Fatal(err)
	}
	db.Close()
	s := NewStore(t.TempDir())
	p, err := Bootstrap(ctx, s, legacy, "fr")
	if err != nil || !p.Complete || p.Root != legacy {
		t.Fatalf("%+v %v", p, err)
	}
	if err = ValidateLibrary(ctx, p.Root, p.LibraryID); err != nil {
		t.Fatal(err)
	}
	os.Remove(DatabasePath(legacy))
	if err = ValidateLibrary(ctx, p.Root, p.LibraryID); !errors.Is(err, ErrMissingLibrary) {
		t.Fatal(err)
	}
}
func TestBootstrapDoesNotCreateNewRoot(t *testing.T) {
	legacy := filepath.Join(t.TempDir(), "new")
	_, err := Bootstrap(context.Background(), NewStore(t.TempDir()), legacy, "en")
	if !errors.Is(err, ErrSetupRequired) {
		t.Fatal(err)
	}
	if _, err = os.Stat(legacy); !errors.Is(err, os.ErrNotExist) {
		t.Fatal(err)
	}
}
