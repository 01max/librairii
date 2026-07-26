//go:build windows

package main

import (
	"context"
	"errors"
	"fmt"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
)

const (
	windowMessageClose   = 0x0010
	windowMessageCommand = 0x0111
	dialogCommandOK      = 1
	dialogCommandCancel  = 2
)

var (
	user32      = windows.NewLazySystemDLL("user32.dll")
	findWindow  = user32.NewProc("FindWindowW")
	isWindow    = user32.NewProc("IsWindow")
	postMessage = user32.NewProc("PostMessageW")
)

func automateHostNativeDialog(
	ctx context.Context,
	title string,
	_ string,
	_ nativeDialogKind,
) error {
	handle, err := waitForNativeDialog(ctx, title)
	if err != nil {
		return err
	}

	// The runtime dialog is already configured with the exact acceptance file
	// or directory. Activate its default action directly so the release gate
	// does not depend on foreground focus or synthetic keyboard input.
	ticker := time.NewTicker(100 * time.Millisecond)
	defer ticker.Stop()
	attempt := 0
	for {
		if attempt%5 == 0 {
			if err := postDialogCommand(handle, dialogCommandOK); err != nil {
				return err
			}
		}
		exists, _, _ := isWindow.Call(handle)
		if exists == 0 {
			return nil
		}
		attempt++
		select {
		case <-ctx.Done():
			_ = postDialogCommand(handle, dialogCommandCancel)
			_, _, _ = postMessage.Call(handle, windowMessageClose, 0, 0)
			return fmt.Errorf(
				"Windows native dialog automation timed out: %w",
				ctx.Err(),
			)
		case <-ticker.C:
		}
	}
}

func waitForNativeDialog(ctx context.Context, title string) (uintptr, error) {
	titleUTF16, err := windows.UTF16PtrFromString(title)
	if err != nil {
		return 0, err
	}
	ticker := time.NewTicker(50 * time.Millisecond)
	defer ticker.Stop()
	for {
		handle, _, _ := findWindow.Call(0, uintptr(unsafe.Pointer(titleUTF16)))
		if handle != 0 {
			return handle, nil
		}
		select {
		case <-ctx.Done():
			return 0, errors.New("Windows native dialog automation timed out")
		case <-ticker.C:
		}
	}
}

func postDialogCommand(handle uintptr, command uintptr) error {
	posted, _, callErr := postMessage.Call(
		handle,
		windowMessageCommand,
		command,
		0,
	)
	if posted == 0 {
		return fmt.Errorf("activate Windows native dialog command: %w", callErr)
	}
	return nil
}
