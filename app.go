package main

import (
	"context"

	coreapp "github.com/01max/librairii/internal/app"
)

// App is the narrow Wails binding facade. Domain behaviour stays in internal
// application services and only stable DTOs cross this boundary.
type App struct {
	core *coreapp.Application
}

func NewApp(core *coreapp.Application) *App {
	return &App{core: core}
}

func (a *App) startup(ctx context.Context) {
	_ = a.core.Start(ctx)
}

func (a *App) domReady(ctx context.Context) {
	a.core.AnnounceReady(ctx)
}

func (a *App) shutdown(ctx context.Context) {
	a.core.Stop(ctx)
}

// ApplicationStatus returns a stable lifecycle snapshot for the frontend.
func (a *App) ApplicationStatus() coreapp.StatusResponse {
	return a.core.StatusResponse()
}
