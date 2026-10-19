package main

import (
	"context"
	"github.com/01max/librairii/internal/preferences"
	"github.com/google/uuid"
	"log"
	"sync"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/platform"
)

type MovePreparation struct {
	ID          string `json:"id"`
	Source      string `json:"source"`
	Destination string `json:"destination"`
	Bytes       int64  `json:"bytes"`
	LibraryID   string `json:"libraryId"`
}

type MoveProgress struct {
	Phase       string `json:"phase"`
	Copied      int64  `json:"copied"`
	Total       int64  `json:"total"`
	Error       string `json:"error,omitempty"`
	CleanupPath string `json:"cleanupPath,omitempty"`
	Cancelable  bool   `json:"cancelable"`
}
type SettingsSnapshot struct {
	Recovery      bool             `json:"recovery"`
	Move          *MoveProgress    `json:"move,omitempty"`
	Preparation   *MovePreparation `json:"preparation,omitempty"`
	Language      string           `json:"language"`
	Root          string           `json:"root"`
	SetupRequired bool             `json:"setupRequired"`
	Isolated      bool             `json:"isolated"`
	PendingOpen   bool             `json:"pendingOpen"`
	Error         string           `json:"error,omitempty"`
}
type settingsController struct {
	mu          sync.Mutex
	storage     *platform.StorageReadiness
	dialogs     coreapp.DialogPort
	selected    string
	move        *MoveProgress
	cancelMove  context.CancelFunc
	preparation *MovePreparation
	pendingOpen bool
	changed     func(context.Context)
}

func WithSettings(storage *platform.StorageReadiness, dialogs coreapp.DialogPort) AppOption {
	return func(a *App) { a.settings = &settingsController{storage: storage, dialogs: dialogs} }
}
func (a *App) Settings() SettingsSnapshot {
	if a.settings == nil {
		return SettingsSnapshot{Language: "en", Isolated: true}
	}
	s := a.settings
	s.mu.Lock()
	defer s.mu.Unlock()
	p := s.storage.Profile()
	root := s.storage.SuggestedRoot()
	if s.selected != "" && !p.Complete {
		root = s.selected
	}
	var move *MoveProgress
	if s.move != nil {
		copy := *s.move
		move = &copy
	}
	if move == nil && s.storage.ConfigurationDirectory() != "" {
		if journal, err := preferences.ReadJournal(s.storage.ConfigurationDirectory()); err == nil && p.Root == journal.Destination {
			move = &MoveProgress{Phase: "cleanup_pending", CleanupPath: journal.Source, Error: "settings.cleanup_pending"}
		}
	}
	pending := s.pendingOpen
	s.pendingOpen = false
	return SettingsSnapshot{Language: p.Language, Root: root, SetupRequired: a.core.Status().State == coreapp.StateSetupRequired, Recovery: a.core.Status().State == coreapp.StateRecovery, Isolated: s.storage.Isolated(), PendingOpen: pending, Preparation: s.preparation, Move: move}
}
func (a *App) SetUILanguage(language string) SettingsSnapshot {
	if a.settings == nil {
		return SettingsSnapshot{Language: "en", Isolated: true, Error: "settings.invalid"}
	}
	err := a.settings.storage.SetLanguage(language)
	result := a.Settings()
	if err != nil {
		result.Error = "settings.invalid"
	} else {
		a.settings.mu.Lock()
		changed := a.settings.changed
		a.settings.mu.Unlock()
		if changed != nil {
			changed(a.runtimeContext())
		}
	}
	return result
}
func (a *App) SelectLibraryLocation() SettingsSnapshot {
	if a.settings == nil {
		return a.Settings()
	}
	if a.settings.storage.Isolated() {
		result := a.Settings()
		result.Error = "settings.override"
		return result
	}
	if !a.libraryGate.TryRLock() {
		result := a.Settings()
		result.Error = "settings.busy"
		return result
	}
	defer a.libraryGate.RUnlock()
	current := a.settings.storage.Profile()
	if current.Complete {
		active := a.core.ActiveOperations(a.runtimeContext())
		if active.Error == nil && hasRunningOperations(active.Operations) {
			result := a.Settings()
			result.Error = "settings.busy"
			return result
		}
	}
	path, err := a.settings.dialogs.OpenDirectory(a.runtimeContext(), "Choose library folder")
	if err == nil && path != "" {
		var prepared *MovePreparation
		if current.Complete {
			source, _ := preferences.Canonical(current.Root)
			target, _ := preferences.Canonical(path)
			if source == target {
				a.settings.mu.Lock()
				a.settings.preparation = nil
				a.settings.selected = ""
				a.settings.mu.Unlock()
				return a.Settings()
			}
			err = preferences.EmptyTarget(path, a.settings.storage.ConfigurationDirectory(), current.Root)
			if err == nil {
				var size int64
				size, err = preferences.TreeSize(current.Root)
				prepared = &MovePreparation{ID: uuid.NewString(), Source: source, Destination: target, Bytes: size, LibraryID: current.LibraryID}
			}
		} else {
			err = preferences.EmptyTarget(path, a.settings.storage.ConfigurationDirectory(), "")
		}
		if err == nil {
			a.settings.mu.Lock()
			a.settings.selected = path
			a.settings.preparation = prepared
			a.settings.mu.Unlock()
		}
	}
	result := a.Settings()
	if err != nil {
		if a.packagedAcceptance != nil {
			log.Printf("settings acceptance folder selection: %v", err)
		}
		result.Error = "settings.destination_invalid"
	}
	return result
}
func (a *App) ConfirmLibrarySetup() coreapp.MutationResponse {
	if !a.libraryGate.TryLock() {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorNotReady, "settings.busy")}
	}
	defer a.libraryGate.Unlock()
	if a.settings == nil {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorInvalidInput, "settings.invalid")}
	}
	snapshot := a.Settings()
	if err := a.settings.storage.InitializeLibrary(a.runtimeContext(), snapshot.Root); err != nil {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorInvalidInput, err.Error())}
	}
	_ = a.core.ResumeStorage(a.runtimeContext())
	return coreapp.MutationResponse{Success: a.core.Status().MutationsAllowed}
}

func (a *App) RetryLibrary() SettingsSnapshot {
	if a.settings == nil {
		return a.Settings()
	}
	if !a.libraryGate.TryLock() {
		result := a.Settings()
		result.Error = "settings.busy"
		return result
	}
	defer a.libraryGate.Unlock()
	_ = a.core.ResumeStorage(a.runtimeContext())
	result := a.Settings()
	if !a.core.Status().MutationsAllowed {
		result.Error = "settings.library_missing"
	}
	return result
}
func (a *App) LocateLibrary() SettingsSnapshot {
	if a.settings == nil {
		return a.Settings()
	}
	if !a.libraryGate.TryLock() {
		result := a.Settings()
		result.Error = "settings.busy"
		return result
	}
	defer a.libraryGate.Unlock()
	path, err := a.settings.dialogs.OpenDirectory(a.runtimeContext(), "Locate library…")
	if err == nil && path != "" {
		err = a.settings.storage.LocateLibrary(a.runtimeContext(), path)
		if err == nil {
			_ = a.core.ResumeStorage(a.runtimeContext())
		}
	}
	result := a.Settings()
	if err != nil {
		result.Error = "settings.identity_mismatch"
	}
	return result
}
