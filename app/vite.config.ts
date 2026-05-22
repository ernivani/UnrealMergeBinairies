import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri-friendly Vite config: fixed port (so tauri.conf.json can point at it),
// no clearScreen (so cargo's own output stays visible), strictPort.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
  },
  build: {
    target: "esnext",
    sourcemap: true,
    outDir: "dist",
    emptyOutDir: true,
  },
});
