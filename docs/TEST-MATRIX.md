# Verification matrix

This is a coverage inventory, not a claim that all integrations are complete.
Unit tests exercise production orchestration with controlled external effects.
OS focus, actual USB display orientation and real provider accounts require
separate integration evidence. A passing fake effect does not prove its native
implementation works.

## Local validation (2026-10-20)

On macOS arm64, the frontend production build, 60 frontend unit tests and 20
Playwright browser tests passed. All 85 Rust library tests and 2 macOS entry
point tests passed with both default and all features; no tests were ignored.
Pre-commit checks are separate; see [CI.md](CI.md) for their commands. These
results do not establish hosted Linux/Windows, real provider or physical-device
validation.

### Settings and tray behavior

| Supported surface                            | Automated coverage                                                                                                                                                                                                      | Native validation and remaining limits                                                                                                                                                                                                                                                           |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Sleep defaults to 0; saved values retained   | `settings::tests::sleep_defaults_off_and_existing_preferences_are_preserved`, `src/model.test.ts`, `tests/settings.spec.ts`                                                                                             | Native Device tab observed with sleep deactivated; no new physical sleep observation                                                                                                                                                                                                             |
| Launch at login, both directions             | `settings::tests::login_enables_disables_and_round_trips_both_preferences`, `failures_leave_live_and_persisted_preferences_unchanged`, `invalid_settings_have_no_os_or_persistence_effects`                             | Production save path with injected registration boundary; covers enable, disable, read/write/verification failures, ignored OS changes, persistence rollback and rollback errors. Real login/logout not exercised                                                                                |
| External login-item changes and startup mode | `settings::tests::startup_reads_os_registration_without_rewriting_preferences`, `login_startup_respects_tray_mode_and_manual_launch_opens_window`                                                                       | Reads real registration at app startup; automatic-login launches are covered by policy tests, not an OS login session                                                                                                                                                                            |
| Close to tray versus quit                    | `settings::tests::closing_uses_current_preference_and_explicit_quit_always_exits`; existing navigation restoration tests                                                                                                | Injected OS boundary. Native quit/close cycle was not run against the user's development process                                                                                                                                                                                                 |
| Left-click opens GUI; right-click opens menu | `navigation::tests::left_tray_click_restores_or_recreates_window_without_changing_navigation`, `tray_click_propagates_foreground_failures`                                                                              | On macOS 2026-10-19, targeted OS mouse events on the live app's tray icon foregrounded the GUI with no menu on left-click; right-click exposed Open/Quit. Windows not run. Tauri does not support disabling left-click menus or tray click events on Linux; desktop panel behavior applies there |
| Tabbed settings and provider editors         | `tests/settings.spec.ts`: keyboard tabs, minimum-window layout, cross-tab drafts, both toggles, save/reload, invalid sleep, save failure, every provider's credential routing/removal/failure and no secret persistence | Browser screenshots inspected at 1380×900 and 1060×720. Native Settings accessibility tree inspected in OpenStreamDeck 0.1.0 (debug, macOS arm64, attached N3 `5548:1001`); running OS application name confirmed as OpenStreamDeck                                                              |

## Action coverage

`engine::tests::every_supported_action_success_and_precancel` iterates the
production `ACTION_KINDS` registry.
`every_external_action_propagates_effect_failure` checks every leaf effect.
`src/action-defaults.test.ts` iterates every UI action.

