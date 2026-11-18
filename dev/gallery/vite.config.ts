import path from "node:path";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

const here = path.dirname(new URL(import.meta.url).pathname);

/** Swap the backend bridge for sample answers, so widgets draw in a plain browser. */
function mockBackend(): Plugin {
  return {
    name: "dock-gallery-mock-backend",
    enforce: "pre",
    async resolveId(source, importer) {
      if (!importer || importer.includes("/dev/gallery/")) return null;
      if (!/ipc(Features)?$/.test(source)) return null;
      const resolved = await this.resolve(source, importer, { skipSelf: true });
      if (resolved?.id.endsWith("/src/core/ipc.ts")) return path.join(here, "mockIpc.ts");
      if (resolved?.id.endsWith("/src/core/ipcFeatures.ts")) return path.join(here, "mockIpcFeatures.ts");
      return null;
    },
  };
}

export default defineConfig({
  root: here,
  plugins: [mockBackend(), react()],
  server: { port: 1430, fs: { allow: [path.resolve(here, "../..")] } },
});
