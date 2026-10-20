# Product screenshots

These selected images document OpenStreamDeck 0.1.0 at source revision
`968d679`, captured on 2026-10-19 on macOS arm64. They show the actual React
application running from Vite, captured with Playwright Chromium at a 1440 ×
1000 CSS-pixel viewport and a device scale factor of 2.

The browser used a fresh context with the application's built-in example
configuration. No sessions, outputs, connection status or native responses were
fabricated. No workflow was executed, no clipboard was read and no credentials
were supplied. The workflow prompt is the public example shipped in the source.

| Asset            | View                                                             |
| ---------------- | ---------------------------------------------------------------- |
| `deck.webp`      | Deck view with the default Home page and Terminal key selected   |
| `dials.webp`     | Deck editor region with Dial 1's press binding selected          |
| `workflows.webp` | Workflow editor region with the included Summarize step selected |
| `providers.webp` | Agents & AI provider card with blank model and credential fields |
| `settings.webp`  | General settings panel with default preferences                  |

Captures preserve the rendered UI; the PNG originals were encoded as lossless
WebP with metadata removed. Region captures exclude surrounding navigation;
there is no retouching or generated UI. Click the images in the root README to
view their original resolution.

The native debug application was also launched through `cargo tauri dev` and
observed through macOS accessibility with the OS application name OpenStreamDeck
and an attached N3 `5548:1001`. Native window capture was blocked by macOS
screen-recording permission. Consequently these images are **browser UI
previews**, not native window captures or evidence of hardware pixel fidelity,
OS effects, credential-store behavior or real provider execution.

To refresh them, run the current app, open its frontend in a fresh browser
context at the dimensions above and navigate to the listed views. Capture
settled UI states with only public example data. Keep the browser preview
indicator visible in the overview and label region captures by rendering
surface. Do not publish personal sessions, device serials, private prompts,
paths or credentials. Review images and the rendered README at desktop and
narrow widths, and update the version, revision and date with each refresh.

Raw captures, scripts and visual-review artifacts belong outside the checkout.
