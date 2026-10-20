import { test, expect, type Page } from "@playwright/test";
import {
  pageTab,
  applicationContext,
  editContext,
  applicationOptions,
  editPage,
  usePageOption,
} from "./page-controls";

// Native boundaries use synthetic application identities and never inspect the
// test machine's foreground application, installed apps, or device.
async function fixture(
  page: Page,
  delayedSnapshot = false,
  foreignAssociation = false,
  setup: "existing" | "first" | "only-associated" = "existing",
  extraPages = 0,
) {
  await page.route(/\/src\/api\.ts(?:\?.*)?$/, (route) =>
    route.fulfill({
      contentType: "application/javascript",
      body: `
        import { defaultConfig, action } from '/src/model.ts';
        export const native = true;
        const listeners = new Map();
        window.__calls = [];
        window.__emit = (event, payload) => {
          for (const handler of listeners.get(event) ?? []) handler(payload);
        };
        export async function on(event, callback) {
          const handler = payload => callback(payload);
          if (!listeners.has(event)) listeners.set(event, new Set());
          const handlers = listeners.get(event);
          handlers.add(handler);
          return () => handlers.delete(handler);
        }
        let config = defaultConfig();
        const home = config.pages[0];
        home.id = 'home';
        config.active_page = 'home';
        config.page_switching = {enabled:true,default_page:'home'};
        config.pages.push(
          {id:'chrome',name:'Chrome',application:{name:'Google Chrome',identities:[{platform:'macos',value:'com.google.Chrome'}]},bindings:{lcd1:action('shortcut','New tab','meta+t')}},
          {id:'code',name:'VS Code',application:{name:'Visual Studio Code',identities:[{platform:'macos',value:'com.microsoft.VSCode'}]},bindings:{lcd1:action('shortcut','Command palette','meta+shift+p')}},
          {id:'manual',name:'Utilities',bindings:{}}
        );
        if (${foreignAssociation}) config.pages[1].application.identities = [{platform:'windows',value:'C:/Example/chrome.exe'}];
        if (${JSON.stringify(setup)} === 'first') {
          config.pages = [home];
          config.page_switching = {enabled:false,default_page:null};
        }
        if (${JSON.stringify(setup)} === 'only-associated') {
          config.pages = config.pages.filter(page => page.application);
          config.active_page = 'chrome';
          config.page_switching = {enabled:false,default_page:null};
        }
        for (let index=1; index<=${extraPages}; index++) config.pages.push({
          id:'extra-'+index,
          name:'Long application workspace '+index,
          application:{name:'Google Chrome',identities:[{platform:'macos',value:'com.google.Chrome'}]},
          bindings:{}
        });
        let context = {revision:1,desired_page:config.active_page,applied_page:config.active_page,reason:config.page_switching.enabled?'default':'manual',application:null,error:null,automatic:config.page_switching.enabled,override_active:false,overlay:false,overlay_applied:false};
        const device = {connected:true,serial:'',product:'N3',message:'Synthetic fixture',packets_in:0,packets_out:0,sleeping:false};
        window.__context = patch => {
          context = {...context,revision:context.revision+1,...patch};
          window.__emit('page-context',context);
        };
        window.__device = patch => window.__emit('device',{...device,...patch});
        window.__saved = () => config;
        window.__replaceConfig = next => { config = next; window.__emit('config', config); };
        window.__saveFailure = false;
        window.__delaySave = false;
        const apps = [
          {name:'Google Chrome',path:'/fixture/Google Chrome.app'},
          {name:'Example',path:'/fixture/Example.app'},
          {name:'No identity',path:'/fixture/No identity.app'}
        ];
        export async function call(command,args) {
          window.__calls.push({command,args});
          if (command === 'snapshot') {
            const snapshot = {navigation:{view:'Deck',revision:0},config,device,page_context:{...context},platform:'macos',agents:[],adapter_errors:[],data_dir:'fixture'};
            if (${delayedSnapshot}) await new Promise(resolve => window.__finishSnapshot = resolve);
            return snapshot;
          }
          if (command === 'render_keys') return [];
          if (command === 'list_applications') return apps;
          if (command === 'application_icon') throw new Error('Icon unavailable in fixture');
          if (command === 'application_identity') {
            if (window.__delayIdentity) await new Promise(resolve => window.__finishIdentity = resolve);
            if (args.path.includes('No identity')) throw new Error('No stable application identity');
            return {name:args.path.includes('Google Chrome')?'Google Chrome':'Example',identities:[{platform:'macos',value:args.path.includes('Google Chrome')?'com.google.Chrome':'com.example.Example'}]};
          }
          if (command === 'save_config') {
            if (window.__delaySave) await new Promise(resolve => window.__finishSave = resolve);
            if (window.__saveFailure) throw new Error('Configuration could not be saved');
            config = structuredClone(args.config);
            window.__emit('config',config);
            return;
          }
          if (command === 'select_page') {
            window.__context({desired_page:args.page,applied_page:args.page,reason:'manual',override_active:true});
            return context;
          }
          if (command === 'resume_automatic') {
            window.__context({desired_page:'home',applied_page:'home',reason:'default',override_active:false});
            return context;
          }
          if (command === 'set_view') return {view:args.view,revision:1};
          throw new Error('Unexpected fixture command: '+command);
        }
      `,
    }),
  );
  await page.goto("/");
}

const pageButton = (page: Page, name: string) =>
  page.getByRole("button", { name, exact: true });
const saveButton = (page: Page) => pageButton(page, "Save changes");

