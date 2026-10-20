# Signed updates

OpenStreamDeck checks for updates on startup and every six hours, including
while running in the tray with the GUI closed. Settings → General → **Software
updates** also offers a manual check and displays the last-check time.
Development builds use `http://127.0.0.1:8080/latest.json`. Release builds use
the GitHub endpoint configured in `src-tauri/tauri.conf.json`. Both use the
public verification key supplied at build time. Rust owns checking, downloads,
signature verification and installation. **Install Update** downloads, verifies,
installs and restarts in one operation. Release notes render as Markdown in the
prompt and Settings, including headings, lists, emphasis, links, code and
GitHub-style tables. HTML is displayed as text, never executed.

When an automatic check finds an update, OpenStreamDeck brings the GUI forward
and displays **Update Now** / **Skip this Update**. **Update Now** runs the
complete download, verification, installation and restart. **Skip this Update**
remembers that version and suppresses its automatic popups and foregrounding,
even after relaunch. A different available version prompts again. Manual checks
still let you install a skipped version. The notice requires one of these two
choices: it has no close button and cannot be dismissed with Escape or a
backdrop click. Further checks wait while the decision is pending. If you have
unsaved edits, skip to continue editing and install later from Settings.

The timestamp of each completed check attempt (including failures) and the
skipped version are saved in `updater-state.json` in the application's data
directory. They are separate from page/settings exports. Every launch checks
immediately; while running, manual checks reset the six-hour interval.
Scheduling accounts for sleep and clock changes, defers during another update
operation, and never queues a burst of missed checks. History-write failures
remain visible and a failed skip is not treated as saved.

## Try it with cargo tauri dev

Serve your `latest.json` and signed update payloads from a directory outside the
checkout:

```sh
python3 -m http.server 8080 --bind 127.0.0.1 --directory /path/to/update-files
```

The manifest must advertise a version newer than the running app, contain an
entry for its platform/architecture, and include the signature contents and
payload URL. For development, use a payload URL such as
`http://127.0.0.1:8080/OpenStreamDeck.app.tar.gz`. Sign the payload with the
private key matching your configured `TAURI_SIGNING_PUBLIC_KEY`.

Configure the root `.env` as described below and rebuild/relaunch:

```sh
cargo tauri dev
```

The startup check opens the update notice. Choose **Update Now**, or use
**Settings → General → Software updates → Check for updates → Install Update**.
The app downloads, verifies, installs and restarts automatically. There is no
confirmation or cancel step. Save pending changes and finish active workflows
before starting. Progress and errors stay visible.

On macOS, `cargo tauri dev` runs inside the generated
`target/debug/OpenStreamDeck.app`. The updater replaces that bundle and restarts
its new executable. Cargo's separate `target/debug/openstreamdeck` remains the
source-built version: launching `cargo tauri dev` again restores that version in
the development bundle. Let the automatic restart launch the installed version.
Serve a bundled payload built with `cargo tauri build` so the installed app has
its own frontend assets and does not depend on the development server.

Quit any already-running installed OpenStreamDeck first: the single-instance
plugin otherwise forwards the launch to that instance. On macOS, the development
launcher uses a hard link inside its app bundle so Tauri accepts the executable
path. Existing generated symlinks are replaced automatically on the next launch.

HTTP payloads and redirects are allowed only on localhost in development.
Release builds require HTTPS, including artifact URLs and redirects. Synthetic
signing data and loopback servers are confined to automated tests.

## Configure keys

Both [`.env.dist`](../.env.dist) and `.env` belong at the **repository root**,
beside `package.json` and `Cargo.toml`. From that directory:

```sh
cp .env.dist .env
```

Fill in the root `.env` yourself. Neither dotenv file belongs in your home
directory or in `src-tauri/`. Agents must never inspect `.env`. It is ignored by
Git; never put keys in a `VITE_` variable or widen Vite's environment prefixes.

Generate one persistent key pair at the repository root:

```sh
cargo tauri signer generate
```

