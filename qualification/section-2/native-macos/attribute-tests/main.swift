// Headless fixture: no accessibility reads, native input or application launch.
// These module-local functions replace the imported AX read functions in this
// test executable only. PID 0 is an inert handle, never a desktop application.
import AppKit
import ApplicationServices
import Foundation

enum TestFailure: Error { case mismatch(String) }
func require(_ value: Bool, _ message: String) throws {
    if !value { throw TestFailure.mismatch(message) }
}

var scalarReads = 0
var batchReads = 0
var actionReads = 0
var settableReads = 0
var batchStatus = AXError.success
var wrongLength = false
var requestedNames: [String] = []
var fixture: [String: Any] = [:]
var scalarFixture: [String: Any] = [:]
var batchFixture: ((AXUIElement) -> Void)?
var scalarReadFixture: ((AXUIElement, String) -> Void)?
var performedActions = 0

func AXUIElementPerformAction(_ element: AXUIElement, _ action: CFString) -> AXError {
    performedActions += 1
    return .success
}

func encodedError(_ error: AXError) -> AXValue {
    var copy = error
    return AXValueCreate(.axError, &copy)!
}

func AXUIElementCopyAttributeValue(_ element: AXUIElement, _ name: CFString,
                                  _ value: UnsafeMutablePointer<CFTypeRef?>) -> AXError {
    scalarReads += 1
    scalarReadFixture?(element, name as String)
    guard let observed = scalarFixture[name as String] ?? fixture[name as String] else { return .noValue }
    if CFGetTypeID(observed as CFTypeRef) == CFNullGetTypeID() { return .noValue }
    if CFGetTypeID(observed as CFTypeRef) == AXValueGetTypeID(),
       AXValueGetType(observed as! AXValue) == .axError {
        var error = AXError.success
        _ = AXValueGetValue(observed as! AXValue, .axError, &error)
        return error
    }
    value.pointee = observed as CFTypeRef
    return .success
}

func AXUIElementCopyMultipleAttributeValues(_ element: AXUIElement, _ names: CFArray,
    _ options: AXCopyMultipleAttributeOptions, _ values: UnsafeMutablePointer<CFArray?>) -> AXError {
    batchReads += 1
    batchFixture?(element)
    requestedNames = names as! [String]
    let observed = requestedNames.map { fixture[$0] ?? encodedError(.attributeUnsupported) }
    values.pointee = (wrongLength ? Array(observed.dropLast()) : observed) as CFArray
    return batchStatus
}

func AXUIElementCopyActionNames(_ element: AXUIElement, _ values: UnsafeMutablePointer<CFArray?>) -> AXError {
    actionReads += 1
    values.pointee = ["AXPress"] as CFArray
    return .success
}

func AXUIElementIsAttributeSettable(_ element: AXUIElement, _ name: CFString,
                                  _ value: UnsafeMutablePointer<DarwinBoolean>) -> AXError {
    settableReads += 1
    value.pointee = false
    return .success
}