test("page row exposes Add, Rename and Delete in order without a page menu", async ({
  page,
}, testInfo) => {
  await fixture(page);
  const row = page.locator(".page-navigation-toolbar");
  for (const width of [1380, 1060]) {
    await page.setViewportSize({ width, height: 900 });
    const add = row.getByRole("button", { name: "Add page", exact: true });
    const rename = row.getByRole("button", {
      name: "Rename page",
      exact: true,
    });
    const remove = row.getByRole("button", {
      name: "Delete page",
      exact: true,
    });
    await expect(add).toBeInViewport();
    await expect(rename).toBeInViewport();
    await expect(remove).toBeInViewport();
    expect((await add.boundingBox())!.x).toBeLessThan(
      (await rename.boundingBox())!.x,
    );
    expect((await rename.boundingBox())!.x).toBeLessThan(
      (await remove.boundingBox())!.x,
    );
    for (const name of ["Page options", "Move left", "Move right"])
      await expect(pageButton(page, name)).toHaveCount(0);
    await page.screenshot({
      path: testInfo.outputPath(`page-actions-${width}.png`),
      animations: "disabled",
      fullPage: true,
    });
  }
  await pageButton(page, "Rename page").focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("dialog", { name: "Rename page", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Page name" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(saveButton(page)).toBeDisabled();
});

test("default, application and manual pages are separate; editing never activates a page", async ({
  page,
}) => {
  await fixture(page);
  await expect(pageTab(page, "Home")).toHaveCount(1);
  await expect(pageTab(page, "Home")).toHaveAccessibleDescription(/Default/);
  await expect(pageTab(page, "Chrome")).toHaveCount(0);
  await expect(pageTab(page, "Utilities")).toBeVisible();
  await expect(applicationContext(page, "Google Chrome")).toBeVisible();
  await expect(applicationContext(page, "Visual Studio Code")).toBeVisible();
  await expect(
    page.getByRole("region", { name: "Page navigation" }).getByRole("combobox"),
  ).toHaveCount(0);
  await editPage(page, "Chrome", "Google Chrome");
  await expect(
    page.getByText("Editing: Chrome", { exact: true }),
  ).toBeVisible();
  await expect(page.getByText(/On device: Default \/ Home/)).toBeVisible();
  await expect(saveButton(page)).toBeDisabled();
  expect(
    await page.evaluate(() =>
      (window as any).__calls.filter((call: any) =>
        ["select_page", "save_config"].includes(call.command),
      ),
    ),
  ).toEqual([]);
  await page.getByLabel("Label", { exact: true }).fill("My browser shortcut");
  await page.evaluate(() =>
    (window as any).__context({
      desired_page: "code",
      applied_page: "code",
      reason: "application",
      application: {
        name: "Visual Studio Code",
        identity: { platform: "macos", value: "com.microsoft.VSCode" },
      },
    }),
  );
  await expect(
    page.getByText(/On device: Visual Studio Code \/ VS Code/),
  ).toBeVisible();
  await expect(
    page.getByText("Editing: Chrome", { exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "My browser shortcut",
  );
  await expect(saveButton(page)).toBeEnabled();
  await usePageOption(page, "Show on deck");
  await expect(
    page.getByText(/On device: Google Chrome \/ Chrome/),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as any).__saved().pages[1].bindings.lcd1.title,
    ),
  ).toBe("New tab");
  await pageButton(page, "Resume automatic").click();
  await expect(page.getByText(/On device: Default \/ Home/)).toBeVisible();
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "My browser shortcut",
  );
});

test("newer application state wins over startup snapshots and stale events", async ({
  page,
}) => {
  await fixture(page, true);
  await page.waitForFunction(
    () => typeof (window as any).__finishSnapshot === "function",
  );
  await page.evaluate(() => {
    (window as any).__context({
      revision: 5,
      desired_page: "chrome",
      applied_page: "chrome",
      reason: "application",
    });
    (window as any).__finishSnapshot();
  });
  await expect(
    page.getByText(/On device: Google Chrome \/ Chrome/),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).__context({
      revision: 3,
      desired_page: "code",
      applied_page: "code",
    }),
  );
  await expect(
    page.getByText(/On device: Google Chrome \/ Chrome/),
  ).toBeVisible();
  await expect(saveButton(page)).toBeDisabled();
});

test("association creation is atomic, tolerates missing icons and prevents duplicates", async ({
  page,
}) => {
  await fixture(page);
  await pageButton(page, "Add application").click();
  let picker = page.getByRole("dialog", { name: "Create application page" });
  await picker.getByRole("button", { name: /Google Chrome \/fixture/ }).click();
  await picker
    .getByRole("button", { name: "Create page", exact: true })
    .click();
  await expect(
    pageButton(page, "Edit existing application page"),
  ).toBeVisible();
  await pageButton(page, "Edit existing application page").click();
  await expect(
    page.getByText("Editing: Chrome", { exact: true }),
  ).toBeVisible();
  await expect(saveButton(page)).toBeDisabled();
  await pageButton(page, "Add application").click();
  picker = page.getByRole("dialog", { name: "Create application page" });
  await picker.getByRole("button", { name: /No identity \/fixture/ }).click();
  await expect(picker.getByRole("alert")).toContainText(
    "No stable application identity",
  );
  await expect(saveButton(page)).toBeDisabled();
  await picker.getByRole("button", { name: /Example \/fixture/ }).click();
  const create = page.getByRole("dialog", { name: "Create application page" });
  await expect(create.getByLabel("Page name")).toHaveValue("Main");
  await create.getByLabel("Page name").fill("Example tools");
  await create
    .getByRole("button", { name: "Create page", exact: true })
    .click();
  await expect(pageTab(page, "Example tools")).toHaveCount(1);
  await expect(pageButton(page, "Show on deck")).toBeVisible();
  await expect(pageButton(page, "Show on deck")).toBeDisabled();
  await saveButton(page).click();
  expect(
    await page.evaluate(
      () =>
        (window as any)
          .__saved()
          .pages.find((item: any) => item.name === "Example tools").application
          .identities,
    ),
  ).toEqual([{ platform: "macos", value: "com.example.Example" }]);
  expect(
    await page.evaluate(() => (window as any).__saved().page_switching),
  ).toEqual({ enabled: true, default_page: "home" });
});

