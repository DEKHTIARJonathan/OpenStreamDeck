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
with wraparound through the current application's pages, or through ordinary
pages in Default. They never enter another application automatically, including
when automatic switching is off. One-page contexts stay on that page. Reserved
buttons cannot be assigned, even on empty or imported pages.

Navigation has two levels. Choose **Default** or an application in the first
row; choose one of its pages in the second row. Default contains all ordinary
pages; its designated fallback is used when no application matches. Application
contexts show their native icon and name. These are views of the same flat page
collection, not separate profiles. Scroll controls reveal more contexts or pages
when a row fills up. Arrow keys and Home/End navigate without activating a page
on the device.

Use **Add application** to create a new application context, and **Add page** to
add an empty page to the selected context. Drag a page tab before or after
another page in that same context to reorder it. The insertion indicator shows
the target; **Save changes** applies the order to hardware Previous/Next.
Reordering keeps the selected page and its bindings, preserves other contexts,
and does not change the Default fallback. Alt+Shift+Left/Right while a page tab
is focused provides a keyboard alternative.

The page row exposes **Add page**, **Rename page**, and the red **Delete page**
button in that order. Rename opens the selected page's name directly, without a
menu. Delete confirms the named target, selects a remaining neighbor in the same
context, and reports links that need updating. The last page in Default is
protected. Deleting its designated fallback requires a replacement from Default.
Deleting an application's last page removes its context and returns editing to
an ordinary page.

The red **Delete application** button in the Applications row confirms the
application name and lists every page it will delete. Deleting an application
removes all its pages together, returns editing to Default, and preserves other
applications. It remains a draft until **Save changes**. Links from other pages
or workflows are reported for retargeting. The button is disabled for Default.

Selecting a page changes **Editing**, independently of **On device**. Edits and
brightness changes remain drafts until **Save changes**. Native hardware uses
saved configuration. **Show on deck**, beside the editing page name, activates
the saved page without applying pending edits; a newly created page must be
saved first. **Run action** requires saved changes. Application switches and
save acknowledgements preserve newer unsaved edits. Errors appear in the shared
banner.

### Application-specific pages

Click **Add application**. In the creation dialog, choose an application and
review its first page's suggested name. If no Default fallback exists, choose an
ordinary page in the same dialog or create one. **Switch automatically** is
checked initially; clear it to prepare the context without enabling automatic
switching. Create the first page, assign its keys and dials, then save. Use
**Add page** within that application to create further pages with the same
association. Creating or cancelling a draft does not change saved device
configuration. Selecting an application that already has pages offers its
existing context instead of creating a competing association.

For example, Chrome may have Browsing and Developer pages, while VS Code has
Editing and Debugging pages. Activating Chrome selects its first saved page;
Previous/Next cycles only Chrome's pages. Activating VS Code selects its first
saved page. Returning to Chrome starts at its first page again; it does not
restore the last hardware page from a previous activation. An unassigned
application selects the designated Default fallback. Reordering application
pages and saving changes which page opens first.

New pages start empty and replace the entire page, including dial bindings; they
do not include preset app shortcuts or inherit bindings from Default.
Application identity is separate from page names and artwork. **Change
application** changes the association of all pages in that context together.
**Move pages to Default** confirms removal of the shared association while
preserving every page's contents. Choose **Set as Default** in an ordinary
page’s editor heading to change the fallback; application pages must first be
moved to Default. Existing configurations keep automatic switching disabled
until configured.

Configuration import permits multiple pages with the same complete normalized
application identity set. Repeated identities inside an association and partial
overlaps between contexts are rejected. Display names and icons do not decide
which application a page belongs to.

Previous/Next, a page action, or **Show on deck** temporarily overrides
automatic selection until a different application becomes active. **Resume
automatic** ends the override immediately. Another window of the same
application does not end it. An explicit page-ID action can deliberately cross
contexts; Previous/Next always uses the context of the page currently selected
for the device. Application switching never launches an app, runs a binding,
changes the selected editor page, or writes a configuration backup.

**On device** distinguishes the desired page from a successfully submitted page.
Disconnected and sleeping states remain explicit. Application changes while
asleep do not wake the N3; the latest selected page is submitted when it wakes.
An explicit Agents overlay stays visible while the underlying page follows the
application. Its LCD keys retain the displayed agent targets; the ordinary
buttons and dials use the current underlying page's saved bindings. Native
monitoring continues with the editor window closed.

### Foreground detection by platform

