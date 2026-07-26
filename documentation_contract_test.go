package main

import (
	"encoding/json"
	"os"
	"strings"
	"testing"

	"github.com/01max/librairii/internal/performancefixture"
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
		"make verify-platform-windows-hosted",
		"scripts/verify-platform-windows.ps1",
		"Unqualified:",
		"Librairii-windows-amd64-candidate",
		"Linux x64 with WebKitGTK 4.1",
		"make verify-platform-linux",
		"scripts/verify-platform-linux",
		"launch the actual packaged",
		"create and reopen SQLite",
		"host-native dialog",
		"complete headless story-library smoke",
		".github/workflows/platform-release.yml",
	}
	for _, fragment := range required {
		if !strings.Contains(matrix, fragment) {
			t.Errorf("release matrix does not contain %q", fragment)
		}
	}

	implementationFiles := map[string][]string{
		".github/workflows/platform-release.yml": {
			"runs-on: windows-11-arm",
			"runs-on: ubuntu-24.04",
			"dbus-x11",
			"pcmanfm",
			"procps",
			"scripts/verify-platform-windows.ps1",
			"-ArtifactOnly",
			"Upload unqualified Windows candidate installer",
			"Librairii-windows-amd64-candidate",
			"scripts/verify-platform-linux",
		},
		"scripts/verify-platform-windows.ps1": {
			"-platform windows/amd64",
			"-nsis",
			"makensis.exe",
			"ProgramFiles(x86)",
			"InstalledBinary",
			"uninstall.exe",
			"uninstall-retention.txt",
			"LIBRAIRII_PACKAGED_ACCEPTANCE",
			"scenario_started",
			"native_import_dialog_selected",
			"native_destination_dialog_selected",
			"native_reveal_succeeded",
			"Write-PackagedApplicationDiagnostics",
			"Packaged acceptance checkpoints:",
			"Recent packaged lifecycle events:",
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
			"gtk_file_chooser_set_current_folder",
			"selectionReady",
			"gtk_widget_get_mapped",
		},
		"packaged_acceptance_native_windows.go": {
			"FindWindowW",
			"PostMessageW",
			"windowMessageCommand",
			"dialogCommandOK",
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

func TestBuildEntrypointsGenerateFrontendBeforeGoConsumesEmbed(t *testing.T) {
	t.Parallel()

	makefile, err := os.ReadFile("Makefile")
	if err != nil {
		t.Fatal(err)
	}
	for _, target := range []string{
		"check: build-frontend",
		"build: build-frontend",
		"build-current-installer: build-frontend",
		"verify-platform-linux: build-frontend",
		"verify-platform-windows: build-frontend",
		"verify-platform-windows-hosted: build-frontend",
	} {
		if !strings.Contains(string(makefile), target) {
			t.Errorf("Makefile does not declare %q", target)
		}
	}

	entrypoints := []struct {
		path       string
		frontend   string
		goConsumer string
	}{
		{
			path:       "scripts/build-current-installer",
			frontend:   "npm --prefix frontend run build",
			goConsumer: `"$wails_cli" build`,
		},
		{
			path:       "scripts/verify-platform-linux",
			frontend:   "npm --prefix frontend run build",
			goConsumer: `"$wails_cli" build`,
		},
		{
			path:       "scripts/verify-platform-windows.ps1",
			frontend:   "& npm --prefix frontend run build",
			goConsumer: "& $WailsCLI build",
		},
	}
	for _, entrypoint := range entrypoints {
		body, readErr := os.ReadFile(entrypoint.path)
		if readErr != nil {
			t.Errorf("read %s: %v", entrypoint.path, readErr)
			continue
		}
		contents := string(body)
		frontendIndex := strings.Index(contents, entrypoint.frontend)
		goConsumerIndex := strings.Index(contents, entrypoint.goConsumer)
		if frontendIndex < 0 {
			t.Errorf("%s does not build the frontend", entrypoint.path)
			continue
		}
		if goConsumerIndex < 0 {
			t.Errorf("%s does not contain the Wails build", entrypoint.path)
			continue
		}
		if frontendIndex > goConsumerIndex {
			t.Errorf("%s builds the frontend after Wails consumes the embed", entrypoint.path)
		}
	}
}

func TestPerformanceFixturesSeparateBrowserAndBackendScale(t *testing.T) {
	t.Parallel()

	body, err := os.ReadFile("frontend/performance-config.json")
	if err != nil {
		t.Fatal(err)
	}
	var config struct {
		BrowserStoryCount int `json:"browserStoryCount"`
	}
	if err := json.Unmarshal(body, &config); err != nil {
		t.Fatal(err)
	}
	if config.BrowserStoryCount != 1_000 {
		t.Fatalf(
			"browser performance story count = %d, want 1000",
			config.BrowserStoryCount,
		)
	}
	if performancefixture.MinimumLargeLibraryStories < 5_000 {
		t.Fatalf(
			"backend performance story count = %d, want at least 5000",
			performancefixture.MinimumLargeLibraryStories,
		)
	}
}

func TestFrontendPerformanceGatesProductCoupledMetricsOnly(t *testing.T) {
	t.Parallel()

	body, err := os.ReadFile("frontend/scripts/frontend-performance.mjs")
	if err != nil {
		t.Fatal(err)
	}
	contents := string(body)
	for _, expected := range []string{
		"const acceptanceBudgets = {",
		"const diagnosticThresholds = {",
		"expansionP95Milliseconds <=\n                acceptanceBudgets.expansionP95Milliseconds",
		"inputDelayP95Milliseconds <=\n                acceptanceBudgets.inputDelayP95Milliseconds",
	} {
		if !strings.Contains(contents, expected) {
			t.Errorf("frontend performance gate is missing %q", expected)
		}
	}
	for _, schedulerPredicate := range []string{
		"frameGapP95Milliseconds <=",
		"timerDelayP95Milliseconds <=",
	} {
		if strings.Contains(contents, schedulerPredicate) {
			t.Errorf(
				"frontend performance gate treats host scheduler metric as acceptance: %q",
				schedulerPredicate,
			)
		}
	}
}

func TestWindowsHostedWorkflowCannotClaimPackagedQualification(t *testing.T) {
	t.Parallel()

	workflowBody, err := os.ReadFile(".github/workflows/platform-release.yml")
	if err != nil {
		t.Fatal(err)
	}
	workflow := string(workflowBody)
	if !strings.Contains(workflow, "runs-on: windows-11-arm") {
		t.Fatal("Windows candidate artifact does not use the Windows runner")
	}
	hostedVerification := strings.Index(
		workflow,
		"run: ./scripts/verify-platform-windows.ps1 -ArtifactOnly",
	)
	if hostedVerification < 0 {
		t.Fatal("Windows workflow does not use artifact-only verification")
	}
	for _, forbidden := range []string{
		"Upload qualified Windows installer",
		"name: Librairii-windows-amd64\n",
		"run: ./scripts/ensure-webview2-runtime.ps1",
	} {
		if strings.Contains(workflow, forbidden) {
			t.Errorf("Windows hosted workflow claims qualification with %q", forbidden)
		}
	}

	verifierBody, err := os.ReadFile("scripts/verify-platform-windows.ps1")
	if err != nil {
		t.Fatal(err)
	}
	verifier := string(verifierBody)
	for _, expected := range []string{
		`$HostArch -notin @("amd64", "arm64")`,
		"[switch]$ArtifactOnly",
		"-platform windows/amd64",
		`$env:GOARCH = "amd64"`,
		`Join-Path $PSScriptRoot "ensure-webview2-runtime.ps1"`,
		"Invoke-PackagedApplication $InstalledBinary",
		"Packaged GUI qualification was not run",
		"0x8664",
	} {
		if !strings.Contains(verifier, expected) {
			t.Errorf("Windows amd64 verifier is missing %q", expected)
		}
	}
	if strings.Contains(verifier, "native_webview2loader") {
		t.Error("Windows verifier still ships the failed legacy WebView2 loader")
	}

	preflightBody, err := os.ReadFile("scripts/ensure-webview2-runtime.ps1")
	if err != nil {
		t.Fatal(err)
	}
	preflightScript := string(preflightBody)
	for _, expected := range []string{
		"{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
		"https://go.microsoft.com/fwlink/p/?LinkId=2124703",
		"Get-AuthenticodeSignature",
		`@("/silent", "/install")`,
	} {
		if !strings.Contains(preflightScript, expected) {
			t.Errorf("WebView2 preflight is missing %q", expected)
		}
	}
}

func TestWindowsInstallerSupportsAMD64EmulationOnWindows11ARM64(t *testing.T) {
	t.Parallel()

	body, err := os.ReadFile("build/windows/installer/project.nsi")
	if err != nil {
		t.Fatal(err)
	}
	installer := string(body)
	for _, expected := range []string{
		"!macro librairii.checkArchitecture",
		"!macro librairii.files",
		"${If} ${IsNativeAMD64}",
		"${If} ${IsNativeARM64}",
		"${If} ${AtLeastWin11}",
		`File "/oname=${PRODUCT_EXECUTABLE}" "${ARG_WAILS_AMD64_BINARY}"`,
		"!insertmacro librairii.checkArchitecture",
		"!insertmacro librairii.files",
	} {
		if !strings.Contains(installer, expected) {
			t.Errorf("Windows installer emulation override is missing %q", expected)
		}
	}
	for _, nativeOnlyMacro := range []string{
		"!insertmacro wails.checkArchitecture",
		"!insertmacro wails.files",
	} {
		if strings.Contains(installer, nativeOnlyMacro) {
			t.Errorf(
				"Windows installer still invokes Wails native-only macro %q",
				nativeOnlyMacro,
			)
		}
	}
}
