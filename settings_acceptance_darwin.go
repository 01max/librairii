package main

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework Cocoa
#import <Cocoa/Cocoa.h>
static int librairiiInvokeSettings(const char *label) {
 __block int found=0;
 dispatch_sync(dispatch_get_main_queue(), ^{
  NSMenu *main=[NSApp mainMenu];if(main.numberOfItems==0)return;
  NSMenu *app=[main itemAtIndex:0].submenu;
  for(NSInteger i=0;i<app.numberOfItems;i++){
   NSMenuItem *item=[app itemAtIndex:i];
   if([item.keyEquivalent isEqualToString:@","] && (item.keyEquivalentModifierMask & NSEventModifierFlagCommand)){
    if(![item.title isEqualToString:[NSString stringWithUTF8String:label]])break;
    [app performActionForItemAtIndex:i];found=1;break;
   }
  }
 });return found;
}
*/
import "C"
import (
	"context"
	"errors"
	"unsafe"
)

func automateHostSettingsMenu(_ context.Context, label string) error {
	value := C.CString(label)
	defer C.free(unsafe.Pointer(value))
	if C.librairiiInvokeSettings(value) != 1 {
		return errors.New("localized native Settings menu command or Command-comma shortcut missing")
	}
	return nil
}
