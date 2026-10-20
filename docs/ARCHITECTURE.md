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

## Configuration and persistence

Configuration version 2 has one `pages` collection and `active_page`, plus
workflows, provider configuration and device settings. Unsupported schemas are
rejected without conversion. Unknown action kinds remain in imported data and
fail explicitly if executed.

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

## Windows and agent state

Closing the main window destroys its webview while USB, actions and monitoring
continue behind the tray. Reopening constructs a new window from current native
state; Quit terminates the app. Explicit Disconnect applies to the current
process. A new launch enables discovery again.

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
