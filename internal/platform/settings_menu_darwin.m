#import <Cocoa/Cocoa.h>

void LibrairiiIntegrateSettingsMenu(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        NSMenu *main = [NSApp mainMenu];
        NSMenuItem *bridge = [main itemWithTitle:@"Librairii Settings"];
        if (!bridge || !bridge.submenu || bridge.submenu.numberOfItems == 0 || main.numberOfItems == 0) return;
        NSMenuItem *command = [[bridge.submenu itemAtIndex:0] retain];
        [bridge.submenu removeItem:command];
        NSMenu *application = [main itemAtIndex:0].submenu;
        [application insertItem:command atIndex:MIN(2, application.numberOfItems)];
        [command release];
        [main removeItem:bridge];
    });
}
