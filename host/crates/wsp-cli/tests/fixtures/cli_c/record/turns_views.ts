// Views and replies the two turns-help probes (turns_picks, turns_target) build their scripts from. Every shape is a
// real recorded reply of daemon/crates/wsp-mcp/tests/answers, or one grown from it with every field the zod schema and the
// Rust type both require. Keys are in the order the schema writes them, since the Rust side serialises in that order.
import { frame, recorded, refused, type Obj } from "./lib.ts";

const base = recorded("run", "a project on another computer forks a machine named off the task");

export const J = (text: string): Obj => JSON.parse(text) as Obj;
const basket = J(base["harnesses.list"]!)["harnesses"] as Obj[];
export const CLAUDE = basket[0]!;
export const CODEX = basket[1]!;

/** A catalog row as the daemon answers it, with the fields a case changes. */
export const catalog = (row: Obj, over: Obj = {}): Obj => ({ ...row, ...over });
export const harnesses = (...rows: Obj[]): string => frame({ harnesses: rows });

/** The two rows of the recorded answer: claude marked default, codex a table row. */
export const BOTH = harnesses(CLAUDE, CODEX);
export const CLAUDE_ONLY = harnesses(CLAUDE);
export const CODEX_DEFAULT = harnesses(catalog(CLAUDE, { isDefault: false }), catalog(CODEX, { isDefault: true }));
export const NO_DEFAULT = harnesses(catalog(CLAUDE, { isDefault: false }), catalog(CODEX, { isDefault: false }));
export const NONE = harnesses();
/** claude read off wsp's own table, whose haiku takes no effort, and a model that has a fast mode. */
export const TABLE_CLAUDE = harnesses(
  catalog(CLAUDE, {
    source: "table",
    models: [
      { value: "claude-sonnet-5", label: "Sonnet 5", isDefault: true, fast: true },
      { value: "claude-opus-5-5", label: "Opus 5.5", efforts: ["high", "max"] },
      { value: "claude-haiku", label: "Haiku", efforts: [] },
    ],
  }),
);
export const FAST_CLAUDE = harnesses(
  catalog(CLAUDE, {
    models: [
      { value: "claude-sonnet-5", label: "Sonnet 5", isDefault: true, fast: true },
      { value: "claude-opus-5-5", label: "Opus 5.5", efforts: ["high", "max"] },
    ],
  }),
);
export const NO_ACCESS = harnesses(catalog(CLAUDE, { access: {} }));
export const OPEN_VALUES = harnesses(catalog(CLAUDE, { models: [], efforts: [] }));

export const project = (id: string, name: string, computer: string, path: string, extra: Obj = {}): Obj => ({
  id,
  name,
  computer,
  source: { kind: "folder", path },
  path,
  remote: "",
  defaultBranch: "main",
  memoryKey: `key-${id}`,
  memoryDir: `/mem/${id}`,
  ...extra,
  createdAt: "2026-09-27T00:00:00.000Z",
});
export const gitTop = (top: string): Obj => ({ git: { top } });
export const resolved = (p: Obj): string => frame({ project: p });
export const listed = (...ps: Obj[]): string => frame({ projects: ps });

export const workspace = (id: string, name: string, kind: string, p: Obj, extra: Obj = {}): Obj => {
  const { folder, ...rest } = extra;
  return {
    id,
    name,
    machineId: kind === "local" ? "here" : "m-1",
    phase: "running",
    kind,
    golden: kind === "local" ? "" : "default",
    createdAt: "2026-09-27T00:00:00.000Z",
    project: { id: p["id"], name: p["name"], path: p["path"], computer: p["computer"] },
    ...(folder !== undefined ? { folder } : {}),
    home: "/root",
    ...rest,
  };
};
export const wsList = (...ws: Obj[]): string => frame({ workspaces: ws });
export const wsOne = (w: Obj): string => frame({ workspace: w });

export const session = (id: string, workspaceId: string, threadId: string | undefined, harness = "claude", extra: Obj = {}): Obj => ({
  workspaceId,
  harness,
  status: "completed",
  id,
  ...(threadId !== undefined ? { threadId } : {}),
  ...extra,
});
export const sessions = (...s: Obj[]): string => frame({ sessions: s });

/** A folded thread row as the host lists it, for the helpers that take a ThreadView. */
export const thread = (id: string, workspaceId: string, extra: Obj = {}): Obj => ({
  id,
  workspaceId,
  harness: "claude",
  startedBy: "person",
  status: "completed",
  title: "hello",
  sessionId: id,
  turns: 1,
  ran: true,
  ...extra,
});

export const defaultsOf = (map: Obj): string => frame({ defaults: map });
export const agentIs = (value: string, from = "default"): Obj => ({ agent: { value, from } });

export const landing = (kind: string | undefined): string => frame({ place: "pl-box", name: "box", capabilities: { sizes: [] }, ...(kind !== undefined ? { kind } : {}) });

export const PREFERENCES = recorded("agents_set", "the picker's lists merged over what stands")["preferences.get"]!;
export const PREFERENCES_NONE = recorded("agents_set", "the picker's lists with none standing")["preferences.get"]!;

export { frame, refused };
