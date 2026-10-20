import { useState, type ReactNode } from "react";
import {
  Bot,
  Cable,
  ChevronRight,
  CircleHelp,
  Fingerprint,
  Monitor,
  Moon,
  Power,
  Rocket,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
} from "lucide-react";
import { call, native } from "./api";
import type { Config, Snapshot } from "./model";

type Tab = "general" | "device" | "agents";
const tabs = [
  { id: "general", label: "General", icon: SlidersHorizontal },
  { id: "device", label: "Device", icon: Monitor },
  { id: "agents", label: "Agents & AI", icon: Bot },
] as const;

function SettingRow({
  icon,
  title,
  description,
  children,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="setting-row">
      <span className="setting-icon">{icon}</span>
      <div className="setting-copy">
        <h3>{title}</h3>
        <p>{description}</p>
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}
function Toggle({
  label,
  checked,
  onChange,
  description,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  description: string;
}) {
  return (
    <button
      type="button"
      className="settings-switch"
      role="switch"
      aria-label={label}
      aria-checked={checked}
      aria-description={description}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

export default function SettingsPanel({
  config,
  snapshot,
  dirty,
  change,
  fail,
  notice,
}: {
  config: Config;
  snapshot: Snapshot;
  dirty: boolean;
  change: (edit: (config: Config) => void) => void;
  fail: (error: unknown) => void;
  notice: (message: string) => void;
}) {
  const [tab, setTab] = useState<Tab>("general");
  const [providerId, setProviderId] = useState(config.providers[0]?.id ?? "");
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [keyBusy, setKeyBusy] = useState(false);
  const provider =
    config.providers.find((p) => p.id === providerId) ?? config.providers[0];
  const updateProvider = (field: "endpoint" | "model", value: string) => {
    if (provider)
      change((config) => {
        config.providers.find((p) => p.id === provider.id)![field] = value;
      });
  };
  const saveKey = async (remove: boolean) => {
    if (!provider || keyBusy) return;
    const id = provider.id;
    setKeyBusy(true);
    try {
      await call("set_provider_key", {
        provider: id,
        key: remove ? "" : keys[id],
      });
      setKeys((keys) => ({ ...keys, [id]: "" }));
      notice(remove ? "Stored key removed" : "Key saved securely");
    } catch (error) {
      fail(error);
    } finally {
      setKeyBusy(false);
    }
  };
  return (
    <div className="settings-shell">
      <div
        className="settings-tabs"
        role="tablist"
        aria-label="Settings categories"
      >
        {tabs.map(({ id, label, icon: Icon }, index) => (
          <button
            key={id}
            type="button"
            role="tab"
            id={`settings-tab-${id}`}
            aria-controls={`settings-panel-${id}`}
            aria-selected={tab === id}
            tabIndex={tab === id ? 0 : -1}
            onClick={() => setTab(id)}
            onKeyDown={(event) => {
              let next = index;
              if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
              else if (event.key === "ArrowLeft")
                next = (index + tabs.length - 1) % tabs.length;
              else if (event.key === "Home") next = 0;
              else if (event.key === "End") next = tabs.length - 1;
              else return;
              event.preventDefault();
              setTab(tabs[next].id);
              document.getElementById(`settings-tab-${tabs[next].id}`)?.focus();
            }}
          >
            <Icon size={17} />
            {label}
          </button>
        ))}
        <span className="settings-save-note">Changes apply when you save</span>
      </div>
      <section
        className="settings-tab-content"
        role="tabpanel"
        id={`settings-panel-${tab}`}
        aria-labelledby={`settings-tab-${tab}`}
        tabIndex={0}
      >
        {tab === "general" && (
          <>
            <div className="settings-section-heading">
              <h2>Make yourself at home</h2>
              <p>Choose how OpenStreamDeck fits into your day.</p>
            </div>
            <div className="settings-card">
              <div className="settings-card-heading">
                <Power size={17} />
                <h3>Startup & window behavior</h3>
              </div>
              <SettingRow
                icon={<Rocket size={21} />}
                title="Launch at login"
                description="Start OpenStreamDeck automatically when you sign in to your computer."
              >
                <Toggle
                  label="Launch at login"
                  checked={config.launch_at_login}
                  description="Applied when you save changes."
                  onChange={(value) =>
                    change((config) => {
                      config.launch_at_login = value;
                    })
                  }
                />
              </SettingRow>
              <SettingRow
                icon={<Monitor size={21} />}
                title="Menu bar only"
                description="Keep your deck running in the menu bar or system tray after you close the window."
              >
                <Toggle
                  label="Menu bar only"
                  checked={config.menu_bar_only}
                  description="When off, closing the window quits OpenStreamDeck. Applied when you save changes."
                  onChange={(value) =>
                    change((config) => {
                      config.menu_bar_only = value;
                    })
                  }
                />
              </SettingRow>
              <div className="settings-behavior">
                <CircleHelp size={16} />
                <p>
                  {config.menu_bar_only
                    ? "Closing the window keeps your keys, workflows and agents running. Reopen from the tray; choose Quit to exit."
                    : "Closing the window will quit OpenStreamDeck. Device controls, workflows and agent monitoring stop with it."}
                </p>
              </div>
            </div>
            {!native && (
              <p className="settings-footnote">
                Browser preview saves preferences locally. Login and window
                behavior apply in the desktop app.
              </p>
            )}
            <div className="settings-card settings-storage">
              <span className="setting-icon">
                <Fingerprint size={21} />
              </span>
              <div>
                <h3>Yours, on this computer</h3>
                <p>Your pages and preferences are saved locally.</p>
                <code>{snapshot.data_dir}</code>
              </div>
              <span className="badge">Local storage</span>
            </div>
          </>
        )}
        {tab === "device" && (
          <>
            <div className="settings-section-heading">
              <h2>Your N3, your way</h2>
              <p>Fine-tune the display and choose when your deck rests.</p>
            </div>
            <div className="settings-device-status">
              <span className="setting-icon">
                <Cable size={23} />
              </span>
              <div>
                <h3>
                  N3 controller{" "}
                  <span
                    className={`settings-connection ${snapshot.device.connected ? "connected" : ""}`}
                  >
                    {snapshot.device.connected ? "Connected" : "Not connected"}
                  </span>
                </h3>
                <p>{snapshot.device.message}</p>
                <small>
                  Connects automatically at launch and when plugged in.
                </small>
              </div>
              <span className="settings-usb">USB 5548:1001</span>
            </div>
            <div className="settings-card">
              <div className="settings-card-heading">
                <Moon size={17} />
                <h3>Power saving</h3>
              </div>
              <SettingRow
                icon={<Moon size={21} />}
                title="Sleep after inactivity"
                description="Turn off the display after a quiet period. Set to 0 to keep it awake."
              >
                <label className="settings-number">
                  <span className="sr-only">
                    Sleep after inactivity (minutes)
                  </span>
                  <input
                    type="number"
                    min="0"
                    max="1440"
                    step="1"
                    value={config.sleep_minutes}
                    onChange={(event) =>
                      change((config) => {
                        config.sleep_minutes = +event.target.value;
                      })
                    }
                  />
                  <span>min</span>
                </label>
              </SettingRow>
              <div className="settings-behavior">
                <span className="settings-dot" />
                <p>
                  {config.sleep_minutes === 0
                    ? "Automatic sleep is deactivated."
                    : `Display sleeps after ${config.sleep_minutes} minutes without device input.`}{" "}
                  You can still use the Sleep action at any time.
                </p>
              </div>
            </div>
            <div className="settings-card">
              <div className="settings-card-heading">
                <Monitor size={17} />
                <h3>Display geometry</h3>
                <span className="badge">Advanced</span>
              </div>
              <div className="settings-field-grid">
                <label>
                  Image size
                  <select
                    aria-label="Image size"
                    value={config.image_size}
                    onChange={(event) =>
                      change((config) => {
                        config.image_size = +event.target.value;
                      })
                    }
                  >
                    {[
                      ...new Set([
                        32,
                        64,
                        72,
                        85,
                        96,
                        112,
                        128,
                        config.image_size,
                      ]),
                    ]
                      .sort((a, b) => a - b)
                      .map((size) => (
                        <option key={size} value={size}>
                          {size} × {size} pixels
                        </option>
                      ))}
                  </select>
                </label>
                <label>
                  Image rotation
                  <select
                    aria-label="Image rotation"
                    value={config.rotation}
                    onChange={(event) =>
                      change((config) => {
                        config.rotation = +event.target.value;
                      })
                    }
                  >
                    {[0, 90, 180, 270].map((rotation) => (
                      <option key={rotation} value={rotation}>
                        {rotation}°{rotation === 90 ? " · Default" : ""}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <div className="settings-behavior">
                <CircleHelp size={16} />
                <p>
                  Default: 64 × 64 pixels, rotated 90° clockwise. Geometry
                  varies by firmware; check the result on your device.
                </p>
              </div>
            </div>
          </>
        )}
        {tab === "agents" && (
          <>
            <div className="settings-section-heading">
              <h2>Connect your intelligence</h2>
              <p>
                Configure providers for workflows and see your local agent
                connections.
              </p>
            </div>
            <div className="settings-card">
              <div className="settings-card-heading">
                <Sparkles size={17} />
                <h3>AI providers</h3>
                <span className="badge">Your choice of model</span>
              </div>
              {provider ? (
                <div className="settings-provider-layout">
                  <div
                    className="settings-provider-list"
                    aria-label="AI providers"
                  >
                    {config.providers.map((p) => (
                      <button
                        type="button"
                        key={p.id}
                        aria-pressed={p.id === provider.id}
                        onClick={() => setProviderId(p.id)}
                      >
                        <span
                          className={`provider-monogram provider-${p.kind}`}
                        >
                          {p.name.slice(0, 1)}
                        </span>
                        <span>
                          {p.name}
                          <small>
                            {p.kind === "compatible"
                              ? "OpenAI-compatible"
                              : p.kind === "gemini"
                                ? "Gemini API"
                                : `${p.name} API`}
                          </small>
                        </span>
                        <ChevronRight size={15} />
                      </button>
                    ))}
                  </div>
                  <section
                    className="settings-provider-editor"
                    aria-label={`${provider.name} configuration`}
                  >
                    <div className="settings-provider-title">
                      <h3>{provider.name}</h3>
                      <span className="badge">{provider.kind}</span>
                    </div>
                    <label>
                      Endpoint
                      <input
                        type="url"
                        spellCheck={false}
                        value={provider.endpoint}
                        onChange={(event) =>
                          updateProvider("endpoint", event.target.value)
                        }
                      />
                    </label>
                    <label>
                      Model ID
                      <input
                        spellCheck={false}
                        value={provider.model}
                        placeholder="Enter a model available to your account"
                        onChange={(event) =>
                          updateProvider("model", event.target.value)
                        }
                      />
                    </label>
                    <div className="settings-credential">
                      <label>
                        API key
                        <input
                          type="password"
                          autoComplete="off"
                          value={keys[provider.id] ?? ""}
                          placeholder="Enter a new key"
                          onChange={(event) =>
                            setKeys((keys) => ({
                              ...keys,
                              [provider.id]: event.target.value,
                            }))
                          }
                        />
                      </label>
                      <p>
                        <ShieldCheck size={14} />
                        Stored in your OS credential store. Never exported.
                      </p>
                      <div className="row">
                        <button
                          type="button"
                          disabled={
                            !native || !keys[provider.id] || dirty || keyBusy
                          }
                          onClick={() => saveKey(false)}
                        >
                          Save key
                        </button>
                        <button
                          type="button"
                          className="settings-text-button"
                          disabled={!native || dirty || keyBusy}
                          onClick={() => saveKey(true)}
                        >
                          Remove stored key
                        </button>
                      </div>
                      {dirty && (
                        <small className="settings-key-hint">
                          Save your changes before updating credentials.
                        </small>
                      )}
                      {!native && (
                        <small className="settings-key-hint">
                          Manage API keys in the desktop app.
                        </small>
                      )}
                    </div>
                  </section>
                </div>
              ) : (
                <p className="settings-footnote">
                  No providers in this configuration. Import a configuration
                  with providers to connect AI workflows.
                </p>
              )}
            </div>
            <div className="settings-card">
              <div className="settings-card-heading">
                <Bot size={17} />
                <h3>Local agents</h3>
                <span className="badge">Detected automatically</span>
              </div>
              <div className="settings-agent-list">
                {[
                  {
                    name: "Herdr",
                    source: "herdr",
                    detail: "Herdr-managed sessions",
                  },
                  {
                    name: "Codex",
                    source: "codex",
                    detail: "Local app-server connection",
                  },
                  {
                    name: "Claude Code",
                    source: "claude",
                    detail: "Local lifecycle hook bridge",
                  },
                ].map((agent) => {
                  const count = snapshot.agents.filter((session) =>
                    session.source.toLowerCase().includes(agent.source),
                  ).length;
                  return (
                    <div key={agent.source}>
                      <Bot size={18} />
                      <span>
                        <strong>{agent.name}</strong>
                        <small>{agent.detail}</small>
                      </span>
                      <span className="settings-agent-state">
                        {count
                          ? `${count} reported ${count === 1 ? "session" : "sessions"}`
                          : "No reported sessions"}
                      </span>
                    </div>
                  );
                })}
              </div>
              <div className="settings-behavior">
                <ShieldCheck size={16} />
                <p>
                  Manage live sessions in the Agents dashboard. Permission
                  requests stay with your agent; only Show on deck enables the
                  session overlay.
                </p>
              </div>
              {snapshot.adapter_errors.length > 0 && (
                <div className="settings-diagnostics">
                  <h4>Connection diagnostics</h4>
                  {snapshot.adapter_errors.map((error, index) => (
                    <p key={index}>{error}</p>
                  ))}
                </div>
              )}
            </div>
          </>
        )}
      </section>
    </div>
  );
}
