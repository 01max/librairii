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
    for error in [AXError.invalidUIElement, .cannotComplete, .attributeUnsupported] {
        scalarFixture[kAXWindowsAttribute] = encodedError(error)
        let beforeRoots = scalarReads
        do { _ = try tree(element, sheetsOnly: false)
            throw TestFailure.mismatch("failed window enumeration became an empty snapshot") }
        catch is Failure { }
        try require(scalarReads == beforeRoots + (error == .invalidUIElement ? 2 : 1),
            "root enumeration retry was unbounded or retried a communication error")
    }
    scalarFixture[kAXWindowsAttribute] = [oldRoot] as [AXUIElement]
    scalarFixture[kAXChildrenAttribute] = encodedError(.failure)
    do { _ = try tree(element, sheetsOnly: false)
        throw TestFailure.mismatch("failed sheet enumeration became an empty snapshot") }
    catch is Failure { }
    try require(performedActions == 0, "snapshot rebuild performed a native action")
    #endif
    print("headless AX snapshot fixtures passed; inert scalar fallback reads=\(scalarReads)")
} catch {
    FileHandle.standardError.write(Data((String(describing: error) + "\n").utf8))
    exit(1)
}
