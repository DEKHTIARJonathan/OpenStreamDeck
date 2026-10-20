import { useEffect, useRef } from "react";
import { Trash2 } from "lucide-react";
import type { Config } from "./model";
import { pageReferenceCount } from "./page-editor";

export default function DeletePageDialog({
  config,
  pageId,
  close,
  confirm,
}: {
  config: Config;
  pageId: string;
  close: () => void;
  confirm: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const page = config.pages.find((page) => page.id === pageId);
  const references = pageReferenceCount(config, pageId);
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    return () => element.close();
  }, []);
  return (
    <dialog
      ref={dialog}
      className="panel page-dialog"
      aria-labelledby="delete-page-title"
      aria-describedby="delete-page-description"
      onCancel={close}
    >
      <h2 id="delete-page-title">
        Delete page{page ? ` “${page.name}”` : ""}?
      </h2>
      <p id="delete-page-description">
        {page
          ? "This removes the page and all its key and dial assignments. Save changes afterward to apply the deletion."
          : "This page no longer exists."}
      </p>
      {references > 0 && (
        <p className="error-text">
          {references} {references === 1 ? "action links" : "actions link"} to
          this page. Choose another target for{" "}
          {references === 1 ? "it" : "them"} after deleting.
        </p>
      )}
      {config.pages.length === 1 && <p>Keep at least one page.</p>}
      <div className="row">
        <button autoFocus onClick={close}>
          Cancel
        </button>
        <button
          className="danger"
          disabled={!page || config.pages.length < 2}
          onClick={confirm}
        >
          <Trash2 size={15} />
          Delete page
        </button>
      </div>
    </dialog>
  );
}
