// [DEBUG-task17-input] Diagnostic-only insert into a private copy of the retained helper.
// The normal gate/helper remains unchanged. Never execute outside disposable native CI.
import Darwin

func probeCPU() -> UInt64 {
    var value = timespec()
    precondition(clock_gettime(CLOCK_THREAD_CPUTIME_ID, &value) == 0)
    return UInt64(value.tv_sec) * 1_000_000_000 + UInt64(value.tv_nsec)
}
var probeReads: [[String: Any]] = []
var probeEnabled = false

final class ProbeAX {
    let lock = NSLock()
    var observations: [[String: Any]] = []
    var status: String = "not started"
    var observer: AXObserver?
    func append(_ element: AXUIElement, _ notification: CFString) {
        let before = DispatchTime.now().uptimeNanoseconds
        var value: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(element, kAXValueAttribute as CFString, &value)
        let after = DispatchTime.now().uptimeNanoseconds
        lock.lock(); defer { lock.unlock() }
        observations.append(["notification": notification as String, "received_ns": before,
                             "value_observed_ns": after, "value": value as? String ?? "<unavailable>",
                             "value_result": result.rawValue])
    }
    func start(_ pid: pid_t) {
        let ready = DispatchSemaphore(value: 0)
        Thread.detachNewThread { [self] in
            defer { ready.signal() }
            let app = AXUIElementCreateApplication(pid)
            var focused: CFTypeRef?
            let result = AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute as CFString, &focused)
            guard result == .success, let focused, CFGetTypeID(focused) == AXUIElementGetTypeID() else {
                status = "focused AX element unavailable: \(result.rawValue)"; return
            }
            let element = focused as! AXUIElement
            let created = AXObserverCreate(pid, { _, element, name, refcon in
                guard let refcon else { return }
                Unmanaged<ProbeAX>.fromOpaque(refcon).takeUnretainedValue().append(element, name)
            }, &observer)
            guard created == .success, let observer else { status = "AX observer creation: \(created.rawValue)"; return }
            let added = AXObserverAddNotification(observer, element, kAXValueChangedNotification as CFString,
                                                  Unmanaged.passUnretained(self).toOpaque())
            guard added == .success else { status = "AX notification subscription: \(added.rawValue)"; return }
            status = "subscribed focused search AXValueChanged"
            CFRunLoopAddSource(CFRunLoopGetCurrent(), AXObserverGetRunLoopSource(observer), .defaultMode)
            ready.signal()
            CFRunLoopRunInMode(.defaultMode, 30, false)
        }
        _ = ready.wait(timeout: .now() + 5)
    }
    func receipt() -> [String: Any] {
        lock.lock(); defer { lock.unlock() }
        return ["status": status, "events": observations]
    }
}
let probeAX = ProbeAX()
var probeSamples: [[String: Any]] = []
let probeSampler = Process()

func probeStart() {
    probeAX.start(pid)
    probeSampler.executableURL = URL(fileURLWithPath: "/usr/bin/sample")
    probeSampler.arguments = [String(pid), "6", "1", "-file", directory.appendingPathComponent("keyboard-stacks.txt").path]
    let log = directory.appendingPathComponent("keyboard-sampler.log")
    FileManager.default.createFile(atPath: log.path, contents: nil)
    if let handle = try? FileHandle(forWritingTo: log) {
        probeSampler.standardOutput = handle; probeSampler.standardError = handle
        try? probeSampler.run()
    }
    probeEnabled = true
}
func probeEnd() throws {
    probeEnabled = false
    let end = DispatchTime.now().uptimeNanoseconds
    let result: [String: Any] = ["scope": "diagnostic only; includes probe overhead; not product qualification",
        "keyboard_end_ns": end, "samples": probeSamples, "ax": probeAX.receipt(),
        "sampling_window": "6 seconds from keyboard start; may overlap pointer phase; no per-event stack attribution"]
    try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]).write(
        to: directory.appendingPathComponent("keyboard-probe.json"), options: .atomic)
}
