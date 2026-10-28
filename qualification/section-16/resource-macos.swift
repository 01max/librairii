// Observe the actual product executable through public AX. No product probe hook.
import AppKit
import ApplicationServices
import CoreGraphics
import Foundation
struct Refusal: Error { let message: String }
guard CommandLine.arguments.count >= 2 else { exit(2) }
let watching = CommandLine.arguments[1] == "watch"
guard CommandLine.arguments.count == (watching ? 2 : 3) else { exit(2) }
var pid: pid_t = 0
if !watching {
    guard let supplied = pid_t(CommandLine.arguments[2]) else { exit(2) }
    pid = supplied
}
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}
func text(_ element: AXUIElement, _ name: String) -> String { attribute(element, name) as? String ?? "" }
func quitFromOwnedMenu(_ app: AXUIElement) throws -> [String: Any] {
    let started = DispatchTime.now().uptimeNanoseconds
    func checkDeadline() throws {
        if DispatchTime.now().uptimeNanoseconds-started >= 2_000_000_000 {
            throw Refusal(message: "owned native Quit menu item deadline")
        }
    }
    var trace: [[String: Any]] = []
    var emitted = false
    defer {
        if !emitted, let data = try? JSONSerialization.data(withJSONObject: ["native_menu_quit_trace": trace], options: [.sortedKeys]),
           let encoded = String(data: data, encoding: .utf8) { fputs(encoded+"\n", stderr) }
    }
    guard let rawBar = attribute(app, kAXMenuBarAttribute), CFGetTypeID(rawBar) == AXUIElementGetTypeID() else {
        throw Refusal(message: "owned native menu bar missing")
    }
    let bar = rawBar as! AXUIElement
    AXUIElementSetMessagingTimeout(bar, 0.1)
    try checkDeadline()
    let entries = attribute(bar, kAXChildrenAttribute) as? [AXUIElement] ?? []
    var applicationMenuTitle = ""
    guard let applicationMenu = try entries.prefix(16).first(where: { item in
        try checkDeadline()
        AXUIElementSetMessagingTimeout(item, 0.1)
        let title = text(item, kAXTitleAttribute)
        if trace.count < 64 { trace.append(["stage": "application-menu", "title": String(title.prefix(160))]) }
        // AppKit uses the process name for a directly launched executable;
        // the internal .app size witness supplies CFBundleName=Librairii.
        let matches = ["Librairii", "librairii-desktop"].contains(title)
        if matches { applicationMenuTitle = title }
        return matches
    }) else {
        throw Refusal(message: "owned Librairii application menu missing")
    }
    try checkDeadline()
    let opened = AXUIElementPerformAction(applicationMenu, kAXPressAction as CFString)
    guard opened == .success else { throw Refusal(message: "owned native application menu action failed: \(opened.rawValue)") }
    while DispatchTime.now().uptimeNanoseconds-started < 2_000_000_000 {
        var stack = [applicationMenu]
        var visited = 0
        while let item = stack.popLast(), visited < 256 {
            try checkDeadline()
            visited += 1
            AXUIElementSetMessagingTimeout(item, 0.1)
            let role = text(item, kAXRoleAttribute)
            let title = text(item, kAXTitleAttribute)
            let enabled = attribute(item, kAXEnabledAttribute) as? Bool ?? false
            if trace.count < 64 { trace.append(["role": role, "title": String(title.prefix(160)), "enabled": enabled]) }
            if role == kAXMenuItemRole as String && ["Quit Librairii", "Quitter Librairii"].contains(title) {
                var rawActions: CFArray?
                AXUIElementCopyActionNames(item, &rawActions)
                let actions = rawActions as? [String] ?? []
                guard enabled && actions.contains(kAXPressAction as String) else { throw Refusal(message: "owned native Quit action unavailable") }
                try checkDeadline()
                let result = AXUIElementPerformAction(item, kAXPressAction as CFString)
                guard result == .success else { throw Refusal(message: "owned native Quit action failed: \(result.rawValue)") }
                emitted = true
                return ["native_quit_posted": true, "pid": pid,
                    "method": "owned native application menu AXPress", "application_menu": applicationMenuTitle,
                    "role": role, "title": title, "actions": actions, "result": result.rawValue]
            }
            let children = attribute(item, kAXChildrenAttribute) as? [AXUIElement] ?? []
            stack.append(contentsOf: children.prefix(max(0, 256-visited-stack.count)).reversed())
        }
        Thread.sleep(forTimeInterval: 0.001)
    }
    throw Refusal(message: "owned native Quit menu item deadline")
}
do {
    guard ProcessInfo.processInfo.environment["GITHUB_ACTIONS"] == "true", AXIsProcessTrusted(), CGPreflightPostEventAccess() else { throw Refusal(message: "disposable native CI observer access required") }
    if watching {
        // Prepare only our external observer; product launch remains timed.
        _ = NSWorkspace.shared.frontmostApplication
        var ownRole: CFTypeRef?
        AXUIElementCopyAttributeValue(AXUIElementCreateApplication(ProcessInfo.processInfo.processIdentifier), kAXRoleAttribute as CFString, &ownRole)
        print("{\"observer_ready\":true}"); fflush(stdout)
        guard let line = readLine(), let supplied = pid_t(line), supplied > 0 else { throw Refusal(message: "owned product PID required") }
        pid = supplied
    }
    let start = DispatchTime.now().uptimeNanoseconds
    while NSRunningApplication(processIdentifier: pid) == nil {
        if DispatchTime.now().uptimeNanoseconds - start > 5_000_000_000 { throw Refusal(message: "product process not registered") }
        Thread.sleep(forTimeInterval: 0.001)
    }
    let running = NSRunningApplication(processIdentifier: pid)!
    running.activate(options: [])
    if CommandLine.arguments[1] == "quit" {
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { throw Refusal(message: "product quit lost foreground PID") }
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 0.1)
        print(String(data: try JSONSerialization.data(withJSONObject: quitFromOwnedMenu(app), options: [.sortedKeys]), encoding: .utf8)!)
    } else {
        // AX requests made while AppKit is still creating its first window can
        // block behind initialization. Observe the owned window through public
        // WindowServer metadata first; the clock already includes this wait.
        // This is only permission to query AX, never the readiness endpoint.
        var windowPolls = 0
        while true {
            windowPolls += 1
            let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
            let ownedWindow = windows.contains { window in
                (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid
                    && (window[kCGWindowLayer as String] as? NSNumber)?.intValue == 0
                    && ((window[kCGWindowAlpha as String] as? NSNumber)?.doubleValue ?? 0) > 0
            }
            if ownedWindow { break }
            if DispatchTime.now().uptimeNanoseconds - start > 10_000_000_000 {
                let diagnostics: [String: Any] = ["stage": "owned-window", "pid": pid,
                    "observer_window_polls": windowPolls,
                    "observer_window_elapsed_ms": Double(DispatchTime.now().uptimeNanoseconds-start)/1e6]
                if let data = try? JSONSerialization.data(withJSONObject: diagnostics, options: [.sortedKeys]),
                   let encoded = String(data: data, encoding: .utf8) { fputs(encoded+"\n", stderr) }
                throw Refusal(message: "actual product owned window deadline")
            }
            Thread.sleep(forTimeInterval: 0.001)
        }
        let windowReadyMilliseconds = Double(DispatchTime.now().uptimeNanoseconds-start)/1e6
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 0.1)
        var witness: [String: String] = [:]
        var queries = 0
        var queryNanoseconds: UInt64 = 0
        func elapsedSinceAttach() -> Double { Double(DispatchTime.now().uptimeNanoseconds-start)/1e6 }
        var trace: [[String: Any]] = []
        var traceDropped = 0
        var passes = 0
        var emitted = false
        defer {
            if !emitted, let data = try? JSONSerialization.data(withJSONObject: [
                "observer_traversal": "owned-AXWindows-preorder", "observer_passes": passes,
                "observer_queries": queries, "observer_query_ms": Double(queryNanoseconds)/1e6,
                "observer_window_ready_ms": windowReadyMilliseconds, "observer_window_polls": windowPolls,
                "observer_trace": trace, "observer_trace_dropped": traceDropped
            ], options: [.sortedKeys]), let encoded = String(data: data, encoding: .utf8) {
                fputs(encoded+"\n", stderr)
            }
        }
        func retain(_ entry: [String: Any]) {
            if trace.count < 512 { trace.append(entry) } else { traceDropped += 1 }
        }
        let attributes = [kAXTitleAttribute, kAXDescriptionAttribute, kAXChildrenAttribute, kAXRoleAttribute] as CFArray
        while witness.isEmpty {
            passes += 1
            // The application's children also include the native menu bar.
            // Search only its real windows, preserving the loaded-story endpoint.
            let windowsStarted = DispatchTime.now().uptimeNanoseconds
            var rawWindows: CFTypeRef?
            let windowsResult = AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &rawWindows)
            let windowsDuration = DispatchTime.now().uptimeNanoseconds-windowsStarted
            queryNanoseconds += windowsDuration; queries += 1
            let windows = rawWindows as? [AXUIElement] ?? []
            retain(["pass": passes, "attribute": "AXWindows", "offset_ms": elapsedSinceAttach(),
                "query_ms": Double(windowsDuration)/1e6, "result": windowsResult.rawValue, "children": windows.count])
            // Preorder visits a window subtree before unrelated siblings. No
            // cached node/known title substitutes for querying the actual row.
            var stack = windows.prefix(16).reversed().map { ($0, 0) }
            var visited = 0
            while let (item, depth) = stack.popLast(), visited < 4096 {
                visited += 1
                let queryStarted = DispatchTime.now().uptimeNanoseconds
                var rawValues: CFArray?
                let result = AXUIElementCopyMultipleAttributeValues(item, attributes, [], &rawValues)
                let duration = DispatchTime.now().uptimeNanoseconds-queryStarted
                queryNanoseconds += duration; queries += 1
                guard result == .success, let values = rawValues as? [Any], values.count == 4 else {
                    retain(["pass": passes, "visited": visited, "depth": depth, "offset_ms": elapsedSinceAttach(),
                        "query_ms": Double(duration)/1e6, "result": result.rawValue])
                    continue
                }
                let title = values[0] as? String ?? ""
                let description = values[1] as? String ?? ""
                let role = values[3] as? String ?? ""
                let children = values[2] as? [AXUIElement] ?? []
                retain(["pass": passes, "visited": visited, "depth": depth, "offset_ms": elapsedSinceAttach(),
                    "query_ms": Double(duration)/1e6, "result": result.rawValue,
                    "role": role, "title": String(title.prefix(160)), "children": children.count])
                if title.hasPrefix("Histoire 0001") || description.hasPrefix("Histoire 0001") {
                    witness = ["role": role, "title": title, "description": description]; break
                }
                stack.append(contentsOf: children.prefix(max(0, 4096-visited-stack.count)).reversed().map { ($0, depth+1) })
            }
            if DispatchTime.now().uptimeNanoseconds - start > 10_000_000_000 { throw Refusal(message: "actual product first story accessibility readiness deadline") }
            if witness.isEmpty { Thread.sleep(forTimeInterval: 0.001) }
        }
        print(String(data: try JSONSerialization.data(withJSONObject: ["pid": pid, "actual_story": witness,
            "observer_attach_to_story_ms": Double(DispatchTime.now().uptimeNanoseconds-start)/1e6,
            "observer_queries": queries, "observer_query_ms": Double(queryNanoseconds)/1e6,
            "observer_window_ready_ms": windowReadyMilliseconds, "observer_window_polls": windowPolls,
            "observer_traversal": "owned-AXWindows-preorder", "observer_passes": passes,
            "observer_trace": trace, "observer_trace_dropped": traceDropped,
            "method": "actual product first loaded story in public native AX tree"]), encoding: .utf8)!)
        emitted = true
    }
} catch { fputs("\(error)\n", stderr); exit(1) }
