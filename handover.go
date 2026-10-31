package main

import (
	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/handover"
	"github.com/01max/librairii/internal/preferences"
)

// ExportNativePreferences receives only whitelisted semantic preferences. The
// backend supplies the actual library identity, then opens the native picker.
func (a *App) ExportNativePreferences(input string) coreapp.MutationResponse {
	refuse := func(message string) coreapp.MutationResponse {
		return coreapp.MutationResponse{Error: coreapp.NewAPIError(coreapp.ErrorInvalidInput, message)}
	}
	if !a.libraryGate.TryRLock() {
		return refuse("settings.busy")
	}
	defer a.libraryGate.RUnlock()
	if a.settings == nil || !a.core.Status().MutationsAllowed {
		return refuse("Library unavailable")
	}
	state, err := handover.DecodeState(input)
	if err != nil {
		return refuse(handover.ErrInvalid.Error())
	}
	profile := a.settings.storage.Profile()
	id, err := preferences.Identity(profile.Root)
	if err != nil {
		return refuse("Library unavailable")
	}
	compatibility := a.core.LibraryCompatibility(a.runtimeContext())
	if compatibility.Error != nil {
		return refuse(handover.ErrInvalid.Error())
	}
	body, err := handover.Encode(id, compatibility.Compatibility.LegacyBrokenTagID, state)
	if err != nil {
		return refuse(handover.ErrInvalid.Error())
	}
	directory, err := a.settings.dialogs.OpenDirectory(a.runtimeContext(), "Export preferences for the native application")
	if err != nil {
		return refuse("Preference transfer could not be saved.")
	}
	if directory == "" {
		return coreapp.MutationResponse{}
	}
	if err := handover.Export(directory, body); err != nil {
		return refuse("Preference transfer could not be saved. Choose a folder without an existing transfer file.")
	}
	return coreapp.MutationResponse{Success: true}
}
