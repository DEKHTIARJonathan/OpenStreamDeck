<p align="center">
  <img src="src-tauri/icons/source.svg" alt="OpenStreamDeck" width="112" height="112">
</p>

<h1 align="center">OpenStreamDeck</h1>

<p align="center">
  <strong>Your apps. Your agents. One deck.</strong><br>
  Turn the N3 into a personal control surface for your desktop and AI workflows.<br>
  Six LCD keys. Three dials. Everything within reach.
</p>

<p align="center">
  <a href="https://github.com/DEKHTIARJonathan/OpenStreamDeck/actions/workflows/ci.yml"><img src="https://github.com/DEKHTIARJonathan/OpenStreamDeck/actions/workflows/ci.yml/badge.svg?branch=master" alt="CI status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--or--later-90dfcb?style=flat-square&amp;labelColor=1b2028" alt="License: GPL-3.0-or-later"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/built_with-Rust-dea584?style=flat-square&amp;logo=rust&amp;logoColor=white&amp;labelColor=1b2028" alt="Built with Rust"></a>
  <a href="src-tauri/Cargo.toml"><img src="https://img.shields.io/badge/desktop-Tauri_2-24c8db?style=flat-square&amp;logo=tauri&amp;logoColor=white&amp;labelColor=1b2028" alt="Desktop app built with Tauri 2"></a>
  <a href="ROADMAP.md"><img src="https://img.shields.io/badge/status-active_development-c2b3f1?style=flat-square&amp;labelColor=1b2028" alt="Active development"></a>
</p>

<p align="center">
  <a href="#quick-start"><img src="https://img.shields.io/badge/Build_from_source-90dfcb?style=for-the-badge&amp;logo=github&amp;logoColor=151a20" alt="Build from source"></a>
  <a href="docs/USAGE.md"><img src="https://img.shields.io/badge/Explore_the_docs-c2b3f1?style=for-the-badge&amp;logo=readthedocs&amp;logoColor=151a20" alt="Explore the documentation"></a>
  <a href="ROADMAP.md"><img src="https://img.shields.io/badge/See_the_roadmap-35446b?style=for-the-badge&amp;logoColor=white" alt="See the roadmap"></a>
</p>

<p align="center">
  <a href="#the-tour">The tour</a> ·
  <a href="#what-can-you-put-on-a-key">Features</a> ·
  <a href="#your-agents-within-reach">Agents</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#contributing">Contributing</a>
</p>

<a href="docs/images/deck.webp"><img src="docs/images/deck.webp" alt="OpenStreamDeck's deck editor with six colorful keys for Terminal, Copy, Paste, Summarize, Agents and Play / pause, three dials, page controls and an action inspector" width="100%"></a>

<p align="center">
  <sub>The real OpenStreamDeck browser UI, using the included example configuration. <a href="docs/images/README.md">Screenshot details</a>.</sub>
</p>

## Less reaching. More doing.

Launch your tools, control your music, run a workflow, or bring an agent session
forward—from the same deck. OpenStreamDeck is a **free, open-source N3
controller** built with Rust, Tauri 2 and React, targeting macOS, Windows and
Linux.

<table>
<tr>
<td width="33%" valign="top">
<h3>⌘ Your everyday shortcuts</h3>
<p>Apps, shortcuts, media and macros, exactly where your hands expect them.
Give each page a purpose and each key a recognizable face.</p>
</td>
<td width="33%" valign="top">
<h3>✦ Your repeatable workflows</h3>
<p>Connect clipboard or file inputs to a model and a next step.
Configure the sequence once, then put it on a key.</p>
</td>
<td width="33%" valign="top">
<h3>◎ Your local control center</h3>
<p>Keep configuration on your machine, credentials in the OS store,
and your choice of AI provider. Stay running from the tray.</p>
</td>
</tr>
</table>

## The tour

Click any image to explore it at full resolution.

