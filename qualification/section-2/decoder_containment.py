#!/usr/bin/env python3
"""Qualification-only LinuxARM containment supervisor; never a production adapter.

Every allocation probe is a separate owned container. Controls must pass first.
No container receives production paths, network access, Docker socket or credentials.
"""
import argparse
import _lzma
import hashlib
import json
import os
import platform
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
import zlib
from adversarial_7z import fixtures as adversarial_fixtures

IMAGE = "rust@sha256:e12566d5abc17a0da741628782cdb504b68b8961f5bfcf677936bf3e8b32de29"
MEMORY = 128 << 20
ADDRESS = 256 << 20


def docker(*arguments, timeout=30):
    result = subprocess.run(["docker", *arguments], text=True, capture_output=True,
                            timeout=timeout, check=False)
    if result.returncode:
        raise RuntimeError(f"containment tooling failure (NOT decoder verdict): {result.stderr.strip()}")
    return result.stdout.strip()


def contained(binary, fixtures, mode, file=None, extra=(), wall=5):
    label = os.environ.get("AO_SESSION_ID")
    if not label:
        raise RuntimeError("AO_SESSION_ID required for ownership labels")
    command = [mode] + (["/fixtures/" + file] if file else []) + list(map(str, extra))
    container = docker("create", "--label", "ao.session=" + label,
                       "--memory", str(MEMORY), "--memory-swap", str(MEMORY),
                       "--ulimit", "cpu=2:2",
                       "--ulimit", "core=0:0", "--cpus", "1", "--pids-limit", "32",
                       "--network", "none", "--read-only", "--cap-drop", "ALL",
                       "--security-opt", "no-new-privileges", "--tmpfs", "/tmp:rw,noexec,nosuid,size=8m",
                       "--mount", f"type=bind,src={binary},dst=/probe,readonly",
                       "--mount", f"type=bind,src={fixtures},dst=/fixtures,readonly",
                       IMAGE, "/bin/bash", "--noprofile", "--norc", "-c",
                       f'ulimit -Sv {ADDRESS // 1024} && ulimit -Hv {ADDRESS // 1024} || exit 125; '
                       '/probe "$@" & child=$!; wait "$child"; result=$?; '
                       'printf "QUALIFICATION_CGROUP_AFTER_BEGIN\\n"; cat /sys/fs/cgroup/memory.events; '
                       'printf "QUALIFICATION_CGROUP_AFTER_END\\nQUALIFICATION_MEMORY_PEAK "; '
                       'cat /sys/fs/cgroup/memory.peak; exit "$result"',
                       "qualification-launcher", *command)
    try:
        settings = json.loads(docker("inspect", "--format", "{{json .HostConfig}}", container))
        assert settings["Memory"] == MEMORY and settings["MemorySwap"] == MEMORY
        assert settings["ReadonlyRootfs"] and settings["NetworkMode"] == "none"
        assert settings["PidsLimit"] == 32
        assert settings["NanoCpus"] == 1_000_000_000
        assert settings["CapDrop"] == ["ALL"]
        assert "no-new-privileges" in settings["SecurityOpt"]
        limits = {row["Name"]: (row["Soft"], row["Hard"]) for row in settings["Ulimits"]}
        assert limits["cpu"] == (2, 2)
        start = time.monotonic()
        timed_out = False
        try:
            result = subprocess.run(["docker", "start", "--attach", container], text=True,
                                    capture_output=True, timeout=wall, check=False)
            stdout, stderr = result.stdout, result.stderr
        except subprocess.TimeoutExpired:
            timed_out = True
            # Capture counters while the cgroup still exists, before killing PID1.
            events_before_kill = docker("exec", container, "cat", "/sys/fs/cgroup/memory.events", timeout=3)
            docker("kill", container, timeout=3)
            docker("wait", container, timeout=3)
            stdout = docker("logs", container, timeout=3)
            stderr = "host wall watchdog killed child"
        elapsed = time.monotonic() - start
        state = json.loads(docker("inspect", "--format", "{{json .State}}", container))
        records = []
        for line in stdout.splitlines():
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError:
                pass
        events_match = re.search(r"QUALIFICATION_CGROUP_AFTER_BEGIN\n(.*?)QUALIFICATION_CGROUP_AFTER_END", stdout, re.S)
        events_text = events_before_kill if timed_out else events_match[1] if events_match else None
        events = dict((key, int(value)) for key, value in (line.split() for line in events_text.strip().splitlines())) if events_text else None
        peak_match = re.search(r"QUALIFICATION_MEMORY_PEAK (\d+)", stdout)
        return {"mode": mode, "file": file, "exitCode": state["ExitCode"],
                "signalFromExitStatus": state["ExitCode"] - 128 if 128 <= state["ExitCode"] <= 192 else None,
                "oomKilled": state["OOMKilled"], "wallWatchdog": timed_out,
                "memoryEventsAfter": events, "memoryEventsCapturedBeforeWatchdogKill": timed_out,
                "cgroupMemoryPeakAfter": int(peak_match[1]) if peak_match else None,
                "callbackCount": sum(row.get("stage") == "callback" for row in records),
                "hostElapsedSeconds": round(elapsed, 4), "records": records,
                "stderr": stderr, "dockerStateError": state["Error"]}
    finally:
        docker("rm", "--force", container, timeout=5)


