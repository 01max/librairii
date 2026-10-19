package preferences

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"sync"
)

type Store struct {
	mu        sync.Mutex
	directory string
	profile   Profile
	loaded    bool
}

func NewStore(directory string) *Store { return &Store{directory: directory} }
func (s *Store) Directory() string     { return s.directory }
func (s *Store) Load() (Profile, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.loaded {
		return s.profile, nil
	}
	file, err := os.Open(filepath.Join(s.directory, "settings.json"))
	if err != nil {
		return Profile{}, err
	}
	defer file.Close()
	decoder := json.NewDecoder(io.LimitReader(file, 64*1024))
	decoder.DisallowUnknownFields()
	var p Profile
	if decoder.Decode(&p) != nil || p.Validate() != nil {
		return Profile{}, ErrInvalid
	}
	var extra any
	if decoder.Decode(&extra) != io.EOF {
		return Profile{}, ErrInvalid
	}
	s.profile = p
	s.loaded = true
	return p, nil
}
func (s *Store) Update(change func(*Profile) error) (Profile, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	next := s.profile
	if !s.loaded {
		next = Profile{Version: Version, Language: "en"}
	}
	if err := change(&next); err != nil {
		return s.profile, err
	}
	if err := next.Validate(); err != nil {
		return s.profile, err
	}
	if err := WriteJSON(filepath.Join(s.directory, "settings.json"), next); err != nil {
		return s.profile, err
	}
	s.profile = next
	s.loaded = true
	return next, nil
}

// WriteJSON publishes a complete synchronized document in a single rename.
func WriteJSON(path string, value any) error {
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		return err
	}
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	file, err := os.CreateTemp(filepath.Dir(path), ".settings-*")
	if err != nil {
		return err
	}
	defer os.Remove(file.Name())
	if err = file.Chmod(0600); err == nil {
		_, err = file.Write(data)
	}
	if err == nil {
		err = file.Sync()
	}
	err = errors.Join(err, file.Close())
	if err != nil {
		return err
	}
	if err = os.Rename(file.Name(), path); err != nil {
		return err
	}
	directory, err := os.Open(filepath.Dir(path))
	if err != nil {
		return err
	}
	defer directory.Close()
	return directory.Sync()
}
