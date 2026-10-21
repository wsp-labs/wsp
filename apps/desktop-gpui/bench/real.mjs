// SPDX-License-Identifier: AGPL-3.0-only
// The `real` bench thread: a long thread shaped like one a person keeps open for days, generated from a seed and the
// same on every run. Person messages carry screenshots and long pastes; tool calls carry long output, some of them
// failing so their row stands alone; some calls are subagents with their own calls; replies carry code in many
// languages, tables and a few long write-ups past gpui-kit's 4 KB background-parse line.

/** Below the host's 16 KB per kept tool result (TOOL_RESULT_KEPT), so no output is cut. */
export const OUTPUT_MAX = 15_000;

/** The sizes screenshots come in, device pixels: a whole Retina window, a half, a region, a dialog. */
export const SHOT_SIZES = [
  [2880, 1800],
  [2560, 1600],
  [1600, 1000],
  [1200, 800],
  [900, 600],
];

const LANGS = ["rust", "ts", "tsx", "python", "go", "sh", "json", "yaml", "sql", "css", "diff", "toml", "swift", "ruby", "c", "java"];
const IDENTS = ["HostClient", "threadMessages", "scrollTop", "useChatThread", "toolUseId", "ThreadView", "sessions_history", "messageId", "flattenContent", "list_state", "row_height", "page_events", "attachment", "render_row", "overdraw", "measure"];
const FILES = ["apps/web/src/components/chat/timeline/list.tsx", "packages/protocol/src/thread-read.ts", "apps/desktop/src/host-lifecycle.ts", "apps/desktop-gpui/host-client/src/lib.rs", "packages/runtime/src/session.ts", "apps/desktop-gpui/app/src/thread_view.rs", "packages/adapter-claude/src/adapter.ts", "daemon/crates/wsp-daemon/src/pty.rs"];
const WORDS = "the list keeps every row it has measured so nothing jumps when a long tool output lands and the anchor holds through the resize while older pages fold under the rows already drawn and the reader stays on the line they were reading".split(" ");
const ASKS = ["Look at why the transcript jumps when a long tool output lands and fix it.", "Can you make the sidebar keep its scroll when a thread moves to the top?", "Add a test that the paging stops at the first event.", "The switch between threads feels slow, find where the time goes.", "Wire the new row type through the adapter and the read.", "Why does the host send the head twice on reconnect?", "Here is what I see now, it still flickers.", "Run the whole suite and tell me what fails.", "Write up what you found so I can send it on.", "Compare the two approaches and pick one."];

/** mulberry32: the same numbers from the same seed on every run. */
export function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = (r, list) => list[Math.floor(r() * list.length)];
const between = (r, lo, hi) => lo + Math.floor(r() * (hi - lo + 1));
const words = (r, n) => Array.from({ length: n }, () => pick(r, WORDS)).join(" ");
const sentence = r => {
  const s = `${words(r, between(r, 8, 22))}, \`${pick(r, IDENTS)}\` ${words(r, between(r, 3, 9))}.`;
  return s[0].toUpperCase() + s.slice(1);
};
const capped = text => (text.length <= OUTPUT_MAX ? text : text.slice(0, text.lastIndexOf("\n", OUTPUT_MAX)));

