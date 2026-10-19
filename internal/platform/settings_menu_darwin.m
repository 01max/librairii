#import <Cocoa/Cocoa.h>

@interface LibrairiiAboutTarget : NSObject
@property(nonatomic, retain) NSString *descriptionText;
- (void)showAbout:(id)sender;
@end
@implementation LibrairiiAboutTarget
- (void)showAbout:(id)sender {
    NSAttributedString *credits = [[[NSAttributedString alloc] initWithString:self.descriptionText] autorelease];
    [NSApp orderFrontStandardAboutPanelWithOptions:@{NSAboutPanelOptionCredits:credits}];
}
@end

void LibrairiiIntegrateSettingsMenu(const char *description) {
    NSString *descriptionText = [[NSString alloc] initWithUTF8String:description];
    dispatch_async(dispatch_get_main_queue(), ^{
        NSMenu *main = [NSApp mainMenu];
        NSMenuItem *bridge = [main itemWithTitle:@"Librairii Settings"];
        if (!bridge || !bridge.submenu || bridge.submenu.numberOfItems == 0 || main.numberOfItems == 0) { [descriptionText release]; return; }
        NSMenuItem *command = [[bridge.submenu itemAtIndex:0] retain];
        [bridge.submenu removeItem:command];
        NSMenu *application = [main itemAtIndex:0].submenu;
        static LibrairiiAboutTarget *aboutTarget = nil;
        if (!aboutTarget) aboutTarget = [LibrairiiAboutTarget new];
        aboutTarget.descriptionText = descriptionText;
        NSMenuItem *about = [application itemAtIndex:0];
        about.target = aboutTarget;
        about.action = @selector(showAbout:);
        [application insertItem:command atIndex:MIN(2, application.numberOfItems)];
        [command release];
        [main removeItem:bridge];
        [descriptionText release];
    });
}