| Action          | Additional regression coverage                                                                                                                                                                       | Native/integration status                                                                                   |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Unassigned      | Pass-through and cancellation                                                                                                                                                                        | No external effect                                                                                          |
| Open app/URL    | Empty configuration; Linux launch arguments; Windows shortcut discovery; cancelled picker selection                                                                                                  | Native launcher implemented; OS launch/icon checks pending on Windows/Linux                                 |
| Shortcut        | Recording backend: valid chords, invalid keys, reverse release on press/release failure                                                                                                              | Enigo; macOS Accessibility permission required                                                              |
| Type text       | Literal Unicode/text dispatch and injected backend failure                                                                                                                                           | Enigo; full OS text entry matrix pending                                                                    |
| Media           | Every media mapping and injected backend failure                                                                                                                                                     | Enigo; real volume/playback assertions pending                                                              |
| Mouse           | Every button mapping, invalid buttons and backend failure                                                                                                                                            | Native click integration pending                                                                            |
| Shell           | Exit code, stderr, precancel/missing binary, cancelled Unix process group, pipe saturation; reject input templates, literal environment metacharacters; Windows job-object parent/descendant fixture | Real `/bin/sh` tests on Unix; Windows fixture must execute on Windows CI                                    |
| Read clipboard  | Workflow pipeline input                                                                                                                                                                              | Real clipboard integration pending                                                                          |
| Write clipboard | Pipeline output equality                                                                                                                                                                             | Real clipboard integration pending                                                                          |
| Read text file  | Actual temporary files: success, missing, bounded oversize, invalid UTF-8                                                                                                                            | Portable filesystem integration tests; no hardware                                                          |
| Write text file | Actual temporary file success and directory write failure                                                                                                                                            | Portable filesystem integration tests; no hardware                                                          |
| AI prompt       | All four production transports: HTTP request contracts, framing, errors, truncation, cancellation/deadlines before headers and mid-body; endpoint-bound credential identities                        | Loopback HTTP fixtures only; real accounts/models unverified                                                |
| Workflow        | Input chaining; failure short-circuit; recursive cycle limit                                                                                                                                         | Native worker linked                                                                                        |
| Multi-action    | Sequential orchestration and cancellation                                                                                                                                                            | Native worker linked                                                                                        |
| Conditional     | Matching/nonmatching branch                                                                                                                                                                          | Production engine tested                                                                                    |
| Cycle           | First/second/wraparound                                                                                                                                                                              | Production engine tested                                                                                    |
| Delay           | Cancel during wait; range rejection                                                                                                                                                                  | Real Tokio timer test                                                                                       |
| Brightness      | Saturating +/-; absolute value; stale `selected` rejection; all-dial assignment                                                                                                                      | Live HID writes observed; every reassignment not yet physically confirmed                                   |
| Sleep/wake      | Success, cancelled dispatch, external failure; release filtering for every control, consumed wake input, queued sleep toggles                                                                        | Native HID commands; visual state confirmation pending                                                      |
| Page            | Next/previous wrap; missing ID                                                                                                                                                                       | Configuration redraw implemented                                                                            |
| Agent dashboard | Dispatch/errors; native window create/restore/focus sequencing and failures; unchanged six LCD frames/bindings; startup/tab races in `tests/dashboard.spec.ts`                                       | Foregrounds OpenStreamDeck and selects Agents; actual OS activation/physical icons need manual confirmation |
| Agent selection | Next/previous validation                                                                                                                                                                             | Repeated dial input must not be dropped                                                                     |
| Agent focus     | Herdr ancestor discovery; ambiguity/cycle rejection                                                                                                                                                  | Herdr pane focus accepted live; terminal activation implementation added                                    |
| Agent prompt    | Empty prompt and adapter failure                                                                                                                                                                     | No prompts sent to live sessions during tests                                                               |
| Agent launch    | Allowed agent selection; subprocess exit/cancel; workflow deadline stops its process and marks only its active launch failed                                                                         | Actual paid/live agent launch not run during tests                                                          |

The Windows shell containment regression
`process_tree::tests::dropping_job_terminates_parent_and_descendant` checks that
both fixture processes are alive before dropping the production job guard and
terminate within bounded waits afterward. It retains the descendant process
handle across cleanup and makes no assumption about the job-close exit code.
This Windows-only regression requires a Windows run; local macOS tests do not
execute it.

## Trigger coverage

All supported raw inputs are defined in `protocol::decode`. The attached device
has emitted actual 512-byte reports with code at byte 9 and state at byte 10.
`protocol::live_fixture_tests::recorded_n3_inputs_decode` replays the sanitized
unique observations in
[`tests/fixtures/live-input.json`](../tests/fixtures/live-input.json).

| Trigger              | Codes                      | Physical observations currently captured                                |
| -------------------- | -------------------------- | ----------------------------------------------------------------------- |
| LCD keys 1–6 down/up | `01`–`06`, state `01`/`00` | Key 1 down/up; key 2 down captured; remaining observations still needed |
| Ordinary buttons 1–3 | `25`, `30`, `31`           | Down/up for all three                                                   |
| Dial 1 press         | `35`                       | Down/up                                                                 |
| Dial 2 / 3 press     | `33` / `34`                | Not yet in saved fixture                                                |
| Dial 1 CCW / CW      | `50` / `51`                | Both directions                                                         |
| Dial 2 CCW / CW      | `90` / `91`                | Both directions                                                         |
| Dial 3 CCW / CW      | `60` / `61`                | Both directions                                                         |

The decoder also accepts release state `02`, which is not present in the
physical-input fixture; `00` is the observed release on this device. Software
fixtures must cover all codes, optional report-ID prefix, malformed
header/length, interleaving and unknown button state. Do not relabel a fixture
as a physical observation.

