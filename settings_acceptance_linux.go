package main

/*
#cgo pkg-config: gtk+-3.0
#include <gtk/gtk.h>
#include <gdk/gdkkeysyms.h>
static volatile gint settingsResult=0;
static gboolean librairiiHasSettingsLabel(GtkWidget *widget, const char *label) {
 if(GTK_IS_MENU_ITEM(widget)) {
  if(g_strcmp0(gtk_menu_item_get_label(GTK_MENU_ITEM(widget)),label)==0)return TRUE;
  GtkWidget *submenu=gtk_menu_item_get_submenu(GTK_MENU_ITEM(widget));
  if(submenu && librairiiHasSettingsLabel(submenu,label))return TRUE;
 }
 if(!GTK_IS_CONTAINER(widget))return FALSE;
 GList *children=gtk_container_get_children(GTK_CONTAINER(widget));gboolean found=FALSE;
 for(GList *entry=children;entry && !found;entry=entry->next)found=librairiiHasSettingsLabel(GTK_WIDGET(entry->data),label);
 g_list_free(children);return found;
}
static gboolean librairiiInvokeSettings(gpointer label){
 gboolean found=FALSE;GList *windows=gtk_window_list_toplevels();
 for(GList *current=windows;current;current=current->next){
  if(librairiiHasSettingsLabel(GTK_WIDGET(current->data),label) && gtk_accel_groups_activate(G_OBJECT(current->data),GDK_KEY_comma,GDK_CONTROL_MASK)){found=TRUE;break;}
 }
 g_list_free(windows);g_atomic_int_set(&settingsResult,found?2:1);return G_SOURCE_REMOVE;
}
static void librairiiRequestSettings(const char *label){g_atomic_int_set(&settingsResult,0);g_idle_add_full(G_PRIORITY_DEFAULT_IDLE,librairiiInvokeSettings,g_strdup(label),g_free);}
static int librairiiSettingsResult(void){return g_atomic_int_get(&settingsResult);}
*/
import "C"
import (
	"context"
	"errors"
	"time"
	"unsafe"
)

func automateHostSettingsMenu(ctx context.Context, label string) error {
	value := C.CString(label)
	defer C.free(unsafe.Pointer(value))
	C.librairiiRequestSettings(value)
	timer := time.NewTimer(5 * time.Second)
	defer timer.Stop()
	ticker := time.NewTicker(20 * time.Millisecond)
	defer ticker.Stop()
	for {
		switch C.librairiiSettingsResult() {
		case 1:
			return errors.New("localized native Settings menu or accelerator missing")
		case 2:
			return nil
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-timer.C:
			return errors.New("native Settings accelerator timed out")
		case <-ticker.C:
		}
	}
}
