//go:build darwin || linux

package storage

import (
	"errors"
	"os"
	"path/filepath"
	"syscall"

	"golang.org/x/sys/unix"
)

const LibraryLockFile = ".librairii-library.lock"

// LibraryEntries distinguishes the reserved, empty protocol inode from user
// payload. Callers must still acquire their lifetime lease before any effect.
// A symlink, hard link, directory or nonempty file at this name is not a control.
func LibraryEntries(root string) ([]os.DirEntry, error) {
	entries, err := os.ReadDir(root)
	if err != nil {
		return nil, err
	}
	result := make([]os.DirEntry, 0, len(entries))
	for _, entry := range entries {
		if entry.Name() == LibraryLockFile {
			info, err := os.Lstat(filepath.Join(root, entry.Name()))
			if err != nil {
				return nil, err
			}
			if !singleRegular(info) || info.Size() != 0 {
				return nil, ErrLeaseChanged
			}
			continue
		}
		result = append(result, entry)
	}
	return result, nil
}

var ErrLibraryInUse = errors.New("close the other Librairii application before retrying")
var ErrLeaseChanged = errors.New("library ownership changed; restart before retrying")

// Lease implements the shared Rust/Go protocol: nonblocking exclusive flock on
// a retained regular file. A filename or PID never proves ownership. Closing a
// lease does not unlink it. An older executable that ignores this protocol must
// still be kept on the source of an independently verified clone adoption.
type Lease struct {
	root      string
	name      string
	directory *os.File
	file      *os.File
	rootInfo  os.FileInfo
	fileInfo  os.FileInfo
}

func AcquireLibrary(root string) (*Lease, error) { return acquireLease(root, LibraryLockFile) }
func AcquireProfile(directory string) (*Lease, error) {
	return acquireLease(directory, ".librairii-authority.lock")
}

func acquireLease(root, name string) (*Lease, error) {
	return acquireLeaseMode(root, name, true)
}

// Short preference publication is independent of the lifetime recovery lease.
// Serialize read-modify-write so a recovery window changing only its language
// cannot overwrite another process's newly committed library root.
func AcquirePreferenceUpdate(directory string) (*Lease, error) {
	return acquireLeaseMode(directory, ".librairii-preference-update.lock", false)
}

func acquireLeaseMode(root, name string, nonblocking bool) (*Lease, error) {
	if !filepath.IsAbs(root) {
		return nil, ErrDataRootNotAbsolute
	}
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return nil, err
	}
	fd, err := unix.Open(canonical, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_NOFOLLOW|unix.O_CLOEXEC, 0)
	if err != nil {
		return nil, err
	}
	directory := os.NewFile(uintptr(fd), canonical)
	lockFD, err := unix.Openat(fd, name, unix.O_RDWR|unix.O_CREAT|unix.O_NOFOLLOW|unix.O_NONBLOCK|unix.O_CLOEXEC, 0600)
	if err != nil {
		return nil, errors.Join(err, directory.Close())
	}
	file := os.NewFile(uintptr(lockFD), filepath.Join(canonical, name))
	refuse := func(err error) (*Lease, error) { return nil, errors.Join(err, file.Close(), directory.Close()) }
	info, err := file.Stat()
	if err != nil {
		return refuse(err)
	}
	if !singleRegular(info) {
		return refuse(ErrLeaseChanged)
	}
	flags := unix.LOCK_EX
	if nonblocking {
		flags |= unix.LOCK_NB
	}
	if err := unix.Flock(lockFD, flags); err != nil {
		if errors.Is(err, unix.EWOULDBLOCK) || errors.Is(err, unix.EAGAIN) {
			err = ErrLibraryInUse
		}
		return refuse(err)
	}
	rootInfo, err := directory.Stat()
	if err != nil {
		return refuse(err)
	}
	lease := &Lease{root: canonical, name: name, directory: directory, file: file, rootInfo: rootInfo, fileInfo: info}
	if err := lease.Revalidate(); err != nil {
		return refuse(err)
	}
	if err := errors.Join(file.Sync(), directory.Sync()); err != nil {
		return refuse(err)
	}
	return lease, nil
}

func (l *Lease) Revalidate() error {
	root, err := os.Stat(l.root)
	if err != nil {
		return err
	}
	if !os.SameFile(l.rootInfo, root) {
		return ErrLeaseChanged
	}
	named, err := os.Lstat(filepath.Join(l.root, l.name))
	if err != nil {
		return err
	}
	if !singleRegular(named) || !os.SameFile(l.fileInfo, named) {
		return ErrLeaseChanged
	}
	return nil
}

// Retarget retains the same locked inode after an owned workspace is published.
func (l *Lease) Retarget(root string) error {
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return err
	}
	previous := l.root
	l.root = canonical
	if err := l.Revalidate(); err != nil {
		l.root = previous
		return err
	}
	return nil
}

func (l *Lease) Close() error { return errors.Join(l.file.Close(), l.directory.Close()) }

func singleRegular(info os.FileInfo) bool {
	stat, ok := info.Sys().(*syscall.Stat_t)
	return ok && info.Mode().IsRegular() && stat.Nlink == 1
}
