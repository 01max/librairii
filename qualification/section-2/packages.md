# Package feasibility (2.11, P57/P59)

Task2.11 is complete for the minimal spike packages. The exact three-target
archive round-trips/native bytes are verified. A fresh offline Linux KVM guest
proves launch/reboot/removal/reinstall with independent canaries; both Mac targets
prove normal/CPU launch/relaunch in a fresh HOME with actual source/toolchain/network
denial controls. See clean-macos.md, linux-vm.md and [selection-gate.md](selection-gate.md).
The selected Mac package is the sealed.app in the tar archive; signing/notarization,
Gatekeeper trust and product installer/upgrade lifecycle remain Task18 work.

Earlier observations below retain their dates, source commits and historical scope.

Status: **ARM local unsigned experiment only; no clean-host qualification**.
Observer librairii-4, 2026-10-20, host in hosts.md. `package-macos.py` creates a
new minimal bundle around the embedded-resource executable, distinct bundle ID
`dev.librairii.qualification`, minimum OS15.0. It refuses an existing output.
No toolchain, Go, Node, browser or source resources are copied into the bundle.

Observed:

1. Optimized ARM build, raw binary SHA
   `25ec42d47bf2bbcd95e785c55c43e0127cb960715c7d4ff8156f59737363877a`.
2. `hdiutil create -volname 'Librairii Qualification' -srcfolder
   target/QualificationRelease.app -format UDZO target/Qualification-arm64-unsigned.dmg`
   succeeded; `hdiutil verify` passed. Local image size10,848,785 bytes, SHA-256
   `5c1a7fdf0a0d5220d7015689a22914e86768151018b00388d51e809313b41cee`.
3. Mounted read-only, launched the bundled executable from that mount with cwd
   `/private/tmp`, `SLINT_BACKEND=winit-skia-software ... --smoke`: exit0,
   construction147 ms. Detached the owned image afterward.
4. `codesign -dv` reports linker ad-hoc signature, no TeamIdentifier, unbound
   Info.plist and no sealed resources. This is **not Developer ID signing**,
   notarization, stapling or Gatekeeper/clean-host install evidence.

No image/binary is published. The local spike image omits a complete third-party
notice inventory and is not a distributable release. A mounted executable smoke
does not prove LaunchServices menu integration, correct visual output or a machine
without developer tools. Native app interaction access remains denied.

## Windows installer options researched now

| Exact candidate / option | Benefits and unresolved terms |
| --- | --- |
| Inno Setup7.1.0 | Maintained immutable release; per-user `{localappdata}` install with `PrivilegesRequired=lowest` is a candidate. [Release](https://github.com/jrsoftware/issrc/releases/tag/is-7_1_0), [license](https://jrsoftware.org/files/is/license.txt). Upstream requests commercial licenses, including CI use, but explicitly says purchase is not strictly required. Record the owner's decision before use; no purchase made. [Commercial terms](https://jrsoftware.org/isorder.php) |
| WiX7.0.0 | MSI per-user authoring and native Windows servicing; larger .NET/tooling surface. Binary release requires OSMF EULA adherence; revenue threshold/maintenance sponsorship must be resolved. Not assumed cost-free. [Release](https://github.com/wixtoolset/wix/releases/tag/v7.0.0), [terms](https://docs.firegiant.com/wix/osmf/) |
| NSIS3.12 (verified comparison release, not a claim of latest) | Simple per-user installer, mostly zlib/libpng terms; compression modules have separate bzip2/CPL terms and LZMA linking exception. [Release](https://nsis.sourceforge.io/Download), [terms](https://nsis.sourceforge.io/License). Re-check a newer release before choosing. |
| MSIX via matching Windows SDK | Optional identity/Store channel; needs makeappx/makepri and certificate/identity/runtime handling outside Store. Does not replace mandatory portable ZIP. [Slint guide](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/windows/packaging/) |

Recommendation to evaluate first: Inno7.1.0 per-user installer plus portable ZIP;
**installer not selected** pending Windows execution and distribution terms.
No setup/uninstall code may delete libraries, profiles or journals. Exercise fresh
install/update/uninstall/reinstall, Unicode/long/locked paths, retained synthetic
data and actual GUI launch on Windows11 x64 before selecting the installer.

Skia/MSVC needs the C++ runtime. Inspect `dumpbin /dependents`; choose an exact
matching-or-newer redistributable after the MSVC build exists, pin its file version,
hash and license. Do not invent a redist pin without that build. Static CRT is a
separate full-native-graph build/redistribution experiment, not a switch assumed
to work with downloaded Skia binaries. [Microsoft requirements](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist),
[redistribution terms](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution).

## Credentials and remaining targets

CLT notarytool/stapler and system codesign/hdiutil/ditto are available. Public
macOS release requires Developer ID signing, hardened runtime/timestamp,
notarization authentication, staple/verify and final-byte clean-host launch.
Credential presence is unverified; no keychain/secret was read and nothing was
submitted to Apple. [Apple requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
Windows Authenticode certificate/signing service and timestamp verification also
remain a public release gate. Credential absence does not block unsigned probes.

Intel .app/DMG, Linux x64 tar.gz/desktop/icon/X11+Wayland linkage and Windows
portable ZIP/installer remain unbuilt/unexecuted here. Minimum native library/OS
claims, complete notices and clean-host launches are open on all four targets.