test("first application page includes fallback and switching setup in one cancellable draft", async ({
  page,
}, testInfo) => {
  await fixture(page, false, false, "first");
  const original = await page.evaluate(() => (window as any).__saved());
  await expect(pageButton(page, "Add application")).toBeInViewport();
  await expect(applicationContext(page, "Default")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.screenshot({
    path: testInfo.outputPath("application-pages-unconfigured.png"),
    fullPage: true,
  });
  await pageButton(page, "Add application").click();
  let create = page.getByRole("dialog", { name: "Create application page" });
  await expect(
    create.getByRole("button", { name: /Google Chrome \/fixture/ }),
  ).toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("create-application-page-choose.png"),
    fullPage: true,
  });
  await create.getByRole("button", { name: /No identity \/fixture/ }).click();
  await expect(create.getByRole("alert")).toContainText(
    "No stable application identity",
  );
  await create.getByRole("button", { name: /Google Chrome \/fixture/ }).click();
  await expect(create.getByLabel("Page name")).toHaveValue("Main");
  await expect(create.getByLabel("Default page", { exact: true })).toHaveValue(
    "home",
  );
  await expect(
    create.getByRole("checkbox", { name: "Switch automatically", exact: true }),
  ).toBeChecked();
  await page.screenshot({
    path: testInfo.outputPath("create-application-page-first.png"),
    fullPage: true,
  });
  await page.setViewportSize({ width: 1060, height: 720 });
  await create
    .getByRole("button", { name: "Create page", exact: true })
    .scrollIntoViewIfNeeded();
  await expect(
    create.getByRole("button", { name: "Create page", exact: true }),
  ).toBeInViewport();
  await page.screenshot({
    path: testInfo.outputPath("create-application-page-first-narrow.png"),
    fullPage: true,
  });
  await page.setViewportSize({ width: 1380, height: 900 });
  await create.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(saveButton(page)).toBeDisabled();
  expect(await page.evaluate(() => (window as any).__saved())).toEqual(
    original,
  );
  await pageButton(page, "Add application").click();
  create = page.getByRole("dialog", { name: "Create application page" });
  await create.getByRole("button", { name: /Google Chrome \/fixture/ }).click();
  await create.getByLabel("Page name").fill("Browser tools");
  await create
    .getByRole("button", { name: "Create page", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.getByText("Editing: Browser tools", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("checkbox", { name: /Automatic page switching/ }),
  ).toBeChecked();
  await expect(saveButton(page)).toBeEnabled();
  expect(await page.evaluate(() => (window as any).__saved())).toEqual(
    original,
  );
  await page.evaluate(() => {
    (window as any).__saveFailure = true;
  });
  await saveButton(page).click();
  await expect(page.getByRole("alert")).toContainText(
    "Configuration could not be saved",
  );
  await expect(
    page.getByText("Editing: Browser tools", { exact: true }),
  ).toBeVisible();
  expect(await page.evaluate(() => (window as any).__saved())).toEqual(
    original,
  );
  await page.evaluate(() => {
    (window as any).__saveFailure = false;
  });
  await pageButton(page, "Save and enable").click();
  await expect(saveButton(page)).toBeDisabled();
  await expect(pageButton(page, "Save and enable")).toHaveCount(0);
  const saved = await page.evaluate(() => (window as any).__saved());
  expect(saved.page_switching).toEqual({ enabled: true, default_page: "home" });
  expect(saved.pages[0].bindings).toEqual(original.pages[0].bindings);
  expect(saved.pages[1]).toMatchObject({
    name: "Browser tools",
    bindings: {},
    application: {
      name: "Google Chrome",
      identities: [{ platform: "macos", value: "com.google.Chrome" }],
    },
  });
});

test("a removed fallback keeps application setup open with the user's choices intact", async ({
  page,
}) => {
  await fixture(page, false, false, "first");
  await pageButton(page, "Add application").click();
  const create = page.getByRole("dialog", { name: "Create application page" });
  await create.getByRole("button", { name: /Google Chrome \/fixture/ }).click();
  await create.getByLabel("Page name").fill("My browser");
  await page.evaluate(() => {
    const next = structuredClone((window as any).__saved());
    next.pages = [{ id: "new-home", name: "New home", bindings: {} }];
    next.active_page = "new-home";
    (window as any).__replaceConfig(next);
  });
  await create
    .getByRole("button", { name: "Create page", exact: true })
    .click();
  await expect(create.getByRole("alert")).toContainText(
    "Choose an available Default page",
  );
  await expect(create.getByLabel("Page name")).toHaveValue("My browser");
  await expect(
    create.getByRole("checkbox", { name: "Switch automatically", exact: true }),
  ).toBeChecked();
  await expect(saveButton(page)).toBeDisabled();
  await create
    .getByLabel("Default page", { exact: true })
    .selectOption("new-home");
  await create
    .getByRole("button", { name: "Create page", exact: true })
    .click();
  await expect(create).toHaveCount(0);
  await saveButton(page).click();
  const saved = await page.evaluate(() => (window as any).__saved());
  expect(saved.page_switching).toEqual({
    enabled: true,
    default_page: "new-home",
  });
  expect(
    saved.pages.find((item: any) => item.name === "My browser").application
      .name,
  ).toBe("Google Chrome");
});

test("creation can add a missing Default without stealing app mappings and can leave switching off", async ({
  page,
}) => {
  await fixture(page, false, false, "only-associated");
  const original = await page.evaluate(() => (window as any).__saved());
  await pageButton(page, "Add application").click();
  const create = page.getByRole("dialog", { name: "Create application page" });
  await create.getByRole("button", { name: /Example \/fixture/ }).click();
  await create
    .getByLabel("Default page", { exact: true })
    .selectOption({ label: "Create a new Default page" });
  await create
    .getByRole("checkbox", { name: "Switch automatically", exact: true })
    .uncheck();
  await create
    .getByRole("button", { name: "Create page", exact: true })
    .click();
  await expect(
    page.getByRole("checkbox", { name: /Automatic page switching/ }),
  ).not.toBeChecked();
  await saveButton(page).click();
  const saved = await page.evaluate(() => (window as any).__saved());
  expect(saved.page_switching.enabled).toBe(false);
  expect(saved.pages).toHaveLength(original.pages.length + 2);
  for (const oldPage of original.pages) {
    expect(saved.pages.find((item: any) => item.id === oldPage.id)).toEqual(
      oldPage,
    );
  }
  const fallback = saved.pages.find(
    (item: any) => item.id === saved.page_switching.default_page,
  );
  expect(fallback).toBeDefined();
  expect(fallback.application ?? null).toBeNull();
  expect(
    saved.pages.find((item: any) => item.name === "Main").application.name,
  ).toBe("Example");
});

test("Default transfer stays inside Default context and preserves bindings", async ({
  page,
}) => {
  await fixture(page);
  await editPage(page, "Chrome", "Google Chrome");
  await expect(pageButton(page, "Set as Default")).toHaveCount(0);
  await editPage(page, "Utilities", "Default");
  await usePageOption(page, "Set as Default");
  const dialog = page.getByRole("dialog");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(saveButton(page)).toBeDisabled();
  await usePageOption(page, "Set as Default");
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Set as Default", exact: true })
    .click();
  await saveButton(page).click();
  const saved = await page.evaluate(() => (window as any).__saved());
  expect(saved.page_switching.default_page).toBe("manual");
  expect(
    saved.pages.find((item: any) => item.id === "chrome").bindings.lcd1.title,
  ).toBe("New tab");
});

test("closing the association picker cancels an in-flight identity lookup", async ({
  page,
}) => {
  await fixture(page);
  await page.evaluate(() => {
    (window as any).__delayIdentity = true;
  });
  await pageButton(page, "Add application").click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: /Example \/fixture/ })
    .click();
  await pageButton(page, "Close app picker").click();
  await page.getByLabel("Label", { exact: true }).fill("Keep editing");
  await page.evaluate(() => (window as any).__finishIdentity());
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Keep editing",
  );
  await expect(pageTab(page, "Example")).toHaveCount(0);
});

test("deleting Default requires a named replacement and supports cancel", async ({
  page,
}) => {
  await fixture(page);
  await usePageOption(page, "Delete page");
  const dialog = page.getByRole("dialog", { name: "Delete page “Home”?" });
  const replacement = dialog.getByLabel("Replacement Default page");
  await expect(replacement).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: "Delete page", exact: true }),
  ).toBeDisabled();
  await replacement.selectOption("manual");
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(pageTab(page, "Home")).toHaveCount(1);
  await expect(saveButton(page)).toBeDisabled();
  await usePageOption(page, "Delete page");
  await replacement.selectOption("manual");
  await dialog
    .getByRole("button", { name: "Delete page", exact: true })
    .click();
  await expect(pageTab(page, "Home")).toHaveCount(0);
  await saveButton(page).click();
  expect(
    await page.evaluate(
      () => (window as any).__saved().page_switching.default_page,
    ),
  ).toBe("manual");
});

test("sleep, disconnect, failed detection and pending transfer do not claim a page is displayed", async ({
  page,
}) => {
  await fixture(page);
  await page.evaluate(() =>
    (window as any).__context({
      desired_page: "chrome",
      applied_page: "home",
      reason: "application",
    }),
  );
  await expect(page.getByText(/Updating to.*Chrome/)).toBeVisible();
  await page.evaluate(() => (window as any).__device({ sleeping: true }));
  await expect(page.getByText(/Sleeping.*Chrome/)).toBeVisible();
  await page.evaluate(() => (window as any).__device({ connected: false }));
  await expect(
    page.getByText(/Disconnected/, { exact: false }).last(),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).__context({
      error: "Application detection is unavailable in this session",
    }),
  );
  await expect(
    page.getByText("Application detection is unavailable in this session", {
      exact: true,
    }),
  ).toBeVisible();
});

test("application changes and save failures preserve pending association and binding edits", async ({
  page,
}) => {
  await fixture(page);
  await editPage(page, "Chrome", "Google Chrome");
  await usePageOption(page, "Move pages to Default");
  await page
    .getByRole("dialog", { name: "Move pages to Default?", exact: true })
    .getByRole("button", { name: "Move pages", exact: true })
    .click();
  await page.getByLabel("Label", { exact: true }).fill("Preserved edit");
  await page.evaluate(() => {
    (window as any).__saveFailure = true;
    (window as any).__context({
      desired_page: "code",
      applied_page: "code",
      reason: "application",
    });
  });
  await saveButton(page).click();
  await expect(page.getByRole("alert")).toContainText(
    "Configuration could not be saved",
  );
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Preserved edit",
  );
  await expect(applicationContext(page, "Google Chrome")).toHaveCount(0);
  await expect(applicationContext(page, "Default")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(saveButton(page)).toBeEnabled();
  expect(
    await page.evaluate(
      () => (window as any).__saved().pages[1].application.name,
    ),
  ).toBe("Google Chrome");
});

