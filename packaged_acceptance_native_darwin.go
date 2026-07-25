//go:build darwin

package main

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework Cocoa

#include <stdlib.h>
#import <Cocoa/Cocoa.h>

static NSOpenPanel *librairiiOpenPanel(void) {
    for (NSWindow *window in [NSApp windows]) {
        if ([window isKindOfClass:[NSOpenPanel class]] &&
            [window isVisible] &&
            [window sheetParent] != nil) {
            return (NSOpenPanel *)window;
        }
        NSWindow *sheet = [window attachedSheet];
        if ([sheet isKindOfClass:[NSOpenPanel class]] &&
            [sheet isVisible]) {
            return (NSOpenPanel *)sheet;
        }
    }
    return nil;
}

static int librairiiAcceptNativeDialog(const char *path) {
    __block int accepted = 0;
    void (^accept)(void) = ^{
        NSOpenPanel *panel = librairiiOpenPanel();
        if (panel == nil) {
            return;
        }
        NSString *selection = [NSString stringWithUTF8String:path];
        NSURL *url = [NSURL fileURLWithPath:selection];
        [panel setDirectoryURL:[url URLByDeletingLastPathComponent]];
        [panel setNameFieldStringValue:[url lastPathComponent]];
        [panel makeKeyAndOrderFront:nil];
        SEL complete = NSSelectorFromString(
            @"completeWithReturnCode:url:urls:"
        );
        if ([panel respondsToSelector:complete]) {
            typedef void (*CompleteDialog)(
                id,
                SEL,
                NSInteger,
                NSURL *,
                NSArray *
            );
            CompleteDialog completeDialog = (CompleteDialog)
                [panel methodForSelector:complete];
            completeDialog(
                panel,
                complete,
                NSModalResponseOK,
                url,
                @[url]
            );
            accepted = 2;
        }
    };

    if ([NSThread isMainThread]) {
        accept();
    } else {
        dispatch_sync(dispatch_get_main_queue(), accept);
    }
    return accepted;
}
*/
import "C"

import (
	"context"
	"errors"
	"time"
	"unsafe"
)

func automateHostNativeDialog(
	ctx context.Context,
	_ string,
	path string,
	_ nativeDialogKind,
) error {
	selection := C.CString(path)
	defer C.free(unsafe.Pointer(selection))

	ticker := time.NewTicker(50 * time.Millisecond)
	defer ticker.Stop()
	seen := false
	for {
		switch C.librairiiAcceptNativeDialog(selection) {
		case 0:
			if seen {
				return nil
			}
		case 2:
			seen = true
		default:
			return nil
		}
		select {
		case <-ctx.Done():
			return errors.New("macOS native dialog automation timed out")
		case <-ticker.C:
		}
	}
}
