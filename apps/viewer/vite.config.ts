import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Static client-only viewer. Also serves as the Tauri v2 webview in the desktop
// build, so the dev server uses a fixed port that tauri.conf.json's devUrl
// points at. No backend, no proxy.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
});