## Protocol, pixels and adapters

- Brightness report exact bytes and 1025-byte API buffer length.
- Image header length, 1024-byte chunks, padding, key/length limits.
- Shared artwork: preview data URL bytes equal reassembled USB JPEG payload,
  deterministic output and 64×64 decoded dimensions.
- Labels XML-escaped; user artwork decoded and resized by native code.
- Codex: real WebSocket upgrade, initialize/initialized, paginated loaded
  registry, metadata-only thread/read and paginated descendants; explicit
  child-source filter. Active, approval-waiting, ready, unloaded and unknown
  state handling. Parent IDs/nicknames, thread-spawn lineage, duplicate IDs,
  identical titles, rejected RPC, closed connection, malformed responses and
  repeated cursors tested.
- Herdr: protocol 22 snapshot parsing and opaque pane target preservation.
- Agent status: done/completed/idle report Ready (turn activity only);
  subprocess exit reports Exited. Working → Ready → Working regression coverage.
- Focus: child technical ID retained in deep link; invalid IDs rejected, absent
  capabilities fail explicitly, dial selection survives discovery/reordering.
  Browser IPC fixture verifies child ID dispatch, parent display and visible
  focus failure. Actual child-window activation remains to be confirmed with a
  child exposed by the live adapter.
- Persistence: round trip, previous-file backup, failed transaction preserves
  live and saved state.
- Editor: hardware configuration events and save acknowledgements preserve
  unsaved/newer edits; malformed imports rejected before render.
- Dashboard: `navigation::tests` exercises the production window orchestration
  through an injected OS boundary, including repeated actions, closed/minimized
  window sequencing, all window-operation failures and unchanged native artwork.
  Playwright verifies the Agents tab survives action completion, webview startup
  and delayed snapshots, reports hardware action failures, and changes LCD mode
  only through the explicit Show on deck control. These tests do not prove
  actual OS foreground focus or physical display readability.
- Browser: dial edit persistence, independent direction binding, page creation,
  workflow ordering, unsupported browser execution error, no page errors.

## Local automation and dependency policy

GitHub Actions YAML is validated by actionlint and yamllint. The CI matrix runs
frontend unit tests, browser tests and backend tests after building the app.

Cargo-deny, cargo-audit and cargo-machete run on every invocation, including
documentation-only changes. The renderer update to resvg 0.48 is covered by the
native artwork tests, including reconstruction of USB JPEG payloads from the
canonical preview bytes. Physical display appearance still requires manual
validation. See [CI.md](CI.md) for unresolved upstream dependency warnings;
passing hooks do not mean those warnings have been fixed.

Keyring 4.2 and Reqwest 0.13 are exercised by the Rust provider fixtures.
`credential_account_preserves_persisted_identity_across_dependency_upgrades`
locks the stored account name and equivalent URL normalization to a synthetic
fixture. OS credential access and live HTTPS/provider behavior remain manual
integration checks; automated tests do not read personal secrets.

## Manual hardware diagnostics

`openstreamdeck --hardware-smoke` exercises wake, reversible brightness,
heartbeat, sleep/wake and restoration of brightness, all six LCD frames and the
prior sleep state. It writes `hardware-smoke.json` in app data. This explicit
manual diagnostic is never invoked by automated tests. Successful HIDAPI writes
establish API-level I/O, not physical screen appearance or switch operation.
Diagnostic failure/short-write tests use an injected writer and require no
device. The current automated run did not perform a physical hardware check.

## Key readability layout

Native generated artwork uses a four-pixel margin on its 96-pixel design grid.
Icons fit their painted bounds in an 88-pixel-wide region, with 60 pixels of
height for one-line labels or 42 for two lines. Bold labels fill the remaining
width and height; overflow is marked with an ellipsis. Uploaded/application
artwork continues to fill the whole key. Hardware and native preview use the
same renderer and JPEG bytes.

`artwork::tests` covers wrapping, overflow and fitted bounds. Actual LCD
readability remains a physical validation step.

## Sleep latch correction

The sleep action toggles in the serialized device worker. Release reports no
longer wake the display; the next press or dial rotation wakes it without also
running its action. Explicit wake and GUI sleep/wake requests remain idempotent.
`device::sleep_input_tests` covers both release variants for all press controls,
all dial directions, and toggle resolution. The offline queue regression also
includes the toggle command. Physical sleep/wake behavior remains a separate
manual validation step.

## Fixed page-navigation controls

