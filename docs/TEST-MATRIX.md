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

## Application-specific pages

The native page coordinator selects the first saved page in an application
context, or the designated Default fallback, without rewriting `active_page` on
every focus event. Contexts are derived from the flat page collection; all
unassociated pages belong to Default. The editor page is independent. Automated
fixtures establish selection and boundary behavior; they do not certify
foreground focus, N3 display completion, or a switching-latency target.

| Surface                             | Automated evidence                                                                                                                                                                    | Native validation and remaining limits                                                                                                                                                                                                                                                                            |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Default, app and manual selection   | `page_context::tests`; frontend model/page-editor tests and browser page tests                                                                                                        | Actual Chrome → VS Code → unmatched-app sequence on the N3 remains unobserved for this change.                                                                                                                                                                                                                    |
| Native lifecycle and query recovery | `foreground::tests::dropping_observer_stops_and_joins_owned_worker`, `observation_retries_recover_without_polling_healthy_applications`                                               | Guard cleanup and fake-clock retry policy; real notification delivery, tray-only operation and OS logout cycles remain separate acceptance checks.                                                                                                                                                                |
| macOS bundle identity               | `foreground::macos::tests::bundle_identity_comes_from_native_metadata_not_its_display_name` uses a temporary bundle                                                                   | NSWorkspace notifications and initial foreground query implemented; Calculator/Finder activations observed on macOS 26.7.1 as described below. Other OS releases and full device switching remain unverified.                                                                                                     |
| Windows executable identity         | `foreground::tests::windows_identity_is_exact_normalized_and_rejects_shared_hosts`; Windows-only `foreground::windows::tests::shortcut_identity_resolves_target_without_executing_it` | Native WinEvent hook, process lookup and COM shortcut resolver implemented; live Windows activation, protected processes, shortcut variants and cleanup need Windows validation. Shared/packaged hosts without an exact executable identity are explicitly unsupported.                                           |
| Linux X11 application identity      | `foreground::tests::linux_picker_and_observer_share_exact_resource_class` covers missing, malformed, ambiguous and case-sensitive metadata                                            | Only local EWMH X11 with matching desktop `StartupWMClass` is supported. No title/launcher inference. Actual window-manager behavior remains unverified.                                                                                                                                                          |
| X11 transport shutdown              | Linux-only `foreground::x11::tests::cancelled_x11_transport_stops_before_waiting_for_a_reply`, `x11_transport_reads_real_socket_data_without_a_display_server`                        | Offline Unix socket fixtures; connection establishment and protocol waits have deadlines and check cancellation. Connection fixtures also cover missing sockets and early cancellation; Linux-only cases cover saturated backlogs and abstract-address precedence. No live X11 server or remote display was used. |
| Wayland and browser preview         | Explicit unavailable capability; manual page controls remain exposed                                                                                                                  | No compositor-specific Wayland foreground integration. XWayland visibility is not treated as whole-session support. Browser fixtures are not OS detection evidence.                                                                                                                                               |
| Association picker atomicity        | `applications::association_tests` tests complete metadata, optional icon failure, missing target and invalid identity before icon lookup                                              | Association selection is separate from Open App binding changes. Missing icons do not prevent association; native app/icon coverage still varies by OS.                                                                                                                                                           |

On 2026-10-20, an isolated harness on macOS arm64 compiled the actual X11
adapter and ran four Unix socket tests successfully, including connection
establishment and pre-connect cancellation/deadlines. The Linux-only
saturated-backlog and abstract-address fixtures were not executed on this macOS
host. A separate target check compiled the actual Windows adapter and its COM
shortcut fixture for `x86_64-pc-windows-gnu`. These checks used production
source files with minimal boundary types outside the checkout; they were not
full Linux/Windows application builds or native desktop runs. The Windows
fixture was compiled, not executed.

A native observer-only probe on 2026-10-20 used OpenStreamDeck 0.1.0 debug
library code from base revision `80068cf` plus these uncommitted changes, on
macOS 26.7.1 arm64. It observed Calculator and Finder activations and confirmed
restoration of the original foreground application without recording that
application's identity. The final run measured 94 ms and 97 ms respectively from
invoking `open` to receiving the observer callback, and 3 ms to stop the
observer. These single samples include application activation overhead; they are
not OS-event-to-HID or physical-display latency. The probe did not access USB or
run a second OpenStreamDeck instance.

