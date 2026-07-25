package platform

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"runtime"

	"github.com/01max/librairii/internal/exporter"
)

var ErrUnsupportedDesktop = errors.New("desktop file manager is unsupported")

type commandRunner func(context.Context, string, ...string) error

type DestinationRevealer struct {
	osName string
	run    commandRunner
}

func NewDestinationRevealer() *DestinationRevealer {
	return &DestinationRevealer{
		osName: runtime.GOOS,
		run: func(ctx context.Context, name string, args ...string) error {
			return exec.CommandContext(ctx, name, args...).Run()
		},
	}
}

func (r *DestinationRevealer) Reveal(
	ctx context.Context,
	destination string,
) error {
	resolved, err := exporter.ResolveDestination(destination)
	if err != nil {
		return err
	}
	command, args, err := revealCommand(r.osName, resolved)
	if err != nil {
		return err
	}
	if err := r.run(ctx, command, args...); err != nil {
		return fmt.Errorf("reveal export destination: %w", err)
	}
	return nil
}

func revealCommand(osName string, destination string) (string, []string, error) {
	switch osName {
	case "darwin":
		return "open", []string{destination}, nil
	case "windows":
		return "explorer.exe", []string{destination}, nil
	case "linux":
		return "xdg-open", []string{destination}, nil
	default:
		return "", nil, ErrUnsupportedDesktop
	}
}
