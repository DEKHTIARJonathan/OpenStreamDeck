# Using OpenStreamDeck

Run the native app to control an N3. It connects automatically and retries when
the device is reattached. Close the vendor controller application first so only
one application owns the device. Browser preview supports editing, but cannot
run native actions, access credentials or verify the physical display.

## Pages and controls

In Deck, select an LCD key or the third ordinary button to edit its action.
Selecting a dial opens its Press, Turn left and Turn right bindings. Changing
one binding leaves the others intact. **Assign the whole dial** configures all
three together: brightness (turn ±5%, press 60%), volume (turn ±, press mute),
or agents (turn to select, press to focus). Changing action type clears
incompatible settings from the previous type.

The first two ordinary buttons are fixed Previous/Next page controls. They cycle
through all pages with wraparound, including empty pages, and cannot be
assigned. Add a page with **+ beside the page tabs**. The blue **Rename page**
button renames the selected page; the red **Delete page** button asks for
confirmation. At least one page must remain. Deleting a page reports existing
links to it; update those links yourself instead of expecting silent
retargeting.

Edits, page selection and brightness changes remain drafts until **Save
changes**. Native hardware uses the saved configuration. **Run action** requires
saved changes. Incoming hardware updates and save acknowledgements preserve
newer unsaved edits. Errors appear in the shared banner.

For Open app or URL, use **Choose an application** to select its name, launch
target and icon together. **URL or custom target** is the optional advanced
path. Choose a key color or upload artwork to customize other keys. In the
native app, the editor preview comes from the same JPEG renderer as the device.
Settings exposes image size and rotation; visible geometry still needs checking
on your device.

The Sleep action toggles sleep/wake. Releasing the button keeps the device
asleep. A new press or dial rotation wakes it without also running the assigned
action. The explicit Wake display action is idempotent. Settings controls
inactivity sleep in minutes; zero disables automatic sleep.

## Workflows and providers

In AI workflows, create a workflow, add actions, reorder steps and save. Assign
**Run workflow** to a key and choose the workflow. Steps run in order; output
becomes the next step's input, and a failure stops the sequence. The default
summarization workflow explicitly reads the clipboard, sends that text to the
chosen provider and writes the result back to the clipboard.

Configure provider endpoint and model ID in Settings, then save configuration
changes before saving an API key. OpenAI, Anthropic, Gemini and compatible
endpoints are supported. Choose a model available to your account or local
server; models are not selected automatically. Credentials are scoped to the
saved provider ID, type and endpoint. Changing that identity requires re-entry.
**Remove stored key** deletes the credential for the current saved identity.

Prompts and ordinary value templates can use `{{input}}` for the previous
result. Shell actions must instead read `OPENSTREAMDECK_INPUT` from the child
environment. For example, on Unix:

```sh
printf '%s' "$OPENSTREAMDECK_INPUT"
```

Do not expand previous output into shell command text. On Windows, have the
child program read its environment directly rather than placing
`%OPENSTREAMDECK_INPUT%` into cmd source. AI output is not automatically
executed.

Multi-action runs child actions in order; Cycle actions advances through its
children on successive executions; If input contains runs children only when the
input contains the configured value. These advanced action types currently use
the child-action JSON editor. Delay is measured in milliseconds. File read/write
actions use a configured path; writing consumes the prior step's output. There
is no retained action or output history.

## Agents

The Agents tab shows local sessions reported by Herdr, Codex and configured
Claude Code hooks. The executables must be available to the native application.
Use **Refresh adapter diagnostics** for discovery errors. Ready means between
turns, not completion of the user's task. Parent and sub-agent identities remain
separate, and unavailable operations are disabled or report a clear error.

**Agent dashboard** brings OpenStreamDeck forward and opens Agents. **Focus**
targets an external session. **Show on deck** replaces the six LCD frames with
agent status; **Return to keys** restores page artwork. Merely opening Agents or
selecting a session does not enable that overlay.

Herdr supports pane focus and prompt routing through its CLI. Foreground
activation is a separate operation: macOS resolves the host application; Linux
uses `xdotool` and requires a compatible graphical session. Windows terminal
activation is unavailable and reports that limitation after pane selection.
Codex discovery uses the app-server proxy and retains child thread IDs; desktop
focus is implemented on macOS only. Direct prompt routing to Codex sessions is
not implemented. Launch agent starts a configured Codex or Claude CLI process;
its exit status is not proof of task completion.

Claude Code's local hook receiver exists, but automatic hook installation and a
bundled sender are not provided. Custom integration must read
`agent-bridge.json` from the native data directory for the current loopback port
and token, then send one newline-terminated JSON object containing `token` and
`event` over TCP. The event requires `session_id` and `hook_event_name`;
supported events include SessionStart, UserPromptSubmit, PreToolUse,
PostToolUse, PermissionRequest, Notification, Stop, StopFailure and SessionEnd.
The token and port change when the app restarts. Never publish the bridge file
or forward private prompt/tool payloads unnecessarily. Preserve existing hooks
and agent permission prompts; this bridge reports status and does not approve
requests or enable focus/prompt routing for hook-only sessions.

## Backing up and restoring

Use **Deck → Export** to save the current draft to a configuration JSON file.
**Deck → Import** validates a selected file and loads a draft; review it and
save to apply. Keep exported configurations private: they can contain prompts,
commands, file paths and artwork, although stored API keys are excluded.

Configuration v2 stores a flat page collection. Import accepts the current
schema; unsupported formats are rejected. Normal native saves retain the
preceding configuration as `config.backup.json`. Importing an external file does
not delete or rewrite that original file.

The app ID is `dev.openstreamdeck.desktop`; Settings shows the resolved native
data directory. The OS credential service is `OpenStreamDeck`. Provider keys
remain in the credential store and must be entered separately when restoring on
another machine. The app does not expose a login-item setup control.

Browser preview uses the separate `openstreamdeck-preview` local-storage key.
Use Export and Import to transfer configurations between the browser preview and
the native app, or between browser origins.

Closing the window leaves the native backend running in the tray. Reopen it from
the tray or launch the app again; use Quit to terminate it. Disconnect pauses
device discovery for the current process. For device permissions, manual
diagnostics and packaging, see [README](../README.md) and [CI](CI.md).
