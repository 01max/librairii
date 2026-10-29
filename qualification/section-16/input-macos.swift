// Public native event delivery to the explicit owned fixture PID only.
import AppKit
import ApplicationServices
import CoreGraphics
import Foundation
struct Refusal: Error { let message: String }
guard CommandLine.arguments.count == 3, let pid = pid_t(CommandLine.arguments[1]) else { exit(2) }
let directory = URL(fileURLWithPath: CommandLine.arguments[2])
func state() throws -> [String: Any] {
    try JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("state.json"))) as! [String: Any]
}
func wait(_ predicate: ([String: Any]) -> Bool, seconds: Double = 10, line: Int = #line) throws -> [String: Any] {
    let start = DispatchTime.now().uptimeNanoseconds
    while true {
        let observed = try state()
        if predicate(observed) { return observed }
        if Double(DispatchTime.now().uptimeNanoseconds - start) / 1e9 > seconds { throw Refusal(message: "native state deadline at line \(line): \(observed)") }
        Thread.sleep(forTimeInterval: 0.001)
    }
}
func key(_ code: CGKeyCode, flags: CGEventFlags = []) throws {
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { throw Refusal(message: "lost owned foreground PID") }
    if flags.contains(.maskShift) {
        let shift = CGEvent(keyboardEventSource: nil, virtualKey: 56, keyDown: true)!
        shift.type = .flagsChanged; shift.flags = .maskShift; shift.post(tap: .cghidEventTap)
    }
    for down in [true, false] {
        guard let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else { throw Refusal(message: "CGEvent allocation") }
        event.flags = flags
        event.post(tap: .cghidEventTap)
    }
    if flags.contains(.maskShift) {
        let shift = CGEvent(keyboardEventSource: nil, virtualKey: 56, keyDown: false)!
        shift.type = .flagsChanged; shift.flags = []; shift.post(tap: .cghidEventTap)
    }
}
func elapsed(_ start: UInt64) -> Double { Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6 }
// Clock synchronization runs after the input cohort, preserving its existing
// event queue and preconditioning. Every round bounds the same clock offset.
func dispatchCalibrate() throws -> [[String: Any]] {
    var clocks: [[String: Any]] = []
    for sequence in 1...64 {
        let before = DispatchTime.now().uptimeNanoseconds
        try Data(String(sequence).utf8).write(to: directory.appendingPathComponent("dispatch-clock-request"), options: .atomic)
        while true {
            if let data = try? Data(contentsOf: directory.appendingPathComponent("dispatch-clock-response")),
                let response = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                response["sequence"] as? Int == sequence, let native = response["native_ns"] as? UInt64 {
                clocks.append(["sequence": sequence, "before_ns": before, "native_ns": native,
                    "after_ns": DispatchTime.now().uptimeNanoseconds])
                break
            }
            if DispatchTime.now().uptimeNanoseconds - before > 2_000_000_000 { throw Refusal(message: "dispatch clock deadline") }
            Thread.sleep(forTimeInterval: 0.001)
        }
    }
    return clocks
}
func dispatchCounter(_ value: [String: Any], _ key: String) throws -> UInt64 {
    guard let dispatch = value["dispatch"] as? [String: Any], let count = dispatch[key+"_sequence"] as? UInt64 else {
        throw Refusal(message: "actual native dispatch counter missing")
    }
    return count
}
func dispatchSample(_ value: [String: Any], _ key: String, _ before: UInt64, _ n: Int, _ start: UInt64, _ posted: UInt64, _ observed: UInt64) throws -> [String: Any] {
    guard let dispatch = value["dispatch"] as? [String: Any], let ack = dispatch[key] as? [String: Any],
        ack["sequence"] as? UInt64 == before+1 else { throw Refusal(message: "ambiguous native callback acknowledgement") }
    return ["n": n, "before_sequence": before, "ack": ack, "start_ns": start, "posted_ns": posted, "observed_ns": observed]
}
func active(_ value: [String: Any]) -> Bool { value["running_import"] as? Bool == true }
func focusSearch() throws {
    for _ in 0..<64 {
        let before = try state()
        if before["query_focused"] as? Bool == true { return }
        try key(48, flags: .maskShift)
        _ = try wait { $0["query_focused"] as? Bool == true || $0["focus_key"] as? Int != before["focus_key"] as? Int }
    }
    throw Refusal(message: "native reverse Tab did not reach Search")
}
struct PointerTarget { let point: CGPoint; let role: String; let name: String; let firstTitleRole: String; let parentRole: String; let parentName: String; let actions: [String] }
func firstStoryPoint() throws -> PointerTarget {
    let app = AXUIElementCreateApplication(pid)
    let start = DispatchTime.now().uptimeNanoseconds
    var firstTitleRole: String? = nil
    while DispatchTime.now().uptimeNanoseconds - start < 5_000_000_000 {
        var queue = [app]; var next = 0
        while next < queue.count && next < 4096 {
            let item = queue[next]; next += 1
            func attribute(_ name: String, of element: AXUIElement? = nil) -> CFTypeRef? {
                var value: CFTypeRef?
                return AXUIElementCopyAttributeValue(element ?? item, name as CFString, &value) == .success ? value : nil
            }
            let title = attribute(kAXTitleAttribute) as? String ?? ""
            let description = attribute(kAXDescriptionAttribute) as? String ?? ""
            if title.hasPrefix("Histoire 0001") || description.hasPrefix("Histoire 0001") {
                let role = attribute(kAXRoleAttribute) as? String ?? ""
                if firstTitleRole == nil { firstTitleRole = role }
                // Slint ListItem becomes AccessKit ListBoxOption/AXStaticText.
                // The browser-list parent and default action distinguish the
                // row from the inspector's duplicate static-text title.
                guard role == kAXStaticTextRole as String,
                    let rawParent = attribute(kAXParentAttribute), CFGetTypeID(rawParent) == AXUIElementGetTypeID() else { continue }
                let parent = rawParent as! AXUIElement
                let parentRole = attribute(kAXRoleAttribute, of: parent) as? String ?? ""
                let parentName = attribute(kAXTitleAttribute, of: parent) as? String ?? ""
                var rawActions: CFArray?
                AXUIElementCopyActionNames(item, &rawActions)
                let actions = rawActions as? [String] ?? []
                guard parentRole == kAXListRole as String, ["Toutes les histoires", "All stories"].contains(parentName), actions.contains(kAXPressAction as String) else { continue }
                if let rawPosition = attribute(kAXPositionAttribute), let rawSize = attribute(kAXSizeAttribute), CFGetTypeID(rawPosition) == AXValueGetTypeID(), CFGetTypeID(rawSize) == AXValueGetTypeID() {
                    var position = CGPoint.zero; var size = CGSize.zero
                    if AXValueGetValue(rawPosition as! AXValue, .cgPoint, &position) && AXValueGetValue(rawSize as! AXValue, .cgSize, &size) && size.width > 8 && size.height > 8 {
                        return PointerTarget(point: CGPoint(x: position.x + size.width / 2, y: position.y + size.height / 2), role: role, name: title.isEmpty ? description : title, firstTitleRole: firstTitleRole!, parentRole: parentRole, parentName: parentName, actions: actions)
                    }
                }
            }
            queue.append(contentsOf: (attribute(kAXChildrenAttribute) as? [AXUIElement] ?? []).prefix(max(0, 4096 - queue.count)))
        }
        Thread.sleep(forTimeInterval: 0.001)
    }
    throw Refusal(message: "actual first story AX pointer bounds missing")
}
func coldFirstPage() throws -> Int {
    for page in 2...7 {
        try Data(String(page).utf8).write(to: directory.appendingPathComponent("page"), options: .atomic)
        _ = try wait { $0["page"] as? Int == page && $0["settled"] as? Bool == true }
    }
    return try state()["first_cover_reloads"] as! Int
}
do {
    guard CommandLine.arguments.count == 3, ProcessInfo.processInfo.environment["GITHUB_ACTIONS"] == "true", AXIsProcessTrusted(), CGPreflightPostEventAccess(),
        let app = NSRunningApplication(processIdentifier: pid) else { throw Refusal(message: "disposable native CI event access required") }
    app.activate(options: [])
    _ = try wait { _ in NSWorkspace.shared.frontmostApplication?.processIdentifier == pid }
    try focusSearch()
    var input: [Double] = []; var inputStates: [[String: Any]] = []
    var inputDispatch: [[String: Any]] = []; var pointerDispatch: [[String: Any]] = []
    for n in 0..<64 {
        let before = try wait { active($0) }
        let sequence = try dispatchCounter(before, "search")
        let wanted = n % 2 == 0 ? "x" : ""
        let start = DispatchTime.now().uptimeNanoseconds
        try key(n % 2 == 0 ? 7 : 51)
        let posted = DispatchTime.now().uptimeNanoseconds
        let end = try wait({ $0["query"] as? String == wanted }, seconds: 1)
        let observed = DispatchTime.now().uptimeNanoseconds
        input.append(Double(observed-start)/1e6); inputStates.append(end)
        inputDispatch.append(try dispatchSample(end, "search", sequence, n, start, posted, observed))
        guard active(end) else { throw Refusal(message: "import ended during native input sample") }
    }
    let target = try firstStoryPoint()
    let point = target.point
    let pointerReceipt: [String: Any] = ["x": point.x, "y": point.y, "role": target.role, "name": target.name, "first_title_role": target.firstTitleRole, "parent_role": target.parentRole, "parent_name": target.parentName, "actions": target.actions]
    try JSONSerialization.data(withJSONObject: pointerReceipt, options: [.sortedKeys]).write(to: directory.appendingPathComponent("pointer-target.json"), options: .atomic)
    var pointers: [Double] = []; var pointerStates: [[String: Any]] = []
    for n in 0..<64 {
        try focusSearch(); let before = try wait { active($0) }
        guard before["query_focused"] as? Bool == true, before["focus_key"] as? Int == 30 else {
            throw Refusal(message: "pointer sample requires observed Search focus")
        }
        let sequence = try dispatchCounter(before, "browser_focus")
        let start = DispatchTime.now().uptimeNanoseconds
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { throw Refusal(message: "lost owned foreground PID before pointer post") }
        for type in [CGEventType.leftMouseDown, CGEventType.leftMouseUp] {
            guard let event = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .left) else { throw Refusal(message: "pointer event allocation") }
            event.post(tap: .cghidEventTap)
        }
        let posted = DispatchTime.now().uptimeNanoseconds
        let end = try wait({ $0["focus_key"] as? Int == 31 }, seconds: 1)
        let observed = DispatchTime.now().uptimeNanoseconds
        pointers.append(Double(observed-start)/1e6); pointerStates.append(end)
        pointerDispatch.append(try dispatchSample(end, "browser_focus", sequence, n, start, posted, observed))
        guard active(end) else { throw Refusal(message: "import ended during native pointer sample") }
    }
    let clockCalibrations = try dispatchCalibrate()
    var navigation: [Double] = []; var endpoints: [[String: Any]] = []
    for n in 0..<21 {
        let coldGeneration = try coldFirstPage()
        try focusSearch(); _ = try wait { active($0) }
        try key(7)
        _ = try wait { $0["query"] as? String == "x" && $0["total"] as? Int == 0 && $0["settled"] as? Bool == true }
        let start = DispatchTime.now().uptimeNanoseconds
        try key(51)
        _ = try wait { $0["total"] as? Int == 1000 && $0["rows"] as? Int == 24 && ($0["first_cover_reloads"] as? Int ?? -1) > coldGeneration && $0["settled"] as? Bool == true }
        for _ in 0..<32 {
            let before = try state()["focus_key"] as? Int
            if before == 31 { break }
            try key(48)
            _ = try wait { $0["focus_key"] as? Int != before }
        }
        guard try state()["focus_key"] as? Int == 31 else { throw Refusal(message: "native Tab did not reach browser") }
        try key(119)
        let end = try wait { $0["selected_index"] as? Int == 23 && $0["settled"] as? Bool == true }
        let duration = elapsed(start)
        guard active(end) else { throw Refusal(message: "import ended during native navigation sample") }
        if n > 0 { navigation.append(duration); endpoints.append(end) }
        try key(115)
        _ = try wait { $0["selected_index"] as? Int == 0 && $0["settled"] as? Bool == true }
    }
    let result: [String: Any] = ["clock_calibrations": clockCalibrations,
        "input_dispatch_samples": inputDispatch, "pointer_dispatch_samples": pointerDispatch, "input_ms": input, "input_states": inputStates, "input_p95_ms": input.sorted()[60],
        "pointer_ms": pointers, "pointer_states": pointerStates, "pointer_p95_ms": pointers.sorted()[60], "pointer_target": pointerReceipt,
        "navigation_ms": navigation, "endpoints": endpoints, "navigation_p95_ms": navigation.sorted()[18],
        "method": "method3: pre-CGEvent post to uniquely identified actual search callback return/browser focus acknowledgement, conservative synchronized clock upper bound; file-observation wall times retained separately; unchanged cold1000-story completion"]
    print(String(data: try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]), encoding: .utf8)!)
} catch { fputs("\(error)\n", stderr); exit(1) }
