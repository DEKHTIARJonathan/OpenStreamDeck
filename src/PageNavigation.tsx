import {
  AppWindow,
  ChevronLeft,
  ChevronRight,
  Home,
  Plus,
  Pencil,
} from "lucide-react";
import {
  useEffect,
  useRef,
  useState,
  type DragEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import type { Config, Device, Page, PageContext } from "./model";
import { pageContextKey, contextPages } from "./page-editor";
import { native } from "./api";

function ScrollRail({
  children,
  revealKey,
  selectedId,
  kind,
  onDragOver,
}: {
  children: ReactNode;
  revealKey: string;
  selectedId: string;
  kind: "pages" | "applications";
  onDragOver?: (event: DragEvent<HTMLDivElement>) => void;
}) {
  const rail = useRef<HTMLDivElement>(null);
  const slot = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const [overflow, setOverflow] = useState({
    needed: false,
    left: false,
    right: false,
  });
  useEffect(() => {
    const element = rail.current!;
    const measure = () => {
      const needed =
        (content.current?.scrollWidth ?? 0) >
        (slot.current?.clientWidth ?? 0) + 1;
      const next = {
        needed,
        left: needed && element.scrollLeft > 1,
        right:
          needed &&
          element.scrollWidth - element.clientWidth - element.scrollLeft > 1,
      };
      setOverflow((previous) =>
        previous.needed === next.needed &&
        previous.left === next.left &&
        previous.right === next.right
          ? previous
          : next,
      );
    };
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    if (slot.current) observer.observe(slot.current);
    if (content.current) observer.observe(content.current);
    element.addEventListener("scroll", measure, { passive: true });
    measure();
    return () => {
      observer.disconnect();
      element.removeEventListener("scroll", measure);
    };
  }, []);
  useEffect(() => {
    const element = rail.current;
    const selected = document.getElementById(selectedId);
    if (!element || !selected || !element.contains(selected)) return;
    const viewport = element.getBoundingClientRect(),
      item = selected.getBoundingClientRect();
    const delta =
      item.left < viewport.left
        ? item.left - viewport.left
        : item.right > viewport.right
          ? item.right - viewport.right
          : 0;
    if (delta) element.scrollBy({ left: delta, behavior: "instant" });
  }, [revealKey, selectedId, overflow.needed]);
  const scroll = (direction: number) => {
    const element = rail.current;
    if (element)
      element.scrollBy({
        left: direction * Math.max(160, element.clientWidth * 0.7),
        behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches
          ? "instant"
          : "smooth",
      });
  };
  return (
    <div className="context-tab-rail" ref={slot} onDragOver={onDragOver}>
      {overflow.needed && (
        <button
          className="context-scroll"
          type="button"
          disabled={!overflow.left}
          aria-label={`Show previous ${kind}`}
          onClick={() => scroll(-1)}
        >
          <ChevronLeft size={16} />
        </button>
      )}
      <div
        className="context-tab-scroll"
        ref={rail}
        data-fade-left={overflow.left}
        data-fade-right={overflow.right}
      >
        <div className="context-tab-content" ref={content}>
          {children}
        </div>
      </div>
      {overflow.needed && (
        <button
          className="context-scroll"
          type="button"
          disabled={!overflow.right}
          aria-label={`Show more ${kind}`}
          onClick={() => scroll(1)}
        >
          <ChevronRight size={16} />
        </button>
      )}
    </div>
  );
}

const pageDragType = "application/x-openstreamdeck-page";
const applicationButtonId = (key: string) =>
  `application-context-${encodeURIComponent(key)}`;
export default function PageNavigation({
  config,
  savedConfig,
  page,
  device,
  context,
  edit,
  selectContext,
  addApplication,
  addPage,
  addDefault,
  renamePage,
  contextOptions,
  removePage,
  removeApplication,
  reorder,
  move,
  toggle,
  resume,
}: {
  config: Config;
  savedConfig: Config;
  page: Page;
  device: Device;
  context: PageContext | null;
  edit: (id: string) => void;
  selectContext: (key: string) => void;
  addApplication: () => void;
  addPage: () => void;
  addDefault?: () => void;
  renamePage: () => void;
  contextOptions?: ReactNode;
  removePage?: ReactNode;
  removeApplication?: ReactNode;
  reorder: (
    source: string,
    target: string,
    position: "before" | "after",
  ) => void;
  move: (id: string, direction: -1 | 1) => void;
  toggle: (enabled: boolean) => void;
  resume: () => void;
}) {
  const defaultPage = config.pages.find(
    (item) => item.id === config.page_switching.default_page,
  );
  const defaults = config.pages.filter((item) => !item.application);
  const selectedContext = pageContextKey(page);
  const orderedPages = contextPages(config, page);
  const applicationByContext = new Map<
    string,
    NonNullable<Page["application"]>
  >();
  for (const item of config.pages) {
    if (item.application && !applicationByContext.has(pageContextKey(item)))
      applicationByContext.set(pageContextKey(item), item.application);
  }
  // Application order stays independent of child-page ordering and deletion.
  const applications = [...applicationByContext.entries()].sort(
    ([leftKey, left], [rightKey, right]) =>
      left.name.localeCompare(right.name) || leftKey.localeCompare(rightKey),
  );
  const name = (id: string | null | undefined) => {
    const saved = savedConfig.pages.find((item) => item.id === id);
    return saved
      ? `${saved.application?.name ?? "Default"} / ${saved.name}`
      : "Unknown page";
  };
  const buttons = useRef(new Map<string, HTMLButtonElement>());
  const dragging = useRef<string | null>(null);
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [drop, setDrop] = useState<{
    id: string;
    position: "before" | "after";
  } | null>(null);
  const [orderNotice, setOrderNotice] = useState("");
  const endDrag = () => {
    dragging.current = null;
    setDraggedId(null);
    setDrop(null);
  };
  const sourceFor = (event: DragEvent) => {
    const id = dragging.current || event.dataTransfer.getData(pageDragType);
    const source = config.pages.find((item) => item.id === id);
    return source && pageContextKey(source) === selectedContext
      ? source.id
      : null;
  };
  const dropAt = (
    event: DragEvent<HTMLDivElement>,
  ): { id: string; position: "before" | "after" } | null => {
    // The entire tab row accepts insertion, including the leading space and
    // gaps between tabs. Keep the management buttons outside the drop area.
    if (
      (event.target as Element).closest(".context-page-actions") ||
      !sourceFor(event)
    )
      return null;
    const viewport = event.currentTarget
      .querySelector(".context-tab-scroll")
      ?.getBoundingClientRect();
    if (viewport && event.clientX < viewport.left)
      return { id: orderedPages[0].id, position: "before" };
    if (viewport && event.clientX > viewport.right)
      return {
        id: orderedPages[orderedPages.length - 1].id,
        position: "after",
      };
    for (const item of orderedPages) {
      const bounds = buttons.current.get(item.id)?.getBoundingClientRect();
      if (bounds && event.clientX < bounds.left + bounds.width / 2)
        return { id: item.id, position: "before" };
    }
    const last = orderedPages[orderedPages.length - 1];
    return last ? { id: last.id, position: "after" } : null;
  };
  const keyboard = (event: KeyboardEvent<HTMLButtonElement>, id: string) => {
    const index = orderedPages.findIndex((item) => item.id === id);
    if (
      event.altKey &&
      event.shiftKey &&
      (event.key === "ArrowLeft" || event.key === "ArrowRight")
    ) {
      event.preventDefault();
      const direction = event.key === "ArrowLeft" ? -1 : 1;
      if (index + direction >= 0 && index + direction < orderedPages.length) {
        move(id, direction);
        setOrderNotice(
          `${orderedPages[index].name} moved ${direction < 0 ? "left" : "right"}. Save changes to apply the order.`,
        );
      }
      return;
    }
    let next: number;
    if (event.key === "ArrowRight") next = (index + 1) % orderedPages.length;
    else if (event.key === "ArrowLeft")
      next = (index + orderedPages.length - 1) % orderedPages.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = orderedPages.length - 1;
    else return;
    event.preventDefault();
    edit(orderedPages[next].id);
    buttons.current.get(orderedPages[next].id)?.focus({ preventScroll: true });
  };
  const renderPage = (item: Page) => (
    <button
      key={item.id}
      ref={(element) => {
        if (element) buttons.current.set(item.id, element);
        else buttons.current.delete(item.id);
      }}
      type="button"
      role="tab"
      id={`page-tab-${item.id}`}
      aria-label={item.name}
      aria-selected={item.id === page.id}
      aria-controls="page-editor"
      aria-describedby={`page-tab-description-${item.id}`}
      aria-keyshortcuts="Alt+Shift+ArrowLeft Alt+Shift+ArrowRight"
      tabIndex={item.id === page.id ? 0 : -1}
      draggable
      className={`context-page-tab${draggedId === item.id ? " page-is-dragging" : ""}${drop?.id === item.id ? ` page-drop-${drop.position}` : ""}`}
      title={`${item.name} · Drag to reorder`}
      onClick={() => edit(item.id)}
      onKeyDown={(event) => keyboard(event, item.id)}
      onDragStart={(event) => {
        dragging.current = item.id;
        setDraggedId(item.id);
        event.dataTransfer.setData(pageDragType, item.id);
        event.dataTransfer.effectAllowed = "move";
      }}
      onDragEnd={endDrag}
    >
      {item.id === config.page_switching.default_page && (
        <Home size={13} aria-hidden="true" />
      )}
      <span>{item.name}</span>
      <span className="sr-only" id={`page-tab-description-${item.id}`}>
        {item.id === config.page_switching.default_page
          ? "Default fallback page. "
          : ""}
        Drag to reorder within {page.application?.name ?? "Default"};
        Alt+Shift+Arrow keys move this page.
      </span>
    </button>
  );
  const overlayApplied = context?.overlay_applied ?? context?.overlay ?? false;
  const updating =
    context?.overlay && !overlayApplied
      ? "Agents overlay"
      : context &&
          !context.overlay &&
          (overlayApplied || context.desired_page !== context.applied_page)
        ? name(context.desired_page)
        : null;
  let live = "Disconnected";
  if (device.connected) {
    live = device.sleeping
      ? `Sleeping — next page: ${name(context?.desired_page)}`
      : overlayApplied
        ? "Agents overlay"
        : context?.applied_page
          ? name(context.applied_page)
          : "Waiting for display confirmation";
  }
  const reason = context?.override_active
    ? "Manual · Until application changes"
    : context && !context.automatic
      ? "Manual · Automatic switching off"
      : context?.error
        ? "Default · Application detection unavailable"
        : context?.reason === "application"
          ? `Automatic · ${context.application?.name ?? "Application match"}`
          : context?.reason === "default"
            ? "Default · No matching application"
            : "Manual";
  const switchingUnsaved =
    native && context && context.automatic !== config.page_switching.enabled;
  return (
    <section className="page-navigation" aria-label="Page navigation">
      <div className="application-context-row">
        <h2 className="context-row-label">Applications</h2>
        <div
          className="application-contexts"
          role="group"
          aria-label="Application contexts"
        >
          <button
            type="button"
            className="application-context-choice application-default-context"
            aria-label={defaults.length ? "Default" : "Add Default pages"}
            aria-pressed={selectedContext === "default"}
            disabled={!defaults.length && !addDefault}
            onClick={() => {
              endDrag();
              if (defaults.length) selectContext("default");
              else addDefault?.();
            }}
          >
            <Home size={18} />
            <span>{defaults.length ? "Default" : "Add Default pages"}</span>
          </button>
          <ScrollRail
            kind="applications"
            selectedId={applicationButtonId(selectedContext)}
            revealKey={selectedContext}
          >
            {applications.map(([key, application]) => (
              <button
                key={key}
                id={applicationButtonId(key)}
                type="button"
                className="application-context-choice"
                aria-label={application.name}
                aria-pressed={selectedContext === key}
                onClick={() => {
                  endDrag();
                  selectContext(key);
                }}
              >
                {application.image ? (
                  <img src={application.image} alt="" />
                ) : (
                  <AppWindow size={18} />
                )}
                <span>{application.name}</span>
              </button>
            ))}
          </ScrollRail>
        </div>
        <div className="context-page-actions">
          <button className="context-add-page" onClick={addApplication}>
            <Plus size={16} />
            Add application
          </button>
          {removeApplication}
          {contextOptions}
        </div>
      </div>
      <div
        className="page-navigation-toolbar"
        onDragOver={(event) => {
          const target = dropAt(event);
          if (!target) {
            setDrop(null);
            return;
          }
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          setDrop(target.id === dragging.current ? null : target);
        }}
        onDragLeave={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null))
            setDrop(null);
        }}
        onDrop={(event) => {
          const source = sourceFor(event);
          const target = dropAt(event);
          if (!source || !target) return;
          event.preventDefault();
          event.stopPropagation();
          if (source !== target.id) {
            reorder(source, target.id, target.position);
            setOrderNotice("Page order changed. Save changes to apply.");
          }
          endDrag();
        }}
      >
        <h2 className="context-row-label">Pages</h2>
        <div
          className="context-page-tabs"
          role="tablist"
          aria-label="Pages"
          aria-orientation="horizontal"
        >
          <ScrollRail
            kind="pages"
            selectedId={`page-tab-${page.id}`}
            revealKey={`${page.id}:${orderedPages.map((item) => item.id).join("|")}`}
            onDragOver={(event) => {
              if (!dragging.current || !sourceFor(event)) return;
              const rail = event.currentTarget.querySelector(
                ".context-tab-scroll",
              );
              if (!rail) return;
              const bounds = rail.getBoundingClientRect();
              if (event.clientX < bounds.left + 24)
                rail.scrollBy({
                  left: -24,
                  behavior: "instant",
                });
              else if (event.clientX > bounds.right - 24)
                rail.scrollBy({ left: 24, behavior: "instant" });
            }}
          >
            {orderedPages.map(renderPage)}
          </ScrollRail>
        </div>
        <div className="context-page-actions">
          <button className="context-add-child" onClick={addPage}>
            <Plus size={15} />
            Add page
          </button>
          <button className="context-add-child" onClick={renamePage}>
            <Pencil size={15} />
            Rename page
          </button>
          {removePage}
        </div>
      </div>
      <span className="sr-only" role="status">
        {orderNotice}
      </span>
      <div className="page-runtime">
        <div className="page-live" role="status" aria-live="polite">
          <strong>On device: {live}</strong>
          {context &&
            device.connected &&
            !device.sleeping &&
            !overlayApplied && <span> · {reason}</span>}
          {device.connected && !device.sleeping && updating && (
            <span> · Updating to {updating}</span>
          )}
          {context?.override_active && (
            <button className="page-resume" onClick={resume}>
              Resume automatic
            </button>
          )}
        </div>
        <label
          className="switching-toggle"
          title={
            !defaultPage
              ? "Set a Default fallback page before enabling automatic switching."
              : undefined
          }
        >
          <input
            aria-label="Automatic page switching"
            type="checkbox"
            checked={config.page_switching.enabled}
            disabled={!defaultPage}
            onChange={(event) => toggle(event.target.checked)}
          />
          <span className="automatic-switch-track" aria-hidden="true">
            <span />
          </span>
          <span>
            Automatic{" "}
            <strong>{config.page_switching.enabled ? "On" : "Off"}</strong>
          </span>
          {switchingUnsaved && <small>Save to apply</small>}
        </label>
      </div>
      {(!native || context?.error) && (
        <p className="page-capability">
          {context?.error ||
            "Browser preview · Application detection requires the desktop app."}
        </p>
      )}
    </section>
  );
}