function codeLine(r, lang, i) {
  const id = pick(r, IDENTS);
  const n = between(r, 1, 900);
  switch (lang) {
    case "rust":
      return i % 6 === 0 ? `fn ${id.toLowerCase()}_${i}(cx: &mut Context<Self>) -> Option<usize> {` : i % 6 === 5 ? "}" : `    let ${id.toLowerCase()} = self.rows.get(${n}).map(|row| row.height * 2).unwrap_or_default();`;
    case "ts":
    case "tsx":
      return i % 6 === 0 ? `export function ${id}${i}(rows: readonly Row[]): number {` : i % 6 === 5 ? "}" : lang === "tsx" && i % 3 === 0 ? `  return <Row key={${n}} height={rows[${n}]?.height ?? 0} onMeasure={measure} />;` : `  const ${id}${i} = rows.at(${n})?.height ?? ${n};`;
    case "python":
      return i % 5 === 0 ? `def ${id.toLowerCase()}_${i}(rows, at=${n}):` : `    total += rows[at + ${i}].height if rows else ${n}  # ${words(r, 3)}`;
    case "go":
      return i % 6 === 0 ? `func ${id}${i}(rows []Row) (int, error) {` : i % 6 === 5 ? "}" : `\tif h := rows[${n}].Height; h > 0 { total += h }`;
    case "sh":
      return `${pick(r, ["rg -n", "sed -n '1,40p'", "wc -l", "git log --oneline --", "jq '.events | length'"])} ${pick(r, FILES)}`;
    case "json":
      return `  "${id}": { "at": ${n}, "height": ${between(r, 12, 400)}, "kept": ${r() < 0.5} },`;
    case "yaml":
      return i % 4 === 0 ? `${id.toLowerCase()}:` : `  - name: ${words(r, 2)}\n    at: ${n}`;
    case "sql":
      return i % 4 === 0 ? `SELECT thread_id, count(*) AS events FROM transcript_${i}` : `  WHERE seq > ${n} AND kind = '${pick(r, ["delta", "tool_use", "result"])}' -- ${words(r, 3)}`;
    case "css":
      return i % 4 === 0 ? `.row-${id.toLowerCase()} {` : i % 4 === 3 ? "}" : `  margin-block: ${between(r, 0, 24)}px; color: oklch(0.${between(r, 2, 9)} 0.0${between(r, 1, 9)} ${n});`;
    case "diff":
      return i % 5 === 0 ? `@@ -${n},7 +${n},8 @@` : `${pick(r, ["+", "-", " "])}  const ${id} = rows[${n}]?.height ?? measure(rows, ${n});`;
    case "toml":
      return i % 4 === 0 ? `[${id.toLowerCase()}]` : `${pick(r, IDENTS).toLowerCase()} = { version = "0.${n}", features = ["${words(r, 1)}"] }`;
    case "swift":
      return i % 6 === 0 ? `func ${id.toLowerCase()}${i}(_ rows: [Row]) -> CGFloat {` : i % 6 === 5 ? "}" : `    let h${i} = rows[${n}].height ?? ${n}.0`;
    case "ruby":
      return i % 5 === 0 ? `def ${id.toLowerCase()}_${i}(rows)` : i % 5 === 4 ? "end" : `  rows.select { |row| row.height > ${n} }.sum(&:height)`;
    case "c":
      return i % 6 === 0 ? `static int ${id.toLowerCase()}_${i}(const struct row *rows, size_t n) {` : i % 6 === 5 ? "}" : `    if (rows[${n}].height > 0) total += rows[${n}].height;`;
    default:
      return i % 6 === 0 ? `public int ${id}${i}(List<Row> rows) {` : i % 6 === 5 ? "}" : `    total += rows.get(${n}).height();`;
  }
}

const fence = (r, lang, n) => `\`\`\`${lang}\n${Array.from({ length: n }, (_, i) => codeLine(r, lang, i)).join("\n")}\n\`\`\``;

function table(r) {
  const columns = between(r, 3, 7);
  const heads = ["File", "Rows", "Height", "Pages", "Parsed", "Images", "Notes"].slice(0, columns);
  const rows = Array.from({ length: between(r, 4, 12) }, () => heads.map((_, c) => (c === 0 ? pick(r, FILES) : c === columns - 1 ? words(r, between(r, 2, 8)) : String(between(r, 1, 5000)))));
  return [`| ${heads.join(" | ")} |`, `| ${heads.map(() => "---").join(" | ")} |`, ...rows.map(row => `| ${row.join(" | ")} |`)].join("\n");
}

function prose(r, lo, hi) {
  const lines = [];
  const n = between(r, lo, hi);
  while (lines.length < n) {
    if (lines.length > 0 && n - lines.length >= 4 && r() < 0.3) {
      lines.push(`${pick(r, ["What changed:", "Where it goes wrong:", "Left to check:"])}`);
      for (let i = between(r, 2, 4); i > 0; i--) lines.push(`- \`${pick(r, IDENTS)}\` in \`${pick(r, FILES)}\`: ${words(r, between(r, 5, 12))}`);
    } else {
      lines.push(sentence(r));
    }
  }
  return lines.join("\n");
}

/** A write-up past 4 KB: headings, paragraphs, lists and a code block or two. */
function writeUp(r) {
  const parts = [`## ${words(r, 4)}`];
  for (let s = 0; s < between(r, 5, 8); s++) {
    parts.push(`### ${words(r, 3)}`, prose(r, 6, 14));
    if (r() < 0.4) parts.push(fence(r, pick(r, LANGS), between(r, 6, 18)));
  }
  return parts.join("\n\n");
}

