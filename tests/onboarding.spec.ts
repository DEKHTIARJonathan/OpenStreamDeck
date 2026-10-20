import { expect, test } from "@playwright/test";

const fixture = `
import { defaultConfig } from '/src/model.ts';
export const native = true;
let config = defaultConfig();
const options = new URLSearchParams(location.search);
config.launch_at_login = options.has('login');
if(options.has('window')) config.menu_bar_only = false;
let navigation = {view:'Deck',revision:0};
let completed = localStorage.getItem('setup-complete') === 'yes';
let platform = new URLSearchParams(location.search).get('os') || 'macos';
let access = new URLSearchParams(location.search).get('access') || (platform === 'macos' ? 'required' : 'not_required');
const listeners = new Map();
export async function on(event, callback) {
  const handler = value => callback(value);
  if(!listeners.has(event)) listeners.set(event,new Set());
  const handlers = listeners.get(event); handlers.add(handler);
  return () => handlers.delete(handler);
}
function emit(event,value) { for(const handler of listeners.get(event) || []) handler(value); }
window.__setupCalls = [];
window.__setupError = '';
window.__loginError = options.has('login-error') ? 'Login registration unavailable' : null;
window.__setLogin = enabled => { config = {...config, launch_at_login:enabled}; window.dispatchEvent(new Event('focus')); };
window.__grant = () => { access = 'granted'; window.dispatchEvent(new Event('focus')); };
window.__updateNotice = () => emit('update_status',{revision:1,phase:'available',current_version:'0.1.0',version:'0.2.0',prompt_version:'0.2.0'});
export async function call(command,args) {
  if(command === 'snapshot') return {onboarding:!completed,platform,navigation,config,device:{connected:false,message:'Fixture'},agents:[],adapter_errors:[],data_dir:'/fixture'};
  if(command === 'render_keys') return [];
  if(command === 'set_view') return navigation={view:args.view,revision:navigation.revision+1};
  window.__setupCalls.push({command,...args});
  if(command === 'onboarding_status') return {completed,platform,input_access:access,launch_at_login:config.launch_at_login,login_error:window.__loginError};
  if(command === 'open_accessibility_settings') {
    if(window.__setupError) throw new Error(window.__setupError);
    return;
  }
  if(command === 'finish_onboarding') {
    if(window.__failAfterLogin) { config = {...config, launch_at_login:true}; emit('config',config); throw new Error('Could not remember setup'); }
    if(window.__setupError) throw new Error(window.__setupError);
    if(args.enableLogin) { config = {...config, launch_at_login:true}; emit('config',config); }
    completed = true; localStorage.setItem('setup-complete','yes'); return;
  }
  if(command === 'save_config') {config=args.config;emit('config',config);return;}
  if(command === 'skip_update') { const update={revision:2,phase:'available',current_version:'0.1.0',version:'0.2.0',prompt_version:null};emit('update_status',update);return update; }
  if(command === 'update_status') return {revision:0,phase:'idle',current_version:'0.1.0',prompt_version:null};
  throw new Error('Unexpected command: '+command);
}
`;

test.beforeEach(async ({ page }) => {
  await page.route(/\/src\/api\.ts(?:\?.*)?$/, (route) =>
    route.fulfill({ contentType: "application/javascript", body: fixture }),
  );
});

test("macOS requires verified access, rechecks on return, and saves explicit startup choice", async ({
  page,
}) => {
  await page.goto("/");
  const setup = page.getByRole("dialog", {
    name: "Give your controls a voice.",
  });
  await expect(setup).toBeVisible();
  await expect(setup.getByRole("heading")).toBeFocused();
  await expect(setup.getByText(/Skip this step and volume/)).toBeVisible();
  await setup
    .getByRole("button", { name: "Open System Settings", exact: true })
    .click();
  await expect(
    setup.getByText("Accessibility access needed", { exact: true }),
  ).toBeVisible();
  await setup.getByRole("button", { name: "Check again" }).click();
  await expect(
    setup.getByText("Accessibility access needed", { exact: true }),
  ).toBeVisible();
  await page.evaluate(() => (window as any).__grant());
  await expect(
    setup.getByText("Accessibility access enabled", { exact: true }),
  ).toBeVisible();
  await setup.getByRole("button", { name: "Continue" }).click();
  const routine = page.getByRole("dialog", { name: "Ready when you are." });
  await expect(routine.getByRole("heading")).toBeFocused();
  await expect(routine.getByText(/Starts in the menu bar/)).toBeVisible();
  await routine.getByRole("button", { name: "Enable & finish" }).click();
  await expect(routine).not.toBeVisible();
  await expect(
    page.getByRole("button", { name: "Deck", exact: true }),
  ).toBeFocused();
  expect(
    await page.evaluate(() =>
      (window as any).__setupCalls.filter(
        (c: any) => c.command === "finish_onboarding",
      ),
    ),
  ).toEqual([{ command: "finish_onboarding", enableLogin: true }]);
  await page.reload();
  await expect(page.getByRole("dialog")).not.toBeVisible();
});

