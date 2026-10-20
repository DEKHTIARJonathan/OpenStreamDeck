import { test, expect } from "@playwright/test";

const fixture = `
  import { defaultConfig, validateConfig } from '/src/model.ts';
  export const native = true;
  let config = JSON.parse(localStorage.getItem('settings-fixture') || 'null') || defaultConfig();
  let navigation = {view:'Deck',revision:0};
  const listeners = new Map();
  function emit(event, payload) { for (const handler of listeners.get(event) ?? []) handler(payload); }
  export async function on(event, callback) {
    const handler = payload => callback(payload);
    if (!listeners.has(event)) listeners.set(event, new Set());
    const handlers = listeners.get(event); handlers.add(handler);
    return () => handlers.delete(handler);
  }
  window.__settingsCalls = [];
  window.__saveError = '';
  window.__keyError = '';
  export async function call(command, args) {
    if (command === 'snapshot') return { navigation, config, device:{connected:false,serial:'',product:'N3',message:'N3 not connected',packets_in:0,packets_out:0,sleeping:false},agents:[],adapter_errors:[],data_dir:'/fixture/OpenStreamDeck',settings_error:sessionStorage.getItem('settings-error') };
    if (command === 'render_keys') return [];
    if (command === 'set_view') return navigation = {view:args.view, revision:navigation.revision+1};
    if (command === 'save_config') {
      if (window.__saveError) throw new Error(window.__saveError);
      config = validateConfig(args.config);
      localStorage.setItem('settings-fixture', JSON.stringify(config));
      emit('config',config);
      return;
    }
    if (command === 'set_provider_key') {
      window.__settingsCalls.push({command,...args});
      if (window.__keyError) throw new Error(window.__keyError);
      return;
    }
    throw new Error('Unexpected command: ' + command);
  }
`;
test.beforeEach(async ({ page }) => {
  await page.route(/\/src\/api\.ts(?:\?.*)?$/, (route) =>
    route.fulfill({ contentType: "application/javascript", body: fixture }),
  );
  await page.goto("/");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
});

test("settings tabs retain edits, save both toggles in both directions and reload", async ({
  page,
}) => {
  const login = page.getByRole("switch", { name: "Launch at login" });
  const tray = page.getByRole("switch", { name: "Menu bar only" });
  await expect(login).not.toBeChecked();
  await expect(tray).toBeChecked();
  await login.click();
  await tray.click();
  await expect(
    page.getByText("Closing the window will quit OpenStreamDeck.", {
      exact: false,
    }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "Device", exact: true }).click();
  await expect(page.getByLabel("Sleep after inactivity (minutes)")).toHaveValue(
    "0",
  );
  await expect(
    page.getByText("Automatic sleep is deactivated.", { exact: false }),
  ).toBeVisible();
  await page.getByLabel("Sleep after inactivity (minutes)").fill("23");
  await page.getByLabel("Image size", { exact: true }).selectOption("96");
  await page.getByLabel("Image rotation").selectOption("180");
  await page.getByRole("tab", { name: "Agents & AI" }).click();
  await page.getByLabel("Model ID").fill("fixture-model");
  await page.getByRole("button", { name: "Anthropic Anthropic API" }).click();
  await page.getByLabel("Model ID").fill("other-model");
  await page.getByRole("button", { name: "OpenAI OpenAI API" }).click();
  await expect(page.getByLabel("Model ID")).toHaveValue("fixture-model");
  await page.getByRole("tab", { name: "General", exact: true }).click();
  await expect(login).toBeChecked();
  await expect(tray).not.toBeChecked();
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(
    page.getByRole("button", { name: "Save changes" }),
  ).toBeDisabled();
  await page.reload();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(login).toBeChecked();
  await expect(tray).not.toBeChecked();
  await login.click();
  await tray.click();
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.reload();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(login).not.toBeChecked();
  await expect(tray).toBeChecked();
  await page.getByRole("tab", { name: "Device", exact: true }).click();
  await expect(page.getByLabel("Sleep after inactivity (minutes)")).toHaveValue(
    "23",
  );
  await expect(page.getByLabel("Image size", { exact: true })).toHaveValue(
    "96",
  );
  await expect(page.getByLabel("Image rotation")).toHaveValue("180");
  await page.getByRole("tab", { name: "Agents & AI" }).click();
  await expect(page.getByLabel("Model ID")).toHaveValue("fixture-model");
});

