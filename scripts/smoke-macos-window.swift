import AppKit
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 2 else {
    fputs("Usage: smoke-macos-window <bundle-id>\n", stderr)
    exit(64)
}

let bundleIdentifier = CommandLine.arguments[1]
guard let app = NSRunningApplication.runningApplications(
    withBundleIdentifier: bundleIdentifier
).first(where: { !$0.isTerminated }) else {
    fputs("No running application found for bundle ID \(bundleIdentifier).\n", stderr)
    exit(2)
}

let windows = CGWindowListCopyWindowInfo(
    [.optionOnScreenOnly, .excludeDesktopElements],
    kCGNullWindowID
) as? [[String: Any]] ?? []

let hasVisibleWindow = windows.contains { window in
    guard
        let ownerPID = (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value,
        ownerPID == app.processIdentifier,
        let layer = (window[kCGWindowLayer as String] as? NSNumber)?.intValue,
        layer == 0,
        let bounds = window[kCGWindowBounds as String] as? [String: Any],
        let width = (bounds["Width"] as? NSNumber)?.doubleValue,
        let height = (bounds["Height"] as? NSNumber)?.doubleValue
    else {
        return false
    }

    return width > 0 && height > 0
}

guard hasVisibleWindow else {
    fputs("The running app has no visible window.\n", stderr)
    exit(1)
}

print(app.processIdentifier)
