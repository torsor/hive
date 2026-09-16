import { defineConfig } from "vite";

const hub = process.env.VITE_HUB_URL;

export default defineConfig({
  base: process.env.VITE_BASE ?? "/",
  clearScreen: false,
  test: {
    environment: "node",
  },
  server: {
    port: 5173,
    strictPort: true,
  },
  define: hub ? { "import.meta.env.VITE_HUB_URL": JSON.stringify(hub) } : {},
});
