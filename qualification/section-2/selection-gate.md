# Independent selection gate (2.13)

**PENDING FINAL QUALIFICATION REVIEW. TOOLKIT NOT SELECTED.**
Draft task PR38 targets `rewrite-in-rust-native-desktop` only. No merge authorized.
Source research/builds/controller tests and a local unsigned ARM image are useful
partial evidence, not completion. Exact supporting commits are in ledger.md.

| Mandatory evidence | ARM macOS | Real Intel | Linux X11 / Wayland | Windows11 x64 |
| --- | --- | --- | --- | --- |
| Native candidate build | Local build | Pending | Pending / pending | Pending |
| Mockup visual/native semantics | Interaction access denied | Pending host | Pending hosts | Pending host |
| Keyboard + VoiceOver/NVDA/Orca | Not observed | Not observed | Not observed | Not observed |
| Dead keys / CJK IME / clipboard / mixed DPI | Not observed | Not observed | Not observed | Not observed |
| Pickers / parenting / menus / reveal | Compiled probes only | Pending | Pending / pending | Pending |
| Actual GPU failure + multilingual CPU | Explicit CPU process smoke only | Pending | Pending / pending | Pending |
| Public controller automation | Local controller smoke | Pending | Pending / pending | Pending |
| Actual input/thumbnail budgets + additional ceilings | Diagnostics only | Pending | Pending / pending | Pending |
| SQLite/full archive corpus parity | Independent subset only | Pending | Pending | Pending |
| Clean-host packages / installer / signing | Local unsigned DMG only | Pending | Pending | Pending |

Distribution/license route is proposed, not final. Product terms, complete native
third-party notices and any paid GUI-tool entitlements remain owner decisions.
Host/access questions are with the user/orchestrator; signing secrets were not
examined. Keep every missing gate open. Eligible headless sections3–4 can proceed
on isolated data with independent module boundaries and frozen behavioral inputs.

The orchestrator may request AO native review of this partial PR; do not trigger
one without that request. Native review approval alone cannot supply missing
physical evidence or authorize merging. Final2.13 review must assess the final
head, exact locked/native graph, full evidence, licensing and all four actual
targets after mandatory gates pass. Review fixes need separate commits and their
threads resolved. Do not mark this gate or external task checkboxes complete.

Historical implemented scope through `3131fb68109a0f82caeba1af84500ef86f20e9e5`
received partial AO approval (ea416650), including the actual-widget and pinned
wrapper fixes. Both inline threads are resolved with evidence. This supplies code
review only. New independent Zstd/resource experiments require fresh final-head
review and CI. The observed pre-callback OOM is an open decoder-resource gate;
the container only bounds the qualification experiment. Neither code approval
nor the small payload/dictionary successes complete2.13, product admission,
production isolation, physical, licensing or full-corpus qualification.
Section1 corpus admission is now complete at1cd2244 through integration9a6c1c1;
it supplies the frozen input, not section2 parity or decoder-resource proof.
