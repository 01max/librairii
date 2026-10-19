package platform

/*
#cgo pkg-config: gtk+-3.0
#include <gtk/gtk.h>

static GtkWidget *librairiiSettingsItem(GtkWidget *widget) {
    if (GTK_IS_MENU_BAR(widget)) {
        GList *menus = gtk_container_get_children(GTK_CONTAINER(widget));
        GtkWidget *result = NULL;
        for (GList *entry = menus; entry; entry = entry->next) {
            GtkMenuItem *menu = GTK_MENU_ITEM(entry->data);
            if (g_strcmp0(gtk_menu_item_get_label(menu), "Librairii") != 0) continue;
            GtkWidget *submenu = gtk_menu_item_get_submenu(menu);
            if (!submenu) continue;
            GList *items = gtk_container_get_children(GTK_CONTAINER(submenu));
            if (items && GTK_IS_MENU_ITEM(items->data)) result = items->data;
            g_list_free(items);
            break;
        }
        g_list_free(menus);
        return result;
    }
    if (!GTK_IS_CONTAINER(widget)) return NULL;
    GList *children = gtk_container_get_children(GTK_CONTAINER(widget));
    GtkWidget *result = NULL;
    for (GList *entry = children; entry && !result; entry = entry->next) {
        result = librairiiSettingsItem(GTK_WIDGET(entry->data));
    }
    g_list_free(children);
    return result;
}

static gboolean librairiiUpdateSettingsLabel(gpointer label) {
    GList *windows = gtk_window_list_toplevels();
    for (GList *entry = windows; entry; entry = entry->next) {
        GtkWidget *item = librairiiSettingsItem(GTK_WIDGET(entry->data));
        if (item) gtk_menu_item_set_label(GTK_MENU_ITEM(item), label);
    }
    g_list_free(windows);
    return G_SOURCE_REMOVE;
}

static void librairiiScheduleSettingsLabel(const char *label) {
    g_idle_add_full(G_PRIORITY_DEFAULT_IDLE, librairiiUpdateSettingsLabel,
                   g_strdup(label), g_free);
}
*/
import "C"
import "unsafe"

// IntegrateSettingsMenu updates the attached menu on the GTK thread, preserving
// Wails' original callback mappings and Control-comma accelerator.
func IntegrateSettingsMenu(_ string, label string) {
	value := C.CString(label)
	defer C.free(unsafe.Pointer(value))
	C.librairiiScheduleSettingsLabel(value)
}
