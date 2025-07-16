import { defineConfig } from "vite";

export default defineConfig({
  // Minimal config without React plugin to avoid import errors
  build: {
    outDir: "dist",
  },
  server: {
    port: 3000,
  },
});
