package main

import (
	"context"
	"errors"
	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/artwork"
	"github.com/01max/librairii/internal/exporter"
	"github.com/01max/librairii/internal/inspection/testfixture"
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/lunii"
	"github.com/01max/librairii/internal/operations"
	"github.com/01max/librairii/internal/platform"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"time"
)

type moveDialogs struct{ file, destination string }

func (d *moveDialogs) OpenFiles(context.Context, coreapp.FileDialogRequest) ([]string, error) {
	return []string{d.file}, nil
}
func (d *moveDialogs) OpenDirectory(context.Context, string) (string, error) {
	return d.destination, nil
}
func (d *moveDialogs) RevealDirectory(context.Context, string) error { return nil }

type offlineCatalog struct{}

func (offlineCatalog) FetchCatalog(context.Context) ([]byte, error) {
	return nil, errors.New("offline")
}
func (offlineCatalog) FetchArtwork(context.Context, string, int64) (lunii.ArtworkPayload, error) {
	return lunii.ArtworkPayload{}, errors.New("offline")
}
func waitOperation(t *testing.T, a *App, id string) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		response := a.OperationSnapshot(id)
		if response.Operation != nil {
			switch response.Operation.Status {
			case operations.StatusSucceeded:
				return
			case operations.StatusFailed, operations.StatusCancelled, operations.StatusInterrupted:
				t.Fatal(response.Operation)
			}
		}
		time.Sleep(5 * time.Millisecond)
	}
	t.Fatal("operation did not finish")
}
func TestApplicationUsesNewRootAfterMove(t *testing.T) {
	ctx := context.Background()
	storage := platform.NewConfiguredStorageReadiness(t.TempDir(), "/unused", "fr")
	source := filepath.Join(t.TempDir(), "old")
	if err := storage.InitializeLibrary(ctx, source); err != nil {
		t.Fatal(err)
	}
	archive, err := testfixture.WriteZIP(t.TempDir(), testfixture.PlainPK())
	if err != nil {
		t.Fatal(err)
	}
	dialogs := &moveDialogs{file: archive, destination: t.TempDir()}
	composition, err := coreapp.NewComposition(coreapp.CompositionOptions{Storage: storage, Clock: facadeClock{}, Dialogs: dialogs, Events: facadeEvents{}, MetadataFetcher: offlineCatalog{}, Workers: 1})
	if err != nil {
		t.Fatal(err)
	}
	a := NewApp(composition.Application, WithSettings(storage, dialogs))
	a.startup(ctx)
	a.settings.pendingOpen = true
	if !a.Settings().PendingOpen || !a.Settings().PendingOpen {
		t.Fatal("early settings request lost before acknowledgement")
	}
	a.DismissSettings()
	if a.Settings().PendingOpen {
		t.Fatal("settings dismissal not acknowledged")
	}
	defer a.shutdown(ctx)
	imported := a.SelectAndImportStories()
	if imported.Operation == nil {
		t.Fatal(imported.Error)
	}
	waitOperation(t, a, imported.Operation.ID)
	request := exporter.PreflightRequest{SourceType: operations.ExportSourceCurrentQuery, Query: library.StoryLibraryQuery{}}
	prepared := a.SelectAndPreflightExport(request)
	if prepared.Preflight == nil || !prepared.Preflight.CanExport {
		t.Fatal(prepared)
	}
	target := filepath.Join(t.TempDir(), "nouvelle bibliothèque")
	dialogs.destination = target
	selection := a.SelectLibraryLocation()
	if selection.Preparation == nil {
		t.Fatal(selection)
	}
	started := a.StartLibraryMove(selection.Preparation.ID)
	if started.Error != "" {
		t.Fatal(started)
	}
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		snapshot := a.Settings()
		if snapshot.Move != nil && (snapshot.Move.Phase == "succeeded" || snapshot.Move.Phase == "failed" || snapshot.Move.Phase == "cleanup_pending") {
			if snapshot.Move.Phase != "succeeded" {
				t.Fatalf("%+v", snapshot.Move)
			}
			break
		}
		time.Sleep(5 * time.Millisecond)
	}
	if a.Settings().Move.Phase != "succeeded" {
		t.Fatalf("%+v", a.Settings().Move)
	}
	// The worker releases exclusive access just after publishing its terminal state.
	a.libraryGate.RLock()
	a.libraryGate.RUnlock()
	if !a.ApplicationStatus().Status.MutationsAllowed {
		t.Fatal(a.ApplicationStatus())
	}
	if page := a.QueryStories(library.StoryLibraryQuery{}); page.Page == nil || page.Page.TotalItems != 1 {
		t.Fatal(page)
	}
	if stale := a.StartPreparedExport(prepared.Preflight.PreparationID); stale.Error == nil {
		t.Fatal("stale preparation executed")
	}
	dialogs.destination = t.TempDir()
	next := a.SelectAndPreflightExport(request)
	if next.Preflight == nil || !next.Preflight.CanExport {
		t.Fatal(next)
	}
	exported := a.StartPreparedExport(next.Preflight.PreparationID)
	if exported.Operation == nil {
		t.Fatal(exported.Error)
	}
	waitOperation(t, a, exported.Operation.ID)
	if _, err := os.Stat(source); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("old root still exists", err)
	}
	if _, err := os.Stat(archive); err != nil {
		t.Fatal("source archive moved", err)
	}
	page := a.QueryStories(library.StoryLibraryQuery{})
	story := page.Page.Stories[0]
	artworkHandler, err := artwork.NewAssetHandler(storage, offlineCatalog{})
	if err != nil {
		t.Fatal(err)
	}
	recorder := httptest.NewRecorder()
	a.gatedArtwork(artworkHandler).ServeHTTP(recorder, httptest.NewRequest("GET", "/artwork/"+story.ArtworkID, nil))
	if recorder.Code != 200 {
		t.Fatal("artwork after relocation", recorder.Code)
	}
	if removed := a.RemoveStory(story.ID); removed.Error != nil {
		t.Fatal(removed.Error)
	}
	if page := a.QueryStories(library.StoryLibraryQuery{}); page.Page.TotalItems != 0 {
		t.Fatal(page)
	}
	if files, err := os.ReadDir(storage.Layout().Logs); err != nil || len(files) == 0 {
		t.Fatal("logging unavailable", err)
	}
	dialogs.destination = t.TempDir()
	if diagnostics := a.SelectAndExportDiagnostics(); diagnostics.Error != nil {
		t.Fatal(diagnostics.Error)
	}
}
