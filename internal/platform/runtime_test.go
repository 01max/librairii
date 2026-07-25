package platform

import (
	"context"
	"os/exec"
	"reflect"
	goruntime "runtime"
	"testing"

	coreapp "github.com/01max/librairii/internal/app"
	"github.com/01max/librairii/internal/exporter"
	"github.com/wailsapp/wails/v2/pkg/runtime"
)

func TestProductionRuntimeDialogsUseHostNativeAdapters(t *testing.T) {
	t.Parallel()

	dialogs := NewRuntimeDialogs()
	if dialogs.openFile == nil ||
		dialogs.openFiles == nil ||
		dialogs.openDirectory == nil ||
		dialogs.revealer == nil {
		t.Fatal("NewRuntimeDialogs() omitted a production native adapter")
	}
	if dialogs.revealer.osName != goruntime.GOOS || dialogs.revealer.run == nil {
		t.Fatalf(
			"production revealer = %#v, GOOS = %q",
			dialogs.revealer,
			goruntime.GOOS,
		)
	}
	command, _, err := revealCommand(goruntime.GOOS, t.TempDir())
	if err != nil {
		t.Fatalf("revealCommand(%q) error = %v", goruntime.GOOS, err)
	}
	if _, err := exec.LookPath(command); err != nil {
		t.Fatalf("host-native reveal command %q is unavailable: %v", command, err)
	}
}

func TestRuntimeDialogsUseNativeMultiFilePicker(t *testing.T) {
	t.Parallel()

	var got runtime.OpenDialogOptions
	dialogs := &RuntimeDialogs{
		openFile: func(context.Context, runtime.OpenDialogOptions) (string, error) {
			t.Fatal("single-file dialog called")
			return "", nil
		},
		openFiles: func(
			_ context.Context,
			options runtime.OpenDialogOptions,
		) ([]string, error) {
			got = options
			return []string{"/native/first.zip", "/native/second.7z"}, nil
		},
		openDirectory: func(context.Context, runtime.OpenDialogOptions) (string, error) {
			return "", nil
		},
	}
	paths, err := dialogs.OpenFiles(context.Background(), coreapp.FileDialogRequest{
		Title:      "Import story archives",
		Extensions: []string{".plain.pk", ".v1.pk", ".v2.pk", ".pk", ".zip", ".7z"},
		Multiple:   true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(paths, []string{"/native/first.zip", "/native/second.7z"}) {
		t.Fatalf("OpenFiles() = %#v", paths)
	}
	if got.Title != "Import story archives" ||
		len(got.Filters) != 1 ||
		got.Filters[0].Pattern != "*.plain.pk;*.v1.pk;*.v2.pk;*.pk;*.zip;*.7z" ||
		!got.ResolvesAliases {
		t.Fatalf("native options = %#v", got)
	}
}

func TestRuntimeDialogsSupportSingleFileAndDirectory(t *testing.T) {
	t.Parallel()

	destination := t.TempDir()
	dialogs := &RuntimeDialogs{
		openFile: func(
			context.Context,
			runtime.OpenDialogOptions,
		) (string, error) {
			return "/native/story.zip", nil
		},
		openFiles: func(
			context.Context,
			runtime.OpenDialogOptions,
		) ([]string, error) {
			t.Fatal("multi-file dialog called")
			return nil, nil
		},
		openDirectory: func(
			_ context.Context,
			options runtime.OpenDialogOptions,
		) (string, error) {
			if !options.CanCreateDirectories || !options.ResolvesAliases {
				t.Fatalf("directory options = %#v", options)
			}
			return destination, nil
		},
	}
	paths, err := dialogs.OpenFiles(context.Background(), coreapp.FileDialogRequest{
		Extensions: []string{"zip", "*", "bad;pattern"},
	})
	if err != nil || !reflect.DeepEqual(paths, []string{"/native/story.zip"}) {
		t.Fatalf("OpenFiles(single) = %#v, %v", paths, err)
	}
	directory, err := dialogs.OpenDirectory(context.Background(), "Export stories")
	expected, resolveErr := exporter.ResolveDestination(destination)
	if resolveErr != nil {
		t.Fatal(resolveErr)
	}
	if err != nil || directory != expected {
		t.Fatalf("OpenDirectory() = %q, %v", directory, err)
	}
}

func TestRuntimeDialogsRevealOnlyValidatedDirectory(t *testing.T) {
	t.Parallel()

	destination := t.TempDir()
	var revealed string
	dialogs := &RuntimeDialogs{
		revealer: &DestinationRevealer{
			osName: "darwin",
			run: func(_ context.Context, _ string, args ...string) error {
				revealed = args[0]
				return nil
			},
		},
	}
	if err := dialogs.RevealDirectory(
		context.Background(),
		destination,
	); err != nil {
		t.Fatal(err)
	}
	expected, err := exporter.ResolveDestination(destination)
	if err != nil {
		t.Fatal(err)
	}
	if revealed != expected {
		t.Fatalf("revealed destination = %q, want %q", revealed, expected)
	}
}