for (const os of ["windows", "linux"]) {
  test(`${os} shows platform guidance and skips without requesting macOS access`, async ({
    page,
  }) => {
    await page.goto(`/?os=${os}`);
    const setup = page.getByRole("dialog");
    await expect(
      setup.getByRole("heading", { name: "Make every control count." }),
    ).toBeVisible();
    await expect(
      setup.getByRole("button", { name: "Open System Settings" }),
    ).toHaveCount(0);
    if (os === "linux")
      await expect(setup.getByText(/udev rule/)).toBeVisible();
    await setup.getByRole("button", { name: "Continue" }).click();
    await expect(
      setup.getByText(/open OpenStreamDeck before using your deck/),
    ).toBeVisible();
    await setup.getByRole("button", { name: "Not now" }).click();
    await expect(setup).not.toBeVisible();
    expect(
      await page.evaluate(() =>
        (window as any).__setupCalls.some(
          (c: any) => c.command === "open_accessibility_settings",
        ),
      ),
    ).toBe(false);
    expect(
      await page.evaluate(
        () =>
          (window as any).__setupCalls.find(
            (c: any) => c.command === "finish_onboarding",
          ).enableLogin,
      ),
    ).toBe(false);
    await page.reload();
    await expect(page.getByRole("dialog")).not.toBeVisible();
  });
}

test("Wayland limitations are explicit", async ({ page }) => {
  await page.goto("/?os=linux&access=limited");
  await expect(
    page.getByText("Wayland input is limited", { exact: true }),
  ).toBeVisible();
  await expect(page.getByText(/may not reach Wayland apps/)).toBeVisible();
});

test("interrupted setup resumes; Escape skips each step and remembers completion", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole("dialog", { name: "Give your controls a voice." }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("dialog", { name: "Ready when you are." }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await page.reload();
  await expect(page.getByRole("dialog")).not.toBeVisible();
});

test("permission and completion failures stay visible and can be retried", async ({
  page,
}) => {
  await page.goto("/");
  const setup = page.getByRole("dialog");
  await page.evaluate(() => {
    (window as any).__setupError = "OS settings unavailable";
  });
  await setup
    .getByRole("button", { name: "Open System Settings", exact: true })
    .click();
  await expect(setup.getByRole("alert")).toContainText(
    "OS settings unavailable",
  );
  await setup.getByRole("button", { name: "Skip for now" }).click();
  await page.evaluate(() => {
    (window as any).__setupError = "Could not remember setup";
  });
  await setup.getByRole("button", { name: "Enable & finish" }).click();
  await expect(setup.getByRole("alert")).toContainText(
    "Could not remember setup",
  );
  await expect(
    setup.getByRole("heading", { name: "Ready when you are." }),
  ).toBeVisible();
  await page.evaluate(() => {
    (window as any).__setupError = "";
  });
  await setup.getByRole("button", { name: "Not now" }).click();
  await expect(setup).not.toBeVisible();
});

test("global skip remembers the choice without enabling startup", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Skip setup", exact: true }).click();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as any).__setupCalls.find(
          (c: any) => c.command === "finish_onboarding",
        ).enableLogin,
    ),
  ).toBe(false);
  await page.reload();
  await expect(page.getByRole("dialog")).not.toBeVisible();
});

