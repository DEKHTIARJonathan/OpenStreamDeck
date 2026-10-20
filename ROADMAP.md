# OpenStreamDeck roadmap

Make the N3 a dependable control surface for everyday desktop work, repeatable
AI workflows and local coding agents. The priority is to make those controls
reliable and easy to configure across macOS, Windows and Linux.

This is the product roadmap. The [README](README.md) introduces the app, the
[user guide](docs/USAGE.md) explains its behavior, and the
[verification matrix](docs/TEST-MATRIX.md) records test coverage and dated
observations. An implementation backed by fixtures still needs separate OS,
provider and physical-device validation. Priorities below carry no release-date
commitment.

## Implemented

These capabilities are present in the current source. Their integration limits
remain part of the feature description; this list does not certify every
platform or firmware variant.

| Area                    | Current implementation                                                                                                                      | Evidence and limits                                                                                                                |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Deck editor             | Six LCD keys, one configurable button, independent dial bindings, whole-dial presets and fixed Previous/Next controls                       | [Trigger coverage](docs/TEST-MATRIX.md#trigger-coverage); some LCD and dial-press observations remain missing                      |
| Pages and configuration | Flat page collection, wraparound navigation, add/rename/delete, draft/save flow, current-format JSON import/export and previous-save backup | [Persistence](docs/TEST-MATRIX.md#configuration-validation-and-persistence); unsupported schemas are rejected                      |
| Native artwork          | Shared renderer for preview and device JPEGs, Lucide glyphs, fitted labels, uploaded artwork and application icons                          | [Pixel coverage](docs/TEST-MATRIX.md#protocol-pixels-and-adapters); actual readability and orientation need physical checks        |
| Actions and workflows   | Desktop actions, configured commands, clipboard/files, ordered steps, conditions, cycles, delays, cancellation and visible errors           | [Action coverage](docs/TEST-MATRIX.md#action-coverage); native-effect coverage is not yet exhaustive                               |
| AI providers            | OpenAI, Anthropic, Gemini and compatible transports, explicit inputs, configurable models and endpoint-scoped credentials                   | Loopback provider fixtures; real accounts/models remain unverified                                                                 |
| Local agents            | Herdr integration, paginated Codex registry/descendant discovery, Claude hook receiver, dashboard navigation and explicit LCD overlay       | [Agent guide](docs/USAGE.md#agents); focus and prompt capabilities vary, Claude setup is manual                                    |
| Device lifecycle        | Automatic discovery/reconnect, brightness, latched sleep/wake, consumed wake input and optional inactivity sleep                            | [Sleep regressions](docs/TEST-MATRIX.md#sleep-latch-correction); physical sleep/wake and reconnect soak checks remain              |
| Desktop lifecycle       | Tray mode, close/quit policies, launch-at-login registration and window restoration                                                         | [Settings coverage](docs/TEST-MATRIX.md#settings-and-tray-behavior); real login/logout and complete native lifecycle checks remain |
| Build and distribution  | Bun/Cargo tooling, offline application tests, three-OS CI configuration and gated installer publication                                     | [CI and releases](docs/CI.md); configured workflows are separate from observed hosted runs and signing is incomplete               |

## Next

### Finish physical N3 validation

Observe all six LCD keys and all three dial presses, retaining the remaining
gaps alongside existing button and rotation observations. Check visible artwork,
orientation, sleep/wake, rapid inputs and reconnect behavior on the device.
Record firmware/build identity, platform and date; keep fixture replay distinct
from physical evidence. Continue using reversible tests only.

### Verify desktop behavior on each OS

Exercise application launch and icon selection, shortcuts, text entry, media and
mouse actions on macOS, Windows and Linux. Verify tray reopen after webview
destruction, close versus quit, actual foreground focus and both login-setting
directions through an OS login cycle. Measure background memory with the build,
active adapters and webview helpers accounted for.

### Close execution and reconnect coverage gaps

Extend fault injection through the production device worker for reconnects,
short writes and queued actions. Complete success, invalid-configuration,
failure, cancellation and applicable timeout coverage for every supported action
and trigger. Keep the action catalog, dispatch and verification matrix
synchronized as those gaps close.

### Make agent integration dependable in daily use

Validate that foreground focus selects the intended session, including Codex
children and Herdr's host terminal window. Keep unavailable Windows/Linux
operations explicit. Improve Claude hook onboarding while preserving existing
hooks and permission prompts. Direct Codex prompt routing remains unavailable
until implemented against a supported interface and verified with authorization.

### Validate provider and credential integration

Check real provider accounts, configured models, streaming failure behavior and
OS credential stores with explicit authorization and nonprivate inputs. Keep
automated coverage on loopback fixtures. Verify that configuration import and
endpoint changes cannot redirect a stored credential.

### Establish release evidence and distribution trust

The signed updater feed and build-time key configuration are prepared. Add the
explicit in-app check, install and restart flow described in
[the update design](docs/UPDATES.md#planned-in-app-behavior), then validate
upgrades between packaged versions. See
[updater evidence](docs/TEST-MATRIX.md#updater-preparation).

Record successful hosted runs for all three operating systems and test every
installer before claiming release readiness. Configure macOS Developer ID
signing/notarization and Windows signing when distribution credentials are
available. Resolve upstream dependency advisories as compatible updates become
available; keep remaining warnings documented in [CI.md](docs/CI.md).

## Product boundaries

OpenStreamDeck focuses on the N3, configured local actions, AI workflows and
agent visibility. Other device families, weather, vendor accounts or stores,
cloud services, general-purpose chat/document editing and retained action
history are outside this scope. Pages remain one flat collection without a
workspace/profile layer.

Plugin execution and animation playback are not implemented features. A catalog
entry or preserved imported action does not make either available. Unknown
extension actions survive import and fail explicitly if executed.

## Keeping this roadmap useful

Update this file when product scope or implementation status changes. Move a
capability into the implemented section only when production behavior and
meaningful tests exist, and link its remaining validation limits. Keep detailed
engineering rules in [AGENTS.md](AGENTS.md) and measured results in the
[verification matrix](docs/TEST-MATRIX.md).
