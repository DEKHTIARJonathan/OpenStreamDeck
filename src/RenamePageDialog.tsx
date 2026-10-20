import { useEffect, useRef, useState } from "react";
import type { Config } from "./model";

export default function RenamePageDialog({
  config,
  pageId,
  initialName,
  close,
  confirm,
}: {
  config: Config;
  pageId: string;
  initialName: string;
  close: () => void;
  confirm: (name: string) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const [name, setName] = useState(initialName);
  const exists = config.pages.some((page) => page.id === pageId);
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    input.current?.select();
    return () => element.close();
  }, []);
  return (
    <dialog
      ref={dialog}
      className="panel page-dialog"
      aria-labelledby="rename-page-title"
      onCancel={close}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (exists && name.trim()) confirm(name);
        }}
      >
        <h2 id="rename-page-title">Rename page</h2>
        <label className="field">
          <span>Page name</span>
          <input
            ref={input}
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
          />
        </label>
        <p>
          {exists
            ? "Save changes afterward to apply the new name."
            : "This page no longer exists."}
        </p>
        <div className="row">
          <button type="button" onClick={close}>
            Cancel
          </button>
          <button
            type="submit"
            className="rename-page"
            disabled={!exists || !name.trim()}
          >
            Rename page
          </button>
        </div>
      </form>
    </dialog>
  );
}
