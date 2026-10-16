// SPDX-License-Identifier: AGPL-3.0-only
import { defineConfig } from "tsup";
import { cloudDefine } from "../wspx/scripts/build-defines.mjs";
import { skillText } from "./skill-text.js";

export default defineConfig({
  entry: ["src/index.ts", "src/bin.ts"],
  format: ["esm"],
  define: cloudDefine(),
  esbuildPlugins: [skillText()],
  // Inlined, so the cut's require of the TOML parser becomes a factory run on first use, which every bundle of this
  // dist carries with no require of its own (a require banner here collides with the bundles').
  noExternal: ["smol-toml"],
});
