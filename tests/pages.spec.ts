import { test, expect } from "@playwright/test";
import applicationPages from "./fixtures/config-application-pages.json" with { type: "json" };

test("application-page configuration survives import, editing, save, reload and export", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator('input[type="file"][accept=".json"]').setInputFiles({
    name: "application-pages.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(applicationPages)),
  });
  await expect(
    page.getByRole("button", { name: "Editor", exact: true }),
  ).toHaveClass(/selected/);
  await page.getByRole("button", { name: "LCD 1: Save", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("Save file");
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "LCD 1: Save file", exact: true }),
  ).toBeVisible();
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  const stream = await (await download).createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(Buffer.from(chunk));
  const expected = structuredClone(applicationPages);
  expected.pages[1].bindings.lcd1!.title = "Save file";
  expect(JSON.parse(Buffer.concat(chunks).toString())).toEqual(expected);
});

test("navigation buttons are fixed on empty pages", async ({ page }) => {
  await page.goto("/");
  for (const number of [1, 2]) {
    await page
      .getByRole("button", { name: `Button ${number}`, exact: true })
      .click();
    await expect(
      page.getByText("FIXED CONTROL", { exact: true }),
    ).toBeVisible();
    await expect(
      page
        .locator(".fixed-navigation")
        .getByText("You have one page.", { exact: false }),
    ).toBeVisible();
    await expect(page.getByLabel("Action", { exact: true })).toHaveCount(0);
    for (const button of await page.locator(".action-grid button").all())
      await expect(button).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "Clear binding", exact: true }),
    ).toHaveCount(0);
  }
  await page.getByRole("button", { name: "Add page", exact: true }).click();
  await expect(
    page.getByText("2 pages · navigation wraps at either end.", {
      exact: true,
    }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await page.reload();
  await page.getByRole("button", { name: "Button 1", exact: true }).click();
  await expect(page.getByText("FIXED CONTROL", { exact: true })).toBeVisible();
  await expect(page.getByLabel("Action", { exact: true })).toHaveCount(0);
  await page
    .getByRole("button", { name: "LCD 1: Unassigned", exact: true })
    .click();
  await expect(page.getByLabel("Action", { exact: true })).toBeVisible();
});

test("edits a dial independently and persists a page across reload", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Your command center" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Dial 1 press", exact: true }).click();
  await page.getByRole("button", { name: "Turn right", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("My volume");
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.reload();
  await page.getByRole("button", { name: "Dial 1 press", exact: true }).click();
  await page.getByRole("button", { name: "Turn right", exact: true }).click();
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "My volume",
  );
  await page.getByRole("button", { name: "Turn left", exact: true }).click();
  await expect(page.getByLabel("Label", { exact: true })).toHaveValue(
    "Volume −",
  );
  await page.getByRole("button", { name: "Add page", exact: true }).click();
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Page 2", exact: true }),
  ).toBeVisible();
});
test("composes and reorders a workflow without silently running it in the browser", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "AI workflows", exact: true }).click();
  await page.getByRole("button", { name: "Add step", exact: true }).click();
  await page.getByLabel("Label", { exact: true }).fill("Review summary");
  await page.getByRole("button", { name: "Move step up" }).click();
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.getByRole("button", { name: "Run workflow", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "requires the desktop app",
  );
  await expect(
    page.getByRole("heading", { name: "AI workflows", exact: true }),
  ).toBeVisible();
});
test("renders deck without browser errors and captures visual baseline", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await expect(
    page.getByRole("button", { name: "LCD 6: Play / pause" }),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/ui-deck.png", fullPage: true });
  expect(errors).toEqual([]);
});
test("whole-dial brightness assignment configures both rotations and clears stale values", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Dial 3 press", exact: true }).click();
  await page.getByLabel("Action", { exact: true }).selectOption("brightness");
  await expect(
    page.getByRole("textbox", { name: "Value", exact: true }),
  ).toHaveValue("+5");
  await page.getByLabel("Assign the whole dial").selectOption("brightness");
  await page.getByRole("button", { name: "Turn right", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Value", exact: true }),
  ).toHaveValue("+5");
  await page.getByRole("button", { name: "Turn left", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Value", exact: true }),
  ).toHaveValue("-5");
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.reload();
  await page.getByRole("button", { name: "Dial 3 press", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Value", exact: true }),
  ).toHaveValue("60");
});
test("Open App presents a picker while preserving optional URL entry", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Terminal Change application", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Choose application" }),
  ).toBeVisible();
  await expect(page.getByRole("dialog").getByRole("alert")).toContainText(
    "desktop app",
  );
  await page.getByRole("button", { name: "Close app picker" }).click();
  await page.getByText("URL or custom target", { exact: true }).click();
  await page
    .getByLabel("Application URL or custom target")
    .fill("https://example.com");
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.reload();
  await page.getByText("URL or custom target", { exact: true }).click();
  await expect(page.getByLabel("Application URL or custom target")).toHaveValue(
    "https://example.com",
  );
});