def controls(binary, fixtures, result):
    smoke = contained(binary, fixtures, "smoke")
    result.append(smoke)
    assert smoke["exitCode"] == 0 and smoke["records"][-1]["outcome"] == "smoke_ok", "tooling/startup failure; NO allocation probes allowed"
    boot = smoke["records"][0]
    assert boot["memoryMax"].strip() == str(MEMORY) and boot["swapMax"].strip() == "0"
    assert re.search(rf"Max address space\s+{ADDRESS}\s+{ADDRESS}", boot["limits"])
    assert re.search(r"Max cpu time\s+2\s+2", boot["limits"])
    address = contained(binary, fixtures, "control-address")
    result.append(address)
    assert address["exitCode"] != 0 and "memory allocation of 536870912 bytes failed" in address["stderr"], "address-space cap NOT proven"
    memory = contained(binary, fixtures, "control-memory")
    result.append(memory)
    assert memory["oomKilled"] and memory["exitCode"] == 137, "cgroup resident-memory cap NOT proven"
    assert memory["memoryEventsAfter"] and memory["memoryEventsAfter"]["oom_kill"] > 0, "independent post-child memory.events missing"
    wall = contained(binary, fixtures, "control-wall", wall=2)
    result.append(wall)
    assert wall["wallWatchdog"] and not wall["oomKilled"] and wall["hostElapsedSeconds"] < 6, "wall watchdog NOT proven"
    cpu = contained(binary, fixtures, "control-cpu", wall=8)
    result.append(cpu)
    assert cpu["exitCode"] == 137 and not cpu["wallWatchdog"] and not cpu["oomKilled"], "CPU-time cap NOT proven"
    return result


def verdict(observed):
    """Process refusal is containment evidence, never a product rejection code."""
    if observed["dockerStateError"]:
        raise RuntimeError("container runtime failed: " + observed["dockerStateError"])
    if observed["exitCode"] != 0:
        return "contained_process_refusal"
    return observed["records"][-1].get("outcome", "missing_decoder_outcome")


def classification(observed, actual):
    if observed["wallWatchdog"]:
        return "supervisor_wall_timeout"
    if observed["memoryEventsAfter"] and observed["memoryEventsAfter"]["oom_kill"]:
        return "decoder_resource_exhaustion_pre_callback" if not observed["callbackCount"] else "decoder_resource_exhaustion_in_callback"
    if observed["exitCode"] != 0:
        return "allocation_refusal_with_os_signal" if "memory allocation" in observed["stderr"] else "os_signal_or_process_failure"
    if actual == "qualification_metadata_refused":
        reason = observed["records"][-1]["details"]["reason"]
        return "experimental_budget_refusal" if reason.startswith("experimental") else "frozen_declared_budget_refusal"
    if actual.startswith("library"):
        error = observed["records"][-1].get("details", {}).get("error", "") or ""
        return "qualification_read_budget_refusal" if "qualification" in error and "ceiling" in error else "decoder_rejection"
    return actual


