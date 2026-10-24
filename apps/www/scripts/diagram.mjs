// SPDX-License-Identifier: AGPL-3.0-only
// Draws a Mermaid diagram as an SVG in the site's colours, on this computer, so a post commits the picture and the
// published page carries no Mermaid at all: `pnpm diagram content/blog/<post>/flow.mmd public/blog/<post>/flow.svg`.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = fileURLToPath(new URL("..", import.meta.url));
const [from, to] = process.argv.slice(2);
if (from === undefined || to === undefined) throw new Error("usage: pnpm diagram <in.mmd> <out.svg>");

/** The site's Tungsten inks, so a diagram sits on the page like its own figures. */
const THEME = {
  background: "#121110",
  primaryColor: "#1a1816",
  primaryTextColor: "#ece6dc",
  primaryBorderColor: "#373129",
  secondaryColor: "#211f1c",
  tertiaryColor: "#121110",
  lineColor: "#9c9288",
  textColor: "#ece6dc",
  mainBkg: "#1a1816",
  clusterBkg: "#121110",
  clusterBorder: "#2a2623",
  edgeLabelBackground: "#121110",
  fontFamily: "Inter Variable, Inter, system-ui, sans-serif",
  fontSize: "15px",
};

const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  await page.addScriptTag({ path: join(root, "node_modules/mermaid/dist/mermaid.min.js") });
  const svg = await page.evaluate(
    async ([source, theme]) => {
      window.mermaid.initialize({ startOnLoad: false, theme: "base", themeVariables: theme, flowchart: { curve: "basis" } });
      return (await window.mermaid.render("diagram", source)).svg;
    },
    [readFileSync(resolve(from), "utf8"), THEME],
  );
  mkdirSync(dirname(resolve(to)), { recursive: true });
  writeFileSync(resolve(to), svg);
  console.log(`wrote ${to}`);
} finally {
  await browser.close();
}
