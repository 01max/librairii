// External CI observer using public macOS accessibility and event APIs only.
import AppKit
import ApplicationServices
import Carbon
import CoreGraphics
import Foundation

enum Failure: Error { case invalid(String) }

func inputSource() -> String {
    let source = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
    guard let value = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) else { return "unavailable" }
    return Unmanaged<CFString>.fromOpaque(value).takeUnretainedValue() as String
}

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

func checkedWindows(_ app: AXUIElement) throws -> [AXUIElement] {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value)
    guard status == .success, let result = value as? [AXUIElement] else {
        throw Failure.invalid("Finder AXWindows query failed: " + String(status.rawValue))
    }
    return result
}

func checkedText(_ element: AXUIElement, _ name: String, optional: Bool = false) throws -> String {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(element, name as CFString, &value)
    if optional && (status == .attributeUnsupported || status == .noValue) { return "" }
    guard status == .success else {
        throw Failure.invalid("Finder " + name + " query failed: " + String(status.rawValue))
    }
    if let result = value as? String { return result }
    if let value, CFGetTypeID(value) == CFURLGetTypeID() { return (value as! URL).absoluteString }
    throw Failure.invalid("Finder attribute has unexpected type: " + name)
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
    if ["key", "digits", "menu"].contains(operation), NSWorkspace.shared.frontmostApplication?.processIdentifier != pid {
        throw Failure.invalid("target app is not frontmost; native input refused")
    }
    switch operation {
    case "snapshot", "status":
        trace("reading owned CG windows and foreground status")
        var windows = (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]] ?? []).filter { ($0[kCGWindowOwnerPID as String] as? Int) == Int(pid) }
        var onlyWindow: Int?
        var documentObservation: [String: Any] = [:]
        if let document = request["document"] as? String {
            let target = URL(fileURLWithPath: document).resolvingSymlinksInPath().standardizedFileURL
            let nativeWindows = try checkedWindows(app)
            // A newly opening Finder window can expose AXTitle=noValue before
            // its title exists. Keep this as incomplete read-only observation,
            // never as proof that our previously verified window has closed.
            let titles = try nativeWindows.map { try checkedText($0, kAXTitleAttribute, optional: true) }
            let unresolvedTitles = titles.filter(\.isEmpty).count
            let matches = try nativeWindows.enumerated().filter { _, window in
                try [kAXDocumentAttribute, kAXURLAttribute].contains { attribute in
                    guard let url = URL(string: try checkedText(window, attribute, optional: true)), url.isFileURL else { return false }
                    return url.resolvingSymlinksInPath().standardizedFileURL == target
                }
            }
            guard matches.count <= 1 else { throw Failure.invalid("owned document window not unique") }
            guard let (index, _) = matches.first else {
                // Only root metadata with the exact owned fixture title may be
                // retained when URL matching fails; no other folder is walked.
                let candidates = nativeWindows.enumerated().filter { index, _ in
                    titles[index] == target.lastPathComponent
                }
                guard candidates.count <= 1 else { throw Failure.invalid("owned Finder title not unique") }
                let diagnostic = candidates.first.map { index, _ in
                    tree(app, onlyWindow: index, sheetsOnly: false)
                } ?? []
                let candidateWindows = candidates.first.map { _, window in
                    windows.filter { $0[kCGWindowName as String] as? String == text(window, kAXTitleAttribute) }
                } ?? []
                guard candidateWindows.count <= 1 else { throw Failure.invalid("owned Finder capture not unique") }
                try output(["pid": pid, "nodes": [], "windows": [],
                    "document_window_list_observed": true,
                    "document_windows_observed": unresolvedTitles == 0,
                    "unresolved_title_window_count": unresolvedTitles,
                    "owned_document_window_present": !candidates.isEmpty,
                    "finder_window_count": nativeWindows.count,
                    "owned_title_candidates": candidates.map { describe($0.element) },
                    "owned_candidate_cg_windows": candidateWindows,
                    "owned_candidate_diagnostic_tree": diagnostic] as [String: Any])
                exit(0)
            }
            onlyWindow = index
            documentObservation = ["document_windows_observed": unresolvedTitles == 0,
                "document_window_list_observed": true,
                "unresolved_title_window_count": unresolvedTitles, "owned_document_window_present": true]
            let title = titles[index]
            windows = windows.filter { !title.isEmpty && $0[kCGWindowName as String] as? String == title }
            guard windows.count <= 1 else { throw Failure.invalid("document capture window not unique") }
        }
        let response: [String: Any] = ["pid": pid, "frontmost_pid": NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1,
                    "keyboard_input_source": inputSource(),
                    "nodes": operation == "status" ? [] : tree(app, onlyWindow: onlyWindow,
                        sheetsOnly: request["document"] == nil,
                        rootsOnly: request["summary"] as? Bool ?? false), "windows": windows]
        try output(response.merging(documentObservation) { _, new in new })
    case "window-status":
        guard let number = request["window_id"] as? Int, number > 0,
              let wanted = CGWindowID(exactly: number),
              let windows = CGWindowListCopyWindowInfo([.excludeDesktopElements], kCGNullWindowID)
                as? [[String: Any]] else {
            throw Failure.invalid("complete CG window query and explicit window ID required")
        }
        try output(["observed": true, "pid": pid, "window_id": number,
            "present": windows.contains { ($0[kCGWindowOwnerPID as String] as? Int) == Int(pid)
                && ($0[kCGWindowNumber as String] as? Int) == Int(wanted) }] as [String: Any])
    case "activate":
        guard NSRunningApplication(processIdentifier: pid)?.activate(options: [.activateAllWindows]) == true else {
            throw Failure.invalid("cannot activate owned app")
        }
        try output(["activated": true])
    case "key":
        try key(request["key"] as? String ?? "")
        try output(["posted": true])
    case "digits":
        let codes: [Character: CGKeyCode] = ["0": 29, "1": 18, "2": 19, "3": 20, "4": 21,
            "5": 23, "6": 22, "7": 26, "8": 28, "9": 25]
        for character in request["text"] as? String ?? "" {
            guard let code = codes[character] else { throw Failure.invalid("native number keys only") }
            try post(code, down: true, flags: [])
            try post(code, down: false, flags: [])
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
