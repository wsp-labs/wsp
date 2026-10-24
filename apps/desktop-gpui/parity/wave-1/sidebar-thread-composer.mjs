// SPDX-License-Identifier: AGPL-3.0-only
// The sidebar, thread view and composer states of wave-1 with a baseline of their own. Each starts
// on New thread at 1280x800. A state's own threads and projects are made in its data and taken out in its cleanup, so
// the fixture's threads stay as fixture.sh made them; what a state leaves in the window (a draft, a stash, a held
// message) its tidy undoes after the capture.
import { rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { deflateSync } from "node:zlib";

const sleep = ms => new Promise(done => setTimeout(done, ms));

const SIDEBAR = "[data-slot=sidebar-container]";
const COMPOSER = "[data-chat-composer]";
const DRAWER = "[data-composer-drawer-layer]";
const BARS = "[data-composer-bar]";
const POPOVER = "[data-slot=popover-popup]";
const TOOLTIP = "[data-slot=tooltip-popup]";
const HEADER = "[data-shell-center] [data-header-row]";
const VIEW = "[data-chat-view]";
const EDITOR = `${COMPOSER} [contenteditable=true]`;
const row = id => `[data-row-id='thread:${id}']`;
const fixtureRow = (app, which) => row(app.fixture.threads[which]);

/** The box around everything the selectors find, grown by `pad` and kept inside the window, in CSS px. */
const around = (app, selectors, pad = 16) =>
  app.page(
    (selectors, pad) => {
      let box = null;
      for (const s of selectors)
        for (const el of document.querySelectorAll(s)) {
          const r = el.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) continue;
          box = box === null ? { l: r.left, t: r.top, r: r.right, b: r.bottom } : { l: Math.min(box.l, r.left), t: Math.min(box.t, r.top), r: Math.max(box.r, r.right), b: Math.max(box.b, r.bottom) };
        }
      if (box === null) throw new Error(`nothing on the page for ${selectors.join(", ")} to clip to`);
      const l = Math.max(0, Math.floor(box.l - pad));
      const t = Math.max(0, Math.floor(box.t - pad));
      const r = Math.min(innerWidth, Math.ceil(box.r + pad));
      const b = Math.min(innerHeight, Math.ceil(box.b + pad));
      return { x: l, y: t, width: r - l, height: b - t };
    },
    selectors,
    pad,
  );
/** The whole sidebar, for a state about its foot or its scroll. */
const sidebar = app => around(app, [SIDEBAR], 0);
/** The sidebar from its New thread row down to the last row of its list, and what opens beside it: the logo row above
 * and the cloud card below are left out unless the state is about them. */
const sidebarList = (...more) => async app => {
  const box = await around(app, [`${SIDEBAR} [data-sidebar-search]`, `${SIDEBAR} [data-sidebar-tree] > li`, ...more], 12);
  const right = more.length > 0 ? (await around(app, [SIDEBAR, ...more], 12)).width : await app.page(sel => Math.ceil(document.querySelector(sel).getBoundingClientRect().right), SIDEBAR);
  return { ...box, x: 0, width: right };
};
/** The composer's box with what docks on it and the line under it. */
const BOX = [`${COMPOSER} [data-slot=composer-shell]`, "[data-prompt-dock]", "[data-composer-under]", DRAWER, BARS];
const composer = (...more) => app => around(app, [...BOX, ...more], 12);

/** Marks the one element `pick` finds in the page, so a click or hover can name it by `[data-parity-pick]`. */
const mark = async (app, pick, ...args) => {
  const found = await app.page(
    (pick, args) => {
      for (const el of document.querySelectorAll("[data-parity-pick]")) el.removeAttribute("data-parity-pick");
      const el = new Function(`return (${pick})`)()(...args);
      if (el === null || el === undefined) return false;
      el.setAttribute("data-parity-pick", "");
      return true;
    },
    String(pick),
    args,
  );
  if (!found) throw new Error(`nothing on the page for ${String(pick)}`);
  return "[data-parity-pick]";
};
/** Presses a chord until what it opens stands: a chord pressed while the page is still taking a thread in can go unheard. */
async function chord(app, keys, selector) {
  for (let i = 0; ; i++) {
    await app.key(keys);
    try {
      return await seen(app, selector, 3_000);
    } catch (e) {
      if (i === 2) throw e;
    }
  }
}

/** Holds what the pointer rests on as it is: a clipped capture moves the pointer to the window's centre first, which
 * would close a hover card or a tooltip and disarm a Stop. Undone by `thaw`, in the state's tidy. */
const POINTER = ["pointerover", "pointerout", "pointerenter", "pointerleave", "pointermove", "mouseover", "mouseout", "mouseenter", "mouseleave", "mousemove", "blur", "focusout"];
const freeze = app =>
  app.page(types => {
    window.parityHold = e => e.stopImmediatePropagation();
    for (const type of types) window.addEventListener(type, window.parityHold, true);
  }, POINTER);
const thaw = app =>
  app.page(types => {
    for (const type of types) window.removeEventListener(type, window.parityHold, true);
  }, POINTER);

/** A mark at a point of the page that the pointer passes through, for a hover where no element names the place. */
const spot = async (app, at) => {
  await app.page(at => {
    const { x, y } = new Function(`return (${at})`)()();
    document.querySelector("[data-parity-spot]")?.remove();
    const el = document.createElement("div");
    el.setAttribute("data-parity-spot", "");
    el.style.cssText = `position:fixed;left:${x - 1}px;top:${y - 1}px;width:2px;height:2px;pointer-events:none`;
    document.body.append(el);
  }, String(at));
  return "[data-parity-spot]";
};
const seen = (app, selector, ms = 10_000) => app.waitFor(selector => document.querySelector(selector) !== null, ms, selector);
const gone = (app, selector, ms = 10_000) => app.waitFor(selector => document.querySelector(selector) === null, ms, selector);

