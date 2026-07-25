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
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/storage"
)

var (
	ErrExportConflict       = errors.New("export destination file already exists")
	ErrExportSourceInvalid  = errors.New("managed export source is invalid")
	ErrExportChecksumFailed = errors.New("export checksum verification failed")
)

type CopyResult = operations.ExportCopyResult

type ProgressFunc = func(deltaBytes int64)

type Copier struct {
	layout     storage.Layout
	link       func(string, string) error
	openSource func(string) (io.ReadCloser, error)
}

func NewCopier(layout storage.Layout) (*Copier, error) {
	if layout.Root == "" ||
		layout.Archives == "" ||
		!filepath.IsAbs(layout.Root) ||
		!filepath.IsAbs(layout.Archives) {
		return nil, ErrExportSourceInvalid
	}
	return &Copier{
		layout: layout,
		link:   os.Link,
		openSource: func(path string) (io.ReadCloser, error) {
			return os.Open(path)
		},
	}, nil
}

func (c *Copier) Copy(
	ctx context.Context,
	item operations.NewItem,
	destination string,
	progress ProgressFunc,
) (CopyResult, error) {
	destination, err := ResolveDestination(destination)
	if err != nil {
		return CopyResult{}, err
	}
	if !supportedArchiveOutputName(item.OutputName) {
		return CopyResult{OutcomeCode: "source_invalid"}, ErrExportSourceInvalid
	}
	sourcePath, err := c.resolveSource(item.ArchiveRelativePath)
	if err != nil {
		return CopyResult{OutcomeCode: "source_invalid"}, err
	}
	finalPath := filepath.Join(destination, item.OutputName)
	if _, err := os.Lstat(finalPath); err == nil {
		return CopyResult{OutcomeCode: "filename_conflict"}, ErrExportConflict
	} else if !errors.Is(err, os.ErrNotExist) {
		return CopyResult{}, fmt.Errorf("inspect export destination: %w", err)
	}

	source, err := c.openSource(sourcePath)
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
	if checksum != item.ArchiveSHA256 || written != item.TotalBytes {
		return CopyResult{OutcomeCode: "checksum_mismatch"}, ErrExportChecksumFailed
	}
	if err := c.link(temporaryPath, finalPath); err != nil {
		if errors.Is(err, os.ErrExist) {
			return CopyResult{OutcomeCode: "filename_conflict"}, ErrExportConflict
		}
		return CopyResult{}, fmt.Errorf("publish export archive atomically: %w", err)
	}
	_ = os.Remove(temporaryPath)
	return CopyResult{
		OutputName:  item.OutputName,
		ByteSize:    written,
		SHA256:      checksum,
		OutcomeCode: "exported",
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
