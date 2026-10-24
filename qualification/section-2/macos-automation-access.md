# Read-only macOS observer preflight (2.6, partial)

The CI helper `macos-automation-access.swift` records whether its own process is
trusted for accessibility, native event posting and screen capture, and whether
it can see a logged-in console session. It never requests privacy permission,
changes TCC, opens a system approval prompt or modifies keyboard settings.
Results are retained independently for Apple silicon and Intel macOS.

A positive inventory allows a subsequent external observer experiment; it is not
native interaction, physical input, screen-reader or clean-host evidence. A
negative inventory identifies the exact access boundary for that process and
must not be relabeled as an application failure. It supplies no passing result
for Task2.6 and does not replace the user's recorded ARM physical clipboard checks.

The helper compiles locally. Within the current restricted command environment
it reports no console session or permissions. That observation describes this
process environment; CI must record its own results. VoiceOver remains excluded
by the owner's decision and Windows remains deferred.
