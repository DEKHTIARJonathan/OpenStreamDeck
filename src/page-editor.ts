import type { Action, Config } from "./model";

export function removePage(config: Config, id: string): void {
  const index = config.pages.findIndex((page) => page.id === id);
  if (index < 0) throw new Error("This page no longer exists.");
  if (config.pages.length === 1) throw new Error("Keep at least one page.");
  config.pages.splice(index, 1);
  if (config.active_page === id) {
    config.active_page =
      config.pages[Math.min(index, config.pages.length - 1)].id;
  }
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
