# N3 USB protocol

OpenStreamDeck targets N3 USB vendor ID `5548`, product ID `1001`. This guide
describes the current encoder, decoder and device worker. It is not an assertion
that every firmware variant or physical control has been verified. See the
[verification matrix](TEST-MATRIX.md) for test coverage and limitations.

## Interface and driver

The vendor-control interface has usage page `FFA0`, usage `1`, 512-byte input
reports and 1024-byte output reports. The separate boot-keyboard interface has
usage page `1`, usage `6`; it is not the application control channel.
OpenStreamDeck selects the vendor interface through HIDAPI. macOS and Windows
use their OS HID drivers; Linux requires the bundled
[udev rule](../src-tauri/bundle/70-openstreamdeck.rules).

The control reports have no report ID. HIDAPI still requires a zero report-ID
prefix in its write buffer: **1025 API bytes represent 1024 report payload
bytes**. The extra byte is an API convention, not part of the USB payload.

## Output reports

All offsets below are HIDAPI buffer offsets, including the prefix. Buffers are
1025 bytes long and unused bytes are zero.

| Offset   | Meaning                |
| -------- | ---------------------- |
| 0        | Zero report-ID prefix  |
| 1–3      | ASCII `CRT`            |
| 4–5      | Zero                   |
| 6 onward | Command and parameters |

| Command   | Parameters                                            | Current use                           |
| --------- | ----------------------------------------------------- | ------------------------------------- |
| `LIG`     | Byte 11: brightness clamped to 0–100                  | Brightness updates and initialization |
| `CONNECT` | None                                                  | Heartbeat every five seconds          |
| `HAN`     | None                                                  | Display sleep                         |
| `DIS`     | None                                                  | Display wake                          |
| `BAT`     | Bytes 11–12: big-endian JPEG length; byte 13: key 1–6 | Image transfer header                 |
| `STP`     | None                                                  | Commit display updates                |
| `CLE`     | Byte 12: `FF`                                         | Initialization                        |

For example, brightness 60 is:

```text
00 43 52 54 00 00 4c 49 47 00 00 3c [zero padding]
```

An image transfer starts with `BAT`. JPEG data follows in chunks of at most 1024
bytes, each with the zero API prefix and padding to 1025 bytes. Raw image chunks
have no `CRT` header. A frame group ends with `STP`. The encoded length must be
1–65535 bytes, and only LCD keys 1–6 accept images.

Initialization sends `DIS`, `LIG`, `STP`, `CLE`, the six image transfers and a
final `STP`. The worker serializes all writes. Failed discovery is retried after
three seconds; read, heartbeat or write failures return the worker to device
discovery. A successful write establishes API acceptance, not visible artwork.

Implementation: [protocol.rs](../src-tauri/src/protocol.rs) and
[device.rs](../src-tauri/src/device.rs). Packet tests cover exact command bytes,
image boundaries, padding and rejection of invalid keys and lengths.

## Input reports

The input payload begins with `ACK 00 00 OK 00 00`; byte 9 identifies the
control and byte 10 carries state. Device reports contain 512 bytes. The decoder
also accepts a leading zero report-ID byte and requires at least the 11 bytes
needed to decode a control. Invalid headers, unknown codes and unsupported
button states are ignored.

State `01` is press; `00` is release. The decoder also accepts `02` as release.
Dial rotations are distinct events. Reports are independent: one button's
release must not clear another button's held state.

| Control                     | Hexadecimal code |
| --------------------------- | ---------------- |
| LCD 1–6                     | `01`–`06`        |
| Ordinary buttons 1, 2, 3    | `25`, `30`, `31` |
| Large dial press            | `35`             |
| Small left/right dial press | `33`, `34`       |
| Large dial CCW/CW           | `50`, `51`       |
| Small left dial CCW/CW      | `90`, `91`       |
| Small right dial CCW/CW     | `60`, `61`       |

[Input fixtures](../tests/fixtures/live-input.json) preserve sanitized device
reports for decoder replay. Generated tests additionally cover all controls,
both release values, rapid rotation, interleaving and malformed packets. Replay
is distinct from physical control validation.

The runtime consumes releases without dispatching actions. While sleeping, the
next press or dial rotation wakes the device and is consumed without executing
its binding. Buttons 1 and 2 always navigate to the previous and next page; page
assignments cannot override them.

## Artwork

[artwork.rs](../src-tauri/src/artwork.rs) is the canonical renderer for both
hardware and the native editor preview. It renders Lucide glyphs, labels and
colors, applies configured rotation and encodes JPEG once. The editor uses the
same JPEG bytes, compensating rotation for viewing, and offers a preview at the
configured pixel dimensions. Tests reconstruct USB image payloads and compare
those bytes with the preview.

The default geometry is 64×64 with 90° clockwise transfer rotation. Settings
expose geometry because the correct visible mapping still needs confirmation for
each firmware variant. Browser-only preview has no Rust renderer and cannot
establish hardware pixel fidelity.

Firmware updates, factory reset, persistent boot images and hardware keyboard
packet generation are unsupported. Use the software keyboard-shortcut action for
OS input. Physical geometry, sleep appearance and all-platform device behavior
remain separate manual checks.
