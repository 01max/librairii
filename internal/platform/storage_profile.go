package platform

import (
	"context"
	"errors"
	"path/filepath"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/preferences"
)

// NewConfiguredStorageReadiness loads the independent profile before storage
// creation. The original constructor remains the explicit isolated-host path.
func NewConfiguredStorageReadiness(directory, legacy, language string) *StorageReadiness {
	return &StorageReadiness{profileStore: preferences.NewStore(directory), legacyRoot: legacy, profile: preferences.Profile{Version: preferences.Version, Language: language}}
}
func (r *StorageReadiness) prepareProfile(ctx context.Context) error {
	if r.profileStore == nil {
		return nil
	}
	p, err := preferences.Bootstrap(ctx, r.profileStore, r.legacyRoot, r.profile.Language)
	r.profile = p
	if errors.Is(err, database.ErrForeignSchema) {
		r.override = r.legacyRoot
		return nil
	}
	if err != nil {
		return err
	}
	if !p.Complete {
		return preferences.ErrSetupRequired
	}
	if err = preferences.ValidateLibrary(ctx, p.Root, p.LibraryID); err != nil {
		return err
	}
	r.override = filepath.Clean(p.Root)
	return nil
}
func (r *StorageReadiness) Profile() preferences.Profile {
	r.mu.RLock()
	defer r.mu.RUnlock()
	p := r.profile
	if r.profileStore == nil {
		p = preferences.Profile{Version: preferences.Version, Language: "en", Root: r.override, Complete: true}
	}
	return p
}