The first standalone probe blocked its main thread while waiting and received
only the initial query. Pumping the main CoreFoundation run loop, as the Tauri
application does, allowed native notifications to arrive. The worker run loop
alone is not sufficient in a standalone observer harness. No production observer
change was needed for this harness correction. Closed-webview/tray operation and
the full Chrome → VS Code → Default N3 sequence still require native acceptance.

Automatic changes should be measured separately at detection, render and final
HID submission. No 250 ms latency guarantee, physical pixel confirmation, or
Windows/Linux run is implied by macOS local checks. Test reports below retain
their original dates; configured CI jobs are not observed hosted runs.

Application-page frame/input regressions in `device::context_frame_tests`
exercise all six reconstructed LCD payloads, every control's captured page
binding, full command queues, deferred sleep, discarded reconnect snapshots,
overlay epochs and the transmitted agent-session identity. Applied overlays
retain LCD session identities while all ordinary controls use the current
underlying saved page, including edited bindings. A requested overlay retains
displayed-page input until transmission succeeds. The shared frame transport
fails before commit on injected write errors. Diagnostics restore the runtime
target rather than the persisted manual page. These boundaries are tested
without HID hardware; they do not establish a complete device-worker soak or
physical input timing.

`page_context::tests` also covers atomic manual persistence failure, automatic
transitions without disk/backup writes, recovery from unavailable detection and
requested versus confirmed overlay status.
`overlay_requests_and_manual_selection_share_one_authoritative_mode` covers both
serialized orders of manual selection and overlay requests using one mode field.
Browser fixtures distinguish pending overlay transfer from the display last
confirmed by the native worker.

Application creation uses one dialog for the application, first-page name,
Default fallback and automatic-switching preference. `src/page-editor.test.ts`
covers atomic creation, preserving an existing Default and bindings, creating a
fallback without removing app associations, and rejecting stale or invalid
choices without mutation. `tests/application-pages.spec.ts` exercises the
first-time flow, cancellation, duplicate contexts, missing identities, failed
saves and retry through synthetic native boundaries. Delayed identity lookup
preserves incoming native settings, active-page changes and added pages; a
removed target remains deleted and leaves an error in the open picker. An
imported application page with ID `new` can be reassociated without opening the
creation flow.

Application context buttons sit above a separate row of page tabs. The same
browser suite covers adding subpages to the selected context, local editor
selection independent of the device, same-named pages in different contexts,
long names, keyboard selection, overflow, resizing and minimum-window layouts.
Drag and keyboard reordering preserve other contexts and failed-save drafts, and
save/reload keeps the order. Real mouse drags cover the blank leading space,
inter-tab gaps, trailing space and the actual first position when the row is
overflow-scrolled. Management-control targets and foreign-context drops do not
reorder pages. Context reassociation and moving all its pages to Default are
covered together. Deletion remains visibly red beside page tabs, protects the
last Default page and removes a context when its final application page is
deleted. Application options disclosure, keyboard dismissal and focus
restoration remain covered. The page row exposes Add page, Rename page and
Delete in order, with no page menu or Move left/Move right commands. Browser
checks cover control order and visibility at desktop and minimum window widths,
plus keyboard-opened rename. These fixtures establish editor behavior, not
physical switching.

The native window sets `dragDropEnabled: false` to let HTML page tabs receive
drag/drop events. Tauri's default native file-drop interception consumes these
events before the frontend receives them; the installed Wry macOS implementation
routes `performDragOperation` through that handler as well.
`navigation::tests::main_window_preserves_html_page_drag_and_drop` checks the
production window configuration used for initial creation and tray recreation.
This regression failed with the previous configuration. Browser drag tests alone
do not cover Tauri's native event interception.

