// SPDX-License-Identifier: AGPL-3.0-only
// The Settings states both apps are shot in, as data: the web shooter and the GPUI driver read this one table and
// send the same host ops around each state through host.mjs's sendOps.

/** The zone both apps tell the time in, so an hour axis or a "Sun 18 Oct" reads the same in each whatever the computer's
 * own zone, or the zone a Chrome launcher guesses from the address it runs at. */
export const TIME_ZONE = "UTC";

/** The host preferences each theme's run sets, the same for both apps. */
export const THEMES = { paper: { theme: "light", lightTheme: "paper" }, graphite: { theme: "dark", darkTheme: "graphite" } };

const tab = segment => ({ click: `[data-k="agents-tabs"] [data-segment="${segment}"]` });
const view = segment => ({ click: `[data-k="activity-views"] [data-segment="${segment}"]` });
const search = word => ({ type: ['[data-k="settings-search"]', word] });

/** Every state: the page stored before Settings opens (`{name}` is a project of the fixture by its name), what it
 * takes from there, and host ops sent before the load and after the shot, which put back what they moved. A step
 * clicks, types into, hovers, scrolls to (`reveal`), presses or waits for a selector (Playwright's, so `:text-is()` and `>> nth=` work); a
 * last step that hovers keeps the pointer there for the shot, and a press marked `web: true` is the web's alone. An
 * `at` of "" keeps no page, as a first open of Settings, with `atShown` the page it lands on; `scroll: "foot"` shoots
 * the page scrolled to its foot. `skip` says why a state is shot by neither app. `pages: false` keeps a dialog's state to the one window;
 * `compare: false` is shot but left out of the compare, since that dialog is compared on its own. `width` is the
 * window's CSS width, 1280 where none is named. `gpui` steps run in the GPUI app alone before the web's: under 768 px
 * the web's sidebar is a sheet, shut, and the app's frame draws no sheet (D23), so the app's sidebar is put away for
 * the page to stand at the web's width. */
