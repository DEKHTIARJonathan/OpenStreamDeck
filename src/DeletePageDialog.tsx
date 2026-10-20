import { useEffect, useRef, useState } from "react";
import { Trash2 } from "lucide-react";
import type { Config } from "./model";
import { contextPages, pageReferenceCount } from "./page-editor";

export default function DeletePageDialog({
  config,
  pageId,
  close,
  confirm,
}: {
  config: Config;
  pageId: string;
  close: () => void;
  confirm: (replacementDefault?: string) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const page = config.pages.find((page) => page.id === pageId);
  const [replacement, setReplacement] = useState("");
  const isDefault = config.page_switching.default_page === pageId;
  const replacementPage = config.pages.find(
    (page) => page.id === replacement && !page.application,
  );
  const siblings = page ? contextPages(config, page) : [];
  const protectedPage =
    config.pages.length < 2 ||
    (!!page && !page.application && siblings.length < 2);
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
      {isDefault && !protectedPage && (
        <label className="field">
          <span>Replacement Default page</span>
          <select
            value={replacement}
            onChange={(event) => setReplacement(event.target.value)}
          >
            <option value="">Choose a replacement…</option>
            {config.pages
              .filter((page) => page.id !== pageId && !page.application)
              .map((page) => (
                <option key={page.id} value={page.id}>
                  {page.name}
                </option>
              ))}
          </select>
          <small>The Default page is used when no application matches.</small>
        </label>
      )}
      {page?.application && siblings.length === 1 && (
        <p>
          {page.application.name} will use the Default page after this deletion
          is saved.
        </p>
      )}
      {references > 0 && (
        <p className="error-text">
          {references} {references === 1 ? "action links" : "actions link"} to
          this page. Choose another target for{" "}
          {references === 1 ? "it" : "them"} after deleting.
        </p>
      )}
      {protectedPage && <p>Keep at least one Default page.</p>}
      <div className="row">
        <button autoFocus onClick={close}>
          Cancel
        </button>
        <button
          className="danger"
          disabled={!page || protectedPage || (isDefault && !replacementPage)}
          onClick={() => confirm(replacement || undefined)}
        >
          <Trash2 size={15} />
          Delete page
        </button>
      </div>
    </dialog>
  );
}
