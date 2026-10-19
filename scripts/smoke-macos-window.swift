import AppKit
import ApplicationServices
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

let appElement = AXUIElementCreateApplication(app.processIdentifier)
AXUIElementSetMessagingTimeout(appElement, 1.0)
let expectedHeading = "BDMV Remuxer main interface"
var inspectedElements = 0

func hasRenderedHeading(_ element: AXUIElement, depth: Int = 0) -> Bool {
    inspectedElements += 1
    guard depth <= 16, inspectedElements <= 3000 else {
        return false
    }

    var roleValue: CFTypeRef?
    if AXUIElementCopyAttributeValue(
        element,
        kAXRoleAttribute as CFString,
        &roleValue
    ) == .success,
       let role = roleValue as? String,
       role == kAXHeadingRole as String {
        for attribute in [kAXTitleAttribute, kAXValueAttribute, kAXDescriptionAttribute] {
            var value: CFTypeRef?
            if AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success,
               let text = value as? String,
               text == expectedHeading {
                return true
            }
        }
    }

    var childrenValue: CFTypeRef?
    guard AXUIElementCopyAttributeValue(
        element,
        kAXChildrenAttribute as CFString,
        &childrenValue
    ) == .success,
    let children = childrenValue as? [AXUIElement] else {
        return false
    }

    return children.contains { hasRenderedHeading($0, depth: depth + 1) }
}

guard hasRenderedHeading(appElement) else {
    fputs(
        "The app has a visible window, but its rendered main interface heading " +
        "\"\(expectedHeading)\" was not found in the accessibility tree. The webview may be blank or failed to load.\n",
        stderr
    )
    exit(3)
}

print(app.processIdentifier)
