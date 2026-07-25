package main

import (
	"os"
	"strings"
	"testing"
)

func TestUserGuideCoversReleaseBehavior(t *testing.T) {
	t.Parallel()

	body, err := os.ReadFile("docs/user-guide.md")
	if err != nil {
		t.Fatal(err)
	}
	guide := string(body)
	required := []string{
		"## Supported imports",
		".plain.pk",
		".v1.pk",
		".v2.pk",
		".pk",
		".zip",
		".7z",
		"## Export passthrough and Lunii.QT",
		"discover, pair with, update",
		"## Search and saved shelves",
		"`%` and `_` have no",
		"## Application data, trash, and recovery",
		"librairii.pre-migration-vNNN-",
		"schema-conflict-recovery-*",
		"## Official metadata and provenance",
		"last-known-good snapshot",
		"## Privacy and network behavior",
		"no archive upload",
		"## Attribution and independence",
		"c8afe43dde21c2be33c0667ced962dc023eb948a",
	}
	for _, fragment := range required {
		if !strings.Contains(guide, fragment) {
			t.Errorf("user guide does not contain %q", fragment)
		}
	}
}