`model::validation_tests::application_context_identity_is_an_exact_normalized_set_independent_of_metadata`
covers complete normalized identity sets, repeated sets, duplicate identities
and partial-overlap rejection. Native engine and page-context regressions cover
scoped navigation with automatic switching enabled or disabled, explicit
cross-context page IDs, first-child selection on distinct activation, manual
child overrides during same-app events, reordered first pages, designated
fallback independent of order, and save/reload. The relevant tests include
`engine::tests::reserved_navigation_cycles_only_its_context_in_saved_order`,
`page_context::tests::app_activation_enters_first_saved_child_and_reorder_preserves_manual_override`
and
`page_context::tests::saved_reordering_controls_scoped_navigation_after_reload`.
On 2026-10-20, the reviewed application-pages worktree passed the complete Rust
workspace suite with all targets and features (114 library tests and 2
executable tests), all 80 frontend unit tests and all 58 Chromium browser tests
on macOS arm64. Eight focused WebKit tests also passed, covering drag
boundaries, overflow, cancellation, save/reload, asynchronous picker
reconciliation and imported page identity. The production frontend build,
standalone Rust formatting and Clippy checks passed. Desktop (1380×900) and
minimum-window (1060×720) browser screenshots were inspected. Prek was
intentionally excluded from this review. These checks add no physical N3 or live
Windows/Linux evidence.

After integration with the signed updater on 2026-10-20, the combined macOS
build passed 140 library tests, 4 executable tests, 84 frontend unit tests and
all 71 Chromium browser tests. The frontend production build and standalone
Clippy also passed. This includes startup configuration preservation, updater
workflows and page selection blocked during installation. Native tests used
synthetic signing values and the resolved temporary target path because Tauri
rejects symlinked executable paths.

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

Previous/Next (`button1`/`button2`) bypass page assignments and cycle only pages
in the current runtime page's context, including empty pages, in saved array
order with wraparound. This remains true with automatic switching off. Explicit
page-ID actions may cross contexts. Reserved bindings remain stored but cannot
override navigation. The GUI exposes a read-only inspector and explains the
one-page-context case. All pages remain in a single collection.

`engine::tests::fixed_navigation_cycles_empty_pages_and_ignores_reserved_assignments`,
`engine::tests::reserved_navigation_cycles_only_its_context_in_saved_order` and
`tests/pages.spec.ts` cover empty/new pages, scope, reordered navigation,
read-only controls and reload. Physical navigation remains a separate manual
validation step.

## Configuration validation and persistence

The current configuration schema has one page collection and an active page.
Frontend import and native persistence tests cover round trips, malformed and
unsupported formats, unknown extension actions, normal save backups and failures
that preserve live and saved state. Unsupported schemas are rejected rather than
converted. Tests use synthetic configuration; no private saved configuration is
used.

`settings::tests::startup_preserves_application_page_settings_through_save_and_reload`
uses the production startup loader with
`tests/fixtures/config-application-pages.json`. It reproduced the
`unknown field page_switching` startup failure before the compatibility fix. It
now verifies loading without rewriting the file, complete JSON preservation, an
ordinary IPC/save/reload cycle and an unchanged original backup.
`updates::tests::native_page_selection_respects_installation_gate_and_recovers`
also verifies manual page selection cannot persist configuration during an
update and resumes normally when the installation gate is released.
Application-page metadata is preserved as typed configuration and automatic
switching is supported in this build. Startup no longer emits the
compatibility-only unsupported-switching warning; OS login-registration errors
remain visible. Frontend model/draft tests and `tests/pages.spec.ts` cover
import, editing, save acknowledgement, reload and export without losing this
metadata. These tests use temporary files and injected OS registration; they do
not launch the native GUI.

`tests/pages.spec.ts` also checks JSON download, import, save/reload and
re-export through the browser UI, preserving the selected page and device
settings.

## Page deletion

