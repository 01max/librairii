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

func inputProperty(_ source: TISInputSource, _ name: CFString) -> Any? {
    guard let value = TISGetInputSourceProperty(source, name) else { return nil }
    return Unmanaged<AnyObject>.fromOpaque(value).takeUnretainedValue()
}

func sourceDescription(_ source: TISInputSource) -> [String: Any] {
    var result: [String: Any] = [:]
    for (key, name) in [("id", kTISPropertyInputSourceID), ("type", kTISPropertyInputSourceType),
                        ("bundle", kTISPropertyBundleID), ("enabled", kTISPropertyInputSourceIsEnabled),
                        ("selected", kTISPropertyInputSourceIsSelected),
                        ("enable_capable", kTISPropertyInputSourceIsEnableCapable),
                        ("select_capable", kTISPropertyInputSourceIsSelectCapable)] {
        if let value = inputProperty(source, name!) { result[key] = value }
    }
    return result
}

func inputSources(_ filter: [String: Any]) -> [TISInputSource] {
    // TIS returns nil for an installed-family filter with no matches. This is
    // an empty inventory, never a selected/available source or a passing gate.
    guard let result = TISCreateInputSourceList(filter as CFDictionary, true)?.takeRetainedValue() else { return [] }
    return result as! [TISInputSource]
}

func sourceWithID(_ id: String) throws -> TISInputSource {
    let sources = inputSources([kTISPropertyInputSourceID as String: id])
    guard sources.count == 1 else { throw Failure.invalid("input source ID not unique: " + id) }
    return sources[0]
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

func checkedWindows(_ app: AXUIElement) throws -> [AXUIElement]? {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value)
    if status == .cannotComplete { return nil }
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
        "q": 12, "2": 19, "comma": 43, "escape": 53, "return": 36, "tab": 48,
        "delete": 51, "space": 49, "left": 123, "right": 124, "home": 115, "end": 119, "f": 3]
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

