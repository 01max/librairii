// External CI observer using public macOS accessibility and event APIs only.
import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

enum Failure: Error { case invalid(String) }

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}

func elements(_ element: AXUIElement, _ name: String) -> [AXUIElement] {
    attribute(element, name) as? [AXUIElement] ?? []
}

func text(_ element: AXUIElement, _ name: String) -> String {
    if let value = attribute(element, name) as? String { return value }
    if let value = attribute(element, name), CFGetTypeID(value) == CFURLGetTypeID() {
        return (value as! URL).absoluteString
    }
    return ""
}

func describe(_ element: AXUIElement) -> [String: Any] {
    var result: [String: Any] = [:]
    for (key, name) in [("role", kAXRoleAttribute), ("subrole", kAXSubroleAttribute),
                        ("title", kAXTitleAttribute), ("description", kAXDescriptionAttribute),
                        ("identifier", kAXIdentifierAttribute), ("value", kAXValueAttribute),
                        ("document", kAXDocumentAttribute), ("url", kAXURLAttribute)] {
        result[key] = text(element, name)
    }
    for (key, name) in [("enabled", kAXEnabledAttribute), ("selected", kAXSelectedAttribute),
                        ("modal", kAXModalAttribute), ("focused", kAXFocusedAttribute)] {
        if let value = attribute(element, name) as? Bool { result[key] = value }
    }
    var editable = DarwinBoolean(false)
    if AXUIElementIsAttributeSettable(element, kAXValueAttribute as CFString, &editable) == .success {
        result["value_settable"] = editable.boolValue
    }
    return result
}

func output(_ value: Any) throws {
    let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    guard data.count < 4 * 1024 * 1024 else { throw Failure.invalid("observer output limit") }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([10]))
}

func trace(_ stage: String) {
    FileHandle.standardError.write(Data(("observer-stage: " + stage + "\n").utf8))
}

func sheets(_ window: AXUIElement) -> [AXUIElement] {
    elements(window, kAXChildrenAttribute).filter { text($0, kAXRoleAttribute) == kAXSheetRole }
}

func roots(_ app: AXUIElement, sheetsOnly: Bool) -> [(AXUIElement, [String: Any])] {
    var result: [(AXUIElement, [String: Any])] = []
    trace("reading AXWindows")
    for (index, window) in elements(app, kAXWindowsAttribute).enumerated() {
        trace("reading child sheets of window " + String(index))
        for (sheetIndex, sheet) in sheets(window).enumerated() {
            result.append((sheet, ["window": index, "sheet": sheetIndex]))
        }
        if !sheetsOnly { result.append((window, ["window": index])) }
    }
    return result
}

func resolve(_ app: AXUIElement, _ selector: [String: Any]) throws -> AXUIElement {
    let windows = elements(app, kAXWindowsAttribute)
    guard let index = selector["window"] as? Int, windows.indices.contains(index) else {
        throw Failure.invalid("window selector changed")
    }
    var element = windows[index]
    if let sheet = selector["sheet"] as? Int {
        let children = sheets(element)
        guard children.indices.contains(sheet) else { throw Failure.invalid("sheet selector changed") }
        element = children[sheet]
    }
    for index in selector["path"] as? [Int] ?? [] {
        let children = elements(element, kAXChildrenAttribute)
        guard children.indices.contains(index) else { throw Failure.invalid("native control changed") }
        element = children[index]
    }
    if let fingerprint = selector["match"] as? [String: String] {
        let observed = describe(element)
        guard fingerprint.allSatisfy({ observed[$0.key] as? String == $0.value }) else {
            throw Failure.invalid("native selector fingerprint changed")
        }
    }
    return element
}

