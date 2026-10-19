package preferences

import (
	"errors"
	"os"
	"path/filepath"
	"sync"
	"testing"
)

func TestStoreAtomicUpdates(t *testing.T) {
	dir := t.TempDir()
	s := NewStore(dir)
	if _, err := s.Load(); !errors.Is(err, os.ErrNotExist) {
		t.Fatal(err)
	}
	if _, err := s.Update(func(p *Profile) error { p.Language = "fr"; return nil }); err != nil {
		t.Fatal(err)
	}
	var wg sync.WaitGroup
	for i := 0; i < 20; i++ {
		wg.Go(func() {
			_, err := s.Update(func(p *Profile) error { p.Root = "/library"; p.LibraryID = "test"; p.Complete = true; return nil })
			if err != nil {
				t.Error(err)
			}
		})
	}
	wg.Wait()
	p, err := NewStore(dir).Load()
	if err != nil || p.Language != "fr" || p.Root != "/library" {
		t.Fatalf("%+v %v", p, err)
	}
	_, err = s.Update(func(p *Profile) error { p.Language = "de"; return nil })
	if !errors.Is(err, ErrInvalid) {
		t.Fatal(err)
	}
	p, _ = s.Load()
	if p.Language != "fr" {
		t.Fatal(p)
	}
}
func TestStoreRejectsCorruptionAndWriteFailure(t *testing.T) {
	for _, data := range []string{`{`, `{"version":2,"language":"en"}`, `{"version":1,"language":"en"} {}`} {
		dir := t.TempDir()
		os.WriteFile(filepath.Join(dir, "settings.json"), []byte(data), 0600)
		if _, err := NewStore(dir).Load(); !errors.Is(err, ErrInvalid) {
			t.Fatalf("%q %v", data, err)
		}
	}
	path := filepath.Join(t.TempDir(), "file")
	os.WriteFile(path, []byte("keep"), 0600)
	s := NewStore(path)
	if _, err := s.Update(func(p *Profile) error { p.Language = "fr"; return nil }); err == nil {
		t.Fatal("write succeeded")
	}
}
func TestConfigPaths(t *testing.T) {
	get := func(string) string { return "" }
	mac, _ := PlatformDirectory("darwin", "/home", get)
	linux, _ := PlatformDirectory("linux", "/home", get)
	if mac != "/home/Library/Preferences/io.github.librairii.app" || linux != "/home/.config/librairii" {
		t.Fatal(mac, linux)
	}
}
