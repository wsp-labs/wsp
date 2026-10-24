// SPDX-License-Identifier: AGPL-3.0-only
// How today's app is driven to the sweep's states, for wave-sweep/states.mjs and the wave-<n> specs' own rows of the same
// states. Every drive starts where baseline.mjs leaves the window: New thread, at 1280x800.
const css = selector => `css:${selector}`;
const seen = (app, selector, ms = 15_000) => app.waitFor(s => document.querySelector(s) !== null, ms, selector);
const real = fixture => {
  if (fixture.realHost === undefined) throw new Error("the fixture has no realHost; make it with this checkout's bench/fixture.sh");
  return fixture.realHost;
};

/** The ids a hash or a link names, read off the fixture's host once per state. */
async function ids(host) {
  const { workspaces } = await host.op("workspaces.list");
  const { projects } = await host.op("projects.list");
  const project = name => projects.find(p => p.name === name)?.id;
  const bench = workspaces.find(w => w.project?.id === project("bench") && w.kind === "local" && w.worktree === undefined)?.id;
  return { bench, quiet: project("quiet"), panes: real(host.fixture).panesThread };
}

/** A person's message long enough that the transcript folds it under "Show full message"; its thread's reply is one
 * short line, so the whole folded bubble stands above the composer in both apps. */
export const LONG_BRIEF = [
    "The transcript jumps when a long reply finishes streaming, and I think it is the measured heights going stale. Here is what I see on my Mac with my longest thread open, about seven thousand lines long:",
    "1. Scroll up a few screens while a turn is running. 2. Wait for the turn to end. 3. The view lurches by roughly the height of the last reply, sometimes twice, and the scrollbar thumb jumps with it.",
    "It does not happen on short threads, and it does not happen when I stay pinned to the bottom. It got worse after the paging change, so look at how a page that lands above the viewport adjusts the anchor, and whether the row heights cached before a reflow are thrown away when the width changes.",
    "Please write down what you find before changing anything, and add a test that scrolls a long thread while a reply streams in and checks that the first visible row stays put to the pixel.",
].join("\n\n");
export const longThread = host => host.thread(LONG_BRIEF, { reply: { kind: "text", text: "I will write down what I find before I change anything." } });

/** The app reloaded on `hash`, as a link the shell takes or an address written by a pick is read at load. */
async function reloadOn(app, hash, ready) {
  await app.page(h => {
    location.hash = h;
    setTimeout(() => location.reload(), 0);
  }, hash);
  await app.sleep(1500);
  await seen(app, ready, 30_000);
  await app.sleep(1500);
  await app.park();
}

/** The right panel opened on its launcher with no tab, then the pane `kind` (panes.ts) picked from it. */
async function pane(app, kind, ready, words) {
  // The thread's composer can hold the focus a moment after the route, so the toggle is pressed until the panel shows.
  for (let tries = 0; !(await app.page(() => document.querySelector("[data-preview-panel-mode]") !== null)); tries++) {
    if (tries === 3) throw new Error("the right panel would not open");
    await app.key("mod+alt+b");
    await app.sleep(800);
  }
  await app.page(() => {
    for (const close of document.querySelectorAll("[data-right-panel-tab-list] [aria-label^='Close ']")) close.click();
  });
  await seen(app, `[data-surface-launch="${kind}"]`);
  await app.click(css(`[data-surface-launch="${kind}"]`));
  if (ready !== undefined) await seen(app, ready, 20_000);
  if (words !== undefined) await app.waitFor((sel, w) => document.querySelector(sel)?.textContent.includes(w), 30_000, ready, words);
  await app.sleep(2500);
  await app.park();
}

/** A side question asked on `thread` with /btw, which the stub agent refuses. */
export async function askAside(app, thread) {
  await app.route.thread(thread);
  await app.click(css('[data-testid="composer-editor"]'));
  await app.type("/btw what was the last reply about?");
  await app.sleep(400);
  await app.key("enter");
  await app.sleep(300);
  if (!(await app.page(() => document.querySelector('[data-k="aside-surface"]') !== null))) await app.key("enter");
  await seen(app, '[data-k="aside-refused"]', 15_000);
}

/** Each drive by name, for the wave-<n> specs' own rows of the same states. */
export const drives = {
  /** No thread selected: the home route, a project's home, the New thread page of the first project. */
  home: async app => (await app.route.home(), await app.sleep(1500), await app.park()),
  thread: async (app, data) => (await app.route.thread(data.thread), await app.park()),
  newThread: (app, data) => reloadOn(app, `#w/${data.bench}/new`, '[data-testid="composer-editor"]'),
  link: (kind, of) => (app, data) => reloadOn(app, `#open/${kind}/${of(data)}`, kind === "settings" ? "[data-settings-page]" : '[data-testid="composer-editor"]'),
  pane: (kind, ready, words) => async (app, data) => (await app.route.thread(data.panes), await pane(app, kind, ready, words)),
  drawer: where => async (app, data) => {
    if (where === "thread") await app.route.thread(data.thread);
    await app.key("mod+j");
    await seen(app, 'aside[data-terminal-owner="drawer"]', 20_000);
    await app.sleep(3000);
    await app.park();
  },
  /** A sub-thread by its address, as the sidebar folds it under its lead; on starting-host.mjs's host. */
  starting: (app, data) => reloadOn(app, `#w/${data.workspace}/t/${data.thread}`, "[data-spawn-line]"),
  onboarding: async app => (await seen(app, "#start", 20_000), await app.park()),
  hover: async (app, data) => {
    await app.route.thread(data.panes);
    await app.hover(css(`[data-row-id='thread:${data.panes}']`), { wait: 1200 });
  },
  aside: async (app, data) => (await askAside(app, data.panes), await app.park()),
  /** A file held over the thread and never let go: a dragenter carrying one on the chat, which ChatDropZone.tsx hears. */
  drop: async (app, data) => {
    await app.route.thread(data.thread);
    await app.park();
    await app.page(() => {
      const held = new DataTransfer();
      held.items.add(new File(["held"], "held.png", { type: "image/png" }));
      document.querySelector("[data-chat-view]").dispatchEvent(new DragEvent("dragenter", { bubbles: true, cancelable: true, dataTransfer: held }));
    });
    await seen(app, "[data-chat-drop-zone]", 5_000);
  },
};

export const withIds = (thread = "short-1") => async host => ({ ...(await ids(host)), thread: host.fixture.threads[thread] ?? thread });
