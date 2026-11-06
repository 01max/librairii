# Rust native desktop integration evidence

Historical bootstrap snapshot from 2026-10-20. The intake counts and pending
statuses below describe that revision, not current implementation progress.
For the implemented runtime and contributor commands, use the
[native architecture](../architecture/0005-rust-native-desktop.md) and
[development guide](../development.md). The current
[platform matrix](../release-platforms.md) separates tested native candidates
from the still-open signed-release and runtime-cutover gates. This original
bootstrap record and its frozen identities remain preserved.

**INCOMPLETE — global integration PR must remain draft and must never be merged automatically.**
This index bootstraps `rewrite-in-rust-native-desktop`. No task section is qualified,
no Rust behavior is established, and no task or global merge is recorded here.
The external task list has **0/176** subtasks checked at intake (2026-10-20).
Bootstrap documentation does not complete task 1.1 or any other numbered subtask.
The global PR is [#37](https://github.com/01max/librairii/pull/37); auto-merge is off.

## Branch and review contract

- One global branch, `rewrite-in-rust-native-desktop`, and one global PR targeting
  `main`. **Never merge the global PR.** Disable auto-merge and retain draft status.
- Each of the 20 sections has its own branch and PR, targeting only
  `rewrite-in-rust-native-desktop`, never `main`. Integration owns this index;
  section owners own their branches. Coordinate assignments through the orchestrator.
- Every numbered subtask, including each independent-review subtask, needs at least
  one distinct Conventional Commit with an `OpenSpec-Task: N.M` trailer and concrete
  evidence. One commit cannot stand in for multiple subtasks. Review fixes get
  separate Conventional Commits; a review subtask can commit its factual evidence
  record after independent review. No empty completion claims or checked boxes
  without evidence. The bootstrap commit carries no task-completion trailer.
- Preserve original subtask and fix commit SHAs using merge commits for section
  PRs. No squash, rebase merge, force-push or history rewriting. Integrating a newer
  global base into a section is the section owner's responsibility, by merge.
- The orchestrator triggers AO native reviews on owned implementation PRs. Final
  independent review must cover the final head after every actionable finding is
  addressed and its thread resolved. A clean earlier review is insufficient.
  AO approval does not substitute for required independent GitHub approvals.
- No section/global merges are authorized during bootstrap. Later section merges
  require all admission conditions below and orchestrator authorization of the
  concrete reviewed head; release/runtime retirement stays gated.

## Frozen Go oracle and source/proposal differences

| Pin | Immutable identity / observation |
| --- | --- |
| Go oracle commit | `73a93303a9a44cf0a92eaf525331561ed8c04460` |
| Go oracle tree | `e797e9d59926d83b73fb7c02e5c3611073e89a16` |
| Remote main at intake | Same commit, confirmed with `git ls-remote origin refs/heads/main` |
| Design and parity planning baseline | Same commit; no source commit/tree delta at bootstrap |
| Source cleanliness | Application worktree clean before this index; no tracked `openspec` entry |
| Independent oracle checkout | `/private/tmp/librairii-go-oracle-73a93303`; HEAD/tree and clean tracked status verified; temporary location, not a durable release artifact |
| Baseline execution | Section 1 owns full `make check`, build/tools, smoke, fixture capture and reproducibility evidence; results pending |

Always address the oracle by the full SHA, never by a moving `main`, task branch
or candidate executable. Preserve it in Git ancestry and use isolated checkouts
or `git archive 73a93303a9a44cf0a92eaf525331561ed8c04460` to reproduce its source.
Record oracle fixture hashes/generator identity in section 1. The prepared checkout
is a source pin, not proof of tests, supported hosts or a working Rust application.

Remaining normative differences require section-1 reconciliation before affected
ports, even though structural OpenSpec validation passes:

| Difference | Effective contract and retained evidence |
| --- | --- |
| Effective `curated-shelves` still specifies protection controls and default exclusions on removal; review text retains protected-management wording | No user protection toggle; ordinary removal does not exclude. Preserve historical fields. See `docs/architecture/0004-collections-and-shelves.md`, `frontend/src/ShelfWorkspace.test.tsx`, `internal/shelves/readdition_test.go`. |
| Effective `story-broken-flag` still requires card action buttons; protected tag wording in `story-tagging` disallows recoloring | Current user guide has warning-only cards, independent detail/bulk controls and recolorable protected tags. See `docs/user-guide.md`, `frontend/src/App.test.tsx`, `internal/metadata/projector_test.go`. |
| Effective browsing/localization retain DOM/pixel and document-language requirements; settings/catalog retain unconditional legacy in-place adoption | Rewrite deltas supersede browser requirements and require lifetime exclusion or verified independent library/native-profile/recovery adoption. No unsafe direct adoption inferred from older text. |
| Effective `lunii-metadata-sync` mentions a configurable schedule | Current guide and rewrite contract allow explicit refresh and lazy artwork only; no scheduled catalog sync. |
| Research calls `cargo-audit` optional while normative quality spec/task 3.7 require it | Require **both** cargo-deny and cargo-audit; do not weaken normative requirements to the shorter research command list. |

Saved-query v5, URL v3, visible/counted Broken stories and universal export skips
already exist at this oracle; they are not missing source updates. Retain
`internal/collections/visible_broken_migration_test.go`,
`internal/library/visible_broken_test.go`, `frontend/src/visible-broken-query.test.ts`
and the independent remove/re-add tests when translating behavior.

## Spec path and evidence ownership

The authoritative local spec root is `/Users/maxime/Dev/librairii/openspec`, a
**separate repository** (HEAD at intake
`816a932c02898b3bd73147083bf9501cc7eb763b`). Read the change at
`changes/rewrite-in-rust-native-desktop/` and all `specs/*/spec.md` there.
Its working tree contains uncommitted completed-change archival/reconciliation,
including newly present Collection/Shelf specs; HEAD alone does not pin those bytes.
The digest inventory below pins intake contents, not a reconciliation approval.

Do not publish to that repository, stage its `.git`, add a gitlink, or copy a nested
repository into the app. Use the explicit external path in worker commands;
OpenSpec CLI reads run from `/Users/maxime/Dev/librairii`. If a tool needs a local
spec layout, use a disposable snapshot outside the app worktree, excluding `.git`.
Record intentional spec changes with new digests and reconciliation evidence;
never silently reinterpret changed bytes as the original contract.

This file is the shipping integration index. Section PRs must supply tracked
subtask-to-commit/evidence records in their own scoped documents and link them here
when admitted; avoid simultaneous edits to this integration-owned file. Preserve
P01–P60 and subcase IDs from the parity register with separate A/I/L/W status.
Unimplemented, failed, blocked, skipped and unverified remain distinct from pass.
Non-PR deliverables belong in each worker's AO artifacts directory; this owner's
directory is `/Users/maxime/.ao/data/artifacts/librairii-2`. Retained CI/provider
evidence must identify immutable runs and hashes rather than ephemeral terminal logs.

Ownership: integration (`librairii-2`) alone edits this index and the separate
OpenSpec checklist/reconciliation files locally. Section 1 (`librairii-3`) owns
`preservation/` for contract inventory, generators, corpus, section evidence and
its own subtask commit ledger; it sends precise Broken then Collections/Shelves
reconciliation findings to integration rather than editing those specs in parallel.
Section 2 (`librairii-4`) owns feasibility research and its separate evidence/commit
ledger; `feasibility/` is proposed as its shipping namespace, pending confirmation.
Non-PR research stays in that worker's AO artifacts directory. Contract reconciliation
remains a gate for the bounded toolkit spike. After section 1 is reconciled and
independently reviewed, sections 3–4 may proceed alongside that bounded section-2
spike. Full feature UI still requires toolkit qualification; concurrency never
waives applicable dependency/review gates.

## Section branch and PR slots

01/02 branches were already present at intake; other names are reservations for
coordination, not branches or PRs created by this bootstrap. All PR/evidence and
qualification slots remain pending. Sections 1/2 are assigned above; remaining
named owners are assigned by the orchestrator.

| Section | Branch slot | Numbered subtasks | PR → global | Evidence / qualification |
| --- | --- | --- | --- | --- |
| 1 Product contract | `rust/rewrite-01-product-contract` | 1.1–1.9 | Pending | Pending |
| 2 Feasibility | `rust/rewrite-02-feasibility` | 2.1–2.13 | Pending | Pending |
| 3 Workspace/tooling | `rust/rewrite-03-workspace` | 3.1–3.9 | Pending | Pending |
| 4 Storage compatibility | `rust/rewrite-04-storage` | 4.1–4.11 | Pending | Pending |
| 5 Inspection/import | `rust/rewrite-05-import` | 5.1–5.10 | Pending | Pending |
| 6 Jobs/recovery | `rust/rewrite-06-runtime` | 6.1–6.9 | Pending | Pending |
| 7 Search/navigation | `rust/rewrite-07-queries` | 7.1–7.7 | Pending | Pending |
| 8 Tags/Broken | `rust/rewrite-08-tags` | 8.1–8.7 | Pending | Pending |
| 9 Collections | `rust/rewrite-09-collections` | 9.1–9.6 | Pending | Pending |
| 10 Shelves/review | `rust/rewrite-10-shelves` | 10.1–10.11 | Pending | Pending |
| 11 Metadata/artwork | `rust/rewrite-11-metadata` | 11.1–11.8 | Pending | Pending |
| 12 Export/trash/diagnostics | `rust/rewrite-12-export` | 12.1–12.9 | Pending | Pending |
| 13 Setup/relocation | `rust/rewrite-13-settings` | 13.1–13.9 | Pending | Pending |
| 14 Native foundations | `rust/rewrite-14-native-foundations` | 14.1–14.8 | Pending | Pending |
| 15 Native workflows | `rust/rewrite-15-native-workflows` | 15.1–15.13 | Pending | Pending |
| 16 Scale/resilience | `rust/rewrite-16-resilience` | 16.1–16.6 | Pending | Pending |
| 17 Handover/rollback | `rust/rewrite-17-handover` | 17.1–17.8 | Pending | Pending |
| 18 Platform packages | `rust/rewrite-18-packages` | 18.1–18.11 | Pending | Pending |
| 19 Release readiness | `rust/rewrite-19-readiness` | 19.1–19.7 | Pending | Pending |
| 20 Cutover/retirement | `rust/rewrite-20-cutover` | 20.1–20.5 | Pending | Pending |

## Section admission: mandatory evidence, not a plan-only pass

The integration owner must refuse admission unless **every** condition holds:

1. Capture latest global base **B**, final section head **H**, H's tree, PR URL,
   named owner, section and all `N.M` IDs. Verify provider base is exactly
   `rewrite-in-rust-native-desktop`. H must contain B (`git merge-base --is-ancestor
   B H`); otherwise the section owner merges the new global base and requalifies.
2. Enumerate every subtask's original full commit SHA, Conventional Commit subject,
   `OpenSpec-Task` trailer, parity IDs, fixture/oracle hashes, executable checks,
   target/features, observed outcome and immutable evidence location. Check ranges
   with `git log --format='%H %s%n%b' B..H`; reject missing/combined completion
   commits. Each of the section's expected subtask IDs needs distinct evidence.
3. Require independent review URL/identity and reviewed SHA **H**; all actionable
   findings fixed, fixes separately committed, threads resolved and review rerun
   on final H. Record required provider approvals/checks. Requesting a review,
   self-review, or approval of an earlier head cannot satisfy this condition.
4. Run regression checks on H **including all prior integrated Rust work**, not
   just section tests or tests against the original oracle. Retain every previous
   required suite/fixture plus the new section checks. Maintain an admitted-PR →
   invariant/assertion/test → run-evidence ledger, covering **every prior Rust PR**.
   Reject deleted, disabled or weakened prior assertions without an independently
   reviewed replacement proving the same invariant. Test discovery/count changes
   must be explained; merely obtaining green from a smaller suite is insufficient.
   Each PR must show meaningful behavior coverage, including affected success,
   failure, cancellation and restart/recovery transitions and any material gaps.
   Use focused coverage reports to find missing assertions; no arbitrary blanket
   coverage percentage can replace those scenario checks or qualify a PR.
   After section 3 establishes
   it, run `cargo xtask check` with locked builds, fmt, Clippy warnings denied,
   nextest, **separate doctests**, differential fixtures, native state checks,
   dependency/license/source checks and both cargo-deny/cargo-audit. Record exact
   tool pins, target and shipping feature sets; do not blindly use all-features.
   Before this entry point exists, run all checks delivered by previous admitted
   sections explicitly; absence of Rust CI is not a green Rust result.
5. Preserve applicable Go regression checks while Go is retained: **`make check`
   plus explicit unit, accessibility, responsive and visual lane results**. The
   corresponding standalone commands are `make test-frontend`,
   `make test-frontend-accessibility`, `make test-frontend-responsive` and
   `make test-frontend-visual`. At the oracle, `make check` invokes `test:release`,
   which already runs these four lanes transitively; capture each actual result
   rather than assuming an umbrella command reached every lane after a failure.
   Reuse complete evidenced runs; run missing/affected lanes separately as needed.
   Retain applicable `make build` and Linux packaged CI as well. These checks do
   not cover Rust; a green Go badge alone cannot admit Rust.
   Baseline failures require their oracle evidence,
   explicit attribution and resolution/decision, never silently disabling a gate.
   Every applicable missing/failed/skipped target or dependent mandatory gate
   keeps the section pending. Partial commits/PRs are allowed without qualification.
6. Re-read provider head/base and global ref immediately before any later merge.
   If B, H, spec inputs or required configuration changed, invalidate qualification
   and rerun cumulative checks and independent final-head review. Record the
   orchestrator's authorization for this exact reviewed H. Merge only with
   commit preservation. Verify original subtask/fix SHAs remain ancestors of the
   resulting global head, and record its SHA/tree and merge commit. Its tree must
   match qualified H; an unexpected conflict-resolution tree needs new review/tests.
   Verify the actual resulting global head with applicable cumulative Go/Rust,
   fixture and composition checks; retain its exact tested SHA and results rather
   than relying only on an earlier task head or provider merge-ref status.
   Do not carry evidence across trees or manufacture a pass for an unavailable host.

Each admission record must contain these fields (all currently **pending**):
`section, PR, owner, B, H, tree, spec digests, subtask→commit/evidence mapping,
parity/subcases, prior-PR regression assertion/suite inventory, commands/results, fixture hashes,
OS/hardware/toolchain/renderer/features, A/I/L/W status, independent final review,
resolved findings, provider checks, exact-head merge authorization, merge SHA,
post-merge ancestry/tree validation`.
Future automation/required checks must implement these predicates without weakening
them. This bootstrap does not install Rust tooling, CI or repository protection.

## Qualification and retirement remain pending

All P01–P60 rows and all four targets are **unimplemented/unverified in Rust**:
`aarch64-apple-darwin`, `x86_64-apple-darwin` (real Intel execution required),
`x86_64-unknown-linux-gnu` (X11/Wayland/software rendering), and
`x86_64-pc-windows-msvc`. Toolkit/license/accessibility/IME/software-rendering,
storage/custody/lock lifetime, handover/rollback, performance and actual final-byte
packages/signing/reopen/uninstall gates are pending. No release/support claim or
old Go/Wails/React runtime removal before every mandatory gate and rehearsal passes.
Even then, this session must never merge the global PR.

Tests and acceptance must use explicit synthetic/temp data **and config** roots
and local HTTP fixtures only. Fail before touching production library/profile or
production endpoints. Preserve exact original archive bytes and source hashes;
only documented timestamps/random IDs/temp paths may be normalized in oracle diffs.
Retain all numerical budgets in `parity-matrix.md`; missing hardware, credentials
or comparable performance evidence stays incomplete rather than becoming a waiver.
Compilation/cross-compilation proves only the recorded build result; it must never
set a required runtime, desktop, accessibility or release-platform gate to passed.

Bootstrap validation: OpenSpec change strict validation passed; all 12 effective
specs passed structural strict validation; approved HTML source/export SHA-256
values match their UI manifest. These are not semantic reconciliation, baseline
`make check`, Rust tests, native accessibility or package qualification results.

## Intake content SHA-256 inventory

Paths below are relative to the external spec root. Recompute from the actual
working-tree bytes before relying on them; record changes explicitly.

```text
ff4326f8b42dcf8851877f4f3989c7fecb8f97d8154686d7d74b7281b70e6b9b  changes/rewrite-in-rust-native-desktop/.openspec.yaml
71d587778431ad5c54ce56f9f927fef0bbb046f92750dcea4fbcccdfe8b995d0  changes/rewrite-in-rust-native-desktop/design.md
d0a9bd0627e5961abcfc44ce2d425e2578f9113e18348d54487cbb4703ed75cc  changes/rewrite-in-rust-native-desktop/parity-matrix.md
e2144e06438eccc748cefc0e410930310e6c9d51b8da618c68db2575166666a0  changes/rewrite-in-rust-native-desktop/proposal.md
3ca8c76713e94b97d91011bd64c9a71d122171e941ccf4241ec6965a23fd8d3b  changes/rewrite-in-rust-native-desktop/research.md
e0f0541dec95dedb567590207179864abad8df1838bc4c5f27c057ebbdd9652a  changes/rewrite-in-rust-native-desktop/specs/application-settings/spec.md
ec38cf1ba97e7ff8fb5b86d8973c285b37dccc381d6013c88ba62f2fc3408a0a  changes/rewrite-in-rust-native-desktop/specs/desktop-distribution/spec.md
9b879c80333139ed335e0122aaaf66fb756e4ff2d681f51a829936bc2c88b163  changes/rewrite-in-rust-native-desktop/specs/library-browsing/spec.md
124bffa89025d246f94302e21533ab22f712316b2c3f1c93710fdb37ce12b164  changes/rewrite-in-rust-native-desktop/specs/library-upgrade-compatibility/spec.md
3aea84a1310fb0ee3510b69fcb8080ba9248b7ca1021267b045a06233e7f4a85  changes/rewrite-in-rust-native-desktop/specs/rewrite-feature-parity/spec.md
6cd181bd65ad5fc66e802b733550ee07e45a95c858c851a7d683a25506cec6a7  changes/rewrite-in-rust-native-desktop/specs/rust-native-runtime/spec.md
b8966a2e78635d963a7b6bf0d68ccd24a88bd355646117640f07f5758b325a6a  changes/rewrite-in-rust-native-desktop/specs/rust-quality-toolchain/spec.md
96fa4c02cff63499c4e2b7b2fdebd030219e400e3c7422f576ad89725e598a60  changes/rewrite-in-rust-native-desktop/specs/ui-localization/spec.md
a3f619d3fd18cb22d6f129a74932cca443e743276948ea66a836f1418ccb5af2  changes/rewrite-in-rust-native-desktop/tasks.md
189ec08765496929bff39aeec1c66ff6e34871ac2a7534981bdb7e5ad4476ed6  changes/rewrite-in-rust-native-desktop/ui-prototypes/README.md
7358506ab4c2b14af2bde95a2cc1230ac4bc16fa0f110429cb90b59482773cde  changes/rewrite-in-rust-native-desktop/ui-prototypes/manifest.json
44cd023b9e09e9bd2b411e76e58f5bef70e18b72508e03f25c8aba6343083460  changes/rewrite-in-rust-native-desktop/ui-prototypes/native-desktop.fragment.html
4df3afb9a27c780657f3c4bf0f2b691f346bd00e589468516a5bc655230d2cb1  changes/rewrite-in-rust-native-desktop/ui-prototypes/native-desktop.html
e6ece7cd376a8f69f2e2a576060ee6409be712eac3229b28a472e3e1e9e88d3b  specs/application-settings/spec.md
e0d384d67ca76a7c739202cb0b41d2fd04b1d6c2e8eaadbf2a734392e7f7534e  specs/curated-shelves/spec.md
9ce019fb553eb7fb0103531b9bafe97ad63db8b5f66ddbf4ce1bbbf974b13bdf  specs/library-browsing/spec.md
a34fcbee72d4b289b7bfd1cc9fefe02acbe4de2741b8816ce00a9ccbccf6a50a  specs/library-location/spec.md
68a2f152890be82f871c4fdf419091d9ad44ac4a56519656f050bedd1855e7e6  specs/local-story-catalog/spec.md
0882a6a216cee9e54c44eff9b1eddb424f9efe43c9d41f6a2373f3dd85414418  specs/lunii-metadata-sync/spec.md
e6c33f213e61dac856ad61b93677349502461bf58cf27b9db4bb712b200578a3  specs/lunii-qt-export/spec.md
f226eb3dd6e65e71710177abf0bec2fb0b2b182a91bdeb0d6c8e67d183e456e3  specs/saved-collections/spec.md
f72989419ffa5b51b1a63d4eec73e36d2c919118a54daacd4faed1a1d3eedc96  specs/shelf-update-reviews/spec.md
ff91332f6c7e4feb974cf750cb41abb28c52b2b3c592b95a527b39bc629eabf0  specs/story-broken-flag/spec.md
f63f48f1a4d7096b8d7c1906cc81a55d4101b491703cd301357b2cce0e0284bb  specs/story-tagging/spec.md
237d3d78168fe93e6155a74dd3f3ac4dff0edbacfe9579be5777ea3349f92cfd  specs/ui-localization/spec.md
```