The red Delete page control beside the page tabs opens a named confirmation
dialog. Deletion remains a draft until Save changes and selects a neighbor in
the same context. The final Default page is protected; deleting the designated
fallback requires an ordinary replacement. Deleting the final application page
removes its context and selects an ordinary page. Existing links are reported
and preserved for explicit retargeting. `src/page-editor.test.ts` covers
selection, preservation, invalid/stale targets and hardware/draft concurrency;
`tests/pages.spec.ts` covers cancel, Escape, confirmation and save/reload.
`tests/application-pages.spec.ts` also covers deleting the final application
page and protecting the final Default page. Application deletion has a separate
visible button, disabled for Default. Unit and browser tests cover cascading all
pages in one exact context, named confirmation and page counts, Cancel/Escape,
stale contexts, surviving links, other-context preservation, active selection,
and failed-save drafts through concurrent native updates.

## Action results and errors

Completed results are discarded after workflow consumption. Execution and
cancellation bookkeeping is transient. Failures use the `action_error` event and
the shared banner. Browser regressions cover staying on the current tab and
visible errors. Event mocks retain an independent cleanup identity for each
subscription, including repeated callbacks during React StrictMode setup.

## Page renaming

Rename is available directly between Add page and Delete beside the page tabs.
Its dialog selects the existing name, supports Cancel/Escape and Enter, rejects
blank names, and leaves the rename pending until Save changes. Page IDs,
bindings and navigation targets stay intact. Helper and browser coverage is in
`src/page-editor.test.ts` and `tests/pages.spec.ts`.

## Updater preparation

The native updater plugin is registered with the build-time public key. All
three signing environment variables are required by the native build. The build
watches root `.env` and updater environment variables; CI updater packaging
requires signing credentials. Release staging checks completeness, signature
format and key IDs, then generates a multi-platform `latest.json` feed and
checksums before draft publication. Workflow syntax is checked by actionlint and
yamllint; no tests duplicate workflow or manifest configuration.

The in-app updater is implemented in `src-tauri/src/updates.rs`, with typed UI
state in `src/UpdatePanel.tsx`. Native loopback tests exercise the actual Tauri
updater: available/current releases, malformed/missing-platform feeds, HTTP and
connection errors, corrupt/interrupted downloads, wrong keys, progress,
deadlines, retries and concurrent requests. Injected installer and restart
boundaries cover the one-click sequence, verified-byte handoff,
dirty/busy/unsupported guards before downloading, installation failure and
automatic restart. Native gate regressions cover overlapping runs whose latest
binding already finished, and saves rejected before configuration/login effects.
Restart retries do not reinstall. Exit-policy tests ensure the updater can
restart while ordinary close/quit remains blocked. Tests never install or
restart an actual application.

`tests/updates.spec.ts` covers the single Install Update action, automatic
restart, non-dismissible progress, removal of confirmation/cancel/restart
controls, literal HTML in release notes, restored webview progress, stale
snapshots, current/feed/signature/installer/restart errors, unsaved drafts,
active workflows and unsupported installation formats. The app-wide progress
dialog prevents editing from download through restart; native state blocks
close/quit and action starts throughout that sequence.

Automatic-check tests use virtual time for startup, six-hour intervals, manual
reset, sleep/clock changes and deferral during active updates. Temporary
app-data fixtures cover last-check persistence, skipped versions across
restarts, renewed prompts for different versions, stale skip requests and failed
writes preserving saved state. Reminder tests exercise publication before the
injected foreground boundary and focus failures; `navigation.rs` tests window
creation/restoration and foreground sequencing. Browser tests cover
startup-restored notices, Update Now/Skip this Update, unsaved drafts, skip
failure/retry and last-check display. Mandatory-choice notice coverage checks
Escape/backdrop behavior, focus containment and scrollable long notes with both
choices visible at 1060 × 720. Real OS activation remains a separate native
validation requirement.

Release publication now requires a dated, nonempty changelog entry matching the
tag and uses its Markdown for both the feed and GitHub description. The shared
Markdown renderer in Settings and the update prompt retains HTML as text and
uses the renderer's default URL sanitization. Browser regressions cover
headings, lists, emphasis, code, tables, blocked script links and native HTTP(S)
link dispatch, including visible browser-launch errors. Native injected-boundary
tests reject non-web URLs before invoking the OS. Scratch fixtures exercised the
production release script: fenced/inline code and comment handling, exact
unbracketed headings, missing/duplicate/empty entries, calendar-date validation,
and matching generated feed notes/checksum. This is local script validation, not
an observed hosted publication.