func tree(_ app: AXUIElement, onlyWindow: Int? = nil, sheetsOnly: Bool = true,
          rootsOnly: Bool = false) -> [[String: Any]] {
    var result: [[String: Any]] = []
    for (root, location) in roots(app, sheetsOnly: sheetsOnly) {
        if let onlyWindow, location["window"] as? Int != onlyWindow { continue }
        trace("traversing native root " + String(describing: location))
        var queue: [(AXUIElement, [Int])] = [(root, [])]
        var next = 0
        while next < queue.count && result.count < 2500 {
            let (element, path) = queue[next]
            next += 1
            var node = describe(element)
            node["selector"] = location.merging(["path": path]) { _, new in new }
            result.append(node)
            if !rootsOnly && path.count < 25 {
                for (index, child) in elements(element, kAXChildrenAttribute).prefix(1000).enumerated() {
                    if queue.count >= 5000 { break }
                    queue.append((child, path + [index]))
                }
            }
        }
    }
    return result
}

func post(_ code: CGKeyCode, down: Bool, flags: CGEventFlags, modifier: Bool = false) throws {
    guard let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else {
        throw Failure.invalid("cannot create native key event")
    }
    event.flags = flags
    if modifier { event.type = .flagsChanged }
    event.post(tap: .cghidEventTap)
    Thread.sleep(forTimeInterval: 0.025)
}

func key(_ name: String) throws {
    let codes: [String: CGKeyCode] = ["a": 0, "g": 5, "c": 8, "v": 9, "x": 7, "z": 6,
        "q": 12, "2": 19, "comma": 43, "escape": 53, "return": 36, "tab": 48]
    let parts = name.split(separator: "+").map(String.init)
    guard let last = parts.last, let code = codes[last] else { throw Failure.invalid("unknown key") }
    var flags = CGEventFlags()
    var held: [(CGKeyCode, CGEventFlags)] = []
    for (name, code, flag) in [("cmd", CGKeyCode(55), CGEventFlags.maskCommand),
                               ("shift", CGKeyCode(56), CGEventFlags.maskShift)] where parts.contains(name) {
        flags.insert(flag)
        try post(code, down: true, flags: flags, modifier: true)
        held.append((code, flag))
    }
    try post(code, down: true, flags: flags)
    try post(code, down: false, flags: flags)
    for (code, flag) in held.reversed() {
        flags.remove(flag)
        try post(code, down: false, flags: flags, modifier: true)
    }
}

func menu(_ app: AXUIElement, title: String) throws {
    guard let bar = attribute(app, kAXMenuBarAttribute) else { throw Failure.invalid("no native menu bar") }
    let barElement = bar as! AXUIElement
    for top in elements(barElement, kAXChildrenAttribute) {
        var queue = elements(top, kAXChildrenAttribute)
        var next = 0
        while next < queue.count && next < 300 {
            let element = queue[next]
            next += 1
            if text(element, kAXTitleAttribute) == title {
                guard AXUIElementPerformAction(top, kAXPressAction as CFString) == .success else {
                    throw Failure.invalid("cannot open native menu")
                }
                Thread.sleep(forTimeInterval: 0.1)
                guard AXUIElementPerformAction(element, kAXPressAction as CFString) == .success else {
                    throw Failure.invalid("cannot activate native menu item")
                }
                return
            }
            queue.append(contentsOf: elements(element, kAXChildrenAttribute).prefix(max(0, 1000 - queue.count)))
        }
    }
    throw Failure.invalid("native menu item missing: " + title)
}

