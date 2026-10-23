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

## Review P2: actual checkbox → Apply regression

The review of9eef0a2 correctly found that direct `decide` callbacks missed the
checkbox's own assignment replacing a one-way binding. Read the exact pinned
[binding rule](https://github.com/slint-ui/slint/blob/v1.18.1/docs/astro/src/content/docs/reference/language/bindings.mdx)
and [Cupertino CheckBox implementation](https://github.com/slint-ui/slint/blob/v1.18.1/internal/compiler/widgets/cupertino/checkbox.slint).
The three probe decisions now have writable boolean properties linked with `<=>`;
Apply resets those same properties. This keeps widget checked/accessibility state
synchronized after its own toggle instead of relying on a replaceable expression.

`qualification-desktop --widget-smoke` uses the public generated focus hooks and
`Window::dispatch_event_with_result(WindowEvent::KeyPressed/KeyReleased)` to route
Space through the actual CheckBox and Apply Button handlers. It asserts the
widget's checked property (the property used by its accessibility state), approval
count and backing decision, then repeats the entire cycle. It does not invoke
`decide` or `apply-review` directly. The public state/focus hooks require retaining
the tiny review controls as static instances; their container is hidden/zero-height
when review is closed.

Observed on the ARM host: the new widget-event regression failed on the old
one-way binding with exit101, `actual widget must reset after Apply`; after the
two-way correction it exits0 with `widget_smoke=passed`. The final regression
does not call show/activate for an OS window and sends no OS input; events remain
inside Slint. No denied Computer Use permission is bypassed, no private testing
crate/commercial service is enabled, and no screen-reader/physical keyboard or
visual observation is claimed. This remains local in-process regression evidence.

## ARM native clipboard regression (2.5 / 2.6)

On 2026-10-23 the owner reported that Search would not paste in the sealed
`9f50fc6` ARM package. Codex reproduced twice with native UI automation. The
minimal red check focuses Search, clears its value, pastes synthetic `0042`,
and asserts that the accessibility field contains `0042`; it failed with an
empty field. This tests actual clipboard/Command-V dispatch, not `set_query`.

The custom application menu used predefined AppKit editing selectors, including
`paste:`, but the Winit view has no corresponding native text responder. Custom
Edit actions now use the public LineEdit editing functions through a weak UI
handle. Native clicks and conventional shortcuts share this route. Actions are
guarded by actual Search focus and both toolkit/native dialog ownership; they
retain the widget's selection and undo history and its normal edited callback.

The exact native paste check passed after the fix, as did select-all replacement
with `Été 日本語`, Undo and Redo. An independent review identified the need to
retain AppKit editing for native picker text fields. Both picker callbacks now
scope a standard responder Edit menu to their native dialog; a Drop guard
restores the application menu and clears native ownership on return/unwind.
Native Go-to-Folder Edit-menu Select All/Copy/Paste round-tripped an authored
`/tmp/SyntheticPasteProbe` path. Cancelling both dialogs preserved Search `0042`,
and subsequent native Search select-all/paste changed it to `0057`.
**Picker keyboard accelerators did not act in automation; that observation is
retained as unresolved, separately from the passing picker menu-click route.**
No IME, spoken accessibility, Intel/Linux clipboard or full input gate is claimed.

`--edit-smoke` supplements this OS check with public widget input, real selection
replacement and resulting model contents, Undo/Redo, native-dialog scope cleanup,
and modal/unfocused no-edit guards. It is included in the bounded raw and
packaged-app smoke runner. It does not claim to exercise the native clipboard or
AppKit menu dispatch. Retain the native empty-Search -> paste -> AX-value check
when changing this adapter; component-only checks missed the original defect.
