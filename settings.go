package main

import (
	"context"
	"sync"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/platform"
)

type SettingsSnapshot struct {
	Language      string `json:"language"`
	Root          string `json:"root"`
	SetupRequired bool   `json:"setupRequired"`
	Isolated      bool   `json:"isolated"`
	PendingOpen   bool   `json:"pendingOpen"`
	Error         string `json:"error,omitempty"`
}
type settingsController struct {
	mu          sync.Mutex
	storage     *platform.StorageReadiness
	dialogs     coreapp.DialogPort
	selected    string
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
	if s.selected != "" {
		root = s.selected
	}
	return SettingsSnapshot{Language: p.Language, Root: root, SetupRequired: !p.Complete, Isolated: s.storage.Isolated(), PendingOpen: s.pendingOpen}
}
func (a *App) SetUILanguage(language string) SettingsSnapshot {
	if a.settings == nil {
		return SettingsSnapshot{Language: "en", Isolated: true, Error: "settings.invalid"}
	}
	err := a.settings.storage.SetLanguage(language)
	result := a.Settings()
	if err != nil {
		result.Error = "settings.invalid"
	} else if a.settings.changed != nil {
		a.settings.changed(a.runtimeContext())
	}
	return result
}
func (a *App) SelectLibraryLocation() SettingsSnapshot {
	if a.settings == nil {
		return a.Settings()
	}
	path, err := a.settings.dialogs.OpenDirectory(a.runtimeContext(), "Choose library folder")
	if err == nil && path != "" {
		a.settings.mu.Lock()
		a.settings.selected = path
		a.settings.mu.Unlock()
	}
	result := a.Settings()
	if err != nil {
		result.Error = "settings.destination_invalid"
	}
	return result
}
func (a *App) ConfirmLibrarySetup() coreapp.MutationResponse {
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
