package main

import (
	"encoding/json"
	"io/fs"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/wailsapp/wails/v2/pkg/logger"
	"github.com/wailsapp/wails/v2/pkg/options/linux"
	"github.com/wailsapp/wails/v2/pkg/options/windows"
)

func TestProductionOptionsHardenTheDesktopBoundary(t *testing.T) {
	t.Parallel()

	app := &App{}
	configuration, err := productionOptions(
		app,
		http.NotFoundHandler(),
		fstest.MapFS{
			"frontend/dist/index.html": &fstest.MapFile{
				Data: []byte(
					`<!doctype html><script src="/assets/index.js"></script>`,
				),
			},
			"frontend/dist/assets/index.js": &fstest.MapFile{Data: []byte("export {};")},
		},
	)
	if err != nil {
		t.Fatalf("production options: %v", err)
	}

	if configuration.Title != applicationTitle ||
		configuration.Width != 1180 ||
		configuration.MinWidth != 560 {
		t.Fatalf("unexpected release window configuration: %#v", configuration)
	}
	if configuration.LogLevel != logger.WARNING ||
		configuration.LogLevelProduction != logger.ERROR {
		t.Fatalf(
			"unexpected release logging: development=%s production=%s",
			configuration.LogLevel,
			configuration.LogLevelProduction,
		)
	}
	if configuration.EnableDefaultContextMenu ||
		configuration.EnableFraudulentWebsiteDetection ||
		configuration.BindingsAllowedOrigins != "" ||
		configuration.Debug.OpenInspectorOnStartup {
		t.Fatal("production browser capabilities are not hardened")
	}
	if configuration.DragAndDrop == nil ||
		!configuration.DragAndDrop.DisableWebViewDrop {
		t.Fatal("webview document drops must be disabled")
	}
	if len(configuration.Bind) != 1 || configuration.Bind[0] != app {
		t.Fatalf("unexpected Wails bindings: %#v", configuration.Bind)
	}
	if configuration.Windows == nil ||
		!configuration.Windows.DisablePinchZoom ||
		configuration.Windows.IsZoomControlEnabled ||
		configuration.Windows.WebviewDisableRendererCodeIntegrity {
		t.Fatal("Windows webview options are not hardened")
	}
	wantDLLSearch := windows.DLLSearchApplicationDir |
		windows.DLLSearchSystem32 |
		windows.DLLSearchUserDirs
	if configuration.Windows.DLLSearchPaths != wantDLLSearch {
		t.Fatalf(
			"unexpected Windows DLL search policy: %x",
			configuration.Windows.DLLSearchPaths,
		)
	}
	if configuration.Mac == nil || !configuration.Mac.DisableZoom {
		t.Fatal("macOS webview zoom must be disabled")
	}
	if configuration.Linux == nil ||
		configuration.Linux.ProgramName != applicationProgramID ||
		configuration.Linux.WebviewGpuPolicy != linux.WebviewGpuPolicyOnDemand {
		t.Fatalf("unexpected Linux release options: %#v", configuration.Linux)
	}
}

func TestProductionOptionsRequireTheEmbeddedFrontend(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name       string
		filesystem fs.FS
	}{
		{name: "missing filesystem"},
		{name: "missing index", filesystem: fstest.MapFS{}},
		{
			name: "empty index",
			filesystem: fstest.MapFS{
				"frontend/dist/index.html": &fstest.MapFile{},
			},
		},
		{
			name: "missing compiled script",
			filesystem: fstest.MapFS{
				"frontend/dist/index.html": &fstest.MapFile{
					Data: []byte("<!doctype html>"),
				},
			},
		},
		{
			name: "development source map",
			filesystem: fstest.MapFS{
				"frontend/dist/index.html": &fstest.MapFile{
					Data: []byte("<!doctype html>"),
				},
				"frontend/dist/assets/index.js": &fstest.MapFile{
					Data: []byte("export {};"),
				},
				"frontend/dist/assets/index.js.map": &fstest.MapFile{
					Data: []byte("{}"),
				},
			},
		},
		{
			name: "development client",
			filesystem: fstest.MapFS{
				"frontend/dist/index.html": &fstest.MapFile{
					Data: []byte(`<script src="/@vite/client"></script>`),
				},
				"frontend/dist/assets/index.js": &fstest.MapFile{
					Data: []byte("export {};"),
				},
			},
		},
	}
	for _, testCase := range cases {
		testCase := testCase
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			_, err := productionOptions(
				&App{},
				http.NotFoundHandler(),
				testCase.filesystem,
			)
			if err == nil || !strings.Contains(err.Error(), "embedded frontend") {
				t.Fatalf("expected embedded frontend error, got %v", err)
			}
		})
	}
}

