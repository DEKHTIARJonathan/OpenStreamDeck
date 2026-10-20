import { describe, expect, it } from "vitest";
import { action, defaultConfig } from "./model";
import { removePage, renamePage, pageReferenceCount } from "./page-editor";
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
