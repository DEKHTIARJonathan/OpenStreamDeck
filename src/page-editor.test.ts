import { describe, expect, it } from "vitest";
import { action, defaultConfig } from "./model";
import {
  removePage,
  removeApplication,
  renamePage,
  pageReferenceCount,
  associatePage,
  setDefaultPage,
  applicationPage,
  createApplicationPage,
  associateApplicationContext,
  contextPages,
  pageAfterDeletion,
  pageContextKey,
  reorderPage,
  movePage,
} from "./page-editor";
import { editConfig, receiveConfig } from "./config-draft";

function config() {
  const w = defaultConfig();
  w.pages.push(
    { id: "second", name: "Second", bindings: {} },
    { id: "third", name: "Third", bindings: {} },
  );
  return w;
}

describe("page deletion", () => {
  it.each([
    ["home", "second"],
    ["second", "third"],
    ["third", "second"],
  ])("deleting active %s selects %s", (id, next) => {
    const w = config();
    w.active_page = id;
    const preserved = structuredClone(w.pages.filter((p) => p.id !== id));
    removePage(w, id);
    expect(w.active_page).toBe(next);
    expect(w.pages).toEqual(preserved);
  });
  it("preserves the selected page when deleting another page", () => {
    const w = config();
    removePage(w, "second");
    expect(w.active_page).toBe("home");
  });
  it("rejects the final page and missing targets without changing state", () => {
    const w = defaultConfig(),
      before = structuredClone(w);
    expect(() => removePage(w, "home")).toThrow("Keep at least one page");
    expect(() => removePage(w, "missing")).toThrow("no longer exists");
    expect(w).toEqual(before);
  });
  it("reports retained links without rewriting or deleting their actions", () => {
    const w = config();
    w.pages[0].bindings.lcd1 = {
      ...action("sequence"),
      children: [action("page", "Jump", " second ")],
    };
    w.pages[1].bindings.lcd1 = action("page", "Self", "second");
    w.workflows[0].steps.push(
      action("page", "Jump", "second"),
      action("page", "Next", "next"),
    );
    expect(pageReferenceCount(w, "second")).toBe(2);
    const workflows = structuredClone(w.workflows),
      bindings = structuredClone(w.pages[0].bindings);
    removePage(w, "second");
    expect(w.workflows).toEqual(workflows);
    expect(w.pages[0].bindings).toEqual(bindings);
  });
  it("preserves a pending deletion when hardware reports the deleted page", () => {
    const w = config();
    w.active_page = "second";
    const local = editConfig(receiveConfig(null, w), (draft) =>
      removePage(draft, "second"),
    );
    const next = receiveConfig(local, { ...w, brightness: 30 });
    expect(next.draft.active_page).toBe("third");
    expect(next.draft.pages.some((p) => p.id === "second")).toBe(false);
    expect(next.draft.brightness).toBe(30);
  });
});

describe("page renaming", () => {
  it("renames the captured page while preserving IDs, bindings, selection and links", () => {
    const w = config();
    w.active_page = "second";
    w.workflows[0].steps.push(action("page", "Jump", "home"));
    const before = structuredClone(w);
    renamePage(w, "home", "  My page  ");
    before.pages[0].name = "My page";
    expect(w).toEqual(before);
  });
  it("rejects blank names and stale targets without changing the config", () => {
    const w = config(),
      before = structuredClone(w);
    expect(() => renamePage(w, "home", "   ")).toThrow("Enter a page name");
    expect(() => renamePage(w, "missing", "New name")).toThrow(
      "no longer exists",
    );
    expect(w).toEqual(before);
  });
});

