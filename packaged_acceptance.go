package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/exporter"
)

const (
	acceptanceCheckpointStarted          = "scenario_started"
	acceptanceCheckpointImportQueued     = "import_queued"
	acceptanceCheckpointImportSucceeded  = "import_succeeded"
	acceptanceCheckpointCollectionLoaded = "collection_loaded"
	acceptanceCheckpointExportPrepared   = "export_prepared"
	acceptanceCheckpointExportQueued     = "export_queued"
	acceptanceCheckpointExportSucceeded  = "export_succeeded"
	acceptanceCheckpointRevealSucceeded  = "reveal_succeeded"
	acceptanceCheckpointComplete         = "complete"
	acceptanceCheckpointFailed           = "failed"
)

var acceptanceCheckpoints = map[string]struct{}{
	acceptanceCheckpointStarted:          {},
	acceptanceCheckpointImportQueued:     {},
	acceptanceCheckpointImportSucceeded:  {},
	acceptanceCheckpointCollectionLoaded: {},
	acceptanceCheckpointExportPrepared:   {},
	acceptanceCheckpointExportQueued:     {},
	acceptanceCheckpointExportSucceeded:  {},
	acceptanceCheckpointRevealSucceeded:  {},
	acceptanceCheckpointComplete:         {},
	acceptanceCheckpointFailed:           {},
}

type packagedAcceptance struct {
	enabled     bool
	source      string
	destination string
	checkpoints string
	mu          sync.Mutex
}

func newPackagedAcceptanceFromEnvironment() (*packagedAcceptance, error) {
	if os.Getenv("LIBRAIRII_PACKAGED_ACCEPTANCE") == "" {
		return &packagedAcceptance{}, nil
	}
	if os.Getenv("LIBRAIRII_PACKAGED_ACCEPTANCE") != "1" {
		return nil, errors.New("packaged acceptance flag must be 1")
	}
	acceptance := &packagedAcceptance{
		enabled:     true,
		source:      filepath.Clean(os.Getenv("LIBRAIRII_ACCEPTANCE_SOURCE")),
		destination: filepath.Clean(os.Getenv("LIBRAIRII_ACCEPTANCE_DESTINATION")),
		checkpoints: filepath.Clean(os.Getenv("LIBRAIRII_ACCEPTANCE_CHECKPOINTS")),
	}
	for label, path := range map[string]string{
		"source":      acceptance.source,
		"destination": acceptance.destination,
		"checkpoints": acceptance.checkpoints,
	} {
		if path == "." || !filepath.IsAbs(path) {
			return nil, fmt.Errorf("packaged acceptance %s path must be absolute", label)
		}
	}
	source, err := os.Stat(acceptance.source)
	if err != nil || !source.Mode().IsRegular() {
		return nil, errors.New("packaged acceptance source must be a regular file")
	}
	destination, err := os.Stat(acceptance.destination)
	if err != nil || !destination.IsDir() {
		return nil, errors.New("packaged acceptance destination must be a directory")
	}
	checkpointParent, err := os.Stat(filepath.Dir(acceptance.checkpoints))
	if err != nil || !checkpointParent.IsDir() {
		return nil, errors.New("packaged acceptance checkpoint parent must be a directory")
	}
	return acceptance, nil
}

func (a *packagedAcceptance) OpenFiles(
	_ context.Context,
	request coreapp.FileDialogRequest,
) ([]string, error) {
	if !a.enabled ||
		!request.Multiple ||
		!containsString(request.Extensions, filepath.Ext(a.source)) {
		return nil, errors.New("packaged acceptance import dialog contract mismatch")
	}
	return []string{a.source}, nil
}

func (a *packagedAcceptance) OpenDirectory(
	_ context.Context,
	title string,
) (string, error) {
	if !a.enabled || title != "Export story archives" {
		return "", errors.New("packaged acceptance directory dialog contract mismatch")
	}
	return exporter.ResolveDestination(a.destination)
}

func (a *packagedAcceptance) RevealDirectory(
	_ context.Context,
	destination string,
) error {
	resolved, err := exporter.ResolveDestination(destination)
	if err != nil {
		return err
	}
	expected, err := exporter.ResolveDestination(a.destination)
	if err != nil {
		return err
	}
	if resolved != expected {
		return errors.New("packaged acceptance reveal destination mismatch")
	}
	return nil
}

func (a *packagedAcceptance) record(checkpoint string) error {
	if !a.enabled {
		return errors.New("packaged acceptance is disabled")
	}
	if _, allowed := acceptanceCheckpoints[checkpoint]; !allowed {
		return errors.New("packaged acceptance checkpoint is invalid")
	}
	a.mu.Lock()
	defer a.mu.Unlock()

	file, err := os.OpenFile(
		a.checkpoints,
		os.O_CREATE|os.O_APPEND|os.O_WRONLY,
		0o600,
	)
	if err != nil {
		return fmt.Errorf("open packaged acceptance checkpoints: %w", err)
	}
	if _, err := file.WriteString(checkpoint + "\n"); err != nil {
		_ = file.Close()
		return fmt.Errorf("write packaged acceptance checkpoint: %w", err)
	}
	syncErr := file.Sync()
	closeErr := file.Close()
	return errors.Join(syncErr, closeErr)
}

func containsString(values []string, expected string) bool {
	for _, value := range values {
		if strings.EqualFold(value, expected) {
			return true
		}
	}
	return false
}
