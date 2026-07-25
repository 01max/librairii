package platform

import (
	"context"
	"strings"
	"time"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/exporter"
	"github.com/wailsapp/wails/v2/pkg/runtime"
)

type SystemClock struct{}

func (SystemClock) Now() time.Time {
	return time.Now()
}

type RuntimeEvents struct{}

func (RuntimeEvents) Emit(ctx context.Context, name string, payload any) {
	runtime.EventsEmit(ctx, name, payload)
}

type RuntimeDialogs struct {
	openFile      func(context.Context, runtime.OpenDialogOptions) (string, error)
	openFiles     func(context.Context, runtime.OpenDialogOptions) ([]string, error)
	openDirectory func(context.Context, runtime.OpenDialogOptions) (string, error)
}

func NewRuntimeDialogs() *RuntimeDialogs {
	return &RuntimeDialogs{
		openFile:      runtime.OpenFileDialog,
		openFiles:     runtime.OpenMultipleFilesDialog,
		openDirectory: runtime.OpenDirectoryDialog,
	}
}

func (d *RuntimeDialogs) OpenFiles(
	ctx context.Context,
	request coreapp.FileDialogRequest,
) ([]string, error) {
	options := runtime.OpenDialogOptions{
		Title: request.Title,
		Filters: []runtime.FileFilter{
			{
				DisplayName: "Story archives",
				Pattern:     extensionPattern(request.Extensions),
			},
		},
		ResolvesAliases: true,
	}
	if request.Multiple {
		return d.openFiles(ctx, options)
	}
	path, err := d.openFile(ctx, options)
	if err != nil || path == "" {
		return nil, err
	}
	return []string{path}, nil
}

func (d *RuntimeDialogs) OpenDirectory(ctx context.Context, title string) (string, error) {
	path, err := d.openDirectory(ctx, runtime.OpenDialogOptions{
		Title:                title,
		CanCreateDirectories: true,
		ResolvesAliases:      true,
	})
	if err != nil || path == "" {
		return path, err
	}
	return exporter.ResolveDestination(path)
}

func extensionPattern(extensions []string) string {
	patterns := make([]string, 0, len(extensions))
	for _, extension := range extensions {
		extension = strings.TrimSpace(extension)
		if extension == "" || strings.ContainsAny(extension, "*;") {
			continue
		}
		if !strings.HasPrefix(extension, ".") {
			extension = "." + extension
		}
		patterns = append(patterns, "*"+extension)
	}
	return strings.Join(patterns, ";")
}
