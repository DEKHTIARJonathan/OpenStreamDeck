import { describe, expect, it } from "vitest";
import { defaultConfig } from "./model";
import { editConfig, isDirty, receiveConfig } from "./config-draft";

describe("config draft concurrency", () => {
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