test("Agents overlay status follows confirmed display transfers in both directions", async ({
  page,
}) => {
  await fixture(page);
  const status = page.getByRole("status").filter({ hasText: "On device:" });
  await page.evaluate(() =>
    (window as any).__context({
      overlay: true,
      overlay_applied: false,
    }),
  );
  await expect(status).toContainText("On device: Default / Home");
  await expect(status).toContainText("Updating to Agents overlay");
  await page.evaluate(() =>
    (window as any).__context({ overlay_applied: true, applied_page: null }),
  );
  await expect(status).toContainText("On device: Agents overlay");
  await expect(status).not.toContainText("Updating to");
  await page.evaluate(() =>
    (window as any).__context({ overlay: false, desired_page: "chrome" }),
  );
  await expect(status).toContainText("On device: Agents overlay");
  await expect(status).toContainText("Updating to Google Chrome / Chrome");
  await page.evaluate(() =>
    (window as any).__context({
      overlay_applied: false,
      applied_page: "chrome",
    }),
  );
  await expect(status).toContainText("On device: Google Chrome / Chrome");
  await expect(status).not.toContainText("Updating to");
  await page.evaluate(() => {
    (window as any).__context({ overlay: true, overlay_applied: true });
    (window as any).__device({ connected: false });
  });
  await expect(status).toContainText("On device: Disconnected");
  await expect(status).not.toContainText("On device: Agents overlay");
  await expect(status).not.toContainText("Updating to");
});

test("foreign-platform associations stay intact and explain how to rebind", async ({
  page,
}) => {
  await fixture(page, false, true);
  await editPage(page, "Chrome", "Google Chrome");
  await expect(
    page.getByText(/This application association is inactive on macos/),
  ).toBeVisible();
  await applicationOptions(page);
  await expect(pageButton(page, "Change application")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(saveButton(page)).toBeDisabled();
  expect(
    await page.evaluate(
      () => (window as any).__saved().pages[1].application.identities,
    ),
  ).toEqual([{ platform: "windows", value: "C:/Example/chrome.exe" }]);
});

test("changing an association preserves keys, uses drafts and survives a delayed save", async ({
  page,
}) => {
  await fixture(page);
  await editPage(page, "Chrome", "Google Chrome");
  await usePageOption(page, "Change application");
  await page
    .getByRole("dialog")
    .getByRole("button", { name: /Example \/fixture/ })
    .click();
  await expect(applicationContext(page, "Example")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "New tab",
  );
  expect(
    await page.evaluate(
      () => (window as any).__saved().pages[1].application.name,
    ),
  ).toBe("Google Chrome");
  await page
    .getByRole("checkbox", { name: /Automatic page switching/ })
    .uncheck();
  await page.evaluate(() => {
    (window as any).__delaySave = true;
  });
  await saveButton(page).click();
  await page.getByLabel("Label", { exact: true }).fill("Newer browser label");
  await page.evaluate(() => {
    (window as any).__context({
      desired_page: "code",
      applied_page: "code",
      reason: "application",
    });
    (window as any).__finishSave();
  });
  await expect(
    page.getByText(/On device: Visual Studio Code \/ VS Code/),
  ).toBeVisible();
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Newer browser label",
  );
  await expect(saveButton(page)).toBeEnabled();
  expect(
    await page.evaluate(() => (window as any).__saved().page_switching.enabled),
  ).toBe(false);
  expect(
    await page.evaluate(
      () => (window as any).__saved().pages[1].application.name,
    ),
  ).toBe("Example");
  await page
    .getByRole("checkbox", { name: /Automatic page switching/ })
    .check();
  await page.evaluate(() => {
    (window as any).__delaySave = false;
  });
  await saveButton(page).click();
  await expect(saveButton(page)).toBeDisabled();
  expect(
    await page.evaluate(() => (window as any).__saved().page_switching.enabled),
  ).toBe(true);
});

test("device status keeps saved page names through unsaved rename and deletion", async ({
  page,
}) => {
  await fixture(page);
  await page.evaluate(() =>
    (window as any).__context({
      desired_page: "chrome",
      applied_page: "chrome",
      reason: "application",
    }),
  );
  await editPage(page, "Chrome", "Google Chrome");
  await usePageOption(page, "Rename page");
  let rename = page.getByRole("dialog", { name: "Rename page", exact: true });
  await rename.getByLabel("Page name").fill("Browser tools");
  await rename.getByLabel("Page name").press("Enter");
  await expect(
    page.getByText("Editing: Browser tools", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("On device: Google Chrome / Chrome", { exact: true }),
  ).toBeVisible();
  await editPage(page, "VS Code", "Visual Studio Code");
  await usePageOption(page, "Rename page");
  rename = page.getByRole("dialog", { name: "Rename page", exact: true });
  await rename.getByLabel("Page name").fill("Coding tools");
  await rename.getByLabel("Page name").press("Enter");
  await page.evaluate(() =>
    (window as any).__context({ desired_page: "code" }),
  );
  await expect(
    page.getByText(/Updating to Visual Studio Code \/ VS Code/),
  ).toBeVisible();
  await page.evaluate(() => (window as any).__device({ sleeping: true }));
  await expect(
    page.getByText(
      "On device: Sleeping — next page: Visual Studio Code / VS Code",
      { exact: true },
    ),
  ).toBeVisible();
  await usePageOption(page, "Delete page");
  await page
    .getByRole("dialog", { name: "Delete page “Coding tools”?" })
    .getByRole("button", { name: "Delete page", exact: true })
    .click();
  await expect(pageTab(page, "Coding tools")).toHaveCount(0);
  await expect(
    page.getByText(
      "On device: Sleeping — next page: Visual Studio Code / VS Code",
      { exact: true },
    ),
  ).toBeVisible();
  await page.evaluate(() => {
    (window as any).__device({ sleeping: false });
    (window as any).__context({ applied_page: "code" });
  });
  await expect(
    page.getByText("On device: Visual Studio Code / VS Code", { exact: true }),
  ).toBeVisible();
  await expect(saveButton(page)).toBeEnabled();
  expect(
    await page.evaluate(() =>
      (window as any).__saved().pages.map((item: any) => item.name),
    ),
  ).toEqual(["Home", "Chrome", "VS Code", "Utilities"]);
});

test("row headings distinguish applications and pages and align both rows", async ({
  page,
}, testInfo) => {
  await fixture(page);
  const navigation = page.getByRole("region", { name: "Page navigation" });
  const applications = navigation.getByRole("heading", {
    name: "Applications",
    exact: true,
  });
  const pages = navigation.getByRole("heading", {
    name: "Pages",
    exact: true,
  });
  for (const width of [1380, 1060]) {
    await page.setViewportSize({ width, height: 900 });
    await expect(applications).toBeVisible();
    await expect(pages).toBeVisible();
    const applicationBounds = (await applications.boundingBox())!;
    const pageBounds = (await pages.boundingBox())!;
    expect(applicationBounds.x).toBe(pageBounds.x);
    expect(applicationBounds.width).toBe(pageBounds.width);
    const contexts = (await navigation
      .getByRole("group", { name: "Application contexts" })
      .boundingBox())!;
    const tabs = (await navigation
      .getByRole("tablist", { name: "Pages" })
      .boundingBox())!;
    expect(contexts.x).toBe(tabs.x);
    expect(
      await applications.evaluate((element) => {
        const style = getComputedStyle(element);
        return [style.textTransform, style.fontWeight, style.borderRightWidth];
      }),
    ).toEqual(["uppercase", "700", "1px"]);
    expect(
      await applications.evaluate(
        (element) => element.scrollWidth <= element.clientWidth,
      ),
    ).toBe(true);
    await page.screenshot({
      path: testInfo.outputPath(`row-headings-${width}.png`),
      animations: "disabled",
      fullPage: true,
    });
  }
});

test("custom page names retain application identity in desktop and compact navigation", async ({
  page,
}, testInfo) => {
  await fixture(page);
  await editPage(page, "Chrome", "Google Chrome");
  await usePageOption(page, "Rename page");
  const rename = page.getByRole("dialog", { name: "Rename page", exact: true });
  await rename.getByLabel("Page name").fill("Browser tools");
  await rename.getByLabel("Page name").press("Enter");
  const tab = pageTab(page, "Browser tools");
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(applicationContext(page, "Google Chrome")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(
    page.getByText("Page renamed. Save changes to apply.", { exact: true }),
  ).toHaveCount(0);
  await page.screenshot({
    path: testInfo.outputPath("application-pages-desktop.png"),
    fullPage: true,
  });
  await page.setViewportSize({ width: 1060, height: 720 });
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(applicationContext(page, "Google Chrome")).toBeInViewport();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: testInfo.outputPath("application-pages-custom-name-narrow.png"),
    fullPage: true,
  });
});

