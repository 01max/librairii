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
var probeKeys: [[String: Any]] = []

final class ProbeAX {
    let lock = NSLock()
    var observations: [[String: Any]] = []
    private var status: String = "not started"
    private var stopRequested = false
    private var subscribed = false
    private var launched = false
    private var joined = false
    private let finished = DispatchSemaphore(value: 0)
    var observer: AXObserver?
    func setStatus(_ value: String) {
        lock.lock(); defer { lock.unlock() }; status = value
    }
    func shouldStop() -> Bool {
        lock.lock(); defer { lock.unlock() }; return stopRequested
    }
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
        lock.lock(); launched = true; lock.unlock()
        Thread.detachNewThread { [self] in
            defer { ready.signal(); finished.signal() }
            let app = AXUIElementCreateApplication(pid)
            AXUIElementSetMessagingTimeout(app, 2)
            var focused: CFTypeRef?
            let result = AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute as CFString, &focused)
            guard result == .success, let focused, CFGetTypeID(focused) == AXUIElementGetTypeID() else {
                setStatus("focused AX element unavailable: \(result.rawValue)"); return
            }
            let element = focused as! AXUIElement
            let created = AXObserverCreate(pid, { _, element, name, refcon in
                guard let refcon else { return }
                Unmanaged<ProbeAX>.fromOpaque(refcon).takeUnretainedValue().append(element, name)
            }, &observer)
            guard created == .success, let observer else { setStatus("AX observer creation: \(created.rawValue)"); return }
            let added = AXObserverAddNotification(observer, element, kAXValueChangedNotification as CFString,
                                                  Unmanaged.passUnretained(self).toOpaque())
            guard added == .success else { setStatus("AX notification subscription: \(added.rawValue)"); return }
            lock.lock(); subscribed = true; status = "subscribed focused search AXValueChanged"; lock.unlock()
            CFRunLoopAddSource(CFRunLoopGetCurrent(), AXObserverGetRunLoopSource(observer), .defaultMode)
            ready.signal()
            while !shouldStop() { CFRunLoopRunInMode(.defaultMode, 0.1, false) }
            AXObserverRemoveNotification(observer, element, kAXValueChangedNotification as CFString)
            CFRunLoopRemoveSource(CFRunLoopGetCurrent(), AXObserverGetRunLoopSource(observer), .defaultMode)
        }
        if ready.wait(timeout: .now() + 5) == .timedOut { setStatus("AX readiness timeout") }
    }
    func stop() -> Bool {
        lock.lock(); stopRequested = true; let started = launched; lock.unlock()
        let stopped = !started || finished.wait(timeout: .now() + 5) == .success
        lock.lock(); joined = stopped; lock.unlock()
        return stopped
    }
    func receipt() -> [String: Any] {
        lock.lock(); defer { lock.unlock() }
        return ["status": status, "subscribed": subscribed, "joined": joined, "events": observations]
    }
}
let probeAX = ProbeAX()
var probeSamples: [[String: Any]] = []
let probeSampler = Process()
var probeStarted = false
var probeEnded = false
var probeSamplerReceipt: [String: Any] = [:]

func probeStart() {
    probeStarted = true
    probeAX.start(pid)
    probeSampler.executableURL = URL(fileURLWithPath: "/usr/bin/sample")
    probeSampler.arguments = [String(pid), "6", "1", "-file", directory.appendingPathComponent("keyboard-stacks.txt").path]
    let log = directory.appendingPathComponent("keyboard-sampler.log")
    FileManager.default.createFile(atPath: log.path, contents: nil)
    do {
        let handle = try FileHandle(forWritingTo: log)
        probeSampler.standardOutput = handle; probeSampler.standardError = handle
        try probeSampler.run()
        probeSamplerReceipt = ["launched": true, "pid": probeSampler.processIdentifier,
                               "start_ns": DispatchTime.now().uptimeNanoseconds]
    } catch {
        probeSamplerReceipt = ["launched": false, "error": String(describing: error)]
    }
    probeEnabled = true
}
func probeEnd() throws {
    guard probeStarted && !probeEnded else { return }
    probeEnded = true
    probeEnabled = false
    let end = DispatchTime.now().uptimeNanoseconds
    let axStopped = probeAX.stop()
    var samplerStopped = probeSamplerReceipt["launched"] as? Bool != true
    if !samplerStopped {
        if probeSampler.isRunning { probeSampler.interrupt() }
        let deadline = Date().addingTimeInterval(5)
        while probeSampler.isRunning && Date() < deadline { Thread.sleep(forTimeInterval: 0.01) }
        if probeSampler.isRunning {
            probeSamplerReceipt["kill_result"] = kill(probeSampler.processIdentifier, SIGKILL)
            let killDeadline = Date().addingTimeInterval(2)
            while probeSampler.isRunning && Date() < killDeadline { Thread.sleep(forTimeInterval: 0.01) }
        }
        samplerStopped = !probeSampler.isRunning
        if samplerStopped {
            probeSampler.waitUntilExit()
            probeSamplerReceipt["exit_status"] = probeSampler.terminationStatus
            probeSamplerReceipt["exit_reason"] = probeSampler.terminationReason.rawValue
        } else { probeSamplerReceipt["error"] = "owned sampler did not settle after SIGINT/SIGKILL" }
        probeSamplerReceipt["end_ns"] = DispatchTime.now().uptimeNanoseconds
    }
    probeSamplerReceipt["joined"] = samplerStopped
    let ax = probeAX.receipt()
    let stacks = directory.appendingPathComponent("keyboard-stacks.txt")
    let stackBytes = (try? Data(contentsOf: stacks).count) ?? 0
    let result: [String: Any] = ["scope": "diagnostic only; includes probe overhead; not product qualification",
        "observer_cpu_scope": "CLOCK_THREAD_CPUTIME_ID measures the Swift polling thread, not the native product",
        "keyboard_end_ns": end, "samples": probeSamples, "ax": ax, "sampler": probeSamplerReceipt,
        "stack_bytes": stackBytes, "cleanup_complete": axStopped && samplerStopped,
        "probes_complete": ax["subscribed"] as? Bool == true && probeSamplerReceipt["launched"] as? Bool == true && stackBytes > 0,
        "sampling_window": "at most 6 seconds from keyboard start, stopped/joined before pointer; includes launch/cleanup; no per-event stack attribution"]
    try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]).write(
        to: directory.appendingPathComponent("keyboard-probe.json"), options: .atomic)
    if !axStopped || !samplerStopped { throw Refusal(message: "diagnostic probe retirement incomplete") }
}
