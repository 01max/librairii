package preferences

import (
	"os"
	"path/filepath"
	"testing"
)

func TestTargetValidationAndReservation(t *testing.T) {
	base := t.TempDir()
	source := filepath.Join(base, "source")
	config := filepath.Join(base, "profile")
	target := filepath.Join(base, "bibliothèque choisie")
	os.Mkdir(source, 0700)
	os.Mkdir(config, 0700)
	for _, bad := range []string{base, source, filepath.Join(source, "nested"), config, filepath.Join(config, "nested")} {
		if EmptyTarget(bad, config, source) == nil {
			t.Fatal("accepted", bad)
		}
	}
	if err := EmptyTarget(target, config, source); err != nil {
		t.Fatal(err)
	}
	if err := Reserve(target, "owner", 0); err != nil {
		t.Fatal(err)
	}
	work := filepath.Join(base, "work")
	os.Mkdir(work, 0700)
	os.WriteFile(filepath.Join(work, "new"), []byte("copy"), 0600)
	os.WriteFile(filepath.Join(target, "foreign"), []byte("keep"), 0600)
	if Publish(work, target, "owner") == nil {
		t.Fatal("overwrote concurrent content")
	}
	if data, err := os.ReadFile(filepath.Join(target, "foreign")); err != nil || string(data) != "keep" {
		t.Fatal(err)
	}
}
func TestSourceLinksAreRejected(t *testing.T) {
	source := t.TempDir()
	os.Symlink(t.TempDir(), filepath.Join(source, "escape"))
	if _, err := TreeSize(source); err == nil {
		t.Fatal("followed source link")
	}
}

func TestMissingTargetAncestorsAreValidatedWithoutCreation(t *testing.T) {
	base := t.TempDir()
	actual := filepath.Join(base, "actual")
	if err := os.Mkdir(actual, 0700); err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(base, "alias")
	if err := os.Symlink(actual, alias); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(alias, ".local", "share", "librairii")
	canonical, err := Canonical(target)
	resolved, resolveErr := filepath.EvalSymlinks(actual)
	if err != nil || resolveErr != nil || canonical != filepath.Join(resolved, ".local", "share", "librairii") {
		t.Fatal(canonical, err, resolveErr)
	}
	if err := EmptyTarget(target, filepath.Join(base, "config"), ""); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(actual, ".local")); !os.IsNotExist(err) {
		t.Fatal("validation created parents", err)
	}
	if err := EmptyTarget(target, filepath.Join(alias, ".local", "share", "librairii", "config"), ""); err == nil {
		t.Fatal("accepted configuration inside target")
	}
	if err := os.Symlink(filepath.Join(base, "absent"), filepath.Join(base, "dangling")); err != nil {
		t.Fatal(err)
	}
	if _, err := Canonical(filepath.Join(base, "dangling", "child")); err == nil {
		t.Fatal("accepted dangling ancestor")
	}
}
