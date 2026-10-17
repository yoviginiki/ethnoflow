import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port; see src-tauri/tauri.conf.json build.devUrl
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // The in-app Help imports docs/guide/*.md from the repository root.
  server: { port: 5173, strictPort: true, fs: { allow: ["../.."] } },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: { target: "es2022", sourcemap: true },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
