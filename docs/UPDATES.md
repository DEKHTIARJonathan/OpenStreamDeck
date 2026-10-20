# Signed updates

The build and release pipeline prepares Tauri 2 updater artifacts and a GitHub
Releases `latest.json` feed. The native updater plugin is registered with a
build-time public key. **Update checks, download/install controls and restart
handling are not implemented yet.** Registering the plugin does not make network
requests or install anything.

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

The configured stable feed is:

```text
https://github.com/DEKHTIARJonathan/OpenStreamDeck/releases/latest/download/latest.json
```

Forks must change the configured endpoint to their own repository. The publisher
uses `GITHUB_REPOSITORY` for artifact URLs. Until a stable release is published,
the feed can return 404. Updater signatures are separate from macOS Developer ID
signing/notarization and Windows Authenticode signing.

## Planned in-app behavior

1. Add **Check for updates** to Settings → General, backed by typed native IPC.
   Rust owns checking, download state, signature verification and installation.
   Return explicit states for up-to-date, available, unsupported installation,
   offline, feed errors and invalid signatures. Do not poll automatically yet.
2. Show version and release notes, then require an explicit install action.
   Preserve unsaved drafts and block installation while workflows are active.
   Keep update state native so closing/reopening the webview cannot lose it.
3. Report download progress and failures. Define cancellation/timeouts and
   serialize concurrent requests. Restart only after a successful verified
   installation and explicit user confirmation, using the existing native
   lifecycle cleanup. Account for Windows installers exiting the running app.
4. Exercise production orchestration with loopback manifests, disposable signed
   artifacts and injected install/restart boundaries. Cover no update,
   malformed/missing platform data, wrong key, corrupt payload, HTTP errors,
   interrupted download, duplicate requests, busy workflows, unsaved drafts and
   install/restart failures. No automated test may install over the running app.
5. Manually validate an upgrade between two real packaged versions on macOS,
   Windows MSI/NSIS and Linux DEB/AppImage. Check configuration retention,
   restart, tray/device cleanup and OS permissions. Document Linux DEB behavior
   explicitly before offering updates to DEB installations.

Hosted release execution and real installation remain separate validation steps.
See [the verification matrix](TEST-MATRIX.md#updater-preparation).
