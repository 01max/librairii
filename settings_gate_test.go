package main

import (
	"github.com/01max/librairii/internal/library"
	"github.com/01max/librairii/internal/operations"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestRelocationExcludesBindingsAndArtwork(t *testing.T) {
	a := &App{}
	a.libraryGate.Lock()
	if response := a.QueryStories(library.StoryLibraryQuery{}); response.Error == nil {
		t.Fatal("query entered during move")
	}
	response := httptest.NewRecorder()
	a.gatedArtwork(http.HandlerFunc(func(http.ResponseWriter, *http.Request) { t.Error("artwork entered") })).ServeHTTP(response, httptest.NewRequest("GET", "/artwork/test", nil))
	if response.Code != 503 {
		t.Fatal(response.Code)
	}
	a.libraryGate.Unlock()
}
func TestInflightArtworkBlocksMove(t *testing.T) {
	a := &App{}
	entered := make(chan struct{})
	release := make(chan struct{})
	done := make(chan struct{})
	handler := a.gatedArtwork(http.HandlerFunc(func(http.ResponseWriter, *http.Request) { close(entered); <-release }))
	go func() {
		handler.ServeHTTP(httptest.NewRecorder(), httptest.NewRequest("GET", "/artwork/test", nil))
		close(done)
	}()
	<-entered
	if err := a.beginLibraryMove(); err != errLibraryBusy {
		t.Fatal(err)
	}
	close(release)
	<-done
}

func TestCompletedReportsDoNotBlockRelocation(t *testing.T) {
	if hasRunningOperations([]operations.Snapshot{{Status: operations.StatusSucceeded}, {Status: operations.StatusFailed}}) {
		t.Fatal("terminal reports treated as active work")
	}
	if !hasRunningOperations([]operations.Snapshot{{Status: operations.StatusRunning}}) {
		t.Fatal("running work was ignored")
	}
}
