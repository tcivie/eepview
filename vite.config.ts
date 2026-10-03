// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import { defineConfig, loadEnv } from "vite";

const uiPages = [
  "index",
  "home",
  "history",
  "bookmarks",
  "stats",
  "settings",
  "setup",
  "blocked",
  "router-down",
  "toolbar",
  "status",
];

const input = Object.fromEntries([
  ["main", "index.html"],
  ...uiPages.map((page) => [`ui-${page}`, `src/ui/${page}.html`]),
]);

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  // loadEnv also reads the real process environment for keys with this prefix.
  const host = loadEnv(mode, ".", "TAURI_").TAURI_DEV_HOST;
  return {
    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    build: {
      rollupOptions: {
        input,
      },
    },
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        // 3. tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
