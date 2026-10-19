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
	title := "Librairii"
	if runtime.GOOS == "darwin" {
		title = "Librairii Settings"
	}
	submenu := result.AddSubmenu(title)
	language := a.interfaceLanguage()
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
	// Wails' Linux replacement leaves the old accelerator attached while
	// discarding its callback mapping. Keep that menu and update its label.
	if runtime.GOOS == "darwin" {
		wruntime.MenuSetApplicationMenu(ctx, settingsMenu(a))
	}
	language := a.interfaceLanguage()
	description := localization.Text(language, "A local-first story archive library for Lunii.QT-compatible collections.") + "\n\n" + localization.Text(language, "Contributors") + ": https://github.com/01max/librairii/graphs/contributors\n\n" + localization.Text(language, "Repository") + ": https://github.com/01max/librairii"
	platform.IntegrateSettingsMenu(description, localization.Text(language, "Settings…"))
}

func (a *App) interfaceLanguage() string {
	if a.settings == nil {
		return "en"
	}
	return a.settings.storage.Profile().Language
}