func menu(_ app: AXUIElement, title: String, submenu: String? = nil) throws -> [String: Any] {
    guard let bar = attribute(app, kAXMenuBarAttribute) else { throw Failure.invalid("no native menu bar") }
    let barElement = bar as! AXUIElement
    let tops = elements(barElement, kAXChildrenAttribute)
    let matches = submenu.map { title in tops.filter { text($0, kAXTitleAttribute) == title } } ?? tops
    if submenu != nil && matches.count != 1 {
        trace("native menu titles: " + String(describing: tops.map { text($0, kAXTitleAttribute) }))
        throw Failure.invalid("requested native submenu is not unique")
    }
    for top in matches {
        // AppKit may populate submenu accessibility children only after the
        // menu opens. An explicit top-level menu is pressed once before lookup,
        // exactly as a user opens Edit, never by retrying the requested action.
        if submenu != nil {
            guard AXUIElementPerformAction(top, kAXPressAction as CFString) == .success else {
                throw Failure.invalid("cannot open requested native submenu")
            }
            Thread.sleep(forTimeInterval: 0.1)
        }
        var queue = elements(top, kAXChildrenAttribute)
        var items: [AXUIElement] = []
        var truncated = queue.count > 1000
        var next = 0
        while next < queue.count && next < 300 {
            let element = queue[next]
            next += 1
            if text(element, kAXTitleAttribute) == title {
                items.append(element)
                if submenu == nil { break }
            }
            let children = elements(element, kAXChildrenAttribute)
            let capacity = max(0, 1000 - queue.count)
            truncated = truncated || children.count > capacity
            queue.append(contentsOf: children.prefix(capacity))
        }
        if submenu != nil && (truncated || next < queue.count || items.count != 1) {
            throw Failure.invalid("requested native menu item is missing, ambiguous or traversal incomplete")
        }
        if let element = items.first {
            let before = describe(element)
            if submenu == nil {
                guard AXUIElementPerformAction(top, kAXPressAction as CFString) == .success else {
                    throw Failure.invalid("cannot open native menu")
                }
                Thread.sleep(forTimeInterval: 0.1)
            }
            let after = describe(element)
            guard AXUIElementPerformAction(element, kAXPressAction as CFString) == .success else {
                throw Failure.invalid("cannot activate native menu item")
            }
            return ["performed": true, "menu_item_before": before, "menu_item_after_open": after,
                "submenu_opened_before_lookup": submenu != nil]
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
    if ["key", "digits", "menu", "keycodes", "performance-input", "performance-navigation"].contains(operation), NSWorkspace.shared.frontmostApplication?.processIdentifier != pid {
        throw Failure.invalid("target app is not frontmost; native input refused")
    }
    switch operation {
    case "input-sources":
        // Exact French layout and the installed Japanese family only. No
        // unbounded all-installed input-source walk on a developer desktop.
        var sources = ["com.apple.inputmethod.Kotoeri.RomajiTyping", "com.apple.inputmethod.Kotoeri"]
            .flatMap { inputSources([kTISPropertyBundleID as String: $0]) }
        sources.append(try sourceWithID("com.apple.keylayout.French"))
        sources.append(TISCopyCurrentKeyboardInputSource().takeRetainedValue())
        guard sources.count <= 64 else { throw Failure.invalid("input-source inventory limit") }
        var seen = Set<String>()
        try output(["current": inputSource(), "sources": sources.compactMap { source -> [String: Any]? in
            let value = sourceDescription(source)
            guard let id = value["id"] as? String, seen.insert(id).inserted else { return nil }
            return value
        }])
    case "input-source":
        // This operation is reserved for the disposable hosted qualification
        // account. Normal local observer use cannot change keyboard settings.
        guard ProcessInfo.processInfo.environment["GITHUB_ACTIONS"] == "true",
              ProcessInfo.processInfo.environment["TASK2_INPUT_CI"] == "1",
              let id = request["source"] as? String else {
            throw Failure.invalid("input-source mutation requires the explicit hosted-CI route")
        }
        let source = try sourceWithID(id)
        let operation = request["operation"] as? String ?? ""
        let status: OSStatus
        switch operation {
        case "enable": status = TISEnableInputSource(source)
        case "disable": status = TISDisableInputSource(source)
        case "select": status = TISSelectInputSource(source)
        default: throw Failure.invalid("unknown input-source mutation")
        }
        guard status == noErr else { throw Failure.invalid("input-source operation failed: " + String(status)) }
        try output(["current": inputSource(), "source": sourceDescription(source)])
    case "input-method-status":
        guard let id = request["source"] as? String else { throw Failure.invalid("exact input mode required") }
        let source = try sourceWithID(id)
        guard inputProperty(source, kTISPropertyBundleID) as? String == "com.apple.inputmethod.Kotoeri.RomajiTyping" else {
            throw Failure.invalid("only the observed Japanese Roman-input family may be inspected")
        }
        // Apple's TIS source-family identifier and the application bundle ID
        // differ. Read both exact public identities, never launch/kill a service.
        let bundles = ["com.apple.inputmethod.Kotoeri.RomajiTyping", "com.apple.JapaneseIM.RomajiTyping"]
        let processes = bundles.flatMap { NSRunningApplication.runningApplications(withBundleIdentifier: $0) }
        guard processes.count <= 8 else { throw Failure.invalid("input-method process inventory limit") }
        try output(["current": inputSource(), "source": sourceDescription(source),
            "processes": processes.map { process in
                ["pid": process.processIdentifier, "bundle": process.bundleIdentifier ?? "",
                    "finished_launching": process.isFinishedLaunching, "terminated": process.isTerminated] as [String: Any]
            }] as [String: Any])
    case "keycodes":
        guard let codes = request["codes"] as? [Int], !codes.isEmpty, codes.count <= 32,
              codes.allSatisfy({ $0 >= 0 && $0 < 128 }) else {
            throw Failure.invalid("bounded physical key-code sequence required")
        }
        for code in codes {
            try post(CGKeyCode(code), down: true, flags: [])
            try post(CGKeyCode(code), down: false, flags: [])
        }
        try output(["posted": true, "source": inputSource()])
    case "performance-input":
        guard let directory = request["directory"] as? String, directory.hasPrefix("/"),
              ProcessInfo.processInfo.environment["GITHUB_ACTIONS"] == "true" else {
            throw Failure.invalid("performance input requires an owned CI rendezvous")
        }
        let stateURL = URL(fileURLWithPath: directory).appendingPathComponent("state.txt")
        var samples: [Double] = []
        for sample in 0..<64 {
            guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
                throw Failure.invalid("performance input lost foreground target")
            }
            let expected = sample % 2 == 0 ? "x" : ""
            let code = CGKeyCode(sample % 2 == 0 ? 7 : 51)
            let start = DispatchTime.now().uptimeNanoseconds
            for down in [true, false] {
                guard let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else {
                    throw Failure.invalid("cannot create performance keyboard event")
                }
                event.post(tap: .cghidEventTap)
            }
            while true {
                let value = try String(contentsOf: stateURL, encoding: .utf8).components(separatedBy: "\n")
                let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
                guard elapsed < 1000 else { throw Failure.invalid("native input acknowledgment deadline") }
                if value.count == 5 && value[0] == expected {
                    samples.append(elapsed)
                    break
                }
                Thread.sleep(forTimeInterval: 0.001)
            }
        }
        try output(["samples_ms": samples, "p95_ms": samples.sorted()[60],
            "method": "pre-CGEvent-post to actual edited-query/model/20ms-rendezvous acknowledgment"])
    case "performance-navigation":
        guard let directory = request["directory"] as? String, directory.hasPrefix("/"),
              ProcessInfo.processInfo.environment["GITHUB_ACTIONS"] == "true" else {
            throw Failure.invalid("navigation requires an owned CI rendezvous")
        }
        let root = URL(fileURLWithPath: directory)
        func geometry() throws -> [String: Any] {
            guard let value = try JSONSerialization.jsonObject(with: Data(contentsOf:
                root.appendingPathComponent("geometry.json"))) as? [String: Any] else {
                throw Failure.invalid("invalid actual navigation geometry")
            }
            return value
        }
        func pulse(_ code: CGKeyCode, reverse: Bool = false) throws {
            guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
                throw Failure.invalid("navigation lost foreground target")
            }
            func shift(_ down: Bool) throws {
                guard let event = CGEvent(keyboardEventSource: nil, virtualKey: 56, keyDown: down) else {
                    throw Failure.invalid("cannot create native Shift event")
                }
                event.type = .flagsChanged
                event.flags = down ? .maskShift : []
                event.post(tap: .cghidEventTap)
            }
            if reverse { try shift(true) }
            for down in [true, false] {
                guard let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else {
                    throw Failure.invalid("cannot create navigation key event")
                }
                event.flags = reverse ? .maskShift : []
                event.post(tap: .cghidEventTap)
            }
            if reverse { try shift(false) }
        }
        func awaitGeometry(_ predicate: ([String: Any]) -> Bool) throws -> [String: Any] {
            let start = DispatchTime.now().uptimeNanoseconds
            while true {
                let value = try geometry()
                if predicate(value) { return value }
                guard DispatchTime.now().uptimeNanoseconds - start < 1_000_000_000 else {
                    throw Failure.invalid("navigation state acknowledgment deadline")
                }
                Thread.sleep(forTimeInterval: 0.001)
            }
        }
        func focus(_ target: Int, reverse: Bool) throws {
            for _ in 0..<24 {
                let previous = try geometry()["focus_key"] as? Int
                if previous == target { return }
                try pulse(48, reverse: reverse)
                _ = try awaitGeometry { $0["focus_key"] as? Int != previous }
            }
            throw Failure.invalid("bounded native navigation focus not reached")
        }
        var samples: [Double] = []
        var endpoints: [[String: Any]] = []
        for sample in 0..<21 {
            guard try geometry()["focus_key"] as? Int == 30 else {
                throw Failure.invalid("Search must be natively focused before expansion")
            }
            try pulse(7) // French physical X; no fixture title contains x.
            _ = try awaitGeometry { $0["rows"] as? Int == 0 }
            let start = DispatchTime.now().uptimeNanoseconds
            try pulse(51)
            _ = try awaitGeometry { $0["rows"] as? Int == 1000 }
            try focus(31, reverse: false)
            try pulse(119)
            let end = try awaitGeometry { $0["rows"] as? Int == 1000
                && $0["selected_id"] as? Int == 999 && $0["selected_row"] as? Int == 999 }
            let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
            if sample > 0 { samples.append(elapsed); endpoints.append(end) }
            try pulse(115) // Real Home resets selection before the next trial.
            _ = try awaitGeometry { $0["selected_id"] as? Int == 0 }
            try focus(30, reverse: true)
        }
        try output(["samples_ms": samples, "p95_ms": samples.sorted()[18], "endpoints": endpoints,
            "method": "native Delete expands0→1000, then Tab/End selects999; one warmup plus20 samples"])
    case "input-selection":
        var raw: CFTypeRef?
        let focusStatus = AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute as CFString, &raw)
        if focusStatus != .success || raw == nil {
            // AccessKit requests its initial tree asynchronously when AppKit
            // first asks for accessible children/focus. Preserve the native
            // result and let the existing bounded reader poll; absence is
            // never a successful selection and no focus/selection is set.
            guard [.success, .noValue, .attributeUnsupported, .cannotComplete].contains(focusStatus) else {
                throw Failure.invalid("native input focus query failed: " + String(focusStatus.rawValue))
            }
            try output(["pid": -1, "frontmost_pid": NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1,
                "focus_status": focusStatus.rawValue, "focused": [:], "selection_observed": false,
                "activation_tree": tree(app, sheetsOnly: false)] as [String: Any])
            break
        }
        guard let raw, CFGetTypeID(raw) == AXUIElementGetTypeID() else {
            throw Failure.invalid("native input focus query returned an unexpected type")
        }
        let element = raw as! AXUIElement
        var focusedPID: pid_t = -1
        guard AXUIElementGetPid(element, &focusedPID) == .success, focusedPID == pid else {
            throw Failure.invalid("focused native input does not belong to the requested app")
        }
        var selected: CFTypeRef?
        let status = AXUIElementCopyAttributeValue(element, kAXSelectedTextRangeAttribute as CFString, &selected)
        var range = CFRange(location: -1, length: 0)
        var observed = false
        if status == .success, let selected, CFGetTypeID(selected) == AXValueGetTypeID() {
            let value = selected as! AXValue
            if AXValueGetType(value) == .cfRange { observed = AXValueGetValue(value, .cfRange, &range) }
        }
        try output(["pid": focusedPID, "frontmost_pid": NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1,
            "focus_status": focusStatus.rawValue, "focused": describe(element),
            "selection_observed": observed, "selection_status": status.rawValue,
            "selection_location": range.location, "selection_length": range.length,
            "selected_text": text(element, kAXSelectedTextAttribute)] as [String: Any])
    case "snapshot", "status":
        trace("reading owned CG windows and foreground status")
        var windows = (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]] ?? []).filter { ($0[kCGWindowOwnerPID as String] as? Int) == Int(pid) }
        var onlyWindow: Int?
        var documentObservation: [String: Any] = [:]
        if let document = request["document"] as? String {
            let target = URL(fileURLWithPath: document).resolvingSymlinksInPath().standardizedFileURL
            guard let nativeWindows = try checkedWindows(app) else {
                // A busy Finder may temporarily reject the read. Preserve the
                // exact result for the existing bounded read-only poll, never
                // as an empty successful inventory or a closed-window witness.
                try output(["pid": pid, "nodes": [], "windows": [],
                    "document_window_query_status": AXError.cannotComplete.rawValue,
                    "document_window_list_observed": false,
                    "document_windows_observed": false] as [String: Any])
                exit(0)
            }
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
    case "display-preferences":
        try output(["reduce_motion": NSWorkspace.shared.accessibilityDisplayShouldReduceMotion,
            "increase_contrast": NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast,
            "screens": NSScreen.screens.map { ["scale": $0.backingScaleFactor,
                "width": $0.frame.width, "height": $0.frame.height] }])
    case "resize":
        let windows = elements(app, kAXWindowsAttribute)
        guard windows.count == 1, let width = request["width"] as? Double,
              let height = request["height"] as? Double,
              let oldWidth = request["content_width"] as? Double,
              let oldHeight = request["content_height"] as? Double,
              (560...1600).contains(width), (400...1000).contains(height),
              let raw = attribute(windows[0], kAXSizeAttribute), CFGetTypeID(raw) == AXValueGetTypeID() else {
            throw Failure.invalid("one owned window and bounded display dimensions required")
        }
        let value = raw as! AXValue
        var old = CGSize.zero
        guard AXValueGetValue(value, .cgSize, &old) else { throw Failure.invalid("native size unavailable") }
        var size = CGSize(width: width + old.width - oldWidth, height: height + old.height - oldHeight)
        guard let updated = AXValueCreate(.cgSize, &size),
              AXUIElementSetAttributeValue(windows[0], kAXSizeAttribute as CFString, updated) == .success else {
            throw Failure.invalid("native window resize failed")
        }
        try output(["performed": true, "old_outer_width": old.width, "old_outer_height": old.height,
            "requested_outer_width": size.width, "requested_outer_height": size.height])
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
        try output(menu(app, title: request["title"] as? String ?? "", submenu: request["submenu"] as? String))
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
