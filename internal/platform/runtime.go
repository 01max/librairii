package platform

import (
	"context"
	"errors"
	"time"

	coreapp "github.com/01max/librairii/internal/app"
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

type PendingDialogs struct{}

func (PendingDialogs) OpenFiles(context.Context, coreapp.FileDialogRequest) ([]string, error) {
	return nil, errors.New("file dialog adapter is not configured")
}

func (PendingDialogs) OpenDirectory(context.Context, string) (string, error) {
	return "", errors.New("directory dialog adapter is not configured")
}