Previous/Next (`button1`/`button2`) bypass page assignments and always cycle the
configuration's pages, including empty pages. Bindings for reserved controls are
stored but cannot override these controls. The GUI exposes a read-only inspector
for them and explains the one-page case. All pages belong to a single
collection.

`engine::tests::fixed_navigation_cycles_empty_pages_and_ignores_reserved_assignments`
and `tests/pages.spec.ts` cover empty/new pages, read-only controls and reload.
Physical navigation remains a separate manual validation step.

## Configuration validation and persistence

The current configuration schema has one page collection and an active page.
Frontend import and native persistence tests cover round trips, malformed and
unsupported formats, unknown extension actions, normal save backups and failures
that preserve live and saved state. Unsupported schemas are rejected rather than
converted. Tests use synthetic configuration; no private saved configuration is
used.

`tests/pages.spec.ts` also checks JSON download, import, save/reload and
re-export through the browser UI, preserving the selected page and device
settings.

## Page deletion

The named Delete page control beside the tabs opens a confirmation dialog.
Deletion remains a draft until Save changes, selects a neighboring page, and
cannot remove the last page. Existing links are reported and preserved for
explicit retargeting. `src/page-editor.test.ts` covers selection, preservation,
invalid/stale targets and hardware/draft concurrency; `tests/pages.spec.ts`
covers cancel, Escape, confirmation and save/reload.

## Action results and errors

Completed results are discarded after workflow consumption. Execution and
cancellation bookkeeping is transient. Failures use the `action_error` event and
the shared banner. Browser regressions cover staying on the current tab and
visible errors. Event mocks retain an independent cleanup identity for each
subscription, including repeated callbacks during React StrictMode setup.

## Page renaming

A blue Rename page button sits immediately left of Delete page. Its dialog
selects the existing name, supports Cancel/Escape and Enter, rejects blank
names, and leaves the rename pending until Save changes. Page IDs, bindings and
navigation targets stay intact. Helper and browser coverage is in
`src/page-editor.test.ts` and `tests/pages.spec.ts`.

## Updater preparation

The native updater plugin is registered with the build-time public key. All
three signing environment variables are required by the native build. The build
watches root `.env` and updater environment variables; CI updater packaging
requires signing credentials. Release staging checks completeness, signature
format and key IDs, then generates a multi-platform `latest.json` feed and
checksums before draft publication. Workflow syntax is checked by actionlint and
yamllint; no tests duplicate workflow or manifest configuration.

These are build/release foundations, not a completed in-app updater. No update
IPC, UI, download or install action is exposed. Hosted signing/publishing and
upgrades between actual packaged versions have not been observed. Signature
key-ID matching is not independent artifact signature verification. The
[update design](UPDATES.md) lists the required runtime coverage and OS checks.

Local macOS validation on 2026-10-20 used disposable keys and synthetic files
outside the checkout. The compiled production build script rejected missing
variables, blank keys and placeholders, accepted an explicitly empty password,
loaded a temporary root dotenv file, honored environment precedence, hid
malformed dotenv contents and emitted only the public key plus the expected
rebuild directives. Release preflight also rejected missing signing variables
and wrote only public configuration. Tauri's signer produced synthetic payload
signatures consumed by production staging/feed generation; missing, duplicate,
empty, malformed and wrong-key assets were rejected.

A direct `cargo tauri build --debug --ci --bundles app` with temporary public
configuration and exported disposable signing variables produced a macOS arm64
app bundle, updater archive and signature. The archive passed cryptographic
verification with `minisign-verify`; changing a payload byte caused verification
to fail. The packaged executable contained the fixture public key and neither
the fixture private key nor its password. This was a debug package, not the
universal release matrix, and no application launch or installation was tested.

## Integration limitations

macOS development naming: the binary's `macos_dev` tests exercise bundle
creation, metadata refresh, reuse of rebuilt executables and filesystem errors
without launching an app. A separate native startup check confirmed that macOS
reports `OpenStreamDeck`, the executable runs inside `OpenStreamDeck.app`, and
re-exec preserves the PID. This check does not establish complete tray lifecycle
behavior.

Native-effect tests are not yet exhaustive. Remaining coverage includes OS
shortcut/media/mouse behavior, real provider account/model verification, Windows
job-object execution evidence and terminal activation, all import variants, full
device-worker reconnect/short-write fault injection, all physical LCD/dial-press
inputs, tray reopen and long-duration soak tests. Plugin execution and animation
playback are not implemented capabilities. Native tray close/reopen tests must
verify actual webview release and process termination; browser tests do not
establish those behaviors. Windows Herdr foreground activation and non-macOS
Codex focus are explicitly unavailable.
