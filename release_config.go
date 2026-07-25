package main

import (
	"context"
	"fmt"
	"io/fs"
	"net/http"
	"reflect"
	"slices"
	"strconv"
	"strings"
	"time"

	"github.com/wailsapp/wails/v2/pkg/logger"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/linux"
	"github.com/wailsapp/wails/v2/pkg/options/mac"
	"github.com/wailsapp/wails/v2/pkg/options/windows"
)

const (
	applicationTitle      = "Librairii"
	applicationProgramID  = "io.github.librairii.app"
	maximumSmokeHold      = 10 * time.Second
	contentSecurityPolicy = "default-src 'self'; base-uri 'none'; " +
		"connect-src 'self'; font-src 'self'; form-action 'none'; " +
		"frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; " +
		"script-src 'self'; style-src 'self' 'unsafe-inline'"
)

var bindingMethodAllowlist = []string{
	"ActiveOperations",
	"ApplicationStatus",
	"CancelOperation",
	"CreateShelf",
	"CreateTagDefinition",
	"CreateTagValue",
	"DeleteShelf",
	"DeleteTagDefinition",
	"DeleteTagValue",
	"DuplicateShelf",
	"FrontendRendered",
	"ListShelves",
	"ListStories",
	"OfficialMetadataStatus",
	"OpenShelf",
	"OperationSnapshot",
	"PackagedAcceptanceMode",
	"PlanTagDefinitionDeletion",
	"PlanTagValueDeletion",
	"PreviewShelves",
	"QueryStories",
	"RecolorTagDefinition",
	"RecordPackagedAcceptance",
	"RecoverStorage",
	"RefreshOfficialMetadata",
	"RemoveStory",
	"RenameShelf",
	"RenameTagDefinition",
	"RenameTagValue",
	"ReorderShelves",
	"ReorderTagDefinitions",
	"ReorderTagValues",
	"ReplaceShelfQuery",
	"RevealExportDestination",
	"SelectAndExportDiagnostics",
	"SelectAndImportStories",
	"SelectAndPreflightExport",
	"SetBooleanTag",
	"SetChoiceTagValue",
	"SetChoiceTagValues",
	"StartPreparedExport",
	"StoryDetail",
	"TagAssignmentWorkspace",
	"TagCatalog",
}

func productionOptions(
	app *App,
	fallback http.Handler,
	embedded fs.FS,
) (*options.App, error) {
	frontendAssets, err := embeddedFrontendAssets(embedded)
	if err != nil {
		return nil, err
	}
	if err := validateBindingFacade(app); err != nil {
		return nil, err
	}

	return &options.App{
		Title:     applicationTitle,
		Width:     1180,
		Height:    900,
		MinWidth:  560,
		MinHeight: 640,
		AssetServer: &assetserver.Options{
			Assets:     frontendAssets,
			Handler:    fallback,
			Middleware: productionSecurityHeaders,
		},
		BackgroundColour:                 options.NewRGBA(27, 38, 54, 1),
		LogLevel:                         logger.WARNING,
		LogLevelProduction:               logger.ERROR,
		OnStartup:                        app.startup,
		OnDomReady:                       app.domReady,
		OnShutdown:                       app.shutdown,
		EnableDefaultContextMenu:         false,
		EnableFraudulentWebsiteDetection: false,
		BindingsAllowedOrigins:           "",
		Bind:                             []interface{}{app},
		Debug: options.Debug{
			OpenInspectorOnStartup: false,
		},
		DragAndDrop: &options.DragAndDrop{
			DisableWebViewDrop: true,
		},
		Windows: &windows.Options{
			DisablePinchZoom:                    true,
			IsZoomControlEnabled:                false,
			Theme:                               windows.SystemDefault,
			WebviewDisableRendererCodeIntegrity: false,
			DLLSearchPaths: windows.DLLSearchApplicationDir |
				windows.DLLSearchSystem32 |
				windows.DLLSearchUserDirs,
		},
		Mac: &mac.Options{
			DisableZoom: true,
			About: &mac.AboutInfo{
				Title: applicationTitle,
				Message: "A local-first story archive library for " +
					"Lunii.QT-compatible collections.",
			},
		},
		Linux: &linux.Options{
			ProgramName:      applicationProgramID,
			WebviewGpuPolicy: linux.WebviewGpuPolicyOnDemand,
		},
	}, nil
}

