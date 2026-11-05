package metadata

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"github.com/01max/librairii/internal/storage"
	"github.com/google/uuid"
)

var (
	ErrInvalidSnapshotID   = errors.New("catalog snapshot identifier is invalid")
	ErrInvalidSnapshotPath = errors.New("catalog snapshot path is invalid")
	ErrSnapshotExists      = errors.New("catalog snapshot destination already exists")
)

type StagedRawSnapshot struct {
	SyncID    string
	Directory string
	Path      string
	SHA256    string
	ByteSize  int64
}

type RawSnapshotStore struct {
	layout storage.Layout
}

func NewRawSnapshotStore(layout storage.Layout) *RawSnapshotStore {
	return &RawSnapshotStore{layout: layout}
}

func (s *RawSnapshotStore) Stage(
	ctx context.Context,
	syncID string,
	payload []byte,
) (StagedRawSnapshot, error) {
	if !validSyncID(syncID) {
		return StagedRawSnapshot{}, ErrInvalidSnapshotID
	}
	if err := ctx.Err(); err != nil {
		return StagedRawSnapshot{}, err
	}
	directory, err := os.MkdirTemp(s.layout.Staging, "catalog-"+syncID+"-*")
	if err != nil {
		return StagedRawSnapshot{}, fmt.Errorf("create catalog staging directory: %w", err)
	}
	cleanup := func(stageErr error) (StagedRawSnapshot, error) {
		_ = os.RemoveAll(directory)
		return StagedRawSnapshot{}, stageErr
	}

	path := filepath.Join(directory, "catalog.json")
	file, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		return cleanup(fmt.Errorf("create staged catalog: %w", err))
	}
	hasher := sha256.New()
	written, writeErr := writePayload(ctx, file, hasher, payload)
	syncErr := file.Sync()
	closeErr := file.Close()
	switch {
	case writeErr != nil:
		return cleanup(fmt.Errorf("write staged catalog: %w", writeErr))
	case syncErr != nil:
		return cleanup(fmt.Errorf("sync staged catalog: %w", syncErr))
	case closeErr != nil:
		return cleanup(fmt.Errorf("close staged catalog: %w", closeErr))
	}
	return StagedRawSnapshot{
		SyncID:    syncID,
		Directory: directory,
		Path:      path,
		SHA256:    hex.EncodeToString(hasher.Sum(nil)),
		ByteSize:  written,
	}, nil
}

func writePayload(
	ctx context.Context,
	file *os.File,
	hasher interface{ Write([]byte) (int, error) },
	payload []byte,
) (int64, error) {
	const chunkSize = 64 * 1024
	var written int64
	for len(payload) > 0 {
		if err := ctx.Err(); err != nil {
			return written, err
		}
		size := min(len(payload), chunkSize)
		chunk := payload[:size]
		count, err := file.Write(chunk)
		if count > 0 {
			_, _ = hasher.Write(chunk[:count])
			written += int64(count)
		}
		if err != nil {
			return written, err
		}
		if count != len(chunk) {
			return written, errors.New("short catalog snapshot write")
		}
		payload = payload[size:]
	}
	return written, nil
}

func (s *RawSnapshotStore) Publish(staged StagedRawSnapshot) (string, error) {
	if !validSyncID(staged.SyncID) {
		return "", ErrInvalidSnapshotID
	}
	contained, err := storage.PathContained(s.layout.Staging, staged.Path)
	if err != nil || !contained || filepath.Base(staged.Path) != "catalog.json" {
		return "", ErrInvalidSnapshotPath
	}
	destinationDirectory := filepath.Join(s.layout.Catalog, staged.SyncID)
	if err := os.Mkdir(destinationDirectory, storage.DirectoryMode); err != nil {
		if errors.Is(err, os.ErrExist) {
			return "", ErrSnapshotExists
		}
		return "", fmt.Errorf("create catalog snapshot directory: %w", err)
	}
	removeDestination := true
	defer func() {
		if removeDestination {
			_ = os.RemoveAll(destinationDirectory)
		}
	}()

	contained, err = storage.PathContained(s.layout.Catalog, destinationDirectory)
	if err != nil || !contained {
		return "", ErrInvalidSnapshotPath
	}
	destination := filepath.Join(destinationDirectory, "catalog.json")
	if err := os.Rename(staged.Path, destination); err != nil {
		return "", fmt.Errorf("publish catalog snapshot: %w", err)
	}
	if err := os.Remove(staged.Directory); err != nil && !errors.Is(err, os.ErrNotExist) {
		return "", fmt.Errorf("remove catalog staging directory: %w", err)
	}
	removeDestination = false
	return filepath.ToSlash(filepath.Join("catalog", staged.SyncID, "catalog.json")), nil
}

func (s *RawSnapshotStore) Cleanup(staged StagedRawSnapshot) error {
	contained, err := storage.PathContained(s.layout.Staging, staged.Directory)
	if err != nil || !contained {
		return ErrInvalidSnapshotPath
	}
	if err := os.RemoveAll(staged.Directory); err != nil {
		return fmt.Errorf("clean catalog staging directory: %w", err)
	}
	return nil
}

func (s *RawSnapshotStore) Remove(relativePath string) error {
	expectedID := filepath.Base(filepath.Dir(filepath.FromSlash(relativePath)))
	if !validSyncID(expectedID) ||
		filepath.ToSlash(relativePath) != filepath.ToSlash(
			filepath.Join("catalog", expectedID, "catalog.json"),
		) {
		return ErrInvalidSnapshotPath
	}
	directory := filepath.Join(s.layout.Catalog, expectedID)
	contained, err := storage.PathContained(s.layout.Catalog, directory)
	if err != nil || !contained {
		return ErrInvalidSnapshotPath
	}
	if err := os.RemoveAll(directory); err != nil {
		return fmt.Errorf("remove catalog snapshot: %w", err)
	}
	return nil
}

func (s *RawSnapshotStore) Resolve(relativePath string) (string, error) {
	expectedID := filepath.Base(filepath.Dir(filepath.FromSlash(relativePath)))
	if !validSyncID(expectedID) ||
		filepath.ToSlash(relativePath) != filepath.ToSlash(
			filepath.Join("catalog", expectedID, "catalog.json"),
		) {
		return "", ErrInvalidSnapshotPath
	}
	path := filepath.Join(s.layout.Root, filepath.FromSlash(relativePath))
	contained, err := storage.PathContained(s.layout.Catalog, path)
	if err != nil || !contained {
		return "", ErrInvalidSnapshotPath
	}
	return path, nil
}

func validSyncID(value string) bool {
	parsed, err := uuid.Parse(value)
	return err == nil && parsed.String() == value
}
