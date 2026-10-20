# Architecture and data boundaries

OpenStreamDeck is a Tauri 2 desktop application with a React/TypeScript editor
and a Rust backend. The frontend edits configuration and calls typed IPC; USB,
processes, files, credentials and OS input remain in Rust. Browser development
mode supports editing and local preview storage, with native effects
unavailable.

## Components

| Component                                    | Responsibility                                                                    |
| -------------------------------------------- | --------------------------------------------------------------------------------- |
| `src/App.tsx`, `src/model.ts`                | Configuration UI, frontend contracts and import validation                        |
| `src/config-draft.ts`                        | Base/draft reconciliation; preserves newer edits during native updates and saves  |
| `src/api.ts`                                 | Tauri IPC and isolated browser preview                                            |
| `src-tauri/src/lib.rs`                       | IPC commands, shared state, lifecycle, tray and worker startup                    |
| `model.rs`, `store.rs`                       | Configuration contracts, validation and persistence                               |
| `protocol.rs`, `device.rs`, `diagnostics.rs` | N3 packets, serialized HID worker and explicit manual diagnostics                 |
| `artwork.rs`                                 | Canonical native preview/device JPEG rendering                                    |
| `engine.rs`, `actions.rs`, `process_tree.rs` | Action validation, orchestration, OS effects, deadlines and child-process cleanup |
| `ai.rs`                                      | Provider requests, bounded streaming and endpoint-scoped credentials              |
| `agents.rs`, `codex_adapter.rs`, `focus.rs`  | Agent discovery, hook reception, capabilities and foreground activation           |
| `navigation.rs`                              | Native tab state and application window restoration                               |

Rust filenames above are relative to `src-tauri/src/`. The
[protocol guide](PROTOCOL.md) describes transport details; the
[verification matrix](TEST-MATRIX.md) links behavior to coverage and limits.

`foreground.rs` and its platform modules observe the active application.
`page_context.rs` owns transient page selection and revisioned display status.

## Configuration and persistence

Configuration version 2 has one `pages` collection and `active_page`, plus
workflows, provider configuration and device settings. Unsupported schemas are
rejected without conversion. Unknown action kinds remain in imported data and
fail explicitly if executed.

Additive `page_switching` preferences select an optional Default fallback and
enable automatic switching. Each page may hold an application association with
platform-tagged identities; Default cannot also be application-specific. Missing
new fields default to disabled/unassociated. Older binaries with strict schemas
may reject exports containing these fields.

A navigation context is derived from existing page metadata, not a new persisted
container. All unassociated pages belong to Default. Application pages share a
context only when their complete normalized identity sets are equal, independent
of identity order, application display name or icon. Multiple pages may share
that exact set. Repeated identities inside one association and partially
overlapping sets across contexts are rejected, so a native identity never picks
between competing application contexts.

`engine::switch_page` filters the flat array to the current page's context for
Previous/Next and preserves array order with wraparound. This scope also applies
when automatic switching is disabled. An explicit page-ID action may cross
contexts. A distinct foreground activation selects the first saved page in the
matching application context; an unmatched application selects the designated
Default page, regardless of its position. There is no persisted per-application
last-page selection. Same-application activations retain a manual page override.

The editor shows application context buttons above page tabs for the selected
context. Drag reordering replaces only that context's occupied slots in the flat
array, preserving other contexts, bindings, the fallback designation and
`active_page`. Reordering stays in the draft until a successful save.
Application reassociation or moving a context's pages to Default applies to all
its pages together. The last ordinary page is protected; deleting the designated
fallback requires an ordinary replacement. The editor may remember a selected
child locally without changing the native first-page activation policy.

`active_page` remains the persisted manual selection. `page_context` maintains
the current foreground application, desired page, last successfully transmitted
page, selection reason and monotonic revision in memory. It emits `page-context`
events without writing configuration. An explicit manual selection persists
normally and overrides automatic selection until the next distinct application
or Resume automatic. The editor selects its own page independently and resolves
live names from saved configuration, preserving pending edits.

