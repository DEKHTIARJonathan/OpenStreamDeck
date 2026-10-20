import { useEffect, useRef } from "react";

export default function MoveApplicationDialog({
  name,
  close,
  confirm,
}: {
  name: string;
  close: () => void;
  confirm: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    return () => element.close();
  }, []);
  return (
    <dialog
      ref={dialog}
      className="panel page-dialog"
      aria-labelledby="move-application-title"
      onCancel={close}
    >
      <h2 id="move-application-title">Move pages to Default?</h2>
      <p>
        All pages for {name} will move to Default, keeping their actions and
        order. {name} will use your Default page when automatic switching is on.
      </p>
      <p>Save changes to apply.</p>
      <div className="row">
        <button autoFocus onClick={close}>
          Cancel
        </button>
        <button className="primary" onClick={confirm}>
          Move pages
        </button>
      </div>
    </dialog>
  );
}
