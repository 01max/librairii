package archive

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/01max/librairii/internal/storage"
)

var (
	ErrInvalidManagedPath = errors.New("managed archive path is invalid")
	ErrDestinationExists  = errors.New("managed archive destination already exists")
	ErrStagedFileChanged  = errors.New("staged archive changed after hashing")
)

type StagedFile struct {
	Directory        string
	Path             string
	OriginalFilename string
	SHA256           string
	ByteSize         int64
}

type Repository struct {
	layout storage.Layout
}

func NewRepository(layout storage.Layout) *Repository {
	return &Repository{layout: layout}
}

func (r *Repository) Stage(ctx context.Context, sourcePath string) (StagedFile, error) {
	source, err := os.Open(sourcePath)
	if err != nil {
		return StagedFile{}, fmt.Errorf("open import source: %w", err)
	}
	defer source.Close()

	info, err := source.Stat()
	if err != nil {
		return StagedFile{}, fmt.Errorf("inspect import source: %w", err)
	}
	if !info.Mode().IsRegular() {
		return StagedFile{}, fmt.Errorf("import source is not a regular file")
	}

	stagingDirectory, err := os.MkdirTemp(r.layout.Staging, "import-*")
	if err != nil {
		return StagedFile{}, fmt.Errorf("create import staging directory: %w", err)
	}
	stagedPath := filepath.Join(stagingDirectory, "archive")
	staged, err := os.OpenFile(stagedPath, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		_ = os.RemoveAll(stagingDirectory)
		return StagedFile{}, fmt.Errorf("create staged archive: %w", err)
	}

	hasher := sha256.New()
	written, copyErr := io.Copy(io.MultiWriter(staged, hasher), contextReader{ctx: ctx, source: source})
	syncErr := staged.Sync()
	closeErr := staged.Close()
	if copyErr != nil || syncErr != nil || closeErr != nil {
		_ = os.RemoveAll(stagingDirectory)
		switch {
		case copyErr != nil:
			return StagedFile{}, fmt.Errorf("copy import source: %w", copyErr)
		case syncErr != nil:
			return StagedFile{}, fmt.Errorf("sync staged archive: %w", syncErr)
		default:
			return StagedFile{}, fmt.Errorf("close staged archive: %w", closeErr)
		}
	}

	return StagedFile{
		Directory:        stagingDirectory,
		Path:             stagedPath,
		OriginalFilename: filepath.Base(sourcePath),
		SHA256:           hex.EncodeToString(hasher.Sum(nil)),
		ByteSize:         written,
	}, nil
}

func (r *Repository) Publish(staged StagedFile) (string, error) {
	if filepath.Base(staged.OriginalFilename) != staged.OriginalFilename ||
		staged.OriginalFilename == "." ||
		staged.OriginalFilename == ".." {
		return "", ErrInvalidManagedPath
	}
	contained, err := storage.PathContained(r.layout.Staging, staged.Path)
	if err != nil || !contained {
		return "", ErrInvalidManagedPath
	}

	checksum, size, err := hashFile(staged.Path)
	if err != nil {
		return "", err
	}
	if checksum != staged.SHA256 || size != staged.ByteSize {
		return "", ErrStagedFileChanged
	}

	relativePath := filepath.Join("archives", checksum, staged.OriginalFilename)
	destination, err := SafeJoin(r.layout.Root, relativePath)
	if err != nil {
		return "", err
	}
	if _, err := os.Stat(destination); err == nil {
		return "", ErrDestinationExists
	} else if !errors.Is(err, os.ErrNotExist) {
		return "", fmt.Errorf("inspect managed destination: %w", err)
	}

	destinationDirectory := filepath.Dir(destination)
	if err := os.MkdirAll(destinationDirectory, storage.DirectoryMode); err != nil {
		return "", fmt.Errorf("create managed archive directory: %w", err)
	}
	if err := os.Rename(staged.Path, destination); err != nil {
		_ = removeDirectoryIfEmpty(destinationDirectory)
		return "", fmt.Errorf("publish managed archive: %w", err)
	}
	if err := os.Remove(staged.Directory); err != nil && !errors.Is(err, os.ErrNotExist) {
		return "", fmt.Errorf("remove staging directory: %w", err)
	}

	return filepath.ToSlash(relativePath), nil
}

