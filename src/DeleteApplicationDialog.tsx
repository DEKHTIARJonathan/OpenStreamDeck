import { useEffect, useRef } from "react";
import { Trash2 } from "lucide-react";
import type { Config } from "./model";
import { pageContextKey, pageReferenceCount } from "./page-editor";

export default function DeleteApplicationDialog({
  config,
  target,
  close,
  confirm,
}: {
  config: Config;
  target: { key: string; name: string };
  close: () => void;
  confirm: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const pages = config.pages.filter(
    (page) => pageContextKey(page) === target.key,
  );
  const remaining = config.pages.filter(
    (page) => pageContextKey(page) !== target.key,
  );
  const references = pages.reduce(
    (count, page) =>
      count + pageReferenceCount({ ...config, pages: remaining }, page.id),
    0,
  );
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    return () => element.close();
  }, []);
  const dismiss = () => {
    dialog.current?.close();
    close();
  };
  return (
    <dialog
      ref={dialog}
      className="panel page-dialog"
      aria-labelledby="delete-application-title"
      aria-describedby="delete-application-description"
      onCancel={(event) => {
        event.preventDefault();
        dismiss();
      }}
    >
      <h2 id="delete-application-title">Delete application “{target.name}”?</h2>
      <p id="delete-application-description">
        This deletes all {pages.length} {pages.length === 1 ? "page" : "pages"}{" "}
        for {target.name}, including their key and dial assignments. Save
        changes afterward to apply the deletion.
      </p>
      {!!pages.length && (
        <ul className="application-delete-pages">
          {pages.map((page) => (
            <li key={page.id}>{page.name}</li>
          ))}
        </ul>
      )}
      {!pages.length && (
        <p className="error-text">This application no longer exists.</p>
      )}
      {!remaining.length && (
        <p className="error-text">
          Add a Default page before deleting this application.
        </p>
      )}
      {references > 0 && (
        <p className="error-text">
          {references} {references === 1 ? "action links" : "actions link"} to
          these pages. Choose new targets for those links after deleting.
        </p>
      )}
      <div className="row">
        <button autoFocus onClick={dismiss}>
          Cancel
        </button>
        <button
          className="danger"
          disabled={
            target.key === "default" || !pages.length || !remaining.length
          }
          onClick={confirm}
        >
          <Trash2 size={15} /> Delete application
        </button>
      </div>
    </dialog>
  );
}