Copy the printed key values into the environment variables below. Choose a
password at the prompt. Back up the private key and password securely; existing
installations trust the corresponding public key. Replacing the key without a
migration prevents those installations from accepting new updates. See the
[Tauri updater signing guide](https://v2.tauri.app/plugin/updater/#signing-updates).

| Variable                             | Value                                             | Required for       |
| ------------------------------------ | ------------------------------------------------- | ------------------ |
| `TAURI_SIGNING_PUBLIC_KEY`           | Complete single-line public key value             | Every native build |
| `TAURI_SIGNING_PRIVATE_KEY`          | Complete single-line private key value            | Every native build |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password; explicitly empty for an unencrypted key | Every native build |

Set all three as environment variables or fill in root `.env`, which the Rust
build script loads into its process environment. Existing environment variables
take precedence, including empty values. All three are required for native
development, builds, tests and Clippy. Only the public key is compiled into the
application. Missing/placeholder variables, blank keys and malformed public keys
fail with a named configuration error. Dotenv parse errors do not print source
lines. Cargo watches `.env` creation/change/deletion and all three environment
variables, so subsequent Cargo builds rerun the build script when they change.
This is Cargo invalidation, not a promise that a running development watcher
automatically restarts on an ignored file; rerun the command after editing it.

Use the native CLI directly:

```sh
cargo tauri dev
cargo tauri build
```

Both commands load the three variables from root `.env` through `build.rs`.
Local builds produce the native application and platform bundles. Signed updater
artifacts are enabled by the release workflow, which provides signing
credentials directly to `cargo tauri build`. When building signed updater
artifacts locally, export all three variables in the terminal before invoking
the CLI. Loading root `.env` in the Rust build script cannot set variables in
the parent Tauri CLI process that signs the artifacts.

`bunfig.toml` disables Bun's automatic dotenv loading, and Vite/Vitest disable
their dotenv loading too. The native build owns this file; frontend commands do
not need it.

Frontend-only builds need no updater keys.

## GitHub Actions

Set `TAURI_SIGNING_PUBLIC_KEY` in the release workflow's `env` block to the
generated public key value that matches your signing key. The public key is safe
to commit. Add repository Actions secrets named `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Supply the generated **private key
value**. An absent or empty password secret becomes an empty password; encrypted
keys require the correct password. Tauri rejects an invalid key or password
during signing.

Ordinary CI generates a disposable key pair, supplies all three fixture
environment variables and removes the temporary key files. PR checks therefore
need no production secrets. Tag packaging disables that disposable setup and
uses the workflow public key and production signing secrets. Staging and
publication validate signatures against that same public key. Its Cargo cache is
restored but never saved, because Cargo fingerprints environment inputs.

Before invoking `cargo tauri build`, the release step validates the three
variables and writes a temporary public-only Tauri configuration that enables
updater artifacts. Missing/placeholder signing values fail before compilation;
an empty password is accepted. Private values are never written to that file.

The release workflow waits for the complete quality matrix, then packages:

| Feed target                                      | Updater payload                  |
| ------------------------------------------------ | -------------------------------- |
| `linux-x86_64-appimage`, fallback `linux-x86_64` | AppImage                         |
| `linux-x86_64-deb`                               | DEB                              |
| `darwin-x86_64`, `darwin-aarch64`                | The same universal `.app.tar.gz` |
| `windows-x86_64-nsis`, fallback `windows-x86_64` | NSIS `.exe`                      |
| `windows-x86_64-msi`                             | MSI `.msi`                       |

DMG remains an initial-install download; DEB, AppImage and Windows installers
also serve as updater payloads. Each updater payload has a `.sig` sidecar.
Staging rejects missing, duplicate or empty assets and malformed signatures. It
also rejects signature key IDs that differ from the public key; Tauri's bundler
alone only warns about that mismatch. This check is not an independent
cryptographic verification of the artifact: the native updater performs that
verification before installation.

After all packaging jobs succeed, the publisher creates `latest.json` with
version, release notes, UTC timestamp, version-specific HTTPS URLs and the
**contents** of each signature. `SHA256SUMS` covers the payloads, installers,
signatures and feed. Uploads go to a draft; only a complete upload is published.
Prereleases are explicitly excluded from GitHub's latest release.

Release notes come from the matching dated version section in the tagged
`CHANGELOG.md`, for example `## v0.1.0 - 2026-10-20` for tag `v0.1.0`. The
initial release-validation job rejects missing, duplicate, invalidly dated or
empty entries before packaging. Move the release's entries out of `Unreleased`
before tagging; `Unreleased` is never published as the notes.

The Markdown content of that section is copied to both `latest.json.notes` and
the GitHub release description. The app reads only the feed's `notes` field.
Editing the GitHub description afterward is possible, but does not update the
already-published feed or what the app displays.

The configured stable feed is:

```text
https://github.com/DEKHTIARJonathan/OpenStreamDeck/releases/latest/download/latest.json
```

Forks must change the configured endpoint to their own repository. The publisher
uses `GITHUB_REPOSITORY` for artifact URLs. Until a stable release is published,
the feed can return 404. Updater signatures are separate from macOS Developer ID
signing/notarization and Windows Authenticode signing.

## Installing a release update

In a supported macOS app bundle, Windows MSI/NSIS installation or Linux
AppImage:

1. Select **Check for updates** and review the version and release notes.
2. Select **Install Update**. That click starts the entire operation: download,
   signature verification, installation and automatic restart. No further
   confirmation or restart click is required.

Unsaved drafts and active workflows block the operation before downloading.
While updating, the progress panel prevents edits, native code blocks new
workflows and ordinary close/quit, and the operation continues independently of
the webview. Checks have a 30-second deadline; downloads have a ten-minute
deadline. Installation is not cancelled or timed out after file replacement
begins. On Windows, the quiet installer handles relaunch; on macOS/Linux, the
app requests restart immediately after installation.

Only bytes verified by Tauri reach the installer, which verifies them again.
Saved pages and settings remain in application data outside the replaced bundle.
State and progress are restored when a webview reopens. Failures unlock the UI
and workflows and remain visible. **Install Update** retries failed downloads or
installations; after a restart failure, it retries only the restart. Linux DEB
in-app installation remains disabled pending package-manager/privilege
validation; update those installations through the package manager.

## Validate actual version replacement

Use two packaged release builds with different versions and the same signing
key. Build both directly with `cargo tauri build`; export the three signing
variables and enable `bundle.createUpdaterArtifacts` when producing the newer
payload. Keep Cargo, package.json and Tauri versions synchronized.

Install the older app. Serve the newer platform payload and a `latest.json`
manifest over HTTPS, with a certificate trusted by the machine. Set the older
app's `plugins.updater.endpoints` to that feed using a temporary `--config` JSON
file when building it. The manifest needs the newer version, the matching
platform entry, its HTTPS download URL and the complete signature contents.
Alternatively, use the configured GitHub feed after publishing a complete stable
release. No insecure-transport exception is enabled for packaged releases.

From the older installed app, check and select **Install Update**. Verify its
new version, retained configuration, restored tray/window behavior, device
reconnection and OS permissions. Run this separately on macOS, Windows MSI/NSIS
and Linux AppImage. Automated tests inject installation/restart boundaries and
never overwrite an application. Hosted publication and actual platform upgrades
remain manual validation requirements; see the
[verification matrix](TEST-MATRIX.md#updater-preparation).
