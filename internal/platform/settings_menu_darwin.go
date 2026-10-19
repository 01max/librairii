package platform

/*
#cgo LDFLAGS: -framework Cocoa
void LibrairiiIntegrateSettingsMenu(void);
*/
import "C"

// IntegrateSettingsMenu retains Wails callbacks while placing the command in
// the OS application menu, whose standard roles Wails v2 otherwise owns.
func IntegrateSettingsMenu() { C.LibrairiiIntegrateSettingsMenu() }
