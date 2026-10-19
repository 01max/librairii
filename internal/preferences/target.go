package preferences

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
)

var ErrDestination = errors.New("settings.destination_invalid")

func Canonical(path string) (string, error) {
	if !filepath.IsAbs(path) {
		return "", ErrDestination
	}
	path = filepath.Clean(path)
	resolved, err := filepath.EvalSymlinks(path)
	if err == nil {
		return resolved, nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return "", err
	}
	parent, err := filepath.EvalSymlinks(filepath.Dir(path))
	if err != nil {
		return "", err
	}
	return filepath.Join(parent, filepath.Base(path)), nil
}
func Contains(parent, child string) bool {
	rel, err := filepath.Rel(parent, child)
	return err == nil && (rel == "." || rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator)))
}
func EmptyTarget(root, config, source string) error {
	target, err := Canonical(root)
	if err != nil {
		return ErrDestination
	}
	conf, err := Canonical(config)
	if err != nil {
		conf = filepath.Clean(config)
	}
	if Contains(target, conf) || Contains(conf, target) {
		return ErrDestination
	}
	if source != "" {
		old, err := Canonical(source)
		if err != nil {
			return err
		}
		if Contains(old, target) || Contains(target, old) {
			return ErrDestination
		}
	}
	entries, err := os.ReadDir(target)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return ErrDestination
	}
	if len(entries) > 0 {
		return ErrDestination
	}
	parent := filepath.Dir(target)
	if err == nil {
		parent = target
	}
	probe, err := os.CreateTemp(parent, ".librairii-probe-*")
	if err != nil {
		return ErrDestination
	}
	probe.Close()
	return os.Remove(probe.Name())
}

func TreeSize(root string) (int64, error) {
	var size int64
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return ErrDestination
		}
		if entry.IsDir() {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return ErrDestination
		}
		size += info.Size()
		return nil
	})
	return size, err
}

func containedResolved(root, path string) bool {
	canonical, err := Canonical(root)
	return err == nil && Contains(canonical, path)
}
