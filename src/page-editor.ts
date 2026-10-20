import {
  applicationContextKey,
  type Action,
  type Config,
  type Page,
  type PageApplication,
} from "./model";

export function pageContextKey(page: Page): string {
  return page.application ? applicationContextKey(page.application) : "default";
}
export function contextPages(config: Config, page: Page): Page[] {
  const key = pageContextKey(page);
  return config.pages.filter((candidate) => pageContextKey(candidate) === key);
}
export function removeApplication(config: Config, context: string): string {
  if (context === "default") throw new Error("Default cannot be deleted.");
  const removed = config.pages.filter(
    (page) => pageContextKey(page) === context,
  );
  if (!removed.length) throw new Error("This application no longer exists.");
  const remaining = config.pages.filter(
    (page) => pageContextKey(page) !== context,
  );
  if (!remaining.length)
    throw new Error("Add a Default page before deleting this application.");
  const next =
    remaining.find((page) => page.id === config.page_switching.default_page) ??
    remaining.find((page) => !page.application) ??
    remaining[0];
  config.pages = remaining;
  if (removed.some((page) => page.id === config.active_page))
    config.active_page = next.id;
  return next.id;
}
export function pageAfterDeletion(config: Config, id: string): string | null {
  const page = config.pages.find((candidate) => candidate.id === id);
  if (!page) return null;
  const siblings = contextPages(config, page);
  const index = siblings.findIndex((candidate) => candidate.id === id);
  const remaining = siblings.filter((candidate) => candidate.id !== id);
  return (
    remaining[Math.min(index, remaining.length - 1)]?.id ??
    config.pages.find(
      (candidate) =>
        candidate.id !== id &&
        candidate.id === config.page_switching.default_page,
    )?.id ??
    config.pages.find(
      (candidate) => candidate.id !== id && !candidate.application,
    )?.id ??
    config.pages.find((candidate) => candidate.id !== id)?.id ??
    null
  );
}
export function removePage(
  config: Config,
  id: string,
  replacementDefault?: string,
): void {
  const index = config.pages.findIndex((page) => page.id === id);
  if (index < 0) throw new Error("This page no longer exists.");
  if (config.pages.length === 1) throw new Error("Keep at least one page.");
  const page = config.pages[index];
  if (!page.application && contextPages(config, page).length === 1)
    throw new Error("Keep at least one Default page.");
  if (config.page_switching.default_page === id) {
    if (
      !replacementDefault ||
      replacementDefault === id ||
      !config.pages.some(
        (page) => page.id === replacementDefault && !page.application,
      )
    )
      throw new Error("Choose a replacement Default page.");
    setDefaultPage(config, replacementDefault);
  }
  const next = pageAfterDeletion(config, id);
  config.pages.splice(index, 1);
  if (config.active_page === id && next) config.active_page = next;
}
export function reorderPage(
  config: Config,
  sourceId: string,
  targetId: string,
  position: "before" | "after",
): void {
  const source = config.pages.find((page) => page.id === sourceId);
  const target = config.pages.find((page) => page.id === targetId);
  if (!source || !target) throw new Error("This page no longer exists.");
  if (pageContextKey(source) !== pageContextKey(target))
    throw new Error("Pages can only be reordered within the same application.");
  if (sourceId === targetId) return;
  const siblings = contextPages(config, source).filter(
    (page) => page.id !== sourceId,
  );
  const targetIndex = siblings.findIndex((page) => page.id === targetId);
  siblings.splice(targetIndex + (position === "after" ? 1 : 0), 0, source);
  let index = 0;
  config.pages = config.pages.map((page) =>
    pageContextKey(page) === pageContextKey(source) ? siblings[index++] : page,
  );
}
export function movePage(config: Config, id: string, direction: -1 | 1): void {
  const page = config.pages.find((page) => page.id === id);
  if (!page) throw new Error("This page no longer exists.");
  const siblings = contextPages(config, page);
  const target =
    siblings[siblings.findIndex((page) => page.id === id) + direction];
  if (target)
    reorderPage(config, id, target.id, direction === -1 ? "before" : "after");
}
export function associateApplicationContext(
  config: Config,
  id: string,
  application: PageApplication | null,
): void {
  const page = config.pages.find((page) => page.id === id);
  if (!page) throw new Error("This page no longer exists.");
  const siblings = contextPages(config, page);
  if (
    application &&
    siblings.some((page) => page.id === config.page_switching.default_page)
  )
    throw new Error("Default pages cannot be associated with an application.");
  const outside = {
    ...config,
    pages: config.pages.filter(
      (candidate) => pageContextKey(candidate) !== pageContextKey(page),
    ),
  };
  if (application && applicationPage(outside, application))
    throw new Error("This application already has pages.");
  for (const sibling of siblings)
    sibling.application = application ? structuredClone(application) : null;
  if (!application && !config.page_switching.default_page)
    config.page_switching.default_page = siblings[0].id;
}

