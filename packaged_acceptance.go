package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/exporter"
)

const (
	acceptanceCheckpointStarted           = "scenario_started"
	acceptanceCheckpointNativeImport      = "native_import_dialog_selected"
	acceptanceCheckpointImportQueued      = "import_queued"
	acceptanceCheckpointImportSucceeded   = "import_succeeded"
	acceptanceCheckpointCollectionLoaded  = "collection_loaded"
	acceptanceCheckpointNativeDestination = "native_destination_dialog_selected"
	acceptanceCheckpointExportPrepared    = "export_prepared"
	acceptanceCheckpointExportQueued      = "export_queued"
	acceptanceCheckpointExportSucceeded   = "export_succeeded"
	acceptanceCheckpointNativeReveal      = "native_reveal_succeeded"
	acceptanceCheckpointRevealSucceeded   = "reveal_succeeded"
	acceptanceCheckpointComplete          = "complete"
	acceptanceCheckpointFailed            = "failed"
)

var acceptanceCheckpoints = map[string]struct{}{
	acceptanceCheckpointStarted:           {},
	acceptanceCheckpointNativeImport:      {},
	acceptanceCheckpointImportQueued:      {},
	acceptanceCheckpointImportSucceeded:   {},
	acceptanceCheckpointCollectionLoaded:  {},
	acceptanceCheckpointNativeDestination: {},
	acceptanceCheckpointExportPrepared:    {},
	acceptanceCheckpointExportQueued:      {},
	acceptanceCheckpointExportSucceeded:   {},
	acceptanceCheckpointNativeReveal:      {},
	acceptanceCheckpointRevealSucceeded:   {},
	acceptanceCheckpointComplete:          {},
	acceptanceCheckpointFailed:            {},
}

type packagedAcceptance struct {
	enabled     bool
	source      string
	destination string
	checkpoints string
	native      coreapp.DialogPort
	automate    nativeDialogAutomation
	mu          sync.Mutex
}

type nativeDialogKind uint8

const (
	nativeFileDialog nativeDialogKind = iota + 1
	nativeDirectoryDialog
)

type nativeDialogAutomation func(
	context.Context,
	string,
	string,
	nativeDialogKind,
) error

func newPackagedAcceptanceFromEnvironment(
	native coreapp.DialogPort,
) (*packagedAcceptance, error) {
	if os.Getenv("LIBRAIRII_PACKAGED_ACCEPTANCE") == "" {
		return &packagedAcceptance{native: native}, nil
	}
	if os.Getenv("LIBRAIRII_PACKAGED_ACCEPTANCE") != "1" {
		return nil, errors.New("packaged acceptance flag must be 1")
	}
	acceptance := &packagedAcceptance{
		enabled:     true,
		source:      filepath.Clean(os.Getenv("LIBRAIRII_ACCEPTANCE_SOURCE")),
		destination: filepath.Clean(os.Getenv("LIBRAIRII_ACCEPTANCE_DESTINATION")),
		checkpoints: filepath.Clean(os.Getenv("LIBRAIRII_ACCEPTANCE_CHECKPOINTS")),
		native:      native,
		automate:    automateHostNativeDialog,
	}
	if native == nil {
		return nil, errors.New("packaged acceptance requires native dialogs")
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
	ctx context.Context,
	request coreapp.FileDialogRequest,
) ([]string, error) {
	if !a.enabled ||
		!request.Multiple ||
		!containsString(request.Extensions, filepath.Ext(a.source)) {
		return nil, errors.New("packaged acceptance import dialog contract mismatch")
	}
	automation := a.startDialogAutomation(
		ctx,
		request.Title,
		a.source,
		nativeFileDialog,
	)
	paths, dialogErr := a.native.OpenFiles(ctx, request)
	automationErr := <-automation
	if err := errors.Join(dialogErr, automationErr); err != nil {
		return nil, err
	}
	if len(paths) != 1 || !sameResolvedPath(paths[0], a.source) {
		return nil, errors.New(
			"packaged acceptance native import selection mismatch",
		)
	}
	return paths, a.record(acceptanceCheckpointNativeImport)
}

func (a *packagedAcceptance) OpenDirectory(
	ctx context.Context,
	title string,
) (string, error) {
	if !a.enabled || title != "Export story archives" {
		return "", errors.New("packaged acceptance directory dialog contract mismatch")
	}
	automation := a.startDialogAutomation(
		ctx,
		title,
		a.destination,
		nativeDirectoryDialog,
	)
	destination, dialogErr := a.native.OpenDirectory(ctx, title)
	automationErr := <-automation
	if err := errors.Join(dialogErr, automationErr); err != nil {
		return "", err
	}
	expected, err := exporter.ResolveDestination(a.destination)
	if err != nil {
		return "", err
	}
	if destination != expected {
		return "", errors.New(
			"packaged acceptance native destination selection mismatch",
		)
	}
	return destination, a.record(acceptanceCheckpointNativeDestination)
}

func (a *packagedAcceptance) RevealDirectory(
	ctx context.Context,
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
	if err := a.native.RevealDirectory(ctx, destination); err != nil {
		return err
	}
	return a.record(acceptanceCheckpointNativeReveal)
}

func (a *packagedAcceptance) startDialogAutomation(
	ctx context.Context,
	title string,
	path string,
	kind nativeDialogKind,
) <-chan error {
	result := make(chan error, 1)
	go func() {
		automationContext, cancel := context.WithTimeout(ctx, 15*time.Second)
		defer cancel()
		result <- a.automate(automationContext, title, path, kind)
	}()
	return result
}

func sameResolvedPath(actual string, expected string) bool {
	actualPath, actualErr := filepath.EvalSymlinks(actual)
	expectedPath, expectedErr := filepath.EvalSymlinks(expected)
	return actualErr == nil &&
		expectedErr == nil &&
		filepath.Clean(actualPath) == filepath.Clean(expectedPath)
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
