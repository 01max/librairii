package main

/*
#cgo pkg-config: gtk+-3.0
#include <gtk/gtk.h>
#include <gdk/gdkkeysyms.h>
static volatile gint settingsResult=0;
static gboolean librairiiInvokeSettings(gpointer unused){
 gboolean found=FALSE;GList *windows=gtk_window_list_toplevels();
 for(GList *current=windows;current;current=current->next){
  if(gtk_accel_groups_activate(G_OBJECT(current->data),GDK_KEY_comma,GDK_CONTROL_MASK)){found=TRUE;break;}
 }
 g_list_free(windows);g_atomic_int_set(&settingsResult,found?2:1);return G_SOURCE_REMOVE;
}
static void librairiiRequestSettings(void){g_atomic_int_set(&settingsResult,0);g_idle_add(librairiiInvokeSettings,NULL);}
static int librairiiSettingsResult(void){return g_atomic_int_get(&settingsResult);}
*/
import "C"
import (
	"context"
	"errors"
	"time"
)

func automateHostSettingsMenu(ctx context.Context) error {
	C.librairiiRequestSettings()
	timer := time.NewTimer(5 * time.Second)
	defer timer.Stop()
	ticker := time.NewTicker(20 * time.Millisecond)
	defer ticker.Stop()
	for {
		switch C.librairiiSettingsResult() {
		case 1:
			return errors.New("native Settings accelerator missing")
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
