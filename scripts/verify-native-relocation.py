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
import plistlib
import re
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


def detach_owned_image(mount, image):
    """Detach only the owned image, verifying disposal independently of exit status."""
    observations = []
    forced = False
    failure = None

    def inventory():
        result = run(["hdiutil", "info", "-plist"], stdout=subprocess.PIPE, timeout=30)
        data = plistlib.loads(result.stdout.encode())
        if not isinstance(data, dict) or not isinstance(data.get('images'), list):
            raise RuntimeError('invalid disk image inventory')
        matches = []
        for row in data['images']:
            if not isinstance(row, dict) or not isinstance(row.get('image-path'), str):
                raise RuntimeError('invalid disk image identity')
            if Path(row['image-path']).resolve() == image.resolve():
                matches.append(row)
        if len(matches) > 1:
            raise RuntimeError('ambiguous owned image attachment')
        # Retain only our image's observation, never ambient image metadata.
        record = None
        if matches:
            entities = matches[0].get('system-entities')
            record = {'image-path': matches[0]['image-path'], 'system-entities': [
                {key: entity[key] for key in ('dev-entry', 'content-hint', 'mount-point')
                 if isinstance(entity.get(key), str)}
                for entity in entities if isinstance(entity, dict)
            ] if isinstance(entities, list) else None}
        observations.append({'owned_image': record})
        return matches[0] if matches else None

    def disposed(row):
        if row is not None:
            return False
        if mount.is_symlink() or os.path.ismount(mount) or mount.stat().st_dev != mount.parent.stat().st_dev:
            raise RuntimeError('image absent but owned mountpoint still uncertain')
        return True

    try:
        for attempt in range(4):
            row = inventory()
            if disposed(row):
                return forced
            entities = row.get('system-entities')
            # Our APFS image was created with the default GUID partition map.
            # Never guess a disk from a mountpoint or reuse a stale device ID.
            if not isinstance(entities, list):
                raise RuntimeError('owned image device inventory missing')
            disks = [e.get('dev-entry') for e in entities if isinstance(e, dict)
                     and e.get('content-hint') == 'GUID_partition_scheme']
            if len(disks) != 1 or not isinstance(disks[0], str) or not re.fullmatch(r'/dev/disk[0-9]+', disks[0]):
                raise RuntimeError('unique owned image backing device required')
            forced = attempt == 3
            command = ['hdiutil', 'detach'] + (['-force'] if forced else []) + [disks[0]]
            result = subprocess.run(command, text=True, capture_output=True, timeout=30)
            observations.append({'command': command, 'returncode': result.returncode,
                                 'stdout': result.stdout, 'stderr': result.stderr})
            if disposed(inventory()):
                return forced
            if result.returncode != 16 or forced:
                raise RuntimeError(f'owned image remains attached after detach: {result.stderr.strip()}')
            time.sleep(attempt + 1)
        raise RuntimeError('owned image disposal incomplete')
    except Exception as error:
        failure = error
        raise
    finally:
        try:
            (image.parent / 'disposal.json').write_text(json.dumps(observations, indent=2) + '\n')
        except OSError as error:
            if failure is None:
                raise RuntimeError(f'owned image disposal receipt write failed: {error}') from error
            failure.add_note(f'owned image disposal receipt write also failed: {error}')


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
                        forced = detach_owned_image(mount, image)
                    else:
                        run(["sudo", "-n", "umount", str(mount)])
                        forced = False
                    detached = True
                except Exception as error:
                    cleanup = error
            disposal = work / 'disposal.json'
            if disposal.is_file():
                output = ROOT / 'target/quality/native-relocation-disposal.json'
                try:
                    output.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(disposal, output)
                except OSError as error:
                    if cleanup is None:
                        cleanup = RuntimeError(f'disposal receipt copy failed: {error}')
                    else:
                        cleanup.add_note(f'disposal receipt copy also failed: {error}')
            if primary is not None or cleanup is not None:
                try:
                    current_device = mount.stat().st_dev
                except OSError:
                    current_device = None
                failure = {"complete": False, "work": str(work), "image": str(image) if image else None,
                           "mount": str(mount), "source": str(source), "mount_attempted": mounted,
                           "mounted_device": mount_device, "current_mount_device": current_device,
                           "owned_volume_detached": detached, "primary_error": repr(primary) if primary else None,
                           "cleanup_error": repr(cleanup) if cleanup else None,
                           "cleanup_notes": getattr(cleanup, '__notes__', [])}
                output = ROOT / "target/quality/native-relocation-failure.json"
                diagnostic_errors = []
                try:
                    output.parent.mkdir(parents=True, exist_ok=True)
                except OSError as error:
                    diagnostic_errors.append(f'failure receipt directory: {error}')
                # These files are outside the mounted tree. Retain them in the
                # CI artifact even though the disposable runner will disappear.
                for name in ("stdout", "stderr"):
                    stream = work / ("rehearsal." + name)
                    if stream.is_file():
                        try:
                            shutil.copy2(stream, output.with_suffix("." + name))
                        except OSError as error:
                            diagnostic_errors.append(f'{name} retention: {error}')
                failure['diagnostic_errors'] = diagnostic_errors
                for receipt in (output, work / 'failure.json'):
                    try:
                        receipt.write_text(json.dumps(failure, indent=2) + '\n')
                    except OSError as error:
                        diagnostic_errors.append(f'{receipt.name} retention: {error}')
                raise RuntimeError(f"Native relocation failed; owned workspace retained at {work}; primary={primary!r}; cleanup={cleanup!r}; diagnostics={diagnostic_errors!r}") from (primary if primary is not None else cleanup)
        evidence["owned_volume_detached"] = True
        evidence["owned_volume_detach_forced"] = forced
        output = ROOT / "target/quality/native-relocation.json"
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(evidence, indent=2) + "\n")
        print(f"Native relocation: 8 SIGKILL boundaries, actual EACCES/ENOSPC, {evidence['filesystem']}, exact-root/reopen and owned-volume disposal verified.")


if __name__ == "__main__":
    run([sys.executable, "-I", "-B", str(ROOT / "qualification/section-16/test_relocation.py")])
    main()
