import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { readFileSync, writeFileSync } from "node:fs";

const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf-8"));

/**
 * Lists the npm packages that end up in the built UI (for the third-party license notices,
 * tools/third-party-notices.mjs). Only active when FILEEPOCH_PACKAGES_OUT names an output file.
 */
function bundledPackages(): Plugin {
  return {
    name: "fileepoch-bundled-packages",
    apply: "build",
    generateBundle(_options, bundle) {
      const out = process.env.FILEEPOCH_PACKAGES_OUT;
      if (!out) return;
      const names = new Set<string>();
      for (const chunk of Object.values(bundle)) {
        if (chunk.type !== "chunk") continue;
        for (const id of Object.keys(chunk.modules)) {
          const m = id.split("\\").join("/").match(/node_modules\/((?:@[^/]+\/)?[^/]+)\//);
          if (m && chunk.modules[id].renderedLength > 0) names.add(m[1]);
        }
      }
      writeFileSync(out, JSON.stringify([...names].sort(), null, 2));
    },
  };
}

// Tauri expects a fixed dev server port and serves the built files from ../dist in release builds.
export default defineConfig({
  plugins: [svelte(), bundledPackages()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: "127.0.0.1" },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true, sourcemap: false },
});