Development builds read a separately served localhost feed. Release builds use
the configured GitHub feed and enforce HTTPS for feed, payload and redirects.
Both verify with the configured signing public key. Synthetic signed fixtures
and loopback servers are compiled only into tests. Development builds permit
one-click installation and automatic restart; the macOS development `.app`
follows the same verified-byte installer path as a packaged app. Policy tests
cover bundled and unbundled macOS executables and the Linux AppImage
requirement. Linux DEB installations cannot invoke the installer. Real
macOS/Windows/AppImage replacement, configuration retention, restart and
hardware reconnection remain unobserved. Hosted signing/publishing has not been
validated by these runtime tests. See [signed updates](UPDATES.md).

Local macOS review validation on 2026-10-20 passed 64 frontend unit tests, 33
browser tests and 114 Rust tests with both default and all features enabled. The
production frontend build passed. Browser screenshots of the Markdown notice and
progress dialog were inspected at 1060 × 720. No real app replacement was
performed. The security scan detected RUSTSEC-2026-0382 in `rustls-webpki`
0.103.15; the lockfile now uses patched 0.103.16. The complete test suite and
`prek run -a` passed again with the corrected lockfile. Existing transitive
maintenance/GLib warnings remain described in [CI](CI.md).

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
creation, metadata refresh, refreshing hard links after Cargo replaces the
executable, migration of an existing symlink and filesystem errors. A bounded
subprocess test launches the test binary from the generated app bundle and
checks Tauri's actual starting-executable validation without launching the UI,
USB worker or agents. This covers the macOS updater failure previously missed by
mock-app tests. A separate native startup check confirmed that macOS reports
`OpenStreamDeck`, the executable runs inside `OpenStreamDeck.app`, and re-exec
preserves the PID. This check does not establish complete tray lifecycle
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

## First-launch onboarding

`onboarding::tests` exercises the production installation-state loader and
completion path with temporary app-data directories and injected login and
permission boundaries. It covers new installs, interrupted setup after a config
save, existing config/backup migration, completion and skip persistence across
configuration removal, invalid state, initial/completion write failures,
verified login saves, rollback on config failure and repeated completion. A
native retry regression retains the first-install classification if an initial
marker write fails and a background action saves configuration before retry. A
native application-state test checks that installation blocks both login changes
and skip persistence and allows completion after recovery. Platform policy tests
cover macOS granted/required access, Windows, X11 and Wayland. Request-routing
tests reject macOS settings requests on other platforms, expose opener failures
and never infer trust from successful opening.

`tests/onboarding.spec.ts` exercises the actual React dialog through browser IPC
fixtures: macOS request/recheck/focus-return, explicit enable, skip and reload,
interrupted setup, Windows/Linux guidance, Wayland limitations, existing login
registration, unavailable login adapter, window-mode copy, visible permission
and completion errors, retry, global skip, Escape, keyboard focus containment
and restoration, backdrop behavior and update-notice priority. Registration
rechecks on all three platforms and recovery from login-adapter errors preserve
explicit startup consent. Compact viewport checks keep actions reachable.
Browser captures are design evidence only.

Native trust queries and the OS settings opener are macOS-only. Automated tests
do not grant real permissions, register the user's app at login or send volume
keys. Real Accessibility approval/revocation, settings navigation, signed app
identity across upgrades, login/logout, Windows elevated-app restrictions, Linux
X11/Wayland behavior and physical N3 volume control remain manual OS and
hardware checks. Existing installations without any saved configuration or
backup cannot be distinguished from a fresh installation and receive setup.

Local macOS validation on 2026-10-20 passed the production frontend build, 84
frontend unit tests, all 86 browser tests (including 15 onboarding cases), and
153 Rust tests with all features enabled. Both onboarding steps were visually
inspected at 1060 × 720; compact 600 × 520 browser checks kept footer actions
visible. Native checks used synthetic signing environment values and temporary
fixtures outside the checkout. No actual OS permission, login item, installed
application or attached device was changed by these checks.
