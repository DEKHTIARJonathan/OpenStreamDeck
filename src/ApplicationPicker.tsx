import { useEffect, useRef, useState } from "react";
import { Search, X, FolderOpen, AppWindow } from "lucide-react";
import type { PageApplication } from "./model";
import { call, native } from "./api";
export interface Application {
  name: string;
  path: string;
}
export interface SelectedApplication extends Application {
  image: string;
}
export default function ApplicationPicker({
  choose,
  close,
  associate,
  create,
}: {
  choose?: (app: SelectedApplication) => void;
  associate?: (app: PageApplication) => void;
  close: () => void;
  create?: {
    defaultPages: { id: string; name: string }[];
    defaultPageId: string | null;
    suggestedDefaultId: string | null;
    submit: (
      application: PageApplication,
      name: string,
      defaultPageId: string | null,
      enabled: boolean,
    ) => void;
  };
}) {
  const callbacks = useRef({ choose, associate });
  callbacks.current = { choose, associate };
  const opener = useRef<HTMLElement | null>(
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    return () => element.close();
  }, []);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const dismiss = () => {
    alive.current = false;
    close();
    requestAnimationFrame(() => {
      if (!document.querySelector("dialog[open]")) opener.current?.focus();
    });
  };
  const [selectedApplication, setSelectedApplication] =
    useState<PageApplication | null>(null);
  const [pageName, setPageName] = useState("");
  const [defaultId, setDefaultId] = useState(
    create?.defaultPageId ?? create?.suggestedDefaultId ?? "",
  );
  const [enabled, setEnabled] = useState(true);
  const [apps, setApps] = useState<Application[]>([]),
    [query, setQuery] = useState(""),
    [loading, setLoading] = useState(true),
    [busy, setBusy] = useState(""),
    [error, setError] = useState("");
  const [resolvedSelection, setResolvedSelection] = useState<
    | { kind: "association"; application: PageApplication }
    | { kind: "launch"; application: SelectedApplication }
    | null
  >(null);
  useEffect(() => {
    if (!resolvedSelection || !alive.current) return;
    // Commit the async result after React has reconciled native configuration
    // events received during lookup. The handler must use that latest draft.
    setResolvedSelection(null);
    try {
      if (resolvedSelection.kind === "association")
        callbacks.current.associate?.(resolvedSelection.application);
      else callbacks.current.choose?.(resolvedSelection.application);
      dismiss();
    } catch (error) {
      setError(String(error));
      setBusy("");
    }
  }, [resolvedSelection]);
  useEffect(() => {
    let alive = true;
    call<Application[]>("list_applications")
      .then((apps) => {
        if (alive) setApps(apps);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, []);
  const select = async (app: Application) => {
    if (!alive.current) return;
    setBusy(app.path);
    setError("");
    try {
      if (associate || create) {
        const application = await call<PageApplication>(
          "application_identity",
          { path: app.path },
        );
        if (!alive.current) return;
        if (!application.identities.length)
          throw new Error(
            "This application has no supported foreground identity.",
          );
        // Identity is required; missing artwork may use a generic app glyph.
        const image = await call<string>("application_icon", {
          path: app.path,
        }).catch(() => null);
        if (alive.current) {
          if (create) {
            setSelectedApplication({ ...application, image });
            setPageName("Main");
            setBusy("");
          } else {
            setResolvedSelection({
              kind: "association",
              application: { ...application, image },
            });
          }
        }
        return;
      }
      const image = await call<string>("application_icon", { path: app.path });
      if (alive.current) {
        setResolvedSelection({
          kind: "launch",
          application: { ...app, image },
        });
      }
    } catch (e) {
      if (alive.current) {
        setError(String(e));
        setBusy("");
      }
    }
  };
  const browse = async () => {
    try {
      if (!native) throw new Error("Choose applications in the desktop app");
      const { open } = await import("@tauri-apps/plugin-dialog");
      const path = await open({
        multiple: false,
        title: "Choose an application",
      });
      if (typeof path === "string")
        await select({
          path,
          name:
            path
              .split(/[\\/]/)
              .pop()
              ?.replace(/\.(app|exe|desktop)$/i, "") || "Application",
        });
    } catch (e) {
      if (alive.current) setError(String(e));
    }
  };
  const filteredApps = apps.filter((app) =>
    app.name.toLowerCase().includes(query.toLowerCase()),
  );
  const effectiveDefaultId = create?.defaultPageId ?? defaultId;
  const fallbackName =
    create?.defaultPages.find((page) => page.id === effectiveDefaultId)?.name ??
    "New Default page";
  const submitPage = () => {
    if (!create || !selectedApplication || !pageName.trim() || busy) return;
    try {
      create.submit(
        selectedApplication,
        pageName.trim(),
        effectiveDefaultId || null,
        enabled,
      );
      dismiss();
    } catch (error) {
      setError(String(error));
    }
  };
  return (
    <dialog
      ref={dialog}
      className="app-picker panel"
      aria-label={create ? "Create application page" : "Choose application"}
      onCancel={dismiss}
    >
      <div className="panel-title">
        <span>
          {create ? "Create application page" : "Choose an application"}
        </span>
        <button aria-label="Close app picker" onClick={dismiss}>
          <X size={18} />
        </button>
      </div>
      {(!create || !selectedApplication) && (
        <p>
          {create
            ? "Choose an app for this page."
            : associate
              ? "Choose the application that will select this page."
              : "Select an app. Its icon will fill the key."}
        </p>
      )}
      {(!create || !selectedApplication) && (
        <>
          <label className="search">
            <Search size={16} />
            <input
              autoFocus
              aria-label="Search applications"
              placeholder="Search applications…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </label>
          <div className="application-list">
            {loading ? (
              <p>Finding applications…</p>
            ) : (
              filteredApps.map((a) => (
                <button
                  key={a.path}
                  disabled={!!busy}
                  onClick={() => select(a)}
                >
                  <AppWindow size={20} />
                  <div>
                    {a.name}
                    <small>
                      {busy === a.path
                        ? associate || create
                          ? "Checking application…"
                          : "Loading icon…"
                        : a.path}
                    </small>
                  </div>
                </button>
              ))
            )}
            {!loading && !filteredApps.length && !error && (
              <p>
                {query
                  ? `No applications match “${query}”. Try another name or browse below.`
                  : "Browse to an application on this computer."}
              </p>
            )}
          </div>
          <button onClick={browse} disabled={!!busy}>
            <FolderOpen size={16} />
            Browse for another app
          </button>
        </>
      )}
      {create && selectedApplication && (
        <form
          id="application-page-create"
          className="app-page-setup"
          onSubmit={(event) => {
            event.preventDefault();
            submitPage();
          }}
        >
          <div className="app-page-selection">
            {selectedApplication.image ? (
              <img src={selectedApplication.image} alt="" />
            ) : (
              <AppWindow size={28} />
            )}
            <div>
              <strong>{selectedApplication.name}</strong>
            </div>
            <button
              type="button"
              onClick={() => {
                setSelectedApplication(null);
                setError("");
              }}
            >
              Change application
            </button>
          </div>
          <label className="field">
            <span>Page name</span>
            <input
              autoFocus
              value={pageName}
              onChange={(event) => setPageName(event.target.value)}
              required
            />
          </label>
          {create.defaultPageId ? (
            <p className="app-page-fallback">
              Default fallback: <strong>{fallbackName}</strong>
            </p>
          ) : (
            <label className="field">
              <span>Default page</span>
              <select
                aria-label="Default page"
                value={defaultId}
                onChange={(event) => setDefaultId(event.target.value)}
              >
                {create.defaultPages.map((page) => (
                  <option key={page.id} value={page.id}>
                    {page.name}
                  </option>
                ))}
                <option value="">Create a new Default page</option>
              </select>
            </label>
          )}
          <label className="app-page-automatic">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(event) => setEnabled(event.target.checked)}
            />
            <span>Switch automatically</span>
          </label>
          <p className="app-page-routing">
            {enabled
              ? `Use ${pageName.trim() || selectedApplication.name} when ${selectedApplication.name} is active; use ${fallbackName} for apps without a page.`
              : "Select this page manually, or turn on automatic switching later."}
          </p>
        </form>
      )}
      {error && (
        <p className="error-text" role="alert">
          {error}
        </p>
      )}
      {create && (
        <div className="app-page-create-footer">
          <p>Add your actions, then save changes to apply.</p>
          <div className="row">
            <button onClick={dismiss}>Cancel</button>
            <button
              className="primary"
              disabled={!selectedApplication || !pageName.trim() || !!busy}
              type="submit"
              form="application-page-create"
            >
              Create page
            </button>
          </div>
        </div>
      )}
    </dialog>
  );
}
