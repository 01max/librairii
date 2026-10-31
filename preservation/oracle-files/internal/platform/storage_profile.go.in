package platform

import (
	"context"
	"errors"
	"os"
	"path/filepath"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/preferences"
	"github.com/01max/librairii/internal/storage"
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
	if p.Recovering {
		id, err := preferences.Identity(p.Root)
		if err != nil || id != p.LibraryID {
			return preferences.ErrMissingLibrary
		}
		r.override = p.Root
		identity, err := (database.SchemaProbe{}).Inspect(ctx, preferences.DatabasePath(p.Root))
		if err != nil {
			return err
		}
		// A crash before preservation must still require the recovery action.
		if identity == database.IdentityForeign {
			return nil
		}
		db, err := database.Open(ctx, preferences.DatabasePath(p.Root))
		if err != nil {
			return err
		}
		if err := db.Close(); err != nil {
			return err
		}
		p, err = r.profileStore.Update(func(next *preferences.Profile) error {
			next.Complete, next.Recovering = true, false
			return nil
		})
		if err != nil {
			return err
		}
		r.profile = p
	}
	if !p.Complete {
		return preferences.ErrSetupRequired
	}
	if recoveryErr := preferences.Reconcile(ctx, r.profileStore.Directory(), p); recoveryErr != nil && !errors.Is(recoveryErr, preferences.ErrCleanupPending) {
		return recoveryErr
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
		language := r.profile.Language
		if language == "" {
			language = "en"
		}
		p = preferences.Profile{Version: preferences.Version, Language: language, Root: r.override, Complete: true}
	}
	return p
}

func (r *StorageReadiness) Isolated() bool { return r.profileStore == nil }

func (r *StorageReadiness) SuggestedRoot() string {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if r.profile.Root != "" {
		return r.profile.Root
	}
	return r.legacyRoot
}
func (r *StorageReadiness) SetLanguage(language string) error {
	if language != "en" && language != "fr" {
		return preferences.ErrInvalid
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.profileStore == nil {
		r.profile.Language = language
		return nil
	}
	p, err := r.profileStore.Update(func(p *preferences.Profile) error { p.Language = language; return nil })
	if err == nil {
		r.profile = p
	}
	return err
}
func (r *StorageReadiness) InitializeLibrary(ctx context.Context, root string) error {
	r.bootstrapMu.Lock()
	defer r.bootstrapMu.Unlock()
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.profileStore == nil {
		return preferences.ErrInvalid
	}
	if r.profile.Complete {
		return preferences.ErrInvalid
	}
	// Retry only a confirmed setup folder with the recorded identity.
	if r.profile.Root != root || r.profile.LibraryID == "" {
		if err := preferences.EmptyTarget(root, r.profileStore.Directory(), ""); err != nil {
			return err
		}
	}
	if r.profile.Root == root && r.profile.LibraryID != "" {
		id, err := preferences.Identity(root)
		if err != nil || id != r.profile.LibraryID {
			return preferences.ErrInvalid
		}
	}
	layout, err := storage.Initialize(root)
	if err != nil {
		return err
	}
	id, err := preferences.EnsureIdentity(root)
	if err != nil {
		return err
	}
	r.profile.Root = root
	r.profile.LibraryID = id
	// Persist ownership of an incomplete setup so initialization can be retried
	// in the same folder, including after an application restart.
	pending, err := r.profileStore.Update(func(p *preferences.Profile) error {
		p.Root = root
		p.LibraryID = id
		p.Complete = false
		p.Language = r.profile.Language
		return nil
	})
	if err != nil {
		return err
	}
	r.profile = pending
	db, err := database.Open(ctx, preferences.DatabasePath(root))
	if err != nil {
		return err
	}
	if err = db.Close(); err != nil {
		return err
	}
	p, err := r.profileStore.Update(func(p *preferences.Profile) error {
		p.Root = root
		p.LibraryID = id
		p.Complete = true
		p.Language = r.profile.Language
		return nil
	})
	if err != nil {
		return err
	}
	r.profile = p
	r.layout = layout
	r.override = root
	return nil
}

func (r *StorageReadiness) CommitRoot(root string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.profileStore == nil {
		return preferences.ErrInvalid
	}
	p, err := r.profileStore.Update(func(p *preferences.Profile) error { p.Root = root; return nil })
	if err != nil {
		return err
	}
	r.profile = p
	r.override = root
	return nil
}
func (r *StorageReadiness) ConfigurationDirectory() string {
	if r.profileStore == nil {
		return ""
	}
	return r.profileStore.Directory()
}

func (r *StorageReadiness) LocateLibrary(ctx context.Context, root string) error {
	p := r.Profile()
	if !p.Complete || r.Isolated() {
		return preferences.ErrInvalid
	}
	if err := preferences.ValidateLibrary(ctx, root, p.LibraryID); err != nil {
		return err
	}
	if _, err := preferences.ReadJournal(r.ConfigurationDirectory()); !errors.Is(err, os.ErrNotExist) {
		return preferences.ErrInvalid
	}
	return r.CommitRoot(root)
}
