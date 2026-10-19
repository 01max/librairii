package main

import (
	"errors"
	"net/http"
)

var errLibraryBusy = errors.New("settings.busy")

func (a *App) beginLibraryMove() error {
	if !a.libraryGate.TryLock() {
		return errLibraryBusy
	}
	active := a.core.ActiveOperations(a.runtimeContext())
	if active.Error != nil || len(active.Operations) > 0 {
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
