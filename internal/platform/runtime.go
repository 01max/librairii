package platform

import (
	"context"
	"path/filepath"
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
	revealer      *DestinationRevealer

	acceptanceFile      string
	acceptanceDirectory string
}

// ConfigureAcceptanceSelections gives the packaged release gate deterministic
// initial locations while preserving the real host-native dialog boundary.
func (d *RuntimeDialogs) ConfigureAcceptanceSelections(
	file string,
	directory string,
) {
	d.acceptanceFile = file
	d.acceptanceDirectory = directory
}

func NewRuntimeDialogs() *RuntimeDialogs {
	return &RuntimeDialogs{
		openFile:      runtime.OpenFileDialog,
		openFiles:     runtime.OpenMultipleFilesDialog,
		openDirectory: runtime.OpenDirectoryDialog,
		revealer:      NewDestinationRevealer(),
	}
}

func (d *RuntimeDialogs) RevealDirectory(
	ctx context.Context,
	destination string,
) error {
	if d.revealer == nil {
		return ErrUnsupportedDesktop
	}
	return d.revealer.Reveal(ctx, destination)
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
	if d.acceptanceFile != "" {
		options.DefaultDirectory = filepath.Dir(d.acceptanceFile)
		options.DefaultFilename = filepath.Base(d.acceptanceFile)
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
	options := runtime.OpenDialogOptions{
		Title:                title,
		CanCreateDirectories: true,
		ResolvesAliases:      true,
	}
	if d.acceptanceDirectory != "" {
		options.DefaultDirectory = d.acceptanceDirectory
	}
	path, err := d.openDirectory(ctx, options)
	if err != nil || path == "" {
		return path, err
	}
	return exporter.ResolveDestination(path)
}

func extensionPattern(extensions []string) string {
	patterns := make([]string, 0, len(extensions))
	seen := make(map[string]struct{}, len(extensions))
	for _, extension := range extensions {
		extension = strings.TrimSpace(extension)
		if extension == "" || strings.ContainsAny(extension, "*;") {
			continue
		}
		if !strings.HasPrefix(extension, ".") {
			extension = "." + extension
		}
		extension = filepath.Ext(extension)
		if extension == "" {
			continue
		}
		pattern := "*" + extension
		normalized := strings.ToLower(pattern)
		if _, exists := seen[normalized]; exists {
			continue
		}
		seen[normalized] = struct{}{}
		patterns = append(patterns, pattern)
	}
	return strings.Join(patterns, ";")
}