/** A log or a stack trace pasted whole, past 4 KB. */
function paste(r) {
  const n = between(r, 60, 120);
  const lines = Array.from({ length: n }, (_, i) =>
    i % 9 === 0
      ? `Error: ${words(r, 6)} (${pick(r, FILES)}:${between(r, 1, 900)}:${between(r, 1, 80)})`
      : `2026-10-${String(between(r, 10, 19)).padStart(2, "0")}T09:${String(between(r, 0, 59)).padStart(2, "0")}:${String(between(r, 0, 59)).padStart(2, "0")}.${between(r, 100, 999)}Z ${pick(r, ["INFO", "WARN", "ERROR", "DEBUG"])} ${pick(r, ["host", "runtime", "adapter", "daemon"])}: ${words(r, between(r, 6, 14))}`,
  );
  return `${pick(r, ["This is what the log says:", "It crashed again, here is the whole output:", "Pasting the trace:"])}\n\n${lines.join("\n")}`;
}

function output(r, kind, n) {
  const file = pick(r, FILES);
  if (kind === "Read") return capped(Array.from({ length: n }, (_, i) => `${String(i + 1).padStart(6)}\t${codeLine(r, "ts", i)}`).join("\n"));
  if (kind === "Grep") return capped(Array.from({ length: n }, () => `${pick(r, FILES)}:${between(r, 1, 900)}: ${codeLine(r, "rust", 1).trim()}`).join("\n"));
  return capped(Array.from({ length: n }, () => `${file}:${between(r, 1, 900)}: ${words(r, between(r, 4, 14))}`).join("\n"));
}

function call(r) {
  const kind = pick(r, ["Read", "Read", "Bash", "Bash", "Grep", "Edit", "Edit", "Write"]);
  const file = pick(r, FILES);
  if (kind === "Edit") return { name: kind, input: { file_path: file, old_string: codeLine(r, "ts", 1), new_string: codeLine(r, "ts", 2) }, output: `The file ${file} has been updated.` };
  if (kind === "Write") return { name: kind, input: { file_path: file, content: fence(r, "ts", between(r, 5, 30)) }, output: `File created successfully at: ${file}` };
  if (kind === "Read") return { name: kind, input: { file_path: file }, output: output(r, kind, between(r, 20, 150)) };
  if (kind === "Grep") return { name: kind, input: { pattern: pick(r, IDENTS), path: "apps" }, output: output(r, kind, between(r, 2, 40)) };
  if (r() < 0.06) {
    const n = between(r, 40, 150);
    const log = Array.from({ length: n }, (_, i) => (i % 7 === 0 ? `FAIL ${pick(r, FILES)} > ${words(r, 5)}` : `    at ${pick(r, IDENTS)} (${pick(r, FILES)}:${between(r, 1, 900)}:${between(r, 1, 80)}) ${words(r, 3)}`));
    return { name: kind, input: { command: "pnpm test", description: "Run the tests" }, output: capped(`Exit code 1\n${log.join("\n")}`), error: true };
  }
  return { name: kind, input: { command: `rg -n ${pick(r, IDENTS)} ${file}`, description: "Find the uses" }, output: output(r, kind, between(r, 1, 80)) };
}

/** A subagent: its launching call, the lines it says and the calls it makes under it, and what it answers. */
function task(r) {
  const children = [];
  for (let i = between(r, 2, 6); i > 0; i--) children.push(r() < 0.4 ? { text: sentence(r) } : call(r));
  return { name: "Task", input: { description: `Trace ${words(r, 3)}`, prompt: prose(r, 2, 4), subagent_type: "general-purpose" }, output: prose(r, 2, 6), children: children.map(c => (c.error ? { ...c, error: false } : c)) };
}

const AREAS = ["app/src", "packages/runtime/src", "packages/protocol/src", "apps/web/src/components"];
const PARTS = ["thread", "list", "rows", "pages", "cards", "scroll", "images", "trace"];

/** The files a turn writes into its folder, so the host records what it changed: `n` paths in folders three deep. */
function changed(r, turn, n) {
  return Array.from({ length: n }, (_, i) => ({
    path: `${pick(r, AREAS)}/${pick(r, PARTS)}/${pick(r, PARTS)}/${pick(r, IDENTS).toLowerCase()}_${i}.ts`,
    lines: between(r, 3, 40),
    turn,
  }));
}

/** What the agent writes into a file a turn changes, the same bytes on every run. */
export const fileText = file => Array.from({ length: file.lines }, (_, i) => `export const line${i} = ${file.turn * 1000 + i};`).join("\n") + "\n";

/** The real thread's turns, `{ prompt, images, thinking, tools, reply, files }`; `images` are `{ name, width, height,
 * seed }`, drawn by `shot` when the fixture sends them, and `files` are `{ path, lines, turn }`, which the stub writes
 * into the thread's folder so the turn ends with a changed-files card. */
