// SPDX-License-Identifier: AGPL-3.0-only
import { defineConfig } from "tsup";
import { cloudDefine } from "../wspx/scripts/build-defines.mjs";
import { skillText } from "./skill-text.js";

export default defineConfig({
  entry: ["src/index.ts", "src/bin.ts", "src/slate-look-main.ts"],
  // The look script runs inside the app's own Electron, which provides it.
  external: ["electron"],
  format: ["esm"],
  define: cloudDefine(),
  esbuildPlugins: [skillText()],
});
