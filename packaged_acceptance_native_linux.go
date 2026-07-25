//go:build linux

package main

/*
#cgo pkg-config: gtk+-3.0

#include <stdlib.h>
#include <gtk/gtk.h>

typedef struct {
    char *path;
    int attempts;
} LibrairiiDialogSelection;

static gboolean librairiiAcceptNativeDialog(gpointer userData) {
    LibrairiiDialogSelection *selection = userData;
    selection->attempts++;
    if (selection->attempts >= 300) {
        free(selection->path);
        free(selection);
        return G_SOURCE_REMOVE;
    }

    GList *windows = gtk_window_list_toplevels();
    for (GList *item = windows; item != NULL; item = item->next) {
        GtkWidget *window = GTK_WIDGET(item->data);
        if (!GTK_IS_FILE_CHOOSER(window)) {
            continue;
        }
        GtkFileChooser *chooser = GTK_FILE_CHOOSER(window);
        gboolean selected = gtk_file_chooser_select_filename(
            chooser,
            selection->path
        );
        g_list_free(windows);
        if (!selected) {
            return G_SOURCE_CONTINUE;
        }
        gtk_dialog_response(GTK_DIALOG(window), GTK_RESPONSE_ACCEPT);
        free(selection->path);
        free(selection);
        return G_SOURCE_REMOVE;
    }
    g_list_free(windows);

    return G_SOURCE_CONTINUE;
}

static void librairiiScheduleNativeDialog(const char *path) {
    LibrairiiDialogSelection *selection = calloc(
        1,
        sizeof(LibrairiiDialogSelection)
    );
    selection->path = strdup(path);
    g_timeout_add(50, librairiiAcceptNativeDialog, selection);
}
*/
import "C"

import (
	"context"
	"errors"
	"unsafe"
)

func automateHostNativeDialog(
	ctx context.Context,
	_ string,
	path string,
	_ nativeDialogKind,
) error {
	select {
	case <-ctx.Done():
		return errors.New("Linux native dialog automation timed out")
	default:
	}
	selection := C.CString(path)
	defer C.free(unsafe.Pointer(selection))
	C.librairiiScheduleNativeDialog(selection)
	return nil
}
