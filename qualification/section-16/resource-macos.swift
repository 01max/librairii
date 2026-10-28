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
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: nil, virtualKey: 12, keyDown: down)!
            event.flags = .maskCommand; event.post(tap: .cghidEventTap)
        }
        print("{\"native_quit_posted\":true}")
    } else {
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 0.1)
        var witness: [String: String] = [:]
        var queries = 0
        var queryNanoseconds: UInt64 = 0
        let attributes = [kAXTitleAttribute, kAXDescriptionAttribute, kAXChildrenAttribute, kAXRoleAttribute] as CFArray
        while witness.isEmpty {
            var queue: [AXUIElement] = [app]; var next = 0
            while next < queue.count && next < 4096 {
                let item = queue[next]; next += 1
                let queryStarted = DispatchTime.now().uptimeNanoseconds
                var rawValues: CFArray?
                let result = AXUIElementCopyMultipleAttributeValues(item, attributes, [], &rawValues)
                queryNanoseconds += DispatchTime.now().uptimeNanoseconds-queryStarted
                queries += 1
                guard result == .success, let values = rawValues as? [Any], values.count == 4 else { continue }
                let title = values[0] as? String ?? ""
                let description = values[1] as? String ?? ""
                if title.hasPrefix("Histoire 0001") || description.hasPrefix("Histoire 0001") {
                    witness = ["role": values[3] as? String ?? "", "title": title, "description": description]; break
                }
                queue.append(contentsOf: (values[2] as? [AXUIElement] ?? []).prefix(max(0, 4096 - queue.count)))
            }
            if DispatchTime.now().uptimeNanoseconds - start > 10_000_000_000 { throw Refusal(message: "actual product first story accessibility readiness deadline") }
            if witness.isEmpty { Thread.sleep(forTimeInterval: 0.001) }
        }
        print(String(data: try JSONSerialization.data(withJSONObject: ["pid": pid, "actual_story": witness,
            "observer_attach_to_story_ms": Double(DispatchTime.now().uptimeNanoseconds-start)/1e6,
            "observer_queries": queries, "observer_query_ms": Double(queryNanoseconds)/1e6,
            "method": "actual product first loaded story in public native AX tree"]), encoding: .utf8)!)
    }
} catch { fputs("\(error)\n", stderr); exit(1) }
