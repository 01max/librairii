package main

import (
	"context"

	coreapp "github.com/01max/librairii/internal/app"
)

// App is the narrow Wails binding facade. Domain behaviour stays in internal
// application services and only stable DTOs cross this boundary.
type App struct {
	core *coreapp.Application
	quit func(context.Context)
}

type AppOption func(*App)

func WithQuitAfterDOMReady(quit func(context.Context)) AppOption {
	return func(app *App) {
		app.quit = quit
	}
}

func NewApp(core *coreapp.Application, options ...AppOption) *App {
	app := &App{core: core}
	for _, option := range options {
		option(app)
	}
	return app
}

func (a *App) startup(ctx context.Context) {
	_ = a.core.Start(ctx)
}

func (a *App) domReady(ctx context.Context) {
	a.core.AnnounceReady(ctx)
	if a.quit != nil {
		a.quit(ctx)
	}
}

func (a *App) shutdown(ctx context.Context) {
	a.core.Stop(ctx)
}

// ApplicationStatus returns a stable lifecycle snapshot for the frontend.
func (a *App) ApplicationStatus() coreapp.StatusResponse {
	return a.core.StatusResponse()
}
