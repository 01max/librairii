package exporter

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/01max/librairii/internal/archive"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/storage"
)

var (
	ErrExportConflict       = errors.New("export destination file already exists")
	ErrExportSourceInvalid  = errors.New("managed export source is invalid")
	ErrExportChecksumFailed = errors.New("export checksum verification failed")
)

type CopyResult struct {
	OutputName string
	ByteSize   int64
	SHA256     string
}

type ProgressFunc func(deltaBytes int64)

type Copier struct {
	layout storage.Layout
	link   func(string, string) error
}

func NewCopier(layout storage.Layout) (*Copier, error) {
	if layout.Root == "" ||
		layout.Archives == "" ||
		!filepath.IsAbs(layout.Root) ||
		!filepath.IsAbs(layout.Archives) {
		return nil, ErrExportSourceInvalid
	}
	return &Copier{layout: layout, link: os.Link}, nil
}

func (c *Copier) Copy(
	ctx context.Context,
	story library.ExportStory,
	destination string,
	progress ProgressFunc,
) (CopyResult, error) {
	destination, err := ResolveDestination(destination)
	if err != nil {
		return CopyResult{}, err
	}
	if !supportedOutputName(story.OriginalFilename, story.DetectedFormat) {
		return CopyResult{}, ErrExportSourceInvalid
	}
	sourcePath, err := c.resolveSource(story.ManagedRelativePath)
	if err != nil {
		return CopyResult{}, err
	}
	finalPath := filepath.Join(destination, story.OriginalFilename)
	if _, err := os.Lstat(finalPath); err == nil {
		return CopyResult{}, ErrExportConflict
	} else if !errors.Is(err, os.ErrNotExist) {
		return CopyResult{}, fmt.Errorf("inspect export destination: %w", err)
	}

	source, err := os.Open(sourcePath)
	if err != nil {
		return CopyResult{}, fmt.Errorf("%w: open source: %v", ErrExportSourceInvalid, err)
	}
	defer source.Close()
	temporary, err := os.CreateTemp(destination, ".librairii-export-*.tmp")
	if err != nil {
		return CopyResult{}, fmt.Errorf("create destination temporary file: %w", err)
	}
	temporaryPath := temporary.Name()
	temporaryClosed := false
	defer func() {
		if !temporaryClosed {
			_ = temporary.Close()
		}
		_ = os.Remove(temporaryPath)
	}()
	if err := temporary.Chmod(0o600); err != nil {
		return CopyResult{}, fmt.Errorf("set export temporary permissions: %w", err)
	}

	hasher := sha256.New()
	writer := io.MultiWriter(temporary, hasher)
	if progress != nil {
		writer = io.MultiWriter(temporary, hasher, progressWriter{progress: progress})
	}
	written, copyErr := io.Copy(
		writer,
		exportContextReader{ctx: ctx, source: source},
	)
	syncErr := temporary.Sync()
	closeErr := temporary.Close()
	temporaryClosed = true
	switch {
	case copyErr != nil:
		return CopyResult{}, fmt.Errorf("copy managed export archive: %w", copyErr)
	case syncErr != nil:
		return CopyResult{}, fmt.Errorf("sync destination temporary file: %w", syncErr)
	case closeErr != nil:
		return CopyResult{}, fmt.Errorf("close destination temporary file: %w", closeErr)
	}

	checksum := hex.EncodeToString(hasher.Sum(nil))
	if checksum != story.SHA256 || written != story.ByteSize {
		return CopyResult{}, ErrExportChecksumFailed
	}
	if err := c.link(temporaryPath, finalPath); err != nil {
		if errors.Is(err, os.ErrExist) {
			return CopyResult{}, ErrExportConflict
		}
		return CopyResult{}, fmt.Errorf("publish export archive atomically: %w", err)
	}
	_ = os.Remove(temporaryPath)
	return CopyResult{
		OutputName: story.OriginalFilename,
		ByteSize:   written,
		SHA256:     checksum,
	}, nil
}

func (c *Copier) resolveSource(relativePath string) (string, error) {
	sourcePath, err := archive.SafeJoin(c.layout.Root, relativePath)
	if err != nil {
		return "", ErrExportSourceInvalid
	}
	info, err := os.Lstat(sourcePath)
	if err != nil {
		return "", fmt.Errorf("%w: %v", ErrExportSourceInvalid, err)
	}
	contained, containmentErr := storage.PathContained(c.layout.Archives, sourcePath)
	if containmentErr != nil ||
		!contained ||
		!info.Mode().IsRegular() ||
		info.Mode()&os.ModeSymlink != 0 {
		return "", ErrExportSourceInvalid
	}
	return sourcePath, nil
}

type progressWriter struct {
	progress ProgressFunc
}

func (w progressWriter) Write(buffer []byte) (int, error) {
	w.progress(int64(len(buffer)))
	return len(buffer), nil
}