func TestBindingFacadeMatchesTheReviewedAllowlist(t *testing.T) {
	t.Parallel()

	if err := validateBindingFacade(&App{}); err != nil {
		t.Fatal(err)
	}
	if err := validateBindingFacade(nil); err == nil {
		t.Fatal("expected a missing-facade error")
	}
}

func TestReleaseMetadataUsesStableProductIdentity(t *testing.T) {
	t.Parallel()

	configBytes, err := os.ReadFile("wails.json")
	if err != nil {
		t.Fatalf("read Wails configuration: %v", err)
	}
	var config struct {
		Name string `json:"name"`
		Info struct {
			CompanyName    string `json:"companyName"`
			ProductName    string `json:"productName"`
			ProductVersion string `json:"productVersion"`
			Copyright      string `json:"copyright"`
			Comments       string `json:"comments"`
		} `json:"info"`
	}
	if err := json.Unmarshal(configBytes, &config); err != nil {
		t.Fatalf("parse Wails configuration: %v", err)
	}
	if config.Name != applicationTitle ||
		config.Info.CompanyName != applicationTitle ||
		config.Info.ProductName != applicationTitle ||
		config.Info.Copyright == "" ||
		config.Info.Comments == "" {
		t.Fatalf("incomplete release metadata: %#v", config)
	}
	versionBytes, err := os.ReadFile("VERSION")
	if err != nil {
		t.Fatalf("read release version: %v", err)
	}
	version := strings.TrimSpace(string(versionBytes))
	if version == "" || config.Info.ProductVersion != version {
		t.Fatalf(
			"Wails product version %q does not match VERSION %q",
			config.Info.ProductVersion,
			version,
		)
	}

	assertContainsFile := func(path string, fragments ...string) {
		t.Helper()
		content, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("read %s: %v", path, err)
		}
		for _, fragment := range fragments {
			if !strings.Contains(string(content), fragment) {
				t.Errorf("%s does not contain %q", path, fragment)
			}
		}
	}
	assertContainsFile("build/darwin/Info.plist", applicationProgramID)
	assertContainsFile(
		"build/darwin/Info.dev.plist",
		applicationProgramID+".dev",
	)
	assertContainsFile(
		"build/windows/wails.exe.manifest",
		applicationProgramID,
		`requestedExecutionLevel level="asInvoker"`,
	)
}

func TestProductionSecurityHeadersCoverEmbeddedAndArtworkResponses(t *testing.T) {
	t.Parallel()

	var called bool
	handler := productionSecurityHeaders(http.HandlerFunc(
		func(writer http.ResponseWriter, request *http.Request) {
			called = true
			writer.WriteHeader(http.StatusNoContent)
		},
	))
	recorder := httptest.NewRecorder()
	handler.ServeHTTP(
		recorder,
		httptest.NewRequest(http.MethodGet, "/artwork/opaque-id", nil),
	)

	if !called || recorder.Code != http.StatusNoContent {
		t.Fatalf("security middleware did not delegate: status=%d", recorder.Code)
	}
	headers := recorder.Header()
	for name, want := range map[string]string{
		"Content-Security-Policy":    contentSecurityPolicy,
		"Cross-Origin-Opener-Policy": "same-origin",
		"Permissions-Policy":         "camera=(), geolocation=(), microphone=()",
		"Referrer-Policy":            "no-referrer",
		"X-Content-Type-Options":     "nosniff",
	} {
		if got := headers.Get(name); got != want {
			t.Fatalf("%s = %q, want %q", name, got, want)
		}
	}
}