def experiments(binary, directory, result):
    for fixture in adversarial_fixtures(directory):
        observed = contained(binary, directory, fixture["mode"], fixture["file"], fixture["extra"])
        actual = verdict(observed)
        differences = [] if actual == fixture["expected"] else [{"expected": fixture["expected"], "actual": actual}]
        if "expectedDecoded" in fixture and observed["exitCode"] == 0:
            decoded = observed["records"][-1].get("details", {}).get("decoded", [])
            actual_entries = [{"name": e["name"], "bytes": e["bytes"], "crc32": e["observedCrc32"]} for e in decoded]
            if actual_entries != fixture["expectedDecoded"]:
                differences.append({"expectedDecoded": fixture["expectedDecoded"], "actualDecoded": actual_entries})
            if any(not e["hasCrc"] or e["headerCrc"] != e["observedCrc32"] for e in decoded):
                differences.append({"crc": "header/observed CRC mismatch"})
        if fixture["mode"] == "cancel-before" and observed["records"][-1].get("sourceBytesRead") != 0:
            differences.append({"cancellation": "read source despite before-header cancellation"})
        result.append({"fixture": fixture, "actual": actual, "classification": classification(observed, actual),
                       "differences": differences, "observation": observed})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="ELF child built from decoder-budget with the pinned image")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    assert binary.read_bytes().startswith(b"\x7fELF"), "toolchain/target failure: expected Linux ELF, not a native macOS binary"
    report = {"scope":"qualification-only LinuxARM resource containment; not desktop/platform/product qualification",
              "image":IMAGE,"binarySha256":hashlib.sha256(binary.read_bytes()).hexdigest(),
              "limits":{"cgroupMemoryBytes":MEMORY,"swapBytes":0,"addressSpaceBytes":ADDRESS,"cpuSeconds":2,"wallSeconds":5},
              "controls":[],"adversarialCases":[],"status":"unqualified"}
    report["observer"] = {"session": os.environ["AO_SESSION_ID"], "date": "2026-10-20",
                          "dockerServer": json.loads(docker("version", "--format", "{{json .Server}}")),
                          "kernel": docker("info", "--format", "{{.KernelVersion}}")}
    report["producer"] = {"python": sys.version, "host": platform.platform(),
                          "zlibRuntime": zlib.ZLIB_RUNTIME_VERSION,
                          "lzmaModuleSha256": hashlib.sha256(Path(_lzma.__file__).read_bytes()).hexdigest(),
                          "formatSource": "https://github.com/ip7z/7zip/blob/9128b80e3a471108678e2b3ec3c985861c8d7b0f/DOC/7zFormat.txt",
                          "fixtureBytes": "original deterministic synthetic_7z.py/adversarial_7z.py output, each length/hash recorded; regenerate without parsing for inspection"}
    report["hypothesisComparisonScope"] = "Initial allocation hypotheses are retained even when disproved; differences are not Go parity failures or toolkit verdicts."
    report["sourceSha256"] = {str(p.relative_to(Path(__file__).parent)): hashlib.sha256(p.read_bytes()).hexdigest()
                              for p in [Path(__file__), Path(__file__).with_name("adversarial_7z.py"),
                                        Path(__file__).with_name("synthetic_7z.py"),
                                        Path(__file__).parent / "decoder-budget/src/main.rs",
                                        Path(__file__).parent / "decoder-budget/Cargo.lock"]}
    try:
        with tempfile.TemporaryDirectory(prefix="librairii-4-decoder-") as temporary:
            directory = Path(temporary)
            controls(binary, directory, report["controls"])
            # No declared-count/dictionary allocation experiment may run before
            # all independent controls have passed in this invocation.
            experiments(binary, directory, report["adversarialCases"])
        report["measuredDifferenceCount"] = sum(len(c["differences"]) for c in report["adversarialCases"])
        report["status"] = "bounded LinuxARM observations only; decoder/platform/product qualification incomplete"
    except (AssertionError, RuntimeError, subprocess.TimeoutExpired) as error:
        report["toolingError"] = str(error)
        report["status"] = "tooling/control failure; not a decoder verdict"
        raise
    finally:
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Observed {len(report['controls'])} controls, {len(report['adversarialCases'])} cases, "
          f"{report['measuredDifferenceCount']} differences; qualification remains incomplete.")
    return 2 if report["measuredDifferenceCount"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
