#!/usr/bin/env python3
"""Qualify native relocation on an owned, disposable, separate filesystem.

Only this process-created image is formatted/mounted. The image, mountpoint,
source fixtures and privileged operations stay under one fresh temp directory.
No production home/profile discovery is used by the Rust rehearsal.
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]

def run(argv, **kwargs):
    return subprocess.run(argv, check=True, text=True, **kwargs)


def main():
    build = run(["cargo", "build", "--locked", "-p", "librairii-storage", "--example", "relocation_handover", "--message-format=json-render-diagnostics"], cwd=ROOT, stdout=subprocess.PIPE)
    artifacts = [message["executable"] for line in build.stdout.splitlines()
                 if (message := json.loads(line)).get("reason") == "compiler-artifact"
                 and message.get("target", {}).get("name") == "relocation_handover"
                 and "example" in message.get("target", {}).get("kind", [])
                 and message.get("executable")]
    if len(artifacts) != 1:
        raise RuntimeError("expected one executable from this Cargo build")
    binary = Path(artifacts[0]).resolve(strict=True)
    binary_digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    system = platform.system()
    if system not in ("Darwin", "Linux"):
        raise RuntimeError("native macOS/Linux host required")
    with tempfile.TemporaryDirectory(prefix="librairii-native-relocation-") as temporary:
        work = Path(temporary).resolve()
        mount = work / "mounted"
        mount.mkdir()
        source = work / "fixtures"
        source.mkdir()
        mounted = False
        try:
            if system == "Darwin":
                image = work / "owned.sparseimage"
                run(["hdiutil", "create", "-size", "128m", "-fs", "APFS", "-volname", "LibrairiiTask13", "-type", "SPARSE", "-o", str(image)], stdout=subprocess.PIPE)
                run(["hdiutil", "attach", "-nobrowse", "-owners", "on", "-mountpoint", str(mount), str(image)], stdout=subprocess.PIPE)
                mounted = True
            else:
                image = work / "owned.ext4"
                with image.open("xb") as stream:
                    stream.truncate(128 * 1024 * 1024)
                run(["mkfs.ext4", "-q", "-F", "-m", "0", str(image)])
                run(["sudo", "-n", "mount", "-t", "ext4", "-o", "loop,nosuid,nodev", str(image), str(mount)])
                mounted = True
                run(["sudo", "-n", "chown", f"{os.getuid()}:{os.getgid()}", str(mount)])
                # This is mkfs's empty directory on our newly created image.
                run(["sudo", "-n", "rmdir", str(mount / "lost+found")])
            mounted = True
            if source.stat().st_dev == mount.stat().st_dev:
                raise RuntimeError("mounted target is not a separate filesystem")
            result = run([str(binary), str(source), str(mount)], cwd=ROOT, stdout=subprocess.PIPE, timeout=600)
            evidence = json.loads(result.stdout)
            if hashlib.sha256(binary.read_bytes()).hexdigest() != binary_digest:
                raise RuntimeError("Cargo executable changed during rehearsal")
            evidence["cargo_artifact_executable"] = str(binary)
            evidence["executable_sha256"] = binary_digest
            evidence["driver_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
            evidence["example_sha256"] = hashlib.sha256((ROOT / "crates/librairii-storage/examples/relocation_handover.rs").read_bytes()).hexdigest()
            evidence["checkout_sha"] = run(["git", "rev-parse", "HEAD"], cwd=ROOT, stdout=subprocess.PIPE).stdout.strip()
            evidence["working_tree_dirty"] = bool(run(["git", "status", "--porcelain"], cwd=ROOT, stdout=subprocess.PIPE).stdout.strip())
            evidence["host_system"] = platform.platform()
            evidence["host_machine"] = platform.machine()
            evidence["filesystem"] = "APFS disk image" if system == "Darwin" else "ext4 loop image"
            evidence["image_bytes"] = 128 * 1024 * 1024
            evidence["volume_owned_by_rehearsal"] = True
            if len(evidence["forced_termination"]) != 8 or len(evidence["filesystem_failures"]) != 2:
                raise RuntimeError("incomplete mandatory native rehearsal")
            if evidence["volume_root"]["volume_root"] != "exact_root_verified":
                raise RuntimeError("mandatory exact mount-root qualification did not execute")
            output = ROOT / "target/quality/native-relocation.json"
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(json.dumps(evidence, indent=2) + "\n")
            print(f"Native relocation: 8 SIGKILL boundaries, actual EACCES/ENOSPC, {evidence['filesystem']}, exact-root/reopen verified.")
        finally:
            if mounted:
                if system == "Darwin":
                    run(["hdiutil", "detach", str(mount)], stdout=subprocess.PIPE)
                else:
                    run(["sudo", "-n", "umount", str(mount)])


if __name__ == "__main__":
    main()