describe("application page roles", () => {
  const chrome = {
    name: "Google Chrome",
    identities: [{ platform: "macos" as const, value: "com.google.Chrome" }],
  };
  it("associates and removes an app without changing content or the manual selection", () => {
    const w = config();
    const before = structuredClone(w.pages[1]);
    associatePage(w, "second", chrome);
    expect(w.pages[1]).toEqual({ ...before, application: chrome });
    expect(w.active_page).toBe("home");
    associatePage(w, "second", null);
    expect(w.pages[1]).toEqual({ ...before, application: null });
  });
  it("changes the fallback only among Default pages without stripping applications", () => {
    const w = config();
    w.page_switching = { enabled: true, default_page: "home" };
    associatePage(w, "second", chrome);
    const before = structuredClone(w);
    expect(() => setDefaultPage(w, "second")).toThrow(
      "Choose a page from Default",
    );
    expect(w).toEqual(before);
    setDefaultPage(w, "third");
    expect(w.page_switching.default_page).toBe("third");
    expect(w.pages).toEqual(before.pages);
    expect(w.active_page).toBe("home");
  });
  it("requires a concrete valid replacement before deleting Default", () => {
    const w = config();
    setDefaultPage(w, "home");
    const before = structuredClone(w);
    for (const id of [undefined, "home", "missing"])
      expect(() => removePage(w, "home", id)).toThrow("replacement Default");
    expect(w).toEqual(before);
    associatePage(w, "second", chrome);
    expect(() => removePage(w, "home", "second")).toThrow(
      "replacement Default",
    );
    removePage(w, "home", "third");
    expect(w.page_switching.default_page).toBe("third");
    expect(w.pages[0].application).toEqual(chrome);
    expect(w.active_page).toBe("third");
  });
  it("does not steal duplicates, including case-insensitive Windows paths", () => {
    const w = config();
    associatePage(w, "second", chrome);
    const before = structuredClone(w);
    expect(applicationPage(w, chrome)?.id).toBe("second");
    expect(() => associatePage(w, "third", chrome)).toThrow(
      "already has a page",
    );
    expect(w).toEqual(before);
    associatePage(w, "second", {
      name: "Editor",
      identities: [{ platform: "windows", value: "C:\\Tools\\Code.exe" }],
    });
    expect(
      applicationPage(w, {
        name: "Other label",
        identities: [{ platform: "windows", value: "c:/tools/code.EXE" }],
      })?.id,
    ).toBe("second");
    expect(() => associatePage(w, "missing", chrome)).toThrow(
      "no longer exists",
    );
  });
  it("creates an application page, fallback and automatic preference together without changing existing actions", () => {
    const w = config();
    const before = structuredClone(w);
    createApplicationPage(w, {
      id: "chrome",
      name: " Browser ",
      application: chrome,
      defaultPageId: "home",
      newDefaultId: "fallback",
      enabled: true,
    });
    expect(w.pages.slice(0, 3)).toEqual(before.pages);
    expect(w.pages[3]).toEqual({
      id: "chrome",
      name: "Browser",
      application: chrome,
      bindings: {},
    });
    expect(w.page_switching).toEqual({ enabled: true, default_page: "home" });
    expect(w.active_page).toBe(before.active_page);
    expect(w.workflows).toEqual(before.workflows);
  });
  it("preserves an existing Default and respects choosing automatic switching off", () => {
    const w = config();
    setDefaultPage(w, "third");
    createApplicationPage(w, {
      id: "chrome",
      name: "Browser",
      application: chrome,
      defaultPageId: "home",
      newDefaultId: "fallback",
      enabled: false,
    });
    expect(w.page_switching).toEqual({ enabled: false, default_page: "third" });
    expect(w.pages).toHaveLength(4);
  });
  it("creates a new empty Default without removing existing application associations", () => {
    const w = defaultConfig();
    const existingApp = {
      name: "Editor",
      identities: [{ platform: "macos" as const, value: "com.example.Editor" }],
    };
    associatePage(w, "home", existingApp);
    const before = structuredClone(w.pages[0]);
    createApplicationPage(w, {
      id: "chrome",
      name: "Browser",
      application: chrome,
      defaultPageId: null,
      newDefaultId: "fallback",
      enabled: true,
    });
    expect(w.pages[0]).toEqual(before);
    expect(w.pages[1]).toEqual({
      id: "fallback",
      name: "Default",
      bindings: {},
    });
    expect(w.page_switching).toEqual({
      enabled: true,
      default_page: "fallback",
    });
  });
  it("rejects stale fallback, duplicate apps, empty names and invalid identities before mutating", () => {
    const w = config();
    associatePage(w, "second", chrome);
    const before = structuredClone(w);
    const options = {
      id: "new",
      name: "Editor",
      application: {
        name: "Editor",
        identities: [
          { platform: "macos" as const, value: "com.example.Editor" },
        ],
      },
      defaultPageId: "home",
      newDefaultId: "fallback",
      enabled: true,
    };
    for (const patch of [
      { defaultPageId: "missing" },
      { defaultPageId: "second" },
      { application: chrome },
      { name: " " },
      { application: { name: "Unknown", identities: [] } },
      { id: "third" },
      { defaultPageId: null, newDefaultId: "new" },
    ]) {
      expect(() =>
        createApplicationPage(w, { ...options, ...patch }),
      ).toThrow();
      expect(w).toEqual(before);
    }
  });
});

