package main

import (
	"context"
	"runtime"

	"github.com/01max/librairii/internal/localization"
	"github.com/01max/librairii/internal/platform"
	"github.com/wailsapp/wails/v2/pkg/menu"
	"github.com/wailsapp/wails/v2/pkg/menu/keys"
	wruntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

const settingsOpenEvent = "settings:open"

func settingsMenu(a *App) *menu.Menu {
	result := menu.NewMenu()
	if runtime.GOOS == "darwin" {
		result.Append(menu.AppMenu())
	}
	submenu := result.AddSubmenu("Librairii Settings")
	language := a.Settings().Language
	submenu.AddText(localization.Text(language, "Settings…"), keys.CmdOrCtrl(","), func(*menu.CallbackData) {
		if a.settings == nil {
			return
		}
		a.settings.mu.Lock()
		a.settings.pendingOpen = true
		a.settings.mu.Unlock()
		wruntime.EventsEmit(a.runtimeContext(), settingsOpenEvent)
	})
	if runtime.GOOS == "darwin" {
		result.Append(menu.EditMenu())
		result.Append(menu.WindowMenu())
	}
	return result
}
func (a *App) refreshSettingsMenu(ctx context.Context) {
	wruntime.MenuSetApplicationMenu(ctx, settingsMenu(a))
	platform.IntegrateSettingsMenu()
}
