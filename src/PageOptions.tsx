import { useEffect, useRef, useState, useId } from "react";
import { Ellipsis } from "lucide-react";

export interface PageOption {
  label: string;
  run: () => void;
  disabled?: boolean;
  title?: string;
  separator?: boolean;
}

export default function PageOptions({
  items,
  label = "Page options",
}: {
  items: PageOption[];
  label?: string;
}) {
  const panelId = useId();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setOpen(false);
        trigger.current?.focus();
      }
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", keyboard);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", keyboard);
    };
  }, [open]);
  return (
    <div
      className="page-options"
      ref={root}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null))
          setOpen(false);
      }}
    >
      <button
        ref={trigger}
        aria-label={label}
        title={label}
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen(!open)}
      >
        <Ellipsis size={20} />
      </button>
      {open && (
        <div
          id={panelId}
          className="page-options-panel"
          role="group"
          aria-label={label}
        >
          {items.map((item) => (
            <button
              key={item.label}
              className={item.separator ? "page-option-separated" : ""}
              disabled={item.disabled}
              title={item.title}
              onClick={() => {
                setOpen(false);
                trigger.current?.focus();
                item.run();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
