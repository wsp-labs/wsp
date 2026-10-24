// SPDX-License-Identifier: AGPL-3.0-only
// The frame and layout, Settings and Usage states of wave-1 that have a baseline of their own.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const LONG_PROJECT = "the nightly release pipeline that builds every package for each platform we ship";
const SHOTS = process.argv.includes("--theme") ? 1 : 2;
let p2Shots = 0;
const LONG_RECIPE = "A recipe for the build machines that run the nightly release job";

const PAGE = "[data-settings-page]";
const PANEL = "[data-preview-panel-mode]";
const TABBAR = "[data-right-panel-tabbar]";
const TABS = "[data-right-panel-tab-list] [data-active-tab]";
const TOOLTIP = "[data-slot=tooltip-popup]";
const MENU = "[data-slot=menu-popup]";
const SELECT = "[data-slot=select-popup]";
const DIALOG = "[data-slot=alert-dialog-popup]";
const CENTRE_HEAD = "[data-shell-center] [data-header-row]";
const card = id => `${PAGE} section[data-settings-card='${id}']`;
const segment = (group, value) => `[data-k=${group}] [data-segment=${value}]`;

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
      return { x: box.l - pad, y: box.t - pad, width: box.r - box.l + 2 * pad, height: box.b - box.t + 2 * pad };
    },
    selectors,
    pad,
  );

const pageBody = app => around(app, [`${PAGE} > *`], 16);

const rowAndNeighbours = async (app, row, cardSelector) => {
  const r = await around(app, [row], 0);
  const c = await around(app, [cardSelector], 16);
  return { x: c.x, y: r.y - 72, width: c.width, height: r.height + 144 };
};

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

const rowTitled = (app, title) =>
  mark(app, title => [...document.querySelectorAll("[data-settings-title], [data-grid-name], [data-settings-label]")].find(el => el.textContent.trim() === title)?.closest("[data-settings-row], [data-grid-row], [role=link], button"), title);

const seen = (app, selector, ms = 10_000) => app.waitFor(selector => document.querySelector(selector) !== null, ms, selector);
const gone = (app, selector, ms = 10_000) => app.waitFor(selector => document.querySelector(selector) === null, ms, selector);
const reveal = (app, selector, block = "center") => app.page((selector, block) => document.querySelector(selector).scrollIntoView({ block }), selector, block);
// Scrolled again at the capture, since the page goes on laying out after go.
const cardAtTop = id => async app => {
  await reveal(app, card(id), "start");
  return around(app, [card(id)]);
};
const settings = (app, at) => app.route.settings(at);

// After the route, which walks the sidebar a narrow window folds away; a key pressed before the sheet lands goes wide.
const narrow = async (app, px) => {
  await app.width(px);
  if (px < 768) await gone(app, "[data-app-sidebar]");
  await app.sleep(300);
};

const makes = (make, undo) => ({ data: make, cleanup: async (host, made) => undo(host, made) });

const addProject = async (host, name) => {
  const folder = join(host.home, "parity-a", name.replace(/[^A-Za-z0-9._-]/g, "-").slice(0, 40));
  mkdirSync(folder, { recursive: true });
  return (await host.op("projects.add", { source: folder, name })).project.id;
};
async function dropProject(host, project) {
  const { workspaces } = await host.op("workspaces.list");
  for (const w of workspaces.filter(w => w.project?.id === project)) await host.op("workspaces.delete", { workspaceId: w.id }).catch(() => {});
  if (project !== undefined) await host.op("projects.remove", { projectId: project, force: true });
}
const runThread = async (host, project, prompt, reply) => {
  host.script(prompt, reply);
  const id = /\bthread ([0-9a-f-]{36})\b/.exec(await host.wsp("run", project, "--agent", "claude", prompt))?.[1];
  if (id === undefined) throw new Error(`wsp run ${project} printed no thread`);
  return id;
};
const dropThread = async (host, threadId) => {
  if (threadId !== undefined) await host.op("sessions.delete", { threadId });
};