func (r *Repository) Cleanup(staged StagedFile) error {
	contained, err := storage.PathContained(r.layout.Staging, staged.Directory)
	if err != nil || !contained {
		return ErrInvalidManagedPath
	}
	if err := os.RemoveAll(staged.Directory); err != nil {
		return fmt.Errorf("clean import staging directory: %w", err)
	}
	return nil
}

func (r *Repository) CleanupAbandoned() error {
	entries, err := os.ReadDir(r.layout.Staging)
	if err != nil {
		return fmt.Errorf("read staging directory: %w", err)
	}
	for _, entry := range entries {
		path := filepath.Join(r.layout.Staging, entry.Name())
		contained, err := storage.PathContained(r.layout.Staging, path)
		if err != nil || !contained {
			return ErrInvalidManagedPath
		}
		if err := os.RemoveAll(path); err != nil {
			return fmt.Errorf("remove abandoned staging entry: %w", err)
		}
	}
	return nil
}

func (r *Repository) MoveToTrash(managedRelativePath string) (string, error) {
	source, err := SafeJoin(r.layout.Root, managedRelativePath)
	if err != nil {
		return "", err
	}
	contained, err := storage.PathContained(r.layout.Archives, source)
	if err != nil || !contained {
		return "", ErrInvalidManagedPath
	}

	trashDirectory, err := os.MkdirTemp(r.layout.Trash, "removed-*")
	if err != nil {
		return "", fmt.Errorf("create trash directory: %w", err)
	}
	destination := filepath.Join(trashDirectory, filepath.Base(source))
	if err := os.Rename(source, destination); err != nil {
		_ = os.RemoveAll(trashDirectory)
		return "", fmt.Errorf("move archive to trash: %w", err)
	}
	_ = removeDirectoryIfEmpty(filepath.Dir(source))

	relative, err := filepath.Rel(r.layout.Root, destination)
	if err != nil {
		return "", err
	}
	return filepath.ToSlash(relative), nil
}

func (r *Repository) Resolve(managedRelativePath string) (string, error) {
	path, err := SafeJoin(r.layout.Root, managedRelativePath)
	if err != nil {
		return "", err
	}
	contained, err := storage.PathContained(r.layout.Root, path)
	if err != nil || !contained {
		return "", ErrInvalidManagedPath
	}
	return path, nil
}

func SafeJoin(root string, relativePath string) (string, error) {
	if relativePath == "" || filepath.IsAbs(relativePath) {
		return "", ErrInvalidManagedPath
	}
	clean := filepath.Clean(filepath.FromSlash(relativePath))
	if clean == "." || clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
		return "", ErrInvalidManagedPath
	}

	candidate := filepath.Join(root, clean)
	relative, err := filepath.Rel(root, candidate)
	if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return "", ErrInvalidManagedPath
	}
	return candidate, nil
}

func hashFile(path string) (string, int64, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", 0, fmt.Errorf("open archive for hashing: %w", err)
	}
	defer file.Close()

	hasher := sha256.New()
	size, err := io.Copy(hasher, file)
	if err != nil {
		return "", 0, fmt.Errorf("hash archive: %w", err)
	}
	return hex.EncodeToString(hasher.Sum(nil)), size, nil
}

func removeDirectoryIfEmpty(path string) error {
	return os.Remove(path)
}

type contextReader struct {
	ctx    context.Context
	source io.Reader
}

func (r contextReader) Read(buffer []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.source.Read(buffer)
}
