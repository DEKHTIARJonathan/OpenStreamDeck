import { test, expect } from "@playwright/test";

const fixture = `
  import { defaultConfig } from '/src/model.ts';
  export const native = true;
  let config = defaultConfig();
  let navigation = {view:'Settings',revision:0,...window.__restoreNavigation};
  const listeners = new Map();
  let update = {revision:0,phase:'idle',current_version:'0.1.0',version:null,notes:null,downloaded:0,total:null,error:null,install_disabled:null,last_checked_at:null,history_error:null,prompt_version:null,...window.__restoreUpdate};
  export async function on(event, callback) {
    const handler = value => callback(value);
    if (!listeners.has(event)) listeners.set(event, new Set());
    const handlers = listeners.get(event); handlers.add(handler);
    return () => handlers.delete(handler);
  }
  function emit(event, value) { for (const handler of listeners.get(event) ?? []) handler(value); }
  function publish(value) { update = {...update,...value,revision:update.revision+1}; emit('update_status',update); return update; }
  window.__update = publish;
  window.__autoUpdate = version => publish({phase:'available',version,prompt_version:version,last_checked_at:Date.now(),error:null});
  window.__skipError = '';
  window.__releaseLinkError = '';
  window.__updateCalls = [];
  window.__updateError = '';
  window.__installError = '';
  window.__restartError = '';
  window.__restartCalls = 0;
  window.__installerCalls = 0;
  window.__holdDownload = false;
  window.__noUpdate = false;
  window.__busyWorkflow = false;
  export async function call(command, args) {
    if (command === 'snapshot') return {navigation,config,update,device:{connected:false,message:'Fixture'},agents:[],adapter_errors:[],data_dir:'/fixture'};
    if (command === 'render_keys') return [];
    if (command === 'set_view') return navigation={view:args.view,revision:navigation.revision+1};
    if (command === 'save_config') {config=args.config;emit('config',config);return;}
    if (command === 'update_status') {
      const snapshot = {...update};
      if (window.__slowStatus) {
        setTimeout(()=>publish({phase:'downloading',downloaded:524288,total:1048576}),5);
        await new Promise(resolve=>setTimeout(resolve,80));
      }
      return snapshot;
    }
    window.__updateCalls.push({command,...args});
    if (command === 'open_release_link') {
      if (window.__releaseLinkError) throw new Error(window.__releaseLinkError);
      return;
    }
    if (command === 'check_updates') {
      publish({phase:'checking',error:null,version:null,prompt_version:null});
      return publish({phase:window.__updateError ? 'error' : window.__noUpdate ? 'current' : 'available',error:window.__updateError || null,version:window.__noUpdate ? null : '0.2.0',notes:'Release notes <script>no HTML</script>',last_checked_at:Date.now()});
    }
    if (command === 'skip_update') {
      if(window.__skipError) throw new Error(window.__skipError);
      return publish({prompt_version:null});
    }
    if (command === 'install_update') {
      if (window.__busyWorkflow) throw new Error('Wait for running actions and workflows to finish before installing.');
      if (update.phase !== 'installed') {
        publish({phase:'downloading',error:null,prompt_version:null,downloaded:524288,total:1048576});
        if(window.__holdDownload) await new Promise(resolve => {window.__finishDownload = resolve;});
        if(window.__updateError) return publish({phase:'error',error:window.__updateError});
        publish({phase:'installing',downloaded:1048576});
        window.__installerCalls++;
        await new Promise(resolve=>setTimeout(resolve,50));
        if(window.__installError) return publish({phase:'error',error:window.__installError});
      }
      window.__restartCalls++;
      return publish({phase:'installed',error:window.__restartError || null});
    }
    throw new Error('Unexpected command: '+command);
  }
`;
test.beforeEach(async ({ page }) => {
  await page.route(/\/src\/api\.ts(?:\?.*)?$/, (route) =>
    route.fulfill({ contentType: "application/javascript", body: fixture }),
  );
  await page.goto("/");
});

