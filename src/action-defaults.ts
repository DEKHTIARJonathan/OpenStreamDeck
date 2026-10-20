import { action, catalog, type Action, type Page } from "./model";
export function configuredAction(kind: string): Action {
  const values: Record<string, string> = {
    brightness: "+5",
    delay: "500",
    page: "next",
    media: "play",
    mouse: "left",
    clipboard_write: "{{input}}",
    ai: "{{input}}",
    agent_select: "next",
    agent_focus: "selected",
  };
  const a = action(
    kind,
    catalog.find((c) => c[0] === kind)?.[1] || kind,
    values[kind] || "",
  );
  if (kind === "agent_launch") a.options.agent = "codex";
  if (kind === "ai") a.options.provider = "openai";
  return a;
}
export function changeActionType(previous: Action, kind: string): Action {
  return {
    ...configuredAction(kind),
    color: previous.color,
    image: previous.image,
  };
}
export function assignDial(
  page: Page,
  dial: string,
  preset: "brightness" | "volume" | "agents",
) {
  if (!/^knob[1-3]$/.test(dial)) throw new Error("Select a dial");
  const entries =
    preset === "brightness"
      ? [
          ["brightness", "Brightness 60%", "60"],
          ["brightness", "Brightness −", "-5"],
          ["brightness", "Brightness +", "+5"],
        ]
      : preset === "volume"
        ? [
            ["media", "Mute", "mute"],
            ["media", "Volume −", "volume_down"],
            ["media", "Volume +", "volume_up"],
          ]
        : [
            ["agent_focus", "Focus agent", "selected"],
            ["agent_select", "Previous agent", "previous"],
            ["agent_select", "Next agent", "next"],
          ];
  ["", ".ccw", ".cw"].forEach((suffix, i) => {
    const [kind, title, value] = entries[i];
    page.bindings[dial + suffix] = action(kind, title, value);
  });
}

export function applicationAction(
  previous: Action,
  app: { name: string; path: string; image: string },
): Action {
  if (!app.path || !app.image.startsWith("data:image/"))
    throw new Error("Application target and icon are required");
  return {
    ...previous,
    kind: "open",
    title: app.name,
    value: app.path,
    image: app.image,
    children: [],
    options: {},
  };
}