test("direct tabs and role controls stay usable at the minimum window size", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  await fixture(page);
  await expect(page.getByRole("tablist", { name: "Pages" })).toBeVisible();
  await editPage(page, "Chrome", "Google Chrome");
  await expect(
    page.getByText("Editing: Chrome", { exact: true }),
  ).toBeVisible();
  await pageButton(page, "Add application").focus();
  await expect(pageButton(page, "Add application")).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("dialog", { name: "Create application page" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(pageButton(page, "Add application")).toBeFocused();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: testInfo.outputPath("application-pages-narrow.png"),
    fullPage: true,
  });
});

test("application subpages are created in their own context and remember local selection", async ({
  page,
}) => {
  await fixture(page);
  await editContext(page, "Google Chrome");
  await expect(pageTab(page, "Chrome")).toBeVisible();
  await expect(pageTab(page, "Home")).toHaveCount(0);
  await expect(pageTab(page, "VS Code")).toHaveCount(0);
  await pageButton(page, "Add page").click();
  await expect(pageTab(page, "Page 2")).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await page.getByLabel("Label", { exact: true }).fill("Browser second page");
  await editContext(page, "Default");
  await expect(pageTab(page, "Page 2")).toHaveCount(0);
  await expect(pageTab(page, "Home")).toBeVisible();
  await expect(pageTab(page, "Utilities")).toBeVisible();
  await editContext(page, "Google Chrome");
  await expect(pageTab(page, "Page 2")).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Browser second page",
  );
  await saveButton(page).click();
  const saved = await page.evaluate(() => (window as any).__saved());
  const siblings = saved.pages.filter(
    (item: any) => item.application?.name === "Google Chrome",
  );
  expect(siblings.map((item: any) => item.name)).toEqual(["Chrome", "Page 2"]);
  expect(siblings[1].application).toEqual(siblings[0].application);
  expect(
    saved.pages
      .filter((item: any) => !item.application)
      .map((item: any) => item.name),
  ).toEqual(["Home", "Utilities"]);
  expect(saved.active_page).toBe("home");
});

test("dragging and keyboard reorder save only the selected context and preserve failed drafts", async ({
  page,
}) => {
  await fixture(page, false, false, "existing", 2);
  const original = await page.evaluate(() => (window as any).__saved());
  await editPage(page, "Chrome", "Google Chrome");
  const source = pageTab(page, "Chrome");
  const target = pageTab(page, "Long application workspace 2");
  const box = await target.boundingBox();
  await source.dragTo(target, {
    targetPosition: { x: box!.width - 3, y: box!.height / 2 },
  });
  const tabs = page.getByRole("tablist", { name: "Pages" }).getByRole("tab");
  await expect
    .poll(async () =>
      tabs.evaluateAll((items) =>
        items.map((item) => item.getAttribute("aria-label")),
      ),
    )
    .toEqual([
      "Long application workspace 1",
      "Long application workspace 2",
      "Chrome",
    ]);
  await expect(source).toHaveAttribute("aria-selected", "true");
  expect(await page.evaluate(() => (window as any).__saved())).toEqual(
    original,
  );
  await source.focus();
  await page.keyboard.press("Alt+Shift+ArrowLeft");
  await expect(source).toBeFocused();
  await expect
    .poll(async () =>
      tabs.evaluateAll((items) =>
        items.map((item) => item.getAttribute("aria-label")),
      ),
    )
    .toEqual([
      "Long application workspace 1",
      "Chrome",
      "Long application workspace 2",
    ]);
  await page.keyboard.press("Alt+Shift+ArrowLeft");
  await expect
    .poll(async () =>
      tabs.evaluateAll((items) =>
        items.map((item) => item.getAttribute("aria-label")),
      ),
    )
    .toEqual([
      "Chrome",
      "Long application workspace 1",
      "Long application workspace 2",
    ]);
  await page.keyboard.press("Alt+Shift+ArrowRight");
  await page.evaluate(() => {
    (window as any).__saveFailure = true;
    (window as any).__context({
      desired_page: "code",
      applied_page: "code",
      reason: "application",
    });
  });
  await saveButton(page).click();
  await expect(page.getByRole("alert")).toContainText(
    "Configuration could not be saved",
  );
  await expect
    .poll(async () =>
      tabs.evaluateAll((items) =>
        items.map((item) => item.getAttribute("aria-label")),
      ),
    )
    .toEqual([
      "Long application workspace 1",
      "Chrome",
      "Long application workspace 2",
    ]);
  await page.evaluate(() => ((window as any).__saveFailure = false));
  await saveButton(page).click();
  const saved = await page.evaluate(() => (window as any).__saved());
  expect(
    saved.pages
      .filter((item: any) => item.application?.name === "Google Chrome")
      .map((item: any) => item.id),
  ).toEqual(["extra-1", "chrome", "extra-2"]);
  expect(
    saved.pages.filter(
      (item: any) => item.application?.name !== "Google Chrome",
    ),
  ).toEqual(
    original.pages.filter(
      (item: any) => item.application?.name !== "Google Chrome",
    ),
  );
  expect(
    saved.pages.find((item: any) => item.id === "chrome").bindings,
  ).toEqual(original.pages.find((item: any) => item.id === "chrome").bindings);
  expect(saved.page_switching.default_page).toBe("home");
  expect(saved.active_page).toBe("home");
});