do {
    let element = AXUIElementCreateApplication(0)
    let url = URL(fileURLWithPath: "/owned-fixture/Été")
    fixture = [kAXRoleAttribute: "AXButton", kAXSubroleAttribute: "AXUnknown",
        kAXTitleAttribute: "Retrouver la même bibliothèque…", kAXDescriptionAttribute: "",
        kAXIdentifierAttribute: "fixture", kAXValueAttribute: encodedError(.noValue),
        kAXDocumentAttribute: url as CFURL, kAXURLAttribute: url as CFURL,
        kAXEnabledAttribute: true, kAXSelectedAttribute: false, kAXModalAttribute: false,
        kAXFocusedAttribute: false, kAXChildrenAttribute: [] as [AXUIElement]]
    let node = try describe(element)
    try require(batchReads == 1 && scalarReads == 0 && actionReads == 1 && settableReads == 1,
        "snapshot RPC count: batch=\(batchReads) scalar=\(scalarReads) action=\(actionReads) settable=\(settableReads)")
    try require(node["title"] as? String == "Retrouver la même bibliothèque…", "UTF-8 title changed")
    try require(node["document"] as? String == url.absoluteString && node["url"] as? String == url.absoluteString,
        "URL conversion changed")
    try require(node["selected"] as? Bool == false && node["enabled"] as? Bool == true,
        "false or true boolean lost")
    try require(node["value"] as? String == "" && node["actions"] as? [String] == ["AXPress"]
        && node["value_settable"] as? Bool == false, "optional value, actions or settability changed")

    #if !LEGACY_SNAPSHOT_TEST
    try require(requestedNames.count == 12 && !requestedNames.contains(kAXChildrenAttribute),
        "nontraversal read requested children")
    let attributes = try snapshotAttributes(element, includingChildren: true)
    try require(requestedNames.count == 13 && requestedNames.last == kAXChildrenAttribute,
        "child traversal was not batched in order")
    try require((attributes[kAXChildrenAttribute] as? [AXUIElement])?.isEmpty == true,
        "empty native child inventory changed")
    // AppKit's Go to Folder sheet returns a generic batch error for AXValue.
    // Read that attribute directly before deciding whether a value is absent.
    fixture[kAXValueAttribute] = encodedError(.failure)
    scalarFixture[kAXValueAttribute] = "Exact folder text"
    let scalarBefore = scalarReads
    let recovered = try describe(element)
    try require(recovered["value"] as? String == "Exact folder text" && scalarReads == scalarBefore + 1,
        "generic batch failure did not preserve the directly observed value")
    for error in [AXError.failure, .attributeUnsupported, .noValue] {
        scalarFixture[kAXValueAttribute] = encodedError(error)
        let absent = try describe(element)
        try require(absent["value"] as? String == "", "optional direct-read failure changed")
    }
    for error in [AXError.cannotComplete, .invalidUIElement, .illegalArgument, .apiDisabled, .notImplemented] {
        scalarFixture[kAXValueAttribute] = encodedError(error)
        do { _ = try describe(element); throw TestFailure.mismatch("direct-read communication failure accepted") }
        catch is Failure { }
    }
    scalarFixture.removeAll()
    fixture[kAXValueAttribute] = encodedError(.noValue)
    fixture[kAXChildrenAttribute] = encodedError(.failure)
    do { _ = try snapshotAttributes(element, includingChildren: true)
        throw TestFailure.mismatch("unavailable children silently truncated traversal") }
    catch Failure.invalid(_) { }
    fixture[kAXChildrenAttribute] = [] as [AXUIElement]
    fixture[kAXEnabledAttribute] = NSNull()
    fixture[kAXTitleAttribute] = encodedError(.attributeUnsupported)
    let optional = try describe(element)
    try require(optional["enabled"] == nil && optional["title"] as? String == "",
        "CFNull or unsupported optional field changed")
    for error in [AXError.cannotComplete, .invalidUIElement, .illegalArgument] {
        fixture[kAXTitleAttribute] = encodedError(error)
        do { _ = try describe(element); throw TestFailure.mismatch("attribute communication failure accepted") }
        catch is Failure { }
    }
    fixture[kAXTitleAttribute] = "Recovery"
    batchStatus = .cannotComplete
    do { _ = try describe(element); throw TestFailure.mismatch("batch communication failure accepted") }
    catch Failure.invalid(_) { }
    batchStatus = .success
    wrongLength = true
    do { _ = try describe(element); throw TestFailure.mismatch("truncated positional batch accepted") }
    catch Failure.invalid(_) { }
    wrongLength = false
    let oldRoot = AXUIElementCreateApplication(1)
    let oldChild = AXUIElementCreateApplication(2)
    let newRoot = AXUIElementCreateApplication(3)
    let newChild = AXUIElementCreateApplication(4)
    scalarFixture[kAXWindowsAttribute] = [oldRoot] as [AXUIElement]
    fixture[kAXChildrenAttribute] = [] as [AXUIElement]
    var reorderedWindows = false
    batchFixture = { node in
        batchStatus = .success
        fixture = [kAXRoleAttribute: "AXStaticText", kAXTitleAttribute: "new child",
                   kAXChildrenAttribute: [] as [AXUIElement]]
        if CFEqual(node, oldRoot) {
            fixture[kAXTitleAttribute] = "old root"
            fixture[kAXChildrenAttribute] = [oldChild] as [AXUIElement]
        } else if CFEqual(node, oldChild) {
            batchStatus = .invalidUIElement
            scalarFixture[kAXWindowsAttribute] = (reorderedWindows ? [newRoot, oldRoot] : [newRoot]) as [AXUIElement]
        } else if CFEqual(node, newRoot) {
            fixture[kAXTitleAttribute] = "new root"
            fixture[kAXChildrenAttribute] = [newChild] as [AXUIElement]
        }
    }
    let beforeTree = batchReads
    let rebuilt = try tree(element, sheetsOnly: false)
    try require(rebuilt.count == 2 && rebuilt.map { $0["title"] as? String } == ["new root", "new child"],
        "stale snapshot retained partial nodes or failed to refresh roots")
    try require(batchReads == beforeTree + 4,
        "stale snapshot was not rebuilt exactly once")
    let paths = rebuilt.compactMap { ($0["selector"] as? [String: Any])?["path"] as? [Int] }
    try require(paths == [[], [0]], "fresh child indices changed")
    reorderedWindows = true
    scalarFixture[kAXWindowsAttribute] = [oldRoot, newRoot] as [AXUIElement]
    fixture[kAXChildrenAttribute] = [] as [AXUIElement]
    let beforeFiltered = batchReads
    do { _ = try tree(element, onlyWindow: 0, sheetsOnly: false)
        throw TestFailure.mismatch("filtered snapshot reused a window index after reordering") }
    catch Failure.staleSnapshot(_) { }
    try require(batchReads == beforeFiltered + 2,
        "filtered snapshot re-enumerated a previously selected window")
    for staleStage in ["candidate-tree", "candidate-description"] {
        scalarFixture[kAXWindowsAttribute] = [oldRoot, newRoot] as [AXUIElement]
        let beforeCandidate = batchReads
        let candidate = try snapshotReadReply(operation: "snapshot", pid: 395,
            document: "/owned/Été 日本語/export") {
            // The production wrapper includes both the URL-selected tree and
            // Finder's title-candidate traversal/description before any output.
            if staleStage == "candidate-tree" {
                return ["owned_candidate_diagnostic_tree": try tree(element, onlyWindow: 0, sheetsOnly: false)]
            }
            return ["owned_title_candidates": [try describe(oldChild)]]
        }
        try require(candidate.exitCode == 75 && candidate.value.count == 5
            && candidate.value["document"] as? String == "/owned/Été 日本語/export"
            && candidate.value["nodes"] == nil && candidate.value["owned_title_candidates"] == nil
            && candidate.value["owned_candidate_diagnostic_tree"] == nil,
            "stale Finder candidate escaped or retained partial observation")
        try require(batchReads == beforeCandidate + (staleStage == "candidate-tree" ? 2 : 1),
            "document construction retried a stale selected index")
    }
    let completeDocument = try snapshotReadReply(operation: "snapshot", pid: 395,
        document: "/owned/Été 日本語/export") { ["nodes": ["complete"]] }
    try require(completeDocument.exitCode == 0 && completeDocument.value["nodes"] as? [String] == ["complete"],
        "complete document observation changed")
    do { _ = try snapshotReadReply(operation: "snapshot", pid: 395,
        document: "/owned/Été 日本語/export") { throw Failure.invalid("communication error") }
        throw TestFailure.mismatch("document construction swallowed another failure") }
    catch Failure.invalid(_) { }
    for error in [AXError.invalidUIElement, .cannotComplete, .illegalArgument] {
        scalarFixture[kAXWindowsAttribute] = [oldRoot] as [AXUIElement]
        fixture[kAXChildrenAttribute] = [] as [AXUIElement]
        batchFixture = { _ in batchStatus = error }
        let beforeFailure = batchReads
        do { _ = try tree(element, sheetsOnly: false)
            throw TestFailure.mismatch("persistent snapshot error accepted") }
        catch is Failure { }
        try require(batchReads == beforeFailure + (error == .invalidUIElement ? 2 : 1),
            "snapshot retry was unbounded or retried a communication/argument error")
    }
    batchFixture = nil
    scalarFixture.removeAll()
    fixture = [kAXRoleAttribute: "AXWindow", kAXTitleAttribute: "ready root",
               kAXChildrenAttribute: [] as [AXUIElement]]
    batchStatus = .success
    var rootReads = 0
    scalarReadFixture = { _, name in
        if name == kAXWindowsAttribute {
            rootReads += 1
            scalarFixture[name] = rootReads == 1 ? encodedError(.cannotComplete) : [oldRoot] as [AXUIElement]
        }
    }
    let available = try tree(element, sheetsOnly: false)
    try require(rootReads == 2 && available.count == 1 && available[0]["title"] as? String == "ready root",
        "temporarily unavailable root did not produce a complete fresh snapshot")
    rootReads = 0
    batchFixture = { _ in batchStatus = .invalidUIElement }
    do { _ = try tree(element, sheetsOnly: false)
        throw TestFailure.mismatch("mixed root/stale errors exceeded the shared retry budget") }
    catch Failure.staleSnapshot(_) { }
    try require(rootReads == 2, "mixed errors acquired more than two root sets")
    batchFixture = nil
    batchStatus = .success
    scalarReadFixture = nil
    for error in [AXError.invalidUIElement, .cannotComplete, .attributeUnsupported] {
        scalarFixture[kAXWindowsAttribute] = encodedError(error)
        let beforeRoots = scalarReads
        do { _ = try tree(element, sheetsOnly: false)
            throw TestFailure.mismatch("failed window enumeration became an empty snapshot") }
        catch is Failure { }
        try require(scalarReads == beforeRoots + (error == .invalidUIElement || error == .cannotComplete ? 2 : 1),
            "root enumeration retry was unbounded or retried a communication error")
    }
    scalarFixture[kAXWindowsAttribute] = encodedError(.cannotComplete)
    let beforeFilteredRoot = scalarReads
    do { _ = try tree(element, onlyWindow: 0, sheetsOnly: false)
        throw TestFailure.mismatch("filtered root unavailability was retried") }
    catch is Failure { }
    try require(scalarReads == beforeFilteredRoot + 1,
        "filtered root query was repeated with a previously selected index")
    scalarFixture[kAXWindowsAttribute] = [oldRoot] as [AXUIElement]
    scalarFixture[kAXChildrenAttribute] = encodedError(.failure)
    do { _ = try tree(element, sheetsOnly: false)
        throw TestFailure.mismatch("failed sheet enumeration became an empty snapshot") }
    catch is Failure { }
    try require(performedActions == 0, "snapshot rebuild performed a native action")
    let pending = unavailableSnapshotReply(Failure.unavailableRoot("AXWindows: -25204"),
        operation: "snapshot", pid: 123, filtered: false)
    guard let pending, let wire = try JSONSerialization.jsonObject(
        with: JSONSerialization.data(withJSONObject: pending)) as? [String: Any] else {
        throw TestFailure.mismatch("missing unavailable root wire receipt")
    }
    try require(wire["format"] as? String == "librairii.native-snapshot-unavailable"
        && wire["pid"] as? Int == 123 && wire["status"] as? Int == -25204
        && wire["attribute"] as? String == kAXWindowsAttribute
        && wire.count == 5 && wire["nodes"] == nil && wire["windows"] == nil,
        "unavailable root was represented as successful or partial data")
    try require(unavailableSnapshotReply(Failure.unavailableRoot("AXWindows: -25204"),
        operation: "snapshot-window", pid: 123, filtered: false) != nil,
        "unfiltered window root unavailability lost its typed receipt")
    for (operation, filtered, pid) in [("press", false, 123), ("status", false, 123),
        ("snapshot", true, 123), ("snapshot", false, 0)] {
        try require(unavailableSnapshotReply(Failure.unavailableRoot("AXWindows: -25204"),
            operation: operation, pid: pid, filtered: filtered) == nil,
            "pending root protocol admitted an action, filtered read or invalid identity")
    }
    scalarFixture.removeAll()
    scalarFixture[kAXWindowsAttribute] = [oldRoot] as [AXUIElement]
    fixture = [kAXRoleAttribute: "AXWindow", kAXChildrenAttribute: [] as [AXUIElement]]
    batchFixture = { _ in batchStatus = .invalidUIElement }
    let beforePendingTree = batchReads
    let pendingTree = try snapshotReadReply(operation: "snapshot", pid: 8519, document: nil) {
        ["nodes": try tree(element, sheetsOnly: false)]
    }
    try require(pendingTree.exitCode == 75 && batchReads == beforePendingTree + 2,
        "exhausted stale tree did not retain the two-attempt helper bound")
    guard let staleWire = try JSONSerialization.jsonObject(
        with: JSONSerialization.data(withJSONObject: pendingTree.value)) as? [String: Any] else {
        throw TestFailure.mismatch("missing unavailable tree wire receipt")
    }
    try require(staleWire["format"] as? String == "librairii.native-tree-snapshot-unavailable"
        && staleWire["schema"] as? Int == 1 && staleWire["pid"] as? Int == 8519
        && staleWire["status"] as? Int == -25202 && staleWire["scope"] as? String == "unfiltered"
        && staleWire.count == 5 && staleWire["nodes"] == nil && staleWire["windows"] == nil,
        "exhausted tree retained partial data or lost its exact identity")
    batchFixture = nil
    batchStatus = .success
    let freshTree = try snapshotReadReply(operation: "snapshot", pid: 8519, document: nil) {
        ["nodes": try tree(element, sheetsOnly: false)]
    }
    try require(freshTree.exitCode == 0 && (freshTree.value["nodes"] as? [[String: Any]])?.count == 1
        && performedActions == 0, "fresh tree failed or pending observation performed an action")
    for operation in ["snapshot", "snapshot-window"] {
        try require(unavailableSnapshotReply(Failure.staleSnapshot("batch: -25202"),
            operation: operation, pid: 8519, filtered: false) != nil,
            "unfiltered snapshot operation lost its typed stale receipt")
    }
    for (operation, filtered, pid) in [("press", false, 8519), ("status", false, 8519),
        ("snapshot", true, 8519), ("snapshot-window", true, 8519), ("snapshot", false, 0)] {
        try require(unavailableSnapshotReply(Failure.staleSnapshot("batch: -25202"),
            operation: operation, pid: pid, filtered: filtered) == nil,
            "pending stale tree admitted an action, selected index or invalid identity")
    }
    for error in [Failure.invalid("AXChildren: -25204")] {
        try require(unavailableSnapshotReply(error, operation: "snapshot", pid: 123, filtered: false) == nil,
            "pending root protocol admitted a different failure")
    }
    let document = "/owned/Été 日本語/export"
    guard let pendingDocument = unavailableDocumentSnapshotReply(Failure.staleSnapshot("batch: -25202"),
        operation: "snapshot", pid: 395, document: document), let documentWire = try JSONSerialization.jsonObject(
        with: JSONSerialization.data(withJSONObject: pendingDocument)) as? [String: Any] else {
        throw TestFailure.mismatch("missing unavailable document wire receipt")
    }
    try require(documentWire["format"] as? String == "librairii.native-document-snapshot-unavailable"
        && documentWire["schema"] as? Int == 1 && documentWire["pid"] as? Int == 395
        && documentWire["status"] as? Int == -25202 && documentWire["document"] as? String == document
        && documentWire.count == 5 && documentWire["nodes"] == nil && documentWire["windows"] == nil
        && documentWire["selector"] == nil && documentWire["window"] == nil,
        "unavailable document retained partial data or a selected window index")
    for (operation, pid, path) in [("press", 395, document), ("status", 395, document),
        ("snapshot-window", 395, document), ("snapshot", 0, document), ("snapshot", 395, "")] {
        try require(unavailableDocumentSnapshotReply(Failure.staleSnapshot("batch: -25202"),
            operation: operation, pid: pid, document: path) == nil,
            "pending document admitted an action, unbound read or invalid identity")
    }
    try require(unavailableDocumentSnapshotReply(Failure.staleSnapshot("batch: -25202"),
        operation: "snapshot", pid: 395, document: nil) == nil,
        "pending document admitted a missing document")
    for error in [Failure.invalid("batch: -25202"), .unavailableRoot("AXWindows: -25204")] {
        try require(unavailableDocumentSnapshotReply(error, operation: "snapshot", pid: 395,
            document: document) == nil, "pending document admitted a different failure")
    }
    #endif
    print("headless AX snapshot fixtures passed; inert scalar fallback reads=\(scalarReads)")
} catch {
    FileHandle.standardError.write(Data((String(describing: error) + "\n").utf8))
    exit(1)
}
