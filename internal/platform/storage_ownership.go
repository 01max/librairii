package platform

import (
	"errors"
	"os"
	"path/filepath"

	"github.com/01max/librairii/internal/storage"
)

// Helpers run under mu. Profile authority excludes cooperating recovery across
// root changes; library leases implement the shared Rust/Go flock protocol.
func (r *StorageReadiness) acquireProfile() error {
	if r.closeFailure != nil {
		return r.closeFailure
	}
	if r.profileStore == nil {
		return nil
	}
	if r.profileLease != nil {
		return r.profileLease.Revalidate()
	}
	directory := r.profileStore.Directory()
	if err := os.MkdirAll(directory, 0700); err != nil {
		return err
	}
	lease, err := storage.AcquireProfile(directory)
	if err != nil {
		return err
	}
	r.profileLease = lease
	return nil
}

func (r *StorageReadiness) acquireLibrary(root string, create bool) error {
	if r.closeFailure != nil {
		return r.closeFailure
	}
	if create {
		if err := os.MkdirAll(root, 0700); err != nil {
			return err
		}
	}
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return err
	}
	if lease := r.leases[canonical]; lease != nil {
		if err := lease.Revalidate(); err != nil {
			return err
		}
		return rejectNativeLibrary(canonical)
	}
	lease, err := storage.AcquireLibrary(canonical)
	if err != nil {
		return err
	}
	if r.leases == nil {
		r.leases = make(map[string]*storage.Lease)
	}
	r.leases[canonical] = lease
	return rejectNativeLibrary(canonical)
}

// Native query versions and recovery records can outlive Go's understanding.
// Presence is refusal only, never cleanup authority or permission to parse it.
func rejectNativeLibrary(root string) error {
	_, err := os.Lstat(filepath.Join(root, ".librairii-native-state.json"))
	if err == nil {
		return storage.ErrNativeLibrary
	}
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	return err
}

func (r *StorageReadiness) releaseOwnership() error {
	var failures []error
	for path, lease := range r.leases {
		failures = append(failures, lease.Close())
		delete(r.leases, path)
	}
	if r.profileLease != nil {
		failures = append(failures, r.profileLease.Close())
		r.profileLease = nil
	}
	return errors.Join(failures...)
}

// Retain leases through PauseStorage, root commitment, cleanup and reconstruction.
func (r *StorageReadiness) BeginRelocation() error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.relocating || r.db == nil {
		return storage.ErrLibraryInUse
	}
	if err := r.acquireProfile(); err != nil {
		return err
	}
	if err := r.acquireLibrary(r.layout.Root, false); err != nil {
		return err
	}
	r.relocating = true
	return nil
}

func (r *StorageReadiness) ProtectRelocationWorkspace(root string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.relocating {
		return storage.ErrLeaseChanged
	}
	return r.acquireLibrary(root, false)
}

func (r *StorageReadiness) PublishedRelocation(workspace, target string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	parent, err := filepath.EvalSymlinks(filepath.Dir(workspace))
	if err != nil {
		return err
	}
	canonical := filepath.Join(parent, filepath.Base(workspace))
	lease := r.leases[canonical]
	if !r.relocating || lease == nil {
		return storage.ErrLeaseChanged
	}
	if err := lease.Retarget(target); err != nil {
		return err
	}
	delete(r.leases, canonical)
	canonical, err = filepath.EvalSymlinks(target)
	if err != nil {
		return err
	}
	r.leases[canonical] = lease
	return nil
}

func (r *StorageReadiness) EndRelocation() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.relocating = false
	// Uncertain cleanup must not release a source lease early. The final Close
	// releases every retained source/destination lease after durable work ends.
}
