import { test, expect } from "@playwright/test";

const fixture = `
  import { defaultConfig } from '/src/model.ts';
  export const native = false;
  let config = defaultConfig();
  const listeners = new Map();
  window.__emit = (event, payload) => { for (const handler of listeners.get(event) ?? []) handler(payload); };
  export async function on(event, callback) {
    // Each subscription owns its cleanup, even when StrictMode reuses a callback.
    const handler = payload => callback(payload);
    if (!listeners.has(event)) listeners.set(event, new Set());
    const handlers = listeners.get(event);
    handlers.add(handler);
    return () => handlers.delete(handler);
  }
  export async function call(command, args) {
    if (command === 'snapshot') return { config, device: { connected:false,serial:'',product:'N3',message:'Fixture',packets_in:0,packets_out:0,sleeping:false }, agents:[],adapter_errors:[],data_dir:'fixture' };
    if (command === 'save_config') return new Promise(resolve => { window.__finishSave = () => {config=args.config;resolve();}; });
    if (command === 'list_applications') return [{name:'Example',path:'/fixture/Example.app'}];
    if (command === 'application_icon') return new Promise(resolve => {window.__finishIcon = () => resolve('data:image/png;base64,fixture');});
    throw new Error('Unexpected command: ' + command);
  }
  window.__brightness = value => {config = {...config, brightness:value};window.__emit('config',config);};
`;

test.beforeEach(async ({ page }) => {
  await page.route(/\/src\/api\.ts(?:\?.*)?$/, (route) =>
    route.fulfill({ contentType: "application/javascript", body: fixture }),
  );
});

test("device events and save acknowledgements preserve newer editor changes", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByLabel("Label", { exact: true }).fill("My terminal");
  await page.evaluate(() =>
    (window as unknown as { __brightness: (n: number) => void }).__brightness(
      75,
    ),
  );
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "My terminal",
  );
  await expect(page.getByRole("slider", { name: "Brightness" })).toHaveValue(
    "75",
  );
  await expect(
    page.getByRole("button", { name: "Save changes" }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.getByLabel("Label", { exact: true }).fill("Newer edit");
  await page.evaluate(() =>
    (window as unknown as { __finishSave: () => void }).__finishSave(),
  );
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Newer edit",
  );
  await expect(
    page.getByRole("button", { name: "Save changes" }),
  ).toBeEnabled();
  await page.evaluate(() =>
    (
      window as unknown as { __emit: (event: string, data: unknown) => void }
    ).__emit("action_error", "Brightness: Device queue full"),
  );
  await expect(page.getByRole("alert")).toContainText(
    "Brightness: Device queue full",
  );
});

test("closing an application picker cancels a pending icon selection", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Terminal Change application", exact: true })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Example /fixture/Example.app", exact: true })
    .click();
  await page.getByRole("button", { name: "Close app picker" }).click();
  await page.getByLabel("Label", { exact: true }).fill("Keep this edit");
  await page.evaluate(() =>
    (window as unknown as { __finishIcon: () => void }).__finishIcon(),
  );
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Keep this edit",
  );
  await expect(
    page.getByRole("button", {
      name: "Keep this edit Change application",
      exact: true,
    }),
  ).toBeVisible();
});
