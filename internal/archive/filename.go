package archive

import (
	"path/filepath"
	"strings"
)

// ValidFilename accepts one stable filesystem entry name and rejects values
// whose normalization would change the name persisted in an operation.
func ValidFilename(name string) bool {
	return name != "" &&
		name == strings.TrimSpace(name) &&
		filepath.Base(name) == name &&
		name != "." &&
		name != ".." &&
		!strings.ContainsAny(name, `/\`) &&
		!strings.ContainsRune(name, '\x00')
}
