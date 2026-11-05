package artwork

import (
	"bytes"
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/metadata"
	"github.com/01max/librairii/internal/storage"
)

const DefaultMaximumBytes int64 = 10 << 20

var ErrArtworkNotFound = errors.New("artwork was not found")

type Asset struct {
	Content     []byte
	ContentType string
	SHA256      string
}

func (r *Repository) PublishCatalog(
	artworkID string,
	mediaType string,
	content []byte,
	maximumBytes int64,
) (string, Asset, error) {
	if !validOpaqueID(artworkID) ||
		!supportedMediaType(mediaType) ||
		len(content) == 0 ||
		int64(len(content)) > maximumBytes ||
		http.DetectContentType(content) != mediaType {
		return "", Asset{}, ErrInvalidArtwork
	}
	checksum := sha256.Sum256(content)
	checksumText := hex.EncodeToString(checksum[:])
	relativePath := filepath.Join(
		"catalog",
		"official",
		artworkID,
		checksumText+extensionForMediaType(mediaType),
	)
	destination, err := archive.SafeJoin(r.layout.Root, relativePath)
	if err != nil {
		return "", Asset{}, ErrInvalidArtwork
	}
	officialRoot := filepath.Join(r.layout.Catalog, "official")
	if err := os.MkdirAll(officialRoot, storage.DirectoryMode); err != nil {
		return "", Asset{}, fmt.Errorf("create official artwork root: %w", err)
	}
	if contained, err := storage.PathContained(r.layout.Catalog, officialRoot); err != nil || !contained {
		return "", Asset{}, ErrInvalidArtwork
	}
	if err := archive.PrepareDestination(officialRoot, destination); err != nil {
		return "", Asset{}, fmt.Errorf("create official artwork directory: %w", err)
	}
	if info, err := os.Lstat(destination); err == nil {
		contained, containmentErr := storage.PathContained(officialRoot, destination)
		if containmentErr != nil ||
			!contained ||
			!info.Mode().IsRegular() ||
			info.Mode()&os.ModeSymlink != 0 {
			return "", Asset{}, ErrInvalidArtwork
		}
		existing, err := os.ReadFile(destination)
		if err != nil {
			return "", Asset{}, fmt.Errorf("read catalog artwork destination: %w", err)
		}
		if bytes.Equal(existing, content) {
			return filepath.ToSlash(relativePath), Asset{
				Content:     existing,
				ContentType: mediaType,
				SHA256:      checksumText,
			}, nil
		}
		return "", Asset{}, errors.New("catalog artwork destination collision")
	} else if !errors.Is(err, os.ErrNotExist) {
		return "", Asset{}, fmt.Errorf("inspect catalog artwork destination: %w", err)
	}

	stagingDirectory, err := os.MkdirTemp(r.layout.Catalog, ".official-artwork-*")
	if err != nil {
		return "", Asset{}, fmt.Errorf("create catalog artwork staging directory: %w", err)
	}
	defer os.RemoveAll(stagingDirectory)
	stagedPath := filepath.Join(stagingDirectory, "artwork")
	staged, err := os.OpenFile(stagedPath, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		return "", Asset{}, fmt.Errorf("create staged catalog artwork: %w", err)
	}
	if _, err := staged.Write(content); err != nil {
		_ = staged.Close()
		return "", Asset{}, fmt.Errorf("write staged catalog artwork: %w", err)
	}
	if err := staged.Sync(); err != nil {
		_ = staged.Close()
		return "", Asset{}, fmt.Errorf("sync staged catalog artwork: %w", err)
	}
	if err := staged.Close(); err != nil {
		return "", Asset{}, fmt.Errorf("close staged catalog artwork: %w", err)
	}

	if err := os.Rename(stagedPath, destination); err != nil {
		return "", Asset{}, fmt.Errorf("publish catalog artwork: %w", err)
	}
	return filepath.ToSlash(relativePath), Asset{
		Content:     append([]byte(nil), content...),
		ContentType: mediaType,
		SHA256:      checksumText,
	}, nil
}