func configuredSmokeQuit(
	rawMilliseconds string,
	quit func(context.Context),
) (func(context.Context), error) {
	if quit == nil {
		return nil, fmt.Errorf("configure packaged smoke quit: missing callback")
	}
	if rawMilliseconds == "" {
		return quit, nil
	}
	milliseconds, err := strconv.Atoi(rawMilliseconds)
	if err != nil ||
		milliseconds < 0 ||
		time.Duration(milliseconds)*time.Millisecond > maximumSmokeHold {
		return nil, fmt.Errorf(
			"configure packaged smoke quit: hold must be 0-%d milliseconds",
			maximumSmokeHold.Milliseconds(),
		)
	}
	hold := time.Duration(milliseconds) * time.Millisecond
	return func(ctx context.Context) {
		go func() {
			timer := time.NewTimer(hold)
			defer timer.Stop()
			select {
			case <-ctx.Done():
			case <-timer.C:
				quit(ctx)
			}
		}()
	}, nil
}

func embeddedFrontendAssets(embedded fs.FS) (fs.FS, error) {
	if embedded == nil {
		return nil, fmt.Errorf("embedded frontend: missing filesystem")
	}
	frontendAssets, err := fs.Sub(embedded, "frontend/dist")
	if err != nil {
		return nil, fmt.Errorf("embedded frontend: %w", err)
	}
	index, err := fs.Stat(frontendAssets, "index.html")
	if err != nil {
		return nil, fmt.Errorf("embedded frontend index: %w", err)
	}
	if index.IsDir() || index.Size() == 0 {
		return nil, fmt.Errorf("embedded frontend index: invalid file")
	}
	if err := validateProductionAssets(frontendAssets); err != nil {
		return nil, err
	}
	return frontendAssets, nil
}

func validateProductionAssets(frontendAssets fs.FS) error {
	hasJavaScript := false
	err := fs.WalkDir(frontendAssets, ".", func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			return nil
		}
		if strings.HasSuffix(path, ".map") {
			return fmt.Errorf("development source map is embedded: %s", path)
		}
		if !strings.HasSuffix(path, ".js") && !strings.HasSuffix(path, ".html") {
			return nil
		}
		if strings.HasSuffix(path, ".js") {
			hasJavaScript = true
		}
		content, err := fs.ReadFile(frontendAssets, path)
		if err != nil {
			return err
		}
		for _, marker := range []string{
			"/@vite/client",
			"__vite_plugin_react_preamble_installed__",
			"sourceMappingURL=",
		} {
			if strings.Contains(string(content), marker) {
				return fmt.Errorf("development marker %q is embedded in %s", marker, path)
			}
		}
		return nil
	})
	if err != nil {
		return fmt.Errorf("embedded frontend assets: %w", err)
	}
	if !hasJavaScript {
		return fmt.Errorf("embedded frontend assets: missing compiled JavaScript")
	}
	return nil
}

func validateBindingFacade(app *App) error {
	if app == nil {
		return fmt.Errorf("validate Wails bindings: missing application facade")
	}
	appType := reflect.TypeOf(app)
	methods := make([]string, 0, appType.NumMethod())
	for index := range appType.NumMethod() {
		methods = append(methods, appType.Method(index).Name)
	}
	slices.Sort(methods)
	if !slices.Equal(methods, bindingMethodAllowlist) {
		return fmt.Errorf(
			"validate Wails bindings: exported methods changed: got %v, want %v",
			methods,
			bindingMethodAllowlist,
		)
	}
	return nil
}

func productionSecurityHeaders(next http.Handler) http.Handler {
	return http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.Header().Set("Content-Security-Policy", contentSecurityPolicy)
		writer.Header().Set("Cross-Origin-Opener-Policy", "same-origin")
		writer.Header().Set("Permissions-Policy", "camera=(), geolocation=(), microphone=()")
		writer.Header().Set("Referrer-Policy", "no-referrer")
		writer.Header().Set("X-Content-Type-Options", "nosniff")
		next.ServeHTTP(writer, request)
	})
}
