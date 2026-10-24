// SPDX-License-Identifier: AGPL-3.0-only
/// <reference types="vitest/config" />
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vitest/config";
import { TEST_ENV } from "../../vitest.env.js";
import { blogMarkdown } from "./scripts/blog-md.mjs";

/** `vite preview` answers as Vercel does: the analytics script Vercel serves itself, and 404.html with a 404 for a path
 * the build has no page for. Vite's own fallback has rewritten a path with a page to its .html by then. */
const likeVercel: Plugin = {
  name: "like-vercel",
  configurePreviewServer(server) {
    server.middlewares.use("/_vercel/insights/script.js", (_req, res) => {
      res.setHeader("content-type", "application/javascript");
      res.end("");
    });
    return () =>
      server.middlewares.use((req, res, next) => {
        if (req.url?.endsWith(".html")) return next();
        res.statusCode = 404;
        res.setHeader("content-type", "text/html");
        res.end(readFileSync(resolve(server.config.root, server.config.build.outDir, "404.html")));
      });
  },
};

export default defineConfig({
  appType: "mpa",
  plugins: [react(), tailwindcss(), likeVercel, blogMarkdown()],
  // BLOG_DRAFTS=1 builds the posts marked draft too, for a look on this computer before they publish.
  define: { __BLOG_DRAFTS__: JSON.stringify(process.env.BLOG_DRAFTS === "1") },
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  test: { environment: "jsdom", globals: true, setupFiles: ["./test/setup.ts", fileURLToPath(new URL("../../vitest.stall-note.ts", import.meta.url))], env: TEST_ENV },
});