export function pageReferenceCount(config: Config, id: string): number {
  const count = (action: Action): number =>
    (action.kind === "page" && action.value.trim() === id ? 1 : 0) +
    action.children.reduce((n, child) => n + count(child), 0);
  return (
    config.pages
      .filter((page) => page.id !== id)
      .reduce(
        (n, page) =>
          n +
          Object.values(page.bindings).reduce(
            (sum, action) => sum + count(action),
            0,
          ),
        0,
      ) +
    config.workflows.reduce(
      (n, workflow) =>
        n + workflow.steps.reduce((sum, action) => sum + count(action), 0),
      0,
    )
  );
}

export function renamePage(config: Config, id: string, name: string): void {
  const page = config.pages.find((page) => page.id === id);
  if (!page) throw new Error("This page no longer exists.");
  const trimmed = name.trim();
  if (!trimmed) throw new Error("Enter a page name.");
  page.name = trimmed;
}

export function setDefaultPage(config: Config, id: string): void {
  const page = config.pages.find((page) => page.id === id);
  if (!page) throw new Error("This page no longer exists.");
  if (page.application) throw new Error("Choose a page from Default.");
  config.page_switching.default_page = id;
}

export function applicationPage(
  config: Config,
  application: PageApplication,
  except?: string,
) {
  return config.pages.find(
    (page) =>
      page.id !== except &&
      page.application?.identities.some((existing) =>
        application.identities.some((next) => {
          const normalize = (platform: string, value: string) =>
            platform === "windows"
              ? value.replaceAll("/", "\\").toLowerCase()
              : value;
          return (
            existing.platform === next.platform &&
            normalize(existing.platform, existing.value) ===
              normalize(next.platform, next.value)
          );
        }),
      ),
  );
}

export function associatePage(
  config: Config,
  id: string,
  application: PageApplication | null,
): void {
  const page = config.pages.find((page) => page.id === id);
  if (!page) throw new Error("This page no longer exists.");
  if (application && config.page_switching.default_page === id)
    throw new Error(
      "Choose another Default page before associating this page.",
    );
  if (application && applicationPage(config, application, id))
    throw new Error("This application already has a page.");
  page.application = application;
}

export function createApplicationPage(
  config: Config,
  options: {
    id: string;
    name: string;
    application: PageApplication;
    defaultPageId: string | null;
    newDefaultId: string;
    enabled: boolean;
  },
): void {
  const { id, application, enabled } = options;
  const name = options.name.trim();
  if (!name) throw new Error("Enter a page name.");
  if (!application.identities.length)
    throw new Error("This application has no supported foreground identity.");
  if (applicationPage(config, application))
    throw new Error("This application already has a page.");
  const fallbackId =
    config.page_switching.default_page || options.defaultPageId;
  const fallback = config.pages.find((page) => page.id === fallbackId);
  if (fallbackId && (!fallback || fallback.application))
    throw new Error("Choose an available Default page.");
  if (
    config.pages.some(
      (page) =>
        page.id === id || (!fallbackId && page.id === options.newDefaultId),
    ) ||
    (!fallbackId && id === options.newDefaultId)
  )
    throw new Error("This page already exists.");
  // Validate everything before changing the draft, including stale selections.
  if (!fallbackId)
    config.pages.push({
      id: options.newDefaultId,
      name: "Default",
      bindings: {},
    });
  config.pages.push({ id, name, application, bindings: {} });
  config.page_switching = {
    default_page: fallbackId || options.newDefaultId,
    enabled,
  };
}
