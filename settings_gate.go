package main

import (
	"errors"
	"github.com/01max/librairii/internal/operations"
	"net/http"
)

var errLibraryBusy = errors.New("settings.busy")

func (a *App) beginLibraryMove() error {
	if !a.libraryGate.TryLock() {
		return errLibraryBusy
	}
	active := a.core.ActiveOperations(a.runtimeContext())
	if active.Error != nil || hasRunningOperations(active.Operations) {
		a.libraryGate.Unlock()
		return errLibraryBusy
	}
	return nil
}
func (a *App) gatedArtwork(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !a.libraryGate.TryRLock() {
			http.Error(w, "library relocating", http.StatusServiceUnavailable)
			return
		}
		defer a.libraryGate.RUnlock()
		next.ServeHTTP(w, r)
	})
}

func hasRunningOperations(items []operations.Snapshot) bool {
	for _, item := range items {
		if item.Status == operations.StatusRunning || item.Status == operations.StatusQueued {
			return true
		}
	}
	return false
}
