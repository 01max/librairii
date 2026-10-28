#!/usr/bin/env python3
"""Qualify native relocation on an owned, disposable, separate filesystem.

Only this process-created image is formatted/mounted. The image, mountpoint,
source fixtures and privileged operations stay under one fresh temp directory.
No production home/profile discovery is used by the Rust rehearsal.
"""
import hashlib
from contextlib import contextmanager
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]

def run(argv, **kwargs):
    return subprocess.run(argv, check=True, text=True, **kwargs)


@contextmanager
def owned_workspace():
    work = Path(tempfile.mkdtemp(prefix="librairii-native-relocation-")).resolve()
    # Failed or uncertain disposal retains the image and fixtures for diagnosis.
    # Automatic TemporaryDirectory cleanup could traverse a still-mounted image.
    yield work
    shutil.rmtree(work)


def rehearse(binary, source, mount, work):
    result = None
    try:
        result = run([str(binary), str(source), str(mount)], cwd=ROOT,
                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=600)
        return result
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        result = error
        raise
    finally:
        if result is not None:
            for name in ("stdout", "stderr"):
                value = getattr(result, name, None) or ""
                (work / ("rehearsal." + name)).write_bytes(value if isinstance(value, bytes) else value.encode())


def detach_owned_image(mount):
    # macOS indexing/disk arbitration can retain the just-tested image after
    # every rehearsal process has exited. Only our fresh image's mountpoint is
    # accepted here; no ambient device/volume discovery or detach is performed.
    for attempt in range(3):
        result = subprocess.run(["hdiutil", "detach", str(mount)], text=True,
                                capture_output=True, timeout=30)
        if result.returncode == 0:
            return False
        if result.returncode != 16:
            raise RuntimeError(f"owned image detach failed: {result.stderr.strip()}")
        time.sleep(attempt + 1)
    run(["hdiutil", "detach", "-force", str(mount)], stdout=subprocess.PIPE, timeout=30)
    return True


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
    with owned_workspace() as work:
        mount = work / "mounted"
        mount.mkdir()
        source = work / "fixtures"
        source.mkdir()
        mounted = False
        detached = False
        primary = None
        cleanup = None
        mount_device = None
        image = None
        try:
            if system == "Darwin":
                image = work / "owned.sparseimage"
                run(["hdiutil", "create", "-size", "128m", "-fs", "APFS", "-volname", "LibrairiiTask13", "-type", "SPARSE", "-o", str(image)], stdout=subprocess.PIPE)
                # Attach may have effects even if its acknowledgement fails.
                mounted = True
                run(["hdiutil", "attach", "-nobrowse", "-owners", "on", "-mountpoint", str(mount), str(image)], stdout=subprocess.PIPE)
            else:
                image = work / "owned.ext4"
                with image.open("xb") as stream:
                    stream.truncate(128 * 1024 * 1024)
                run(["mkfs.ext4", "-q", "-F", "-m", "0", str(image)])
                mounted = True
                run(["sudo", "-n", "mount", "-t", "ext4", "-o", "loop,nosuid,nodev", str(image), str(mount)])
                run(["sudo", "-n", "chown", f"{os.getuid()}:{os.getgid()}", str(mount)])
                # This is mkfs's empty directory on our newly created image.
                run(["sudo", "-n", "rmdir", str(mount / "lost+found")])
            mount_device = mount.stat().st_dev
            if source.stat().st_dev == mount_device:
                raise RuntimeError("mounted target is not a separate filesystem")
            result = rehearse(binary, source, mount, work)
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
        except Exception as error:
            primary = error
        finally:
            if mounted:
                try:
                    if system == "Darwin":
                        forced = detach_owned_image(mount)
                    else:
                        run(["sudo", "-n", "umount", str(mount)])
                        forced = False
                    detached = True
                except Exception as error:
                    cleanup = error
            if primary is not None or cleanup is not None:
                try:
                    current_device = mount.stat().st_dev
                except OSError:
                    current_device = None
                failure = {"complete": False, "work": str(work), "image": str(image) if image else None,
                           "mount": str(mount), "source": str(source), "mount_attempted": mounted,
                           "mounted_device": mount_device, "current_mount_device": current_device,
                           "owned_volume_detached": detached, "primary_error": repr(primary) if primary else None,
                           "cleanup_error": repr(cleanup) if cleanup else None}
                output = ROOT / "target/quality/native-relocation-failure.json"
                output.parent.mkdir(parents=True, exist_ok=True)
                # These files are outside the mounted tree. Retain them in the
                # CI artifact even though the disposable runner will disappear.
                for name in ("stdout", "stderr"):
                    stream = work / ("rehearsal." + name)
                    if stream.is_file():
                        shutil.copy2(stream, output.with_suffix("." + name))
                output.write_text(json.dumps(failure, indent=2) + "\n")
                (work / "failure.json").write_text(json.dumps(failure, indent=2) + "\n")
                raise RuntimeError(f"Native relocation failed; owned evidence retained at {work}; primary={primary!r}; cleanup={cleanup!r}") from (primary if primary is not None else cleanup)
        evidence["owned_volume_detached"] = True
        evidence["owned_volume_detach_forced"] = forced
        output = ROOT / "target/quality/native-relocation.json"
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(evidence, indent=2) + "\n")
        print(f"Native relocation: 8 SIGKILL boundaries, actual EACCES/ENOSPC, {evidence['filesystem']}, exact-root/reopen and owned-volume disposal verified.")


if __name__ == "__main__":
    run([sys.executable, "-I", "-B", str(ROOT / "qualification/section-16/test_relocation.py")])
    main()
