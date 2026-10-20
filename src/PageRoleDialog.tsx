import { useEffect, useRef, useState } from "react";
import type { Config } from "./model";

export default function PageRoleDialog({
  config,
  target,
  close,
  confirm,
}: {
  config: Config;
  target?: string;
  close: () => void;
  confirm: (value: string) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [value, setValue] = useState(target ?? "");
  const page = config.pages.find((page) => page.id === value);
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    return () => element.close();
  }, []);
  return (
    <dialog
      ref={dialog}
      className="panel page-dialog"
      aria-labelledby="page-role-title"
      onCancel={close}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (value.trim() && page) confirm(value.trim());
        }}
      >
        <h2 id="page-role-title">Set Default page</h2>
        <p>
          The Default page appears whenever the active application has no
          matching page.{" "}
          {config.page_switching.default_page
            ? "Your other Default pages stay in their current order."
            : "Existing key and dial assignments stay unchanged."}
        </p>
        <label className="field">
          <span>Default page</span>
          <select
            autoFocus
            value={value}
            onChange={(event) => setValue(event.target.value)}
            required
          >
            <option value="">Choose a page…</option>
            {config.pages
              .filter((page) => !page.application)
              .map((page) => (
                <option key={page.id} value={page.id}>
                  {page.name}
                </option>
              ))}
          </select>
        </label>
        <p>Save changes to apply this role.</p>
        <div className="row">
          <button type="button" onClick={close}>
            Cancel
          </button>
          <button
            className="primary"
            type="submit"
            disabled={!value.trim() || !page}
          >
            Set as Default
          </button>
        </div>
      </form>
    </dialog>
  );
}
