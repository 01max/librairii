# Approved native rendering reference

The user approved both **Story shelves** and **Desktop workspace** on 2026-10-20. `approved/` preserves their exact editable source, standalone export and digest manifest from the separate OpenSpec change. `reference-manifest.json` also pins the historical canonical prototype, checksum test, synthetic application screenshots and fixture/capture sources. Preserve those historical bytes; native implementation creates a new reviewed baseline.

Story shelves supplies the browsing/inspector hierarchy, restrained desktop chrome, separate Collections/Shelves navigation, intrinsic-ratio artwork and captions outside the artwork. Desktop workspace supplies compact approved UUID membership with provenance/status and a separate source-review pane. Preserve visible Broken/unavailable state, independent Broken controls, explicit scope and Not now defaults followed by deliberate Apply.

The HTML is design evidence only; it contains illustrative scripts and optional CDN resources. It is never a production runtime, a native screenshot or a toolkit qualification result. The snapshot can be inspected offline as text without contacting its CDN. Approved browser pixels permit documented OS/toolkit typography, controls, chrome, scaling and accessibility adaptations. Unrelated hierarchy or behavior redesign needs a product decision.

Representative omissions do not remove parity obligations:

- All three durable List/Small covers/Large covers modes, search/advanced tags/language/compatibility, sort, pagination, complete-scope counts and bulk selection remain required (P13–P19).
- Full initial seeding, removals/Keep, explicit exclusions/unexclude, unavailable UUIDs, incomplete sources, expired/stale previews, detachment/deletion and duplication remain required (P27–P38). The addition-only demo is not a complete review implementation.
- Independent Broken detail/bulk controls, guarded Undo, warning-only cards, tag colors/derived identities and every export skip/revalidation/no-clobber rule remain required (P20–P24, P44–P49).
- History and notice dismissal, sync visibility, typed navigation/handover, English/French, first-run/location/recovery and actual native picker/menu/reveal behavior remain required (P39–P43, P50–P55).
- Keyboard/screen-reader roles and focus, contrast/non-color cues, reduced motion, IME, long labels, high DPI, independently scrolling short/narrow layouts and every A/I/L/W package gate remain required (P56–P60).

Native screenshot/model/accessibility captures must record the approved source/export hashes, candidate commit, exact OS/architecture, renderer/features, scale/viewport, synthetic fixture, observations and allowed adaptations. Current `docs/assets/*.png` are synthetic **React browser** captures with the documented viewports/clock, not native windows. Browser visual/axe acceptance passed in `../baseline.json`; native toolkit, four-platform screenshots and assistive-technology acceptance remain unimplemented/unverified. Do not rename a browser screenshot into native evidence.
