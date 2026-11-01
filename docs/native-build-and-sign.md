# Build a native package and choose when to sign it

You can build and use an internal native package without Apple credentials.
Developer ID signing and notarization are needed for the public Mac release
and its final Gatekeeper checks. They can wait while development continues.

The helpers below keep those decisions separate. The package helper previews
its work by default. The signing helper only prints a command. Neither helper
provisions credentials, changes GitHub settings, publishes a release or merges
code. Running the printed signing command is your explicit decision.

## Build an internal package

Use a clean, committed checkout and the prerequisites in
[the development guide](development.md#build-hosts-and-tools). Builds require the
pinned compiler and matching native machine: macOS 15+ ARM, actual Intel
macOS 15+, or Ubuntu 24.04 x86-64. Intel qualification cannot use Rosetta.

Preview the inputs, paths and three commands, without building or creating
output directories:

```bash
python3 -I -B scripts/build-native-package.py --target aarch64-apple-darwin --name local-arm
```

When you choose to build, add `--execute`:

```bash
python3 -I -B scripts/build-native-package.py --target aarch64-apple-darwin --name local-arm --execute
```

For Intel use `x86_64-apple-darwin` on an Intel Mac. For Linux use
`x86_64-unknown-linux-gnu` on the recorded Ubuntu baseline. Choose a new name
for each build. An existing directory is refused; failed builds retain their
logs and partial outputs for inspection.

The helper calls the existing sealed release builder, verifies the stage, then
assembles a package. Look in `target/native-release/local-arm/package/` for the
DMG or tar.gz, its checksum and `artifact.json`. The sibling `stage/` contains
build logs, the compiler graph, payload and source identity. The executable's
headless identity commands run during verification; the product GUI does not.
No build dependencies are installed automatically.

Mac candidates have an ad-hoc integrity signature and an `-unsigned.dmg` name.
Building them does not establish public-release or installed-GUI acceptance.
The existing [native packages workflow](../.github/workflows/rust-packages.yml)
builds all three targets and runs installed acceptance on disposable CI.

## Prepare public Mac signing when you are ready

This is a one-time owner setup plus an approval for each selected candidate.
You need an Apple Developer ID Application identity with its private key and
notarization credentials. Prepare a dedicated encrypted keychain containing
only that identity and a `notarytool` profile named `librairii-notary`, following
[Apple's notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow).
Keep your ordinary login keychain separate. If you do not have the identity
or want to defer public distribution, stop here; internal builds remain usable.

In [repository settings](https://github.com/01max/librairii/settings/environments),
create `native-macos-signing`. Configure a required reviewer and select only
the `main` and `rewrite-in-rust-native-desktop` branches for deployments.
GitHub's [environment guide](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments)
explains the controls. A sole owner can keep self-review available so they can
explicitly approve their own dispatch; a team can require another reviewer.

Enter these through GitHub's secure settings yourself:

| Location | Name | Value |
| --- | --- | --- |
| Environment secret | `NATIVE_SIGNING_KEYCHAIN_BASE64` | Base64 of the encrypted dedicated keychain file. |
| Environment secret | `NATIVE_SIGNING_KEYCHAIN_PASSWORD` | Password for that dedicated keychain. |
| Repository Actions variable | `NATIVE_MACOS_TEAM_ID` | The selected Apple team ID. |
| Repository Actions variable | `NATIVE_MACOS_CERT_SHA1` | The selected Developer ID Application certificate SHA-1. |

Do not paste credentials into this repository, build arguments, logs or chat.
The signing helper has no credential inputs and never accesses a keychain.

Review and publish the [signing workflow](../.github/workflows/rust-sign-macos.yml)
and its trusted scripts through your normal merge process. GitHub must have
registered its manual workflow definition, which may require publication on
the default branch. The chosen trusted branch must contain the reviewed
workflow and scripts. This guide and its helpers do not publish them for you.

## Select and approve one candidate

Use a reviewed, passing native-package CI run. Independently check its exact
40-character source commit, architecture and the SHA-256 of the **inner DMG**
against its retained `artifact.json` and checksum. A downloaded provider ZIP
has a different hash. Do not select an incomplete, failed or expired candidate.
Local builds are not inputs to the protected workflow; it downloads the exact
artifact from the selected package run.

Fill these values from that evidence, then preview the dispatch:

```bash
python3 -I -B scripts/prepare-native-signing.py \
  --source "$librairii_source" \
  --candidate-run "$librairii_package_run" \
  --target aarch64-apple-darwin \
  --unsigned-sha256 "$librairii_dmg_sha256"
```

The helper validates input syntax and prints a
[GitHub CLI dispatch command](https://cli.github.com/manual/gh_workflow_run).
It does not contact GitHub or verify the candidate. Review the printed command,
then run it yourself with an authenticated GitHub CLI when ready. Approve the
waiting `native-macos-signing` job in GitHub after checking the selected source,
target, hash and trusted workflow revision. Repeat independently for Intel.

The workflow signs, notarizes and staples the app and DMG, then a separate
credentialless job checks the final installed bytes and GUI. Inspect both jobs
and retain their reports and final checksums. Signing success alone does not
complete release acceptance. Publication and runtime cutover are later manual
decisions; these scripts never perform them. The detailed evidence contract is
in [the package report](rewrite/section-18.md#protected-mac-signing).

Collect the exact archives, checksums, failed attempts and review records with
[the offline evidence-manifest helper](native-release-evidence.md). It links
the selected final distribution to its retained installed bytes for your review;
signing and release acceptance remain owner-operated.