do {
    guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else { throw Failure.invalid("native observer access unavailable") }
    guard CommandLine.arguments.count == 3, let data = CommandLine.arguments[2].data(using: .utf8),
          let request = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          let number = request["pid"] as? Int, let pid = pid_t(exactly: number) else {
        throw Failure.invalid("explicit target PID required")
    }
    let operation = CommandLine.arguments[1]
    let app = AXUIElementCreateApplication(pid)
    AXUIElementSetMessagingTimeout(app, 1)
    if ["key", "text", "menu"].contains(operation), NSWorkspace.shared.frontmostApplication?.processIdentifier != pid {
        throw Failure.invalid("target app is not frontmost; native input refused")
    }
    switch operation {
    case "snapshot", "status":
        trace("reading owned CG windows and foreground status")
        var windows = (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]] ?? []).filter { ($0[kCGWindowOwnerPID as String] as? Int) == Int(pid) }
        var onlyWindow: Int?
        if let document = request["document"] as? String {
            let target = URL(fileURLWithPath: document).resolvingSymlinksInPath().standardizedFileURL
            let matches = elements(app, kAXWindowsAttribute).enumerated().filter { _, window in
                let value = text(window, kAXDocumentAttribute)
                guard let url = URL(string: value), url.isFileURL else { return false }
                return url.resolvingSymlinksInPath().standardizedFileURL == target
            }
            guard matches.count <= 1 else { throw Failure.invalid("owned document window not unique") }
            guard let (index, window) = matches.first else {
                try output(["pid": pid, "nodes": [], "windows": []] as [String: Any])
                exit(0)
            }
            onlyWindow = index
            let title = text(window, kAXTitleAttribute)
            windows = windows.filter { !title.isEmpty && $0[kCGWindowName as String] as? String == title }
            guard windows.count <= 1 else { throw Failure.invalid("document capture window not unique") }
        }
        try output(["pid": pid, "frontmost_pid": NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1,
                    "nodes": operation == "status" ? [] : tree(app, onlyWindow: onlyWindow,
                        sheetsOnly: request["document"] == nil,
                        rootsOnly: request["summary"] as? Bool ?? false), "windows": windows])
    case "activate":
        guard NSRunningApplication(processIdentifier: pid)?.activate(options: [.activateAllWindows]) == true else {
            throw Failure.invalid("cannot activate owned app")
        }
        try output(["activated": true])
    case "key":
        try key(request["key"] as? String ?? "")
        try output(["posted": true])
    case "text":
        let text = request["text"] as? String ?? ""
        for character in text {
            var units = Array(String(character).utf16)
            guard let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true) else {
                throw Failure.invalid("cannot create text event")
            }
            event.keyboardSetUnicodeString(stringLength: units.count, unicodeString: &units)
            event.post(tap: .cghidEventTap)
            event.type = .keyUp
            event.post(tap: .cghidEventTap)
        }
        try output(["posted": true])
    case "press", "set", "select":
        guard let selector = request["selector"] as? [String: Any] else { throw Failure.invalid("control selector required") }
        let element = try resolve(app, selector)
        var status: AXError
        if operation == "press" {
            status = AXUIElementPerformAction(element, kAXPressAction as CFString)
        } else if operation == "set" {
            status = AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString,
                (request["value"] as? String ?? "") as CFString)
        } else {
            let selectors = request["rows"] as? [[String: Any]] ?? []
            let rows = try selectors.map { try resolve(app, $0) }
            guard !rows.isEmpty else { throw Failure.invalid("selected native rows required") }
            status = AXUIElementSetAttributeValue(element, kAXSelectedRowsAttribute as CFString, rows as CFArray)
            let observed = elements(element, kAXSelectedRowsAttribute)
            guard observed.count == rows.count, rows.allSatisfy({ wanted in observed.contains { CFEqual(wanted, $0) } }) else {
                throw Failure.invalid("native selected rows differ")
            }
        }
        guard status == .success else { throw Failure.invalid("native control action failed: " + String(status.rawValue)) }
        var reply: [String: Any] = ["performed": true]
        if operation == "set" { reply["value"] = text(element, kAXValueAttribute) }
        try output(reply)
    case "menu":
        try menu(app, title: request["title"] as? String ?? "")
        try output(["performed": true])
    case "clipboard-set":
        NSPasteboard.general.clearContents()
        guard NSPasteboard.general.setString(request["text"] as? String ?? "", forType: .string) else {
            throw Failure.invalid("native clipboard write failed")
        }
        try output(["performed": true])
    case "clipboard-get":
        try output(["text": NSPasteboard.general.string(forType: .string) ?? ""])
    case "finder":
        try output(["pids": NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.finder").map(\.processIdentifier)])
    default:
        throw Failure.invalid("unknown observer operation")
    }
} catch {
    FileHandle.standardError.write(Data((String(describing: error) + "\n").utf8))
    exit(1)
}
