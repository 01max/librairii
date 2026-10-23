package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"

	"github.com/01max/librairii/internal/database"
	"github.com/01max/librairii/internal/preferences"
)

func recoveryError(err error) string {
	if err == nil {
		return ""
	}
	switch {
	case errors.Is(err, preferences.ErrCleanupPending):
		return "cleanup_pending"
	case errors.Is(err, preferences.ErrMissingLibrary):
		return "library_missing"
	case errors.Is(err, preferences.ErrInvalid):
		return "settings_invalid"
	case errors.Is(err, os.ErrNotExist):
		return "not_found"
	}
	panic(fmt.Sprintf("unclassified recovery fixture error: %v", err))
}
func exists(path string) bool {
	_, err := os.Lstat(path)
	if err == nil {
		return true
	}
	if errors.Is(err, os.ErrNotExist) {
		return false
	}
	panic(err)
}
func cloneManifest(source, target string, entries []preferences.Entry) {
	must(os.Mkdir(target, 0700))
	for _, entry := range entries {
		path := filepath.Join(target, entry.Path)
		if entry.Directory {
			must(os.Mkdir(path, 0700))
			continue
		}
		must(os.WriteFile(path, bytesAt(filepath.Join(source, entry.Path)), 0600))
	}
}
func manifestIfPresent(path string) []preferences.Entry {
	if !exists(path) {
		return []preferences.Entry{}
	}
	return take(preferences.Manifest(ctx, path))
}

