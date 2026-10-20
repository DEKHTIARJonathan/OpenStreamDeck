# OpenStreamDeck

A local N3 controller built with Tauri 2, React and Rust. Configure six LCD
keys, a customizable button and three dials; use two fixed page buttons; compose
actions and AI workflows; monitor local agent activity. This is an actively
developed implementation, not a claim of full feature parity with the vendor
application.

## Develop

Install Rust (the repository pins its toolchain), Bun 1.4.2, the native Tauri
prerequisites, and the Cargo Tauri CLI. Linux dependencies are listed in
[CI setup](.github/actions/setup/action.yml).

```sh
bun ci
cargo tauri dev
```

`cargo tauri dev` starts Vite automatically. React changes reload through Vite;
Rust changes rebuild the native app. Do not start a second server on port 1420.
Dev builds have no optimization or LTO and retain full debug information. The
pinned JavaScript CLI is also available as `bun run tauri dev`.

The N3 connects automatically. Settings → General controls **Launch at login**
and **Menu bar only**. With Menu bar only enabled (the default), closing the
window keeps the controller running in the system tray; with it disabled,
closing quits the app. The tray Quit command always terminates it. A cold
background launch is `openstreamdeck --background`. Automatic sleep defaults to
0 (disabled), configurable in Settings → Device.

## Check and package

```sh
make test
prek install
prek run -a
bun run tauri build
```

Install the Chromium test browser once with
`bunx --no-install playwright install chromium`. Install prek as described in
[CI and releases](docs/CI.md). Every automated test uses fixtures or local test
services and runs without an N3, OS input permissions, agent apps or API keys.
Manual hardware diagnostics are separate from the test suite.

Pushing a matching `vMAJOR.MINOR.PATCH` tag triggers all-platform CI and
packages Linux DEB/AppImage, universal macOS DMG and Windows MSI/NSIS.
Publication waits for every required job and installer. See
[release requirements](docs/CI.md), including signing limitations. Configured CI
is separate from observed hosted results; check the workflow run for the
revision being released.

## Device access and diagnostics

macOS and Windows use the OS HID driver; no vendor driver is bundled. Linux
requires the included [udev rule](src-tauri/bundle/70-openstreamdeck.rules),
installed automatically by the DEB. Reconnect the device after installing the
rule. Do not run the vendor application and OpenStreamDeck against the same
device simultaneously.

With OpenStreamDeck running, `openstreamdeck --hardware-smoke` asks its existing
USB worker to exercise heartbeat, brightness, sleep/wake and all six LCD image
transfers, then restore the saved brightness, page and sleep state. It writes
`hardware-smoke.json` in the app-data directory. If no app is running, that
command starts the background controller and waits briefly for connection. This
is an explicit manual diagnostic, never a CI test. HIDAPI write acceptance does
not verify visible pixels or physical button operation.

Raw USB tracing is disabled by default. For a diagnostic launch only, set
`OPENSTREAMDECK_USB_TRACE=1`; the resulting app-data logs may contain private
key artwork and should not be published. API credentials remain in the OS
keyring and are scoped to the saved provider ID, type and endpoint.

## Documentation

- [N3 USB protocol](docs/PROTOCOL.md)
- [Verification matrix and integration gaps](docs/TEST-MATRIX.md)
- [Using pages, actions, workflows and agents](docs/USAGE.md)
- [Architecture and data boundaries](docs/ARCHITECTURE.md)
- [CI, prek and tag releases](docs/CI.md)

Unit/browser tests and hardware write checks establish different kinds of
validation. Windows/Linux physical behavior, some OS actions, display
orientation, remaining physical inputs and longer soak tests still need direct
validation. Agent Ready means between turns, not completion of the user's task.

Licensed GPL-3.0-or-later. See [LICENSE](LICENSE) and
[third-party notices](THIRD-PARTY-NOTICES.md).

### Pages and fixed navigation buttons

There is one collection of pages. The two arrow buttons always move to the
previous/next page, including empty pages, and wrap at either end. They cannot
be assigned other actions. Add pages using **+ beside the page tabs** in Deck,
then save. Rename the selected page with the blue **Rename page** button beside
**Delete page**, then save. To remove a page, select its tab, click **Delete
page**, confirm, then **Save changes**. At least one page must remain.

The configuration format is version 2, with a single page collection. Import
accepts the current schema and rejects unsupported formats. Native saves retain
the previous configuration as `config.backup.json`. Use **Deck → Export** and
**Deck → Import** to back up and restore configurations.

The OpenStreamDeck application ID is `dev.openstreamdeck.desktop`, and its OS
credential-store service is `OpenStreamDeck`. Browser preview storage uses
`openstreamdeck-preview` and is separate from native storage. Exported
configurations do not include provider API keys. Shell actions receive prior
output through `OPENSTREAMDECK_INPUT`.