const HELD = new Set(["question", "permission", "hold"]);
const partsOf = reply => (Array.isArray(reply) ? reply : [reply]).map(p => (typeof p === "string" ? { kind: p } : p));
const until = async (what, fn, ms = 30_000) => {
  for (const end = Date.now() + ms; ; await sleep(250)) {
    const got = await fn();
    if (got) return got;
    if (Date.now() > end) throw new Error(`${what} not seen in ${ms / 1000} s`);
  }
};
const threadOf = async (host, id) => (await host.threads()).find(t => t.id === id || t.threadId === id);

/** The statuses of a turn that has ended; a thread listed before its turn starts has none of them yet. */
const ENDED = new Set(["completed", "failed", "interrupted"]);

/** A thread of the state's own on `project`, its first message `prompt` and its agent playing `reply`: its id, once
 * the turn has ended, or for a reply that holds, once it runs (and asks, for a question or a permission). */
async function made(host, prompt, reply, { title, project = "bench", flags = [] } = {}) {
  if (reply !== undefined) host.script(prompt, reply);
  const parts = reply === undefined ? [] : partsOf(reply);
  const held = parts.some(p => HELD.has(p.kind));
  const asks = parts.some(p => p.kind === "question" || p.kind === "permission");
  const said = await host.wsp("run", project, "--agent", "claude", ...(title !== undefined ? ["--title", title] : []), ...flags, "--detach", prompt);
  const id = /\b([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b/.exec(said)?.[1];
  if (id === undefined) throw new Error(`wsp run printed no thread: ${said.trim().slice(0, 200)}`);
  await until(`thread ${title ?? prompt} ${held ? "running" : "finished"}`, async () => {
    const t = await threadOf(host, id);
    return t !== undefined && (held ? t.status === "running" && (!asks || t.asking !== undefined) : ENDED.has(t.status));
  }).catch(async e => {
    await drop(host, id);
    throw e;
  });
  // The stub's held part comes after the lines before it; the host has them by the time the turn reads as running.
  if (held) await sleep(500);
  return id;
}
/** A thread left working: the title every state with a running turn of its own shows. */
const WORKING = "Run the tests again and fix what fails";
const working = (host, reply = [{ kind: "text", text: "I will run the tests first." }, "hold"], prompt = "Run the tests again and fix what fails.") => made(host, prompt, reply, { title: WORKING });
/** Ends a thread's running turn and takes the thread away; a message the window queued behind the stopped turn
 * starts another, which is stopped in turn. */
async function drop(host, id) {
  if (id === undefined) return;
  for (let i = 0; i < 5; i++) {
    if ((await threadOf(host, id)) === undefined) return;
    if ((await threadOf(host, id))?.status === "running") {
      await host.wsp("stop", id).catch(() => {});
      await until(`thread ${id} stopped`, async () => (await threadOf(host, id))?.status !== "running", 15_000).catch(() => {});
    }
    if (await host.op("sessions.delete", { threadId: id }).then(() => true, () => false)) return;
    await sleep(1_000);
  }
}
const dropAll = async (host, ids) => {
  for (const id of [ids ?? []].flat()) await drop(host, id);
};

/** Nothing focused, the sidebar shown, the drawer and right panel shut. */
async function fresh(app) {
  await app.page(async () => {
    document.activeElement?.blur();
    const pause = () => new Promise(done => setTimeout(done, 250));
    if (innerWidth >= 768 && document.querySelector("[data-sidebar-state=collapsed]") !== null) {
      document.querySelector("[data-sidebar=trigger]")?.click();
      await pause();
    }
    // An inline right panel takes the header's toggles with it, so it is shut by its chord below.
    for (const label of ["Toggle terminal drawer", "Toggle right panel"]) {
      const on = document.querySelector(`[aria-label='${label}'][aria-pressed=true]`);
      if (on !== null) {
        on.click();
        await pause();
      }
    }
  });
  if (await app.page(() => document.querySelector("[data-preview-panel-mode]") !== null)) await app.key("mod+alt+b");
}

/** A thread opened from the sidebar, the pointer off the window. */
const open = async (app, which) => {
  await app.route.thread(which);
  await app.park();
};
/** Opens a thread whose turn has ended and holds until the window has the whole turn drawn: no Stop in the composer,
 * the reply's time under its last reply, and, where the turn worked, its "Worked for" fold. The route waits for the
 * first row alone, and a busy host hands the window the rest later. */
const openEnded = async (app, id, { fold = false } = {}) => {
  await open(app, id);
  await app.waitFor(
    fold => {
      if (document.querySelector("[data-chat-composer] button[aria-label^='Stop']") !== null) return false;
      if (fold && !/^Worked for/.test(document.querySelector("[data-timeline-row-kind=turn-fold] button")?.textContent ?? "")) return false;
      return [...document.querySelectorAll("[data-message-role=assistant]")].at(-1)?.querySelector("[data-reply-time]") != null;
    },
    30_000,
    fold,
  );
};
/** Clicks the toggle `selector` names until it reads open: a click that lands while the rows above still move goes
 * to the row under it. */
async function expand(app, selector) {
  for (let i = 0; i < 3; i++) {
    if (await app.page(sel => document.querySelector(sel)?.getAttribute("aria-expanded") === "true", selector)) return;
    await app.click(selector);
    try {
      return await app.waitFor(sel => document.querySelector(sel)?.getAttribute("aria-expanded") === "true", 3_000, selector);
    } catch {}
  }
  throw new Error(`${selector} did not open after three clicks`);
}
/** Opens the ended turn's "Worked for" fold and holds until the row `inside` it is drawn. */
const unfold = async (app, inside) => {
  await expand(app, "[data-timeline-row-kind=turn-fold] button");
  await seen(app, inside, 15_000);
};
/** Empties the composer of a thread (New thread without one) as a person would: select all, delete, until empty. */
async function clearDraft(app, which) {
  if (which === undefined) await app.route.home();
  else await app.route.thread(which);
  for (let i = 0; i < 4; i++) {
    const drafted = await app.page(sel => {
      const editor = document.querySelector(sel);
      if (editor === null || (editor.textContent.trim() === "" && editor.querySelector("[data-lexical-decorator]") === null)) return false;
      editor.focus();
      document.execCommand("selectAll");
      return true;
    }, EDITOR);
    if (!drafted) return;
    await app.key("backspace");
    await app.sleep(200);
  }
}
const tidyShort1 = app => clearDraft(app, "short-1");

/** Opens a fixture thread and types into its composer. */
const typeIn = async (app, which, text) => {
  await open(app, which);
  await app.click(EDITOR);
  if (text !== "") await app.type(text);
};

/** The rows of the newest turn: the person's last message and everything after it. */
const lastTurn = app =>
  app.page(() => {
    const rows = [...document.querySelectorAll("[data-timeline-row-id]")];
    const at = rows.findLastIndex(el => el.getAttribute("data-message-role") === "user");
    rows.forEach((el, i) => (i >= Math.max(0, at) ? el.setAttribute("data-parity-turn", "") : el.removeAttribute("data-parity-turn")));
    return rows.length - at;
  });
/** A clip of transcript rows, kept below the thread's title row and, unless the composer is asked for, above it. */
const rows = (selectors, pad = 12) => async app => {
  const box = await around(app, selectors, pad);
  const [top, bottom] = await app.page(
    (view, composer, keep) => [document.querySelector(view)?.getBoundingClientRect().top ?? 0, keep ? innerHeight : (document.querySelector(composer)?.getBoundingClientRect().top ?? innerHeight)],
    VIEW,
    COMPOSER,
    selectors.some(sel => sel.startsWith(COMPOSER)),
  );
  const y = Math.max(box.y, Math.floor(top));
  return { ...box, y, height: Math.min(box.y + box.height, Math.ceil(bottom)) - y };
};
const turnClip = (...more) => async app => {
  await lastTurn(app);
  return rows(["[data-parity-turn]", ...more])(app);
};

/** Two files in bench's folder for the @ menu to offer, written by a turn of the stub's, its thread then taken away. */
const benchFiles = async host => drop(host, await made(host, "Write the two anchor files.", ["files", { kind: "text", text: "Wrote both files." }], { title: "Write the two anchor files" }));
const noBenchFiles = host => rmSync(join(host.home, "bench", "src"), { recursive: true, force: true });
const LONG_PROJECT = "the nightly release pipeline for every package and platform";
const LONG_TITLE = "A thread title long enough to run past every edge the app gives it, so the tile, the crumb and the card each have to cut it somewhere; it keeps going with more words until it reaches two hundred chars".padEnd(200, ".").slice(0, 200);
const LONG_MESSAGE = Array.from({ length: 40 }, (_, i) => `Line ${i + 1} of the long message: the list keeps every row it has measured, so nothing jumps when output lands.`).join("\n").slice(0, 2000);
const INSIGHT = ["The anchor holds now.", "", "`★ Insight ─────────────────────────────────────`", "- The list measures a row after it paints, so a late height moves what sits above it.", "- Holding the anchor through the resize keeps the reader's place.", "- A measured height is kept per row, so paging back is free.", "`─────────────────────────────────────────────────`", "", "Run `pnpm test` to check it."].join("\n");
const MATH = ["The energy is $E=mc^2$, and the area under the line is", "", "$$\\int_0^1 x\\,dx$$", "", "The flow:", "", "```mermaid", "graph TD; A-->B", "```"].join("\n");

/** A 320x200 PNG of two flat colours, written where the fixture's command line can send it. */
function shot(host) {
  const w = 320;
  const h = 200;
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const at = y * (w * 3 + 1) + 1 + x * 3;
      const [r, g, b] = x < w / 2 ? [64, 112, 200] : [232, 168, 56];
      raw[at] = r;
      raw[at + 1] = g;
      raw[at + 2] = b;
    }
  const chunk = (type, body) => {
    const out = Buffer.alloc(12 + body.length);
    out.writeUInt32BE(body.length, 0);
    out.write(type, 4, "latin1");
    body.copy(out, 8);
    out.writeUInt32BE(crc(Buffer.concat([Buffer.from(type, "latin1"), body])), 8 + body.length);
    return out;
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  const png = Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
  const path = join(host.home, "shot.png");
  writeFileSync(path, png);
  return path;
}
function crc(buf) {
  let c = ~0;
  for (const byte of buf) {
    c ^= byte;
    for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1;
  }
  return ~c >>> 0;
}

/** Waits for `selector`, and where it does not come because the host has no wsp-daemon (the app says so in a notice),
 * throws that reason instead: `what` is what the daemon would have given. */
async function viaDaemon(app, selector, what, ms = 15_000) {
  try {
    await seen(app, selector, ms);
  } catch (e) {
    const missing = await app.page(() => [...document.querySelectorAll("[data-notice]")].some(n => n.textContent.includes("wsp-daemon binary missing")));
    if (missing) throw new Error(`the fixture's host has no wsp-daemon, which ${what}`);
    throw e;
  }
}

/** Picks an option of the project switcher by the project's name. */
async function pickProject(app, name) {
  await app.click("[data-k=project-switcher]");
  await seen(app, "[data-project-switcher-menu]");
  await app.click(await mark(app, name => [...document.querySelectorAll("[data-project-switcher-menu] [role=option]")].find(el => el.textContent.trim() === name), name));
  await gone(app, "[data-project-switcher-menu]");
}

/** Opens short-3 and settles it with its chord. */
async function settle(app) {
  await open(app, "short-3");
  await app.key("mod+shift+e");
  await viaDaemon(app, "[data-drop-settled]:not([data-drop-only])", "the app's settle asks first for what still runs beside the thread");
}
const restoreShort3 = host => host.op("sessions.restore", { threadIds: [host.fixture.threads["short-3"]] });

/** Some threads of the state's own, each finished. */
const threads = async (host, n, tag) => {
  const ids = [];
  for (let i = 1; i <= n; i++) ids.push(await made(host, `Check sidebar case ${tag}-${i} and say what you find.`, { kind: "text", text: `Case ${i} reads fine.` }, { title: `Sidebar case ${i}` }));
  return ids;
};

const states = [
  // The sidebar.
  {
    id: "W1-SBR-018",
    data: host => host.project("e"),
    go: async app => {
      await pickProject(app, "e");
      await seen(app, "[data-k=no-workspaces]");
    },
    clip: sidebarList(),
    cleanup: host => host.removeProject("e"),
  },
  {
    id: "W1-SBR-019",
    data: host => threads(host, 5, "a"),
    go: async app => {
      await seen(app, "[data-row-id^='thread:']");
      await app.scroll("[data-sidebar-tree]", 400);
      await app.park();
    },
    clip: sidebar,
    cleanup: dropAll,
  },
  {
    id: "W1-SBR-020",
    data: async host => {
      await host.project(LONG_PROJECT);
      return made(host, "Read the long names and report back.", { kind: "text", text: "The long names read fine." }, { project: LONG_PROJECT, title: LONG_TITLE });
    },
    go: (app, id) => seen(app, row(id)),
    clip: sidebarList(),
    cleanup: async (host, id) => {
      await drop(host, id);
      await host.removeProject(LONG_PROJECT);
    },
  },
  {
    id: "W1-SBR-022",
    data: host => working(host, "question"),
    go: (app, id) => seen(app, `${row(id)} [data-thread-status]`),
    clip: sidebarList(),
    cleanup: drop,
  },
  {
    id: "W1-SBR-023",
    data: host => made(host, "Run the failing tests and report.", "error", { title: "Run the failing tests" }),
    go: (app, id) => seen(app, `${row(id)} [data-thread-status=failed]`),
    clip: sidebarList(),
    cleanup: drop,
  },
  {
    id: "W1-SBR-024",
    data: host => made(host, "hi"),
    go: (app, id) => seen(app, `${row(id)} [data-thread-status=done]`),
    clip: sidebarList(),
    cleanup: drop,
  },
  {
    id: "W1-SBR-026",
    go: async app => {
      await app.hover(fixtureRow(app, "short-2"), { wait: 500 });
      await seen(app, "[data-tile-card]");
      await freeze(app);
    },
    clip: sidebarList("[data-tile-card]"),
    tidy: thaw,
  },
  {
    id: "W1-SBR-027",
    go: async app => {
      await app.click(`${fixtureRow(app, "short-1")} [data-thread-title]`, { count: 2 });
      await seen(app, `${SIDEBAR} input`);
    },
    clip: sidebarList(),
  },
  {
    id: "W1-SBR-028",
    go: async app => {
      await settle(app);
      await app.park();
    },
    clip: sidebarList(),
    cleanup: restoreShort3,
  },
  {
    id: "W1-SBR-029",
    go: async app => {
      await settle(app);
      await app.click("[data-drop-settled] button");
      await seen(app, `[data-drop-settled] ~ li ${fixtureRow(app, "short-3")}`);
      await app.park();
    },
    clip: sidebarList(),
    cleanup: restoreShort3,
  },
  {
    id: "W1-SBR-030",
    go: async app => {
      const top = await app.page(() => document.querySelector("[data-sidebar-tree]").getBoundingClientRect().top);
      await app.drag(fixtureRow(app, "short-2"), { x: 128, y: top + 14 }, { hold: true });
      await seen(app, "[data-tree-dragging]");
      // The heads show a frame after the drag starts, so the drag passes over the Pinned head again once it stands.
      await app.page(() => {
        const head = document.querySelector("[data-section=pinned]");
        const r = head.getBoundingClientRect();
        head.dispatchEvent(new DragEvent("dragover", { bubbles: true, cancelable: true, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2 }));
      });
      await seen(app, "[data-section=pinned][data-drop-over]");
    },
    clip: sidebarList(),
  },
  {
    id: "W1-SBR-033",
    data: host => working(host, [{ kind: "tools", names: ["Task"], n: 1 }, "hold"]),
    go: (app, id) => seen(app, `${row(id)} ~ *, [data-row-id^='subagent:']`),
    clip: sidebarList(),
    cleanup: drop,
  },
  {
    id: "W1-SBR-034",
    go: async app => {
      await app.click("[data-k=computer-switcher]");
      await seen(app, "[data-computer-switcher-menu]");
    },
    clip: app => around(app, ["[data-sidebar-search]", POPOVER], 8),
  },
  {
    id: "W1-SBR-035",
    go: async app => {
      await app.click(fixtureRow(app, "short-1"));
      await app.key("arrowdown");
      await app.park();
    },
    clip: sidebarList(),
  },
  {
    id: "W1-SBR-036",
    data: host => working(host),
    go: async (app, id) => {
      await app.hover(row(id));
      await app.click("Stop thread");
      await app.click("Press again to stop");
      await seen(app, `${row(id)} [data-thread-status=stopped]`);
      await app.park();
    },
    clip: sidebarList(),
    cleanup: drop,
  },
  {
    // A fresh host holds no project, so there is no bench to remove; onboarding is passed to reach the window.
    id: "W1-SBR-107",
    home: "empty",
    go: async app => {
      await app.click("#start");
      await app.waitFor(() => document.querySelector("#agents:not([hidden])") !== null, 10_000);
      await app.click("#open");
      await app.window("app");
      await seen(app, "[data-k=new-project]", 20_000);
      await app.park();
    },
    clip: sidebarList(),
  },
  {
    id: "W1-SBR-110",
    go: async app => {
      await app.click("[data-k=project-switcher]");
      await seen(app, "[data-project-switcher-menu]");
    },
    clip: app => around(app, ["[data-sidebar-search]", POPOVER], 8),
  },
  {
    id: "W1-SBR-111",
    data: host => working(host),
    go: async (app, id) => {
      await app.hover(row(id));
      // The arm lapses after 2 s, sooner than the capture comes; its timer is held off until the state's tidy.
      await app.page(() => {
        const set = (window.parityTimeout = window.setTimeout);
        window.setTimeout = (fn, ms, ...args) => (ms === 2000 ? 0 : set(fn, ms, ...args));
      });
      await app.click("Stop thread");
      await seen(app, "[data-stop-act][data-armed]");
      await freeze(app);
    },
    clip: sidebarList(),
    cleanup: drop,
    tidy: async app => {
      await thaw(app);
      await app.page(() => window.parityTimeout !== undefined && (window.setTimeout = window.parityTimeout));
    },
  },
  {
    id: "W1-SBR-112",
    data: async host => {
      await host.pref("cloudCardDismissed", true);
      return threads(host, 7, "b");
    },
    go: app => gone(app, "[data-cloud-card]"),
    clip: sidebar,
    cleanup: async (host, ids) => {
      await dropAll(host, ids);
      await host.pref("cloudCardDismissed", false);
    },
  },
  {
    id: "W1-SBR-116",
    data: host => working(host),
    go: async (app, id) => {
      await app.menu(row(id), "Snooze thread");
      await app.click("In 1 hour");
      await seen(app, `${row(id)}`);
      await app.park();
    },
    clip: sidebarList(),
    cleanup: drop,
  },
  {
    id: "W1-SBR-117",
    go: async app => {
      await app.menu(fixtureRow(app, "short-2"), "Pin thread");
      await seen(app, `[data-section=pinned] ${fixtureRow(app, "short-2")}`);
      await app.park();
    },
    clip: sidebarList(),
    cleanup: host => host.op("sessions.mark", { threadIds: [host.fixture.threads["short-2"]], marks: { pinned: false } }),
  },
  {
    id: "W1-SBR-124",
    data: async host => {
      await host.project("e");
      return host;
    },
    go: async (app, host) => {
      await pickProject(app, "e");
      await seen(app, "[data-k=no-workspaces]");
      await host.removeProject("e");
      await app.waitFor(() => document.querySelector("[data-k=project-switcher] [data-switcher-name]")?.textContent.trim() === "All projects", 10_000);
      await app.park();
    },
    clip: sidebarList(),
    cleanup: host => host.removeProject("e"),
  },

  // The thread view.
  { id: "W1-THR-022", go: app => open(app, "short-1") },
  {
    id: "W1-THR-023",
    go: async app => {
      await open(app, "long");
      const first = "Make the code blocks wrap instead of scrolling sideways. (long, turn 1)";
      for (const end = Date.now() + 90_000; ; ) {
        const top = await app.page(first => {
          const root = document.querySelector("[data-timeline-root]");
          let scroller = root?.parentElement;
          while (scroller !== null && scroller !== undefined && scroller.scrollHeight <= scroller.clientHeight) scroller = scroller.parentElement;
          if (scroller === null || scroller === undefined) return false;
          scroller.scrollTop = 0;
          return [...document.querySelectorAll("[data-timeline-row-id]")].some(el => el.textContent.includes(first)) && scroller.scrollTop === 0;
        }, first);
        if (top) break;
        if (Date.now() > end) throw new Error("long's first turn was not reached by scrolling up for 90 s");
        await app.sleep(400);
      }
      await app.sleep(800);
    },
    clip: async app => ({ ...(await around(app, [VIEW], 0)), height: 380 }),
  },
  {
    id: "W1-THR-024",
    data: host => working(host),
    go: open,
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-025",
    data: host => made(host, "Run the failing tests and report.", "error", { title: "Run the failing tests" }),
    go: async (app, id) => {
      await open(app, id);
      await app.click("[data-timeline-row-kind=turn-fold] button");
      await app.click("[data-timeline-row-kind=work-toggle] button");
      await seen(app, "[data-timeline-row-kind=work-toggle] button[aria-expanded=true]");
      await app.park();
    },
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-026",
    data: host => made(host, LONG_MESSAGE, { kind: "text", text: "Read it all." }, { title: "A long message" }),
    go: open,
    clip: rows(["[data-message-role=user]"], 8),
    cleanup: drop,
  },
  {
    id: "W1-THR-027",
    data: host => made(host, "Show the whole row file.", { kind: "code", lines: 400 }, { title: "Show the whole row file" }),
    go: async (app, id) => {
      await openEnded(app, id);
      await seen(app, "[aria-label='Disable line wrap']", 15_000);
      await app.click("Disable line wrap");
      await seen(app, "[aria-label='Wrap lines']", 15_000);
      await app.park();
    },
    clip: async app => {
      const box = await rows([".chat-markdown-codeblock"], 8)(app);
      return { ...box, height: Math.min(box.height, 340) };
    },
    cleanup: drop,
  },
  {
    id: "W1-THR-028",
    data: host => made(host, "Tabulate the changes by file.", { kind: "table", columns: 12 }, { title: "Tabulate the changes" }),
    go: open,
    clip: rows(["[data-timeline-row-id] table"], 16),
    cleanup: drop,
  },
  {
    id: "W1-THR-029",
    data: host => made(host, "Trace every scroll write and fix them.", { kind: "tools", n: 40, names: ["Bash", "Read", "Edit", "Grep"] }, { title: "Trace every scroll write" }),
    go: open,
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-030",
    data: host => made(host, "Trace every scroll write and fix them.", { kind: "tools", n: 40, names: ["Bash", "Read", "Edit", "Grep"] }, { title: "Trace every scroll write" }),
    go: async (app, id) => {
      await open(app, id);
      await app.click("[data-timeline-row-kind=turn-fold] button");
      await seen(app, "[data-timeline-row-kind=work-toggle] button");
      await app.click("[data-timeline-row-kind=work-toggle] button");
      await seen(app, "[data-call-row]");
      await app.click(await mark(app, () => document.querySelector("[data-call-row] button[aria-label^='Ran ']")));
      await seen(app, "[data-call-row=open] [data-call-output]");
      await app.sleep(400);
      await app.click(await mark(app, () => document.querySelector("[data-call-row=closed] button[aria-label^='Edited ']")));
      await seen(app, "[data-call-row=open] [data-call-hunks]");
      await app.park();
    },
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-032",
    go: async app => {
      await open(app, "long");
      await chord(app, "mod+f", "[data-thread-find]");
      await app.type("scrollTop");
      await app.waitFor(() => /\d/.test(document.querySelector("[data-find-count]")?.textContent ?? ""), 15_000);
    },
    clip: app => around(app, ["[data-thread-find]"], 16),
  },
  {
    id: "W1-THR-033",
    go: async app => {
      await open(app, "long");
      await chord(app, "mod+f", "[data-thread-find]");
      await app.type("qqzz");
      await app.waitFor(() => document.querySelector("[data-thread-find]")?.getAttribute("data-thread-find") === "whole" && (document.querySelector("[data-find-count]")?.textContent ?? "") !== "", 30_000);
    },
    clip: app => around(app, ["[data-thread-find]"], 16),
  },
  {
    id: "W1-THR-034",
    go: async app => {
      await open(app, "short-1");
      await app.hover("[data-minimap-strip]");
      await seen(app, "[data-minimap-preview]");
      await freeze(app);
    },
    clip: async app => {
      const box = await around(app, ["[data-minimap-preview]"], 48);
      const left = await app.page(() => document.querySelector("[data-testid=timeline-minimap]").getBoundingClientRect().left);
      return { ...box, x: Math.floor(left), width: box.x + box.width - Math.floor(left) };
    },
    tidy: thaw,
  },
  {
    id: "W1-THR-035",
    go: async app => {
      await open(app, "short-1");
      const p = await mark(app, () => [...document.querySelectorAll("[data-message-role=assistant] [data-find-entry] p")].at(-1));
      const end = await app.page(sel => {
        const r = document.querySelector(sel).getBoundingClientRect();
        return { x: r.right - 4, y: r.top + 10 };
      }, p);
      await app.drag(p, end);
      await seen(app, "[data-quote-selection]");
    },
    clip: app => around(app, ["[data-parity-pick]", "[data-quote-selection]"], 10),
  },
  {
    id: "W1-THR-036",
    go: async app => {
      await open(app, "short-1");
      await app.hover(await mark(app, () => [...document.querySelectorAll("[data-message-role=assistant]")].at(-1)));
      await freeze(app);
    },
    clip: app => around(app, ["[data-parity-pick]"], 8),
    tidy: thaw,
  },
  {
    id: "W1-THR-039",
    data: host => working(host, ["retry", { kind: "hold", call: false }]),
    go: async (app, id) => {
      await open(app, id);
      await app.waitFor(() => /trying again/i.test(document.querySelector("[data-chat-view]")?.textContent ?? ""), 10_000).catch(async e => {
        if (await app.page(() => /waiting for .* to answer/.test(document.querySelector("[data-chat-view]")?.textContent ?? ""))) throw new Error("the turn reads as waiting for its computer to answer, which a host without a wsp-daemon says over the retry line");
        throw e;
      });
    },
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-040",
    go: async app => {
      await open(app, "short-1");
      await app.hover("[data-context-ring]");
      await seen(app, POPOVER);
      await freeze(app);
    },
    clip: app => around(app, [HEADER, POPOVER], 8),
    tidy: thaw,
  },
  {
    id: "W1-THR-041",
    go: async app => {
      await open(app, "short-1");
      await viaDaemon(app, "[data-git-more]", "reads the folder's checkout the git split is drawn from");
      await app.click("[data-git-more]");
      await seen(app, "[data-git-menu]");
    },
    clip: app => around(app, ["[data-git-more]", "[data-git-menu]"], 16),
  },
  {
    id: "W1-THR-124",
    data: host => made(host, "Clear the build folder and rebuild.", "permission", { title: "Clear the build folder", flags: ["--access", "ask"] }),
    go: open,
    clip: turnClip(...BOX),
    cleanup: drop,
  },
  {
    id: "W1-THR-125",
    data: host => made(host, "Think about why the anchor moves.", ["thinking", { kind: "text", text: "The anchor moves because rows are measured after paint." }], { title: "Why the anchor moves" }),
    go: async (app, id) => {
      await openEnded(app, id, { fold: true });
      await unfold(app, "[data-timeline-row-kind=work-toggle] button");
      await expand(app, "[data-timeline-row-kind=work-toggle] button");
      await seen(app, "[data-timeline-row-kind=work] [aria-expanded]", 15_000);
      await expand(app, "[data-timeline-row-kind=work] [aria-expanded]");
      await seen(app, "[data-timeline-row-id] pre", 15_000);
      await app.park();
    },
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-126",
    data: host => made(host, "Plan the anchor fix.", "plan", { title: "Plan the anchor fix" }),
    go: open,
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-127",
    data: host => made(host, "Trace the scroll writes with a helper.", { kind: "tools", names: ["Task"], n: 1, listed: false }, { title: "Trace the scroll writes" }),
    go: async (app, id) => {
      await openEnded(app, id, { fold: true });
      await unfold(app, "[data-subagent-trigger]");
      await app.click("[data-subagent-trigger]");
      await seen(app, "[data-subagent-body]", 15_000);
      await app.park();
    },
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-128",
    data: host => made(host, "What is in this picture?", { kind: "text", text: "Two flat colours, blue and amber." }, { title: "What is in this picture", flags: ["--file", shot(host)] }),
    go: async (app, id) => {
      await open(app, id);
      await app.click("[data-chat-image] button");
      await seen(app, "[data-slot=dialog-popup]");
      await app.park();
    },
    clip: app => around(app, ["[data-slot=dialog-popup]"], 48),
    cleanup: drop,
  },
  {
    id: "W1-THR-129",
    data: host => made(host, "Explain the anchor fix with an insight.", { kind: "text", text: INSIGHT }, { title: "Explain the anchor fix" }),
    go: open,
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-130",
    data: host => made(host, "Show the formula and the flow.", { kind: "text", text: MATH }, { title: "Show the formula and the flow" }),
    go: async (app, id) => {
      await open(app, id);
      await seen(app, "[data-timeline-row-id] svg[id^=mermaid], [data-timeline-row-id] .katex", 15_000);
      await app.sleep(1_000);
    },
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-131",
    data: host => working(host),
    go: async (app, id) => {
      await open(app, id);
      await app.click(`${COMPOSER} button[aria-label^='Stop']`);
      await app.waitFor(() => /You stopped/.test(document.querySelector("[data-chat-view]")?.textContent ?? ""), 15_000);
      await app.park();
    },
    clip: turnClip(),
    cleanup: drop,
  },
  {
    id: "W1-THR-132",
    data: host => made(host, "Write the two anchor files.", ["files", { kind: "text", text: "Wrote both files." }], { title: "Write the two anchor files" }),
    go: async (app, id) => {
      await open(app, id);
      await viaDaemon(app, "[data-changed-files-state]", "reads what the turn changed in its folder");
    },
    clip: turnClip(),
    cleanup: async (host, id) => {
      await drop(host, id);
      noBenchFiles(host);
    },
  },
  {
    id: "W1-THR-133",
    go: async app => {
      await open(app, "short-1");
      await app.click("[data-breadcrumb-thread]", { count: 2 });
      await seen(app, `${HEADER} input`);
    },
    clip: app => around(app, [HEADER], 0),
  },
  {
    id: "W1-THR-134",
    go: async app => {
      await open(app, "short-1");
      await chord(app, "mod+f", "[data-thread-find]");
      await chord(app, "alt+r", "[data-find-toggle=regex][aria-pressed=true]");
      await app.type("(");
      await seen(app, "[data-find-note]");
    },
    clip: app => around(app, ["[data-thread-find]"], 16),
  },
  {
    id: "W1-THR-135",
    go: async app => {
      await open(app, "short-1");
      await chord(app, "mod+alt+b", "[data-preview-panel-mode]");
      await app.hover(await spot(app, () => {
        const r = document.querySelector("[data-chat-view]").getBoundingClientRect();
        return { x: r.left + 30, y: r.top + r.height / 2 };
      }));
    },
  },
  {
    id: "W1-THR-136",
    data: host => made(host, "Trace the scroll writes with a helper.", { kind: "tools", names: ["Task"], n: 1, listed: false }, { title: "Trace the scroll writes" }),
    go: async (app, id) => {
      await openEnded(app, id, { fold: true });
      await unfold(app, "[data-subagent-trigger]");
      await app.click("[data-subagent-trigger]");
      await seen(app, "[data-subagent-body]", 15_000);
      await app.click("Open its page");
      await app.waitFor(() => /\/a\//.test(location.hash) && document.body.textContent.includes("Back to lead") && [...document.querySelectorAll("[data-timeline-row-id]")].some(el => el.textContent.includes("the one at line 418 runs on resize")), 15_000);
      await app.park();
    },
    clip: app => around(app, ["[data-shell-center]"], 0),
    cleanup: drop,
  },

  // The composer.
  {
    id: "W1-CMP-018",
    go: async app => {
      await app.click(EDITOR);
      for (let i = 1; i <= 12; i++) {
        await app.type(`Line ${i} of a draft that grows the box`);
        if (i < 12) await app.key("shift+enter");
      }
    },
    clip: composer(),
    tidy: app => clearDraft(app),
  },
  {
    id: "W1-CMP-019",
    go: async app => {
      await typeIn(app, "short-1", "x");
      await open(app, "short-2");
      await open(app, "short-1");
    },
    clip: composer(),
    tidy: tidyShort1,
  },
  {
    id: "W1-CMP-020",
    go: async app => {
      await typeIn(app, "short-1", "/");
      await seen(app, "[data-composer-command-drawer]");
    },
    clip: composer(),
    tidy: tidyShort1,
  },
  {
    id: "W1-CMP-021",
    data: benchFiles,
    go: async app => {
      await typeIn(app, "short-1", "@");
      await seen(app, "[data-composer-command-drawer] [role=option]");
      await app.key("enter");
      await gone(app, "[data-composer-command-drawer]");
    },
    clip: composer(),
    tidy: tidyShort1,
    cleanup: noBenchFiles,
  },
  {
    id: "W1-CMP-022",
    go: async app => {
      await app.click("[data-composer-picker=model]");
      await seen(app, POPOVER);
    },
    clip: app => around(app, [POPOVER, "[data-composer-picker=model]"], 12),
  },
  {
    id: "W1-CMP-023",
    data: host => working(host),
    go: async (app, id) => {
      await open(app, id);
      await app.click(EDITOR);
      await app.type("a");
      await app.key("enter");
      await seen(app, "[data-drawer-row=queue]");
      await app.park();
    },
    clip: (app, id) => around(app, [row(id), ...BOX], 0),
    cleanup: drop,
  },
  {
    id: "W1-CMP-024",
    data: async host => {
      await host.pref("midTurn", "steer");
      return working(host);
    },
    go: async (app, id) => {
      await open(app, id);
      await app.click(EDITOR);
      await app.type("a");
      await app.key("enter");
      await app.sleep(800);
      await app.park();
    },
    clip: turnClip(...BOX),
    cleanup: async (host, id) => {
      await drop(host, id);
      await host.pref("midTurn", "queue");
    },
  },
  {
    id: "W1-CMP-025",
    data: host => working(host, "question"),
    go: async (app, id) => {
      await open(app, id);
      await seen(app, "[data-prompt-option]");
    },
    clip: (app, id) => around(app, [row(id), HEADER, ...BOX], 0),
    cleanup: drop,
  },
  {
    id: "W1-CMP-026",
    go: async app => {
      await typeIn(app, "short-1", "/");
      await app.key("escape");
      await app.hover(`${COMPOSER} button[type=submit]`);
      await seen(app, TOOLTIP);
      await freeze(app);
    },
    clip: composer(TOOLTIP),
    tidy: async app => {
      await thaw(app);
      await tidyShort1(app);
    },
  },
  {
    id: "W1-CMP-029",
    data: async host => {
      await benchFiles(host);
      await host.pref("textSize", 17);
    },
    cleanup: async host => {
      noBenchFiles(host);
      await host.pref("textSize", null);
    },
    go: async app => {
      await typeIn(app, "short-1", "@");
      await seen(app, "[data-composer-command-drawer] [role=option]");
      await app.key("enter");
      await gone(app, "[data-composer-command-drawer]");
    },
    clip: composer(),
    tidy: tidyShort1,
  },
  {
    id: "W1-CMP-030",
    data: host => working(host, [{ kind: "tools", names: ["TodoWrite"], n: 1 }, "hold"]),
    go: async (app, id) => {
      await open(app, id);
      await seen(app, "[data-drawer-row=tasks]");
    },
    clip: composer(),
    cleanup: drop,
  },
  {
    id: "W1-CMP-031",
    data: async host => {
      await host.project("second");
      await host.pref("newThreadIn", "ask");
    },
    go: async app => {
      await chord(app, "mod+t", "[data-slot=command-dialog-popup]");
      await app.park();
    },
    clip: app => around(app, ["[data-slot=command-dialog-popup]"], 24),
    cleanup: async host => {
      await host.removeProject("second");
      await host.pref("newThreadIn", "current");
    },
  },
  {
    id: "W1-CMP-032",
    data: async host => (await host.threads()).map(t => t.id),
    go: async app => {
      // The room stands for the few ms the host takes to answer the create; the create is held 12 s so it is shot.
      await app.page(() => {
        const send = WebSocket.prototype.send;
        WebSocket.prototype.send = function (data) {
          if (typeof data === "string" && data.includes('"op":"workspaces.create"')) {
            WebSocket.prototype.send = send;
            const release = () => (clearTimeout(late), (window.parityRelease = undefined), send.call(this, data));
            const late = setTimeout(release, 12_000);
            window.parityRelease = release;
            return;
          }
          return send.call(this, data);
        };
      });
      await app.click(EDITOR);
      await app.type("x");
      await app.key("enter");
      await seen(app, "[data-k=setting-up-room]");
      await app.park();
    },
    clip: app => around(app, ["[data-k=setting-up-room] section", ...BOX], 16),
    tidy: async app => {
      await app.page(() => window.parityRelease?.());
      await app.waitFor(() => document.querySelector("[data-k=setting-up-room]") === null, 15_000).catch(() => {});
    },
    cleanup: async (host, before) => {
      const known = new Set(before ?? []);
      const made = await until("the thread the held create opens", async () => (await host.threads()).filter(t => !known.has(t.id)).map(t => t.id).filter(Boolean), 30_000).catch(() => []);
      await dropAll(host, made);
    },
  },
  {
    id: "W1-CMP-093",
    go: async app => {
      await app.click(EDITOR);
      await app.type("https://github.com/x/y/pull/1");
      await app.sleep(800);
    },
    clip: composer(),
  },
  {
    id: "W1-CMP-095",
    go: async app => {
      await chord(app, "mod+k", "[data-slot=command-dialog-popup]");
      await app.type("Resume a conversation");
      await app.sleep(300);
      await app.key("enter");
      await seen(app, "[data-resume-picker] [data-resume-empty]", 15_000);
      await app.park();
    },
    clip: app => around(app, ["[data-resume-picker]"], 24),
  },
  {
    id: "W1-CMP-096",
    go: async app => {
      await typeIn(app, "short-1", "a");
      await app.key("mod+s");
      await app.type("b");
      await app.key("mod+s");
      await app.key("mod+s");
      await seen(app, "[data-composer-stash-drawer]");
    },
    clip: composer("[data-composer-stash-drawer]"),
    tidy: async app => {
      for (let i = 0; i < 4 && (await app.page(() => document.querySelector("[data-stash-delete]") !== null)); i++) {
        await app.click("[data-stash-delete]");
        await app.sleep(300);
      }
      await clearDraft(app, "short-1");
      // One entry left stands as the box's word, not a list; Mod+S on an empty box brings it back to be deleted.
      if (await app.page(() => document.querySelector("[data-composer-stash-word]") !== null)) {
        await app.click(EDITOR);
        await app.key("mod+s");
        await clearDraft(app, "short-1");
      }
    },
  },
  {
    id: "W1-CMP-097",
    data: host => working(host),
    go: async (app, id) => {
      await open(app, id);
      await app.click(EDITOR);
      await app.type("a");
      await app.key("enter");
      await app.click("1 message waiting");
      await app.park();
    },
    clip: composer(),
    cleanup: drop,
  },
  {
    id: "W1-CMP-099",
    go: app => typeIn(app, "short-1", "A draft line that keeps going past the edge of the one-line box, so the box has to grow to hold it, and it goes on with more plain words until it is two hundred chars".padEnd(200, ".").slice(0, 200)),
    clip: composer(),
    tidy: tidyShort1,
  },
  { id: "W1-CMP-104", width: 980, go: app => open(app, "long") },
  {
    id: "W1-CMP-105",
    data: host => working(host, [{ kind: "tools", names: ["TodoWrite"], n: 1 }, "hold"]),
    go: async (app, id) => {
      await open(app, id);
      await app.click("[data-drawer-row=tasks]");
      await seen(app, "[data-composer-bar=tasks]");
      await app.park();
    },
    clip: composer(),
    cleanup: drop,
  },
  {
    id: "W1-CMP-106",
    go: async app => {
      await typeIn(app, "short-1", "@zzzz");
      await app.sleep(800);
    },
    clip: composer(),
    tidy: tidyShort1,
  },
  {
    id: "W1-CMP-107",
    go: async app => {
      await app.click("[data-composer-picker=model]");
      await seen(app, POPOVER);
      await app.click("Favourites");
      await app.sleep(400);
    },
    clip: app => around(app, [POPOVER, "[data-composer-picker=model]"], 12),
  },

  // Narrow states last: the driver's reset reloads the page before it sets the window back to 1280x800.
  {
    id: "W1-THR-038",
    go: async app => {
      await open(app, "short-1");
      await app.width(600);
      await chord(app, "mod+f", "[data-thread-find]");
    },
  },
  { id: "W1-CMP-027", width: 600, go: () => {} },
];

/** The fixture's threads read once, so each tile reads the same in every state whatever an earlier one opened. */
export async function setup(host) {
  for (const id of Object.values(host.fixture.threads)) await host.op("sessions.read", { threadId: id });
}

/** The window the last shot was driven in, for a state's tidy after its capture. */
let shown;

export default states.map(state => ({
  ...state,
  go: async (app, data) => {
    shown = app;
    await fresh(app);
    await state.go(app, data);
  },
  // A state's tidy undoes in the window what a reload of the page would not (a held message, a draft, a stash).
  cleanup: async (host, data) => {
    if (state.tidy !== undefined && shown !== undefined) await state.tidy(shown).catch(() => {});
    await state.cleanup?.(host, data);
  },
}));