<table>
<tr>
<td width="50%" valign="top">
<a href="docs/images/dials.webp"><img src="docs/images/dials.webp" alt="Dial 1 selected in the editor, with separate Press, Turn left and Turn right controls and a whole-dial assignment menu" width="100%"></a>
<h3>01 · Make every turn count</h3>
<p>Press, clockwise and counterclockwise bindings are independent.
Use whole-dial presets for quick setup, or tune each direction yourself.</p>
</td>
<td width="50%" valign="top">
<a href="docs/images/workflows.webp"><img src="docs/images/workflows.webp" alt="The included summarization workflow with Read clipboard, Summarize and Copy result steps, plus the AI prompt inspector" width="100%"></a>
<h3>02 · Put a workflow on a key</h3>
<p>Compose steps, pass results forward and stop on failure.
The included example turns copied text into a summary and copies it back.</p>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<a href="docs/images/providers.webp"><img src="docs/images/providers.webp" alt="Provider settings for OpenAI, Anthropic, Google Gemini and a local endpoint, with configurable endpoint and model and an empty API key field" width="100%"></a>
<h3>03 · Bring your model</h3>
<p>Choose OpenAI, Anthropic, Gemini or an OpenAI-compatible endpoint,
including a local server. Set the model you want to use.</p>
</td>
<td width="50%" valign="top">
<a href="docs/images/settings.webp"><img src="docs/images/settings.webp" alt="General settings showing launch-at-login and menu-bar-only preferences, with an explanation of local storage" width="100%"></a>
<h3>04 · Close the window. Keep the deck.</h3>
<p>Tray mode keeps controls and monitoring running after the editor closes.
Reopen when you need it, or choose Quit to stop the app.</p>
</td>
</tr>
</table>

These are captures of the running React UI with public example data, not
mockups. USB, OS actions and credential storage run in the desktop app; browser
screenshots do not establish native or physical-device validation.

## What can you put on a key?

|                            | Your controls                                                                                                                                                    |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Apps & shortcuts**       | Pick an application with its native icon; open a URL; run a keyboard shortcut; type text; control media; click a mouse button                                    |
| **Artwork & pages**        | Colors, glyphs, labels and uploaded artwork; add, rename and delete pages; fixed Previous/Next buttons with wraparound; draft editing and explicit saves         |
| **Workflows & macros**     | Ordered actions, conditions, cycles, delays and cancellation; clipboard and text-file steps; configured shell commands                                           |
| **AI workflows**           | Explicit inputs, configurable models and endpoints, streaming provider transports and visible execution errors                                                   |
| **Local coding agents**    | Herdr sessions, Codex parent/child discovery and Claude Code lifecycle hooks; dashboard navigation, selection, focus and prompts where the adapter supports them |
| **Device & desktop**       | Automatic discovery/reconnect, brightness, sleep/wake, optional inactivity sleep, tray operation and launch-at-login settings                                    |
| **Portable configuration** | JSON import/export for pages, workflows and settings; validation and a previous-save backup; credentials excluded from exports                                   |

### One key. Three steps.

The included **Summarize clipboard** workflow is a starting point:

```text
    COPY SOME TEXT             PRESS YOUR KEY               PASTE THE RESULT
          │                          │                             ▲
          └──────► Read clipboard ──► AI summary ──► Copy result ────┘
```

Choose a provider and model in **Settings → Agents & AI**, store its API key in
the desktop app if needed, then assign **Run workflow** to a key. Edit the
prompt or add steps to make the workflow yours.

## Your agents, within reach

Open the **Agents** dashboard to see reported activity from your local coding
sessions. **Agent dashboard** brings OpenStreamDeck forward. **Focus** targets
the selected external session. **Show on deck** explicitly puts session status
on the LCD keys; **Return to keys** restores your page.

| Integration     | What is available today                                                                                                      |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| **Herdr**       | Session discovery, pane targeting and supported host-terminal focus. Windows terminal activation remains unavailable.        |
| **Codex**       | Paginated live-thread discovery, distinct parent/child identities and macOS focus. Direct prompt routing is not implemented. |
| **Claude Code** | Lifecycle reporting through additive hooks. Hook setup is currently manual.                                                  |