// pair.redeem goes first on a socket of its own, as a new device sends it.
async function freshSocket(host) {
  const stateDir = dirname(host.fixture.state);
  const port = JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8")).port;
  const boot = await (await fetch(`http://127.0.0.1:${port}/`)).text();
  const ws = new WebSocket(`ws://127.0.0.1:${port}${/"wsPath":"([^"]+)"/.exec(boot)?.[1] ?? "/ws"}`);
  await new Promise((done, fail) => ((ws.onopen = done), (ws.onerror = () => fail(new Error("the fixture's host refused a new socket")))));
  let next = 0;
  const waiting = new Map();
  ws.onmessage = m => {
    const msg = JSON.parse(m.data);
    if (typeof msg.id === "number") waiting.get(msg.id)?.(msg), waiting.delete(msg.id);
  };
  const op = (name, params = {}) =>
    new Promise((done, fail) => {
      const id = next++;
      waiting.set(id, msg => (msg.ok === false ? fail(new Error(`${name}: ${msg.error}`)) : done(msg)));
      ws.send(JSON.stringify({ id, op: name, ...params }));
    });
  return { op, close: () => ws.close() };
}

// No host operation takes a key out of the vault, so its line goes from the fixture's own vault file.
async function dropVaultKey(host, name) {
  const vault = join(dirname(host.fixture.state), ".env");
  if (!existsSync(vault)) return;
  writeFileSync(vault, readFileSync(vault, "utf8").split("\n").filter(line => !line.startsWith(`${name}=`)).join("\n"), { mode: 0o600 });
}

const HERE = { placeId: "here" };
const SERVER = { target: HERE, agent: "claude", name: "x" };
const server = () =>
  makes(
    host => host.op("servers.add", { ...SERVER, url: "https://example.com/mcp" }),
    host => host.op("servers.remove", SERVER),
  );
// A thread of the state's own, so the fixture's threads end as fixture.sh left them for the states after these.
const WORKING = "Run the tests again and fix what fails";
const working = (reply = [{ kind: "text", text: "I will run the tests first." }], prompt = `${WORKING}.`) => ({
  data: host => host.thread(prompt, { title: WORKING, reply: [...reply, "hold"] }),
  cleanup: async (host, id) => {
    if (id !== undefined) await host.removeThread(id);
  },
});

// The driver's reset leaves these as an earlier state set them: a cut socket, a hidden sidebar, an open drawer or panel.
const fresh = app =>
  app.page(async () => {
    const pause = (ms = 250) => new Promise(done => setTimeout(done, ms));
    if (window.__parityRejoin !== undefined) {
      window.__parityRejoin();
      delete window.__parityRejoin;
      for (const until = Date.now() + 15_000; document.querySelector("[data-disconnected-banner]") !== null && Date.now() < until; ) await pause(100);
    }
    if (innerWidth >= 768 && document.querySelector("[data-sidebar-state=collapsed]") !== null) {
      document.querySelector("[data-sidebar=trigger]")?.click();
      await pause();
    }
    for (const label of ["Toggle terminal drawer", "Toggle right panel"]) {
      const on = document.querySelector(`[aria-label='${label}'][aria-pressed=true]`);
      if (on !== null) {
        on.click();
        await pause();
      }
    }
  });

const thread = async (app, name) => {
  await app.route.thread(name);
  await fresh(app);
};

async function launcher(app) {
  await app.key("mod+alt+b");
  await seen(app, PANEL);
  await app.page(() => {
    for (const close of document.querySelectorAll("[data-right-panel-tab-list] [aria-label^='Close ']")) close.click();
  });
  await seen(app, "[aria-label='Open a panel']");
}

async function tabs(app) {
  await thread(app, "short-1");
  await launcher(app);
  // The launcher takes its letters only from inside the panel, and the thread's composer can win the focus back.
  await app.page(() => document.querySelector("[aria-label='Open a panel']").focus());
  await app.key("m");
  await seen(app, TABS);
  await app.click("Add a panel");
  await seen(app, MENU);
  await app.key("a");
  await app.waitFor(sel => document.querySelectorAll(sel).length === 2, 10_000, TABS);
  // The menu hands focus back to its trigger, whose ring is the keyboard's and not this state's.
  await app.page(() => document.activeElement?.blur());
  await app.park();
}

const launcherClip = async app => {
  const panel = await around(app, [PANEL], 0);
  const tiles = await around(app, ["[data-surface-launch]"], 32);
  return { x: panel.x, y: panel.y, width: panel.width, height: tiles.y + tiles.height - panel.y };
};

