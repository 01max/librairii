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

func TestReleaseMatrixHasIndependentPlatformTasks(t *testing.T) {
	t.Parallel()

	body, err := os.ReadFile("docs/release-platforms.md")
	if err != nil {
		t.Fatal(err)
	}
	matrix := string(body)
	required := []string{
		"Windows x64",
		"make verify-platform-windows",
		"scripts/verify-platform-windows.ps1",
		"Linux x64 with WebKitGTK 4.1",
		"make verify-platform-linux",
		"scripts/verify-platform-linux",
		"launch the actual packaged",
		"create and reopen SQLite",
		"native dialog/reveal adapter checks",
		"complete\nheadless story-library smoke",
		".github/workflows/platform-release.yml",
	}
	for _, fragment := range required {
		if !strings.Contains(matrix, fragment) {
			t.Errorf("release matrix does not contain %q", fragment)
		}
	}

	implementationFiles := map[string][]string{
		".github/workflows/platform-release.yml": {
			"runs-on: windows-2025",
			"runs-on: ubuntu-24.04",
			"scripts/verify-platform-windows.ps1",
			"scripts/verify-platform-linux",
		},
		"scripts/verify-platform-windows.ps1": {
			"-platform windows/amd64",
			"-nsis",
			"LIBRAIRII_PACKAGED_ACCEPTANCE",
			"scenario_started",
			"./cmd/foundation-smoke",
			"./internal/platform",
			"./cmd/release-smoke",
		},
		"scripts/verify-platform-linux": {
			"-platform linux/amd64",
			"-tags webkit2_41",
			"LIBRAIRII_PACKAGED_ACCEPTANCE",
			"scenario_started",
			"./cmd/foundation-smoke",
			"./internal/platform",
			"./cmd/release-smoke",
		},
	}
	for path, fragments := range implementationFiles {
		contents, err := os.ReadFile(path)
		if err != nil {
			t.Errorf("read %s: %v", path, err)
			continue
		}
		for _, fragment := range fragments {
			if !strings.Contains(string(contents), fragment) {
				t.Errorf("%s does not contain %q", path, fragment)
			}
		}
	}
}
