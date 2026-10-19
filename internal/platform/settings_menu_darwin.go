package platform

/*
#cgo LDFLAGS: -framework Cocoa
#include <stdlib.h>
void LibrairiiIntegrateSettingsMenu(const char *description);
*/
import "C"
import "unsafe"

// IntegrateSettingsMenu retains Wails callbacks while placing the command in
// the OS application menu, whose standard roles Wails v2 otherwise owns.
func IntegrateSettingsMenu(description, _ string) {
	value := C.CString(description)
	defer C.free(unsafe.Pointer(value))
	C.LibrairiiIntegrateSettingsMenu(value)
}