// The report is read again so a key an earlier state took out of the vault is not drawn from a held read.
async function agentsTab(app, tab) {
  await settings(app, "agents");
  await app.page(() => document.querySelector("[data-k=agents-refresh]")?.click());
  await app.waitFor(() => document.querySelector("[data-k=agents-refresh]:not([disabled])") !== null, 15_000);
  await app.click(segment("agents-tabs", tab));
  await seen(app, `${segment("agents-tabs", tab)}[data-checked]`);
}

async function claudePage(app) {
  await agentsTab(app, "agents");
  await app.click("[data-agent-row=claude] [data-settings-title]");
  await seen(app, `${PAGE}[data-settings-at='agent:claude']`);
  await seen(app, card("agent-models"));
}

// The window keeps the tab last picked.
const usage = async (app, tab = "used") => {
  await app.route.usage();
  await seen(app, segment("usage-tabs", tab));
  await app.click(segment("usage-tabs", tab));
  await seen(app, `${segment("usage-tabs", tab)}[data-checked]`, 15_000);
  if (tab === "used") await seen(app, card("usage-chart"));
};

const limitsRead = async (app, left) => {
  await usage(app, "limits");
  await app.waitFor(left => [...document.querySelectorAll("[data-k=left]")].some(el => el.textContent.replace(/\s/g, "") === left), 20_000, left);
};

// The window keeps the view last picked.
const activity = async (app, view = "threads") => {
  await settings(app, "activity");
  await app.click(segment("activity-views", view));
  await seen(app, `${segment("activity-views", view)}[data-checked]`);
  await gone(app, "[data-k=activity-loading], [data-k=activity-totals-loading]", 30_000);
};

const openRow = async (app, id) => {
  const group = `[data-activity-group='${id}']`;
  await seen(app, group, 15_000);
  await app.click(`${group} [data-grid-row]`);
  await seen(app, `${group} [data-proc-row]`);
};

const searchSettings = async (app, words) => {
  await app.click("[data-k=settings-search]");
  await app.type(words);
};