test("setup contains focus, ignores backdrop, fits compact screens and yields to update notices", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  await page.goto("/");
  const setup = page.getByRole("dialog", {
    name: "Give your controls a voice.",
  });
  await expect(setup).toBeVisible();
  await setup
    .getByRole("button", { name: "Open System Settings", exact: true })
    .focus();
  await page.keyboard.press("Tab");
  await expect(
    setup.getByRole("button", { name: "Skip setup", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(
    setup.getByRole("button", { name: "Open System Settings", exact: true }),
  ).toBeFocused();
  await page.mouse.click(5, 5);
  await expect(setup).toBeVisible();
  await page.evaluate(() => (window as any).__updateNotice());
  await expect(setup).not.toBeVisible();
  const update = page.getByRole("dialog", {
    name: "Update available",
    exact: true,
  });
  await expect(update).toBeVisible();
  await update.getByRole("button", { name: "Skip this Update" }).click();
  await expect(setup).toBeVisible();
  await setup.getByRole("button", { name: "Skip for now" }).click();
  await page.setViewportSize({ width: 600, height: 520 });
  await expect(
    page.getByRole("button", { name: "Enable & finish" }),
  ).toBeInViewport();
  await expect(page.getByRole("button", { name: "Not now" })).toBeInViewport();
});

test("existing login registration is preserved and window-mode copy is accurate", async ({
  page,
}) => {
  await page.goto("/?os=windows&login&window");
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  const setup = page.getByRole("dialog");
  await expect(
    setup.getByText("Launch at login is enabled", { exact: true }),
  ).toBeVisible();
  await expect(
    setup.getByText(/Launch at login is already enabled/),
  ).toBeVisible();
  await expect(
    setup.getByText(/Opens the editor when you sign in/),
  ).toBeVisible();
  await setup.getByRole("button", { name: "Finish", exact: true }).click();
  expect(
    await page.evaluate(
      () =>
        (window as any).__setupCalls.find(
          (c: any) => c.command === "finish_onboarding",
        ).enableLogin,
    ),
  ).toBe(false);
});

test("unavailable login adapter remains optional", async ({ page }) => {
  await page.goto("/?os=linux&login-error");
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  const setup = page.getByRole("dialog");
  await expect(
    setup.getByText("Login registration unavailable", { exact: true }),
  ).toBeVisible();
  await setup.getByRole("button", { name: "Not now", exact: true }).click();
  await expect(setup).not.toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as any).__setupCalls.find(
          (c: any) => c.command === "finish_onboarding",
        ).enableLogin,
    ),
  ).toBe(false);
});

test("login registration failure can recover without implicitly enabling startup", async ({
  page,
}) => {
  await page.goto("/?os=linux&login-error");
  const setup = page.getByRole("dialog");
  await setup.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(
    setup.getByText("Login registration unavailable", { exact: true }),
  ).toBeVisible();
  await page.evaluate(() => {
    (window as any).__loginError = null;
  });
  await setup.getByRole("button", { name: "Try again", exact: true }).click();
  await expect(
    setup.getByRole("button", { name: "Enable & finish", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as any).__setupCalls.some(
        (call: any) => call.command === "finish_onboarding",
      ),
    ),
  ).toBe(false);
  await setup.getByRole("button", { name: "Not now", exact: true }).click();
  await expect(setup).not.toBeVisible();
});

for (const os of ["macos", "windows", "linux"]) {
  test(`${os} rechecks login changes when returning to automatic startup`, async ({
    page,
  }) => {
    await page.goto(`/?os=${os}`);
    const setup = page.getByRole("dialog");
    await setup
      .getByRole("button", { name: "Skip for now", exact: true })
      .click();
    await expect(
      setup.getByRole("button", { name: "Enable & finish", exact: true }),
    ).toBeVisible();
    await page.evaluate(() => (window as any).__setLogin(true));
    await expect(
      setup.getByText("Launch at login is enabled", { exact: true }),
    ).toBeVisible();
    await expect(
      setup.getByRole("button", { name: "Finish", exact: true }),
    ).toBeVisible();
    await page.evaluate(() => (window as any).__setLogin(false));
    await expect(
      setup.getByRole("button", { name: "Enable & finish", exact: true }),
    ).toBeVisible();
  });
}

test("saved login state is refreshed when remembering completion fails", async ({
  page,
}) => {
  await page.goto("/?os=windows");
  const setup = page.getByRole("dialog");
  await setup.getByRole("button", { name: "Continue", exact: true }).click();
  await page.evaluate(() => {
    (window as any).__failAfterLogin = true;
  });
  await setup.getByRole("button", { name: "Enable & finish" }).click();
  await expect(setup.getByRole("alert")).toContainText(
    "Could not remember setup",
  );
  await expect(
    setup.getByText("Launch at login is enabled", { exact: true }),
  ).toBeVisible();
  await expect(
    setup.getByText(/Launch at login is already enabled/),
  ).toBeVisible();
  await page.evaluate(() => {
    (window as any).__failAfterLogin = false;
  });
  await setup.getByRole("button", { name: "Finish", exact: true }).click();
  await expect(setup).not.toBeVisible();
});