| Platform        | Supported identity and limitations                                                                                                                                                                                                                                       |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| macOS           | Native NSWorkspace activation notifications and bundle identifiers. App names and window titles are not matching keys. Applications without a bundle identifier report unavailable identity.                                                                             |
| Windows         | Native foreground event hook and resolved executable path. Start Menu shortcuts are resolved through COM. Inaccessible processes and shared host executables report unavailable identity instead of matching unrelated hosted apps.                                      |
| Linux X11       | Local X11 display with EWMH active-window support. The application desktop entry must contain `StartupWMClass`; it matches the exact WM_CLASS resource class. Missing/ambiguous metadata and generic launcher guesses are rejected. Remote X11 displays are unsupported. |
| Linux Wayland   | Automatic detection is unavailable, including sessions exposing an XWayland display. Manual pages remain usable.                                                                                                                                                         |
| Browser preview | Configuration and simulated status only; real foreground detection is unavailable.                                                                                                                                                                                       |

Detection failures fall back to Default with a persistent explanation. Temporary
missing foreground handles are coalesced briefly; failures retry without needing
another activation event. Disabling automatic switching stops the observer.
Imported associations for another OS remain stored; rebind explicitly on the new
platform. Removed applications retain their saved association until you change
or remove it. Icons are optional for page associations; the Open App action
still requires its normal atomic target/name/icon selection.

Adapters and fixture tests do not establish physical switching latency or
display correctness. Native platform and N3 validation limits are recorded in
the [verification matrix](TEST-MATRIX.md#application-specific-pages).

For Open app or URL, use **Choose an application** to select its name, launch
target and icon together. **URL or custom target** is the optional advanced
path. Choose a key color or upload artwork to customize other keys. In the
native app, the editor preview comes from the same JPEG renderer as the device.
Settings → Device exposes image size and rotation; visible geometry still needs
checking on your device.

The Sleep action toggles sleep/wake. Releasing the button keeps the device
asleep. A new press or dial rotation wakes it without also running the assigned
action. The explicit Wake display action is idempotent. Settings controls
inactivity sleep in minutes; the default is **0**, which disables automatic
sleep. Existing saved sleep values are preserved.

## Workflows and providers

In AI workflows, create a workflow, add actions, reorder steps and save. Assign
**Run workflow** to a key and choose the workflow. Steps run in order; output
becomes the next step's input, and a failure stops the sequence. The default
summarization workflow explicitly reads the clipboard, sends that text to the
chosen provider and writes the result back to the clipboard.

Configure provider endpoint and model ID in Settings → Agents & AI, then save
configuration changes before saving an API key. OpenAI, Anthropic, Gemini and
compatible endpoints are supported. Choose a model available to your account or
local server; models are not selected automatically. Credentials are scoped to
the saved provider ID, type and endpoint. Changing that identity requires
re-entry. **Remove stored key** deletes the credential for the current saved
identity.

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
schema; unsupported formats are rejected. Application associations and automatic
page settings are additive fields in version 2. Older binaries may reject
exports containing these fields; downgrade compatibility is not guaranteed.
Normal native saves retain the preceding configuration as `config.backup.json`.
Importing an external file does not delete or rewrite that original file.

The app ID is `dev.openstreamdeck.desktop`; Settings shows the resolved native
data directory. The OS credential service is `OpenStreamDeck`. Provider keys
remain in the credential store and must be entered separately when restoring on
another machine.

Settings has three tabs: **General** for application behavior, **Device** for
sleep and display geometry, and **Agents & AI** for provider configuration and
local agent status. Select a provider to edit its endpoint, model and API key.
The **Rename page** button sits between Add page and Delete beside the page tabs
in Deck.

In **General**, enable **Launch at login** to register OpenStreamDeck with the
operating system. Disabling it removes that registration. Both changes take
effect with **Save changes**. Registration is checked after updating it; errors
appear in the banner and failed saves preserve the prior configuration. The app
reads the OS registration at startup, respecting changes made outside the app.
Login launch starts in the tray when Menu bar only is enabled and opens the
window when it is disabled. A normal manual launch opens the window.

Browser preview uses the separate `openstreamdeck-preview` local-storage key.
Use Export and Import to transfer configurations between the browser preview and
the native app, or between browser origins.

**Menu bar only** is enabled by default. With it on, closing the window releases
the webview and leaves the native backend running in the menu bar/system tray.
Left-click the tray icon to immediately reopen or foreground the window;
right-click opens its menu on macOS and Windows. Linux tray behavior depends on
the desktop panel; its menu also provides **Open OpenStreamDeck**. You can also
launch the app again. With it off, closing the window quits OpenStreamDeck,
stopping device controls, workflows and monitoring. Explicit **Quit** always
exits. Apply this preference with **Save changes**. Disconnect pauses device
discovery for the current process. For device permissions, manual diagnostics
and packaging, see [README](../README.md) and [CI](CI.md).
