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
func wait(_ predicate: ([String: Any]) -> Bool, seconds: Double = 10) throws -> [String: Any] {
    let start = DispatchTime.now().uptimeNanoseconds
    while true {
        let observed = try state()
        if predicate(observed) { return observed }
        if Double(DispatchTime.now().uptimeNanoseconds - start) / 1e9 > seconds { throw Refusal(message: "native state deadline: \(observed)") }
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
func firstStoryPoint() throws -> CGPoint {
    let app = AXUIElementCreateApplication(pid)
    let start = DispatchTime.now().uptimeNanoseconds
    while DispatchTime.now().uptimeNanoseconds - start < 5_000_000_000 {
        var queue = [app]; var next = 0
        while next < queue.count && next < 4096 {
            let item = queue[next]; next += 1
            func attribute(_ name: String) -> CFTypeRef? {
                var value: CFTypeRef?
                return AXUIElementCopyAttributeValue(item, name as CFString, &value) == .success ? value : nil
            }
            let title = attribute(kAXTitleAttribute) as? String ?? ""
            let description = attribute(kAXDescriptionAttribute) as? String ?? ""
            if title.hasPrefix("Histoire 0001") || description.hasPrefix("Histoire 0001") {
                if let rawPosition = attribute(kAXPositionAttribute), let rawSize = attribute(kAXSizeAttribute), CFGetTypeID(rawPosition) == AXValueGetTypeID(), CFGetTypeID(rawSize) == AXValueGetTypeID() {
                    var position = CGPoint.zero; var size = CGSize.zero
                    if AXValueGetValue(rawPosition as! AXValue, .cgPoint, &position) && AXValueGetValue(rawSize as! AXValue, .cgSize, &size) && size.width > 8 && size.height > 8 {
                        return CGPoint(x: position.x + size.width / 2, y: position.y + size.height / 2)
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
    for n in 0..<64 {
        _ = try wait { active($0) }
        let wanted = n % 2 == 0 ? "x" : ""
        let start = DispatchTime.now().uptimeNanoseconds
        try key(n % 2 == 0 ? 7 : 51)
        let end = try wait({ $0["query"] as? String == wanted }, seconds: 1)
        input.append(elapsed(start)); inputStates.append(end)
        guard active(end) else { throw Refusal(message: "import ended during native input sample") }
    }
    let point = try firstStoryPoint()
    var pointers: [Double] = []; var pointerStates: [[String: Any]] = []
    for _ in 0..<64 {
        try focusSearch(); _ = try wait { active($0) }
        let start = DispatchTime.now().uptimeNanoseconds
        for type in [CGEventType.leftMouseDown, CGEventType.leftMouseUp] {
            guard let event = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .left) else { throw Refusal(message: "pointer event allocation") }
            event.post(tap: .cghidEventTap)
        }
        let end = try wait({ $0["focus_key"] as? Int == 31 }, seconds: 1)
        pointers.append(elapsed(start)); pointerStates.append(end)
        guard active(end) else { throw Refusal(message: "import ended during native pointer sample") }
    }
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
    let result: [String: Any] = ["input_ms": input, "input_states": inputStates, "input_p95_ms": input.sorted()[60],
        "pointer_ms": pointers, "pointer_states": pointerStates, "pointer_p95_ms": pointers.sorted()[60], "pointer_target": ["x": point.x, "y": point.y],
        "navigation_ms": navigation, "endpoints": endpoints, "navigation_p95_ms": navigation.sorted()[18],
        "method": "pre-CGEvent post to observed actual production UI/controller acknowledgement; bounded24-row page of1000, native Tab/End selects last visible story"]
    print(String(data: try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]), encoding: .utf8)!)
} catch { fputs("\(error)\n", stderr); exit(1) }
