package main

import (
	"embed"
	"log"
	"os"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/platform"
	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/runtime"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
	readiness := platform.NewStorageReadiness(os.Getenv("LIBRAIRII_DATA_ROOT"))
	clock := platform.SystemClock{}
	events := platform.RuntimeEvents{}
	importRuntime, err := coreapp.NewImportRuntime(readiness, clock, events, 2)
	if err != nil {
		log.Fatal(err)
	}
	core, err := coreapp.New(coreapp.Dependencies{
		Clock:      clock,
		Dialogs:    platform.NewRuntimeDialogs(),
		Events:     events,
		Readiness:  readiness,
		Operations: importRuntime,
		Resources:  []coreapp.ResourcePort{readiness},
	})
	if err != nil {
		log.Fatal(err)
	}
	appOptions := make([]AppOption, 0, 1)
	if os.Getenv("LIBRAIRII_SMOKE_EXIT") == "1" {
		appOptions = append(appOptions, WithQuitAfterDOMReady(runtime.Quit))
	}
	app := NewApp(core, appOptions...)

	err = wails.Run(&options.App{
		Title:  "Librairii",
		Width:  1024,
		Height: 768,
		AssetServer: &assetserver.Options{
			Assets: assets,
		},
		BackgroundColour: &options.RGBA{R: 27, G: 38, B: 54, A: 1},
		OnStartup:        app.startup,
		OnDomReady:       app.domReady,
		OnShutdown:       app.shutdown,
		Bind: []interface{}{
			app,
		},
	})

	if err != nil {
		log.Print(err)
	}
}
