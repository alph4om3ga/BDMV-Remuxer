import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

let expectedHeading = "BDMV Remuxer main interface"

func reportMissingHeading() -> Never {
    fputs(
        "The app has a visible window, and macOS accessibility inspection completed, but its rendered main interface heading " +
        "\"\(expectedHeading)\" was not found in the accessibility tree. The webview may be blank, may have failed to load, or may not expose the expected heading.\n",
        stderr
    )
    exit(3)
}

func reportAccessibilityInspectionFailure(_ reason: String) -> Never {
    fputs(
        "macOS accessibility inspection failed: \(reason). The accessibility tree could not be inspected completely, so the interface heading could not be verified.\n",
        stderr
    )
    exit(4)
}

if CommandLine.arguments.count == 2,
   CommandLine.arguments[1] == "--test-missing-heading" {
    reportMissingHeading()
}

if CommandLine.arguments.count == 2,
   CommandLine.arguments[1] == "--test-accessibility-failure" {
    reportAccessibilityInspectionFailure(
        "Accessibility access is disabled or not authorized (AX error -25211)"
    )
}

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
var inspectedElements = 0
var firstInspectionFailure: String?

func recordInspectionFailure(_ reason: String) {
    if firstInspectionFailure == nil {
        firstInspectionFailure = reason
    }
}

func describeAccessibilityError(_ error: AXError) -> String {
    if error == .apiDisabled {
        return "Accessibility access is disabled or not authorized (AX error \(error.rawValue))"
    }
    if error == .cannotComplete {
        return "macOS could not complete an accessibility query (AX error \(error.rawValue))"
    }
    return "macOS returned an accessibility query error (AX error \(error.rawValue))"
}

func hasRenderedHeading(_ element: AXUIElement, depth: Int = 0) -> Bool {
    guard depth <= 16 else {
        recordInspectionFailure("accessibility-tree traversal exceeded the maximum depth")
        return false
    }
    guard inspectedElements < 3000 else {
        recordInspectionFailure("accessibility-tree traversal exceeded the element limit")
        return false
    }
    inspectedElements += 1

    var roleValue: CFTypeRef?
    let roleStatus = AXUIElementCopyAttributeValue(
        element,
        kAXRoleAttribute as CFString,
        &roleValue
    )
    if roleStatus == .success,
       let role = roleValue as? String,
       role == kAXHeadingRole as String {
        var labelWasInspectable = false
        for attribute in [kAXTitleAttribute, kAXValueAttribute, kAXDescriptionAttribute] {
            var value: CFTypeRef?
            let status = AXUIElementCopyAttributeValue(element, attribute as CFString, &value)
            if status == .success {
                labelWasInspectable = true
                if let text = value as? String, text == expectedHeading {
                    return true
                }
            } else if status == .noValue {
                labelWasInspectable = true
            } else if status != .attributeUnsupported {
                recordInspectionFailure(
                    "\(describeAccessibilityError(status)) while reading a heading label"
                )
            }
        }
        if !labelWasInspectable {
            recordInspectionFailure("macOS does not expose a readable label for an accessibility heading")
        }
    } else if roleStatus == .success {
        recordInspectionFailure("macOS returned an unreadable accessibility role")
    } else if roleStatus != .noValue {
        recordInspectionFailure(
            "\(describeAccessibilityError(roleStatus)) while reading an element role"
        )
    }

    var childrenValue: CFTypeRef?
    let childrenStatus = AXUIElementCopyAttributeValue(
        element,
        kAXChildrenAttribute as CFString,
        &childrenValue
    )
    if childrenStatus == .noValue {
        return false
    }
    guard childrenStatus == .success else {
        recordInspectionFailure(
            "\(describeAccessibilityError(childrenStatus)) while reading child elements"
        )
        return false
    }
    guard let children = childrenValue as? [AXUIElement] else {
        recordInspectionFailure("macOS returned unreadable accessibility child elements")
        return false
    }

    for child in children {
        if inspectedElements >= 3000 {
            recordInspectionFailure("accessibility-tree traversal exceeded the element limit")
            break
        }
        if hasRenderedHeading(child, depth: depth + 1) {
            return true
        }
    }
    return false
}

guard hasRenderedHeading(appElement) else {
    if let failure = firstInspectionFailure {
        reportAccessibilityInspectionFailure(failure)
    }
    reportMissingHeading()
}

print(app.processIdentifier)