export const STATES = [
  { name: "general", at: "general", how: "General" },
  { name: "about", at: "general", how: "General, the sidebar's Version row", steps: [{ click: '[data-row-id="section:version"]' }], pages: false },
  { name: "appearance", at: "appearance", how: "Appearance; the theme the run sets is off the default System, so Restore defaults stands" },
  { name: "agents", at: "agents", how: "Agents, its Agents tab" },
  { name: "agents-servers", at: "agents", how: "Agents, Tool servers tab: the fixture's docs server and wsp's own", steps: [tab("servers")] },
  { name: "agents-server-docs", at: "agents", how: "Agents, Tool servers, the docs row", steps: [tab("servers"), { click: '[data-kind-item^="server-global-docs"] [role=link]' }] },
  { name: "agents-server-remove-dialog", at: "agents", how: "Agents, Tool servers, docs, Remove", steps: [tab("servers"), { click: '[data-kind-item^="server-global-docs"] [role=link]' }, { click: '[data-k="act-remove"]' }, { wait: '[data-k="confirm-remove"]' }], pages: false },
  { name: "agents-server-add", at: "agents", how: "Agents, Tool servers, Add a tool server", steps: [tab("servers"), { click: '[data-k="kind-add"]' }] },
  { name: "agents-servers-nothing", at: "agents", how: "Agents, Tool servers, a search that finds none", steps: [tab("servers"), { type: ['[data-k="kind-search"]', "zzz"] }] },
  { name: "agents-skills", at: "agents", how: "Agents, Skills tab: the wsp skill wsp mcp install wrote", steps: [tab("skills")] },
  { name: "agents-skill-wsp", at: "agents", how: "Agents, Skills, the first row", steps: [tab("skills"), { click: "[data-kind-item] [role=link]" }] },
  { name: "agents-skill-add", at: "agents", how: "Agents, Skills, Add a skill, nothing typed", steps: [tab("skills"), { click: '[data-k="kind-add"]' }] },
  { name: "agents-plugins", at: "agents", how: "Agents, Plugins tab, none yet", steps: [tab("plugins")] },
  { name: "agent-claude", at: "agent:claude", how: "Agents, the claude page, installed (the bench's stub)" },
  { name: "agent-claude-program", at: "agent:claude", how: "The claude page, How it runs, Program's Change", steps: [{ click: '[data-k="agent-program-change"]' }], pages: false },
  { name: "agent-codex", at: "agent:codex", how: "Agents, the codex page, not installed" },
  { name: "agent-opencode", at: "agent:opencode", how: "Agents, the opencode page, not installed" },
  { name: "agent-cursor", at: "agent:cursor", how: "Agents, the cursor page, not installed" },
  { name: "computers", at: "computers", how: "Computers: this computer, no clouds, Add a cloud provider" },
  { name: "computer-here", at: "computer:here", how: "Computers > this computer" },
  { name: "add-computer", at: "computers", how: "Computers, Add a computer, its Where step", steps: [{ click: '[data-k="add-computer-button"]' }, { wait: '[data-add-computer="where"]' }], pages: false, compare: false },
  { name: "add-cloud", at: "computers", how: "Computers, Add a cloud provider, its key step", steps: [{ click: '[data-k="add-cloud-button"]' }, { wait: '[data-add-cloud="key"]' }], pages: false, compare: false },
  { name: "recipes", at: "recipes", how: "Recipes: the fixture's Web app" },
  { name: "recipes-none", at: "recipes", how: "Recipes with none saved", before: [{ op: "recipes.remove", name: "web-app" }], after: [{ op: "recipes.save", name: "Web app", file: "fixture" }] },
  { name: "recipe-web-app", at: "recipes", how: "Recipes, the Web app row", steps: [{ click: '[data-recipe-row="web-app"]' }, { wait: '[data-k="recipe-head"]' }] },
  { name: "remove-recipe-dialog", at: "recipes", how: "Recipes, Web app, Delete", steps: [{ click: '[data-recipe-row="web-app"]' }, { click: '[data-k="delete"]' }, { wait: "[data-delete-recipe-dialog]" }], pages: false },
  { name: "projects", at: "projects", how: "Projects: bench (a workspace on it) and notes" },
  { name: "add-project-dialog", at: "projects", how: "Projects, Add a project", steps: [{ click: '[data-k="add-project-button"]' }, { wait: '[data-k="add-project"]' }], pages: false, compare: false },
  { name: "project-bench", at: "project:{bench}", how: "Projects > bench: its Remove held while a workspace uses it" },
  { name: "project-notes", at: "project:{notes}", how: "Projects > notes: no threads, an origin" },
  { name: "remove-project-dialog", at: "project:{notes}", how: "Projects > notes, Remove", steps: [{ click: '[data-k="remove-project"]' }, { wait: "[data-remove-project-dialog]" }], pages: false },
  { name: "activity", at: "activity", how: "Activity, Threads view" },
  { name: "activity-processes", at: "activity", how: "Activity, Processes view", steps: [view("processes")] },
  { name: "activity-kinds", at: "activity", how: "Activity, Kinds view", steps: [view("kinds")] },
  { name: "activity-ports", at: "activity", how: "Activity, Ports view", steps: [view("ports")] },
  { name: "keybindings", at: "keybindings", how: "Keybindings, all defaults" },
  { name: "keybindings-changed", at: "keybindings", how: "Keybindings with Toggle sidebar moved to mod+shift+b: Reset and Restore defaults", before: [{ op: "preferences.set", patch: { keybindings: { "sidebar.toggle": "mod+shift+b" } } }], after: [{ op: "preferences.set", patch: { keybindings: { "sidebar.toggle": null } } }] },
  { name: "keybindings-capturing", at: "keybindings", how: "Keybindings, Toggle sidebar's chord clicked: Press keys", steps: [{ click: '[data-chord="sidebar.toggle"]' }, { wait: '[data-k="capturing"]' }], pages: false },
  { name: "privacy", at: "privacy", how: "Privacy, all defaults" },
  { name: "privacy-changed", at: "privacy", how: "Privacy with Usage logs off: Restore defaults", before: [{ op: "preferences.set", patch: { usageLogs: false } }], after: [{ op: "preferences.set", patch: { usageLogs: true } }] },
  { name: "devices", at: "devices", how: "Devices: the fixture's Phone" },
  { name: "devices-none", at: "devices", how: "Devices with none paired", before: [{ op: "devices.revoke", device: "Phone" }], after: [{ pair: "Phone" }] },
  { name: "revoke-device-dialog", at: "devices", how: "Devices, Phone, Revoke", steps: [{ click: '[data-k="revoke"]' }, { wait: "[data-revoke-device-dialog]" }], pages: false },
  { name: "account", at: "account", how: "Account, signed out (no relay login on a throwaway host)" },
  { name: "search-theme", at: "appearance", how: "Search settings: theme", steps: [search("theme")], atShown: "search" },
  { name: "search-thread", at: "appearance", how: "Search settings: thread", steps: [search("thread")], atShown: "search" },
  { name: "search-nothing", at: "appearance", how: "Search settings: zzz, Nothing matches", steps: [search("zzz")], atShown: "search" },
  ...narrowStates(),
  ...specStates(),
];

