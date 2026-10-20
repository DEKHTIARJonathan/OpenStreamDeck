import { useEffect, useRef, useState } from "react";
import { Search, X, FolderOpen, AppWindow } from "lucide-react";
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
}: {
  choose: (app: SelectedApplication) => void;
  close: () => void;
}) {
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
  };
  const [apps, setApps] = useState<Application[]>([]),
    [query, setQuery] = useState(""),
    [loading, setLoading] = useState(true),
    [busy, setBusy] = useState(""),
    [error, setError] = useState("");
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
    setBusy(app.path);
    setError("");
    try {
      const image = await call<string>("application_icon", { path: app.path });
      if (alive.current) {
        choose({ ...app, image });
        dismiss();
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
      setError(String(e));
    }
  };
  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) dismiss();
      }}
    >
      <section
        className="app-picker panel"
        role="dialog"
        aria-modal="true"
        aria-label="Choose application"
      >
        <div className="panel-title">
          <span>Choose an application</span>
          <button aria-label="Close app picker" onClick={dismiss}>
            <X size={18} />
          </button>
        </div>
        <p>Select an app. Its icon will fill the key.</p>
        <label className="search">
          <Search size={16} />
          <input
            autoFocus
            placeholder="Search applications…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <div className="application-list">
          {loading ? (
            <p>Finding applications…</p>
          ) : (
            apps
              .filter((a) => a.name.toLowerCase().includes(query.toLowerCase()))
              .map((a) => (
                <button
                  key={a.path}
                  disabled={!!busy}
                  onClick={() => select(a)}
                >
                  <AppWindow size={20} />
                  <div>
                    {a.name}
                    <small>{busy === a.path ? "Loading icon…" : a.path}</small>
                  </div>
                </button>
              ))
          )}
          {!loading && !apps.length && !error && (
            <p>Browse to an application on this computer.</p>
          )}
        </div>
        {error && (
          <p className="error-text" role="alert">
            {error}
          </p>
        )}
        <button onClick={browse} disabled={!!busy}>
          <FolderOpen size={16} />
          Browse for another app
        </button>
      </section>
    </div>
  );
}
