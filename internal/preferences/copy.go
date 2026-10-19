package preferences

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"io"
	"os"
	"path/filepath"
)

type contextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (r contextReader) Read(p []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.reader.Read(p)
}
func fileHash(ctx context.Context, path string) (string, error) {
	info, err := os.Lstat(path)
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() {
		return "", ErrDestination
	}
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()
	opened, err := file.Stat()
	if err != nil || !os.SameFile(info, opened) {
		return "", ErrDestination
	}
	hash := sha256.New()
	if _, err := io.CopyBuffer(hash, contextReader{ctx, file}, make([]byte, 128*1024)); err != nil {
		return "", err
	}
	return hex.EncodeToString(hash.Sum(nil)), nil
}
func Manifest(ctx context.Context, root string) ([]Entry, error) {
	var entries []Entry
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if err := ctx.Err(); err != nil {
			return err
		}
		if path == root {
			return nil
		}
		rel, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		item := Entry{Path: rel, Directory: info.IsDir()}
		if !item.Directory {
			if !info.Mode().IsRegular() {
				return ErrDestination
			}
			item.Size = info.Size()
			item.SHA256, err = fileHash(ctx, path)
			if err != nil {
				return err
			}
		}
		entries = append(entries, item)
		return nil
	})
	return entries, err
}

type progressWriter struct {
	writer   io.Writer
	progress func(int64)
}

func (w progressWriter) Write(p []byte) (int, error) {
	n, err := w.writer.Write(p)
	if w.progress != nil {
		w.progress(int64(n))
	}
	return n, err
}
func CopyLibrary(ctx context.Context, j Journal, progress func(int64)) error {
	if err := j.Validate(); err != nil {
		return err
	}
	if err := os.Mkdir(j.Workspace(), 0700); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(j.Workspace(), moveMarker), []byte(j.ID), 0600); err != nil {
		return err
	}
	for _, entry := range j.Entries {
		if err := ctx.Err(); err != nil {
			return err
		}
		target := filepath.Join(j.Workspace(), entry.Path)
		if entry.Directory {
			if err := os.Mkdir(target, 0700); err != nil {
				return err
			}
			continue
		}
		if err := copyEntry(ctx, filepath.Join(j.Source, entry.Path), target, entry, progress); err != nil {
			return err
		}
	}
	for i := len(j.Entries) - 1; i >= 0; i-- {
		if j.Entries[i].Directory {
			if err := syncDirectory(filepath.Join(j.Workspace(), j.Entries[i].Path)); err != nil {
				return err
			}
		}
	}
	return syncDirectory(j.Workspace())
}
func copyEntry(ctx context.Context, source, target string, entry Entry, progress func(int64)) error {
	info, err := os.Lstat(source)
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() {
		return ErrDestination
	}
	input, err := os.Open(source)
	if err != nil {
		return err
	}
	defer input.Close()
	opened, err := input.Stat()
	if err != nil || !os.SameFile(info, opened) {
		return ErrDestination
	}
	output, err := os.OpenFile(target, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	hash := sha256.New()
	n, err := io.CopyBuffer(io.MultiWriter(output, hash), contextReader{ctx, progressReader{reader: input, progress: progress}}, make([]byte, 128*1024))
	if err == nil && (n != entry.Size || hex.EncodeToString(hash.Sum(nil)) != entry.SHA256) {
		err = ErrInvalid
	}
	if err == nil {
		err = output.Sync()
	}
	return errors.Join(err, output.Close())
}

type progressReader struct {
	reader   io.Reader
	progress func(int64)
}

func (r progressReader) Read(p []byte) (int, error) {
	n, err := r.reader.Read(p)
	if r.progress != nil {
		r.progress(int64(n))
	}
	return n, err
}
func syncDirectory(path string) error {
	dir, err := os.Open(path)
	if err != nil {
		return err
	}
	defer dir.Close()
	return dir.Sync()
}
