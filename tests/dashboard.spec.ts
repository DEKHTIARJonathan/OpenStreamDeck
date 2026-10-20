import { test, expect, type Page } from "@playwright/test";

async function fixture(page: Page, startup = "Deck", delayedSnapshot = false) {
  await page.route(/\/src\/api\.ts(?:\?.*)?$/, (route) =>
    route.fulfill({
      contentType: "application/javascript",
      body: `
    import { defaultConfig } from '/src/model.ts';
    export const native = true;
    const listeners = new Map();
    window.__calls = [];
    window.__emit = (name, value) => listeners.get(name)?.(value);
    let navigation = {view:${JSON.stringify(startup)},revision:0};
    const config = defaultConfig();
    export async function call(command,args) {
      window.__calls.push({command,args});
      if(command==='snapshot') {
        const snapshot = {navigation:{...navigation},config,device:{connected:false,serial:'',product:'N3',message:'Fixture',packets_in:0,packets_out:0,sleeping:false},agents:[],adapter_errors:[],data_dir:'fixture'};
        if (${delayedSnapshot}) await new Promise(resolve => window.__finishSnapshot = resolve);
        return snapshot;
      }
      if(command==='render_keys') return [];
      if(command==='set_view') return navigation = {view:args.view,revision:navigation.revision+1};
      if(command==='run_action') {
        navigation = {view:'Agents',revision:navigation.revision+1};
        window.__emit('navigate',navigation);
        await new Promise(resolve => window.__finishRun = resolve);
        return 'fixture-run';
      }
      if(command==='show_agents') return;
      throw new Error('Unexpected fixture call: '+command);
    }
    export async function on(name,fn) {const handler = value => fn(value); listeners.set(name,handler); return ()=>{if(listeners.get(name)===handler) listeners.delete(name);};}
  `,
    }),
  );
  await page.goto("/");
}

test("dashboard opens Agents and action completion preserves the tab and normal keys", async ({
  page,
}) => {
  await fixture(page);
  await expect(page.locator(".lcd")).toHaveCount(6);
  const labels = await page
    .locator(".lcd")
    .evaluateAll((keys) => keys.map((k) => k.getAttribute("aria-label")));
  await page.getByRole("button", { name: /LCD 5:/ }).click();
  await expect(page.getByLabel("Action", { exact: true })).toHaveValue(
    "agents",
  );
  await page.getByRole("button", { name: "Run action", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Agents", exact: true }),
  ).toBeVisible();
  await page.evaluate(() => (window as any).__finishRun());
  await expect(
    page.getByRole("heading", { name: "Agents", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Deck", exact: true })
    .click();
  await expect(page.locator(".lcd")).toHaveCount(6);
  expect(
    await page
      .locator(".lcd")
      .evaluateAll((keys) => keys.map((k) => k.getAttribute("aria-label"))),
  ).toEqual(labels);
  const calls = await page.evaluate(() => (window as any).__calls);
  expect(
    calls.filter((c: any) =>
      ["agent_focus", "agent_launch", "show_agents", "save_config"].includes(
        c.command,
      ),
    ),
  ).toEqual([]);
  expect(
    calls
      .filter((c: any) => c.command === "set_view")
      .map((c: any) => c.args.view),
  ).toEqual(["Deck"]);
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Agents", exact: true })
    .click();
  await page.getByRole("button", { name: "Show on deck", exact: true }).click();
  expect(
    await page.evaluate(() =>
      (window as any).__calls.filter((c: any) => c.command === "show_agents"),
    ),
  ).toEqual([{ command: "show_agents", args: { enabled: true } }]);
});

test("a recreated webview restores the requested Agents tab from native state", async ({
  page,
}) => {
  await fixture(page, "Agents");
  await expect(
    page.getByRole("heading", { name: "Agents", exact: true }),
  ).toBeVisible();
});

test("dashboard navigation from a workflow survives the workflow response", async ({
  page,
}) => {
  await fixture(page);
  await page.getByRole("button", { name: /LCD 4:/ }).click();
  await expect(page.getByLabel("Action", { exact: true })).toHaveValue(
    "workflow",
  );
  await page.getByRole("button", { name: "Run action", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Agents", exact: true }),
  ).toBeVisible();
  await page.evaluate(() => (window as any).__finishRun());
  await expect(
    page.getByRole("heading", { name: "Agents", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as any).__calls
        .filter((c: any) => c.command === "set_view")
        .map((c: any) => c.args.view),
    ),
  ).toEqual([]);
});

test("hardware navigation wins over a delayed startup snapshot and stale events", async ({
  page,
}) => {
  await fixture(page, "Deck", true);
  await page.waitForFunction(
    () => typeof (window as any).__finishSnapshot === "function",
  );
  await page.evaluate(() => {
    (window as any).__emit("navigate", { view: "Agents", revision: 2 });
    (window as any).__finishSnapshot();
  });
  await expect(
    page.getByRole("heading", { name: "Agents", exact: true }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).__emit("navigate", { view: "Settings", revision: 1 }),
  );
  await expect(
    page.getByRole("heading", { name: "Agents", exact: true }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).__emit(
      "action_error",
      "Agent dashboard: Could not focus OpenStreamDeck window",
    ),
  );
  await expect(page.getByRole("alert")).toContainText(
    "Could not focus OpenStreamDeck window",
  );
});