test("one Install Update click shows uninterrupted progress and automatically restarts", async ({
  page,
}) => {
  await expect(
    page.getByText("OpenStreamDeck 0.1.0 · Local first", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await expect(page.getByText("Version 0.2.0 is available.")).toBeVisible();
  const updates = page.getByRole("region", { name: "Software updates" });
  await updates.getByText("Release notes · 0.2.0").click();
  await expect(
    updates.getByText("Release notes <script>no HTML</script>", {
      exact: true,
    }),
  ).toBeVisible();
  await page.evaluate(() => {
    (window as any).__holdDownload = true;
  });
  await page
    .getByRole("button", { name: "Install Update", exact: true })
    .click();
  const progress = page.getByRole("dialog", {
    name: "Downloading update…",
    exact: true,
  });
  await expect(progress).toBeVisible();
  await expect(
    progress.getByRole("progressbar", { name: "Update download" }),
  ).toHaveAttribute("value", "524288");
  await expect(progress.getByRole("button")).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: /Download update|Cancel update|Restart now|Install…/,
    }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(progress).toBeVisible();
  await page.evaluate(() => {
    (window as any).__finishDownload();
  });
  await expect(
    page.getByRole("dialog", { name: "Restarting…", exact: true }),
  ).toBeVisible();
  expect(await page.evaluate(() => (window as any).__restartCalls)).toBe(1);
  expect(await page.evaluate(() => (window as any).__installerCalls)).toBe(1);
  expect(
    await page.evaluate(() =>
      (window as any).__updateCalls.map((c: any) => c.command),
    ),
  ).toEqual(["check_updates", "install_update"]);
});

test("drafts and active workflows block the operation before it starts", async ({
  page,
}) => {
  await page
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await page.getByRole("switch", { name: "Launch at login" }).click();
  await expect(
    page.getByRole("button", { name: "Install Update", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByText("Save your changes before updating.", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.evaluate(() => {
    (window as any).__busyWorkflow = true;
  });
  await page
    .getByRole("button", { name: "Install Update", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "running actions and workflows",
  );
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).__installerCalls)).toBe(0);
  await page.getByRole("button", { name: "Dismiss error" }).click();
  await page.evaluate(() => {
    (window as any).__busyWorkflow = false;
  });
  await page
    .getByRole("button", { name: "Install Update", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Restarting…", exact: true }),
  ).toBeVisible();
});

test("current and feed errors remain visible", async ({ page }) => {
  await page.evaluate(() => {
    (window as any).__noUpdate = true;
  });
  await page
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await expect(page.getByText("You’re up to date.")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Install Update", exact: true }),
  ).toHaveCount(0);
  await page.evaluate(() => {
    (window as any).__noUpdate = false;
    (window as any).__updateError =
      "The update server did not return a release feed.";
  });
  await page
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "did not return a release feed",
  );
});

test("download, installation and restart errors recover through the same button", async ({
  page,
}) => {
  await page
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await page.evaluate(() => {
    (window as any).__updateError = "Update signature verification failed.";
  });
  await page
    .getByRole("button", { name: "Install Update", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "signature verification failed",
  );
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).__installerCalls)).toBe(0);
  await page.evaluate(() => {
    (window as any).__updateError = "";
    (window as any).__installError = "Installation failed: Permission denied";
  });
  await page
    .getByRole("button", { name: "Install Update", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText("Permission denied");
  expect(await page.evaluate(() => (window as any).__restartCalls)).toBe(0);
  await page.evaluate(() => {
    (window as any).__installError = "";
    (window as any).__restartError = "Restart failed: OS restart failure";
  });
  await page
    .getByRole("button", { name: "Install Update", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText("Restart failed");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.evaluate(() => {
    (window as any).__restartError = "";
  });
  await page
    .getByRole("button", { name: "Install Update", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Restarting…", exact: true }),
  ).toBeVisible();
  expect(await page.evaluate(() => (window as any).__installerCalls)).toBe(2);
  expect(await page.evaluate(() => (window as any).__restartCalls)).toBe(2);
});

test("unsupported installation formats cannot start the update", async ({
  page,
}) => {
  const reason =
    "In-app installation currently supports Linux AppImage. Update DEB installations through your package manager.";
  await page.evaluate((install_disabled) => {
    (window as any).__update({ install_disabled });
  }, reason);
  await page
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await expect(
    page
      .getByRole("tabpanel", { name: "General", exact: true })
      .getByText(reason, { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Install Update", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Download update", exact: true }),
  ).toHaveCount(0);
});

test("a delayed status snapshot cannot rewind newer download progress", async ({
  page,
}) => {
  await page.getByRole("tab", { name: "Device", exact: true }).click();
  await page.evaluate(() => {
    (window as any).__slowStatus = true;
  });
  await page.getByRole("tab", { name: "General", exact: true }).click();
  await expect(
    page.getByRole("progressbar", { name: "Update download" }),
  ).toHaveAttribute("value", "524288");
  await page.waitForTimeout(120);
  await expect(
    page.getByRole("progressbar", { name: "Update download" }),
  ).toHaveAttribute("value", "524288");
});

test("reopening the webview restores the native update progress without another click", async ({
  page,
}) => {
  await page.addInitScript(() => {
    (window as any).__restoreUpdate = {
      revision: 12,
      phase: "downloading",
      version: "0.2.0",
      downloaded: 524288,
      total: 1048576,
    };
  });
  await page.reload();
  const progress = page.getByRole("dialog", {
    name: "Downloading update…",
    exact: true,
  });
  await expect(progress).toBeVisible();
  await expect(progress.getByRole("progressbar")).toHaveAttribute(
    "value",
    "524288",
  );
  await page.keyboard.press("Escape");
  await expect(progress).toBeVisible();
  expect(await page.evaluate(() => (window as any).__updateCalls)).toEqual([]);
});

test("automatic availability opens a notice, skips the named version and offers the next version", async ({
  page,
}) => {
  await page.evaluate(() => {
    (window as any).__autoUpdate("0.2.0");
  });
  const notice = page.getByRole("dialog", {
    name: "Update available",
    exact: true,
  });
  await expect(notice).toBeVisible();
  await notice
    .getByRole("button", { name: "Skip this Update", exact: true })
    .click();
  await expect(notice).not.toBeVisible();
  expect(await page.evaluate(() => (window as any).__updateCalls)).toEqual([
    { command: "skip_update", version: "0.2.0" },
  ]);
  expect(await page.evaluate(() => (window as any).__installerCalls)).toBe(0);
  await expect(
    page.getByRole("button", { name: "Install Update", exact: true }),
  ).toBeEnabled();
  await page.evaluate(() => {
    (window as any).__autoUpdate("0.3.0");
  });
  const next = page.getByRole("dialog", {
    name: "Update available",
    exact: true,
  });
  await expect(next).toBeVisible();
  await next.getByRole("button", { name: "Update Now", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "Restarting…", exact: true }),
  ).toBeVisible();
  expect(await page.evaluate(() => (window as any).__installerCalls)).toBe(1);
  expect(await page.evaluate(() => (window as any).__restartCalls)).toBe(1);
});

test("startup snapshot restores the reminder and last-check time", async ({
  page,
}) => {
  await page.addInitScript(() => {
    (window as any).__restoreNavigation = { view: "Deck", revision: 1 };
    (window as any).__restoreUpdate = {
      revision: 12,
      phase: "available",
      version: "0.2.0",
      prompt_version: "0.2.0",
      last_checked_at: 1792497600000,
    };
  });
  await page.reload();
  const notice = page.getByRole("dialog", {
    name: "Update available",
    exact: true,
  });
  await expect(notice).toBeVisible();
  await notice
    .getByRole("button", { name: "Skip this Update", exact: true })
    .click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.locator("time")).toHaveAttribute(
    "datetime",
    new Date(1792497600000).toISOString(),
  );
  await page.getByRole("tab", { name: "Device", exact: true }).click();
  await page.getByRole("tab", { name: "General", exact: true }).click();
  await expect(page.locator("time")).toHaveAttribute(
    "datetime",
    new Date(1792497600000).toISOString(),
  );
  expect(await page.evaluate(() => (window as any).__updateCalls)).toEqual([
    { command: "skip_update", version: "0.2.0" },
  ]);
});

test("reminders preserve unsaved drafts and skipping failures keep the notice open", async ({
  page,
}) => {
  await page.getByRole("switch", { name: "Launch at login" }).click();
  await page.evaluate(() => {
    (window as any).__autoUpdate("0.2.0");
  });
  const notice = page.getByRole("dialog", {
    name: "Update available",
    exact: true,
  });
  await expect(
    notice.getByRole("button", { name: "Update Now", exact: true }),
  ).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(notice).toBeVisible();
  await notice
    .getByRole("button", { name: "Skip this Update", exact: true })
    .click();
  await expect(notice).not.toBeVisible();
  await expect(
    page.getByRole("switch", { name: "Launch at login" }),
  ).toBeChecked();
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.evaluate(() => {
    (window as any).__autoUpdate("0.2.0");
    (window as any).__skipError =
      "Could not save update history: Permission denied";
  });
  await notice
    .getByRole("button", { name: "Skip this Update", exact: true })
    .click();
  await expect(notice.getByRole("alert")).toContainText(
    "Could not save update history",
  );
  await expect(notice).toBeVisible();
  await page.evaluate(() => {
    (window as any).__skipError = "";
  });
  await notice
    .getByRole("button", { name: "Skip this Update", exact: true })
    .click();
  await expect(notice).not.toBeVisible();
});

test("Markdown release notes render safely and open links through the native boundary", async ({
  page,
}) => {
  const notes = [
    "### Features",
    "- **One-click updates** with _automatic restart_.",
    "- ~~Old wording~~ and `inline code`.",
    "",
    "| Platform | Support |",
    "| --- | --- |",
    "| macOS | Ready |",
    "",
    "[Read the guide](https://example.com/guide)",
    "",
    "[Unsafe link](javascript:alert(1))",
    "",
    "<script>window.__markdownExecuted = true</script>",
  ].join("\n");
  await page.evaluate((notes) => {
    (window as any).__autoUpdate("0.2.0");
    (window as any).__update({ notes });
  }, notes);
  const notice = page.getByRole("dialog", {
    name: "Update available",
    exact: true,
  });
  await expect(
    notice.getByRole("heading", { name: "Features", exact: true }),
  ).toBeVisible();
  await expect(
    notice.locator("strong", { hasText: "One-click updates" }),
  ).toBeVisible();
  await expect(
    notice.locator("em", { hasText: "automatic restart" }),
  ).toBeVisible();
  await expect(notice.locator("del")).toHaveText("Old wording");
  await expect(notice.locator("code")).toHaveText("inline code");
  await expect(notice.getByRole("cell", { name: "macOS" })).toBeVisible();
  await expect(notice.getByRole("link", { name: "Unsafe link" })).toHaveCount(
    0,
  );
  await expect(notice.locator(".release-markdown script")).toHaveCount(0);
  expect(
    await page.evaluate(() => (window as any).__markdownExecuted),
  ).toBeUndefined();
  await notice.getByRole("link", { name: "Read the guide" }).click();
  expect(await page.evaluate(() => (window as any).__updateCalls)).toEqual([
    { command: "open_release_link", url: "https://example.com/guide" },
  ]);
  await expect(notice).toBeVisible();
  await page.evaluate(() => {
    (window as any).__releaseLinkError = "Browser unavailable";
  });
  await notice.getByRole("link", { name: "Read the guide" }).click();
  await expect(notice.getByRole("alert")).toContainText("Browser unavailable");
  await notice
    .getByRole("button", { name: "Skip this Update", exact: true })
    .click();
  const updates = page.getByRole("region", { name: "Software updates" });
  await updates.getByText("Release notes · 0.2.0").click();
  await expect(
    updates.getByRole("heading", { name: "Features", exact: true }),
  ).toBeVisible();
  await expect(updates.getByRole("table")).toBeVisible();
});

test("long Markdown notes keep both mandatory choices visible at the minimum window size", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1060, height: 720 });
  await page.evaluate(() => {
    (window as any).__autoUpdate("0.2.0");
    (window as any).__update({
      notes:
        "### Highlights\n" +
        "- A useful improvement to your deck.\n".repeat(100),
    });
  });
  const notice = page.getByRole("dialog", {
    name: "Update available",
    exact: true,
  });
  await expect(notice.getByRole("button")).toHaveCount(2);
  for (const name of ["Update Now", "Skip this Update"]) {
    await expect(
      notice.getByRole("button", { name, exact: true }),
    ).toBeInViewport();
  }
  await page.keyboard.press("Escape");
  await page.mouse.click(5, 5);
  await expect(notice).toBeVisible();
  expect(await page.evaluate(() => (window as any).__updateCalls)).toEqual([]);
  for (const key of ["Tab", "Shift+Tab"]) {
    for (let count = 0; count < 6; count++) {
      await page.keyboard.press(key);
      expect(
        await notice.evaluate((dialog) =>
          dialog.contains(document.activeElement),
        ),
      ).toBe(true);
    }
  }
  const notes = notice.locator(".release-markdown");
  expect(
    await notes.evaluate(
      (element) => element.scrollHeight > element.clientHeight,
    ),
  ).toBe(true);
  await notice
    .getByRole("button", { name: "Skip this Update", exact: true })
    .click();
  await expect(notice).not.toBeVisible();
});
