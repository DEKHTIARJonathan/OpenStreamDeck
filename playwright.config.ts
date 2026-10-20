import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  use: {
    baseURL: "http://127.0.0.1:1420",
    viewport: { width: 1380, height: 900 },
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "bun run dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
  },
});
