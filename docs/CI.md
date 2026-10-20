# Automated validation and releases

OpenStreamDeck validates the project on **Linux, macOS and Windows** and gates
tag publication on that matrix. Automated tests use local fixtures and do not
require hardware, agent applications or provider credentials.

## Local checks and pre-commit

Install Bun **1.4.2** (also `.bun-version` and `packageManager`), Rust
**1.96.0** (`rust-toolchain.toml`) and platform Tauri prerequisites.

Configure the native build's three signing environment variables using
[`.env.dist`](../.env.dist) and [the key setup guide](UPDATES.md#configure-keys)
before running Cargo checks. CI generates disposable signing variables for these
checks; it does not require production credentials.

Install development tools through the shared Make target:

```sh
make setup
bun ci
bunx --no-install playwright install chromium
prek install
prek run -a
make test
```

On Linux, use `playwright install --with-deps chromium` and install the packages
listed in
[.github/actions/setup/action.yml](../.github/actions/setup/action.yml). On
Windows, install the MSVC C++ build tools and WebView2; on macOS, install Xcode
command line tools. GitHub runners provide those Windows/macOS tools. prek
provisions its isolated generic checks and actionlint hook; the project checks
use the installed Bun/Rust toolchains. The first run needs network access to
provision tools. Normal tests do not contact provider APIs.

`prek run -a` checks merge conflicts, filename case conflicts,
JSON/TOML/YAML/XML, Python literals/docstrings/debug statements, private key
signatures, trailing whitespace, final newlines, frontend/tooling/documentation
formatting, YAML style and Actions syntax. Rust hooks run formatting checks and
Clippy directly through Cargo; Clippy uses default features so a clean checkout
does not need built frontend assets. Tests are deliberately excluded from
pre-commit.

Apply Prettier with `prek run prettier -a`; it covers TypeScript, TSX,
JavaScript, JSX, CSS, JSON and Markdown. Apply Rust formatting with
`cargo fmt --all`.

`make test` runs these separate targets in sequence, even with `make -j`:

1. `make test-frontend`: frontend unit tests (`bun run test`) and browser tests
   (`bun run test:e2e`).
2. `make test-backend`: `cargo test --workspace --all-targets --locked`.

`make test` runs tests only: no formatting, Clippy or production builds. Backend
tests use the default features and do not require frontend assets. CI runs Rust
formatting and Clippy in the Linux Prek job, followed by the Build and Test
matrix. Backend tests use `--all-features` after the frontend build because
`custom-protocol` embeds those assets. Bun runs frontend tests; Cargo runs
backend tests. CI runs each command explicitly, so its Windows runner does not
need Make. The browser runner reuses a developer's existing Vite server locally.
CI starts and stops its own server. No workflow starts the actual native app or
opens USB, clipboard, real application windows, agent sessions or provider
credentials. Tests exercise production logic through controlled boundaries; they
cannot prove that a real OS focus call or physical display behaved correctly.
See [TEST-MATRIX.md](TEST-MATRIX.md) for the remaining integration gaps.

The dependency hooks also always run: cargo-deny checks advisories, licenses and
sources against `deny.toml`; cargo-audit checks `Cargo.lock`; cargo-machete
checks unused dependencies. CI installs cargo-audit with cargo-binstall, with
source compilation disabled. The other tools are provisioned by pinned hooks.
Machete uses its source scanner because its metadata mode incorrectly reports
`tauri-build`, which is used in `build.rs`. No dependency is exempted from
unused-dependency scanning.

Four maintenance advisories remain in Tauri 2's transitive Linux stack and have
individual, reasoned cargo-deny exceptions: proc-macro-error
(`RUSTSEC-2024-0370`), zvariant (`RUSTSEC-2026-0370`), zbus_names
(`RUSTSEC-2026-0371`) and zvariant_derive (`RUSTSEC-2026-0372`). Remove these
exceptions when the corresponding dependency updates are available in Tauri's
GTK stack and single-instance plugin and Keyring's Secret Service backend. New
advisories still fail cargo-deny.

Cargo-audit continues to report those maintenance warnings and the upstream
[glib VariantStrIter unsoundness advisory](https://rustsec.org/advisories/RUSTSEC-2024-0429.html).
Tauri 2's GTK3 dependencies require glib 0.18; the fix is in glib 0.20 and is
not a compatible lockfile update. This remains an upstream limitation, not a
resolved finding. No cargo-audit warnings are suppressed. Dependency audits
fetch the public advisory database; application tests use controlled local
fixtures and need no external services or credentials.

Keyring 4.2 uses its explicit `v1` feature for macOS Keychain, Windows
Credential Manager and Linux Secret Service, replacing the old per-platform
features. Reqwest 0.13 uses `rustls` in place of `rustls-tls`, with JSON and
streaming enabled and default features disabled. See the upstream
[Keyring API](https://docs.rs/keyring/4.2.0/keyring/v1/index.html) and
[Reqwest features](https://docs.rs/reqwest/0.13.5/reqwest/#optional-features).
Provider request/stream/cancellation tests use loopback HTTP. Credential
identity fixtures protect existing endpoint-scoped account names; actual
credential-store access and live HTTPS providers still require separate
integration validation.

## Continuous integration

[dependabot.yml](../.github/dependabot.yml) checks Bun, the root Cargo workspace
(including `src-tauri`) and GitHub Actions weekly. Bun and Cargo updates target
direct dependencies with `versioning-strategy: increase`, preferring manifest
and lockfile changes together. Dependabot has no strict filter requiring both:
broad Cargo requirements can still yield lockfile-only updates, and this policy
does not guarantee exclusion of lower-bound-only changes. See the
[Dependabot versioning strategies](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference#versioning-strategy).

[ci.yml](../.github/workflows/ci.yml) runs for pushes to `master` and pull
requests targeting `master`. The tag release workflow also calls it to validate
releases. The sequence is:

1. **Prek**, on `ubuntu-latest`: lint, formatting and dependency checks.
2. **Build**, on `[ubuntu-latest, windows-latest, macos-latest]`: the Tauri
   build runs `bun run build` once and compiles the production native
   executable. Cargo then compiles all-feature backend test binaries with
   `--no-run`.
3. **Test**, on those same matrix runners: frontend unit, Playwright and
   all-feature backend tests. Browser failure reports are retained.

The matrix depends on the Linux Prek job succeeding. Build and Test stay in the
same job on each OS, sharing the checkout, `target/` and freshly built `dist/`.
Each compilation is a separate step, so a failed build prevents all tests.
Formatting and Clippy are not repeated in the matrix. Prek does not execute
tests.

Windows MSVC builds embed the Common Controls v6 application manifest through
the linker for all targets, including library unit-test executables. Tauri's
default resource embedding reaches application binaries only; tests that link
its dialog imports can otherwise exit before the harness starts with
`0xc0000139` (`STATUS_ENTRYPOINT_NOT_FOUND`). The build script applies the
[upstream workaround](https://github.com/tauri-apps/tauri/issues/13419#issuecomment-3398457618)
using `src-tauri/windows-app-manifest.xml`, while retaining static VC runtime
linking. Running the Windows backend tests verifies that the harness loads;
`--no-run` alone cannot establish this.

The shared setup uses `dtolnay/rust-toolchain@v1` to install Rust/Cargo and the
components declared in `rust-toolchain.toml`, including for release packaging.
The shared setup installs cargo-binstall with
`cargo-bins/cargo-binstall@v1.26.0` and installs the native Tauri CLI with it.
Native builds invoke `cargo tauri` directly. The Prek job installs prek with
`j178/prek-action@v3.0.2`, then installs cargo-audit using the same binary
installation approach as `make setup`. Missing tool binaries fail installation
instead of falling back to compilation. The local setup target and prek hook
configuration are unchanged. These tools use their current releases without CI
version overrides. Rust and Bun versions come from `rust-toolchain.toml` and
`.bun-version`.

Only Cargo is cached between runs. The shared
[toolchain action](../.github/actions/setup/action.yml) uses a common
[rust-cache shared key](https://github.com/Swatinem/rust-cache#example-usage),
partitioned by OS and architecture, with automatic Rust environment and
dependency hashes. Prek restores the Linux cache but does not save it.
Successful matrix jobs save after both build and tests, so an incomplete
lint-only cache cannot occupy the immutable key first. On a cold cache the first
Prek job and matrix start without saved artifacts; subsequent runs can reuse the
populated cache. Cache misses always fall back to compilation.

Release packaging uses the same runners, Cargo settings and shared cache.
Signing jobs restore the cache but never save it, so production signing
environment fingerprints cannot enter the shared cache. Release and test
profiles still require distinct compiler outputs; both are retained in the Cargo
target cache. macOS universal packaging also compiles both target architectures.
No frontend dependency or output cache is added; frontend builds remain
inexpensive and run once per matrix job.

All platforms install dependencies with `bun ci`. Tests need no N3, installed
agent application, API key or test account. Workflow actions and remote hook
revisions use release version tags. CI has read-only repository permissions.

These are configured checks, not a claim that hosted jobs have passed. Cache hit
rates and timings must be measured in a hosted run. Require `Prek` and the three
`Build and test (...)` checks in branch protection if they should gate merging;
repository settings are not modified by these files.

## Tag releases

[publish.yml](../.github/workflows/publish.yml) responds only to `v*` tag
pushes. A tag such as `v0.1.0` must match the versions in `package.json`,
`src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` exactly. Update the Cargo
lockfile's workspace package version when changing versions. Prerelease tags
such as `v0.2.0-rc.1` are supported and marked prerelease. Inconsistent versions
fail before packaging. The tagged revision must also contain exactly one dated,
nonempty `CHANGELOG.md` section for that version. Neither workflow accepts
manual dispatch.

The release sequence is:

1. Validate the tag, versions and matching changelog entry.
2. Run Linux Prek, then the three-platform Build and Test matrix on the tagged
   revision.
3. Build Linux DEB/AppImage, macOS universal app/DMG (Apple Silicon + Intel),
   and Windows MSI/NSIS installers, including signed updater payloads and `.sig`
   sidecars. All matrix jobs must succeed.
4. Require every expected format, reject duplicates, empty files and signatures
   with mismatched key IDs, then generate `latest.json` using the matching
   changelog section as its notes, followed by `SHA256SUMS`.
5. Create a draft with the same notes, upload all assets, and publish only after
   successful upload.

Before tagging, move the release's curated entries out of `Unreleased` into a
heading such as `## v0.1.0 - 2026-10-20`, leaving `Unreleased` at the top for
future changes. Prereleases use the complete version, for example
`## v0.2.0-rc.1 - 2026-10-20`. The content below that heading, up to the next
level-two heading, supplies the release notes. Category headings alone do not
count as notes. The pipeline never uses `Unreleased` as a fallback or derives
notes from commits, tag messages or pull requests.

Maintainers can edit a GitHub release description after publication, but that
does not rewrite its uploaded `latest.json` or checksums. Keep the desired
shared text in `CHANGELOG.md` before tagging; GitHub-only edits are not
automatically synchronized to the app.

Only the final publication job can write repository contents. Installer jobs
have no release credentials. If upload fails, an unpublished draft remains for
inspection; a retry fails explicitly if a release with the same tag already
exists. Remove the failed draft before rerunning. Existing published assets are
never overwritten automatically. No tag, remote push or release is created by
local validation. Release publication starts when maintainers push a matching
tag.

macOS bundles use ad-hoc signing; **Developer ID signing and notarization are
not configured**. Windows installers are unsigned. Operating systems may show
untrusted-publisher warnings. Distribution signing requires separate certificate
configuration. The signed updater feed and native check/download/install
controls are implemented; actual OS upgrades remain a manual validation gate.
Configure the workflow public key and two Actions secrets described in
[UPDATES.md](UPDATES.md#github-actions) before pushing a release tag. Linux
packages are built on the `ubuntu-latest` runner image; deployment on other
distributions requires separate validation. No proprietary vendor drivers or
firmware are bundled. macOS/Windows use the platform HID APIs. The Linux DEB
installs the N3 udev rule; AppImage users must install it themselves:

```sh
sudo install -m 644 src-tauri/bundle/70-openstreamdeck.rules /etc/udev/rules.d/70-openstreamdeck.rules
sudo udevadm control --reload-rules
```

Reconnect the device after rule installation. This rule is part of the
repository and does not require downloading a vendor driver.
