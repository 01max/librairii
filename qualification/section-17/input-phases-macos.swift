// [DEBUG-task17-phases] Private helper additions; no stack sampler or AX calls.
var phaseCalibrations: [[String: Any]] = []
var phaseInputs: [[String: Any]] = []
var phaseKeys: [[String: Any]] = []
var phaseEnabled = false
var phaseFinished = false
func phaseCalibrate() throws {
    for sequence in 1...64 {
        let before = DispatchTime.now().uptimeNanoseconds
        try Data(String(sequence).utf8).write(to: directory.appendingPathComponent("clock-request"), options: .atomic)
        let deadline = before + 2_000_000_000
        while true {
            if let data = try? Data(contentsOf: directory.appendingPathComponent("clock-response")),
               let response = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               response["sequence"] as? Int == sequence, let native = response["native_ns"] as? UInt64 {
                let after = DispatchTime.now().uptimeNanoseconds
                guard before >= native else { throw Refusal(message: "native clock origin must follow host boot") }
                phaseCalibrations.append(["sequence": sequence, "before_ns": before, "native_ns": native, "after_ns": after,
                    "offset_lower_ns": before - native, "offset_upper_ns": after - native])
                break
            }
            if DispatchTime.now().uptimeNanoseconds > deadline { throw Refusal(message: "clock calibration deadline") }
            Thread.sleep(forTimeInterval: 0.001)
        }
    }
    phaseEnabled = true
}
func phaseFinish() throws {
    guard !phaseFinished else { return }; phaseFinished = true; phaseEnabled = false
    let lower = phaseCalibrations.compactMap { $0["offset_lower_ns"] as? UInt64 }.max() ?? 0
    let upper = phaseCalibrations.compactMap { $0["offset_upper_ns"] as? UInt64 }.min() ?? 0
    let result: [String: Any] = ["scope": "diagnostic only; actual callback/publisher phases; no qualification substitution",
        "calibrations": phaseCalibrations, "offset_lower_ns": lower, "offset_upper_ns": upper,
        "clock_bounds_valid": phaseCalibrations.count == 64 && lower <= upper,
        "samples": phaseInputs]
    try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]).write(
        to: directory.appendingPathComponent("helper-phases.json"), options: .atomic)
}