test("deleting a page supports cancel, confirmation, save and last-page protection", async ({
  page,
}) => {
  await page.goto("/");
  const deleteButton = page.getByRole("button", {
    name: "Delete page",
    exact: true,
  });
  await expect(deleteButton).toBeDisabled();
  await page.getByRole("button", { name: "Add page", exact: true }).click();
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await deleteButton.click();
  const dialog = page.getByRole("dialog", { name: "Delete page “Page 2”?" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Page 2", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Save changes", exact: true }),
  ).toBeDisabled();
  await deleteButton.click();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await deleteButton.click();
  await dialog
    .getByRole("button", { name: "Delete page", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Page 2", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Home", exact: true }),
  ).toHaveClass("selected");
  await expect(
    page.getByRole("button", { name: "Save changes", exact: true }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Page 2", exact: true }),
  ).toHaveCount(0);
  await expect(deleteButton).toBeDisabled();
});

test("Rename page is left of Delete page and supports cancel, Enter and save/reload", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator(".page-actions button")).toHaveText([
    "Rename page",
    "Delete page",
  ]);
  const rename = page
    .locator(".page-actions")
    .getByRole("button", { name: "Rename page", exact: true });
  await rename.click();
  const dialog = page.getByRole("dialog", { name: "Rename page", exact: true });
  await expect(dialog.getByRole("textbox", { name: "Page name" })).toHaveValue(
    "Home",
  );
  await dialog.getByRole("textbox", { name: "Page name" }).fill("   ");
  await expect(
    dialog.getByRole("button", { name: "Rename page", exact: true }),
  ).toBeDisabled();
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Home", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Save changes", exact: true }),
  ).toBeDisabled();
  await rename.click();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await rename.click();
  await dialog.getByRole("textbox", { name: "Page name" }).fill("My shortcuts");
  await dialog.getByRole("textbox", { name: "Page name" }).press("Enter");
  await expect(
    page.getByRole("button", { name: "My shortcuts", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "My shortcuts", exact: true }),
  ).toBeVisible();
});

test("JSON import, save and export preserve pages and settings", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Add page", exact: true }).click();
  const firstDownload = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  const downloaded = await firstDownload;
  expect(downloaded.suggestedFilename()).toBe("config.openstreamdeck.json");
  const stream = await downloaded.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(Buffer.from(chunk));
  const config = JSON.parse(Buffer.concat(chunks).toString());
  expect(config.pages.map((p: { name: string }) => p.name)).toEqual([
    "Home",
    "Page 2",
  ]);
  expect(
    await page.evaluate(() => localStorage.getItem("openstreamdeck-preview")),
  ).toBeNull();
  config.pages.push({
    id: "imported-page",
    name: "Imported page",
    bindings: {},
  });
  config.active_page = "imported-page";
  config.brightness = 37;
  const upload = page.locator('input[type="file"][accept=".json"]');
  await upload.setInputFiles({
    name: "pages.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(config)),
  });
  await expect(
    page.getByRole("button", { name: "Imported page", exact: true }),
  ).toHaveClass(/selected/);
  await expect(
    page.getByRole("button", { name: "Save changes" }),
  ).toBeEnabled();
  expect(
    await page.evaluate(() => localStorage.getItem("openstreamdeck-preview")),
  ).toBeNull();
  await upload.setInputFiles({
    name: "invalid.json",
    mimeType: "application/json",
    buffer: Buffer.from("{}"),
  });
  await expect(page.getByRole("alert")).toContainText("Not an OpenStreamDeck");
  await expect(
    page.getByRole("button", { name: "Imported page", exact: true }),
  ).toHaveClass(/selected/);
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Imported page", exact: true }),
  ).toHaveClass(/selected/);
  const roundTripDownload = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  const saved = await (await roundTripDownload).createReadStream();
  const savedChunks: Buffer[] = [];
  for await (const chunk of saved) savedChunks.push(Buffer.from(chunk));
  expect(JSON.parse(Buffer.concat(savedChunks).toString())).toEqual(config);
});
