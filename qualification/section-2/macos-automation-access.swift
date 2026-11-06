// Read-only preflight for the external native observer. Never request TCC grants.
import ApplicationServices
import CoreGraphics
import Foundation

let session = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
let report: [String: Any] = [
    "schema": 1,
    "accessibility_trusted": AXIsProcessTrusted(),
    "event_posting_allowed": CGPreflightPostEventAccess(),
    "screen_capture_allowed": CGPreflightScreenCaptureAccess(),
    "session_available": !session.isEmpty,
    "on_console": session["kCGSessionOnConsoleKey"] as? Bool ?? false,
    "login_complete": session["kCGSessionLoginDoneKey"] as? Bool ?? false,
    "native_interaction_qualified": false,
    "privacy_permissions_requested_or_changed": false,
    "scope": "Read-only permissions inventory for this observer process; no UI or physical input result.",
]
let bytes = try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(bytes)
FileHandle.standardOutput.write(Data([10]))