test("application identity changes and removal affect all subpages together", async ({
  page,
}) => {
  await fixture(page, false, false, "existing", 2);
  await editPage(page, "Chrome", "Google Chrome");
  await usePageOption(page, "Change application");
  await page
    .getByRole("dialog")
    .getByRole("button", { name: /Example \/fixture/ })
    .click();
  await expect(applicationContext(page, "Google Chrome")).toHaveCount(0);
  await expect(applicationContext(page, "Example")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(
    page.getByRole("tablist", { name: "Pages" }).getByRole("tab"),
  ).toHaveCount(3);
  await saveButton(page).click();
  let saved = await page.evaluate(() => (window as any).__saved());
  expect(
    saved.pages.filter((item: any) => item.application?.name === "Example"),
  ).toHaveLength(3);
  await usePageOption(page, "Move pages to Default");
  await page
    .getByRole("dialog", { name: "Move pages to Default?", exact: true })
    .getByRole("button", { name: "Move pages", exact: true })
    .click();
  await expect(applicationContext(page, "Example")).toHaveCount(0);
  await expect(applicationContext(page, "Default")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(
    page.getByRole("tablist", { name: "Pages" }).getByRole("tab"),
  ).toHaveCount(5);
  await saveButton(page).click();
  saved = await page.evaluate(() => (window as any).__saved());
  expect(saved.pages.filter((item: any) => item.application)).toHaveLength(1);
  expect(
    saved.pages.find((item: any) => item.id === "chrome").bindings.lcd1.title,
  ).toBe("New tab");
});

test("Delete application cascades all its pages, protects Default and preserves drafts on failed saves", async ({
  page,
}, testInfo) => {
  await fixture(page, false, false, "existing", 2);
  const remove = pageButton(page, "Delete application");
  await expect(remove).toBeVisible();
  await expect(remove).toBeDisabled();
  await page.getByLabel("Label", { exact: true }).fill("Keep Default draft");
  const original = await page.evaluate(() => (window as any).__saved());
  await editPage(page, "Chrome", "Google Chrome");
  await expect(remove).toBeEnabled();
  await remove.click();
  const dialog = page.getByRole("dialog", {
    name: "Delete application “Google Chrome”?",
  });
  await expect(dialog).toContainText("all 3 pages");
  await expect(dialog.getByRole("listitem")).toHaveCount(3);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(remove).toBeFocused();
  await remove.click();
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(applicationContext(page, "Google Chrome")).toBeVisible();
  await remove.click();
  await page.screenshot({
    path: testInfo.outputPath("delete-application.png"),
    animations: "disabled",
    fullPage: true,
  });
  await dialog
    .getByRole("button", { name: "Delete application", exact: true })
    .click();
  await expect(applicationContext(page, "Google Chrome")).toHaveCount(0);
  await expect(applicationContext(page, "Default")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(remove).toBeDisabled();
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Keep Default draft",
  );
  expect(await page.evaluate(() => (window as any).__saved())).toEqual(
    original,
  );
  await page.evaluate(() => {
    (window as any).__saveFailure = true;
    (window as any).__replaceConfig({
      ...(window as any).__saved(),
      active_page: "extra-2",
      brightness: 30,
    });
  });
  await saveButton(page).click();
  await expect(page.getByRole("alert")).toContainText(
    "Configuration could not be saved",
  );
  await expect(applicationContext(page, "Google Chrome")).toHaveCount(0);
  await page.evaluate(() => ((window as any).__saveFailure = false));
  await saveButton(page).click();
  const saved = await page.evaluate(() => (window as any).__saved());
  expect(saved.pages.map((item: any) => item.id)).toEqual([
    "home",
    "code",
    "manual",
  ]);
  expect(saved.pages.find((item: any) => item.id === "code")).toEqual(
    original.pages.find((item: any) => item.id === "code"),
  );
  expect(saved.pages[0].bindings.lcd1.title).toBe("Keep Default draft");
  expect(saved.active_page).toBe("home");
  expect(saved.page_switching).toEqual(original.page_switching);
});

test("application deletion reports surviving links and refuses a stale context", async ({
  page,
}) => {
  await fixture(page, false, false, "existing", 1);
  await page.evaluate(() => {
    const config = structuredClone((window as any).__saved());
    const link = {
      ...config.pages[0].bindings.lcd1,
      kind: "page",
      value: "extra-1",
    };
    config.pages[0].bindings.lcd1 = link;
    config.pages.find((item: any) => item.id === "chrome").bindings.lcd2 = link;
    (window as any).__replaceConfig(config);
  });
  await editPage(page, "Chrome", "Google Chrome");
  await pageButton(page, "Delete application").click();
  const dialog = page.getByRole("dialog", {
    name: "Delete application “Google Chrome”?",
  });
  await expect(dialog).toContainText("1 action links");
  await page.evaluate(() => {
    const config = structuredClone((window as any).__saved());
    config.pages = config.pages.filter(
      (item: any) => item.application?.name !== "Google Chrome",
    );
    (window as any).__replaceConfig(config);
  });
  await expect(dialog).toContainText("This application no longer exists");
  await expect(
    dialog.getByRole("button", { name: "Delete application", exact: true }),
  ).toBeDisabled();
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(applicationContext(page, "Visual Studio Code")).toBeVisible();
});

test("deleting the final application page removes its context and protects the final Default page", async ({
  page,
}) => {
  await fixture(page);
  await editPage(page, "Chrome", "Google Chrome");
  const remove = pageButton(page, "Delete page");
  await expect(remove).toBeVisible();
  const color = await remove.evaluate(
    (element) => getComputedStyle(element).color,
  );
  const channels = color.match(/\d+/g)!.map(Number);
  expect(channels[0]).toBeGreaterThan(channels[1]);
  expect(channels[0]).toBeGreaterThan(channels[2]);
  await remove.click();
  const dialog = page.getByRole("dialog", { name: "Delete page “Chrome”?" });
  await expect(dialog).toContainText("Google Chrome");
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(applicationContext(page, "Google Chrome")).toBeVisible();
  await remove.click();
  await dialog
    .getByRole("button", { name: "Delete page", exact: true })
    .click();
  await expect(applicationContext(page, "Google Chrome")).toHaveCount(0);
  await expect(applicationContext(page, "Default")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await editPage(page, "Utilities");
  await remove.click();
  await page
    .getByRole("dialog", { name: "Delete page “Utilities”?" })
    .getByRole("button", { name: "Delete page", exact: true })
    .click();
  await expect(pageTab(page, "Home")).toHaveAttribute("aria-selected", "true");
  await expect(remove).toBeDisabled();
  await expect(applicationContext(page, "Visual Studio Code")).toBeVisible();
});

test("overflow subpages remain separate from contexts and keyboard editing preserves drafts", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  await fixture(page, false, false, "existing", 17);
  await editPage(page, "Chrome", "Google Chrome");
  const tabs = page.getByRole("tablist", { name: "Pages" });
  await expect(tabs.getByRole("tab")).toHaveCount(18);
  await expect(pageTab(page, "Home")).toHaveCount(0);
  await page.getByLabel("Label", { exact: true }).fill("Keep browser draft");
  await pageTab(page, "Chrome").focus();
  await page.keyboard.press("End");
  await expect(pageTab(page, "Long application workspace 17")).toBeFocused();
  await expect(pageTab(page, "Long application workspace 17")).toBeInViewport({
    ratio: 0.95,
  });
  await expect(applicationContext(page, "Default")).toBeInViewport();
  for (const name of [
    "Add application",
    "Add page",
    "Delete page",
    "Rename page",
  ])
    await expect(pageButton(page, name)).toBeInViewport();
  await page.screenshot({
    path: testInfo.outputPath("application-subpages-overflow.png"),
    fullPage: true,
  });
  await page.keyboard.press("Home");
  await expect(pageTab(page, "Chrome")).toBeFocused();
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Keep browser draft",
  );
  await editContext(page, "Default");
  await expect(tabs.getByRole("tab")).toHaveCount(2);
  await editContext(page, "Google Chrome");
  await expect(pageTab(page, "Chrome")).toHaveAttribute(
    "aria-selected",
    "true",
  );
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  expect(
    await page.evaluate(() =>
      (window as any).__calls.filter((call: any) =>
        ["save_config", "select_page"].includes(call.command),
      ),
    ),
  ).toEqual([]);
});

test("same-named subpages remain distinct across application contexts", async ({
  page,
}, testInfo) => {
  await fixture(page, false, false, "existing", 2);
  await page.evaluate(() => {
    const next = structuredClone((window as any).__saved());
    next.pages.find((item: any) => item.id === "chrome").name = "Main";
    next.pages.find((item: any) => item.id === "code").name = "Main";
    const development = next.pages.find((item: any) => item.id === "extra-1");
    development.name = "Development";
    development.bindings = structuredClone(
      next.pages.find((item: any) => item.id === "chrome").bindings,
    );
    development.bindings.lcd1.title = "Developer tools";
    development.bindings.lcd1.value = "meta+alt+i";
    next.pages.find((item: any) => item.id === "extra-2").name = "Media";
    (window as any).__replaceConfig(next);
  });
  await editPage(page, "Main", "Google Chrome");
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "New tab",
  );
  await page.getByLabel("Label", { exact: true }).fill("Chrome draft");
  await editPage(page, "Main", "Visual Studio Code");
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Command palette",
  );
  await page.evaluate(() =>
    (window as any).__context({
      desired_page: "chrome",
      applied_page: "chrome",
      reason: "application",
    }),
  );
  await expect(
    page.getByText("On device: Google Chrome / Main", { exact: true }),
  ).toBeVisible();
  await expect(applicationContext(page, "Visual Studio Code")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await editContext(page, "Google Chrome");
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Chrome draft",
  );
  await saveButton(page).click();
  await expect(page.getByText("Changes saved", { exact: true })).toHaveCount(0);
  await editPage(page, "Development");
  await expect(
    page.getByRole("tablist", { name: "Pages" }).getByRole("tab"),
  ).toHaveCount(3);
  await page.screenshot({
    path: testInfo.outputPath("application-subpages-desktop.png"),
    fullPage: true,
    animations: "disabled",
  });
  await page.setViewportSize({ width: 1060, height: 720 });
  await expect(pageTab(page, "Main")).toBeInViewport();
  await expect(pageTab(page, "Development")).toBeInViewport();
  await expect(pageTab(page, "Media")).toBeInViewport();
  await page.screenshot({
    path: testInfo.outputPath("application-subpages-narrow.png"),
    fullPage: true,
    animations: "disabled",
  });
});

