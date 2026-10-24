# Fresh Linux VM package qualification (2.11, partial)

`check-linux-vm.py` starts one owned Ubuntu24.04 x86-64 guest on the existing
Linux CI runner. It requires usable KVM and a positive QMP `query-kvm` result;
TCG/emulation is not a substitute. The immutable
[20261017 cloud image](https://cloud-images.ubuntu.com/releases/noble/release-20261017/)
is verified against its pinned official
[SHA256SUMS](https://cloud-images.ubuntu.com/releases/noble/release-20261017/SHA256SUMS).
The image digest is `04e8f2461a16a44e78a370284471f29616cdd9baf1893df25807b71f985b6c94`.

The VM has a disposable copy-on-write disk, two read-only seed/payload CDs,
2 virtual CPUs and3GiB memory. It has no shared directories or host credentials.
Cloud-init creates a locked fresh user and installs only the declared OS runtime
and observation packages. The distribution's Python/cloud-init and the copied
qualification observers are intentional OS/test tooling; application source,
Rust/Go/Node/C++ toolchains and browser runtimes are absent. The application
does not invoke the Python observer. Actual packages and executable bytes are
recorded. Initial provisioning can reach Ubuntu repositories; the entire NIC
and NAT backend are removed before the first application process starts.
The guest must then expose only its loopback interface. Control/evidence use
the guest agent over a private host Unix socket, independently of networking.

The authenticated archive is extracted into `/opt/librairii`. The application
runs as the ordinary fresh user with `/tmp` as its working directory. Existing
native observers exercise all eight X11 cases. The guest then performs a real
OS reboot; changed boot ID, unchanged installed executable and continued absence
of networking are required. All eight Wayland native cases and the existing
normal/CPU Wayland rendering/component probes run after reboot. Removal and
reinstallation of the same verified archive are followed by a fresh owned
shutdown probe. Four unrelated byte canaries must remain unchanged across the
reboot/removal/reinstallation. They are preservation controls, not a real product
database, application persistence test or Go-library upgrade fixture.

This qualifies only the recorded spike/archive route. It does not establish a
production installer, version-to-version migration/rollback, full native product
workflow, physical keyboard/IME/display or either macOS clean host. It cannot
select the toolkit. Those broader gates remain open.

Each guest command, monitor reply and evidence transfer is bounded. Existing
native probes retain their deadlines and exact callback/parent checks. The
overall experiment has a30-minute deadline. Unexpected exits, truncated output,
unsafe/oversized evidence, incomplete cases and failure to reap the owned VM or
remove its disks fail the result. Interrupt handling also retires the VM. Raw
serial output, QMP hardware/offline state, command results, before/after guest
records, screenshots and hashes remain in `linux-vm/` in the CI artifact.

Native execution is pending the first CI run; local unit tests are recorder
regressions and do not qualify a VM or target. The prior container results in
`clean-linux.md` and `native-linux.md` remain separately attributed.
