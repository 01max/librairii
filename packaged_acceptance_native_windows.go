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
	keyboardInput     = 1
	keyEventKeyUp     = 0x0002
	keyEventUnicode   = 0x0004
	virtualKeyControl = 0x11
	virtualKeyMenu    = 0x12
	virtualKeyReturn  = 0x0d
)

var (
	user32              = windows.NewLazySystemDLL("user32.dll")
	findWindow          = user32.NewProc("FindWindowW")
	setForegroundWindow = user32.NewProc("SetForegroundWindow")
	sendInput           = user32.NewProc("SendInput")
)

type keyboardInputEvent struct {
	virtualKey uint16
	scanCode   uint16
	flags      uint32
	time       uint32
	extraInfo  uintptr
}

type windowsInput struct {
	inputType uint32
	_         uint32
	keyboard  keyboardInputEvent
	_         [8]byte
}

func automateHostNativeDialog(
	ctx context.Context,
	title string,
	path string,
	kind nativeDialogKind,
) error {
	handle, err := waitForNativeDialog(ctx, title)
	if err != nil {
		return err
	}
	if activated, _, callErr := setForegroundWindow.Call(handle); activated == 0 {
		return fmt.Errorf("activate Windows native dialog: %w", callErr)
	}
	time.Sleep(200 * time.Millisecond)

	if kind == nativeFileDialog {
		if err := sendHotkey(virtualKeyMenu, 'N'); err != nil {
			return err
		}
	} else {
		if err := sendHotkey(virtualKeyControl, 'L'); err != nil {
			return err
		}
	}
	if err := sendText(path); err != nil {
		return err
	}
	if err := sendKey(virtualKeyReturn); err != nil {
		return err
	}
	if kind == nativeDirectoryDialog {
		time.Sleep(350 * time.Millisecond)
		if err := sendHotkey(virtualKeyMenu, 'S'); err != nil {
			return err
		}
	}
	return nil
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

func sendHotkey(modifier uint16, key uint16) error {
	if err := sendKeyboardInput(modifier, 0); err != nil {
		return err
	}
	if err := sendKeyboardInput(key, 0); err != nil {
		return err
	}
	if err := sendKeyboardInput(key, keyEventKeyUp); err != nil {
		return err
	}
	return sendKeyboardInput(modifier, keyEventKeyUp)
}

func sendKey(key uint16) error {
	if err := sendKeyboardInput(key, 0); err != nil {
		return err
	}
	return sendKeyboardInput(key, keyEventKeyUp)
}

func sendText(value string) error {
	characters, err := windows.UTF16FromString(value)
	if err != nil {
		return err
	}
	for _, character := range characters {
		if character == 0 {
			continue
		}
		if err := sendKeyboardInput(0, keyEventUnicode, character); err != nil {
			return err
		}
		if err := sendKeyboardInput(
			0,
			keyEventUnicode|keyEventKeyUp,
			character,
		); err != nil {
			return err
		}
	}
	return nil
}

func sendKeyboardInput(
	virtualKey uint16,
	flags uint32,
	scanCode ...uint16,
) error {
	event := windowsInput{
		inputType: keyboardInput,
		keyboard: keyboardInputEvent{
			virtualKey: virtualKey,
			flags:      flags,
		},
	}
	if len(scanCode) == 1 {
		event.keyboard.scanCode = scanCode[0]
	}
	inserted, _, callErr := sendInput.Call(
		1,
		uintptr(unsafe.Pointer(&event)),
		unsafe.Sizeof(event),
	)
	if inserted != 1 {
		return fmt.Errorf("send Windows native dialog input: %w", callErr)
	}
	return nil
}
