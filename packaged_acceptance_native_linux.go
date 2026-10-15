//go:build linux

package main

/*
#cgo pkg-config: gtk+-3.0

#include <stdlib.h>
#include <gtk/gtk.h>

typedef struct {
    char *path;
    int kind;
    int attempts;
    int chooserAttempts;
    int selectedAtAttempt;
    gboolean selectionReady;
} LibrairiiDialogSelection;

enum {
    LIBRAIRII_FILE_DIALOG = 1,
    LIBRAIRII_DIRECTORY_DIALOG = 2
};

static gboolean librairiiAcceptNativeDialog(gpointer userData) {
    LibrairiiDialogSelection *selection = userData;
    selection->attempts++;
    gboolean logAcceptance = g_getenv("LIBRAIRII_ACCEPTANCE_LOG") != NULL;
    if (logAcceptance && selection->attempts == 1) {
        g_printerr(
            "packaged acceptance: waiting for Linux native dialog (%s)\n",
            selection->path
        );
    }
    if (selection->attempts >= 300) {
        if (logAcceptance) {
            g_printerr(
                "packaged acceptance: Linux native dialog did not accept %s\n",
                selection->path
            );
        }
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
        selection->chooserAttempts++;
        GtkFileChooser *chooser = GTK_FILE_CHOOSER(window);
        if (selection->selectionReady) {
            char *selectedPath = gtk_file_chooser_get_filename(chooser);
            if (
                selectedPath == NULL &&
                selection->kind == LIBRAIRII_DIRECTORY_DIALOG
            ) {
                selectedPath = gtk_file_chooser_get_current_folder(chooser);
            }
            gboolean exactSelection =
                g_strcmp0(selectedPath, selection->path) == 0;
            g_free(selectedPath);
            if (!exactSelection) {
                selection->selectionReady = FALSE;
                g_list_free(windows);
                return G_SOURCE_CONTINUE;
            }
            if (
                !gtk_widget_get_mapped(window) ||
                selection->attempts - selection->selectedAtAttempt < 2
            ) {
                g_list_free(windows);
                return G_SOURCE_CONTINUE;
            }
            if (logAcceptance) {
                g_printerr(
                    "packaged acceptance: accepting Linux native dialog (%s)\n",
                    selection->path
                );
            }
            gtk_dialog_response(GTK_DIALOG(window), GTK_RESPONSE_ACCEPT);
            g_list_free(windows);
            free(selection->path);
            free(selection);
            return G_SOURCE_REMOVE;
        }

        gboolean selected;
        if (selection->kind == LIBRAIRII_DIRECTORY_DIALOG) {
            selected = gtk_file_chooser_set_current_folder(
                chooser,
                selection->path
            );
        } else {
            selected = gtk_file_chooser_select_filename(
                chooser,
                selection->path
            );
        }
        if (
            logAcceptance &&
            (selection->chooserAttempts == 1 ||
             selection->chooserAttempts % 20 == 0)
        ) {
            g_printerr(
                "packaged acceptance: Linux chooser action=%d selected=%d path=%s\n",
                gtk_file_chooser_get_action(chooser),
                selected,
                selection->path
            );
        }
        g_list_free(windows);
        if (selected) {
            selection->selectionReady = TRUE;
            selection->selectedAtAttempt = selection->attempts;
        }
        return G_SOURCE_CONTINUE;
    }
    g_list_free(windows);

    return G_SOURCE_CONTINUE;
}

static void librairiiScheduleNativeDialog(const char *path, int kind) {
    LibrairiiDialogSelection *selection = calloc(
        1,
        sizeof(LibrairiiDialogSelection)
    );
    selection->path = strdup(path);
    selection->kind = kind;
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
	kind nativeDialogKind,
) error {
	select {
	case <-ctx.Done():
		return errors.New("Linux native dialog automation timed out")
	default:
	}
	selection := C.CString(path)
	defer C.free(unsafe.Pointer(selection))
	C.librairiiScheduleNativeDialog(selection, C.int(kind))
	return nil
}
