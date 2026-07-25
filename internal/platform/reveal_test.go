package platform

import (
	"context"
	"errors"
	"reflect"
	"testing"

	"github.com/01max/librairii/internal/exporter"
)

func TestDestinationRevealerUsesPlatformFileManager(t *testing.T) {
	t.Parallel()

	destination := t.TempDir()
	resolved, err := exporter.ResolveDestination(destination)
	if err != nil {
		t.Fatal(err)
	}
	cases := []struct {
		osName  string
		command string
	}{
		{osName: "darwin", command: "open"},
		{osName: "windows", command: "explorer.exe"},
		{osName: "linux", command: "xdg-open"},
	}
	for _, testCase := range cases {
		testCase := testCase
		t.Run(testCase.osName, func(t *testing.T) {
			t.Parallel()

			var gotCommand string
			var gotArgs []string
			revealer := &DestinationRevealer{
				osName: testCase.osName,
				run: func(
					_ context.Context,
					command string,
					args ...string,
				) error {
					gotCommand = command
					gotArgs = args
					return nil
				},
			}
			if err := revealer.Reveal(context.Background(), destination); err != nil {
				t.Fatal(err)
			}
			if gotCommand != testCase.command ||
				!reflect.DeepEqual(gotArgs, []string{resolved}) {
				t.Fatalf("command = %q %#v", gotCommand, gotArgs)
			}
		})
	}
}

func TestDestinationRevealerRejectsInvalidPathsAndUnsupportedSystems(t *testing.T) {
	t.Parallel()

	called := false
	revealer := &DestinationRevealer{
		osName: "darwin",
		run: func(context.Context, string, ...string) error {
			called = true
			return nil
		},
	}
	if err := revealer.Reveal(context.Background(), "relative"); err == nil {
		t.Fatal("Reveal(relative) succeeded")
	}
	if called {
		t.Fatal("file manager launched for invalid path")
	}

	revealer.osName = "plan9"
	if err := revealer.Reveal(
		context.Background(),
		t.TempDir(),
	); !errors.Is(err, ErrUnsupportedDesktop) {
		t.Fatalf("Reveal(unsupported) error = %v", err)
	}
	if called {
		t.Fatal("file manager launched for unsupported system")
	}
}

func TestDestinationRevealerReturnsLaunchFailure(t *testing.T) {
	t.Parallel()

	launchErr := errors.New("launch failed")
	revealer := &DestinationRevealer{
		osName: "darwin",
		run: func(context.Context, string, ...string) error {
			return launchErr
		},
	}
	if err := revealer.Reveal(
		context.Background(),
		t.TempDir(),
	); !errors.Is(err, launchErr) {
		t.Fatalf("Reveal() error = %v", err)
	}
}
