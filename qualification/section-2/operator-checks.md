# Task 2 native operator checks

This package is a synthetic qualification spike, not the Librairii product.
It never opens your library/profile, imports archives, or contacts production
services. Pickers accept only this run's disposable fixture directory. Closing
the app cancels and joins its synthetic worker before retiring that directory.
No system accessibility, keyboard, display, privacy, or security setting is
changed by the scripts. Use an already configured reader/IME/session or record
that check as not run. Keep macOS security prompts visible and record them;
the scripts never remove quarantine or change Gatekeeper settings.

Owner scope decision (2026-10-23): skip VoiceOver execution on macOS for this
qualification. Record it as skipped by owner decision; no spoken macOS
accessibility pass is inferred. Other checks below retain their scope.

Use a separate record for ARM macOS, **real Intel macOS**, Linux **X11**, and
Linux **Wayland**. A hosted build or Rosetta run does not supply physical/AT
qualification. macOS apps have an ad-hoc bundle integrity seal; they have no
Developer ID signature or Apple notarization. Public release signing is later.

1. Download the matching `task2-<target>-<commit>` artifact from PR38's **Task 2
   native qualification** workflow. Verify its `.tar.gz.sha256`, extract it,
   and enter the extracted directory. Keep `manifest.json` with the binary.
2. With Python 3 available on the qualification host, create a record:
   `python3 observe.py --package . --operator YOUR_HANDLE --output observations.json`.
   It checks all manifest bytes/native architecture and macOS bundle integrity,
   then starts **every check
   as not_run**. Add host hardware/reader/IME/desktop/file-manager versions in
   observations, without personal paths or credentials. Clean-host launch is
   a separate check and must also run without Python/development tools.
3. Run `./qualification-desktop` on Linux, or open `LibrairiiQualification.app`
   on macOS. A macOS terminal launch uses
   `./LibrairiiQualification.app/Contents/MacOS/qualification-desktop`.
   Repeat with `SLINT_BACKEND=winit-skia-software` for the explicit CPU path.
   Default startup requests Skia's normal renderer; that alone does not prove
   GPU use. Record the selected driver/backend with diagnostics where possible.
   A downloaded macOS app may still require your explicit per-app approval.
   If you trust the verified qualification build, use Apple's normal
   [Open Anyway procedure](https://support.apple.com/en-us/102445). This is a
   manual trust decision, separate from signature integrity. Record an ordinary
   launch block and any subsequent approved launch as separate observations.
   A “damaged” message or failed signature verification is a package failure:
   stop and report it rather than approving that copy.
4. Complete the checks below. In the JSON, replace a check's status with pass,
   fail, or not_run and describe **what actually happened**, including failures,
   exact reproduction, reader speech and capture references. Leave numerical
   gates not_run without actual attributed samples. Do not turn the expected
   behavior below into a purported observation.

## Interaction and accessibility

- Tab/Shift-Tab through search, navigation, three views, selection, review,
  pickers and work controls. Scroll/select an initially offscreen story. Verify
  visible focus and that selection survives view/language changes.
- Open Settings from its button and native menu; initial focus should be Close.
  Tab/Shift-Tab stays inside English/Français/Close, Escape closes and returns
  focus to the actual invoker. Repeat Details and About. Underlying actions
  must be disabled during a dialog. At `--size=560x400`, use Navigation,
  Escape/Close and verify focus return and that every action remains reachable.
- With VoiceOver or Orca already configured, record actual spoken labels,
  list/selection/checked/disabled semantics, dialog navigation and announcements.
  The current overlay exposes a groupbox: explicitly record whether that is
  usable and whether background semantics remain navigable. AX text alone does
  not pass spoken-output qualification. Capture any toolkit limitation.
- Review has 300 candidates/12 pages, at most 25 mounted choices. Leave a page
  untouched (Not now), check Add on two different pages, return and verify both
  choices, Apply, then verify all choices reset and repeat. No implicit approval.
- Start Synthetic work, cancel while running, start again and finish. Record
  progress and a single terminal announcement. Exercise Synthetic error and
  verify the failure/retry message without repeated speech flooding.

## Language, input and OS integration

- Type actual French dead-key sequences and actual CJK IME composition into
  search; test candidate selection, editing and clipboard. Pasting CJK text is
  useful but does not qualify IME composition. Switch interface language with
  unsaved input/selection/work active and verify they remain unchanged.
- Inspect long French labels at wide/intermediate/compact sizes, normal/high
  DPI and mixed monitors; check contrast/focus and existing reduced-motion
  preferences. Record available configurations and omissions.
- In each native picker choose/cancel with the parent visible. Choose **both**
  synthetic files, one file, the fixture folder, then reject an outside folder.
  Verify the parent remains modal and focus returns. Do not inspect personal
  files; only the synthetic fixture choices are needed.
- On macOS verify Settings in the **application menu**, Command-comma, standard
  Quit/Hide/Services and editing commands. On Linux verify the application menu,
  Control-comma and in-app Settings. Repeat English/French. Launch with
  `--settings-early` and confirm buffered startup delivery.
- About in the top-level menu must display the actual AboutSlint widget.
  Reveal fixture must visibly open this run's exact disposable directory in the
  file manager. An exit code saying the request was sent does not pass reveal.

## Rendering, packages and measurements

- Inspect French/Japanese/Chinese glyph shaping on normal and explicit CPU paths.
  For Linux, record a **real GPU initialization failure** (method, session,
  drivers, errors and resulting CPU rendering). Explicit CPU selection or
  `LIBGL_ALWAYS_SOFTWARE=1` alone does not prove failure recovery. Preserve a
  failure rather than altering drivers/security or declaring a fallback passed.
- On a clean native host/user without Rust/Go/Node/source checkout, launch the
  package offline, use both dialogs/menus, close and relaunch. Record missing
  native runtime/portal/font dependencies. Inspect linkage/processes for a
  browser/webview/Go runtime. An ad-hoc seal does not establish ordinary
  Gatekeeper admission; Developer ID signing/notarization remains open.
- Record identified hardware, renderer, sample count/distribution, startup,
  sustained idle CPU, peak RSS and final package size. Native 1,000-story
  completion must be ≤3,000 ms and actual input dispatch p95 ≤100 ms under
  background/thumbnail activity. Run the binary with `--thumbnail-smoke` to
  capture the bounded 1,000-PNG/cache workload and component-dispatch diagnostics.
  This uses original tiny synthetic PNGs and a 64-entry shared LRU; visible
  delegates can also retain their mounted images. Its first-24 marker includes
  queue/event-loop delay; decode p95 is one PNG, not a batch of 24. The construction
  marker, three-second smoke and in-process event timings do not establish
  native first-presentation/input gates or the retained representative 24-artwork
  loads plus full PNG decode ≤50 ms p95. Record these separately with attributed
  samples; neither a fast synthetic result nor CI passes those physical gates.

Return the filled JSON plus referenced screenshots/logs. Redact personal paths
from captures. Missing or failed observations keep Task 2 incomplete and full
feature UI gated; final independent review decides qualification.