describe("application contexts and ordering", () => {
  const app = {
    name: "Chrome",
    identities: [
      { platform: "macos" as const, value: "com.google.Chrome" },
      { platform: "windows" as const, value: "C:/Apps/Chrome.EXE" },
    ],
  };
  function grouped() {
    const w = config();
    w.page_switching = { enabled: true, default_page: "home" };
    w.pages.splice(
      1,
      0,
      {
        id: "chrome1",
        name: "Main",
        application: structuredClone(app),
        bindings: { lcd1: action("page", "Jump", "third") },
      },
      {
        id: "chrome2",
        name: "Tools",
        application: structuredClone(app),
        bindings: {},
      },
    );
    w.pages.push({
      id: "chrome3",
      name: "Media",
      application: structuredClone(app),
      bindings: {},
    });
    return w;
  }
  it("groups exact identities independent of order, Windows spelling, name and icon", () => {
    const w = grouped();
    const application = w.pages[2].application!;
    application.identities.reverse();
    application.identities[0].value = "c:\\apps\\chrome.exe";
    application.name = "Different label";
    expect(pageContextKey(w.pages[1])).toBe(pageContextKey(w.pages[2]));
    expect(contextPages(w, w.pages[1]).map((page) => page.id)).toEqual([
      "chrome1",
      "chrome2",
      "chrome3",
    ]);
  });
  it("reorders only sibling slots and preserves every binding, active selection and fallback", () => {
    const w = grouped(),
      before = structuredClone(w);
    reorderPage(w, "chrome3", "chrome1", "before");
    expect(w.pages.map((page) => page.id)).toEqual([
      "home",
      "chrome3",
      "chrome1",
      "second",
      "third",
      "chrome2",
    ]);
    expect(w.pages.filter((page) => !page.application)).toEqual(
      before.pages.filter((page) => !page.application),
    );
    expect([...w.pages].sort((a, b) => a.id.localeCompare(b.id))).toEqual(
      [...before.pages].sort((a, b) => a.id.localeCompare(b.id)),
    );
    expect(w.active_page).toBe(before.active_page);
    expect(w.page_switching).toEqual(before.page_switching);
    movePage(w, "chrome3", 1);
    expect(contextPages(w, w.pages[1]).map((page) => page.id)).toEqual([
      "chrome1",
      "chrome3",
      "chrome2",
    ]);
    reorderPage(w, "home", "third", "after");
    expect(
      w.pages.filter((page) => !page.application).map((page) => page.id),
    ).toEqual(["second", "third", "home"]);
    expect(w.page_switching.default_page).toBe("home");
  });
  it("rejects stale and cross-context moves atomically; edges and self-drops are no-ops", () => {
    const w = grouped(),
      before = structuredClone(w);
    expect(() => reorderPage(w, "chrome1", "home", "before")).toThrow(
      "same application",
    );
    expect(() => reorderPage(w, "missing", "home", "after")).toThrow(
      "no longer exists",
    );
    expect(() => movePage(w, "missing", 1)).toThrow("no longer exists");
    reorderPage(w, "chrome1", "chrome1", "after");
    movePage(w, "chrome1", -1);
    movePage(w, "chrome3", 1);
    expect(w).toEqual(before);
  });
  it("preserves reordered draft through native updates and a save acknowledgement", () => {
    const w = grouped();
    const pending = editConfig(receiveConfig(null, w), (draft) =>
      reorderPage(draft, "chrome3", "chrome1", "before"),
    );
    const incoming = { ...w, active_page: "chrome2", brightness: 30 };
    const reconciled = receiveConfig(pending, incoming);
    expect(reconciled.draft.pages).toEqual(pending.draft.pages);
    expect(reconciled.draft.active_page).toBe("chrome2");
    expect(reconciled.draft.brightness).toBe(30);
    expect(
      receiveConfig(reconciled, structuredClone(reconciled.draft)).draft,
    ).toEqual(reconciled.draft);
  });
  it("changes all application siblings atomically and rejects collisions outside that context", () => {
    const w = grouped();
    const next = {
      name: "Code",
      identities: [
        { platform: "macos" as const, value: "com.microsoft.VSCode" },
      ],
    };
    const preserved = w.pages.map(({ application: _, ...page }) => page);
    associateApplicationContext(w, "chrome2", next);
    expect(contextPages(w, w.pages[1])).toHaveLength(3);
    expect(
      w.pages
        .filter((page) => page.application)
        .every((page) => page.application?.name === "Code"),
    ).toBe(true);
    expect(w.pages.map(({ application: _, ...page }) => page)).toEqual(
      preserved,
    );
    w.pages.push({
      id: "other",
      name: "Other",
      bindings: {},
      application: app,
    });
    const before = structuredClone(w);
    expect(() => associateApplicationContext(w, "chrome2", app)).toThrow(
      "already has pages",
    );
    expect(() => associateApplicationContext(w, "home", next)).toThrow(
      "Default pages",
    );
    expect(() => associateApplicationContext(w, "missing", next)).toThrow(
      "no longer exists",
    );
    expect(w).toEqual(before);
    associateApplicationContext(w, "chrome2", null);
    expect(
      w.pages.filter((page) => page.application).map((page) => page.id),
    ).toEqual(["other"]);
    expect(w.page_switching.default_page).toBe("home");
  });
  it("cascade deletes only the named context and selects Default when its active page is removed", () => {
    const w = grouped();
    w.active_page = "chrome2";
    w.pages[0].bindings.lcd1 = action("page", "Chrome link", "chrome1");
    w.workflows[0].steps.push(action("page", "Chrome child link", "chrome3"));
    const before = structuredClone(w);
    expect(removeApplication(w, pageContextKey(w.pages[1]))).toBe("home");
    expect(w.pages).toEqual(before.pages.filter((page) => !page.application));
    expect(w.workflows).toEqual(before.workflows);
    expect(w.page_switching).toEqual(before.page_switching);
    expect(w.active_page).toBe("home");
    expect(JSON.parse(JSON.stringify(w))).toEqual(w);
  });
  it("protects Default, missing contexts and the last remaining context without mutation", () => {
    const w = grouped(),
      before = structuredClone(w);
    expect(() => removeApplication(w, "default")).toThrow(
      "Default cannot be deleted",
    );
    expect(() => removeApplication(w, "missing")).toThrow("no longer exists");
    expect(w).toEqual(before);
    w.pages = w.pages.filter((page) => page.application);
    w.active_page = "chrome1";
    w.page_switching = { enabled: false, default_page: null };
    const onlyApplication = structuredClone(w);
    expect(() => removeApplication(w, pageContextKey(w.pages[0]))).toThrow(
      "Add a Default page",
    );
    expect(w).toEqual(onlyApplication);
  });
  it("preserves other selected pages and pending edits during cascade deletion and native updates", () => {
    const w = grouped();
    w.active_page = "third";
    const context = pageContextKey(w.pages[1]);
    const pending = editConfig(receiveConfig(null, w), (draft) => {
      draft.pages[0].bindings.lcd1.title = "Pending edit";
      removeApplication(draft, context);
    });
    expect(pending.draft.active_page).toBe("third");
    const reconciled = receiveConfig(pending, {
      ...w,
      active_page: "chrome3",
      brightness: 20,
    });
    expect(reconciled.draft.pages).toEqual(pending.draft.pages);
    expect(reconciled.draft.active_page).toBe("third");
    expect(reconciled.draft.brightness).toBe(20);
  });
  it("deletes to a sibling, falls back after the final app page, and protects final Default", () => {
    const w = grouped();
    w.active_page = "chrome2";
    expect(pageAfterDeletion(w, "chrome2")).toBe("chrome3");
    removePage(w, "chrome2");
    expect(w.active_page).toBe("chrome3");
    removePage(w, "chrome3");
    expect(w.active_page).toBe("chrome1");
    removePage(w, "second");
    removePage(w, "third");
    const before = structuredClone(w);
    expect(() => removePage(w, "home", "chrome1")).toThrow(
      "Keep at least one Default page",
    );
    expect(w).toEqual(before);
    removePage(w, "chrome1");
    expect(w.active_page).toBe("home");
  });
});