/** The Settings and Usage states of parity/wave-1/spec.md and more.md (the W1-SET and W1-USE items with a
 * drive), each named by its id with the spec's words in `how`, driven as parity/wave-1 drives Electron.
 * A state the spec lists that these tools cannot reach carries `skip`, the reason, and is shot by neither app. */
function specStates() {
  const away = { gpui: [{ press: "ctrl+b" }] };
  const pref = (patch, back) => ({ before: [{ op: "preferences.set", patch }], after: [{ op: "preferences.set", patch: back }] });
  const textSize = pref({ textSize: 15 }, { textSize: null });
  const sendWith = pref({ sendWith: "mod-enter" }, { sendWith: "enter" });
  const claudeModels = { after: [{ op: "preferences.set", patch: { agentDefaults: { claude: null } } }] };
  // The turn parity's working() leaves running on a thread of its own, its use and limits taken back with it.
  const WORK = "Run the tests again and fix what fails.";
  const working = {
    before: [{ usage: "keep" }, { thread: WORK, title: WORK.slice(0, -1), reply: [{ kind: "text", text: "I will run the tests first." }, { kind: "hold" }] }],
    after: [{ dropThread: WORK }, { usage: "restore" }],
  };
  const group = `[data-activity-group]:has-text("${WORK.slice(0, -1)}")`;
  const titled = words => `:is([data-settings-title], [data-grid-name], [data-settings-label]):text-is("${words}")`;
  const chord = { click: '[data-chord="commandPalette.toggle"]' };
  const claudePage = [{ click: '[data-agent-row="claude"] [data-settings-title]' }, { wait: '[data-settings-at="agent:claude"]' }, { wait: '[data-settings-card="agent-models"]' }];
  const server = {
    before: [{ op: "servers.add", target: { placeId: "here" }, agent: "claude", name: "x", url: "https://example.com/mcp" }],
    after: [{ op: "servers.remove", target: { placeId: "here" }, agent: "claude", name: "x" }],
  };
  const serverX = { click: `[data-kind-item]:has(${titled("x")}) [role=link]` };
  const noRecipe = { before: [{ op: "recipes.remove", name: "web-app" }], after: [{ op: "recipes.save", name: "Web app", file: "fixture" }] };
  const project = name => ({ before: [{ project: name }], after: [{ dropProject: name }] });
  const LONG_PROJECT = "the nightly release pipeline that builds every package for each platform we ship";
  const LONG_RECIPE = "A recipe for the build machines that run the nightly release job";
  const twelve = Array.from({ length: 12 }, (_, i) => `r${i + 1}`);
  const thirty = Array.from({ length: 30 }, (_, i) => `p${String(i + 1).padStart(2, "0")}`);
  const usageTab = segment => ({ click: `[data-k="usage-tabs"] [data-segment="${segment}"]` });
  const range = segment => ({ click: `[data-k="usage-range"] [data-segment="${segment}"]` });
  const split = segment => ({ click: `[data-k="usage-split"] [data-segment="${segment}"]` });
  const limits = (prompt, fiveHour, sevenDay) => ({
    before: [{ usage: "keep" }, { thread: prompt, reply: [{ kind: "rate-limit", fiveHour, sevenDay }, { kind: "text", text: "The limits are read." }] }],
    after: [{ dropThread: prompt }, { usage: "restore" }],
    steps: [usageTab("limits"), { wait: '[data-k="left"]' }],
  });
  return [
    { name: "W1-SET-026", at: "general", how: 'Search results, unmatched groups at 50%; settings general, type "send"', steps: [search("send")], atShown: "search" },
    { name: "W1-SET-027", at: "general", how: 'Nothing matches; settings general, type "zzzz"', steps: [search("zzzz")], atShown: "search" },
    { name: "W1-SET-028", at: "appearance", how: "Off defaults; +pref textSize=15; settings appearance", ...textSize },
    { name: "W1-SET-029", at: "keybindings", how: "Chord refused; settings keybindings, click Search's keycaps, key mod+b", steps: [chord, { wait: '[data-k="capturing"]' }, { press: "Control+b" }, { wait: '[data-k="chord-refused"]' }], pages: false },
    // The web's sidebar is a sheet under 768 px, which mod+b opens; the app's frame keeps its sidebar there (D23).
    { name: "W1-SET-030", at: "general", how: 'Sidebar as a sheet; width 760; settings general, key mod+b, type "send"', width: 760, steps: [{ press: "Control+b", web: true }, { wait: '[data-k="settings-search"]' }, search("send")] },
    { name: "W1-SET-031", at: "appearance", how: "Slots under words; width 600, +pref textSize=15; settings appearance", width: 600, ...away, ...textSize },
    { name: "W1-SET-032", at: "general", how: "Desktop rows (When you quit, Start at login, Logs, App and Host lines); settings general (W1-FRM-022's shot)" },
    { name: "W1-SET-033", at: "agents", how: "Agents tab, every agent on PATH needing sign-in; settings agents" },
    { name: "W1-SET-034", at: "agents", how: "Token field open; settings agents, click Sign in on Claude Code", steps: [{ click: '[data-agent-row="claude"] [data-k="act-sign-in"]' }, { wait: '[data-k="sign-in-flow"]' }] },
    { name: "W1-SET-035", at: "agents", how: "Signed in by key; +op agents.key {agent:claude,key:x,way:key}; settings agents", before: [{ keepFile: "state/.env" }, { op: "agents.key", agent: "claude", key: "x", way: "key" }], after: [{ restoreFile: "state/.env" }] },
    { name: "W1-SET-036", at: "agents", how: "A server's page; +op servers.add {target:{placeId:here},agent:claude,name:x,url}; settings agents, click Tool servers, x", ...server, steps: [tab("servers"), serverX] },
    { name: "W1-SET-037", at: "agents", how: "Claude's page; settings agents, click Claude Code", steps: claudePage },
    { name: "W1-SET-038", at: "projects", how: "Project in use, Remove held; settings projects, click bench", steps: [{ click: titled("bench") }, { wait: '[data-k="remove-project"]' }] },
    { name: "W1-SET-039", at: "devices", how: "A paired device; +op pair.issue, pair.redeem {code,name} first on a new socket; settings devices", before: [{ op: "devices.revoke", device: "Phone" }, { pair: "Parity phone" }], after: [{ op: "devices.revoke", device: "Parity phone" }, { pair: "Phone" }] },
    { name: "W1-SET-040", at: "computers", how: "Computers: one row, Clouds head, Add a cloud provider; settings computers" },
    { name: "W1-SET-041", at: "computers", how: "This computer's page; +working long; settings computers, click this computer", ...working, steps: [{ click: '[data-place-row="here"]' }, { wait: '[data-settings-at="computer:here"]' }] },
    { name: "W1-SET-042", at: "recipes", how: "Recipes empty; settings recipes", ...noRecipe },
    {
      name: "W1-SET-043",
      at: "recipes",
      how: 'Twelve recipes, one open; +op recipes.save {"name":"rN","file":{"name":"rN"}} x12; settings recipes, click r1',
      before: [...noRecipe.before, ...twelve.map(name => ({ op: "recipes.save", name, file: { name } }))],
      after: [...twelve.map(name => ({ op: "recipes.remove", name })), ...noRecipe.after],
      steps: [{ click: titled("r1") }, { wait: '[data-k="recipe-head"]' }],
    },
    { name: "W1-SET-044", at: "activity", how: "Activity, a working thread open; +working long; settings activity, click long", ...working, steps: [{ click: `${group} [data-grid-row]` }, { wait: `${group} [data-proc-row]` }] },
    { name: "W1-SET-045", at: "activity", how: "Narrow: columns drop under 768 px, tabs become a select under 640; width 600; settings activity", width: 600, ...away },
    { name: "W1-SET-046", at: "general", how: "Sidebar hidden at full width: the header is the frame row, toggle at its left, no New thread glyph; settings general, key mod+b", steps: [{ press: "Control+b" }] },
    {
      name: "W1-SET-047",
      at: "keybindings",
      how: 'A chord of the person\'s own: Reset by its keycaps, Restore defaults in the header; +pref keybindings={"commandPalette.toggle":"mod+shift+k"}; settings keybindings',
      ...pref({ keybindings: { "commandPalette.toggle": "mod+shift+k" } }, { keybindings: { "commandPalette.toggle": null } }),
    },
    { name: "W1-SET-048", at: "general", how: "General off a default: that row's reset arrow, no Restore defaults; +pref sendWith=mod-enter; settings general", ...sendWith },
    { name: "W1-SET-229", at: "agents", how: "Slots under the words; width 600; settings agents", width: 600, ...away },
    { name: "W1-SET-237", at: "", atShown: "appearance", how: "First open lands on Appearance; settings, no page kept", pages: false },
    { name: "W1-SET-239", at: "general", how: "Trail lit as the page scrolls; settings general, scroll page to the foot", scroll: "foot" },
    { name: "W1-SET-240", at: "keybindings", how: "Keycaps listening; settings keybindings, click Search's keycaps", steps: [chord, { wait: '[data-k="capturing"]' }], pages: false },
    { name: "W1-SET-241", at: "appearance", how: "Theme previewed under the pointer; settings appearance, hover a theme card not picked", steps: [{ hover: "[data-theme-option]:not([data-checked])" }], pages: false },
    { name: "W1-SET-242", at: "general", how: "Narrowest window; width 480; settings general", width: 480, ...away },
    { name: "W1-SET-244", at: "agents", how: "Copy line sign-in; settings agents, click Sign in on OpenCode", steps: [{ click: '[data-agent-row="opencode"] [data-k="act-sign-in"]' }, { wait: '[data-k="sign-in-line"]' }] },
    {
      name: "W1-SET-245",
      at: "agents",
      how: "Tool servers empty; settings agents, click Tool servers (the fixture's docs and wsp servers taken out of claude's config for the shot)",
      before: [{ keepFile: ".claude.json" }, { wsp: ["servers", "remove", "docs", "--agent", "claude", "--yes"] }, { wsp: ["servers", "remove", "wsp", "--agent", "claude", "--yes"] }],
      after: [{ restoreFile: ".claude.json" }],
      steps: [tab("servers")],
    },
    { name: "W1-SET-246", at: "agents", how: "Add a tool server form; settings agents, click Tool servers, click Add a tool server", steps: [tab("servers"), { click: '[data-k="kind-add"]' }, { wait: '[data-k="add-server-go"]' }] },
    { name: "W1-SET-247", at: "agents", how: 'Add skill results; settings agents, click Skills, Add skill, type "pdf" (needs network)', steps: [tab("skills"), { click: '[data-k="kind-add"]' }, { type: ['[data-k="add-search"]', "pdf"] }, { wait: `${titled("pdf")}` }] },
    {
      name: "W1-SET-248",
      at: "agents",
      how: "Skill rows; +op skills.add {target:{placeId:here},skill:anthropics/skills/pdf,agents:[claude]} (needs network); settings agents, click Skills",
      before: [{ op: "skills.add", target: { placeId: "here" }, skill: "anthropics/skills/pdf", agents: ["claude"] }],
      after: [{ op: "skills.remove", target: { placeId: "here" }, name: "pdf" }],
      steps: [tab("skills")],
    },
    { name: "W1-SET-249", at: "agents", how: "Plugins empty; settings agents, click Plugins", steps: [tab("plugins")] },
    { name: "W1-SET-250", at: "agents", how: "Remove asks first; +op servers.add as for the server's page; settings agents, Tool servers, x, click Remove", ...server, steps: [tab("servers"), serverX, { click: '[data-k="act-remove"]' }, { wait: '[data-k="confirm-remove"]' }], pages: false },
    { name: "W1-SET-251", at: "agents", how: "Model select open; settings agents, click Claude Code, click Model", steps: [...claudePage, { click: '[data-k="agent-model"]' }, { wait: '[data-slot="select-popup"]' }], pages: false },
    {
      name: "W1-SET-252",
      at: "projects",
      how: "Many projects; +project p01 to p30; settings projects",
      before: thirty.map(name => ({ project: name })),
      after: thirty.map(name => ({ dropProject: name })),
    },
    { name: "W1-SET-253", at: "projects", how: "Long project name; +project <80-character name>; settings projects, click it", ...project(LONG_PROJECT), steps: [{ click: titled(LONG_PROJECT) }, { wait: '[data-settings-card="project"]' }] },
    { name: "W1-SET-254", at: "projects", how: "Removable project; +project other; settings projects, click other, click Remove", ...project("other"), steps: [{ click: titled("other") }, { click: '[data-k="remove-project"]' }, { wait: "[data-remove-project-dialog]" }], pages: false },
    { name: "W1-SET-256", at: "devices", how: "Nothing paired; settings devices", before: [{ op: "devices.revoke", device: "Phone" }], after: [{ pair: "Phone" }] },
    { name: "W1-SET-258", at: "account", how: "Account signed out; settings account" },
    {
      name: "W1-SET-260",
      at: "recipes",
      how: "A recipe named in 64 characters, list and page; +op recipes.save {name and file name the 64 characters}; settings recipes, click it",
      before: [{ op: "recipes.save", name: LONG_RECIPE, file: { name: LONG_RECIPE } }],
      after: [{ op: "recipes.remove", name: LONG_RECIPE }],
      steps: [{ click: titled(LONG_RECIPE) }, { wait: '[data-k="recipe-head"]' }],
    },
    { name: "W1-SET-265", at: "activity", how: 'Activity, nothing matches; settings activity, click "Filter by name, port or thread", type "zzzz"', steps: [{ type: ['[data-k="activity-filter"] input, input[data-k="activity-filter"]', "zzzz"] }] },
    { name: "W1-SET-266", at: "activity", how: "Activity Processes, Every process on; settings activity, click Processes, click Every process", steps: [view("processes"), { click: '[data-k="activity-every"]' }] },
    { name: "W1-SET-267", at: "activity", how: "Activity Kinds, then Ports; +working long; settings activity, click Kinds; click Ports", ...working, steps: [view("kinds"), view("ports")] },
    { name: "W1-SET-268", at: "activity", how: "Activity row hovered, its menu button shown; +working long; settings activity, click long, hover its agent row", ...working, steps: [{ click: `${group} [data-grid-row]` }, { wait: `${group} [data-proc-row]` }, { hover: `${group} [data-proc-row][data-kind="agent"]` }], pages: false },
    { name: "W1-SET-269", at: "computers", how: "Computers at 760 px: Cores and Memory gone; width 760; settings computers", width: 760, ...away },
    {
      name: "W1-SET-270",
      at: "computers",
      how: 'A limit off its default, reset arrow beside it; +op places.set {"placeId":"here","threads":2}; settings computers, click this computer',
      before: [{ op: "places.set", placeId: "here", threads: 2 }],
      after: [{ op: "places.set", placeId: "here", reset: ["threads"] }],
      steps: [{ click: '[data-place-row="here"]' }, { wait: '[data-k="row-reset"]' }],
    },
    { name: "W1-SET-273", at: "keybindings", how: "Send the message reads Ctrl Enter once Send with is on it; +pref sendWith=mod-enter; settings keybindings", ...sendWith },
    { name: "W1-SET-274", at: "agents", how: "A model off the picker; settings agents, click Claude Code, click a model's Show switch", ...claudeModels, steps: [...claudePage, { click: '[data-model] >> nth=1 >> [data-k="model-shown"]' }, { reveal: '[data-model]:has([data-k="model-shown"]:not([data-checked]))' }] },
    { name: "W1-SET-275", at: "agents", how: 'An added model id with its x; settings agents, click Claude Code, type "my-model" in Add a model by its id, key Enter', ...claudeModels, steps: [...claudePage, { type: ['[data-k="agent-models-id"]', "my-model"] }, { press: "Enter" }, { wait: '[data-model="my-model"]' }, { reveal: '[data-model="my-model"]' }] },
    {
      name: "W1-SET-277",
      at: "projects",
      how: 'Project with its own default agent: "Set here for this project." and its reset arrow; settings projects, click bench, click Default agent, click Claude Code',
      after: [{ op: "preferences.set", patch: { projectDefaults: { "{bench}": null } } }],
      steps: [{ click: titled("bench") }, { click: '[data-k="project-agent"]' }, { wait: '[data-slot="select-popup"]' }, { click: '[data-slot="select-popup"] [role=option]:has([data-harness-mark="claude"])' }],
    },
    { name: "W1-USE-016", at: "usage", how: "Week by agent: Claude Code, its model, 1.86k today; usage" },
    { name: "W1-USE-017", at: "usage", how: "Today, hourly; usage, click Today", steps: [range("day")] },
    {
      name: "W1-USE-018",
      at: "usage",
      how: 'Two lines; +project p2, +thread p2 "hi"; usage, click Project',
      before: [{ usage: "keep" }, { project: "p2" }, { thread: "hi", project: "p2", reply: { kind: "text", text: "Hi." } }],
      after: [{ dropThread: "hi" }, { dropProject: "p2" }, { usage: "restore" }],
      steps: [split("project")],
    },
    { name: "W1-USE-019", at: "usage", how: "Chart hover tooltip; usage, hover the chart's right end", steps: [{ hover: '[data-usage-chart] [data-k="point"] >> nth=-1' }], pages: false },
    { name: "W1-USE-020", at: "usage", how: "Narrow, long name cut; width 600; usage, click Account", width: 600, ...away, steps: [split("account")] },
    { name: "W1-USE-021", at: "usage", how: "Limits, no accounts; usage, click Limits", steps: [usageTab("limits")] },
    { name: "W1-USE-022", at: "usage", how: "One account, 5-hour and Week; +thread with +reply rate-limit (five_hour 62%, seven_day 30%); usage, click Limits", ...limits("Read the plan's limits now.", 62, 30) },
    { name: "W1-USE-023", at: "usage", how: "Near and over the limit: 92%, reached; +reply rate-limit five_hour 100%, seven_day 92%; usage, click Limits", ...limits("Read the plan's limits once more.", 100, 92) },
    { name: "W1-USE-084", at: "usage", how: "30 days, ticks every 5th day; usage, click 30 days", steps: [range("month")] },
    { name: "W1-USE-085", at: "usage", how: "Width 980: the page fills, nothing folds; width 980; usage", width: 980 },
  ];
}

