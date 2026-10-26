import { readFileSync } from "node:fs";
const R = "/root/wsp-vm/wt-2441-cut";
const pkg = n => `${R}/packages/${n}/src/index.ts`;
const alias = { "@wsp/protocol/slate": `${R}/packages/protocol/src/slate/index.ts` };
for (const n of ["engine","adapter-claude","adapter-codex","adapter-cursor","adapter-opencode","keys","own-file","protocol","runtime","host","collect","catalog"]) alias[`@wsp/${n}`] = pkg(n);
export default {
  root: "/root/cli-b-probe",
  plugins: [{ name: "md-text", load: id => (id.endsWith(".md") ? `export default ${JSON.stringify(readFileSync(id, "utf8"))};` : undefined) }],
  resolve: { alias },
  test: { include: [process.env.P ?? "*.probe.ts"], pool: "forks", poolOptions: { forks: { singleFork: true } }, testTimeout: 120000 },
};
