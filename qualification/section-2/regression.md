# GUI regression route (2.8, P56–P57)

Candidate route: ordinary Rust controller tests plus generated public Slint
component callbacks/property/model APIs. `qualification-desktop --controller-smoke`
asserts search/selection/explicit review decision/apply/reset through the compiled
component. It is deliberately labelled controller smoke; it does not synthesize
native keyboard events, inspect OS accessibility, prove layout or replace AT.

Observed 2026-10-20 by librairii-4 on the ARM host in hosts.md:
`SLINT_BACKEND=winit-skia-software qualification-desktop --controller-smoke`
exited 0 with `controller_smoke=passed`. Search produced one row, selection ID42
was retained, explicit Add changed the count, and Apply cleared count/checkbox
state. Rust 1.99.1, locked Slint 1.18.1 graph. No native input was dispatched.

The pure tests and this executable use no internal `i-slint-*` testing API,
commercial system-testing service, MCP UI server, browser or production library.
No additional GUI testing license was accepted. Slint toolkit attribution still
applies to distributing the compiled spike.

Upstream's [testing.md at 1.18.1](https://github.com/slint-ui/slint/blob/v1.18.1/docs/testing.md)
describes Slint's own infrastructure, not a promised public application API.
The [paid plan terms](https://slint.dev/terms-and-conditions) may include GUI
automation tooling. Presence of `system-testing` in Cargo features is not proof
of free tooling entitlements. It is disabled here.

2.8 remains **partial** until this route runs on each target and a maintained
OS-accessibility automation path is demonstrated with the observed native tree,
events, roles and screenshots. Native interaction access is currently denied on
the ARM test app; Linux/Windows/Intel interactive hosts are not secured. Any
necessary commercial tool must have exact version/terms/cost/owner approval
recorded before using it. No paid dependency is silently introduced.
