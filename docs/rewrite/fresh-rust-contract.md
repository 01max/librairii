# Fresh Rust application

Owner decision, 2026-11-03: deliver a new Rust native application with the
previous product's user capabilities. Breaking changes are acceptable. Existing
Go installations, libraries, settings and browser preferences are not migration
inputs or release prerequisites. This decision supersedes the legacy adoption,
Go bridge, compatible rollback and paid Mac signing requirements in earlier
rewrite reports.

First launch uses the independent Rust profile and proposes a fresh library.
It must not discover or import the Go profile, database, recovery journals or
browser state. Invalid old settings must not prevent fresh setup. Existing Rust
profiles still reopen their own library, and missing or damaged Rust state must
enter recovery rather than silently creating a replacement. Importing archive
files remains a normal product workflow. No personal files are deleted by this
repository change.

The maintained runtime, build commands and CI become Rust-only. Frozen synthetic
fixtures, reference outputs, original source provenance, historical reports and
Git history remain available for checking functional parity. A frozen reference
generator may use Go as a development tool; Go, Node and browser runtimes are not
application or normal package prerequisites. Retaining a behavioral fixture does
not promise compatibility with a user's previous database.

The default macOS distribution is a free ad-hoc signed app in a DMG. It is not
Developer ID signed or Apple-notarized. Trusted downloaded builds may require
the per-app **System Settings → Privacy & Security → Open Anyway** approval.
Paid signing remains an optional future channel and does not gate this release.
Linux remains a native executable archive. ARM Mac, real Intel Mac and Linux
must each pass installed-package, fresh-library, reopen and recovery checks.

Functional parity, custody of newly imported archives, offline/privacy behavior,
accessibility within the recorded platform scope, dependencies/notices and
source-bound package checks remain required. Performance stays blocking on
every PR with the existing budgets, including 100 ms input dispatch and 3000 ms
navigation completion. Historical failed measurements keep their verdicts;
removing legacy code or signing requirements does not establish a performance fix.

Task 20 retires the tracked legacy runtime and updates the maintained commands,
package contract and CI in one branch/PR, with separate subtask commits. Final
artifacts are selected and verified from the resulting Rust-only source. Release
publication, PR merging and OpenSpec archival remain separate actions.
