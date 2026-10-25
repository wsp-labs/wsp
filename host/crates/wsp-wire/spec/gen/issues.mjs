// issues.mjs <out.json>: zod's first issue, whole, and ZodError.message, for chosen refusals.
import M from "./bundle.mjs";
import fs from "node:fs";
const V = n => M[`views/${n}`];
const out = [];
const sub = { branch: "b", pushedSha: null, commits: 0, unpushed: 0, changed: 0, readAt: "t" };
const proc = { pid: 3, kind: "tab", origins: ["https://a.example"], contents: [1] };
const cases = [
  ["SubResult", "views/sub-row", "a missing key", { ...sub, branch: undefined }],
  ["SubResult", "views/sub-row", "a string where a number goes", { ...sub, commits: "7" }],
  ["SubResult", "views/sub-row", "a fraction where an integer goes", { ...sub, commits: 2.5 }],
  ["SubResult", "views/sub-row", "a negative count", { ...sub, unpushed: -1 }],
  ["SubResult", "views/sub-row", "a missing nullable key", (({ pushedSha, ...r }) => r)(sub)],
  ["AppProc", "views/app-procs", "a pid of 0 where it must be positive", { ...proc, pid: 0 }],
  ["AppProc", "views/app-procs", "an origin past 256 UTF-16 units", { ...proc, origins: ["a".repeat(257)] }],
  ["AppProc", "views/app-procs", "an origin of 129 emoji, 258 UTF-16 units but 129 chars", { ...proc, origins: ["😀".repeat(129)] }],
  ["AppProc", "views/app-procs", "21 origins", { ...proc, origins: Array(21).fill("x") }],
  ["AppProc", "views/app-procs", "an unknown kind", { ...proc, kind: "worker" }],
  ["AppProcList", "views/app-procs", "the second row's pid negative", [proc, { ...proc, pid: -1 }]],
  ["PreferencesTarget", "views/preferences", "an unknown key on a strict object", { workspace: "w", extra: 1, more: 2 }],
  ["ThreadScope", "views/workspace", "the wrong literal", { kind: "lead", threadId: "t", workspaceId: "w", rootThreadId: "r" }],
  ["SessionEvent", "views/session-events", "an unknown type", { type: "session.nope", sessionId: "s" }],
  ["SessionEvent", "views/session-events", "no type at all", { sessionId: "s" }],
  ["ProjectOverrides", "thread-defaults", "a tracker failing its regex", { trackers: ["no slash"] }],
  ["WorkspaceCopy", "views/place", "a refine that wants an absolute path", { from: "rel", at: "/x" }],
  ["PreferencesPatch", "views/preferences", "a refine that wants a size off the table", { textSize: 99 }],
];
for (const [name, mod, what, input] of cases) {
  const r = M[mod][name].safeParse(input);
  if (r.success) throw new Error(`${name} ${what} parsed`);
  out.push({ type: name, module: mod, case: what, input, issue: r.error.issues[0], text: JSON.stringify([r.error.issues[0]], null, 2), message: r.error.message === JSON.stringify(r.error.issues, null, 2) ? "issues" : "?" });
}
fs.writeFileSync(process.argv[2], JSON.stringify(out, null, 1) + "\n");
for (const o of out) console.log(o.type, "|", o.case, "|", JSON.stringify(o.issue));
