import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Static client-only viewer. No backend, no proxy.
export default defineConfig({
  plugins: [react()],
});
