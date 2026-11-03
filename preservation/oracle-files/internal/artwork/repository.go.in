package artwork

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/storage"
	"github.com/google/uuid"
)

var ErrInvalidArtwork = errors.New("embedded artwork is invalid")

type Repository struct {
	layout storage.Layout
}

func NewRepository(layout storage.Layout) *Repository {
	return &Repository{layout: layout}
}

func (r *Repository) Publish(storyUUID string, mediaType string, content []byte) (string, error) {
	parsedUUID, err := uuid.Parse(storyUUID)
	if err != nil || parsedUUID == uuid.Nil || mediaType != "image/png" || len(content) == 0 {
		return "", ErrInvalidArtwork
	}

	checksum := sha256.Sum256(content)
	relativePath := filepath.Join(
		"catalog",
		"embedded",
		parsedUUID.String(),
		hex.EncodeToString(checksum[:])+".png",
	)
	destination, err := archive.SafeJoin(r.layout.Root, relativePath)
	if err != nil {
		return "", ErrInvalidArtwork
	}
	if existing, err := os.ReadFile(destination); err == nil {
		if bytes.Equal(existing, content) {
			return filepath.ToSlash(relativePath), nil
		}
		return "", fmt.Errorf("embedded artwork destination collision")
	} else if !errors.Is(err, os.ErrNotExist) {
		return "", fmt.Errorf("inspect embedded artwork destination: %w", err)
	}

	stagingDirectory, err := os.MkdirTemp(r.layout.Catalog, ".artwork-*")
	if err != nil {
		return "", fmt.Errorf("create artwork staging directory: %w", err)
	}
	defer os.RemoveAll(stagingDirectory)
	stagedPath := filepath.Join(stagingDirectory, "artwork.png")
	staged, err := os.OpenFile(stagedPath, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		return "", fmt.Errorf("create staged artwork: %w", err)
	}
	if _, err := staged.Write(content); err != nil {
		_ = staged.Close()
		return "", fmt.Errorf("write staged artwork: %w", err)
	}
	if err := staged.Sync(); err != nil {
		_ = staged.Close()
		return "", fmt.Errorf("sync staged artwork: %w", err)
	}
	if err := staged.Close(); err != nil {
		return "", fmt.Errorf("close staged artwork: %w", err)
	}

	embeddedRoot := filepath.Join(r.layout.Catalog, "embedded")
	if err := os.MkdirAll(embeddedRoot, storage.DirectoryMode); err != nil {
		return "", fmt.Errorf("create embedded artwork root: %w", err)
	}
	if contained, err := storage.PathContained(r.layout.Catalog, embeddedRoot); err != nil || !contained {
		return "", ErrInvalidArtwork
	}
	if err := archive.PrepareDestination(embeddedRoot, destination); err != nil {
		return "", fmt.Errorf("create embedded artwork directory: %w", err)
	}
	if err := os.Rename(stagedPath, destination); err != nil {
		return "", fmt.Errorf("publish embedded artwork: %w", err)
	}
	return filepath.ToSlash(relativePath), nil
}

func (r *Repository) Remove(relativePath string) error {
	path, err := archive.SafeJoin(r.layout.Root, relativePath)
	if err != nil {
		return ErrInvalidArtwork
	}
	embeddedRoot := filepath.Join(r.layout.Catalog, "embedded")
	contained, err := storage.PathContained(embeddedRoot, path)
	if err != nil || !contained {
		return ErrInvalidArtwork
	}
	if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("remove embedded artwork: %w", err)
	}
	_ = removeEmptyParents(filepath.Dir(path), embeddedRoot)
	return nil
}

func (r *Repository) Resolve(relativePath string) (string, error) {
	path, err := archive.SafeJoin(r.layout.Root, relativePath)
	if err != nil {
		return "", ErrInvalidArtwork
	}
	embeddedRoot := filepath.Join(r.layout.Catalog, "embedded")
	contained, err := storage.PathContained(embeddedRoot, path)
	if err != nil || !contained {
		return "", ErrInvalidArtwork
	}
	return path, nil
}

func removeEmptyParents(directory string, stop string) error {
	for {
		relative, err := filepath.Rel(stop, directory)
		if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			return nil
		}
		if directory == stop {
			return nil
		}
		if err := os.Remove(directory); err != nil {
			return nil
		}
		directory = filepath.Dir(directory)
	}
}