// Finalize the real closed SQLite representation before Manifest, exactly as a
// move pauses/closes storage before capturing entries. VACUUM INTO starts a
// standalone DELETE-mode snapshot; an ordinary Go open/close establishes the
// retained WAL header so VerifyCopy does not alter the captured DB bytes.
func relocationFixtures(libraryRoot, libraryID string) {
	finalized := take(database.Open(ctx, preferences.DatabasePath(libraryRoot)))
	must(finalized.Close())
	entries := take(preferences.Manifest(ctx, libraryRoot))
	var marker []preferences.Entry
	archiveCount, dbCount := 0, 0
	for _, e := range entries {
		if e.Path == preferences.IdentityFile {
			marker = append(marker, e)
		}
		if e.Path == "db/librairii.sqlite3" {
			dbCount++
		}
		if !e.Directory && filepath.Dir(e.Path) == "archives" {
			archiveCount++
		}
	}
	if dbCount != 1 || archiveCount != 5 || len(marker) != 1 {
		panic("finalized library manifest is incomplete")
	}
	jsonFile("profiles/library-manifest.json", entries)
	base := preferences.Journal{ID: "b0000000-0000-4000-8000-000000000001", LibraryID: libraryID, Source: "/fixture/source", Destination: "/fixture/destination", Phase: "reserved", Entries: entries}
	for _, phase := range []string{"reserved", "published"} {
		j := base
		j.Phase = phase
		must(j.Validate())
		jsonFile("profiles/relocation-"+phase+".json", j)
	}
	negative := base
	negative.Phase = "published"
	negative.Entries = marker
	must(negative.Validate())
	jsonFile("profiles/relocation-incomplete-negative.json", negative)
	outcomes := []any{}
	for _, test := range []struct {
		name, phase                                  string
		published, committed, incomplete, verifyOnly bool
	}{
		{"reserved-copied-before-commit", "reserved", false, false, false, false},
		{"published-before-profile-commit", "published", true, false, false, false},
		{"published-after-profile-commit", "published", true, true, false, false},
		{"incomplete-copy-negative", "reserved", false, false, true, true},
		{"incomplete-cleanup-negative", "published", true, true, true, false},
	} {
		dir := take(os.MkdirTemp("", "librairii-relocation-replay-"))
		defer os.RemoveAll(dir)
		source := filepath.Join(dir, "source")
		destination := filepath.Join(dir, "destination")
		config := filepath.Join(dir, "profile")
		cloneManifest(libraryRoot, source, entries)
		j := base
		j.Source = source
		j.Destination = destination
		j.Phase = test.phase
		if test.incomplete {
			j.Entries = marker
		}
		copyJournal := j
		if test.incomplete && !test.verifyOnly {
			copyJournal.Entries = entries
		}
		var total int64
		for _, entry := range copyJournal.Entries {
			total += entry.Size
		}
		must(preferences.Reserve(destination, j.ID, total))
		must(preferences.CopyLibrary(ctx, copyJournal, nil))
		verifyErr := preferences.VerifyCopy(ctx, copyJournal, copyJournal.Workspace())
		if test.verifyOnly {
			if !errors.Is(verifyErr, preferences.ErrMissingLibrary) {
				panic("incomplete copy did not fail missing-library verification")
			}
			outcomes = append(outcomes, map[string]any{"case": test.name, "classification": "negative incomplete manifest", "inputJournal": "relocation-incomplete-negative.json", "copyPhase": test.phase, "verifyError": recoveryError(verifyErr), "workspaceHasDatabase": exists(preferences.DatabasePath(j.Workspace())), "sourceUnchanged": reflect.DeepEqual(entries, manifestIfPresent(source)), "reconcile": "not attempted: copy cannot qualify for publication"})
			must(os.RemoveAll(dir))
			continue
		}
		must(verifyErr)
		if test.published {
			must(preferences.Publish(j.Workspace(), destination, j.ID))
			must(preferences.VerifyCopy(ctx, copyJournal, destination))
		}
		root := source
		authority := "source"
		if test.committed {
			root = destination
			authority = "destination"
		}
		p := preferences.Profile{Version: 1, Language: "fr", Root: root, LibraryID: libraryID, Complete: true}
		store := preferences.NewStore(config)
		take(store.Update(func(next *preferences.Profile) error { *next = p; return nil }))
		must(preferences.WriteJournal(config, j))
		err := preferences.Reconcile(ctx, config, take(preferences.NewStore(config).Load()))
		firstError := recoveryError(err)
		secondError := recoveryError(preferences.Reconcile(ctx, config, take(preferences.NewStore(config).Load())))
		// Reconcile probes a WAL database read-only and can leave empty WAL/SHM
		// files. Audit every captured byte through VerifyCopy and its real Go
		// writer open/close before recording a closed-library manifest; do not
		// omit files or scrub hashes from the observation.
		must(preferences.VerifyCopy(ctx, copyJournal, root))
		sourceManifest := manifestIfPresent(source)
		destinationManifest := manifestIfPresent(destination)
		if test.incomplete {
			if firstError != "cleanup_pending" || secondError != "cleanup_pending" || !exists(filepath.Join(config, "relocation.json")) || !reflect.DeepEqual(sourceManifest, entries) {
				panic("incomplete journal lost unlisted source files or claimed cleanup success")
			}
		} else {
			must(err)
			if secondError != "" || exists(j.Workspace()) || exists(filepath.Join(config, "relocation.json")) {
				panic("positive recovery was not complete and idempotent")
			}
			if test.committed {
				if exists(source) || !reflect.DeepEqual(destinationManifest, entries) {
					panic("committed recovery did not preserve all bytes and remove old source")
				}
				must(preferences.VerifyCopy(ctx, j, destination))
			} else {
				if exists(destination) || !reflect.DeepEqual(sourceManifest, entries) {
					panic(fmt.Sprintf("precommit recovery mismatch %s: destination=%t expected=%+v actual=%+v", test.name, exists(destination), entries, sourceManifest))
				}
				must(preferences.VerifyCopy(ctx, j, source))
			}
		}
		classification := "positive complete manifest"
		if test.incomplete {
			classification = "negative incomplete manifest"
		}
		reloaded := take(preferences.NewStore(config).Load())
		if reloaded.Root != root || reloaded.Language != "fr" {
			panic("recovery changed durable root/language authority")
		}
		outcomes = append(outcomes, map[string]any{"case": test.name, "classification": classification, "phase": test.phase, "profileAuthority": authority, "copyVerifyError": recoveryError(verifyErr), "reconcileError": firstError, "secondReconcileError": secondError, "sourceExists": exists(source), "destinationExists": exists(destination), "workspaceExists": exists(j.Workspace()), "journalExists": exists(filepath.Join(config, "relocation.json")), "sourceManifest": sourceManifest, "destinationManifest": destinationManifest, "profileLanguage": reloaded.Language, "closedAuthorityAudit": "complete-manifest VerifyCopy including real Go database open/close passed"})
		must(os.RemoveAll(dir))
	}
	jsonFile("profiles/recovery-outcomes.json", outcomes)
}
