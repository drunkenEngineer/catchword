import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The development server listens on this computer only. The app itself
// serves its built files without any port (SEC-2).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { host: "127.0.0.1", port: 1420, strictPort: true },
  build: { target: "es2022", outDir: "dist" },
  test: { environment: "jsdom" },
});
