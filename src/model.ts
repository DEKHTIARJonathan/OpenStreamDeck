export interface Action {
  kind: string;
  value: string;
  title: string;
  color: string;
  image: string | null;
  children: Action[];
  options: Record<string, string>;
}
export interface Page {
  id: string;
  name: string;
  bindings: Record<string, Action>;
}
export interface Workflow {
  id: string;
  name: string;
  description: string;
  steps: Action[];
}
export interface Provider {
  id: string;
  name: string;
  kind: string;
  endpoint: string;
  model: string;
}
export interface Config {
  version: number;
  pages: Page[];
  active_page: string;
  workflows: Workflow[];
  providers: Provider[];
  brightness: number;
  image_size: number;
  rotation: number;
  sleep_minutes: number;
}
export interface Device {
  connected: boolean;
  serial: string;
  product: string;
  message: string;
  packets_in: number;
  packets_out: number;
  sleeping: boolean;
}
export interface Agent {
  id: string;
  source: string;
  agent: string;
  title: string;
  status: string;
  raw_status: string;
  session_id: string | null;
  target: string;
  updated_at: number;
  parent_session_id: string | null;
  is_subagent: boolean;
  can_focus: boolean;
  can_prompt: boolean;
}
export interface InputEvent {
  control: string;
  event: string;
  code: number;
  state: number;
  at: number;
}
export type View = "Deck" | "AI workflows" | "Agents" | "Settings";
export interface Navigation {
  view: View;
  revision: number;
}
export interface Snapshot {
  navigation: Navigation;
  config: Config;
  device: Device;
  agents: Agent[];
  adapter_errors: string[];
  data_dir: string;
}
export const action = (
  kind = "none",
  title = "Unassigned",
  value = "",
  color = "#292b38",
): Action => ({
  kind,
  title,
  value,
  color,
  image: null,
  children: [],
  options: {},
});
export const catalog = [
  ["open", "Open app or URL", "Applications"],
  ["shortcut", "Keyboard shortcut", "Applications"],
  ["text", "Type text", "Applications"],
  ["media", "Media control", "Applications"],
  ["mouse", "Mouse click", "Applications"],
  ["shell", "Run command", "Applications"],
  ["workflow", "Run workflow", "AI & agents"],
  ["ai", "AI prompt", "AI & agents"],
  ["agents", "Agent dashboard", "AI & agents"],
  ["agent_launch", "Launch agent", "AI & agents"],
  ["agent_select", "Select agent", "AI & agents"],
  ["agent_focus", "Focus agent", "AI & agents"],
  ["agent_prompt", "Prompt agent", "AI & agents"],
  ["sequence", "Multi-action", "Flow"],
  ["cycle", "Cycle actions", "Flow"],
  ["condition", "If input contains", "Flow"],
  ["delay", "Delay", "Flow"],
  ["clipboard_read", "Read clipboard", "Flow"],
  ["clipboard_write", "Write clipboard", "Flow"],
  ["file_read", "Read text file", "Flow"],
  ["file_write", "Write text file", "Flow"],
  ["page", "Switch page", "Device"],
  ["brightness", "Brightness", "Device"],
  ["sleep", "Toggle sleep / wake", "Device"],
  ["wake", "Wake display", "Device"],
  ["none", "Unassigned", "Device"],
] as const;
export const uid = () => crypto.randomUUID();
export function defaultConfig(): Config {
  const bindings: Record<string, Action> = {};
  const seeds = [
    ["lcd1", "open", "Terminal", "Terminal", "#254d41"],
    ["lcd2", "shortcut", "Copy", "meta+c", "#35446b"],
    ["lcd3", "shortcut", "Paste", "meta+v", "#35446b"],
    ["lcd4", "workflow", "Summarize", "summarize", "#59427d"],
    ["lcd5", "agents", "Agents", "", "#315c67"],
    ["lcd6", "media", "Play / pause", "play", "#674f33"],
    ["button3", "sleep", "Sleep", ""],
    ["knob1", "media", "Mute", "mute"],
    ["knob1.cw", "media", "Volume +", "volume_up"],
    ["knob1.ccw", "media", "Volume −", "volume_down"],
    ["knob2.cw", "brightness", "Brightness +", "+5"],
    ["knob2.ccw", "brightness", "Brightness −", "-5"],
    ["knob3", "agent_focus", "Focus agent", "selected"],
    ["knob3.cw", "agent_select", "Next agent", "next"],
    ["knob3.ccw", "agent_select", "Previous agent", "previous"],
  ];
  seeds.forEach(([key, kind, title, value, color]) => {
    bindings[key] = action(kind, title, value, color);
  });
  return {
    version: 2,
    pages: [{ id: "home", name: "Home", bindings }],
    active_page: "home",
    brightness: 60,
    image_size: 64,
    rotation: 90,
    sleep_minutes: 15,
    workflows: [
      {
        id: "summarize",
        name: "Summarize clipboard",
        description: "Turn copied text into a concise summary.",
        steps: [
          action("clipboard_read", "Read clipboard"),
          {
            ...action(
              "ai",
              "Summarize",
              "Summarize the following clearly and concisely:\n\n{{input}}",
            ),
            options: { provider: "openai" },
          },
          action("clipboard_write", "Copy result", "{{input}}"),
        ],
      },
    ],
    providers: [
      {
        id: "openai",
        name: "OpenAI",
        kind: "openai",
        endpoint: "https://api.openai.com/v1",
        model: "",
      },
      {
        id: "anthropic",
        name: "Anthropic",
        kind: "anthropic",
        endpoint: "https://api.anthropic.com/v1",
        model: "",
      },
      {
        id: "gemini",
        name: "Google Gemini",
        kind: "gemini",
        endpoint: "https://generativelanguage.googleapis.com/v1beta",
        model: "",
      },
      {
        id: "local",
        name: "Local endpoint",
        kind: "compatible",
        endpoint: "http://127.0.0.1:11434/v1",
        model: "",
      },
    ],
  };
}
/** Validate before putting imported JSON into React state. Unknown action kinds
 * are intentionally retained; malformed structures must never reach render. */
