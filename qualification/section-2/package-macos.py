#!/usr/bin/env python3
"""Assemble a local unsigned spike bundle, never a qualified release package."""
import pathlib
import plistlib
import shutil
import sys

binary = pathlib.Path(sys.argv[1]).resolve(strict=True)
output = pathlib.Path(sys.argv[2]).resolve()
if output.suffix != ".app" or output.exists():
    raise SystemExit("use a new .app output path")
contents = output / "Contents"
(contents / "MacOS").mkdir(parents=True)
shutil.copy2(binary, contents / "MacOS" / "qualification-desktop")
with (contents / "Info.plist").open("wb") as stream:
    plistlib.dump({
        "CFBundleExecutable": "qualification-desktop",
        "CFBundleIdentifier": "dev.librairii.qualification",
        "CFBundleName": "Librairii Qualification",
        "CFBundlePackageType": "APPL",
        "CFBundleVersion": "0.0.0",
        "LSMinimumSystemVersion": "15.0",
        "NSHighResolutionCapable": True,
    }, stream)
print(output)
