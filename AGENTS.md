# OpenStreamDeck contributor guide

OpenStreamDeck puts desktop actions, AI workflows and local agent sessions on
the N3. The architecture is a React editor over typed Tauri IPC, a Rust action
engine shared by UI and hardware triggers, and a serialized device worker.

Start with the [README](README.md) for the product, [ROADMAP](ROADMAP.md) for
priorities and [verification matrix](docs/TEST-MATRIX.md) for evidence and
remaining gaps. The rules below apply to human and agent contributions alike.

## Engineering principles

- **Keep one execution path.** UI requests and physical controls use the native
  action engine. Test production orchestration through injected boundaries.
- **Keep effects native.** React edits configuration and renders state. Rust
  owns USB, processes, OS input, credentials and persistence.
- **Keep one source of pixels.** Native preview and device frames come from the
  same renderer, including glyphs, labels, rotation and JPEG encoding.
- **Handle failures visibly.** Invalid imports, malformed reports and
  unavailable adapters must produce useful errors. Preserve drafts and saved
  state when an operation fails; add a regression for each corrected behavior.
- **Match claims to evidence.** Implementation, fixture tests, API-level I/O and
  physical observations establish different things. Document the limits.

## Guide map

| Working on                   | Read first                                                                                                                           |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Product or documentation     | [Product scope](#product-and-authorized-scope), [documentation policy](#keep-this-guide-current)                                     |
| Setup or dependencies        | [Commands](#commands-and-dependency-management), [process ownership](#development-process-ownership)                                 |
| Native or frontend code      | [Architecture](#architecture), [regression coverage](#required-regression-coverage)                                                  |
| N3 packets or rendering      | [Protocol](#device-protocol-and-validation), [pixel fidelity](#pixel-fidelity-and-runtime-behavior)                                  |
| Pages, settings or lifecycle | [Runtime behavior](#pixel-fidelity-and-runtime-behavior)                                                                             |
| Providers or sessions        | [AI behavior](#agent-and-ai-behavior), [adapters](#adapter-correctness), [session identity](#agent-identity-activity-and-sub-agents) |
| Shell workflows              | [Data boundary](#shell-workflow-data-boundary)                                                                                       |
| Validation or release        | [Reporting](#validation-and-reporting), [CI and release gates](#review-ci-and-release-requirements)                                  |

## Product and authorized scope

OpenStreamDeck is a local N3 controller and AI workflow deck, using Tauri 2,
Rust, React and TypeScript. Primary hardware is N3 USB `5548:1001`, vendor usage
page `0xFFA0`, usage `1`. Support macOS, Windows and Linux; explicitly
distinguish platform build coverage from physical hardware validation.

Use OpenStreamDeck for product branding, `openstreamdeck` for packages and the
executable, and `OPENSTREAMDECK_` for environment variables. Keep UI, manifests,
release tooling, storage identifiers and documentation synchronized when naming
changes. Identify hardware by model and USB IDs. Use explicit placeholders when
sanitizing records; retain the measured build identity and validation date.

On macOS, development launches must display OpenStreamDeck in the Dock too. The
debug entry point re-execs inside a lightweight app bundle under the target
directory before Tauri starts. Preserve the PID, arguments and stdio so Cargo
reload and termination still work. Verify the running OS application name;
embedded plist contents alone do not prove the Dock label is correct. Use a
regular file/hard link for the development bundle executable, never a symlink:
Tauri rejects symlinked executable paths when initializing the updater. Refresh
the hard link when Cargo replaces the binary and cover the bundled subprocess
with Tauri's actual starting-executable validation.

The UI is a modern control deck. Include OS/application shortcuts, media
controls, pages, macros, artwork, AI workflows and Claude Code/Codex/Herdr
session status. Exclude other device families, weather, vendor
accounts/store/cloud services, and a general-purpose AI chat/document editor.

The user authorized reversible tests on the attached N3. Do not flash firmware,
factory-reset it, or test persistent boot-image writes. Preserve existing vendor
profiles and do not run the vendor application and OpenStreamDeck against the
same device simultaneously. Hardware buttons require a human to operate; do not
claim physical input verification based on injected software events.

## Commands and dependency management

### Updater environment and secrets

Both `.env` and `.env.dist` live at the **repository root**, alongside
`package.json` and `Cargo.toml`. Never place or look for these files in the
user's home directory or in `src-tauri/`. From the repository root, the user
creates their private configuration with `cp .env.dist .env`.

**Agents must NEVER read `.env` or other private dotenv files**, including with
search, diff, logging, diagnostic tools or indirect commands that print their
contents. Do not edit or overwrite them. Only `.env.dist` is safe to inspect.
Configure signing with key values in the three environment variables below; do
not introduce named signing-key files. Build code may load the root `.env` at
build time without logging its contents; this is not permission for an agent to
inspect it. Validate with synthetic environment values and temporary fixtures
outside the checkout.

Native builds require all three environment variables:
`TAURI_SIGNING_PUBLIC_KEY`, `TAURI_SIGNING_PRIVATE_KEY` and an explicitly set
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty is valid for an unencrypted key).
Only the public verification key may be embedded. Use `cargo tauri dev` and
`cargo tauri build` directly. Do not add a Bun Tauri script or a build-command
wrapper. Rust loads the root `.env` variables; CI supplies signing credentials
directly to the CLI environment. Fail clearly without printing values. Track the
root `.env` and these variables with Cargo rebuild directives. Keep private
keys/passwords out of frontend environment prefixes, generated configuration,
release artifacts and logs. Ordinary CI uses disposable signing variables; only
tag packaging receives production signing secrets. Disable disposable signing
setup for tag packaging so it cannot overwrite the release public key. Build,
staging and publication must use the same public verification key. Release
packaging must not save its Cargo cache because Cargo fingerprints environment
inputs.

Document only the supported build/update flow, without unused legacy methods.
Keep update checks, downloads, verification and installation native. Check once
on startup and six hours after the last completed automatic or manual attempt,
including while the GUI is closed. Persist the last-check timestamp and skipped
version atomically in app data, separately from editable configuration.
Automatic availability brings the GUI forward and restores a native-backed
Update Now / Skip this Update notice. The update notice must offer only these
two choices: no close button, Escape dismissal or backdrop dismissal. Keep it
open until a choice succeeds, and defer further checks while the choice is
pending. Preserve unsaved edits; if they block installation, explain that
skipping lets the user continue editing and install later from Settings.
Skipping suppresses automatic prompts and foregrounding for that exact version
across restarts; a different version prompts again. Manual checks may still
install a skipped version. Test timers with virtual time, foregrounding through
the window boundary, persistence failures, startup hydration, skipped/new
versions and serialization with active installations. One **Install Update**
click authorizes download, verification, installation and automatic restart
without confirmation or cancel controls. Keep the complete sequence native and
independent of the webview lifetime. Reject unsaved drafts and active workflows
before downloading; block edits, new workflows and ordinary close/quit through
automatic restart. Permit the updater’s own restart exit. Serialize update
requests and release the work gate on failure. Check every running action in the
cancellation registry, including overlapping runs of the same binding. Serialize
configuration saves with installation so queued IPC cannot persist changes or
alter login registration during an update. Development builds use
`http://127.0.0.1:8080/latest.json`; release builds use the configured GitHub
feed. Development supports real, one-click installation from the local feed. On
macOS, replace the generated development `.app` bundle and restart that bundle;
keep the Cargo executable separate so a later Cargo launch can restore the
source-built version. Test the complete download/install/restart orchestration,
failure recovery, serialization and development installation policy with
injected installers. Both use the configured signing public key; no demo updater
button or embedded fixture server. Keep synthetic feeds/keys test-only.
Production feeds, payloads and redirects use HTTPS; development permits HTTP
payloads and redirects only on localhost. Test production orchestration with
loopback responses and injected installer/restart boundaries; automated tests
must never replace an application. Report packaged OS upgrade validation
separately from download/signature tests.

Run commands from the repository root. Use Bun for all JavaScript dependencies;
do not use npm, pnpm, yarn, or add their lockfiles. The root `package.json` is
Bun's standard manifest (not `packages.json`). Commit `bun.lock` and the root
`Cargo.lock`.

Keep Dependabot configured for Bun, the root Cargo workspace and GitHub Actions.
Prefer updates that change both declared and resolved dependency versions over
lower-bound-only or lockfile-only churn. Use direct dependency updates with the
`increase` strategy as a best effort; Dependabot cannot enforce both-file
changes for every update. Validate the configuration with the YAML hooks.

```sh
bun install                 # local dependency installation
bun ci                      # frozen installation from bun.lock
bun update                  # intentional dependency update
bun run dev                 # browser UI development
cargo tauri dev             # full desktop app; cargo-tauri must be installed
bun run build               # TypeScript + production frontend
bun run test                # frontend tests
bun run test:watch          # rerun affected frontend tests on changes
bun run test:e2e            # Playwright browser tests
make test                   # frontend and backend tests only
cargo test --workspace      # Rust tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo tauri build           # native platform bundle
```

Use Prettier for TypeScript, TSX, JavaScript, JSX, CSS, JSON and Markdown via
`prek run prettier -a`. Keep frontend source included in the hook; JSX must be
formatted into readable blocks. Use `cargo fmt --all` to apply Rust formatting
before running the formatting check.

Use `make setup` to install development tools with cargo-binstall. CI installs
cargo-binstall and prek with their dedicated GitHub Actions, and Rust/Cargo with
the Rust toolchain action. Install cargo-audit in CI with cargo-binstall and
disable its fallback to source compilation. Keep CI installation changes in the
workflows and shared CI action; do not modify the user's Makefile setup target
or prek configuration to implement them.

Do not duplicate tool version pins in CI. Read Rust and Bun versions from their
existing toolchain files; install the current prek, cargo-binstall and
cargo-audit releases without explicit tool-version overrides. GitHub Actions
still use release tags.

The root Cargo workspace contains `src-tauri`; no custom `cargo tauri run`
command is needed. Native builds require the platform Tauri prerequisites.

Keep development builds optimized for compilation speed: the root Cargo dev
profile disables optimization and LTO, preserves full debug information, enables
incremental compilation and uses 256 codegen units, including build
dependencies. Do not add dependency optimization overrides to dev. Release
tuning is separate.

## Architecture

| Boundary      | Location                                                                    | Responsibility                                                                                      |
| ------------- | --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Editor        | `src/`                                                                      | React UI, typed IPC client, physical N3 editor, workflows, agents and settings                      |
| Draft state   | `src/config-draft.ts`, `src/page-editor.ts`                                 | Reconcile native updates and saves without losing pending edits                                     |
| Configuration | `model.rs`, `store.rs`                                                      | Configuration contracts, validation and persistence                                                 |
| Packet format | `protocol.rs`                                                               | Pure byte encoding/decoding; no OS or UI effects                                                    |
| Hardware      | `device.rs`, `diagnostics.rs`                                               | Vendor-interface HID lifecycle, serialized writes, display updates and explicit manual diagnostics  |
| Pixels        | `artwork.rs`                                                                | Canonical renderer for native preview and device frames                                             |
| Execution     | `engine.rs`, `actions.rs`, `process_tree.rs`                                | Local actions, workflow composition, cancellation and process cleanup                               |
| Providers     | `ai.rs`                                                                     | Provider adapters; API credentials stay in the OS credential store                                  |
| Sessions      | `agents.rs`, `codex_adapter.rs`, `focus.rs`                                 | Herdr/Codex interfaces, Claude lifecycle bridge and OS focus                                        |
| Desktop       | `lib.rs`, `navigation.rs`, `settings.rs`, `applications.rs`, `macos_dev.rs` | IPC, lifecycle, window/tab state, login settings, application picker and macOS development identity |
| Documentation | `docs/`                                                                     | Maintained guides, architecture, protocol and verification summaries                                |
| Fixtures      | `tests/fixtures/`                                                           | Sanitized inputs consumed by automated tests                                                        |
| Tooling       | `scripts/`                                                                  | Development and release utilities                                                                   |

Rust filenames are relative to `src-tauri/src/`. See
[ARCHITECTURE.md](docs/ARCHITECTURE.md) for execution and data flow. These are
responsibility boundaries within the current workspace, not separate crates.

Keep project source, maintained documentation and sanitized fixtures required by
application tests in this repository. External checkouts, exploratory tools and
raw diagnostic archives belong outside every checkout. Generated dumps and raw
diagnostic captures are not documentation. Curated product screenshots requested
for public documentation may live in `docs/images/`; retain only the selected
assets and record their version, revision, capture date and rendering surface.
Use public example data, exclude secrets, device serials and personal sessions,
and distinguish browser previews from native screenshots and physical-device
evidence. Keep capture scripts, intermediate images and visual QA outside the
repository. Verify the rendered README, image links and captions after updates.

Keep USB, secrets, process execution and persistence in Rust. Do not add broad
shell/filesystem permissions to the webview. Do not expose raw arbitrary USB
writes through frontend IPC. Property inspectors/plugins are separate trust
boundaries. Preserve unknown imported actions instead of silently deleting them.

## Device protocol and validation

N3 descriptor: 512-byte input payload and 1024-byte output payload. HIDAPI
outbound buffers include the report-ID prefix, so they contain 1025 bytes. Do
not confuse that API convention with the USB report descriptor. The second
interface is a keyboard, not the ordinary vendor-control interface.

Consult `docs/PROTOCOL.md` and the sanitized test fixtures before changing
packets. Image sizing, rotation and release values can vary by firmware.
Distinguish configured defaults and supported packet variants from live
observations. Unknown firmware/variant behavior must not be silently
generalized.

Each supported capability should link implementation, tests and validation
limits. Distinguish source inspection, fixture replay, API-level hardware I/O
and physical observation. HID writes succeeding do not prove the screen looks
correct. API call traces are not USB bus captures.

Do not commit device serials in new public fixtures, credentials, private
configurations, prompts or session transcripts. Runtime logs stay in app data;
keep private diagnostic material outside this repository. Retain licenses and
attribution for reused code and assets; a dependency's top-level license does
not automatically cover every bundled component.

## Agent and AI behavior

Herdr is the preferred source for Herdr-managed sessions. Use its versioned
socket API; distinguish reported state from independently verified outcome.
Codex uses supported app-server interfaces; Claude Code uses structured events
or additive hooks. Preserve existing integrations and permission behavior. Never
infer success from silence, scrape unrelated personal transcripts, or
auto-approve an agent request. Deduplicate by real session IDs, not titles.
Unavailable adapters must display a useful unavailable/unknown state.

AI model IDs are user-configurable. No hardcoded claim about a latest model.
Support OpenAI, Anthropic, Gemini and local compatible endpoints. Explicit
workflow inputs only; no background clipboard/document collection. Execute only
configured command steps, not arbitrary model-generated commands. Credentials
must never enter configuration exports, frontend snapshots or logs.

## Validation and reporting

Use meaningful protocol fixtures, provider event fixtures and persistence tests.
Test malformed/truncated packets, simultaneous inputs, image chunk boundaries,
cancellation, reconnect, missing providers and unavailable agents. Validate
frontend behavior in Playwright and the actual native application separately.
Keep hardware/OS/provider limitations in the verification report. Report
failures and unfinished work plainly; a skeleton, mocked flow or catalog entry
is not a completed capability. Avoid placeholder controls that appear
functional. Run relevant checks before widening test scope.

Browser event fixtures must give each subscription its own cleanup identity,
including repeated subscriptions using the same callback. React StrictMode
cleanup must not remove a newer listener; preserve hardware-error banner
coverage in `tests/regressions.spec.ts`.

## Required regression coverage

The user explicitly requires tests for EVERY supported action and trigger. This
overrides the usual preference to skip tests for low-impact edits. Keep the
action catalog, native dispatch and coverage matrix synchronized. Every new
action must have success, invalid-configuration and failure-path coverage;
asynchronous actions also need cancellation and timeout coverage where
applicable. Exercise the production orchestration with controlled external
interfaces, not a separate implementation of the behavior in test code.

Test all six LCD keys, all three ordinary buttons, all three dial presses and
both directions of every dial. Include release variants, malformed/truncated
reports, interleaved inputs and rapid rotation. Replay the sanitized real-device
reports in `tests/fixtures/live-input.json`. Never equate replay with physical
verification of a control that was not observed.

Changing an action type MUST reset incompatible values/options/children. A
previous `agent_focus` value such as `selected` must never become a brightness
value. A whole-dial assignment must explicitly configure press, clockwise and
counterclockwise bindings; a press-only edit must be labeled as such. Test
assignment to every dial and preservation of unrelated bindings. Show execution
errors in the UI, including errors originating from hardware triggers.

Keep `docs/TEST-MATRIX.md` accurate: supported surface, corresponding test,
integration check and remaining limitation. A test count is not a coverage
claim. Unknown extension actions must survive import but fail explicitly if
executed.

## Pixel fidelity and runtime behavior

### Native artwork

Preserve the React design the user approved. Render that design onto the N3. The
native artwork renderer is the canonical source of frames for BOTH the hardware
and native editor preview, including Lucide glyphs, labels, colors, dimensions,
rotation and JPEG encoding. Do not reintroduce text-only device artwork or
independent UI stand-ins. Compare reconstructed USB image payloads with preview
bytes in tests; offer a preview at the native pixel dimensions. Fit glyphs to
their painted bounds with minimal margins and use large, bold, width-fitted
labels; wrap to two lines and mark overflow instead of shrinking long labels
into tiny text. The browser-only design preview is not evidence of hardware
pixel fidelity.

### Connection, tray and login

Connect the N3 immediately at launch and retry discovery/reconnect
automatically. Explicit Disconnect applies to the current process. With Menu bar
only enabled, closing the GUI must release its webview while USB, actions and
monitoring remain running behind the tray icon; reopening must recreate the
window from current native state. With Menu bar only disabled, closing the main
window quits the app. Explicit Quit always terminates it. Left-clicking the tray
icon must immediately restore or recreate and foreground the GUI without opening
a menu; right-click opens the tray menu. Cover both mouse phases and buttons,
window restoration/creation and OS failures at the production boundary. Preserve
tray mode by default for existing configurations; automatic sleep defaults to 0
(disabled), without overwriting an existing sleep preference.

Settings uses General, Device and Agents & AI tabs. Launch at login must both
register and unregister through the native OS adapter, verify the resulting
state, and roll back registration if saving fails. Read OS registration at
startup so external login-item changes are respected. Cover both toggle
directions, invalid settings, OS/persistence failures, close/quit policy and
login startup behavior with injected boundaries; verify native lifecycle
separately from browser fixtures. Login launch opens the GUI when tray mode is
off and stays in the tray when it is on.

### Performance evidence

Keep memory claims measured. Record platform, build, connected hardware, active
adapters/workflows, cold background versus closed window, and metric. macOS
physical footprint and RSS are different; do not present main-process memory as
the whole application if webview helpers remain. Do not extrapolate a macOS
measurement into a verified Windows/Linux number.

### Application picker

Open App must be an application-picker flow. Selecting an application sets its
launch target, name and full-key native icon together; never require manual path
entry or a separate icon upload for the normal flow. Keep URL/custom targets as
an optional advanced path. Extract OS icons using structured arguments, never
interpolate an application path into shell/script code. Test missing apps,
extraction failures and atomic selection updates. Async picker completion must
apply to the latest editor state while retaining the captured target. Native
updates during lookup must survive; deleted targets must fail visibly without
restoring an old configuration or dismissing the draft.

### Agent dashboard

The Agent dashboard action must foreground OpenStreamDeck and select its Agents
tab, including when the window is minimized or its webview has been closed. Keep
the requested tab in native state so startup cannot lose navigation events;
action completion must not overwrite the requested tab. Dashboard navigation and
agent selection must not enable the LCD session overlay or focus an external
agent. Only the explicit Show on deck control enables the overlay. Test window
creation/restoration and failures at the OS boundary, navigation startup races,
and preservation of all six native frames and bindings. Record actual OS focus
and physical display validation separately from those automated tests.

### Sleep and wake

Sleep must latch after a press and survive release reports (both `0` and `2`).
Only a new press or dial rotation may wake the device, and that wake input must
be consumed rather than also executing its binding. Resolve sleep-action toggles
inside the serialized device worker; explicit wake remains idempotent. Cover all
controls and release variants through the production input-routing policy.

### Pages and configuration

Keep one flat page collection. Derive contexts from page metadata: all
unassociated pages belong to Default, and application pages share a context only
when their complete normalized identity sets are equal. Identity order, names
and icons do not affect grouping. Allow repeated exact sets across pages; reject
duplicate identities inside one association and partial overlaps between
different sets. Do not introduce a persisted context, workspace or profile
layer.

Use two levels of direct navigation: Default/application context buttons, then
the selected context's page tabs. Keep Add application visible beside contexts
and Add page, Rename page and a red Delete control in that order beside page
tabs. Do not add a page overflow menu or Move left/Move right commands; pages
reorder by drag and drop, with Alt+Shift+Arrow keys as a keyboard alternative.
Keep `dragDropEnabled: false` in the native window configuration so Tauri's
file-drop handler does not consume HTML page-tab drops. Cover this production
configuration in native tests; Chromium drag tests alone do not establish
dragging in the desktop webview. Window recreation must use the same setting.
Include a separate red Delete application button beside application controls,
disabled for Default. Confirm the application name and cascade deletion through
all its pages in one draft edit, preserving other contexts and reporting
surviving links. Cover Cancel/Escape, stale contexts and failed saves. Use
options for secondary management. Overflow scrolls with visible controls; do not
replace navigation with dropdowns or repeated role cards. Support arrow keys and
Home/End with one tab stop per selected row. Keep the page being edited separate
from the page on the device. Live labels use saved configuration, and automatic
foreground updates must neither erase drafts nor mark them dirty. Cover long
labels, same-named pages in different contexts, overflow, keyboard dismissal and
focus restoration.

First-time Add application must combine application selection, first-page
naming, Default fallback and the automatic-switching preference in one dialog.
Create these draft changes atomically, preserve bindings and expose Save
changes. Add page inherits the selected context's complete association. Changing
an application or moving its pages to Default applies to the whole context.
Cover cancellation, missing identities, duplicate contexts, stale fallback
choices and failed saves without losing the draft.

Drag reordering stays within a context and changes only that context's occupied
slots in the flat array. Preserve other contexts, page IDs, bindings,
`active_page` and the fallback designation. Provide keyboard movement and
visible insertion feedback. Save the draft order before using it on the device.
Cover both directions, boundaries, overflow, save failure, reload and navigation
order.

Rust observes application identity. A distinct application activation selects
the first saved page in its matching context; unmatched applications select the
designated Default. Do not remember the last runtime child per application.
Automatic changes never persist `active_page` or rotate save backups. Explicit
navigation temporarily overrides automatic selection until the foreground
application changes or the user resumes it; same-application events preserve the
override. Previous/Next cycles only the current runtime page's context in saved
array order, even with automatic switching disabled. Explicit page-ID actions
may cross contexts. Cover first-page activation after reorder, manual child
overrides and fallback selection independently of Default's position.

Device frames and input bindings must use the same saved page snapshot. Capture
the binding before asynchronous execution; coalesce automatic updates to the
latest target, defer them during sleep and restore the latest target on
reconnect. The explicit Agents overlay takes priority without losing the
underlying page target. An applied overlay retains its transmitted LCD session
identities while ordinary buttons and dials use the current underlying saved
page. Keep overlay mode in one authoritative native state; requesting an overlay
does not change input ownership until its frames are transmitted. Test these
policies through native injected boundaries and browser fixtures. Foreground
capability varies by OS: report unavailable detection visibly and retain manual
controls. Never infer physical display correctness from HID success.

The Previous/Next hardware buttons (`button1`/`button2`) have fixed roles and
must never be assignable. Resolve them before page bindings, including on empty
or imported pages; imported assignments cannot override navigation. Keep their
inspector read-only. Wrap within the current page context; explain that
navigation needs more than one page in that context. Preserve imported
configuration data without executing those reserved-button assignments. Cover
empty pages, imported overrides, wraparound and the read-only editor.

Use one flat page collection with `pages` and `active_page`. Saving and JSON
import/export operate on the app's page and settings configuration. Do not
introduce a separate workspace or profile layer. Keep only current features:
delete code, tests, fixtures and documentation together when removing a feature.
Preserve locally imported optional fields through concurrent native updates,
including property additions and removals. Cover configuration round trips,
validation, normal save backups and preservation of unknown extension actions.
Current-format application-page settings (`page_switching` and page
`application`) can be written by another build sharing the app-data directory.
Preserve these fields through native startup, IPC, editing, import/export and
save/reload even when this build cannot execute page switching; show that
limitation at startup. Never fix compatibility by dropping fields or resetting
saved configuration. Reproduce startup regressions through `settings::load` with
public synthetic fixtures before fixing them, and assert subsequent saves retain
the complete configuration and original backup.

Page deletion must be visible beside the page tabs, confirm the named target,
protect the last Default page and select a remaining neighbor in the same
context. Deleting the last application page removes its derived context and
selects an ordinary page. Deleting the designated fallback requires an explicit
ordinary replacement. Apply deletion through the normal draft/save flow,
preserving unrelated bindings and pending local edits. Report links to deleted
pages without silently rewriting their targets. Cover cancel/Escape, stale
targets, active-page selection and save/reload behavior.

### Transient execution state

Do not add an Activity tab or retain action/input history. Keep only active-job
bookkeeping needed for execution/cancellation, and the latest input for the live
key highlight. Drop completed outputs after workflow consumption; provider
streaming must not copy them into UI history. Send action failures directly to
the shared error banner, including hardware-triggered errors.

## Adapter correctness

Follow the installed adapter's actual transport and schema. In particular,
`codex app-server proxy` is a raw WebSocket tunnel, not newline-delimited JSON
RPC. Contract tests must exercise the real WebSocket handshake and messages. Do
not claim detection alone proves focus or prompt routing works.

Herdr pane focus and OS terminal-window activation are separate steps. Test
target selection and invoke both when the user requests foreground focus;
surface missing/ambiguous host-window capabilities. Preserve agent permission
prompts and never infer completion from a timeout. No integration test may send
a prompt to someone else's/live working agent without explicit authorization.

## Keep this guide current

Update this file when the user changes a constraint or a regression reveals a
missing engineering policy. Record the reusable rule and its verification, not a
running transcript. Update affected documentation and tests in the same change.
Keep unresolved work visible; do not silently redefine completion.

### Public documentation

Keep the README focused on product value, a tour of real capabilities, feature
and architecture tables, setup and contribution paths. Use OpenStreamDeck's
voice and actual implementation when adapting another project's documentation;
retain required attribution for reused material. Reference projects do not
change our product scope, stack, licensing or engineering rules.

Maintain `ROADMAP.md` as a public product direction with implemented
capabilities, next priorities and explicit boundaries. Link implementation
claims to `docs/TEST-MATRIX.md`; distinguish pending validation from missing
features. Update those descriptions with behavior changes. Do not invent
screenshots, download availability, sponsorship channels, performance figures or
platform certification. Check local documentation links and Markdown formatting
when editing these guides. Keep private task tracking outside the repository.

### Temporary agent files

Keep agent plans, task lists, TODO tracking, progress notes, review notes, and
scratch lessons outside the repository tree, including all submodules. Use a
unique task directory under `/tmp/` (for example, create one with
`mktemp -d /tmp/openstreamdeck-task.XXXXXX`) or another OS temporary directory
whose resolved path is outside every checkout.

Never create or update these working files inside the checkout, even in ignored
directories such as `tasks/`, `todos/`, `.cursor`, or `plans/`. Never stage or
commit them.

Do not append new work to any tracked task or lesson files. Keep durable product
and architecture documentation in `docs/`, without embedding task checklists or
linking to temporary files. Before an authorized commit, inspect the staged
filenames and exclude all agent working files.

## Development process ownership

The user runs `cargo tauri dev` interactively. Do not leave a second Vite server
on port 1420 or a stale OpenStreamDeck instance that captures single-instance
launches. Before starting a native dev session, inspect existing
listeners/processes. Reuse the user's frontend server for browser checks. Stop
only test processes you started, using their recorded PIDs; never kill an
arbitrary port owner. Prefer finite `bun run build` and test commands while the
user's watcher runs. Do not run competing Cargo builds against the same target
directory. Preserve `strictPort: true`; silently moving the frontend port breaks
Tauri's dev URL.

## Agent identity, activity and sub-agents

A finished turn is NOT a completed user task. Map Herdr done and Claude Stop to
Ready; retain raw status for diagnostics. Process exit means Exited, not goal
completion. Never infer completion or success from idle/disconnected/stale
state.

Enumerate Codex's live thread registry with pagination and metadata-only reads.
Also enumerate descendants of live roots with explicit child source filters;
unloaded children remain discoverable without being labeled completed. Default
thread/list queries exclude sub-agents. Preserve parent thread IDs, child
identities and nicknames; do not deduplicate by name or route every child to its
parent. Focus must use the selected child's technical thread ID, and hardware
focus errors must surface in the GUI. Test pagination, repeated cursors,
parent/child identity, idle-to-working transitions, unavailable focus and
transport errors. OS deep-link dispatch succeeding is not proof the intended
view appeared.

## Review, CI and release requirements

CI runs in order: Prek, Build, Test. Run Prek only on `ubuntu-latest`, then gate
a matrix of `[ubuntu-latest, windows-latest, macos-latest]` on its success. Keep
Build followed by Test on the same matrix runner so they reuse the Cargo target
directory and frontend assets. Compile test binaries during Build without
running them. Cache only Cargo dependencies and compilation outputs, shared by
OS, architecture and toolchain; do not cache the frontend. Prek restores the
Linux cache, while successful matrix jobs save it after all checks complete.
Pre-commit (`prek run -a`) runs lint, formatting and dependency checks only; do
not add tests to its hooks. Bun runs frontend tests and Cargo runs backend
tests. TypeScript must not implement hardware effects; keep those in Rust behind
typed IPC. `make test` orchestrates the frontend and backend test suites
serially. Do not include formatting, Clippy or production builds in `make test`;
CI runs those as separate steps. CI also tests all backend features after
building frontend assets for `custom-protocol`. All AUTOMATED tests must run
without hardware, agent applications, provider accounts, API secrets, OS input
permissions or external network services. Inject device transports and OS-input
backends; use loopback HTTP/WebSocket fixtures. Manual `--hardware-smoke`
diagnostics are separate and never invoked by CI or hooks. Do not retain ignored
live-only tests that require a user's running session.

Process-tree termination tests must establish that fixture processes are alive
before cleanup and wait for their termination afterward with bounded waits.
Windows kill-on-job-close does not guarantee a nonzero exit code; do not use
`ExitStatus::success()` as evidence that a process survived cleanup. Retain
descendant handles across cleanup so PID reuse cannot produce false positives.

Before committing review work, finish actionable corrections, run the complete
suite and `prek run -a`, and record failures and platform limits. Use the review
skill when explicitly invoked. A configured matrix is not an observed hosted CI
run. Never claim Linux/Windows validation from a macOS-only local run.

Run CI on pushes to `master` and pull requests targeting `master`. The release
workflow runs only on `v*` tag pushes and calls the reusable CI checks before
packaging. Do not enable manual dispatch for either workflow.

Tag releases must validate all manifest versions, pass the complete three-OS
matrix and build every installer before publishing. Use release version tags for
third-party GitHub Actions and pre-commit hooks; commit-hash pinning is not
used. Keep CI read-only; grant contents:write only to the final publisher. Do
not publish partial matrix releases. Do not create/push tags or a repository
merely because release automation was requested.

Do not write tests that parse or duplicate GitHub Actions configuration,
pre-commit configuration, Makefile recipes or manifest constants. Use actionlint
and yamllint for workflow validation. Test executable behavior with meaningful
inputs and outcomes. Do not add CI or release-tooling test suites.

Remove unused vulnerable dependencies instead of suppressing their advisories.
Transitive maintenance exceptions must identify the advisory, upstream
dependency and removal condition; keep unresolved audit warnings visible.

Keep runtime-linking policy in `WindowsAttributes::static_vc_runtime(true)`; the
build script clears the CLI-injected environment override before applying this
policy. Keep build configuration fields supported by the project's Tauri CLI.
Keep API credentials scoped to provider ID, type and normalized endpoint so
imported configuration cannot redirect a previously stored secret. Raw USB
traces are opt-in and may contain private artwork. Sanitize fixtures before
commits; exclude vendor installers and generated build/schema outputs.

## Shell workflow data boundary

Never interpolate previous AI, clipboard or file output into shell source. Shell
actions reject `{{input}}` and receive the prior result only through the
`OPENSTREAMDECK_INPUT` child environment variable. On Unix, quoted parameter
expansion such as `printf '%s' "$OPENSTREAMDECK_INPUT"` reads literal data. On
Windows, the child program must read its environment directly; expanding
`%OPENSTREAMDECK_INPUT%` into cmd command text is not a safe data boundary. Do
not execute model-generated commands implicitly. Test attempted metacharacter
input through the production launcher, and test template rejection before any
effect is dispatched.

# Changelog Maintenance

Before completing any product change, agents must assess whether it represents a
meaningful customer-facing feature, fix, performance improvement, or other
notable outcome. When it does, ensure the appropriate section under
`CHANGELOG.md` `Unreleased` is updated with the new feature.

The changelog is public marketing material and must:

- Include only the most meaningful customer-facing changes.
- Use high-level language with the absolute minimum technical detail.
- Group entries under conventional headings such as `Features`, ` UI/UX`,
  `Fixes`, and `Performance`.
- Replace an earlier entry when the same change evolves again before release, so
  only its final outcome remain.
- Omit routine refactors, dependency updates, CI changes, tests, documentation,
  and implementation details unless they materially change the customer
  experience.
- Only update `CHANGELOG.md` if the change is visible to the customer,
  everything else should be internal and not documented in the `CHANGELOG.md`.

When creating a version tag, move the applicable `Unreleased` entries into a new
version section dated `YYYY-MM-DD`, then leave an empty `Unreleased` section at
the top for future work.

Use `## vMAJOR.MINOR.PATCH - YYYY-MM-DD` for release headings, without square
brackets (include the prerelease suffix when applicable). Release validation
must reject missing, duplicate, undated or empty entries for the tag before
packaging starts. The tagged changelog section is the single source for both
`latest.json.notes` and the GitHub release description; never substitute
`Unreleased` or generic boilerplate. Later GitHub description edits do not
update the published feed. Render these notes as Markdown in both the update
prompt and Settings, with GitHub-style lists and tables, escaped HTML and safe
link protocols. Open HTTP(S) release-note links through a validated native
browser boundary and show launch failures. Changelog extraction must preserve
literal Markdown code and ignore release headings inside fenced examples or
comments. Keep long notes scrollable without removing the Update Now / Skip this
Update choices.