test("failed preference saves surface errors and retain an unsaved draft", async ({
  page,
}) => {
  await page.getByRole("switch", { name: "Launch at login" }).click();
  await page.evaluate(() => {
    (window as any).__saveError =
      "Could not update launch at login: registration denied";
  });
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByRole("alert")).toContainText("registration denied");
  await expect(
    page.getByRole("switch", { name: "Launch at login" }),
  ).toBeChecked();
  await expect(
    page.getByRole("button", { name: "Save changes" }),
  ).toBeEnabled();
  await page.reload();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(
    page.getByRole("switch", { name: "Launch at login" }),
  ).not.toBeChecked();
  await page.getByRole("tab", { name: "Device", exact: true }).click();
  await page.getByLabel("Sleep after inactivity (minutes)").fill("-1");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByRole("alert")).toContainText("Invalid sleep minutes");
  await page.evaluate(() =>
    sessionStorage.setItem(
      "settings-error",
      "Could not read launch at login: OS unavailable",
    ),
  );
  await page.reload();
  await expect(page.getByRole("alert")).toContainText(
    "Could not read launch at login: OS unavailable",
  );
});

test("settings tabs support keyboard navigation and fit the minimum desktop window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  const general = page.getByRole("tab", { name: "General", exact: true });
  await general.focus();
  await page.keyboard.press("ArrowRight");
  await expect(
    page.getByRole("tab", { name: "Device", exact: true }),
  ).toBeFocused();
  await expect(page.getByRole("tabpanel")).toHaveAccessibleName("Device");
  await page.keyboard.press("End");
  await expect(page.getByRole("tab", { name: "Agents & AI" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.getByLabel("Endpoint")).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.keyboard.press("Home");
  await expect(general).toBeFocused();
  await expect(general).toHaveAttribute("aria-selected", "true");
});

test("provider credentials stay scoped, require saved settings, and expose failures", async ({
  page,
}) => {
  await page.getByRole("tab", { name: "Agents & AI" }).click();
  for (const [button, id] of [
    ["OpenAI OpenAI API", "openai"],
    ["Anthropic Anthropic API", "anthropic"],
    ["Google Gemini Gemini API", "gemini"],
    ["Local endpoint OpenAI-compatible", "local"],
  ]) {
    await page.getByRole("button", { name: button }).click();
    await page.getByLabel("API key", { exact: true }).fill("fixture-secret");
    await page.getByLabel("Model ID").fill(`test-${id}`);
    await expect(
      page.getByRole("button", { name: "Save key", exact: true }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "Remove stored key" }),
    ).toBeDisabled();
    await page.getByRole("button", { name: "Save changes" }).click();
    await page.getByRole("button", { name: "Save key", exact: true }).click();
    await expect(page.getByLabel("API key", { exact: true })).toHaveValue("");
    await page.getByRole("button", { name: "Remove stored key" }).click();
    expect(
      await page.evaluate(() => (window as any).__settingsCalls.slice(-2)),
    ).toEqual([
      { command: "set_provider_key", provider: id, key: "fixture-secret" },
      { command: "set_provider_key", provider: id, key: "" },
    ]);
  }
  await page.getByLabel("API key", { exact: true }).fill("retry-secret");
  await page.evaluate(() => {
    (window as any).__keyError = "Credential store unavailable";
  });
  await page.getByRole("button", { name: "Save key", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Credential store unavailable",
  );
  await expect(page.getByLabel("API key", { exact: true })).toHaveValue(
    "retry-secret",
  );
  expect(
    await page.evaluate(() => localStorage.getItem("settings-fixture")),
  ).not.toContain("secret");
});
