import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  Grid2X2,
  Workflow as WorkflowIcon,
  Bot,
  Settings,
  Plus,
  Play,
  Save,
  ChevronRight,
  ChevronLeft,
  ArrowDown,
  ArrowUp,
  Trash2,
  Pencil,
  Copy,
  Download,
  Upload,
  Sun,
  Power,
  Search,
  Cable,
  Terminal,
  Sparkles,
  Keyboard,
  Circle,
  X,
  Check,
  Radio,
  ExternalLink,
} from "lucide-react";
import { call, native, on } from "./api";
import ApplicationPicker from "./ApplicationPicker";
import SettingsPanel from "./SettingsPanel";
import DeletePageDialog from "./DeletePageDialog";
import { removePage, renamePage } from "./page-editor";
import RenamePageDialog from "./RenamePageDialog";
import {
  receiveConfig,
  editConfig,
  isDirty,
  type ConfigDraft,
} from "./config-draft";
import {
  configuredAction,
  changeActionType,
  assignDial,
  applicationAction,
} from "./action-defaults";
import {
  action,
  catalog,
  uid,
  validateConfig,
  type Action,
  type Config,
  type Snapshot,
  type Device,
  type Agent,
  type InputEvent,
  type View,
  type Navigation,
} from "./model";
const views = [
  ["Deck", Grid2X2],
  ["AI workflows", WorkflowIcon],
  ["Agents", Bot],
  ["Settings", Settings],
] as const;
function Icon({ kind, size = 22 }: { kind: string; size?: number }) {
  const C =
    kind === "ai" || kind === "workflow"
      ? Sparkles
      : kind.startsWith("agent")
        ? Bot
        : kind === "shortcut" || kind === "text"
          ? Keyboard
          : kind === "open" || kind === "shell"
            ? Terminal
            : kind === "media"
              ? Play
              : kind === "brightness"
                ? Sun
                : Circle;
  return <C size={size} />;
}
function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
    </label>
  );
}
function ActionEditor({
  value: a,
  change,
  config: w,
}: {
  value: Action;
  change: (a: Action) => void;
  config: Config;
}) {
  const [pickApp, setPickApp] = useState(false);
  const [json, setJson] = useState(JSON.stringify(a.children, null, 2));
  const [jsonError, setJsonError] = useState("");
  useEffect(() => {
    setJson(JSON.stringify(a.children, null, 2));
    setJsonError("");
  }, [a.children]);
  const update = (patch: Partial<Action>) => change({ ...a, ...patch });
  const options = (key: string, value: string) =>
    update({ options: { ...a.options, [key]: value } });
  return (
    <div className="action-editor">
      <Field label="Label">
        <input
          value={a.title}
          onChange={(e) => update({ title: e.target.value })}
        />
      </Field>
      <Field label="Action">
        <select
          aria-label="Action"
          value={a.kind}
          onChange={(e) => change(changeActionType(a, e.target.value))}
        >
          {!catalog.some((c) => c[0] === a.kind) && <option>{a.kind}</option>}
          {catalog.map(([id, name]) => (
            <option key={id} value={id}>
              {name}
            </option>
          ))}
        </select>
      </Field>
      {a.kind === "open" ? (
        <div className="open-app-editor">
          <button className="choose-app" onClick={() => setPickApp(true)}>
            {a.image ? <img src={a.image} alt="" /> : <Plus size={23} />}
            <span>
              {a.value ? a.title : "Choose an application"}
              <small>
                {a.value ? "Change application" : "Use its icon on the key"}
              </small>
            </span>
          </button>
          <details>
            <summary>URL or custom target</summary>
            <input
              aria-label="Application URL or custom target"
              value={a.value}
              onChange={(e) => update({ value: e.target.value })}
            />
          </details>
        </div>
      ) : a.kind === "workflow" ? (
        <Field label="Workflow">
          <select
            value={a.value}
            onChange={(e) => update({ value: e.target.value })}
          >
            <option value="">Choose a workflow</option>
            {w.workflows.map((f) => (
              <option key={f.id} value={f.id}>
                {f.name}
              </option>
            ))}
          </select>
        </Field>
      ) : a.kind === "media" ? (
        <Field label="Control">
          <select
            value={a.value}
            onChange={(e) => update({ value: e.target.value })}
          >
            {[
              "play",
              "next",
              "previous",
              "volume_up",
              "volume_down",
              "mute",
            ].map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        </Field>
      ) : (
        ![
          "none",
          "sleep",
          "wake",
          "clipboard_read",
          "agents",
          "agent_focus",
          "sequence",
          "cycle",
        ].includes(a.kind) && (
          <Field
            label={
              a.kind === "ai" ||
              a.kind === "agent_launch" ||
              a.kind === "agent_prompt"
                ? "Prompt · use {{input}} for the previous result"
                : a.kind === "delay"
                  ? "Delay in milliseconds"
                  : a.kind === "shortcut"
                    ? "Shortcut (example: meta+shift+c)"
                    : a.kind === "page"
                      ? "Page ID, next, or previous"
                      : "Value"
            }
          >
            <textarea
              rows={
                a.kind === "ai" || a.kind === "shell" || a.kind === "text"
                  ? 5
                  : 2
              }
              value={a.value}
              onChange={(e) => update({ value: e.target.value })}
            />
          </Field>
        )
      )}
      {a.kind === "ai" && (
        <Field label="Provider">
          <select
            value={a.options.provider || ""}
            onChange={(e) => options("provider", e.target.value)}
          >
            <option value="">Choose a provider</option>
            {w.providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
                {!p.model && " · configure model"}
              </option>
            ))}
          </select>
        </Field>
      )}
      {a.kind === "agent_launch" && (
        <Field label="Agent">
          <select
            value={a.options.agent || "codex"}
            onChange={(e) => options("agent", e.target.value)}
          >
            <option value="codex">Codex</option>
            <option value="claude">Claude Code</option>
          </select>
        </Field>
      )}
      {a.kind === "shell" && (
        <p className="muted">
          The previous result is passed as the OPENSTREAMDECK_INPUT environment
          variable. Shell scripts cannot use {"{{input}}"}. On Unix, read it
          with <code>{`printf '%s' "$OPENSTREAMDECK_INPUT"`}</code>. On Windows,
          read the environment in the child program; expanding it into cmd
          command text can execute its contents.
        </p>
      )}
      {["shell", "agent_launch"].includes(a.kind) && (
        <Field label="Working directory">
          <input
            value={a.options.cwd || ""}
            placeholder="Absolute path"
            onChange={(e) => options("cwd", e.target.value)}
          />
        </Field>
      )}
      {["sequence", "cycle", "condition"].includes(a.kind) && (
        <Field label="Child actions · JSON">
          <textarea
            rows={12}
            className="code"
            value={json}
            onChange={(e) => setJson(e.target.value)}
            onBlur={() => {
              try {
                const children = JSON.parse(json);
                if (
                  !Array.isArray(children) ||
                  children.some((a) => typeof a.kind !== "string")
                )
                  throw new Error("Expected an array of actions");
                update({ children });
                setJsonError("");
              } catch (e) {
                setJsonError(String(e));
              }
            }}
          />
          {jsonError && <small className="error-text">{jsonError}</small>}
          <button
            onClick={() =>
              update({
                children: [...a.children, action("delay", "Wait", "500")],
              })
            }
          >
            <Plus size={14} /> Add child
          </button>
        </Field>
      )}
      <div className="row">
        <Field label="Key color">
          <input
            type="color"
            value={a.color}
            onChange={(e) => update({ color: e.target.value })}
          />
        </Field>
        <Field label="Artwork">
          <label className="button">
            <Upload size={14} /> Choose image
            <input
              type="file"
              hidden
              accept="image/png,image/jpeg,image/gif"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (!f) return;
                if (f.size > 2_000_000) {
                  setJsonError("Artwork must be under 2 MB");
                  return;
                }
                const r = new FileReader();
                r.onload = () => update({ image: String(r.result) });
                r.readAsDataURL(f);
              }}
            />
          </label>
          {a.image && (
            <button onClick={() => update({ image: null })}>Remove</button>
          )}
        </Field>
      </div>
      {pickApp && (
        <ApplicationPicker
          close={() => setPickApp(false)}
          choose={(app) => change(applicationAction(a, app))}
        />
      )}
    </div>
  );
}
export default function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null),
    [editor, setEditor] = useState<ConfigDraft | null>(null),
    [view, setView] = useState<View>("Deck"),
    [selected, setSelected] = useState("lcd1"),
    [notice, setNotice] = useState(""),
    [error, setError] = useState(""),
    [query, setQuery] = useState(""),
    [lastInput, setLastInput] = useState<InputEvent | null>(null),
    [workflowId, setWorkflowId] = useState("summarize"),
    [stepIndex, setStepIndex] = useState(0),
    [prompt, setPrompt] = useState("");
  const [renameTarget, setRenameTarget] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [deletePageId, setDeletePageId] = useState<string | null>(null);
  const navigationRevision = useRef(-1);
  const applyNavigation = (navigation?: Navigation) => {
    if (navigation && navigation.revision > navigationRevision.current) {
      navigationRevision.current = navigation.revision;
      setView(navigation.view);
    }
  };
  const w = editor?.draft ?? null;
  const dirty = editor ? isDirty(editor) : false;
  const [saving, setSaving] = useState(false);
  const [frames, setFrames] = useState<string[]>([]);
  const frameActions = w?.pages.find((p) => p.id === w.active_page)?.bindings;
  useEffect(() => {
    if (!native || !w || !frameActions) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      call<string[]>("render_keys", {
        actions: Array.from(
          { length: 6 },
          (_, i) => frameActions[`lcd${i + 1}`] || action(),
        ),
        size: w.image_size,
        rotation: w.rotation,
      })
        .then((frames) => {
          if (!cancelled) setFrames(frames);
        })
        .catch((e) => {
          if (!cancelled) setError(String(e));
        });
    }, 100);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [frameActions, w?.image_size, w?.rotation]);
  const fail = (e: unknown) => setError(String(e));
  useEffect(() => {
    let disposed = false;
    const cleanups: (() => void)[] = [];
    const sub = <T,>(event: string, cb: (p: T) => void) =>
      on<T>(event, cb).then((off) => {
        if (disposed) off();
        else cleanups.push(off);
      });
    // Subscribe before reading state; revisions prevent an older snapshot from
    // overriding navigation received while the webview is starting.
    Promise.all([
      sub<Device>("device", (device) => setSnap((s) => s && { ...s, device })),
      sub<Config>("config", (config) =>
        setEditor((current) => receiveConfig(current, config)),
      ),
      sub<Agent[]>("agents", (agents) => setSnap((s) => s && { ...s, agents })),
      sub<InputEvent>("input", setLastInput),
      sub<Navigation>("navigate", applyNavigation),
      sub<string>("action_error", fail),
    ])
      .then(async () => {
        if (disposed) return;
        const s = await call<Snapshot>("snapshot");
        if (!disposed) {
          setSnap(s);
          if (s.settings_error) fail(s.settings_error);
          setEditor((current) => receiveConfig(current, s.config));
          applyNavigation(s.navigation);
        }
      })
      .catch((e) => {
        if (!disposed) fail(e);
      });
    return () => {
      disposed = true;
      cleanups.forEach((off) => off());
    };
  }, []);
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(""), 4000);
    return () => clearTimeout(timer);
  }, [notice]);
  const change = (fn: (w: Config) => void) =>
    setEditor((current) => current && editConfig(current, fn));
  const save = async () => {
    if (!w || saving) return;
    setSaving(true);
    try {
      await call("save_config", { config: w });
      if (!native) setEditor((current) => receiveConfig(current, w));
      setNotice("Changes saved");
    } catch (e) {
      fail(e);
    } finally {
      setSaving(false);
    }
  };
  const perform = async (command: string, args?: Record<string, unknown>) => {
    try {
      await call(command, args);
    } catch (e) {
      fail(e);
    }
  };
  const navigate = async (view: View) => {
    setView(view);
    if (native) {
      try {
        applyNavigation(await call<Navigation>("set_view", { view }));
      } catch (e) {
        fail(e);
      }
    }
  };
  const run = async (a: Action, binding: string) => {
    if (dirty) {
      setError("Save your changes before running an action.");
      return;
    }
    await perform("run_action", { action: a, binding });
  };
  if (!w || !snap)
    return (
      <div className="loading">
        <Grid2X2 size={36} />
        <h1>OpenStreamDeck</h1>
        <p>{error || "Opening your pages…"}</p>
      </div>
    );
  const page = w.pages.find((p) => p.id === w.active_page)!;
  const binding = page.bindings[selected] || action();
  const workflow = w.workflows.find((f) => f.id === workflowId);
  const step = workflow?.steps[stepIndex];
  const fixedNavigation = selected === "button1" || selected === "button2";
  const editBinding = (a: Action) => {
    if (fixedNavigation) return;
    change((w) => {
      w.pages.find((p) => p.id === w.active_page)!.bindings[selected] = a;
    });
  };
  const editWorkflow = (fn: (f: NonNullable<typeof workflow>) => void) =>
    change((w) => {
      const f = w.workflows.find((f) => f.id === workflowId);
      if (f) fn(f);
    });
  const confirmPageRename = (name: string) => {
    if (!editor || !renameTarget) return;
    try {
      setEditor(
        editConfig(editor, (draft) => renamePage(draft, renameTarget.id, name)),
      );
      setRenameTarget(null);
      setNotice("Page renamed. Save changes to apply.");
    } catch (e) {
      fail(e);
    }
  };
  const confirmPageDeletion = () => {
    if (!editor || !deletePageId) return;
    try {
      setEditor(editConfig(editor, (draft) => removePage(draft, deletePageId)));
      setDeletePageId(null);
      setNotice("Page removed. Save changes to apply.");
    } catch (e) {
      fail(e);
    }
  };
  const exportFile = async () => {
    try {
      if (native) {
        const { save } = await import("@tauri-apps/plugin-dialog");
        const path = await save({
          defaultPath: "config.openstreamdeck.json",
          filters: [
            { name: "OpenStreamDeck configuration", extensions: ["json"] },
          ],
        });
        if (path) await call("write_config_file", { path, config: w });
      } else {
        const u = URL.createObjectURL(
          new Blob([JSON.stringify(w, null, 2)], { type: "application/json" }),
        );
        const a = document.createElement("a");
        a.href = u;
        a.download = "config.openstreamdeck.json";
        a.click();
        URL.revokeObjectURL(u);
      }
    } catch (e) {
      fail(e);
    }
  };
  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">
            <Grid2X2 size={22} />
          </span>
          <div>
            <span className="brand-name">OpenStreamDeck</span>
            <small>YOUR DESK, CONNECTED</small>
          </div>
        </div>
        <div className="nav-caption">OPENSTREAMDECK</div>
        <nav>
          {views.map(([name, C]) => (
            <button
              key={name}
              className={view === name ? "nav active" : "nav"}
              onClick={() => void navigate(name)}
            >
              <C size={18} />
              {name}
              {name === "Agents" && snap.agents.length > 0 && (
                <span className="count">{snap.agents.length}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="device-mini">
            <Cable size={20} />
            <div>
              N3
              <small>
                <i className={snap.device.connected ? "dot online" : "dot"} />
                {snap.device.connected
                  ? "Connected over USB"
                  : native
                    ? "Not connected"
                    : "Browser preview"}
              </small>
            </div>
          </div>
          <button
            className="connect"
            onClick={() =>
              perform("connect_device", { connect: !snap.device.connected })
            }
          >
            {snap.device.connected ? "Disconnect device" : "Connect device"}
          </button>
          <span className="version">OpenStreamDeck 0.1 · Local first</span>
        </div>
      </aside>
      <main>
        <header className="topbar">
          <div className="breadcrumb">
            OpenStreamDeck <ChevronRight size={14} />
            <strong>{view}</strong>
          </div>
          <div className="row">
            {!native && <span className="badge">Browser preview</span>}
            <span className={dirty ? "unsaved" : "muted"}>
              {dirty ? "Unsaved changes" : "All changes saved"}
            </span>
            <button
              className="primary"
              onClick={save}
              disabled={!dirty || saving}
            >
              <Save size={15} />
              Save changes
            </button>
          </div>
        </header>
        <div className="content">
          <div className="page-heading">
            <div>
              <div className="eyebrow">
                {view === "Deck"
                  ? "MAKE IT YOURS"
                  : view === "AI workflows"
                    ? "A SHORTCUT FROM IDEA TO ACTION"
                    : view === "Agents"
                      ? "YOUR AGENTS, AT A GLANCE"
                      : "DESIGNED AROUND YOU"}
              </div>
              <h1>{view === "Deck" ? "Your command center" : view}</h1>
              <p>
                {view === "Deck"
                  ? "Six keys. Three dials. Everything within reach."
                  : view === "AI workflows"
                    ? "Connect inputs, models and actions. Put a workflow on any key."
                    : view === "Agents"
                      ? "Live agent activity, including sub-agents. Ready means between turns; it does not mean the task is complete."
                      : "A few thoughtful adjustments. A deck that feels like yours."}
              </p>
            </div>
            {view === "Deck" && (
              <div className="row">
                <button onClick={exportFile}>
                  <Download size={15} />
                  Export
                </button>
                <label className="button">
                  <Upload size={15} />
                  Import
                  <input
                    hidden
                    type="file"
                    accept=".json"
                    onChange={async (e) => {
                      const f = e.target.files?.[0];
                      if (f)
                        try {
                          const value = JSON.parse(await f.text());
                          const imported = native
                            ? await call<Config>("import_config", {
                                value,
                              })
                            : validateConfig(value);
                          setEditor(
                            (current) =>
                              current && { ...current, draft: imported },
                          );
                          setNotice("Imported for review. Save to apply.");
                        } catch (e) {
                          fail(e);
                        }
                    }}
                  />
                </label>
              </div>
            )}
          </div>
          {view === "Deck" && (
            <>
              <div className="deck-toolbar">
                <div className="page-tabs">
                  {w.pages.map((p) => (
                    <button
                      key={p.id}
                      className={p.id === page.id ? "selected" : ""}
                      onClick={() =>
                        change((w) => {
                          w.active_page = p.id;
                        })
                      }
                    >
                      {p.name}
                    </button>
                  ))}
                  <button
                    aria-label="Add page"
                    onClick={() =>
                      change((w) => {
                        const id = uid();
                        w.pages.push({
                          id,
                          name: `Page ${w.pages.length + 1}`,
                          bindings: {},
                        });
                        w.active_page = id;
                      })
                    }
                  >
                    <Plus size={15} />
                  </button>
                </div>
                <div className="row page-actions">
                  <button
                    className="rename-page"
                    onClick={() =>
                      setRenameTarget({ id: page.id, name: page.name })
                    }
                  >
                    <Pencil size={15} />
                    Rename page
                  </button>
                  <button
                    className="delete-page danger"
                    disabled={w.pages.length < 2}
                    title={
                      w.pages.length < 2
                        ? "Keep at least one page"
                        : `Delete ${page.name}`
                    }
                    onClick={() => setDeletePageId(page.id)}
                  >
                    <Trash2 size={15} />
                    Delete page
                  </button>
                </div>
              </div>
              {w.pages.length === 1 && (
                <p className="muted">
                  You have one page. Add a page with + above to use the
                  Previous/Next buttons. Keep at least one page.
                </p>
              )}
              <div className="deck-layout">
                <section className="panel device-stage">
                  <div className="panel-title">
                    <span>
                      <i
                        className={snap.device.connected ? "dot online" : "dot"}
                      />{" "}
                      N3
                    </span>
                    <span className="muted">
                      {snap.device.connected
                        ? snap.device.serial.slice(-4)
                        : "Device layout"}
                    </span>
                  </div>
                  <div className="hardware">
                    <div className="lcd-grid">
                      {Array.from({ length: 6 }, (_, i) => `lcd${i + 1}`).map(
                        (id, i) => {
                          const a = page.bindings[id] || action();
                          return (
                            <button
                              key={id}
                              aria-label={`LCD ${i + 1}: ${a.title}`}
                              className={`lcd ${selected === id ? "selected" : ""} ${lastInput?.control === id && Date.now() - lastInput.at < 800 ? "pressed" : ""}`}
                              style={{ background: a.color }}
                              onClick={() => setSelected(id)}
                            >
                              {frames[i] ? (
                                <img
                                  className="device-frame"
                                  src={frames[i]}
                                  style={{
                                    transform: `rotate(${-w.rotation}deg)`,
                                  }}
                                  alt={a.title}
                                />
                              ) : a.image ? (
                                <img src={a.image} alt={a.title} />
                              ) : (
                                <>
                                  <Icon kind={a.kind} size={28} />
                                  <span>{a.title}</span>
                                </>
                              )}
                              <small>{i + 1}</small>
                            </button>
                          );
                        },
                      )}
                    </div>
                    <div className="dials">
                      {[1, 2, 3].map((n) => (
                        <div className={`dial-group dial-${n}`} key={n}>
                          <button
                            className={`dial ${selected === `knob${n}` ? "selected" : ""}`}
                            aria-label={`Dial ${n} press`}
                            onClick={() => setSelected(`knob${n}`)}
                          >
                            <span />
                          </button>
                          <div className="dial-label">DIAL {n}</div>
                        </div>
                      ))}
                    </div>
                    <div className="hardware-buttons">
                      {[1, 2, 3].map((n) => (
                        <button
                          key={n}
                          className={
                            selected === `button${n}` ? "selected" : ""
                          }
                          onClick={() => setSelected(`button${n}`)}
                          aria-label={`Button ${n}`}
                        >
                          {n === 1 ? (
                            <ChevronLeft size={17} />
                          ) : n === 2 ? (
                            <ChevronRight size={17} />
                          ) : (
                            <Power size={15} />
                          )}
                        </button>
                      ))}
                    </div>
                    <div className="hardware-label">
                      N3 <span>OPENSTREAMDECK</span>
                    </div>
                  </div>
                  <div className="device-caption">
                    Select a key, dial press, or rotation to edit its action.
                  </div>
                  <div className="device-footer">
                    <Sun size={16} />
                    <input
                      aria-label="Brightness"
                      type="range"
                      min="0"
                      max="100"
                      value={w.brightness}
                      onChange={(e) =>
                        change((w) => {
                          w.brightness = +e.target.value;
                        })
                      }
                    />
                    <span>{w.brightness}%</span>
                    <button
                      onClick={() =>
                        perform("sleep_device", {
                          sleep: !snap.device.sleeping,
                        })
                      }
                    >
                      <Power size={15} />
                      {snap.device.sleeping ? "Wake" : "Sleep"}
                    </button>
                  </div>
                </section>
                <section className="panel inspector">
                  <div className="panel-title">
                    <span>
                      {selected
                        .replace("lcd", "Key ")
                        .replace("knob", "Dial ")
                        .replace("button", "Button ")}
                    </span>
                    <span className="badge">
                      {fixedNavigation ? "FIXED CONTROL" : "ACTION"}
                    </span>
                  </div>
                  {fixedNavigation ? (
                    <div className="fixed-navigation">
                      <h2>
                        {selected === "button1" ? "Previous page" : "Next page"}
                      </h2>
                      <p>
                        This button always cycles through all pages, including
                        empty pages. Its role cannot be changed.
                      </p>
                      <p className="muted">
                        {w.pages.length === 1
                          ? "You have one page. Use + beside the page tabs to add another page."
                          : `${w.pages.length} pages · navigation wraps at either end.`}
                      </p>
                    </div>
                  ) : (
                    <>
                      {selected.startsWith("knob") && (
                        <div className="dial-inspector">
                          <div className="row">
                            {[
                              ["", "Press"],
                              [".ccw", "Turn left"],
                              [".cw", "Turn right"],
                            ].map(([suffix, label]) => (
                              <button
                                key={suffix}
                                className={
                                  selected === selected.split(".")[0] + suffix
                                    ? "selected"
                                    : ""
                                }
                                onClick={() =>
                                  setSelected(selected.split(".")[0] + suffix)
                                }
                              >
                                {label}
                              </button>
                            ))}
                          </div>
                          <Field label="Assign the whole dial">
                            <select
                              value=""
                              onChange={(e) => {
                                const preset = e.target.value as
                                  "brightness" | "volume" | "agents";
                                change((w) =>
                                  assignDial(
                                    w.pages.find(
                                      (p) => p.id === w.active_page,
                                    )!,
                                    selected.split(".")[0],
                                    preset,
                                  ),
                                );
                              }}
                            >
                              <option value="" disabled>
                                Choose a dial function…
                              </option>
                              <option value="brightness">
                                Brightness: turn ±5%, press 60%
                              </option>
                              <option value="volume">
                                Volume: turn ±, press mute
                              </option>
                              <option value="agents">
                                Agents: turn to select, press focus
                              </option>
                            </select>
                          </Field>
                        </div>
                      )}
                      <ActionEditor
                        key={selected}
                        value={binding}
                        change={editBinding}
                        config={w}
                      />
                      {selected.startsWith("lcd") &&
                        frames[Number(selected.slice(3)) - 1] && (
                          <div className="native-pixels">
                            <img
                              width={w.image_size}
                              height={w.image_size}
                              src={frames[Number(selected.slice(3)) - 1]}
                              style={{ transform: `rotate(${-w.rotation}deg)` }}
                              alt="Exact device pixels"
                            />
                            <span>
                              {w.image_size} × {w.image_size} · native pixels
                            </span>
                          </div>
                        )}
                      <div className="row inspector-actions">
                        <button
                          className="primary"
                          onClick={() => run(binding, selected)}
                        >
                          <Play size={14} />
                          Run action
                        </button>
                        <button
                          title="Clear binding"
                          onClick={() => editBinding(action())}
                        >
                          <Trash2 size={15} />
                        </button>
                      </div>
                    </>
                  )}
                </section>
              </div>
              <section className="panel library">
                <div className="panel-title">
                  <span>Action library</span>
                  <label className="search">
                    <Search size={15} />
                    <input
                      placeholder="Find an action…"
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                    />
                  </label>
                </div>
                {fixedNavigation && (
                  <p className="muted">
                    Select a key, dial or power button to assign an action. The
                    page arrows have fixed roles.
                  </p>
                )}
                <div className="action-grid">
                  {catalog
                    .filter(
                      ([id, title]) =>
                        id !== "none" &&
                        title.toLowerCase().includes(query.toLowerCase()),
                    )
                    .map(([id, title, group]) => (
                      <button
                        key={id}
                        disabled={fixedNavigation}
                        onClick={() => editBinding(configuredAction(id))}
                      >
                        <span className="action-icon">
                          <Icon kind={id} size={18} />
                        </span>
                        <div>
                          {title}
                          <small>{group}</small>
                        </div>
                        <Plus size={14} />
                      </button>
                    ))}
                </div>
              </section>
            </>
          )}
          {view === "AI workflows" && (
            <div className="workflow-layout">
              <section className="panel workflow-list">
                <div className="panel-title">
                  Workflows
                  <button
                    aria-label="Add workflow"
                    onClick={() => {
                      const id = uid();
                      change((w) =>
                        w.workflows.push({
                          id,
                          name: "New workflow",
                          description: "",
                          steps: [],
                        }),
                      );
                      setWorkflowId(id);
                      setStepIndex(0);
                    }}
                  >
                    <Plus size={16} />
                  </button>
                </div>
                {w.workflows.map((f) => (
                  <button
                    className={`workflow-link ${f.id === workflowId ? "selected" : ""}`}
                    key={f.id}
                    onClick={() => {
                      setWorkflowId(f.id);
                      setStepIndex(0);
                    }}
                  >
                    <Sparkles size={17} />
                    <div>
                      {f.name}
                      <small>{f.steps.length} steps</small>
                    </div>
                  </button>
                ))}
              </section>
              {workflow ? (
                <>
                  <section className="panel workflow-canvas">
                    <div className="panel-title">
                      <span>Workflow</span>
                      <button
                        className="primary"
                        onClick={() =>
                          run(
                            action("workflow", workflow.name, workflow.id),
                            workflow.id,
                          )
                        }
                      >
                        <Play size={14} />
                        Run workflow
                      </button>
                    </div>
                    <Field label="Name">
                      <input
                        value={workflow.name}
                        onChange={(e) =>
                          editWorkflow((f) => {
                            f.name = e.target.value;
                          })
                        }
                      />
                    </Field>
                    <Field label="Description">
                      <input
                        value={workflow.description}
                        onChange={(e) =>
                          editWorkflow((f) => {
                            f.description = e.target.value;
                          })
                        }
                      />
                    </Field>
                    <div className="steps">
                      {workflow.steps.map((s, i) => (
                        <div key={i}>
                          <button
                            className={`step ${i === stepIndex ? "selected" : ""}`}
                            onClick={() => setStepIndex(i)}
                          >
                            <span className="step-number">{i + 1}</span>
                            <Icon kind={s.kind} />
                            <div>
                              {s.title}
                              <small>
                                {catalog.find((c) => c[0] === s.kind)?.[1] ||
                                  s.kind}
                              </small>
                            </div>
                            <ChevronRight size={16} />
                          </button>
                          {i < workflow.steps.length - 1 && (
                            <ArrowDown className="connector" size={18} />
                          )}
                        </div>
                      ))}
                    </div>
                    <button
                      className="add-step"
                      onClick={() => {
                        setStepIndex(workflow.steps.length);
                        editWorkflow((f) =>
                          f.steps.push(action("ai", "AI prompt", "{{input}}")),
                        );
                      }}
                    >
                      <Plus size={17} />
                      Add step
                    </button>
                    <div className="row workflow-bottom">
                      <button
                        onClick={() => {
                          const id = uid();
                          change((w) =>
                            w.workflows.push({
                              ...structuredClone(workflow),
                              id,
                              name: workflow.name + " copy",
                            }),
                          );
                          setWorkflowId(id);
                        }}
                      >
                        <Copy size={14} />
                        Duplicate
                      </button>
                      <button
                        onClick={() => {
                          change((w) => {
                            w.workflows = w.workflows.filter(
                              (f) => f.id !== workflowId,
                            );
                          });
                          setWorkflowId(
                            w.workflows.find((f) => f.id !== workflowId)?.id ||
                              "",
                          );
                        }}
                      >
                        <Trash2 size={14} />
                        Delete
                      </button>
                    </div>
                  </section>
                  <section className="panel inspector">
                    <div className="panel-title">
                      {step ? `Step ${stepIndex + 1}` : "Build your workflow"}
                    </div>
                    {step ? (
                      <>
                        <ActionEditor
                          key={`${workflowId}:${stepIndex}`}
                          value={step}
                          config={w}
                          change={(a) =>
                            editWorkflow((f) => {
                              f.steps[stepIndex] = a;
                            })
                          }
                        />
                        <div className="row">
                          <button
                            disabled={stepIndex === 0}
                            aria-label="Move step up"
                            onClick={() => {
                              editWorkflow((f) => {
                                [f.steps[stepIndex - 1], f.steps[stepIndex]] = [
                                  f.steps[stepIndex],
                                  f.steps[stepIndex - 1],
                                ];
                              });
                              setStepIndex(stepIndex - 1);
                            }}
                          >
                            <ArrowUp size={16} />
                          </button>
                          <button
                            disabled={stepIndex === workflow.steps.length - 1}
                            aria-label="Move step down"
                            onClick={() => {
                              editWorkflow((f) => {
                                [f.steps[stepIndex + 1], f.steps[stepIndex]] = [
                                  f.steps[stepIndex],
                                  f.steps[stepIndex + 1],
                                ];
                              });
                              setStepIndex(stepIndex + 1);
                            }}
                          >
                            <ArrowDown size={16} />
                          </button>
                          <button
                            aria-label="Delete step"
                            onClick={() => {
                              editWorkflow((f) => f.steps.splice(stepIndex, 1));
                              setStepIndex(Math.max(0, stepIndex - 1));
                            }}
                          >
                            <Trash2 size={16} />
                          </button>
                        </div>
                      </>
                    ) : (
                      <p className="muted">
                        Add a step to start. Each result becomes the next step’s
                        input.
                      </p>
                    )}
                  </section>
                </>
              ) : (
                <div className="empty">
                  <Sparkles />
                  <h2>Create your first workflow</h2>
                  <p>Add a workflow using the + button.</p>
                </div>
              )}
            </div>
          )}
          {view === "Agents" && (
            <>
              <div className="section-toolbar">
                <span className="badge">
                  <Radio size={13} />
                  {snap.agents.length} sessions
                </span>
                <div className="row">
                  <button
                    onClick={() => perform("show_agents", { enabled: true })}
                  >
                    <Grid2X2 size={15} />
                    Show on deck
                  </button>
                  <button
                    onClick={() => perform("show_agents", { enabled: false })}
                  >
                    Return to keys
                  </button>
                </div>
              </div>
              <div className="agent-grid">
                {snap.agents.map((a) => (
                  <section className="panel agent-card" key={a.id}>
                    <div className="row">
                      <span className="agent-logo">
                        <Bot size={24} />
                      </span>
                      <span className={`status ${a.status}`}>{a.status}</span>
                    </div>
                    <h2>{a.title}</h2>
                    <p>
                      {a.agent} · {a.source}
                      {a.is_subagent && " · Sub-agent"}
                    </p>
                    {a.parent_session_id && (
                      <small className="muted">
                        Parent:{" "}
                        {snap.agents.find(
                          (parent) => parent.session_id === a.parent_session_id,
                        )?.title || a.parent_session_id}
                      </small>
                    )}
                    <small className="muted">
                      Reported state: {a.raw_status}
                    </small>
                    <div className="row">
                      <button
                        disabled={!a.can_focus}
                        onClick={() => perform("agent_focus", { id: a.id })}
                      >
                        <ExternalLink size={14} />
                        Focus
                      </button>
                      <button
                        disabled={!a.can_prompt || !prompt.trim()}
                        onClick={() =>
                          perform("agent_prompt", { id: a.id, text: prompt })
                        }
                      >
                        Send prompt
                      </button>
                    </div>
                  </section>
                ))}
              </div>
              {!snap.agents.length && (
                <div className="empty">
                  <Bot size={40} />
                  <h2>No active sessions found</h2>
                  <p>
                    Open a Herdr or Codex session, or connect Claude Code with
                    the local hook bridge.
                  </p>
                  <p className="muted">
                    Status appears only when an adapter reports it.
                  </p>
                </div>
              )}
              <section className="panel">
                <Field label="Prompt to send to a selected session">
                  <textarea
                    value={prompt}
                    onChange={(e) => setPrompt(e.target.value)}
                    placeholder="Describe the next task…"
                  />
                </Field>
                <button
                  onClick={async () => {
                    const next = await call<Snapshot>("snapshot").catch((e) => {
                      fail(e);
                      return null;
                    });
                    if (next) setSnap(next);
                  }}
                >
                  Refresh adapter diagnostics
                </button>
                {snap.adapter_errors.map((e) => (
                  <p className="error-text" key={e}>
                    {e}
                  </p>
                ))}
              </section>
            </>
          )}
          {view === "Settings" && (
            <SettingsPanel
              config={w}
              snapshot={snap}
              dirty={dirty}
              change={change}
              fail={fail}
              notice={setNotice}
            />
          )}
        </div>
      </main>
      {renameTarget && (
        <RenamePageDialog
          config={w}
          pageId={renameTarget.id}
          initialName={renameTarget.name}
          close={() => setRenameTarget(null)}
          confirm={confirmPageRename}
        />
      )}{" "}
      {deletePageId && (
        <DeletePageDialog
          config={w}
          pageId={deletePageId}
          close={() => setDeletePageId(null)}
          confirm={confirmPageDeletion}
        />
      )}{" "}
      {notice && (
        <div className="toast">
          <Check size={16} />
          {notice}
        </div>
      )}
      {error && (
        <div className="error-banner" role="alert">
          <span>{error}</span>
          <button aria-label="Dismiss error" onClick={() => setError("")}>
            <X size={18} />
          </button>
        </div>
      )}
    </div>
  );
}