The native observer lives with the process, including while the webview is
closed. macOS uses NSWorkspace notifications, Windows foreground events use
executable identity, and local X11 uses active-window and exact resource-class
metadata. Wayland detection is explicitly unavailable. See the
[application-page guide](USAGE.md#application-specific-pages) for platform
limits. No window titles or application-usage history are collected.

The native data directory is shown in Settings. `config.json` is the current
configuration; normal saves write and sync `config.tmp`, keep the previous file
in `config.backup.json`, and replace the current file. Failed saves do not
replace live configuration. Import through Deck creates a draft for review; Save
changes applies it.

Browser preview uses `openstreamdeck-preview` in local storage. It does not
share native storage. See the [usage guide](USAGE.md#backing-up-and-restoring)
for export and import.

## Execution and security

Hardware input and UI requests share the production action engine. Each binding
has active-job and cancellation bookkeeping, with a ten-minute execution
boundary. Workflows consume one step's output as the next step's input; failures
stop downstream execution. A launched agent that reaches the execution deadline
is marked failed and its child process is stopped; other sessions and already
exited launches retain their status. Completed output is discarded after use.
There is no action/input history. Errors, including hardware-triggered failures,
are sent through `action_error` to the shared banner.

AI prompts use only configured inputs. Provider credentials stay in the OS
credential store under `OpenStreamDeck`, scoped by provider ID, kind and
normalized endpoint. Configuration exports and snapshots exclude credentials.
Credential-bearing requests reject redirects; incomplete or failed streams fail
instead of supplying partial output to subsequent actions.

Shell commands are explicitly configured by the user. Previous step output is
passed as `OPENSTREAMDECK_INPUT` in the child environment, never inserted into
shell source; shell actions reject `{{input}}`. Unix process groups and Windows
Job Objects bound child-process lifetime. Platform execution limits are recorded
in the verification matrix. Application-picker paths use structured arguments or
a private child environment variable for icon extraction.

The webview has no general-purpose shell/filesystem access and cannot issue raw
USB reports. The device worker handles bounded queued commands and reconnects
automatically. Raw USB tracing is opt-in with `OPENSTREAMDECK_USB_TRACE=1` and
may contain private artwork; diagnostic logs belong in app data.

The worker treats the desired page as a latest-state mailbox, so queue pressure
does not discard the final application change. All six page frames and the input
binding snapshot come from the same saved configuration. Rendering skips
obsolete targets; applied status follows successful transfer, and bindings are
captured before asynchronous action execution. Sleeping defers frames until
wake. Agent-overlay frames carry a mode epoch so queued frames cannot overwrite
a later explicit page selection. Overlay mode lives in `page_context`, shared by
the producer and device worker. Once overlay frames are applied, LCD input uses
the transmitted session identities and ordinary buttons/dials follow the current
underlying saved page. A pending overlay still uses the previously transmitted
page until the overlay transfer succeeds.

## Windows and agent state

With `menu_bar_only` enabled (default), closing the main window destroys its
webview while USB, actions and monitoring continue behind the tray. Reopening
constructs a new window from current native state. Left-click uses the same
foreground path (create, restore, show and focus); only right-click opens the
tray menu. Tray clicks preserve the selected tab and LCD mode. When disabled,
closing quits the process. Explicit Quit always exits. `settings.rs` owns the
tested close and startup policies and the login-registration/persistence
transaction. The native adapter calls Tauri autostart enable/disable and
verifies the OS result before saving; errors roll back the OS change and
preserve live configuration. Startup reads actual OS registration without
re-enabling an externally removed entry. The login item uses `--from-autostart`:
tray mode starts without a webview, otherwise it opens the window. Explicit
Disconnect applies to the current process. A new launch enables discovery again.

Native navigation stores a tab and monotonically increasing revision. The
frontend subscribes before reading its startup snapshot and ignores older
navigation revisions. The Agent dashboard action restores/focuses this app and
selects Agents. It neither activates another agent nor replaces key artwork.
Only Show on deck enables the session overlay.

Agent status describes reported activity, not task completion. Ready includes a
finished turn; a process exit is Exited. Session IDs, parent IDs and child
identities are retained. Herdr is preferred when it already represents a
session; Codex uses metadata-only live-registry and descendant queries over a
WebSocket transport. A local token-authenticated Claude hook receiver accepts
structured events; user hook setup is not automated. Adapter errors and absent
focus/prompt capabilities must remain visible. OS foreground behavior and
physical screen appearance require manual validation beyond fixture tests.

## Native updates

`src-tauri/src/updates.rs` owns the update state machine and the complete native
download/verify/install/restart operation. One **Install Update** command starts
a task independent of the webview lifetime. `src/UpdatePanel.tsx` renders
versioned snapshots/events; stale hydration cannot rewind progress. The
app-level progress panel blocks edits from download through automatic restart
and restores when the webview reopens. The action engine and updater share the
active-action lock before downloading, closing the workflow-start race. Failures
release the work gate; restart failures retry without reinstalling. Ordinary
close/quit is blocked during the operation while the updater's restart exit is
permitted. Development macOS app bundles support installation from localhost;
release builds use GitHub. Both verify with the configured public key. Loopback
fixtures and injected installer/restart boundaries keep automated tests from
replacing apps. The native scheduler checks on startup and six hours after the
latest completed attempt. `updates/history.rs` atomically stores the timestamp
and skipped version in app data. Reminders are published before
foregrounding/recreating the GUI so a new webview recovers the notice from its
snapshot. Skipped versions suppress both automatic foregrounding and prompts;
manual installation remains available. The scheduler shares normal check
serialization and stays independent of GUI lifetime. See
[signed updates](UPDATES.md) for operation and validation limits.
