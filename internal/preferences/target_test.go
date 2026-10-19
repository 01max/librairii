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
