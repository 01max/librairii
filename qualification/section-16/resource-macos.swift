// Observe the actual product executable through public AX. No product probe hook.
import AppKit
import ApplicationServices
import CoreGraphics
import Foundation
struct Refusal: Error { let message: String }
guard CommandLine.arguments.count == 3, let pid = pid_t(CommandLine.arguments[2]) else { exit(2) }
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}
func text(_ element: AXUIElement, _ name: String) -> String { attribute(element, name) as? String ?? "" }
do {
    guard ProcessInfo.processInfo.environment["GITHUB_ACTIONS"] == "true", AXIsProcessTrusted(), CGPreflightPostEventAccess() else { throw Refusal(message: "disposable native CI observer access required") }
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
        while witness.isEmpty {
            var queue: [AXUIElement] = [app]; var next = 0
            while next < queue.count && next < 4096 {
                let item = queue[next]; next += 1
                let title = text(item, kAXTitleAttribute)
                let description = text(item, kAXDescriptionAttribute)
                if title.hasPrefix("Histoire 0001") || description.hasPrefix("Histoire 0001") {
                    witness = ["role": text(item, kAXRoleAttribute), "title": title, "description": description]; break
                }
                queue.append(contentsOf: (attribute(item, kAXChildrenAttribute) as? [AXUIElement] ?? []).prefix(max(0, 4096 - queue.count)))
            }
            if DispatchTime.now().uptimeNanoseconds - start > 10_000_000_000 { throw Refusal(message: "actual product first story accessibility readiness deadline") }
            if witness.isEmpty { Thread.sleep(forTimeInterval: 0.001) }
        }
        print(String(data: try JSONSerialization.data(withJSONObject: ["pid": pid, "actual_story": witness, "method": "actual product first loaded story in public native AX tree"]), encoding: .utf8)!)
    }
} catch { fputs("\(error)\n", stderr); exit(1) }
