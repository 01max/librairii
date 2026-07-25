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
		"host-native dialog and reveal acceptance",
		"complete headless\nstory-library smoke",
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
			"dbus-x11",
			"pcmanfm",
			"procps",
			"scripts/verify-platform-windows.ps1",
			"scripts/verify-platform-linux",
		},
		"scripts/verify-platform-windows.ps1": {
			"-platform windows/amd64",
			"-nsis",
			"InstalledBinary",
			"uninstall.exe",
			"uninstall-retention.txt",
			"LIBRAIRII_PACKAGED_ACCEPTANCE",
			"scenario_started",
			"native_import_dialog_selected",
			"native_destination_dialog_selected",
			"native_reveal_succeeded",
			"./cmd/foundation-smoke",
			"./internal/platform",
			"./cmd/release-smoke",
		},
		"scripts/verify-platform-linux": {
			"-platform linux/amd64",
			"-tags webkit2_41",
			"LIBRAIRII_PACKAGED_ACCEPTANCE",
			"scenario_started",
			"native_import_dialog_selected",
			"native_destination_dialog_selected",
			"native_reveal_succeeded",
			"dbus-run-session",
			"xdg-mime",
			"run-linux-native-acceptance",
			"./cmd/foundation-smoke",
			"./internal/platform",
			"./cmd/release-smoke",
		},
		"packaged_acceptance.go": {
			"a.native.OpenFiles",
			"a.native.OpenDirectory",
			"a.native.RevealDirectory",
			"native_import_dialog_selected",
			"native_destination_dialog_selected",
			"native_reveal_succeeded",
		},
		"packaged_acceptance_native_darwin.go": {
			"NSOpenPanel",
			"completeWithReturnCode:url:urls:",
		},
		"packaged_acceptance_native_linux.go": {
			"GTK_IS_FILE_CHOOSER",
			"gtk_file_chooser_select_filename",
		},
		"packaged_acceptance_native_windows.go": {
			"FindWindowW",
			"SendInput",
		},
		"scripts/run-linux-native-acceptance": {
			"/proc/$file_manager_pid/comm",
			"/usr/bin/pcmanfm",
			"pcmanfm",
			"expected_destination",
		},
		"scripts/librairii-linux-file-manager-evidence": {
			"LIBRAIRII_NATIVE_REVEAL_EVIDENCE",
			"/usr/bin/pcmanfm",
			"setsid --fork",
			"pgrep -x pcmanfm",
			"resolved_expected",
		},
		"build/linux/librairii-acceptance-file-manager.desktop": {
			"Exec=librairii-linux-file-manager-evidence %f",
			"MimeType=inode/directory;",
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
	windowsScript, err := os.ReadFile("scripts/verify-platform-windows.ps1")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(windowsScript), "-windowsconsole") {
		t.Error("Windows release verifier builds a console-subsystem executable")
	}
	if !strings.Contains(string(windowsScript), "0x0002") {
		t.Error("Windows release verifier does not enforce the GUI PE subsystem")
	}
}