const states = [
  { id: "W1-FRM-017", go: () => {} },
  { id: "W1-FRM-018", go: launcher, clip: launcherClip },
  { id: "W1-FRM-019", go: tabs, clip: app => around(app, [TABBAR], 0) },
  {
    id: "W1-FRM-020",
    go: async app => {
      await tabs(app);
      await app.click("Add a panel");
      await seen(app, MENU);
    },
    clip: app => around(app, [TABBAR, MENU], 8),
  },
  {
    id: "W1-FRM-021",
    go: async app => {
      // New thread puts the caret in its composer, which keeps mod+b for itself; a person clicks off it first.
      await app.page(() => document.activeElement?.blur());
      await app.key("mod+b");
      await seen(app, "[data-sidebar-state=collapsed]");
    },
  },
  { id: "W1-FRM-022", go: app => settings(app, "general") },
  {
    id: "W1-FRM-023",
    width: 980,
    go: async app => {
      await thread(app, "short-1");
      await launcher(app);
    },
  },
  {
    id: "W1-FRM-024",
    width: 760,
    go: async app => {
      await app.click("Toggle main sidebar");
      await seen(app, "[data-sidebar=sidebar][data-mobile]");
    },
  },
  { id: "W1-FRM-025", width: 360, go: launcher },
  {
    id: "W1-FRM-026",
    width: 1100,
    data: host => host.pref("sidebarWidth", 480),
    go: async app => {
      await thread(app, "short-1");
      await launcher(app);
    },
  },
  {
    id: "W1-FRM-027",
    go: async app => {
      // The page's own socket is cut, and every redial, the host left serving.
      await app.page(() => {
        const proto = WebSocket.prototype;
        const send = proto.send;
        const opened = Object.getOwnPropertyDescriptor(proto, "onopen");
        proto.send = function () {
          this.close();
        };
        Object.defineProperty(proto, "onopen", {
          configurable: true,
          get: opened.get,
          set() {
            opened.set.call(this, function () {
              this.close();
            });
          },
        });
        window.__parityRejoin = () => {
          proto.send = send;
          Object.defineProperty(proto, "onopen", opened);
        };
        // The next shot's theme reaches the page over this socket, so the cut ends soon after the capture.
        setTimeout(() => window.__parityRejoin?.(), 3_000);
      });
      // Picking a thread sends the host a message, which is what closes the socket.
      await app.page(id => document.querySelector(`[data-row-id='thread:${id}']`).click(), app.fixture.threads["short-1"]);
      await seen(app, "[data-disconnected-banner=reconnecting]");
      await app.route.home();
    },
    clip: app => around(app, ["[data-disconnected-banner]", CENTRE_HEAD], 0),
  },
  {
    id: "W1-FRM-028",
    go: async app => {
      await thread(app, "short-1");
      await thread(app, "short-2");
      // Control stays down until the next reset lets it go, so the overlay stands for the capture.
      await app.keyDown("ctrl");
      await app.key("tab");
      await seen(app, "[data-workspace-switcher]");
      await app.sleep(400);
    },
    clip: app => around(app, ["[data-workspace-switcher]"], 24),
  },
  {
    id: "W1-FRM-029",
    ...working([{ kind: "tools", names: ["Task"], n: 1 }], "Trace the scroll writes while the tests run."),
    go: async (app, id) => {
      await thread(app, id);
      // The list is still scrolling to its end as the thread opens, so a press can land beside the row; it is pressed again.
      for (let tries = 0; tries < 3 && !(await app.page(() => document.querySelector("[data-subagent-page]") !== null)); tries++) {
        await app.click(await mark(app, () => [...document.querySelectorAll("[data-subagent-row] a[data-subagent-open]")].at(-1)));
        await app.sleep(1500);
      }
      await seen(app, "[data-subagent-page]");
    },
  },
  {
    id: "W1-FRM-030",
    go: async app => {
      await thread(app, "short-1");
      await app.key("mod+j");
      await seen(app, "[aria-label='Toggle terminal drawer'][aria-pressed=true]", 15_000);
      await launcher(app);
    },
  },
  {
    id: "W1-FRM-110",
    pointer: true,
    go: async app => {
      await tabs(app);
      await app.hover("[data-right-panel-tab-list] [data-active-tab=false]");
      await seen(app, TOOLTIP);
    },
    clip: app => around(app, [TABBAR, TOOLTIP], 8),
  },
  {
    id: "W1-FRM-111",
    pointer: true,
    go: async app => {
      await app.hover("[data-sidebar=rail]");
      await seen(app, TOOLTIP);
    },
    clip: async app => {
      const tip = await around(app, [TOOLTIP], 48);
      return { x: 200, y: tip.y, width: tip.x + tip.width - 200, height: tip.height };
    },
  },
  {
    id: "W1-FRM-112",
    go: async app => {
      await launcher(app);
      await app.key("arrowdown");
    },
    clip: launcherClip,
  },
  {
    id: "W1-FRM-113",
    go: async app => {
      await tabs(app);
      // Three browsers as the drive says, then more until the strip overflows: the fade is what this state shows.
      const overflows = () =>
        app.page(() => {
          const list = document.querySelector("[data-right-panel-tab-list]");
          const strip = list.querySelector("[data-slot=scroll-area-viewport]") ?? list;
          return strip.scrollWidth > strip.clientWidth + 1;
        });
      for (let i = 0; i < 3 || (i < 8 && !(await overflows())); i++) {
        await app.click("Add a panel");
        await seen(app, MENU);
        await app.key("b");
        await gone(app, MENU);
        await app.waitFor((sel, n) => document.querySelectorAll(sel).length === n, 10_000, TABS, 3 + i);
      }
      await app.page(() => document.activeElement?.blur());
    },
    clip: app => around(app, [TABBAR], 0),
  },

  {
    id: "W1-SET-026",
    go: async app => {
      await settings(app, "general");
      await searchSettings(app, "send");
      await seen(app, `${PAGE}[data-settings-at=search]`);
    },
  },
  {
    id: "W1-SET-027",
    go: async app => {
      await settings(app, "general");
      await searchSettings(app, "zzzz");
      await seen(app, "[data-k=nothing-matches]");
    },
  },
  {
    id: "W1-SET-028",
    data: host => host.pref("textSize", 15),
    go: async app => {
      await settings(app, "appearance");
      await reveal(app, card("fonts"), "start");
    },
    clip: cardAtTop("fonts"),
  },
  {
    id: "W1-SET-029",
    go: async app => {
      await settings(app, "keybindings");
      await app.click("[data-chord='commandPalette.toggle']");
      await seen(app, "[data-k=capturing]");
      await app.key("mod+b");
      await seen(app, "[data-k=chord-refused]");
    },
    clip: app => around(app, [`${card("shell")} h2`, "[data-settings-line='commandPalette.toggle']"]),
  },
  {
    id: "W1-SET-030",
    go: async app => {
      await settings(app, "general");
      await narrow(app, 760);
      await app.key("mod+b");
      await seen(app, "[data-k=settings-search]");
      await searchSettings(app, "send");
    },
  },
  {
    id: "W1-SET-031",
    data: host => host.pref("textSize", 15),
    go: async app => {
      await settings(app, "appearance");
      await narrow(app, 600);
      await reveal(app, "[data-settings-row=text-size]");
    },
  },
  { id: "W1-SET-033", go: app => agentsTab(app, "agents"), clip: app => around(app, [card("agents-on")]) },
  {
    id: "W1-SET-034",
    go: async app => {
      await agentsTab(app, "agents");
      await app.click("[data-agent-row=claude] [data-k=act-sign-in]");
      await seen(app, "[data-k=sign-in-flow]");
    },
    clip: app => around(app, ["[data-agent-row=claude]", "[data-k=sign-in-flow]"]),
  },
  {
    id: "W1-SET-035",
    data: host => host.op("agents.key", { agent: "claude", key: "x", way: "key" }),
    go: app => agentsTab(app, "agents"),
    clip: app => around(app, ["[data-agent-row=claude]"]),
    cleanup: host => dropVaultKey(host, "ANTHROPIC_API_KEY"),
  },
  {
    id: "W1-SET-036",
    ...server(),
    go: async app => {
      await agentsTab(app, "servers");
      await app.click(await rowTitled(app, "x"));
    },
    clip: pageBody,
  },
  { id: "W1-SET-037", go: claudePage, clip: pageBody },
  {
    id: "W1-SET-038",
    go: async app => {
      await settings(app, "projects");
      await app.click(await rowTitled(app, "bench"));
      await seen(app, "[data-k=remove-project]");
    },
    clip: app => around(app, [card("acts")]),
  },
  {
    id: "W1-SET-039",
    ...makes(
      async host => {
        const { code } = await host.op("pair.issue");
        const socket = await freshSocket(host);
        try {
          await socket.op("pair.redeem", { code, name: "Parity phone" });
        } finally {
          socket.close();
        }
      },
      async host => {
        for (const d of (await host.op("devices.list")).devices) await host.op("devices.revoke", { deviceId: d.id });
      },
    ),
    go: async app => {
      await settings(app, "devices");
      await seen(app, "[data-k=revoke]");
    },
    clip: app => around(app, [card("devices")]),
  },
  { id: "W1-SET-040", go: app => settings(app, "computers"), clip: pageBody },
  {
    id: "W1-SET-041",
    ...working(),
    go: async app => {
      await settings(app, "computers");
      await app.click("[data-place-row=here]");
      await seen(app, `${PAGE}[data-settings-at='computer:here']`);
    },
    clip: pageBody,
  },
  { id: "W1-SET-042", go: app => settings(app, "recipes"), clip: app => around(app, [card("recipes")]) },
  {
    id: "W1-SET-043",
    ...makes(
      async host => {
        for (let n = 1; n <= 12; n++) await host.op("recipes.save", { name: `r${n}`, file: { name: `r${n}` } });
      },
      async host => {
        for (let n = 1; n <= 12; n++) await host.op("recipes.remove", { name: `r${n}` }).catch(() => {});
      },
    ),
    go: async app => {
      await settings(app, "recipes");
      await app.click(await rowTitled(app, "r1"));
    },
    clip: pageBody,
  },
  {
    id: "W1-SET-044",
    ...working(),
    go: async (app, id) => {
      await activity(app, "threads");
      await openRow(app, id);
    },
    clip: pageBody,
  },
  {
    id: "W1-SET-045",
    go: async app => {
      await activity(app, "threads");
      await narrow(app, 600);
    },
  },
  {
    id: "W1-SET-046",
    go: async app => {
      await settings(app, "general");
      await app.key("mod+b");
      await seen(app, "[data-sidebar-state=collapsed]");
    },
  },
  {
    id: "W1-SET-047",
    data: host => host.pref("keybindings", { "commandPalette.toggle": "mod+shift+k" }),
    go: async app => {
      await settings(app, "keybindings");
      await seen(app, "[data-k=restore-defaults]");
    },
    clip: app => around(app, [CENTRE_HEAD, `${card("shell")} h2`, "[data-settings-line='commandPalette.toggle']"], 0),
  },
  {
    id: "W1-SET-048",
    data: host => host.pref("sendWith", "mod-enter"),
    go: async app => {
      await settings(app, "general");
      await seen(app, "[data-settings-row=send-with] [data-k=row-reset]");
    },
    clip: app => around(app, [CENTRE_HEAD, card("composer")], 0),
  },
  {
    id: "W1-SET-229",
    go: async app => {
      await agentsTab(app, "agents");
      await narrow(app, 600);
    },
  },
  {
    id: "W1-SET-237",
    go: async app => {
      // The page Settings opens on is read from storage once, as the page loads; the reset puts back the first visit's.
      if (await app.page(() => localStorage.getItem("wsp:settings-at") !== null)) throw new Error("this fixture's window kept a Settings page in its storage before the run began, so no first open of Settings is left to show; make a fresh fixture");
      await app.page(() => document.querySelector("[data-k=settings-row]").click());
      await seen(app, `${PAGE}[data-settings-at=appearance]`);
    },
    clip: app => around(app, [CENTRE_HEAD, "[data-row-id='group:appearance']", "[data-k=settings-trail]", card("mode")], 0),
  },
  {
    id: "W1-SET-239",
    go: async app => {
      await settings(app, "general");
      await app.scroll(PAGE, 4000);
      await app.page(() => {
        const viewport = document.querySelector("[data-settings-page]").parentElement;
        viewport.scrollTop = viewport.scrollHeight;
      });
      await app.sleep(600);
    },
    clip: app => around(app, ["[data-row-id='group:general']", "[data-k=settings-trail]"], 8),
  },
  {
    id: "W1-SET-240",
    go: async app => {
      await settings(app, "keybindings");
      await app.click("[data-chord='commandPalette.toggle']");
      await seen(app, "[data-k=capturing]");
    },
    clip: app => around(app, [`${card("shell")} h2`, "[data-settings-line='commandPalette.toggle']"]),
  },
  {
    id: "W1-SET-241",
    pointer: true,
    go: async app => {
      await settings(app, "appearance");
      const option = await mark(app, () => [...document.querySelectorAll("[data-theme-option]")].find(el => !el.hasAttribute("data-checked")));
      await app.hover(option);
      await app.waitFor(option => document.documentElement.dataset.theme === document.querySelector(option).dataset.themeOption, 5_000, option);
    },
    clip: app => around(app, [card("theme")]),
  },
  {
    id: "W1-SET-242",
    go: async app => {
      await settings(app, "general");
      await narrow(app, 480);
    },
  },
  {
    id: "W1-SET-244",
    go: async app => {
      await agentsTab(app, "agents");
      await app.click("[data-agent-row=opencode] [data-k=act-sign-in]");
      await seen(app, "[data-k=sign-in-line]");
    },
    clip: app => around(app, ["[data-agent-row=opencode]", "[data-k=sign-in-flow]"]),
  },
  { id: "W1-SET-245", go: app => agentsTab(app, "servers"), clip: pageBody },
  {
    id: "W1-SET-246",
    go: async app => {
      await agentsTab(app, "servers");
      await app.click("[data-k=kind-add]");
      await seen(app, "[data-k=add-server-go]");
    },
    clip: pageBody,
  },
  {
    id: "W1-SET-247",
    go: async app => {
      await agentsTab(app, "skills");
      await app.click("[data-k=kind-add]");
      await app.click("[data-k=add-search]");
      await app.type("pdf");
      await app.waitFor(() => [...document.querySelectorAll("[data-settings-title]")].some(el => el.textContent.trim() === "pdf"), 30_000);
    },
    clip: pageBody,
  },
  {
    id: "W1-SET-248",
    ...makes(
      host => host.op("skills.add", { target: HERE, skill: "anthropics/skills/pdf", agents: ["claude"] }),
      host => host.op("skills.remove", { target: HERE, name: "pdf" }),
    ),
    go: app => agentsTab(app, "skills"),
    clip: pageBody,
  },
  { id: "W1-SET-249", go: app => agentsTab(app, "plugins"), clip: pageBody },
  {
    id: "W1-SET-250",
    ...server(),
    go: async app => {
      await agentsTab(app, "servers");
      await app.click(await rowTitled(app, "x"));
      await app.click("Remove");
      await seen(app, DIALOG);
    },
    clip: app => around(app, [DIALOG], 24),
  },
  {
    id: "W1-SET-251",
    go: async app => {
      await claudePage(app);
      await app.click("[data-k=agent-model]");
      await seen(app, SELECT);
    },
    clip: app => around(app, ["[data-k=agent-model]", SELECT], 16),
  },
  {
    id: "W1-SET-252",
    ...makes(
      async host => {
        const ids = [];
        for (let n = 1; n <= 30; n++) ids.push(await addProject(host, `p${String(n).padStart(2, "0")}`));
        return ids;
      },
      async (host, ids) => {
        for (const id of ids ?? []) await dropProject(host, id).catch(() => {});
      },
    ),
    go: app => settings(app, "projects"),
    clip: pageBody,
  },
  {
    id: "W1-SET-253",
    ...makes(
      host => addProject(host, LONG_PROJECT),
      (host, id) => dropProject(host, id),
    ),
    go: async app => {
      await settings(app, "projects");
      await app.click(await rowTitled(app, LONG_PROJECT));
    },
    clip: app => around(app, [card("project")]),
  },
  {
    id: "W1-SET-254",
    ...makes(
      host => addProject(host, "other"),
      (host, id) => dropProject(host, id),
    ),
    go: async app => {
      await settings(app, "projects");
      await app.click(await rowTitled(app, "other"));
      await app.click("[data-k=remove-project]");
      await seen(app, DIALOG);
    },
    clip: app => around(app, [DIALOG], 24),
  },
  { id: "W1-SET-256", go: app => settings(app, "devices"), clip: app => around(app, [card("devices")]) },
  { id: "W1-SET-258", go: app => settings(app, "account"), clip: app => around(app, [card("account")]) },
  {
    id: "W1-SET-260",
    ...makes(
      host => host.op("recipes.save", { name: LONG_RECIPE, file: { name: LONG_RECIPE } }),
      host => host.op("recipes.remove", { name: LONG_RECIPE }),
    ),
    go: async app => {
      await settings(app, "recipes");
      await app.click(await rowTitled(app, LONG_RECIPE));
    },
    clip: app => around(app, [card("recipe")]),
  },
  {
    id: "W1-SET-265",
    go: async app => {
      await activity(app, "threads");
      await app.click("[data-k=activity-filter]");
      await app.type("zzzz");
    },
    clip: pageBody,
  },
  {
    id: "W1-SET-266",
    go: async app => {
      await activity(app, "processes");
      await app.click("[data-k=activity-every]");
    },
    clip: pageBody,
  },
  {
    id: "W1-SET-267",
    ...working(),
    go: async app => {
      await activity(app, "kinds");
      await activity(app, "ports");
    },
    clip: pageBody,
  },
  {
    id: "W1-SET-268",
    pointer: true,
    ...working(),
    go: async (app, id) => {
      await activity(app, "threads");
      await openRow(app, id);
      await app.hover(`[data-activity-group='${id}'] [data-proc-row][data-kind=agent]`);
    },
    clip: pageBody,
  },
  {
    id: "W1-SET-269",
    go: async app => {
      await settings(app, "computers");
      await narrow(app, 760);
    },
  },
  {
    id: "W1-SET-270",
    data: host => host.op("places.set", { placeId: "here", threads: 2 }),
    go: async app => {
      await settings(app, "computers");
      await app.click("[data-place-row=here]");
      await seen(app, "[data-k=row-reset]");
    },
    clip: app => around(app, [card("computer-limits")]),
    cleanup: host => host.op("places.set", { placeId: "here", reset: ["threads"] }),
  },
  {
    id: "W1-SET-273",
    data: host => host.pref("sendWith", "mod-enter"),
    go: async app => {
      await settings(app, "keybindings");
      await reveal(app, card("fixed"), "start");
    },
    clip: cardAtTop("fixed"),
  },
  {
    id: "W1-SET-274",
    go: async app => {
      await claudePage(app);
      const model = await app.page(() => document.querySelectorAll("[data-model]")[1].dataset.model);
      await app.click(`[data-model='${model}'] [data-k=model-shown]`);
      await seen(app, `[data-model='${model}'] [data-k=model-shown]:not([data-checked])`);
      await reveal(app, `[data-model='${model}']`);
    },
    clip: app => rowAndNeighbours(app, "[data-model]:has([data-k=model-shown]:not([data-checked]))", card("agent-models")),
  },
  {
    id: "W1-SET-275",
    go: async app => {
      await claudePage(app);
      await app.click("[data-k=agent-models-id]");
      await app.type("my-model");
      await app.key("enter");
      await seen(app, "[data-model=my-model]");
      await reveal(app, "[data-model=my-model]");
    },
    clip: app => rowAndNeighbours(app, "[data-model=my-model]", card("agent-models")),
  },
  {
    id: "W1-SET-277",
    go: async app => {
      await settings(app, "projects");
      await app.click(await rowTitled(app, "bench"));
      await app.click("[data-k=project-agent]");
      await seen(app, SELECT);
      await app.click(`${SELECT} [role=option]:has([data-harness-mark=claude])`);
      await gone(app, `${SELECT}[data-open]`);
    },
    clip: app => around(app, [card("project-new-threads")]),
  },

  {
    id: "W1-USE-016",
    go: async app => {
      await usage(app);
      await reveal(app, card("usage-used"), "start");
    },
    clip: cardAtTop("usage-used"),
  },
  {
    id: "W1-USE-017",
    go: async app => {
      await usage(app);
      await app.click(segment("usage-range", "day"));
      await seen(app, `${segment("usage-range", "day")}[data-checked]`);
    },
    clip: app => around(app, [card("usage-chart")]),
  },
  {
    id: "W1-USE-018",
    data: async host => {
      await host.project("p2");
      return host.thread("hi", { project: "p2", reply: { kind: "text", text: "Hi." } });
    },
    // Only the last shot takes p2 away: gone before the next shot, the chart would name that shot's line by its id.
    cleanup: async (host, id) => {
      if (++p2Shots < SHOTS) return;
      p2Shots = 0;
      await host.removeThread(id);
      await host.removeProject("p2");
    },
    go: async app => {
      await usage(app);
      await app.click(segment("usage-split", "project"));
      await app.waitFor(() => document.querySelectorAll("[data-usage-chart] [data-line]").length >= 2, 10_000);
    },
    clip: app => around(app, [card("usage-chart")]),
  },
  {
    id: "W1-USE-019",
    pointer: true,
    go: async app => {
      await usage(app);
      await app.hover(await mark(app, () => [...document.querySelectorAll("[data-usage-chart] [data-k=point]")].at(-1)));
      await seen(app, TOOLTIP);
    },
    clip: app => around(app, [card("usage-chart"), TOOLTIP]),
  },
  {
    id: "W1-USE-020",
    go: async app => {
      await usage(app);
      await app.click(segment("usage-split", "account"));
      await narrow(app, 600);
    },
  },
  { id: "W1-USE-021", go: app => usage(app, "limits"), clip: pageBody },
  {
    id: "W1-USE-022",
    ...makes(
      host => runThread(host, "bench", "Read the plan's limits now.", [{ kind: "rate-limit", fiveHour: 62, sevenDay: 30 }, { kind: "text", text: "The limits are read." }]),
      dropThread,
    ),
    go: app => limitsRead(app, "38%"),
    clip: pageBody,
  },
  {
    id: "W1-USE-023",
    ...makes(
      host => runThread(host, "bench", "Read the plan's limits once more.", [{ kind: "rate-limit", fiveHour: 100, sevenDay: 92 }, { kind: "text", text: "The limits are read again." }]),
      dropThread,
    ),
    go: app => limitsRead(app, "0%"),
    clip: pageBody,
  },
  {
    id: "W1-USE-084",
    go: async app => {
      await usage(app);
      await app.click(segment("usage-range", "month"));
      await seen(app, `${segment("usage-range", "month")}[data-checked]`);
    },
    clip: app => around(app, [card("usage-chart")]),
  },
  {
    id: "W1-USE-085",
    go: async app => {
      await usage(app);
      await app.width(980);
    },
  },
];

export default states.map(({ pointer, ...state }) => ({
  ...state,
  go: async (app, data) => {
    await fresh(app);
    await state.go(app, data);
    // A click leaves the pointer resting on what it pressed, whose hover is not the state's.
    if (pointer !== true) await app.park();
  },
}));
