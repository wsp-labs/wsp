// SPDX-License-Identifier: AGPL-3.0-only
// The sweep's states for baseline.mjs --wave sweep: today's app driven to each route, centre screen, pane and empty
// state sweep.mjs lists, every one whole window at 1280x800. The fixture is fixture.sh's, with its real host shape
// (fixture.realHost): the display name, the project quiet with no thread and the panes thread.
import wave1 from "../wave-1/states.mjs";
import wave2 from "../wave-2/states.mjs";
import { drives, longThread, withIds } from "./drives.mjs";

// Made before the first shot, so every state's sidebar holds the same threads in both apps.
export const setup = async host => {
  await longThread(host);
};

const byId = (states, id) => {
  const state = states.find(s => s.id === id);
  if (state === undefined) throw new Error(`no state ${id} to drive from`);
  return state;
};

const state = (id, go, extra = {}) => ({ id, data: withIds(), go, ...extra });
/** A state another wave drives already, shot here whole window. */
const borrowed = (id, from) => ({ ...from, id, clip: undefined });

export default [
  state("SWP-pane-preview", drives.pane("preview")),
  state("SWP-pane-terminal", drives.pane("terminal", 'aside[data-terminal-owner="right-panel"]')),
  state("SWP-pane-diff", drives.pane("diff", "[data-diff-surface]")),
  state("SWP-pane-files", drives.pane("files", "[data-files-pane]")),
  state("SWP-pane-machine", drives.pane("machine", "[data-machine]")),
  state("SWP-pane-processes", drives.pane("processes", "[data-procs], [data-procs-thread], [data-procs-nothing]")),
  state("SWP-pane-agents", drives.pane("agents", "[data-agents-panel]", "checked")),
  state("SWP-pane-aside", drives.aside),
  state("SWP-pane-slate", drives.pane("slate", "[data-slate-surface]")),
  state("SWP-home", drives.home),
  state("SWP-thread", drives.thread),
  state("SWP-new-thread", drives.newThread),
  state("SWP-link-workspace", drives.link("workspace", d => d.bench)),
  state("SWP-link-project", drives.link("project", d => d.quiet)),
  state("SWP-link-settings", drives.link("settings", () => "general")),
  state("SWP-drawer-computer", drives.drawer("home")),
  state("SWP-drawer-thread", drives.drawer("thread")),
  state("SWP-hover", drives.hover),
  state("SWP-drop", drives.drop),
  { id: "SWP-long-message", data: longThread, go: async (app, id) => (await app.route.thread(id), await app.park()) },
  { id: "SWP-starting", data: host => ({ thread: host.fixture.threads.starting, workspace: host.fixture.workspaces.starting }), go: drives.starting },
  { id: "SWP-started", data: host => ({ thread: host.fixture.threads.sub, workspace: host.fixture.workspaces.starting }), go: drives.starting },
  borrowed("SWP-reconnecting", byId(wave1, "W1-FRM-027")),
  { id: "SWP-onboarding", home: "empty", go: drives.onboarding },
  borrowed("SWP-first-run", byId(wave2, "W2-DLG-040")),
];
