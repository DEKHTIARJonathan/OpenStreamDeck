import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  // Root .env belongs to native updater signing, not frontend configuration.
  envDir: false,
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  clearScreen: false,
});
