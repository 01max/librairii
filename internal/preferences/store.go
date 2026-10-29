package preferences

import (
	"encoding/json"
	"errors"
	"github.com/01max/librairii/internal/storage"
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
	failure   error
	publish   func(string, any) error
}

func NewStore(directory string) *Store { return &Store{directory: directory} }
func (s *Store) Directory() string     { return s.directory }
func (s *Store) Load() (Profile, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.loadLocked()
}

func (s *Store) loadLocked() (Profile, error) {
	if s.failure != nil {
		return Profile{}, s.failure
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
	if s.failure != nil {
		return s.profile, s.failure
	}
	if err := os.MkdirAll(s.directory, 0700); err != nil {
		return s.profile, err
	}
	guard, err := storage.AcquirePreferenceUpdate(s.directory)
	if err != nil {
		return s.profile, err
	}
	defer guard.Close()
	wasLoaded := s.loaded
	if _, err := s.loadLocked(); err != nil {
		if !errors.Is(err, os.ErrNotExist) {
			return s.profile, err
		}
		if wasLoaded {
			return s.profile, ErrInvalid
		}
	}
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
	publish := s.publish
	if publish == nil {
		publish = WriteJSON
	}
	if err := publish(filepath.Join(s.directory, "settings.json"), next); err != nil {
		if errors.Is(err, ErrCommitUncertain) {
			s.failure = err
		}
		return s.profile, err
	}
	s.profile = next
	s.loaded = true
	return next, nil
}

var ErrCommitUncertain = errors.New("settings.restart_required")

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
		return errors.Join(ErrCommitUncertain, err)
	}
	defer directory.Close()
	if err := directory.Sync(); err != nil {
		return errors.Join(ErrCommitUncertain, err)
	}
	return nil
}