func (r *Repository) LoadCatalog(
	record metadata.CatalogArtwork,
	maximumBytes int64,
) (Asset, error) {
	if !validOpaqueID(record.ID) ||
		record.ManagedPath == "" ||
		!supportedMediaType(record.ContentType) ||
		record.ByteSize <= 0 ||
		record.ByteSize > maximumBytes ||
		len(record.SHA256) != 64 {
		return Asset{}, ErrInvalidArtwork
	}
	expectedPrefix := filepath.ToSlash(filepath.Join(
		"catalog",
		"official",
		record.ID,
	))
	if !strings.HasPrefix(filepath.ToSlash(record.ManagedPath), expectedPrefix) {
		return Asset{}, ErrInvalidArtwork
	}
	path, err := archive.SafeJoin(r.layout.Root, record.ManagedPath)
	if err != nil {
		return Asset{}, ErrInvalidArtwork
	}
	return readValidatedAsset(
		filepath.Join(r.layout.Catalog, "official"),
		path,
		record.ContentType,
		record.SHA256,
		record.ByteSize,
		maximumBytes,
	)
}

func (r *Repository) LoadEmbedded(
	ctx context.Context,
	database *sql.DB,
	opaqueID string,
	maximumBytes int64,
) (Asset, error) {
	const prefix = "embedded:"
	if !strings.HasPrefix(opaqueID, prefix) {
		return Asset{}, ErrArtworkNotFound
	}
	storyID, err := strconv.ParseInt(strings.TrimPrefix(opaqueID, prefix), 10, 64)
	if err != nil || storyID <= 0 {
		return Asset{}, ErrArtworkNotFound
	}
	var relativePath string
	err = database.QueryRowContext(
		ctx,
		`SELECT embedded_artwork_path
		 FROM stories
		 WHERE id = ? AND embedded_artwork_path IS NOT NULL`,
		storyID,
	).Scan(&relativePath)
	if errors.Is(err, sql.ErrNoRows) {
		return Asset{}, ErrArtworkNotFound
	}
	if err != nil {
		return Asset{}, err
	}
	path, err := archive.SafeJoin(r.layout.Root, relativePath)
	if err != nil {
		return Asset{}, ErrInvalidArtwork
	}
	return readValidatedAsset(
		filepath.Join(r.layout.Catalog, "embedded"),
		path,
		"image/png",
		"",
		0,
		maximumBytes,
	)
}

func (r *Repository) RemoveCatalog(relativePath string) error {
	path, err := archive.SafeJoin(r.layout.Root, relativePath)
	if err != nil {
		return ErrInvalidArtwork
	}
	officialRoot := filepath.Join(r.layout.Catalog, "official")
	contained, err := storage.PathContained(officialRoot, path)
	if err != nil || !contained {
		return ErrInvalidArtwork
	}
	if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("remove catalog artwork: %w", err)
	}
	_ = removeEmptyParents(filepath.Dir(path), officialRoot)
	return nil
}

func readValidatedAsset(
	root string,
	path string,
	expectedType string,
	expectedSHA256 string,
	expectedSize int64,
	maximumBytes int64,
) (Asset, error) {
	info, err := os.Lstat(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return Asset{}, ErrArtworkNotFound
		}
		return Asset{}, err
	}
	contained, err := storage.PathContained(root, path)
	if err != nil ||
		!contained ||
		!info.Mode().IsRegular() ||
		info.Mode()&os.ModeSymlink != 0 ||
		info.Size() <= 0 ||
		info.Size() > maximumBytes ||
		expectedSize > 0 && info.Size() != expectedSize {
		return Asset{}, ErrInvalidArtwork
	}
	content, err := os.ReadFile(path)
	if err != nil {
		return Asset{}, err
	}
	if http.DetectContentType(content) != expectedType {
		return Asset{}, ErrInvalidArtwork
	}
	checksum := sha256.Sum256(content)
	checksumText := hex.EncodeToString(checksum[:])
	if expectedSHA256 != "" && checksumText != expectedSHA256 {
		return Asset{}, ErrInvalidArtwork
	}
	return Asset{
		Content:     content,
		ContentType: expectedType,
		SHA256:      checksumText,
	}, nil
}

func validOpaqueID(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range value {
		if character < '0' || character > '9' {
			if character < 'a' || character > 'f' {
				return false
			}
		}
	}
	return true
}

func supportedMediaType(value string) bool {
	switch value {
	case "image/png", "image/jpeg", "image/webp":
		return true
	default:
		return false
	}
}

func extensionForMediaType(value string) string {
	switch value {
	case "image/jpeg":
		return ".jpg"
	case "image/webp":
		return ".webp"
	default:
		return ".png"
	}
}
