import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { ArrowRight, Download, RefreshCw, ShieldCheck } from "lucide-react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { call, native, on } from "./api";

const appIcon = new URL("../src-tauri/icons/source.svg", import.meta.url).href;

export function containDialogFocus(event: KeyboardEvent<HTMLDialogElement>) {
  if (event.key !== "Tab") return;
  const dialog = event.currentTarget;
  const controls = [
    ...dialog.querySelectorAll<HTMLElement>(
      'a[href], button:not(:disabled), [tabindex="0"]',
    ),
  ].filter((element) => element.getClientRects().length > 0);
  const first = controls[0];
  const last = controls.at(-1);
  if (!first || !last) {
    event.preventDefault();
    dialog.focus();
  } else if (
    event.shiftKey &&
    (document.activeElement === first || document.activeElement === dialog)
  ) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

function ReleaseNotes({ notes }: { notes: string }) {
  const [error, setError] = useState<string | null>(null);
  useEffect(() => setError(null), [notes]);
  return (
    <div className="release-markdown" tabIndex={0} aria-label="Release notes">
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) =>
            href ? (
              <a
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(event) => {
                  if (!native) return;
                  event.preventDefault();
                  setError(null);
                  void call("open_release_link", { url: href }).catch((error) =>
                    setError(String(error)),
                  );
                }}
              >
                {children}
              </a>
            ) : (
              <span>{children}</span>
            ),
        }}
      >
        {notes}
      </Markdown>
      {error && (
        <p className="update-dialog-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

export type UpdateStatus = {
  revision: number;
  phase:
    | "idle"
    | "checking"
    | "current"
    | "available"
    | "downloading"
    | "installing"
    | "installed"
    | "error";
  current_version: string;
  version: string | null;
  notes: string | null;
  downloaded: number;
  total: number | null;
  error: string | null;
  install_disabled: string | null;
  last_checked_at: number | null;
  history_error: string | null;
  prompt_version: string | null;
};

function applying(status: UpdateStatus | null) {
  return (
    status?.phase === "downloading" ||
    status?.phase === "installing" ||
    (status?.phase === "installed" && !status.error)
  );
}

function UpdateProgress({ status }: { status: UpdateStatus }) {
  return (
    <div className="update-progress">
      <progress
        aria-label="Update download"
        value={status.total ? status.downloaded : undefined}
        max={status.total ?? 1}
      />
      <small>
        {(status.downloaded / 1024 / 1024).toFixed(2)} MB
        {status.total
          ? ` of ${(status.total / 1024 / 1024).toFixed(2)} MB`
          : ""}
      </small>
    </div>
  );
}

export default function UpdatePanel({
  dirty,
  fail,
}: {
  dirty: boolean;
  fail: (error: unknown) => void;
}) {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [pending, setPending] = useState(false);
  const revision = useRef(-1);
  const apply = (next: UpdateStatus) => {
    if (next && next.revision >= revision.current) {
      revision.current = next.revision;
      setStatus(next);
    }
  };
  useEffect(() => {
    if (!native) return;
    let alive = true;
    let unlisten: (() => void) | undefined;
    // Subscribe before hydrating; revisions prevent a slow snapshot from reverting progress.
    void on<UpdateStatus>("update_status", (next) => {
      if (alive) apply(next);
    })
      .then(async (off) => {
        if (!alive) {
          off();
          return;
        }
        unlisten = off;
        const current = await call<UpdateStatus>("update_status");
        if (alive) apply(current);
      })
      .catch((error) => {
        if (alive) fail(error);
      });
    return () => {
      alive = false;
      unlisten?.();
    };
    // The parent error callback changes on every render; subscriptions belong to this mount.
  }, []);
  const run = async (command: string, args?: Record<string, unknown>) => {
    setPending(true);
    try {
      const next = await call<UpdateStatus | undefined>(command, args);
      if (next) apply(next);
    } catch (error) {
      fail(error);
    } finally {
      setPending(false);
    }
  };
  const phase = status?.phase ?? "idle";
  const busy = pending || phase === "checking" || applying(status);
  const checked =
    status?.last_checked_at == null ? null : new Date(status.last_checked_at);
  const checkedDate =
    checked && Number.isFinite(checked.getTime()) ? checked : null;
  const canInstall =
    !!status?.version &&
    (phase === "available" ||
      phase === "error" ||
      (phase === "installed" && !!status.error));
  const messages: Record<UpdateStatus["phase"], string> = {
    idle: "Updates are checked automatically every 6 hours.",
    checking: "Checking for updates…",
    current: "You’re up to date.",
    available: `Version ${status?.version} is available.`,
    downloading: "Downloading update…",
    installing: "Installing update…",
    installed: "Restarting…",
    error: "The update failed. Please retry.",
  };
  return (
    <section
      className="settings-card update-card"
      aria-labelledby="updates-title"
    >
      <div className="settings-card-heading">
        <Download size={17} />
        <h3 id="updates-title">Software updates</h3>
        {status && (
          <span className="badge">Version {status.current_version}</span>
        )}
      </div>
      <div className="update-content">
        {!native ? (
          <p>Check for updates in the desktop app.</p>
        ) : (
          <>
            <p role={status?.error ? "alert" : "status"}>
              {status?.error ?? messages[phase]}
            </p>
            <small className="muted">
              Last checked:{" "}
              {checkedDate ? (
                <time dateTime={checkedDate.toISOString()}>
                  {checkedDate.toLocaleString()}
                </time>
              ) : (
                "Not yet"
              )}
            </small>
            {status?.history_error && (
              <p role="alert">{status.history_error}</p>
            )}
            {status?.install_disabled && (
              <p className="muted">{status.install_disabled}</p>
            )}
            {status?.notes && (
              <details className="update-notes">
                <summary>Release notes · {status.version}</summary>
                <ReleaseNotes notes={status.notes} />
              </details>
            )}
            {canInstall && dirty && <p>Save your changes before updating.</p>}
            <div className="row update-controls">
              {phase !== "installed" && (
                <button
                  disabled={!status || busy}
                  onClick={() => void run("check_updates")}
                >
                  <RefreshCw size={15} />
                  Check for updates
                </button>
              )}
              {canInstall && (
                <button
                  className="primary"
                  disabled={busy || dirty || !!status?.install_disabled}
                  onClick={() => void run("install_update", { dirty })}
                >
                  Install Update
                </button>
              )}
            </div>
            {canInstall && !status?.install_disabled && (
              <small className="muted">Restarts automatically.</small>
            )}
          </>
        )}
      </div>
    </section>
  );
}

// One progress surface blocks edits throughout the native operation, including download.
// It restores from native state after a webview reload and never asks for another click.
export function UpdateInstallGuard({
  status,
}: {
  status: UpdateStatus | null;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const busy = applying(status);
  const phase = status?.phase;
  const stage = phase === "installed" ? 2 : phase === "installing" ? 1 : 0;
  useEffect(() => {
    const element = dialog.current!;
    if (busy) {
      if (!element.open) element.showModal();
    } else element.close();
  }, [busy]);
  return (
    <dialog
      ref={dialog}
      className="update-dialog update-install-dialog"
      aria-labelledby="update-install-status"
      aria-describedby="update-install-description"
      onCancel={(event) => event.preventDefault()}
      onKeyDown={containDialogFocus}
    >
      <div className="update-dialog-body">
        <div className="update-brand">
          <img src={appIcon} alt="" width="44" height="44" />
          <span>
            OpenStreamDeck<small>SOFTWARE UPDATE</small>
          </span>
        </div>
        <div className="update-install-symbol" aria-hidden="true">
          <RefreshCw size={28} />
        </div>
        <h2 id="update-install-status" aria-live="polite">
          {phase === "installed"
            ? "Restarting…"
            : phase === "installing"
              ? "Installing update…"
              : "Downloading update…"}
        </h2>
        <p id="update-install-description">
          We’ll take it from here. OpenStreamDeck will restart automatically.
        </p>
        <ol className="update-stages" aria-label="Update progress">
          {["Download", "Install", "Restart"].map((label, index) => (
            <li
              key={label}
              className={index <= stage ? "active" : ""}
              aria-current={index === stage ? "step" : undefined}
            >
              <span aria-hidden="true">{index < stage ? "✓" : index + 1}</span>
              {label}
            </li>
          ))}
        </ol>
        {phase === "downloading" && status && (
          <UpdateProgress status={status} />
        )}
      </div>
    </dialog>
  );
}

// Native state preserves the notice across window creation/reloads and remembers skips.
export function UpdatePrompt({
  status,
  dirty,
  apply,
  fail,
}: {
  status: UpdateStatus | null;
  dirty: boolean;
  apply: (status: UpdateStatus) => void;
  fail: (error: unknown) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const version = status?.prompt_version;
  useEffect(() => {
    setError(null);
    const element = dialog.current!;
    if (version) {
      if (!element.open) element.showModal();
    } else element.close();
  }, [version]);
  const run = async (command: "install_update" | "skip_update") => {
    if (!version || pending) return;
    setPending(true);
    setError(null);
    try {
      const next = await call<UpdateStatus>(
        command,
        command === "install_update" ? { dirty } : { version },
      );
      apply(next);
      if (next.error && !next.prompt_version) fail(next.error);
    } catch (error) {
      setError(String(error));
    } finally {
      setPending(false);
    }
  };
  return (
    <dialog
      ref={dialog}
      className="update-dialog"
      aria-labelledby="update-available-title"
      aria-describedby="update-available-description"
      onCancel={(event) => event.preventDefault()}
      onKeyDown={containDialogFocus}
    >
      <div className="update-dialog-body">
        <div className="update-brand">
          <img src={appIcon} alt="" width="44" height="44" />
          <span>
            OpenStreamDeck<small>SOFTWARE UPDATE</small>
          </span>
        </div>
        <h2 id="update-available-title">Update available</h2>
        <p id="update-available-description">
          A new version of OpenStreamDeck is ready to install.
        </p>
        <div className="update-version-change" aria-label="Version upgrade">
          <div>
            <span>Current version</span>
            <strong>{status?.current_version}</strong>
          </div>
          <ArrowRight size={20} aria-hidden="true" />
          <div>
            <span>
              New version <em>NEW</em>
            </span>
            <strong>{version}</strong>
          </div>
        </div>
        {status?.notes?.trim() && (
          <section
            className="update-release-preview"
            aria-labelledby="update-release-heading"
          >
            <h3 id="update-release-heading">What’s new</h3>
            <ReleaseNotes notes={status.notes} />
          </section>
        )}
        {dirty && (
          <p className="update-dialog-warning">
            You have unsaved changes. Skip this update to keep editing, then
            install it from Settings when you’re ready.
          </p>
        )}
        {status?.install_disabled && (
          <p className="update-dialog-warning">{status.install_disabled}</p>
        )}
        {error && (
          <p className="update-dialog-error" role="alert">
            {error}
          </p>
        )}
        <p className="update-restart-note">
          <ShieldCheck size={16} aria-hidden="true" /> Installs securely and
          restarts automatically.
        </p>
      </div>
      <footer className="update-dialog-footer">
        <div className="update-dialog-actions">
          <button disabled={pending} onClick={() => void run("skip_update")}>
            Skip this Update
          </button>
          <button
            autoFocus
            className="primary"
            disabled={pending || dirty || !!status?.install_disabled}
            onClick={() => void run("install_update")}
          >
            <Download size={16} aria-hidden="true" />
            Update Now
          </button>
        </div>
        <small>Skip to stop reminders for this version.</small>
      </footer>
    </dialog>
  );
}
