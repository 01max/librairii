package main

import (
	"context"
	"errors"
	"github.com/01max/librairii/internal/preferences"
	"os"
)

func (a *App) StartLibraryMove(preparationID string) SettingsSnapshot {
	if a.settings == nil {
		return a.Settings()
	}
	if err := a.beginLibraryMove(); err != nil {
		result := a.Settings()
		result.Error = err.Error()
		return result
	}
	s := a.settings
	s.mu.Lock()
	prepared := s.preparation
	if prepared == nil || prepared.ID != preparationID || s.storage.Isolated() {
		s.mu.Unlock()
		a.libraryGate.Unlock()
		result := a.Settings()
		result.Error = "settings.invalid"
		return result
	}
	copy := *prepared
	prepared = &copy
	s.preparation = nil
	ctx, cancel := context.WithCancel(a.runtimeContext())
	s.cancelMove = cancel
	s.move = &MoveProgress{Phase: "copying", Total: prepared.Bytes, Cancelable: true}
	s.mu.Unlock()
	go func() { defer cancel(); a.runLibraryMove(ctx, prepared) }()
	return a.Settings()
}
func (a *App) CancelLibraryMove() SettingsSnapshot {
	if a.settings == nil {
		return a.Settings()
	}
	s := a.settings
	s.mu.Lock()
	if s.move != nil && s.move.Cancelable && s.cancelMove != nil {
		s.cancelMove()
	}
	s.mu.Unlock()
	return a.Settings()
}
func (a *App) runLibraryMove(ctx context.Context, p *MovePreparation) {
	defer a.libraryGate.Unlock()
	s := a.settings
	if err := s.storage.BeginRelocation(); err != nil {
		s.mu.Lock()
		s.move.Phase = "failed"
		s.move.Error = "settings.busy"
		s.move.Cancelable = false
		s.cancelMove = nil
		s.mu.Unlock()
		return
	}
	defer s.storage.EndRelocation()
	directory := s.storage.ConfigurationDirectory()
	j := preferences.Journal{ID: p.ID, LibraryID: p.LibraryID, Source: p.Source, Destination: p.Destination, Phase: "reserved"}
	committed := false
	paused := false
	phase := func(value string, cancelable bool) {
		s.mu.Lock()
		s.move.Phase = value
		s.move.Cancelable = cancelable
		s.mu.Unlock()
	}
	work := func() error {
		current := s.storage.Profile()
		canonicalSource, _ := preferences.Canonical(current.Root)
		if canonicalSource != p.Source || current.LibraryID != p.LibraryID {
			return preferences.ErrInvalid
		}
		if _, err := preferences.ReadJournal(directory); err == nil {
			return preferences.ErrCleanupPending
		} else if !errors.Is(err, os.ErrNotExist) {
			return err
		}
		if err := preferences.EmptyTarget(p.Destination, directory, p.Source); err != nil {
			return err
		}
		if err := preferences.ValidateLibrary(ctx, p.Source, p.LibraryID); err != nil {
			return err
		}
		paused = true
		if err := a.core.PauseStorage(); err != nil {
			return err
		}
		var err error
		j.Entries, err = preferences.Manifest(ctx, p.Source)
		if err != nil {
			return err
		}
		var total int64
		for _, entry := range j.Entries {
			total += entry.Size
		}
		s.mu.Lock()
		s.move.Total = total
		s.mu.Unlock()
		if err = preferences.WriteJournal(directory, j); err != nil {
			return err
		}
		if err = preferences.Reserve(p.Destination, p.ID, total); err != nil {
			return err
		}
		if err = preferences.CopyLibrary(ctx, j, func(n int64) { s.mu.Lock(); s.move.Copied += n; s.mu.Unlock() }); err != nil {
			return err
		}
		if err = s.storage.ProtectRelocationWorkspace(j.Workspace()); err != nil {
			return err
		}
		phase("verifying", true)
		if err = preferences.VerifyCopy(ctx, j, j.Workspace()); err != nil {
			return err
		}
		if err = ctx.Err(); err != nil {
			return err
		}
		// Cancellation admission and the commit transition share the same mutex.
		s.mu.Lock()
		if err = ctx.Err(); err == nil {
			s.move.Cancelable = false
			s.move.Phase = "committing"
		}
		s.mu.Unlock()
		if err != nil {
			return err
		}
		if err = preferences.Publish(j.Workspace(), p.Destination, p.ID); err != nil {
			return err
		}
		if err = s.storage.PublishedRelocation(j.Workspace(), p.Destination); err != nil {
			return err
		}
		j.Phase = "published"
		if err = preferences.WriteJournal(directory, j); err != nil {
			return err
		}
		if err = preferences.VerifyCopy(ctx, j, p.Destination); err != nil {
			return err
		}
		if err = s.storage.CommitRoot(p.Destination); err != nil {
			return err
		}
		committed = true
		phase("cleanup", false)
		if err = preferences.Reconcile(context.Background(), directory, s.storage.Profile()); err != nil {
			return err
		}
		return nil
	}
	err := work()
	if !committed && !errors.Is(err, preferences.ErrCommitUncertain) {
		if _, journalErr := preferences.ReadJournal(directory); journalErr == nil {
			recoveryErr := preferences.Reconcile(context.Background(), directory, s.storage.Profile())
			if recoveryErr != nil {
				err = errors.Join(err, recoveryErr)
			}
		}
	}
	if paused {
		_ = a.core.ResumeStorage(a.runtimeContext())
	}
	s.mu.Lock()
	s.cancelMove = nil
	s.move.Cancelable = false
	switch {
	case err == nil:
		s.move.Phase = "succeeded"
	case errors.Is(err, context.Canceled):
		s.move.Phase = "cancelled"
	case committed:
		s.move.Phase = "cleanup_pending"
		s.move.CleanupPath = p.Source
		s.move.Error = "settings.cleanup_pending"
	default:
		s.move.Phase = "failed"
		s.move.Error = "settings.move_failed"
	}
	if paused && !a.core.Status().MutationsAllowed {
		s.move.Error = "settings.library_missing"
	}
	if errors.Is(err, preferences.ErrCommitUncertain) {
		s.move.Error = "settings.restart_required"
	}
	s.selected = ""
	s.mu.Unlock()
}
