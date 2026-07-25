package main

import (
	"embed"
	"log"
	"os"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/artwork"
	"github.com/01max/librairii/internal/lunii"
	"github.com/01max/librairii/internal/platform"
	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/runtime"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
	readiness := platform.NewStorageReadiness(os.Getenv("LIBRAIRII_DATA_ROOT"))
	catalogClient, err := lunii.NewCatalogClient(lunii.ProductionConfig())
	if err != nil {
		log.Fatal(err)
	}
	artworkHandler, err := artwork.NewAssetHandler(readiness, catalogClient)
	if err != nil {
		log.Fatal(err)
	}
	clock := platform.SystemClock{}
	events := platform.RuntimeEvents{}
	importRuntime, err := coreapp.NewImportRuntime(
		readiness,
		clock,
		events,
		2,
		coreapp.WithMetadataFetcher(catalogClient),
	)
	if err != nil {
		log.Fatal(err)
	}
	core, err := coreapp.New(coreapp.Dependencies{
		Clock:       clock,
		Dialogs:     platform.NewRuntimeDialogs(),
		Events:      events,
		Readiness:   readiness,
		Operations:  importRuntime,
		Library:     importRuntime,
		Removal:     importRuntime,
		Tags:        importRuntime,
		Shelves:     importRuntime,
		Diagnostics: importRuntime,
		Resources:   []coreapp.ResourcePort{readiness},
	})
	if err != nil {
		log.Fatal(err)
	}
	appOptions := make([]AppOption, 0, 1)
	if os.Getenv("LIBRAIRII_SMOKE_EXIT") == "1" {
		appOptions = append(appOptions, WithQuitAfterDOMReady(runtime.Quit))
	}
	app := NewApp(core, appOptions...)

	appOptionsConfig, err := productionOptions(app, artworkHandler, assets)
	if err != nil {
		log.Fatal(err)
	}
	err = wails.Run(appOptionsConfig)

	if err != nil {
		log.Print(err)
	}
}
