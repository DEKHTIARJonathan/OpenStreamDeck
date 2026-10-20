import { describe, expect, it } from "vitest";
import { defaultConfig, validateConfig } from "./model";
import { editConfig, isDirty, receiveConfig } from "./config-draft";
import applicationPages from "../tests/fixtures/config-application-pages.json";

describe("config draft concurrency", () => {
  it("retains application-page settings through edits and native save acknowledgements", () => {
    const initial = validateConfig(structuredClone(applicationPages));
    const edited = editConfig(receiveConfig(null, initial), (config) => {
      config.pages[1].bindings.lcd1.title = "Save file";
    });
    const received = receiveConfig(edited, { ...initial, brightness: 42 });
    const expected = structuredClone(applicationPages);
    expected.brightness = 42;
    expected.pages[1].bindings.lcd1!.title = "Save file";
    expect(received.draft).toEqual(expected);
    const saved = receiveConfig(received, received.draft);
    expect(saved.draft).toEqual(expected);
    expect(isDirty(saved)).toBe(false);
  });
  it("keeps newly imported optional fields when native events arrive before saving", () => {
    const original = defaultConfig();
    const imported = validateConfig(structuredClone(applicationPages));
    const pending = { base: original, draft: imported };
    const received = receiveConfig(pending, { ...original, brightness: 75 });
    expect(received.draft.page_switching).toEqual(imported.page_switching);
    expect(received.draft.pages).toEqual(imported.pages);
    const saved = receiveConfig(received, structuredClone(received.draft));
    expect(saved.draft.page_switching).toEqual(imported.page_switching);
    expect(isDirty(saved)).toBe(false);
    expect(receiveConfig(null, saved.draft).draft).toEqual(saved.draft);
  });
  it("preserves explicit local removal and accepts remote removal of optional fields", () => {
    const original = validateConfig(structuredClone(applicationPages));
    const pending = editConfig(receiveConfig(null, original), (draft) => {
      delete (draft as Partial<typeof draft>).page_switching;
    });
    expect(
      receiveConfig(pending, { ...original, brightness: 75 }).draft,
    ).not.toHaveProperty("page_switching");
    const remote = structuredClone(original);
    delete (remote as Partial<typeof remote>).page_switching;
    const clean = receiveConfig(receiveConfig(null, original), remote);
    expect(clean.draft).not.toHaveProperty("page_switching");
    expect(isDirty(clean)).toBe(false);
  });
  it("keeps unsaved key edits when a hardware dial changes brightness", () => {
    const initial = defaultConfig();
    const local = editConfig(receiveConfig(null, initial), (w) => {
      w.pages[0].bindings.lcd1.title = "Edited";
    });
    const updated = receiveConfig(local, { ...initial, brightness: 75 });
    expect(updated.draft.pages[0].bindings.lcd1.title).toBe("Edited");
    expect(updated.draft.brightness).toBe(75);
    expect(isDirty(updated)).toBe(true);
  });
  it("keeps edits made while a save is pending, and clears dirty only for saved content", () => {
    let state = receiveConfig(null, defaultConfig());
    state = editConfig(state, (w) => {
      w.brightness = 42;
    });
    const submitted = state.draft;
    state = editConfig(state, (w) => {
      w.brightness = 43;
    });
    state = receiveConfig(state, submitted);
    expect(state.draft.brightness).toBe(43);
    expect(isDirty(state)).toBe(true);
    state = receiveConfig(state, state.draft);
    expect(isDirty(state)).toBe(false);
  });
  it("adopts remote changes when clean and preserves a locally selected page", () => {
    const initial = defaultConfig();
    initial.pages.push({ id: "second", name: "Second", bindings: {} });
    const clean = receiveConfig(receiveConfig(null, initial), {
      ...initial,
      active_page: "second",
    });
    expect(clean.draft.active_page).toBe("second");
    expect(isDirty(clean)).toBe(false);
    const local = editConfig(clean, (w) => {
      w.active_page = "home";
    });
    expect(
      receiveConfig(local, {
        ...initial,
        active_page: "second",
        brightness: 70,
      }).draft.active_page,
    ).toBe("home");
  });
});