export function realTurns(plan) {
  // Their own stream, so the files leave the rest of the thread as it was.
  const f = rng(plan.seed + 1);
  const writes = new Map();
  while (writes.size < plan.changes) writes.set(between(f, 1, plan.turns - 1), between(f, 100, 140));
  const r = rng(plan.seed);
  const tasks = new Set();
  while (tasks.size < plan.tasks) tasks.add(between(r, 1, plan.turns - 1));
  return Array.from({ length: plan.turns }, (_, t) => {
    const roll = r();
    const pasted = t > 0 && roll < plan.pastes;
    const shown = t > 0 && !pasted && roll < plan.pastes + plan.shots;
    const prompt = pasted ? paste(r) : shown ? `${pick(r, ASKS)}` : r() < 0.3 ? `${pick(r, ASKS)}\n${sentence(r)}\n${sentence(r)}` : pick(r, ASKS);
    const images = shown ? Array.from({ length: between(r, 1, 3) }, (_, i) => { const [width, height] = pick(r, SHOT_SIZES); return { name: `shot-${t + 1}-${i + 1}.png`, width, height, seed: plan.seed * 100_000 + t * 10 + i }; }) : [];
    const tools = Array.from({ length: between(r, 0, plan.calls * 2) }, () => call(r));
    if (tasks.has(t)) tools.splice(between(r, 0, tools.length), 0, task(r));
    const thinking = r() < 0.3 ? prose(r, 3, 10) : undefined;
    const kind = r();
    const reply = kind < plan.writeUps ? writeUp(r) : kind < plan.writeUps + plan.tables ? `${prose(r, 2, 5)}\n\n${table(r)}\n\n${prose(r, 1, 3)}` : kind < plan.writeUps + plan.tables + plan.code ? `${prose(r, 2, 6)}\n\n${fence(r, pick(r, LANGS), between(r, 8, 90))}\n\n${prose(r, 1, 3)}` : prose(r, 2, 10);
    const files = writes.has(t) ? changed(f, t, writes.get(t)) : [];
    return { prompt: `${prompt} (real, turn ${t + 1})`, images, thinking, tools, reply, files };
  });
}

/** One screenshot's pixels, RGB: a window's chrome, a sidebar, rows of text bars, a few coloured blocks and a photo's
 * worth of noise, so it decodes and weighs like a real capture rather than a flat fill. */
export function shot({ width, height, seed }) {
  const r = rng(seed);
  const pixels = Buffer.alloc(width * height * 3);
  const fill = (x0, y0, w, h, [cr, cg, cb]) => {
    const x1 = Math.min(width, x0 + w);
    const y1 = Math.min(height, y0 + h);
    for (let y = Math.max(0, y0); y < y1; y++) {
      for (let x = Math.max(0, x0); x < x1; x++) {
        const at = (y * width + x) * 3;
        pixels[at] = cr;
        pixels[at + 1] = cg;
        pixels[at + 2] = cb;
      }
    }
  };
  const dark = r() < 0.5;
  fill(0, 0, width, height, dark ? [28, 28, 32] : [250, 250, 248]);
  const side = Math.round(width * 0.2);
  fill(0, 0, side, height, dark ? [36, 36, 42] : [240, 240, 236]);
  fill(0, 0, width, Math.round(height * 0.05), dark ? [44, 44, 50] : [232, 232, 228]);
  const ink = dark ? [210, 210, 214] : [40, 40, 44];
  for (let y = Math.round(height * 0.08); y < height - 24; y += between(r, 22, 40)) {
    let x = side + 32;
    while (x < width - 80) {
      const w = between(r, 20, 140);
      fill(x, y, w, 10, r() < 0.08 ? [between(r, 40, 220), between(r, 40, 220), between(r, 40, 220)] : ink);
      x += w + between(r, 6, 14);
      if (r() < 0.12) break;
    }
  }
  for (let i = between(r, 2, 5); i > 0; i--) fill(between(r, side, width - 200), between(r, 0, height - 200), between(r, 80, 400), between(r, 40, 200), [between(r, 0, 255), between(r, 0, 255), between(r, 0, 255)]);
  const nw = Math.round(width * 0.3);
  const nh = Math.round(height * 0.3);
  const nx = between(r, side, width - nw);
  const ny = between(r, 0, height - nh);
  for (let y = ny; y < ny + nh; y++) for (let x = nx; x < nx + nw; x++) {
    const at = (y * width + x) * 3;
    const v = Math.floor(r() * 256);
    pixels[at] = v;
    pixels[at + 1] = (v + x) & 0xff;
    pixels[at + 2] = (v + y) & 0xff;
  }
  return { width, height, channels: 3, pixels };
}