Ready means a session is between turns—not that its task is complete.
[Set up your integrations →](docs/USAGE.md#agents)

## Made for the N3

**Six LCD keys · Three push dials · Two page buttons · One customizable button**

The supported device is the **N3, USB `5548:1001`**, vendor usage page `0xFFA0`,
usage `1`. Previous/Next always navigate pages; the third ordinary button is
yours to assign. Every dial has separate press, left and right bindings.

The desktop app renders both its native key preview and device JPEGs through one
Rust artwork renderer. It discovers the N3 at launch and retries after a
disconnect. Close the vendor controller before connecting OpenStreamDeck.

> [!NOTE] OpenStreamDeck is actively developed for macOS, Windows and Linux. OS,
> provider and physical-device coverage varies. The
> [verification matrix](docs/TEST-MATRIX.md) records what was tested and what
> still needs observation; the CI badge links to actual workflow results.

## Quick start

### Build and launch

Install Rust and Bun using the versions in
[rust-toolchain.toml](rust-toolchain.toml) and [.bun-version](.bun-version),
plus the
[native platform prerequisites](docs/CI.md#local-checks-and-pre-commit). The
project uses Bun for JavaScript dependencies.

```sh
git clone https://github.com/DEKHTIARJonathan/OpenStreamDeck.git
cd OpenStreamDeck
make setup          # Install the development tools, including the Tauri CLI
bun ci              # Install dependencies from bun.lock
cargo tauri dev     # Start the native app and its frontend
```

### Make your first key

1. **Connect the N3.** Close the vendor app first. OpenStreamDeck discovers the
   device automatically.
2. **Choose a key.** Select **Open app or URL**, then **Choose an application**.
   Its target, name and icon are set together.
3. **Save changes.** Try the key, then add a page, dial assignment or workflow.

Linux needs the included [udev rule](src-tauri/bundle/70-openstreamdeck.rules);
reconnect after installing it. macOS and Windows use the OS HID driver.

<details>
<summary><strong>Development, packaging and platform notes</strong></summary>

`cargo tauri dev` starts Vite automatically on port 1420. React changes reload;
Rust changes rebuild the app. Keep one development server running. For browser
editing and design preview only, use `bun run dev`.

```sh
bun run build       # Check TypeScript and build the frontend
cargo tauri build   # Build the native bundle for this platform
```

| Build target | Packaging      | Distribution notes                                                       |
| ------------ | -------------- | ------------------------------------------------------------------------ |
| **macOS**    | Universal DMG  | Ad-hoc signing; Developer ID signing and notarization are not configured |
| **Windows**  | MSI / NSIS     | Unsigned installers                                                      |
| **Linux**    | DEB / AppImage | DEB includes the udev rule; AppImage users install it separately         |

These are configured release targets. See the
[Releases page](https://github.com/DEKHTIARJonathan/OpenStreamDeck/releases) for
published artifacts and [CI and releases](docs/CI.md) for the packaging gates.
Start with the source build above.

</details>

## Local by design

- **Your configuration stays on your machine.** Export pages, workflows and
  settings as JSON; normal saves keep a previous-file backup.
- **Your keys stay in the OS credential store.** Provider identity and endpoint
  scope each credential. API keys are excluded from configuration exports.
- **Your inputs are explicit.** Clipboard and file reads are configured actions.
  AI steps send the configured input to the provider you choose.
- **Your commands stay configured.** Model output is never implicitly executed.
  Shell steps receive previous output as environment data.

<details>
<summary><strong>Under the hood</strong></summary>

```text
   React editor ─── typed Tauri IPC ──┐
                                     ▼
   N3 buttons & dials ───────► Rust action engine
                                     │
                 ┌───────────────────┼────────────────────┐
                 ▼                   ▼                    ▼
          Desktop actions      AI workflows        Local agents

   Native artwork renderer ──► Editor preview + N3 display frames
```

USB, processes, OS input, persistence and secrets stay in Rust. UI requests and
hardware triggers use the same action engine. A serialized device worker owns
HID writes; one renderer supplies native preview and device frames.

Read the [architecture guide](docs/ARCHITECTURE.md),
[N3 protocol](docs/PROTOCOL.md) and
[shell workflow boundary](AGENTS.md#shell-workflow-data-boundary).

</details>

## Contributing

Help make the deck dependable on more desks: improve an action, add a meaningful
regression, report an OS integration issue, or document a physical N3
observation. Start with [AGENTS.md](AGENTS.md) and the [roadmap](ROADMAP.md).

```sh
bunx --no-install playwright install chromium
make test           # Frontend, browser and Rust tests
prek run -a         # Formatting, lint and dependency checks
```

Automated tests use fixtures and local test services. They require no N3, agent
apps, API keys or OS input permissions. Physical controls and visible device
pixels need separate observation. See the
[hardware diagnostic guide](docs/TEST-MATRIX.md#manual-hardware-diagnostics)
before running manual checks.

| Keep exploring                             |                                                              |
| ------------------------------------------ | ------------------------------------------------------------ |
| [User guide](docs/USAGE.md)                | Configure pages, actions, workflows, agents and settings     |
| [Roadmap](ROADMAP.md)                      | Current capabilities, next priorities and product boundaries |
| [Verification matrix](docs/TEST-MATRIX.md) | Tests, measured observations and remaining gaps              |
| [CI & releases](docs/CI.md)                | Development prerequisites, checks and installer publication  |
| [Architecture](docs/ARCHITECTURE.md)       | Components, execution and trust boundaries                   |
| [Protocol](docs/PROTOCOL.md)               | N3 packets, image transfers and observed variants            |

---

<p align="center">
  <strong>Your desk, connected.</strong><br>
  Free and open source under <a href="LICENSE">GPL-3.0-or-later</a>.<br>
  <sub>Dependency and icon attribution: <a href="THIRD-PARTY-NOTICES.md">third-party notices</a>.</sub>
</p>