test("subpage overflow controls recover on resize and foreground events keep scroll position", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1800, height: 900 });
  await fixture(page, false, false, "existing", 5);
  await editContext(page, "Google Chrome");
  const more = pageButton(page, "Show more pages");
  await expect(more).toHaveCount(0);
  await page.setViewportSize({ width: 1060, height: 720 });
  await expect(more).toBeVisible();
  await more.click();
  await expect(pageButton(page, "Show previous pages")).toBeEnabled();
  const position = await pageTab(page, "Chrome").boundingBox();
  await page.evaluate(() =>
    (window as any).__context({
      desired_page: "code",
      applied_page: "code",
      reason: "application",
    }),
  );
  await expect(
    page.getByText("On device: Visual Studio Code / VS Code", { exact: true }),
  ).toBeVisible();
  await expect
    .poll(async () => (await pageTab(page, "Chrome").boundingBox())!.x)
    .toBeCloseTo(position!.x, 0);
  await page.setViewportSize({ width: 1800, height: 900 });
  await expect(more).toHaveCount(0);
  await expect(pageButton(page, "Show previous pages")).toHaveCount(0);
});

test("newly added subpage stays visible when its row first overflows", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  await fixture(page);
  await editContext(page, "Google Chrome");
  const more = pageButton(page, "Show more pages");
  await expect(more).toHaveCount(0);
  for (let index = 2; index <= 15; index++) {
    await pageButton(page, "Add page").click();
    const tab = pageTab(page, `Page ${index}`);
    await expect(tab).toHaveAttribute("aria-selected", "true");
    await expect(tab).toBeInViewport({ ratio: 0.99 });
    if (await more.count()) break;
  }
  await expect(more).toBeVisible();
  await expect(applicationContext(page, "Google Chrome")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(
    page.getByText("On device: Default / Home", { exact: true }),
  ).toBeVisible();
});

test("deleting the first application subpage keeps application context order stable", async ({
  page,
}) => {
  await fixture(page, false, false, "existing", 2);
  const choices = page
    .getByRole("group", { name: "Application contexts", exact: true })
    .locator("button[aria-pressed]");
  const before = await choices.evaluateAll((items) =>
    items.map((item) => item.getAttribute("aria-label")),
  );
  await editPage(page, "Chrome", "Google Chrome");
  await pageButton(page, "Delete page").click();
  await page
    .getByRole("dialog", { name: "Delete page “Chrome”?" })
    .getByRole("button", { name: "Delete page", exact: true })
    .click();
  await expect(pageTab(page, "Long application workspace 1")).toHaveAttribute(
    "aria-selected",
    "true",
  );
  expect(
    await choices.evaluateAll((items) =>
      items.map((item) => item.getAttribute("aria-label")),
    ),
  ).toEqual(before);
});

test("many application contexts scroll independently of the subpage row", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  await fixture(page);
  await page.evaluate(() => {
    const next = structuredClone((window as any).__saved());
    for (let index = 1; index <= 14; index++)
      next.pages.push({
        id: `context-${index}`,
        name: "Main",
        application: {
          name: `Workspace application ${String(index).padStart(2, "0")}`,
          identities: [
            { platform: "macos", value: `com.fixture.context${index}` },
          ],
        },
        bindings: {},
      });
    (window as any).__replaceConfig(next);
  });
  await expect(pageButton(page, "Show more applications")).toBeVisible();
  await expect(pageButton(page, "Show more pages")).toHaveCount(0);
  await editContext(page, "Workspace application 14");
  await expect(
    applicationContext(page, "Workspace application 14"),
  ).toBeInViewport({ ratio: 0.99 });
  await expect(applicationContext(page, "Default")).toBeInViewport({
    ratio: 0.99,
  });
  await expect(pageTab(page, "Main")).toHaveAttribute("aria-selected", "true");
  await expect(pageButton(page, "Show previous applications")).toBeEnabled();
  await expect(pageButton(page, "Show more pages")).toHaveCount(0);
  await page.screenshot({
    path: testInfo.outputPath("application-contexts-overflow.png"),
    fullPage: true,
  });
  await editContext(page, "Default");
  await expect(pageTab(page, "Home")).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByText("On device: Default / Home", { exact: true }),
  ).toBeVisible();
});

test("dragging at an overflowing page rail edge scrolls without moving application context", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  await fixture(page, false, false, "existing", 17);
  await editContext(page, "Google Chrome");
  const tab = pageTab(page, "Chrome");
  const source = (await tab.boundingBox())!;
  const rail = page
    .getByRole("tablist", { name: "Pages" })
    .locator(".context-tab-scroll");
  const bounds = (await rail.boundingBox())!;
  await page.mouse.move(
    source.x + source.width / 2,
    source.y + source.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    source.x + source.width / 2 + 12,
    source.y + source.height / 2,
    { steps: 3 },
  );
  for (let index = 0; index < 6; index++)
    await page.mouse.move(
      bounds.x + bounds.width - 5 - (index % 2),
      bounds.y + bounds.height / 2,
      { steps: 2 },
    );
  await expect
    .poll(() => rail.evaluate((element) => element.scrollLeft))
    .toBeGreaterThan(0);
  await page.keyboard.press("Escape");
  await page.mouse.up();
  await expect(applicationContext(page, "Google Chrome")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(saveButton(page)).toBeDisabled();
});

