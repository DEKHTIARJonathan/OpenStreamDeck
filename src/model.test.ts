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

describe("automatic application page configuration", () => {
  const mapped = () => {
    const w = defaultConfig();
    w.page_switching = { enabled: true, default_page: "home" };
    w.pages.push({
      id: "chrome",
      name: "Chrome",
      bindings: {},
      application: {
        name: "Google Chrome",
        identities: [{ platform: "macos", value: "com.google.Chrome" }],
      },
    });
    return w;
  };
  it("defaults absent fields and round trips associations on foreign platforms", () => {
    const old = { ...defaultConfig() } as Record<string, unknown>;
    delete old.page_switching;
    expect(validateConfig(old).page_switching).toEqual({
      enabled: false,
      default_page: null,
    });
    const w = mapped();
    w.pages[1].application!.identities.push(
      { platform: "windows", value: "C:\\Apps\\Chrome.exe" },
      { platform: "linux", value: "wmclass:Google-chrome" },
    );
    expect(validateConfig(JSON.parse(JSON.stringify(w)))).toEqual(w);
  });
  it("requires a valid unassociated Default, including disabled stale references", () => {
    const w = mapped();
    w.page_switching.default_page = null;
    expect(() => validateConfig(w)).toThrow("Default page");
    w.page_switching = { enabled: false, default_page: "missing" };
    expect(() => validateConfig(w)).toThrow("Default page is missing");
    w.page_switching.default_page = "chrome";
    expect(() => validateConfig(w)).toThrow("cannot have an application");
  });
  it("rejects duplicate and malformed identities while preserving unknown actions", () => {
    const w = mapped();
    w.pages.push({ ...structuredClone(w.pages[1]), id: "duplicate" });
    expect(validateConfig(w).pages).toHaveLength(3);
    w.pages[2].application!.identities.push({
      platform: "linux",
      value: "wmclass:Chrome",
    });
    expect(() => validateConfig(w)).toThrow("overlapping identities");
    w.pages.pop();
    w.pages[1].application!.identities.push({
      ...w.pages[1].application!.identities[0],
    });
    expect(() => validateConfig(w)).toThrow("Duplicate application identity");
    w.pages[1].application!.identities.pop();
    w.pages[1].application!.identities[0].value = "bad\nidentity";
    expect(() => validateConfig(w)).toThrow("Invalid application identity");
    w.pages[1].application!.identities = [];
    expect(() => validateConfig(w)).toThrow("Invalid application identities");
    w.pages[1].application!.identities = [
      { platform: "macos", value: "com.google.Chrome" },
    ];
    w.pages[1].bindings.lcd1 = {
      ...w.pages[0].bindings.lcd1,
      kind: "custom.extension",
    };
    expect(validateConfig(w).pages[1].bindings.lcd1.kind).toBe(
      "custom.extension",
    );
  });
});
