package preferences

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"

	"github.com/google/uuid"
)

const moveMarker = ".librairii-move-owner"

type Entry struct {
	Path      string `json:"path"`
	Size      int64  `json:"size"`
	SHA256    string `json:"sha256,omitempty"`
	Directory bool   `json:"directory,omitempty"`
}
type Journal struct {
	ID          string  `json:"id"`
	LibraryID   string  `json:"libraryId"`
	Source      string  `json:"source"`
	Destination string  `json:"destination"`
	Phase       string  `json:"phase"`
	Entries     []Entry `json:"entries"`
}

func (j Journal) Workspace() string {
	return filepath.Join(filepath.Dir(j.Destination), ".librairii-move-"+j.ID)
}
func (j Journal) Validate() error {
	if _, err := uuid.Parse(j.ID); err != nil {
		return ErrInvalid
	}
	if _, err := uuid.Parse(j.LibraryID); err != nil {
		return ErrInvalid
	}
	if !filepath.IsAbs(j.Source) || !filepath.IsAbs(j.Destination) || Contains(j.Source, j.Destination) || Contains(j.Destination, j.Source) {
		return ErrInvalid
	}
	for _, entry := range j.Entries {
		if entry.Path == "" || filepath.IsAbs(entry.Path) || !Contains("/library", filepath.Join("/library", entry.Path)) {
			return ErrInvalid
		}
	}
	return nil
}
func WriteJournal(directory string, j Journal) error {
	if err := j.Validate(); err != nil {
		return err
	}
	return WriteJSON(filepath.Join(directory, "relocation.json"), j)
}
func ReadJournal(directory string) (Journal, error) {
	file, err := os.Open(filepath.Join(directory, "relocation.json"))
	if err != nil {
		return Journal{}, err
	}
	defer file.Close()
	var j Journal
	decoder := json.NewDecoder(io.LimitReader(file, 64<<20))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&j) != nil || j.Validate() != nil {
		return j, ErrInvalid
	}
	var extra any
	if decoder.Decode(&extra) != io.EOF {
		return j, ErrInvalid
	}
	return j, nil
}
func ClearJournal(directory string) error {
	err := os.Remove(filepath.Join(directory, "relocation.json"))
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	return err
}