export function validateConfig(value: unknown): Config {
  const object = (v: unknown, label: string): Record<string, unknown> => {
    if (!v || typeof v !== "object" || Array.isArray(v))
      throw new Error(`Invalid ${label}`);
    return v as Record<string, unknown>;
  };
  const text = (v: unknown, label: string): string => {
    if (typeof v !== "string") throw new Error(`Invalid ${label}`);
    return v;
  };
  const array = (v: unknown, label: string): unknown[] => {
    if (!Array.isArray(v)) throw new Error(`Invalid ${label}`);
    return v;
  };
  const unique = (values: unknown[], label: string) => {
    const ids = values.map((v) => text(object(v, label).id, `${label} ID`));
    if (ids.some((id) => !id) || new Set(ids).size !== ids.length)
      throw new Error(`${label} IDs must be nonempty and unique`);
  };
  let count = 0;
  const checkAction = (value: unknown, depth = 0): void => {
    if (depth > 32 || ++count > 10000)
      throw new Error("Too many nested actions");
    const a = object(value, "action");
    for (const k of ["kind", "value", "title", "color"])
      text(a[k], `action ${k}`);
    if (a.image !== null && typeof a.image !== "string")
      throw new Error("Invalid action image");
    if (typeof a.image === "string" && a.image.length > 8000000)
      throw new Error("Action image exceeds 8 MB");
    for (const v of Object.values(object(a.options, "action options")))
      text(v, "action option");
    array(a.children, "action children").forEach((child) =>
      checkAction(child, depth + 1),
    );
  };
  const w = object(value, "config");
  if (Object.keys(w).some((key) => !Object.hasOwn(defaultConfig(), key)))
    throw new Error("Unsupported config field");
  if (w.version !== 2 || !Array.isArray(w.pages) || !w.pages.length)
    throw new Error("Not an OpenStreamDeck config");
  unique(w.pages, "Page");
  for (const value of w.pages) {
    const page = object(value, "page");
    text(page.name, "page name");
    Object.values(object(page.bindings, "page bindings")).forEach((a) =>
      checkAction(a),
    );
  }
  const workflows = array(w.workflows, "workflows");
  unique(workflows, "Workflow");
  for (const value of workflows) {
    const workflow = object(value, "workflow");
    text(workflow.name, "workflow name");
    text(workflow.description, "workflow description");
    array(workflow.steps, "workflow steps").forEach((a) => checkAction(a));
  }
  const providers = array(w.providers, "providers");
  unique(providers, "Provider");
  for (const value of providers) {
    const provider = object(value, "provider");
    for (const key of ["name", "kind", "endpoint", "model"])
      text(provider[key], `provider ${key}`);
  }
  const result = w as unknown as Config;
  if (!result.pages.some((p) => p.id === result.active_page))
    throw new Error("Active page is missing");
  if (
    !Number.isInteger(result.brightness) ||
    result.brightness < 0 ||
    result.brightness > 100
  )
    throw new Error("Invalid brightness");
  if (
    !Number.isInteger(result.image_size) ||
    result.image_size < 32 ||
    result.image_size > 128 ||
    ![0, 90, 180, 270].includes(result.rotation)
  )
    throw new Error("Invalid display settings");
  if (
    !Number.isInteger(result.sleep_minutes) ||
    result.sleep_minutes < 0 ||
    result.sleep_minutes > 1440
  )
    throw new Error("Invalid sleep minutes");
  return result;
}
