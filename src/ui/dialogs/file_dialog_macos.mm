#import <Cocoa/Cocoa.h>

#include <optional>
#include <string>

std::optional<std::string> macosOpenFile(const char* filter, const char* initialDir) {
    @autoreleasepool {
        NSOpenPanel* panel = [NSOpenPanel openPanel];
        [panel setCanChooseFiles:YES];
        [panel setCanChooseDirectories:NO];
        [panel setAllowsMultipleSelection:NO];

        // Set initial directory if provided.
        if (initialDir && strlen(initialDir) > 0) {
            NSString* dirPath = [NSString stringWithUTF8String:initialDir];
            NSURL* dirURL = [NSURL fileURLWithPath:dirPath isDirectory:YES];
            [panel setDirectoryURL:dirURL];
        }

        // Parse filter and set allowed file types.
        // Filter format: "*.sox" or similar.
        if (filter && strlen(filter) > 0) {
            NSString* filterStr = [NSString stringWithUTF8String:filter];
            NSArray* parts = [filterStr componentsSeparatedByString:@"*."];
            NSMutableArray* extensions = [NSMutableArray array];
            for (NSString* part in parts) {
                NSString* trimmed = [part stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceCharacterSet]];
                if ([trimmed length] > 0) {
                    [extensions addObject:trimmed];
                }
            }
            if ([extensions count] > 0) {
                [panel setAllowedFileTypes:extensions];
            }
        }

        if ([panel runModal] == NSModalResponseOK) {
            NSURL* url = [[panel URLs] firstObject];
            if (url) {
                return std::string([[url path] UTF8String]);
            }
        }
    }
    return std::nullopt;
}

std::optional<std::string> macosSaveFile(const char* filter, const char* defaultName) {
    @autoreleasepool {
        NSSavePanel* panel = [NSSavePanel savePanel];

        if (defaultName) {
            [panel setNameFieldStringValue:[NSString stringWithUTF8String:defaultName]];
        }

        // Parse filter and set allowed file types.
        if (filter && strlen(filter) > 0) {
            NSString* filterStr = [NSString stringWithUTF8String:filter];
            NSArray* parts = [filterStr componentsSeparatedByString:@"*."];
            NSMutableArray* extensions = [NSMutableArray array];
            for (NSString* part in parts) {
                NSString* trimmed = [part stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceCharacterSet]];
                if ([trimmed length] > 0) {
                    [extensions addObject:trimmed];
                }
            }
            if ([extensions count] > 0) {
                [panel setAllowedFileTypes:extensions];
            }
        }

        if ([panel runModal] == NSModalResponseOK) {
            NSURL* url = [panel URL];
            if (url) {
                return std::string([[url path] UTF8String]);
            }
        }
    }
    return std::nullopt;
}

std::optional<std::string> macosOpenFolder() {
    @autoreleasepool {
        NSOpenPanel* panel = [NSOpenPanel openPanel];
        [panel setCanChooseFiles:NO];
        [panel setCanChooseDirectories:YES];
        [panel setAllowsMultipleSelection:NO];
        [panel setMessage:@"Select the game's SOX folder"];

        if ([panel runModal] == NSModalResponseOK) {
            NSURL* url = [[panel URLs] firstObject];
            if (url) {
                return std::string([[url path] UTF8String]);
            }
        }
    }
    return std::nullopt;
}
