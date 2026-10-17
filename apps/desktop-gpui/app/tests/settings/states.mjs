// SPDX-License-Identifier: AGPL-3.0-only
// The Settings states both apps are shot in, as data: the web shooter and the GPUI driver read this one table and
// send the same host ops around each state through host.mjs's sendOps.

/** The host preferences each theme's run sets, the same for both apps. */
export const THEMES = { paper: { theme: "light", lightTheme: "paper" }, graphite: { theme: "dark", darkTheme: "graphite" } };

const tab = segment => ({ click: `[data-k="agents-tabs"] [data-segment="${segment}"]` });
const view = segment => ({ click: `[data-k="activity-views"] [data-segment="${segment}"]` });
const search = word => ({ type: ['[data-k="settings-search"]', word] });

/** Every state: the page stored before Settings opens (`{name}` is a project of the fixture by its name), what it
 * takes from there, and host ops sent before the load and after the shot, which put back what they moved. A step
 * clicks, types into, presses or waits for a selector. `pages: false` keeps a dialog's state to the one window;
 * `compare: false` is shot but left out of the compare, since another ticket owns that dialog. */
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
];

/** A state's stored page with its `{name}` read as that project's id, off projects.list's answer by name. */
export const atOf = (at, projects) => at.replace(/\{(\w+)\}/, (_, name) => projects[name] ?? `missing-project-${name}`);
