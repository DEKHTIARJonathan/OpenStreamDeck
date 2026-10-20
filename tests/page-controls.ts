import { expect, type Page } from "@playwright/test";

export function pageTab(page: Page, name: string) {
  return page
    .getByRole("tablist", { name: "Pages" })
    .getByRole("tab", { name, exact: true });
}

export function applicationContext(page: Page, name: string) {
  return page
    .getByRole("group", { name: "Application contexts", exact: true })
    .getByRole("button", { name, exact: true });
}

export async function editContext(page: Page, name: string) {
  await applicationContext(page, name).click();
  await expect(applicationContext(page, name)).toHaveAttribute(
    "aria-pressed",
    "true",
  );
}

export async function applicationOptions(page: Page) {
  await page
    .getByRole("button", { name: "Application options", exact: true })
    .click();
}

export async function editPage(page: Page, name: string, context?: string) {
  if (context) await editContext(page, context);
  const tab = pageTab(page, name);
  await expect(tab).toHaveCount(1);
  await tab.click();
}

export async function usePageOption(page: Page, name: string) {
  if (["Change application", "Move pages to Default"].includes(name))
    await applicationOptions(page);

  await page.getByRole("button", { name, exact: true }).click();
}
