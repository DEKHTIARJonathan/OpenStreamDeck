import { describe, expect, it } from "vitest";
import { defaultConfig, validateConfig } from "./model";
import applicationPages from "../tests/fixtures/config-application-pages.json";

it("preserves current-version application-page settings through import and export", () => {
  const imported = validateConfig(structuredClone(applicationPages));
  expect(JSON.parse(JSON.stringify(imported))).toEqual(applicationPages);
});

describe("config import boundary", () => {
  it.each([
    null,
    { ...defaultConfig(), workflows: null },
    { ...defaultConfig(), providers: "broken" },
    { ...defaultConfig(), image_size: 999999 },
    { ...defaultConfig(), sleep_minutes: -1 },
  ])("rejects malformed config before it reaches React: %s", (value) => {
    expect(() => validateConfig(value)).toThrow();
  });
  it("rejects duplicate provider/workflow IDs and malformed actions", () => {
    const w = defaultConfig();
    w.providers.push({ ...w.providers[0] });
    expect(() => validateConfig(w)).toThrow("Provider IDs");
    w.providers.pop();
    w.workflows.push({ ...w.workflows[0] });
    expect(() => validateConfig(w)).toThrow("Workflow IDs");
    w.workflows.pop();
    (w.pages[0].bindings.lcd1 as unknown as { options: unknown }).options =
      null;
    expect(() => validateConfig(w)).toThrow("action options");
  });
  it("rejects unsupported versions, orphaned pages and invalid brightness", () => {
    expect(() => validateConfig({ ...defaultConfig(), version: 999 })).toThrow(
      "Not an OpenStreamDeck",
    );
    expect(() =>
      validateConfig({ ...defaultConfig(), active_page: "missing" }),
    ).toThrow("Active page");
    for (const brightness of [-1, 101, NaN, 1.5])
      expect(() => validateConfig({ ...defaultConfig(), brightness })).toThrow(
        "brightness",
      );
  });
  it("preserves unknown action kinds when importing and exporting", () => {
    const w = defaultConfig();
    w.pages[0].bindings.lcd1.kind = "vendor-extension";
    const imported = validateConfig(JSON.parse(JSON.stringify(w)));
    expect(imported.pages[0].bindings.lcd1.kind).toBe("vendor-extension");
  });
});

it("round-trips page configs and rejects unsupported fields and formats", () => {
  const config = defaultConfig();
  expect(validateConfig(JSON.parse(JSON.stringify(config)))).toEqual(config);
  for (const version of [1, 0, 999]) {
    expect(() => validateConfig({ ...config, version })).toThrow(
      "Not an OpenStreamDeck",
    );
  }
  expect(() => validateConfig({ ...config, unexpected: [] })).toThrow(
    "Unsupported config field",
  );
});

it("defaults sleep off and preserves settings through import/export", () => {
  const config = defaultConfig();
  expect(config.sleep_minutes).toBe(0);
  expect(config.launch_at_login).toBe(false);
  expect(config.menu_bar_only).toBe(true);
  for (const enabled of [true, false]) {
    const next = {
      ...config,
      launch_at_login: enabled,
      menu_bar_only: enabled,
      sleep_minutes: 30,
    };
    expect(validateConfig(JSON.parse(JSON.stringify(next)))).toEqual(next);
  }
  const previous = JSON.parse(JSON.stringify(config));
  delete previous.launch_at_login;
  delete previous.menu_bar_only;
  previous.sleep_minutes = 15;
  expect(validateConfig(previous)).toEqual({ ...config, sleep_minutes: 15 });
});

it.each(["launch_at_login", "menu_bar_only"])("rejects malformed %s", (key) => {
  for (const value of [null, "true", 0, [], {}])
    expect(() => validateConfig({ ...defaultConfig(), [key]: value })).toThrow(
      `Invalid ${key}`,
    );
});