/** The narrow widths' states: today's app under 640 px (`max-sm:`) and under 768 px (`max-md:`). The Usage ones want
 * tests/usage/seed.mjs run on the fixture's state folder first. */
function narrowStates() {
  const away = { gpui: [{ press: "ctrl+b" }] };
  const reading = { before: [{ op: "preferences.set", patch: { textSize: 15 } }], after: [{ op: "preferences.set", patch: { textSize: null } }] };
  return [
    { name: "general-600", at: "general", how: "General at 600 px: rows' slots under their words, the page's px-4 pt-6", width: 600, ...away },
    { name: "general-480", at: "general", how: "General at 480 px, the narrowest the spec drives", width: 480, ...away },
    { name: "general-760", at: "general", how: "General at 760 px: the web's sidebar a sheet, shut; the app's frame keeps its own", width: 760 },
    { name: "general-sidebar-away", at: "general", how: "General, the sidebar put away with its key: the header is the frame row", steps: [{ press: "Control+b" }] },
    { name: "appearance-600", at: "appearance", how: "Appearance at 600 px with Reading size 15: two theme columns, 8 px gaps, Restore defaults its glyph, selects 144 px", width: 600, ...away, ...reading },
    { name: "agents-600", at: "agents", how: "Agents at 600 px: rows' slots under their words", width: 600, ...away },
    { name: "activity-600", at: "activity", how: "Activity at 600 px: tiles two by two, the bar a grid of two, the views a select, the narrow rows", width: 600, ...away },
    { name: "activity-760", at: "activity", how: "Activity at 760 px: the narrow rows with their menus shown, a thread head's menu button", width: 760, ...away },
    { name: "activity-processes-760", at: "activity", how: "Activity, Processes view at 760 px: the name, memory and the menu", width: 760, ...away, steps: [view("processes")] },
    { name: "recipe-web-app-600", at: "recipes", how: "Recipes, the Web app at 600 px: each pick row's slot under its name, 72 px in", width: 600, ...away, steps: [{ click: '[data-recipe-row="web-app"]' }, { wait: '[data-k="recipe-head"]' }] },
    { name: "computers-760", at: "computers", how: "Computers at 760 px: Cores and Memory gone", width: 760, ...away },
    { name: "computers-600", at: "computers", how: "Computers at 600 px", width: 600, ...away },
    { name: "revoke-device-dialog-600", at: "devices", how: "Devices, Phone, Revoke at 600 px: the alert a sheet at the foot", width: 600, ...away, steps: [{ click: '[data-k="revoke"]' }, { wait: "[data-revoke-device-dialog]" }], pages: false },
    { name: "usage-600", at: "usage", how: "Usage at 600 px, by account: totals two by two, the table's tokens and estimate", width: 600, ...away, steps: [{ click: '[data-k="usage-split"] [data-segment="account"]' }] },
    { name: "usage-760", at: "usage", how: "Usage at 760 px, by account", width: 760, ...away, steps: [{ click: '[data-k="usage-split"] [data-segment="account"]' }] },
    { name: "usage-limits-600", at: "usage", how: "Usage, Limits at 600 px: a window's figure over its meter", width: 600, ...away, steps: [{ click: '[data-k="usage-tabs"] [data-segment="limits"]' }] },
  ];
}

/** A state's stored page with its `{name}` read as that project's id, off projects.list's answer by name. */
export const atOf = (at, projects) => at.replace(/\{(\w+)\}/, (_, name) => projects[name] ?? `missing-project-${name}`);