test("application lookup applies to the latest configuration without resurrecting a removed context", async ({
  page,
}) => {
  await fixture(page, false, false, "existing", 1);
  await editContext(page, "Google Chrome");
  await usePageOption(page, "Change application");
  await page.evaluate(() => ((window as any).__delayIdentity = true));
  await page.getByRole("button", { name: /Example.*fixture/ }).click();
  await page.evaluate(() => {
    const next = structuredClone((window as any).__saved());
    next.brightness = 37;
    next.active_page = "manual";
    next.pages.push({ id: "incoming", name: "Incoming", bindings: {} });
    (window as any).__replaceConfig(next);
  });
  await expect(pageTab(page, "Chrome")).toBeVisible();
  await page.evaluate(() => (window as any).__finishIdentity());
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(applicationContext(page, "Example")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await saveButton(page).click();
  const saved = await page.evaluate(() => (window as any).__saved());
  expect(saved.brightness).toBe(37);
  expect(saved.active_page).toBe("manual");
  expect(saved.pages.some((item: any) => item.id === "incoming")).toBe(true);
  expect(
    saved.pages.filter((item: any) => item.application?.name === "Example"),
  ).toHaveLength(2);

  await usePageOption(page, "Change application");
  await page.getByRole("button", { name: /Google Chrome.*fixture/ }).click();
  await page.evaluate(() => {
    const next = structuredClone((window as any).__saved());
    next.pages = next.pages.filter(
      (item: any) => item.application?.name !== "Example",
    );
    (window as any).__replaceConfig(next);
  });
  await expect(applicationContext(page, "Example")).toHaveCount(0);
  await page.evaluate(() => (window as any).__finishIdentity());
  const picker = page.getByRole("dialog", { name: "Choose application" });
  await expect(picker.getByRole("alert")).toContainText(
    "This page no longer exists",
  );
  await expect(applicationContext(page, "Google Chrome")).toHaveCount(0);
  await expect(saveButton(page)).toBeDisabled();
});

async function dragPageToPoint(
  page: Page,
  name: string,
  point: { x: number; y: number },
) {
  const source = (await pageTab(page, name).boundingBox())!;
  await page.mouse.move(
    source.x + source.width / 2,
    source.y + source.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    source.x + source.width / 2 + 12,
    source.y + source.height / 2,
    { steps: 3 },
  );
  await page.mouse.move(point.x, point.y, { steps: 8 });
  await page.mouse.move(point.x + 1, point.y, { steps: 2 });
  await page.mouse.up();
}

async function pageOrder(page: Page) {
  return page
    .getByRole("tablist", { name: "Pages" })
    .getByRole("tab")
    .evaluateAll((items) =>
      items.map((item) => item.getAttribute("aria-label")),
    );
}

test("page drops accept actual leading space, inter-tab gaps and trailing space", async ({
  page,
}) => {
  await fixture(page, false, false, "existing", 2);
  await page.evaluate(() => {
    const next = structuredClone((window as any).__saved());
    next.pages.find((item: any) => item.id === "extra-1").name = "Second";
    next.pages.find((item: any) => item.id === "extra-2").name = "Third";
    (window as any).__replaceConfig(next);
  });
  await editContext(page, "Google Chrome");
  let first = (await pageTab(page, "Chrome").boundingBox())!;
  await dragPageToPoint(page, "Third", {
    x: first.x - 6,
    y: first.y + first.height / 2,
  });
  await expect
    .poll(() => pageOrder(page))
    .toEqual(["Third", "Chrome", "Second"]);
  first = (await pageTab(page, "Third").boundingBox())!;
  const middle = (await pageTab(page, "Chrome").boundingBox())!;
  await dragPageToPoint(page, "Second", {
    x: (first.x + first.width + middle.x) / 2,
    y: first.y + first.height / 2,
  });
  await expect
    .poll(() => pageOrder(page))
    .toEqual(["Third", "Second", "Chrome"]);
  const last = (await pageTab(page, "Chrome").boundingBox())!;
  await dragPageToPoint(page, "Third", {
    x: last.x + last.width + 12,
    y: last.y + last.height / 2,
  });
  await expect
    .poll(() => pageOrder(page))
    .toEqual(["Second", "Chrome", "Third"]);
  await saveButton(page).click();
  expect(
    (await page.evaluate(() => (window as any).__saved())).pages
      .filter((item: any) => item.application?.name === "Google Chrome")
      .map((item: any) => item.name),
  ).toEqual(["Second", "Chrome", "Third"]);
});

test("management controls and foreign-context page drops do not change order", async ({
  page,
}) => {
  await fixture(page, false, false, "existing", 1);
  await editContext(page, "Google Chrome");
  const original = await pageOrder(page);
  const add = (await pageButton(page, "Add page").boundingBox())!;
  await dragPageToPoint(page, "Chrome", {
    x: add.x + add.width / 2,
    y: add.y + add.height / 2,
  });
  expect(await pageOrder(page)).toEqual(original);
  await expect(saveButton(page)).toBeDisabled();
  const transfer = await page.evaluateHandle(() => {
    const data = new DataTransfer();
    data.setData("application/x-openstreamdeck-page", "home");
    return data;
  });
  await page
    .locator(".page-navigation-toolbar")
    .dispatchEvent("drop", { dataTransfer: transfer, clientX: 0 });
  expect(await pageOrder(page)).toEqual(original);
  await expect(saveButton(page)).toBeDisabled();
});

test("dropping in the outer leading space of an overflowed row moves to the actual first position", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  await fixture(page, false, false, "existing", 17);
  await editContext(page, "Google Chrome");
  await pageTab(page, "Chrome").focus();
  await page.keyboard.press("End");
  const source = pageTab(page, "Long application workspace 17");
  await expect(source).toBeInViewport({ ratio: 0.95 });
  const heading = (await page
    .locator(".page-navigation-toolbar .context-row-label")
    .boundingBox())!;
  await dragPageToPoint(page, "Long application workspace 17", {
    x: heading.x + heading.width - 3,
    y: heading.y + heading.height / 2,
  });
  await expect
    .poll(async () => (await pageOrder(page))[0])
    .toBe("Long application workspace 17");
  await expect(source).toHaveAttribute("aria-selected", "true");
});

test("an imported application page named new can change its application", async ({
  page,
}) => {
  await fixture(page);
  await page.evaluate(() => {
    const config = structuredClone((window as any).__saved());
    config.pages.find((item: any) => item.id === "chrome").id = "new";
    (window as any).__replaceConfig(config);
  });
  await editContext(page, "Google Chrome");
  await usePageOption(page, "Change application");
  await expect(
    page.getByRole("dialog", { name: "Choose application" }),
  ).toBeVisible();
  await page.getByRole("button", { name: /Example.*fixture/ }).click();
  await expect(applicationContext(page, "Example")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await saveButton(page).click();
  const saved = await page.evaluate(() => (window as any).__saved());
  expect(saved.pages).toHaveLength(4);
  expect(
    saved.pages.find((item: any) => item.id === "new").application.name,
  ).toBe("Example");
});
