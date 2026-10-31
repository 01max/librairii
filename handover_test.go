package main

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/handover"
	"github.com/01max/librairii/internal/platform"
)

func TestPreferenceExportUsesActualLibraryAndRetainsExistingFiles(t *testing.T) {
	ctx := context.Background()
	readiness := platform.NewConfiguredStorageReadiness(t.TempDir(), "/unused", "fr")
	root := filepath.Join(t.TempDir(), "bibliothèque")
	if err := readiness.InitializeLibrary(ctx, root); err != nil {
		t.Fatal(err)
	}
	dialogs := &moveDialogs{destination: t.TempDir()}
	composition, err := coreapp.NewComposition(coreapp.CompositionOptions{Storage: readiness, Clock: facadeClock{}, Dialogs: dialogs, Events: facadeEvents{}, MetadataFetcher: offlineCatalog{}, Workers: 1})
	if err != nil {
		t.Fatal(err)
	}
	app := NewApp(composition.Application, WithSettings(readiness, dialogs))
	app.startup(ctx)
	defer app.shutdown(ctx)
	input := `{"presentation":"large_covers","dismissedOperations":[],"syncPanelHidden":true,"frames":[{"hash":"#/library?v=3&page=3","kind":"library"}],"position":0}`
	if response := app.ExportNativePreferences(input); !response.Success {
		t.Fatal(response)
	}
	body, err := os.ReadFile(filepath.Join(dialogs.destination, handover.Filename))
	if err != nil {
		t.Fatal(err)
	}
	var value handover.Document
	if err := json.Unmarshal(body, &value); err != nil {
		t.Fatal(err)
	}
	if value.LibraryID != readiness.Profile().LibraryID || strings.Contains(string(body), root) {
		t.Fatalf("wrong transfer binding or private path: %s", body)
	}
	if response := app.ExportNativePreferences(input); response.Error == nil {
		t.Fatal("overwrote prior transfer")
	}
	after, err := os.ReadFile(filepath.Join(dialogs.destination, handover.Filename))
	if err != nil || string(after) != string(body) {
		t.Fatal("prior transfer changed", err)
	}
	dialogs.destination = ""
	if response := app.ExportNativePreferences(input); response.Success || response.Error != nil {
		t.Fatal("picker cancellation was not preserved", response)
	}
}
