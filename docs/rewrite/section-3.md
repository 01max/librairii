# Section 3: workspace and quality tooling

Owner: librairii-5. Scope: infrastructure beside the working Go/React product.
Parity P57/P59 foundations only; no product workflow or platform qualification.
The separate OpenSpec repository is read-only here and never nested into this tree.

## Plan and commit ledger

Each implementation commit carries `OpenSpec-Task: 3.N`. Full original SHAs are
retained here as subsequent subtasks land; use `git log --format=full` to verify.
Review fixes are separate commits. Section 3.9 stays pending independent AO review
of the final head and clean-checkout checks; unavailable target evidence is incomplete.

| Subtask | Commit | Evidence |
| --- | --- | --- |
| 3.1 | d289e5f | Exact stable 1.99.1, edition 2024/resolver 3, locked workspace build |
| 3.2 | 886b07b828663368f8fcf7cc2e5d990d41f92398 | Typed identity bytes/ranges, distinct group scope, unapproved review default, typed errors and compile-fail boundary |
| 3.3 | Pending commit | Stable rustfmt and Clippy, compiler warnings denied in checks |
| 3.4–3.8 | Pending | Implementation follows the numbered OpenSpec tasks |
| 3.9 | Pending | Independent review and clean-checkout/native target evidence |

## Baseline pins and boundaries

Rust 1.99.1 was verified against the official [release announcement](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/)
and installed through rustup. Normal builds require this stable pin, never nightly.
Workspace MSRV deliberately matches the tested compiler. The root lockfile covers
only the maintained workspace; qualification/section-2 and fuzz are separate graphs.
No GUI toolkit, SQLite, HTTP or archive library is selected by these skeletons.

Dependency direction: UI → app → core. Storage/adapters → app/core; desktop composes
the four outer boundaries. Testkit is synthetic support only. Core cannot import
application, persistence, network or GUI types. Empty module boundaries implement
no product behavior. The desktop executable deliberately exits unsuccessfully
before any data access; it is not an application or packaging acceptance claim.

Typed foundations are intentionally not persisted codecs or the reconciled product contract.
No new repository/product license is set; distribution decisions remain pending.

## Lint policy

Every member inherits compiler and Clippy lints. Safe Rust is enforced (`forbid`);
future OS FFI needs its own deliberately reviewed policy change. Ordinary Clippy
warnings are checked with `-D warnings`. Product/test code returns errors instead
of unwrap, expect, panic, todo or unimplemented. Pedantic/restriction groups are
not enabled wholesale. No generated/vendor exclusions or lint exceptions exist.
Any future exception must have a reason next to a narrow `#[expect(...)]`.
Native matrix jobs compile their actual target-specific modules; a host-only pass
never establishes the other targets. The current skeleton has no native FFI.
