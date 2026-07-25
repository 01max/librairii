package diagnostics

import (
	"archive/zip"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/01max/librairii/internal/storage"
	"github.com/google/uuid"
)

const formatVersion = 1

type Manifest struct {
	FormatVersion   int    `json:"formatVersion"`
	GeneratedAt     string `json:"generatedAt"`
	OperatingSystem string `json:"operatingSystem"`
	Architecture    string `json:"architecture"`
	SchemaVersion   int    `json:"schemaVersion"`
	StoryCount      int    `json:"storyCount"`
	ShelfCount      int    `json:"shelfCount"`
	OperationCount  int    `json:"operationCount"`
}

type Report struct {
	FileName string `json:"fileName"`
	ByteSize int64  `json:"byteSize"`
}

type Service struct {
	layout storage.Layout
	db     *sql.DB
	logger *Logger
	now    func() time.Time
	newID  func() string
}

func NewService(
	layout storage.Layout,
	database *sql.DB,
	logger *Logger,
) (*Service, error) {
	if layout.Root == "" ||
		!filepath.IsAbs(layout.Root) ||
		database == nil ||
		logger == nil {
		return nil, errors.New("diagnostic export dependency is invalid")
	}
	return &Service{
		layout: layout,
		db:     database,
		logger: logger,
		now:    time.Now,
		newID:  uuid.NewString,
	}, nil
}

func (s *Service) Export(
	ctx context.Context,
	destination string,
) (Report, error) {
	destination, err := resolveDestination(destination)
	if err != nil {
		return Report{}, err
	}
	manifest, err := s.manifest(ctx)
	if err != nil {
		return Report{}, err
	}
	logs, err := s.logger.snapshots()
	if err != nil {
		return Report{}, err
	}
	identifier := s.newID()
	if _, err := uuid.Parse(identifier); err != nil {
		return Report{}, errors.New("diagnostic export identifier is invalid")
	}
	fileName := "librairii-diagnostics-" +
		s.now().UTC().Format("20060102T150405.000000000Z") +
		"-" + strings.ReplaceAll(identifier, "-", "")[:8] +
		".zip"
	temporary, err := os.CreateTemp(
		destination,
		".librairii-diagnostics-*.tmp",
	)
	if err != nil {
		return Report{}, fmt.Errorf("create diagnostic temporary: %w", err)
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
		return Report{}, fmt.Errorf("protect diagnostic temporary: %w", err)
	}

	writer := zip.NewWriter(temporary)
	manifestBytes, err := json.MarshalIndent(manifest, "", "  ")
	if err == nil {
		manifestBytes = append(manifestBytes, '\n')
		err = writeZipFile(writer, "diagnostics.json", manifestBytes)
	}
	for _, log := range logs {
		if err != nil {
			break
		}
		err = writeZipFile(writer, "logs/"+log.name, log.bytes)
	}
	closeZipErr := writer.Close()
	syncErr := temporary.Sync()
	closeErr := temporary.Close()
	temporaryClosed = true
	switch {
	case err != nil:
		return Report{}, err
	case closeZipErr != nil:
		return Report{}, fmt.Errorf("close diagnostic archive: %w", closeZipErr)
	case syncErr != nil:
		return Report{}, fmt.Errorf("sync diagnostic archive: %w", syncErr)
	case closeErr != nil:
		return Report{}, fmt.Errorf("close diagnostic temporary: %w", closeErr)
	}

	finalPath := filepath.Join(destination, fileName)
	if err := publishNoReplace(temporaryPath, finalPath); err != nil {
		return Report{}, err
	}
	info, err := os.Stat(finalPath)
	if err != nil {
		return Report{}, fmt.Errorf("inspect diagnostic archive: %w", err)
	}
	_ = os.Remove(temporaryPath)
	_ = s.logger.Record(LevelInfo, EventDiagnosticsExport, "succeeded")
	return Report{FileName: fileName, ByteSize: info.Size()}, nil
}

func (s *Service) manifest(ctx context.Context) (Manifest, error) {
	manifest := Manifest{
		FormatVersion:   formatVersion,
		GeneratedAt:     s.now().UTC().Format(time.RFC3339Nano),
		OperatingSystem: runtime.GOOS,
		Architecture:    runtime.GOARCH,
	}
	queries := []struct {
		statement string
		target    *int
	}{
		{
			statement: "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
			target:    &manifest.SchemaVersion,
		},
		{
			statement: "SELECT COUNT(*) FROM stories",
			target:    &manifest.StoryCount,
		},
		{
			statement: "SELECT COUNT(*) FROM shelves",
			target:    &manifest.ShelfCount,
		},
		{
			statement: "SELECT COUNT(*) FROM file_operations",
			target:    &manifest.OperationCount,
		},
	}
	for _, query := range queries {
		if err := s.db.QueryRowContext(ctx, query.statement).Scan(query.target); err != nil {
			return Manifest{}, fmt.Errorf("collect diagnostic aggregate: %w", err)
		}
	}
	return manifest, nil
}

func resolveDestination(path string) (string, error) {
	if !filepath.IsAbs(path) {
		return "", errors.New("diagnostic destination is invalid")
	}
	resolved, err := filepath.EvalSymlinks(filepath.Clean(path))
	if err != nil {
		return "", errors.New("diagnostic destination is invalid")
	}
	info, err := os.Stat(resolved)
	if err != nil || !info.IsDir() {
		return "", errors.New("diagnostic destination is invalid")
	}
	return resolved, nil
}

func writeZipFile(writer *zip.Writer, name string, payload []byte) error {
	header := &zip.FileHeader{
		Name:     name,
		Method:   zip.Deflate,
		Modified: time.Unix(0, 0).UTC(),
	}
	header.SetMode(0o600)
	target, err := writer.CreateHeader(header)
	if err != nil {
		return fmt.Errorf("create diagnostic entry: %w", err)
	}
	if _, err := target.Write(payload); err != nil {
		return fmt.Errorf("write diagnostic entry: %w", err)
	}
	return nil
}

func publishNoReplace(temporaryPath string, finalPath string) error {
	if err := os.Link(temporaryPath, finalPath); err == nil {
		return nil
	} else if errors.Is(err, os.ErrExist) {
		return errors.New("diagnostic archive already exists")
	}
	source, err := os.Open(temporaryPath)
	if err != nil {
		return fmt.Errorf("open diagnostic temporary: %w", err)
	}
	defer source.Close()
	target, err := os.OpenFile(
		finalPath,
		os.O_CREATE|os.O_EXCL|os.O_WRONLY,
		0o600,
	)
	if errors.Is(err, os.ErrExist) {
		return errors.New("diagnostic archive already exists")
	}
	if err != nil {
		return fmt.Errorf("create diagnostic archive: %w", err)
	}
	published := false
	defer func() {
		_ = target.Close()
		if !published {
			_ = os.Remove(finalPath)
		}
	}()
	if _, err := io.Copy(target, source); err != nil {
		return fmt.Errorf("publish diagnostic archive: %w", err)
	}
	if err := target.Sync(); err != nil {
		return fmt.Errorf("sync diagnostic archive: %w", err)
	}
	if err := target.Close(); err != nil {
		return fmt.Errorf("close diagnostic archive: %w", err)
	}
	published = true
	return nil
}
