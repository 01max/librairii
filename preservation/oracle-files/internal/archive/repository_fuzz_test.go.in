package archive

import (
	"errors"
	"path/filepath"
	"strings"
	"testing"
)

func FuzzSafeJoinNeverEscapesManagedRoot(f *testing.F) {
	root := f.TempDir()
	for _, relativePath := range []string{
		"archives/aa/story.zip",
		"",
		".",
		"..",
		"../story.zip",
		"archives/../../story.zip",
		"/tmp/story.zip",
		`C:\private\story.zip`,
		`archives\..\story.zip`,
		"archives/story\x00.zip",
	} {
		f.Add(relativePath)
	}

	f.Fuzz(func(t *testing.T, relativePath string) {
		joined, err := SafeJoin(root, relativePath)
		if err != nil {
			if !errors.Is(err, ErrInvalidManagedPath) {
				t.Fatalf("SafeJoin(%q) error = %v", relativePath, err)
			}
			return
		}
		if !filepath.IsAbs(joined) ||
			filepath.Clean(joined) != joined ||
			strings.ContainsRune(joined, '\x00') {
			t.Fatalf("SafeJoin(%q) returned malformed path %q", relativePath, joined)
		}
		relative, err := filepath.Rel(root, joined)
		if err != nil ||
			relative == "." ||
			relative == ".." ||
			strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			t.Fatalf(
				"SafeJoin(%q) escaped root: joined=%q relative=%q error=%v",
				relativePath,
				joined,
				relative,
				err,
			)
		}
	})
}
