# OpenStreamDeck

**Your apps, workflows and AI agents. One physical control deck.**

OpenStreamDeck is a free, open-source desktop controller for the N3. Turn six
LCD keys and three dials into application launchers, shortcuts, media controls
and repeatable AI workflows. Keep local coding sessions in view while you work.

Built with Tauri 2, Rust and React for macOS, Windows and Linux. Configuration
stays on your machine; AI steps send their configured inputs to the provider you
choose. Development is active, with platform and hardware validation limits
documented in the [verification matrix](docs/TEST-MATRIX.md).

[Quick start](#quick-start) · [User guide](docs/USAGE.md) ·
[Roadmap](ROADMAP.md) · [Contributing](#contributing)

## Why OpenStreamDeck

- **Keep everyday actions within reach.** Launch an app with its own icon, run a
  shortcut, adjust volume or switch pages from the N3.
- **Make the deck yours.** Give keys artwork, colors and readable labels. Set
  each dial's press and rotation actions, or assign all three together.
- **Build repeatable workflows.** Chain configured actions, pass results between
  steps and stop on failure. Use AI with your own endpoint and model settings.
- **See your agents at work.** View sessions from Herdr, Codex and configured
  Claude Code hooks. Keep parent and child sessions distinct, with explicit
  availability and permission states.
- **Stay in control of your data.** Provider keys live in the OS credential
  store. Clipboard and file reads are explicit actions. Commands run only as
  configured; model output is never implicitly executed.
- **Keep working with the window closed.** The default tray mode keeps device
  controls and monitoring running. Reopen the editor when you need it.

## A tour

### Deck: arrange your controls

Select a key, button or dial in the editor, choose its action and save. The two
arrow buttons always cycle through pages; the third ordinary button is
customizable. Add, rename and delete pages beside the page tabs. Changes stay in
a draft until **Save changes**, and the last page is protected.

Use **Choose an application** to set an app's launch target, name and icon
together. For other actions, choose a color, glyph or your own artwork. The
native editor and N3 share the same Rust renderer and JPEG frames.

### AI workflows: turn a sequence into a key press

Create a workflow, order its steps and assign **Run workflow** to a key. For
example, the included summarization workflow reads the clipboard, sends the text
to your configured provider and copies the result back. Each step consumes the
previous result; a failure stops the sequence and appears in the error banner.

### Agents: keep local sessions in view

Open Agents to inspect reported session activity. **Agent dashboard** brings
OpenStreamDeck forward; **Focus** targets an external session where supported.
**Show on deck** explicitly switches the LCD keys to session status, and
**Return to keys** restores page artwork. Ready means a session is between
turns, not that the user's task is complete.

### Settings: choose how the deck fits your desktop

**General** controls launch at login and tray behavior. **Device** controls
display geometry and inactivity sleep, which defaults to disabled. **Agents &
AI** holds provider configuration and adapter status. With **Menu bar only**
enabled, closing the window keeps the controller running; otherwise it quits.
The tray's **Quit** command always exits.

## Features

| Area              | What you can configure                                                                                                                       |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| N3 controls       | Six LCD keys, one customizable ordinary button, two fixed Previous/Next buttons, and three dials with separate press and left/right bindings |
| Desktop actions   | Application picker with native icons, URLs, keyboard shortcuts, text entry, media controls and mouse clicks                                  |
| Pages and artwork | One page collection, wraparound navigation, draft/save editing, colors, Lucide glyphs, labels and uploaded images                            |
| Automation        | Ordered workflows, multi-actions, conditional steps, cycling actions, delays, configured shell commands, clipboard and text-file steps       |
| AI providers      | OpenAI, Anthropic, Gemini and compatible endpoints, including local servers; configurable model IDs and OS-stored credentials                |
| Agent sessions    | Herdr integration, Codex live-thread discovery including descendants, and a Claude Code hook receiver requiring manual setup                 |
| Device behavior   | Automatic discovery/reconnect, brightness, sleep/wake and optional inactivity sleep                                                          |
| Desktop lifecycle | Menu bar/system tray mode, launch-at-login settings and native window restoration                                                            |
| Configuration     | Current-format JSON import/export, validation and a previous-save backup; API keys excluded                                                  |

Capabilities depend on the OS and adapter. Codex focus is implemented on macOS
only; direct Codex prompt routing is not implemented. Windows Herdr terminal
activation is unavailable. Claude hook setup is manual. See the
[user guide](docs/USAGE.md#agents) and
[action coverage](docs/TEST-MATRIX.md#action-coverage) for exact limits.

## Get OpenStreamDeck

Build from source using the quick start below. Check the repository's
[Releases page](https://github.com/DEKHTIARJonathan/OpenStreamDeck/releases) for
published installers. The release workflow is configured to produce:

| Platform | Packages                                  | Installation notes                                                       |
| -------- | ----------------------------------------- | ------------------------------------------------------------------------ |
| macOS    | Universal DMG for Apple Silicon and Intel | Ad-hoc signing; Developer ID signing and notarization are not configured |
| Windows  | MSI and NSIS installers                   | Unsigned; native input and focus need separate OS validation             |
| Linux    | DEB and AppImage                          | DEB installs the N3 udev rule; AppImage users install it separately      |

These are packaging targets, not a claim that an installer or hosted CI run
exists for every revision. See [CI and releases](docs/CI.md) for prerequisites,
signing limitations and the checks required before publication.

## Quick start

Install Rust and Bun using the versions in
[rust-toolchain.toml](rust-toolchain.toml) and [.bun-version](.bun-version).
Install the [platform prerequisites](docs/CI.md#local-checks-and-pre-commit) and
Cargo Tauri CLI; `make setup` installs the development tools.

From the repository root:

```sh
bun ci
cargo tauri dev
```

Tauri starts Vite automatically. React edits reload through Vite; Rust edits
rebuild the desktop app. Use the existing server on port 1420 instead of
starting a second one. The pinned JavaScript CLI also supports
`bun run tauri dev`. Development builds prioritize compilation speed, with no
optimization or LTO and full debug information.

Close the vendor application, connect the N3 and let OpenStreamDeck discover it.
Hardware identification is **N3 USB `5548:1001`**, vendor usage page `0xFFA0`,
usage `1`. macOS and Windows use the OS HID driver. Linux requires the included
[udev rule](src-tauri/bundle/70-openstreamdeck.rules); reconnect after
installation. Keep only one controller application connected to the N3.

Start with an application key, save it, then add pages or a dial assignment. For
an explicit background launch, use `openstreamdeck --background`. Browser
development with `bun run dev` supports editing and preview only; native effects
and hardware access require the desktop app.

## Architecture

The React editor talks to Rust through typed Tauri IPC. Hardware input and UI
requests use the same native action engine; device writes are serialized.

| Component            | Location                                                    | Responsibility                                                        |
| -------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------- |
| Editor               | `src/`                                                      | Deck, workflows, agents, settings, draft reconciliation and typed IPC |
| Configuration        | `model.rs`, `store.rs`                                      | Validated contracts, current-format import and durable saves          |
| Protocol and device  | `protocol.rs`, `device.rs`                                  | Pure packet encoding/decoding, HID lifecycle and input routing        |
| Artwork              | `artwork.rs`                                                | Canonical JPEG frames for the native preview and physical keys        |
| Execution            | `engine.rs`, `actions.rs`, `process_tree.rs`                | Action dispatch, composition, cancellation and process cleanup        |
| Providers and agents | `ai.rs`, `agents.rs`, `codex_adapter.rs`, `focus.rs`        | Provider transports, scoped credentials, session discovery and focus  |
| Desktop integration  | `lib.rs`, `navigation.rs`, `settings.rs`, `applications.rs` | IPC, windows, tray, login registration and application selection      |

Rust filenames are relative to `src-tauri/src/`. USB, secrets, process execution
and persistence remain behind the native boundary. See
[Architecture](docs/ARCHITECTURE.md) for data flow and
[Protocol](docs/PROTOCOL.md) for packet details.

## Contributing

Useful contributions include action regressions, reproducible OS integration
reports, N3 observations and clearer documentation. Read [AGENTS.md](AGENTS.md)
for engineering rules and [ROADMAP.md](ROADMAP.md) for product priorities. Every
supported action and trigger requires regression coverage through the production
implementation with controlled external interfaces.

```sh
bunx --no-install playwright install chromium
make test
prek install
prek run -a
cargo tauri build
```

`make test` runs frontend, browser and backend tests. Prek checks formatting,
lint and dependencies separately. Automated tests need no N3, agent apps, API
keys or OS input permissions. The configured CI sequence is Linux Prek, then
Build and Test on Linux, Windows and macOS; tagged releases wait for all checks
and installer builds before publication.

### Hardware validation and diagnostics

Physical input, visible pixels and OS focus need direct observation in addition
to fixture tests. Remaining gaps are tracked in the
[verification matrix](docs/TEST-MATRIX.md), including unobserved controls,
Windows/Linux device behavior and longer soak tests.

`openstreamdeck --hardware-smoke` is an explicit manual diagnostic. It uses the
existing USB worker, starting a background controller if needed, to exercise
heartbeat, brightness, sleep/wake and all six image transfers, then restore
saved brightness, page and sleep state. Results go to `hardware-smoke.json` in
app data. Accepted HID writes do not establish physical input or display
correctness. Firmware flashing, factory reset and persistent boot-image writes
are outside the project's authorized testing scope.

Raw tracing is opt-in through `OPENSTREAMDECK_USB_TRACE=1`; logs may contain
private artwork and must remain private. Do not publish device serials,
credentials, personal configurations or session transcripts.

## Documentation

| Guide                                      | What it covers                                                     |
| ------------------------------------------ | ------------------------------------------------------------------ |
| [User guide](docs/USAGE.md)                | Pages, controls, workflows, agents, settings and backups           |
| [Roadmap](ROADMAP.md)                      | Current implementation and next priorities                         |
| [Architecture](docs/ARCHITECTURE.md)       | Components, execution, persistence and trust boundaries            |
| [N3 protocol](docs/PROTOCOL.md)            | USB reports, image transfers and observed variants                 |
| [Verification matrix](docs/TEST-MATRIX.md) | Tests, dated observations and remaining integration gaps           |
| [CI and releases](docs/CI.md)              | Tooling, platform prerequisites, dependency warnings and packaging |
| [Contributor guide](AGENTS.md)             | Product constraints, engineering rules and required regressions    |

Configuration v2 stores one page collection plus workflows and settings. Use
**Deck → Export/Import** for backups and transfers; normal native saves keep
`config.backup.json`. The app ID is `dev.openstreamdeck.desktop`, the credential
service is `OpenStreamDeck`, and browser preview storage is
`openstreamdeck-preview`. Exports exclude API keys but may contain private
prompts and paths. Shell steps receive prior output through
`OPENSTREAMDECK_INPUT`; see the
[workflow guide](docs/USAGE.md#workflows-and-providers).

## License and credits

OpenStreamDeck is licensed under [GPL-3.0-or-later](LICENSE). See
[third-party notices](THIRD-PARTY-NOTICES.md) for dependency and icon
attribution.
