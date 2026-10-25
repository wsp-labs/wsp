// SPDX-License-Identifier: AGPL-3.0-only
//! Unit mcp_files (spec/golden-mcp_files.md): golden-mcp.ts:1-335 at a4033dc34. Server row ids, the plan off a
//! recipe, the one read of every config off a machine and its parse, the string rewrites, the edit of every scope and
//! the landing of what changed.
use indexmap::{IndexMap, IndexSet};
use wsp_catalog::config_write::{config_landing, config_refusal, config_sum, config_write_line, under_base, ConfigWrite};
use wsp_catalog::mac_paths::{MAC_BIN_DIRS, MAC_BREW};
use wsp_catalog::mcp::{EditOutcome, McpEditScope, McpFormat};
use wsp_catalog::mcp_travelled::FromTo;
use wsp_catalog::road_modules::BREW_PREFIX;
use wsp_catalog::roads::GUEST_HOME;
use wsp_wire::types::agents_report::{CONFIG_NOT_PLAIN, CONFIG_PAST_CAP, CONFIG_READ_MAX_BYTES};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::MCP_ID_PREFIX;
use wsp_words::js::{js_trim, to_number};

use super::import_roads::{withheld, WITHHELD_NOTE};
use super::tools_disk::{inline_s, reason_of};
use super::{
    AsideRow, DropName, EditedName, EditedOutcome, McpAgentPlan, McpPlan, McpScope, McpToolRow, ProjectMove, RecipeEntry, ScopeFile,
    ScopeOutcome, ScopeRead,
};
use crate::exec_detached::INLINE_EXEC_MS;
use crate::land_bytes::{land_bytes, LandBytesOptions};
use crate::machine::{ExecOptions, ExecResult, Machine};

const MCP_REMOTE_ID: &str = "agents/mcp/mcp-remote";

/// golden-mcp.ts:21 `McpAgentSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpAgentSource {
    pub label: String,
    pub format: McpFormat,
    pub files: Vec<String>,
}

/// golden-mcp.ts:74 `McpPlanOptions`. `agents` keeps its order: the plan walks it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpPlanOptions {
    pub home: String,
    /// `None` is `GUEST_HOME`.
    pub guest_home: Option<String>,
    pub agents: IndexMap<String, McpAgentSource>,
    /// `None` is `MAC_BIN_DIRS`.
    pub bin_dirs: Option<Vec<String>>,
}

/// golden-mcp.ts:99 `at`: the user scope (`false`), the person's home folder (`true`), or a project folder by path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpAt {
    User,
    Home,
    Folder(String),
}

// TS golden-mcp.ts:96-100
// /** One server's row id: the agent it belongs to, where it sits (the user scope, the person's home folder with `true`,
//  * or a project folder on the machine by its path), and its name. The one spelling, read by the plan that edits the
//  * configs and by every caller that sets a server aside without one. */
// export const mcpRowId = (agent: string, at: boolean | string, name: string): string =>
//   `${MCP_ID_PREFIX}${agent}/${typeof at === "string" ? `@${encodeURIComponent(at)}/` : at ? "home/" : ""}${name}`;
/// golden-mcp.ts:99 `mcpRowId`: `agents/mcp/<agent>/<name>`, `.../home/<name>`, or `.../@<encodeURIComponent(folder)>/<name>`.
pub fn mcp_row_id(agent: &str, at: &McpAt, name: &str) -> String {
    let at = match at {
        McpAt::User => String::new(),
        McpAt::Home => "home/".to_string(),
        McpAt::Folder(folder) => format!("@{}/", encode_component(folder)),
    };
    format!("{MCP_ID_PREFIX}{agent}/{at}{name}")
}

/// golden-mcp.ts:103 `scopeRowId`: the folder where the scope names one, else home where it moves a project, else the
/// user scope.
pub fn scope_row_id(agent: &str, project: Option<&ProjectMove>, folder: Option<&str>, name: &str) -> String {
    let at = match (folder, project) {
        (Some(folder), _) => McpAt::Folder(folder.to_string()),
        (None, Some(_)) => McpAt::Home,
        (None, None) => McpAt::User,
    };
    mcp_row_id(agent, &at, name)
}

/// golden-mcp.ts:107 `parseMcpId`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpId {
    pub agent: String,
    pub home: bool,
    pub folder: Option<String>,
    pub name: String,
}

// TS golden-mcp.ts:105-115
// /** A row's agent, scope and server name from its id: `agents/mcp/<agent>/<name>`, `agents/mcp/<agent>/home/<name>`, or
//  * `agents/mcp/<agent>/@<folder>/<name>`; nothing for a row that is not a server, the mcp-remote row included. */
// export function parseMcpId(id: string): { agent: string; home: boolean; folder?: string; name: string } | undefined {
//   if (!id.startsWith(MCP_ID_PREFIX) || id === MCP_REMOTE_ID) return undefined;
//   const rest = id.slice(MCP_ID_PREFIX.length).split("/");
//   const agent = rest[0];
//   if (agent === undefined || rest.length < 2) return undefined;
//   if (rest.length >= 3 && rest[1]!.startsWith("@")) return { agent, home: false, folder: decodeURIComponent(rest[1]!.slice(1)), name: rest.slice(2).join("/") };
//   const home = rest.length === 3 && rest[1] === "home";
//   return { agent, home, name: home ? rest[2]! : rest.slice(1).join("/") };
// }
/// golden-mcp.ts:107 `parseMcpId`. A folder that does not decode is the TS's thrown `URIError`; this answers `None`.
pub fn parse_mcp_id(id: &str) -> Option<McpId> {
    if !id.starts_with(MCP_ID_PREFIX) || id == MCP_REMOTE_ID {
        return None;
    }
    let rest: Vec<&str> = id[MCP_ID_PREFIX.len()..].split('/').collect();
    let agent = rest[0].to_string();
    if rest.len() < 2 {
        return None;
    }
    if rest.len() >= 3 && rest[1].starts_with('@') {
        return Some(McpId { agent, home: false, folder: Some(decode_component(&rest[1][1..])?), name: rest[2..].join("/") });
    }
    let home = rest.len() == 3 && rest[1] == "home";
    let name = if home { rest[2].to_string() } else { rest[1..].join("/") };
    Some(McpId { agent, home, folder: None, name })
}

// TS golden-mcp.ts:93-94
// /** Rows whose last id segment is the binary they put on PATH; taps, casks and the toolchain install none. */
// const BINARY_ROW = /^tools\/(brew|go|cargo|npm|pnpm|bun|pipx|uv|hand)\//;
// TS golden-mcp.ts:117-156
// /** The plan from every row of the recipe with its tick: ticked servers stay, unticked ones come out with the
//  * row's own reason, a ticked server whose secret was not answered copy comes out too, and an agent that is not
//  * ticked has its servers set aside. Nothing when no row is a server. */
// export function mcpPlanFor(rows: readonly RecipeEntry[], opts: McpPlanOptions): McpPlan | undefined {
//   const guestHome = opts.guestHome ?? GUEST_HOME;
//   const ticked = new Set(rows.filter(e => e.bring === true).map(e => e.id));
//   const agents: McpAgentPlan[] = [];
//   for (const [agent, source] of Object.entries(opts.agents)) {
//     const own = rows.flatMap(e => {
//       const p = parseMcpId(e.id);
//       return p !== undefined && p.agent === agent ? [{ row: e, home: p.home, name: p.name }] : [];
//     });
//     if (own.length === 0) continue;
//     if (!ticked.has(`agents/${agent}`)) {
//       agents.push({ id: agent, label: source.label, scopes: [], aside: own.map(o => ({ id: o.row.id, name: o.name, reason: `${source.label} is not ticked, so its config did not travel` })) });
//       continue;
//     }
//     const scopes: McpScope[] = [];
//     for (const home of [false, true]) {
//       const rows = own.filter(o => o.home === home);
//       if (rows.length === 0) continue;
//       scopes.push({
//         files: source.files,
//         format: source.format,
//         ...(home ? { project: { from: opts.home, to: guestHome } } : {}),
//         keep: rows.filter(o => ticked.has(o.row.id) && !withheld(o.row)).map(o => o.name),
//         drop: rows.filter(o => !ticked.has(o.row.id) || withheld(o.row)).map(o => ({ name: o.name, reason: !ticked.has(o.row.id) ? (o.row.reason ?? "unticked") : WITHHELD_NOTE })),
//       });
//     }
//     agents.push({ id: agent, label: source.label, scopes, aside: [] });
//   }
//   if (agents.length === 0) return undefined;
//   return {
//     agents,
//     guestHome,
//     rewrites: [[`${opts.home}/`, `${guestHome}/`], [`${MAC_BREW}/`, `${BREW_PREFIX}/`]],
//     binDirs: [`${opts.home}/.local/bin/`, "~/.local/bin/", ...(opts.binDirs ?? MAC_BIN_DIRS)],
//     tools: rows.filter(e => e.rung === "tools" && BINARY_ROW.test(e.id)).map(e => ({ id: e.id, ticked: e.bring === true, ...(e.reason !== undefined ? { reason: e.reason } : {}) })),
//   };
// }
/// golden-mcp.ts:120 `mcpPlanFor(rows, opts)`: `None` when no agent has a server row.
pub fn mcp_plan_for(rows: &[RecipeEntry], opts: &McpPlanOptions) -> Option<McpPlan> {
    let guest_home = opts.guest_home.clone().unwrap_or_else(|| GUEST_HOME.to_string());
    let ticked: IndexSet<&str> = rows.iter().filter(|e| e.bring == Some(true)).map(|e| e.id.as_str()).collect();
    let mut agents = Vec::new();
    for (agent, source) in &opts.agents {
        let own: Vec<Own> = rows
            .iter()
            .filter_map(|e| parse_mcp_id(&e.id).filter(|p| p.agent == *agent).map(|p| Own { row: e, home: p.home, name: p.name }))
            .collect();
        if own.is_empty() {
            continue;
        }
        if !ticked.contains(format!("agents/{agent}").as_str()) {
            let aside = own
                .iter()
                .map(|o| AsideRow {
                    id: o.row.id.clone(),
                    name: o.name.clone(),
                    reason: format!("{} is not ticked, so its config did not travel", source.label),
                })
                .collect();
            agents.push(McpAgentPlan { id: agent.clone(), label: source.label.clone(), scopes: Vec::new(), aside });
            continue;
        }
        let mut scopes = Vec::new();
        for home in [false, true] {
            let mine: Vec<&Own> = own.iter().filter(|o| o.home == home).collect();
            if mine.is_empty() {
                continue;
            }
            let is_ticked = |o: &Own| ticked.contains(o.row.id.as_str());
            scopes.push(McpScope {
                files: source.files.clone(),
                format: source.format,
                project: home.then(|| ProjectMove { from: opts.home.clone(), to: guest_home.clone() }),
                keep: mine.iter().filter(|o| is_ticked(o) && !withheld(o.row)).map(|o| o.name.clone()).collect(),
                drop: mine
                    .iter()
                    .filter(|o| !is_ticked(o) || withheld(o.row))
                    .map(|o| DropName {
                        name: o.name.clone(),
                        reason: if is_ticked(o) {
                            WITHHELD_NOTE.to_string()
                        } else {
                            o.row.reason.clone().unwrap_or_else(|| "unticked".to_string())
                        },
                        keys: None,
                    })
                    .collect(),
                travelled: None,
                own: None,
                folder: None,
                trust: None,
                edit: None,
            });
        }
        agents.push(McpAgentPlan { id: agent.clone(), label: source.label.clone(), scopes, aside: Vec::new() });
    }
    if agents.is_empty() {
        return None;
    }
    let bin_dirs = match &opts.bin_dirs {
        Some(dirs) => dirs.clone(),
        None => MAC_BIN_DIRS.iter().map(|d| d.to_string()).collect(),
    };
    Some(McpPlan {
        agents,
        rewrites: vec![(format!("{}/", opts.home), format!("{guest_home}/")), (format!("{MAC_BREW}/"), format!("{BREW_PREFIX}/"))],
        bin_dirs: [vec![format!("{}/.local/bin/", opts.home), "~/.local/bin/".to_string()], bin_dirs].concat(),
        tools: rows
            .iter()
            .filter(|e| e.rung == "tools" && is_binary_row(&e.id))
            .map(|e| McpToolRow { id: e.id.clone(), ticked: e.bring == Some(true), reason: e.reason.clone() })
            .collect(),
        guest_home,
    })
}

/// golden-mcp.ts:161.
pub const READ_MS: u64 = 120_000;
/// golden-mcp.ts:163.
pub const MCP_READ_MARK: &str = "wsp-mcp";
/// golden-mcp.ts:208.
pub const MCP_READ_END: &str = "wsp-mcp end";

/// golden-mcp.ts:167 `refused`: a call the machine never ran as exit -1 with the error's words on stderr.
pub fn refused(message: &str) -> ExecResult {
    ExecResult { exit_code: (-1i64).into(), stdout: String::new(), stderr: message.to_string() }
}

/// golden-mcp.ts:172 `readReason`: `reason_of` with stdout blanked; `None` is `READ_MS / 1000` seconds.
pub fn read_reason(res: &ExecResult, timeout_s: Option<f64>) -> String {
    let blank = ExecResult { exit_code: res.exit_code, stdout: String::new(), stderr: res.stderr.clone() };
    reason_of(&blank, timeout_s.unwrap_or(READ_MS as f64 / 1000.0))
}

/// golden-mcp.ts:175 `readFailed`.
pub fn read_failed(res: &ExecResult) -> String {
    format!("the config edit did not run ({})", read_reason(res, None))
}

/// One scope as the read is asked for it (golden-mcp.ts:186).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadAsk {
    pub files: Vec<String>,
    pub base: String,
}

// TS golden-mcp.ts:177-204
// /** What a config's line carries in place of its bytes when the read left it alone; neither is a base64 word. */
// const UNREAD_WORDS: Record<string, string> = { "-plain": CONFIG_NOT_PLAIN, "-cap": CONFIG_PAST_CAP };
//
// /** One run reads every scope's config: the first of the scope's files that is there, base64 on one line, under the
//  * scope's place in the plan and the file's place in the scope. The file is read at its real path, and only where
//  * that is a plain file inside the scope's base, the folder its write stays inside, so a dotfiles link reads as the
//  * write writes through it; anything else is named and not opened, and a file past the cap is named and not sent: the
//  * home a workspace writes may point the name at a file out of it or at a device that never ends, and every byte read
//  * comes back to the host. */
// export function readConfigsCmd(scopes: readonly { files: readonly string[]; base: string }[]): string {
//   const cap = CONFIG_READ_MAX_BYTES;
//   return [
//     "wsp_mcp_read() {",
//     '  i="$1"; b=$(realpath "$2" 2>/dev/null) || b=; shift 2; n=0',
//     '  for f in "$@"; do',
//     '    if [ -e "$f" ] || [ -L "$f" ]; then',
//     `      r=$(realpath "$f" 2>/dev/null) && [ -n "$b" ] && ${underBase('[ -f "$r" ]', "false")} || { printf '${MCP_READ_MARK} %s %s -plain\\n' "$i" "$n"; return 0; }`,
//     `      if [ "$(head -c ${cap + 1} < "$r" | wc -c | tr -d ' ')" -gt ${cap} ]; then printf '${MCP_READ_MARK} %s %s -cap\\n' "$i" "$n"; return 0; fi`,
//     `      printf '${MCP_READ_MARK} %s %s ' "$i" "$n"; head -c ${cap} < "$r" | base64 | tr -d '\\n'; printf '\\n'; return 0`,
//     "    fi",
//     "    n=$((n+1))",
//     "  done",
//     `  printf '${MCP_READ_MARK} %s - -\\n' "$i"`,
//     "}",
//     ...scopes.map((s, i) => `wsp_mcp_read ${i} ${shellQuote(s.base)} ${s.files.map(shellQuote).join(" ")}`),
//     `printf '%s\\n' '${MCP_READ_END}'`,
//   ].join("\n");
// }
/// golden-mcp.ts:186 `readConfigsCmd`.
pub fn read_configs_cmd(scopes: &[ReadAsk]) -> String {
    let cap = CONFIG_READ_MAX_BYTES;
    let mut lines = vec![
        "wsp_mcp_read() {".to_string(),
        r#"  i="$1"; b=$(realpath "$2" 2>/dev/null) || b=; shift 2; n=0"#.to_string(),
        r#"  for f in "$@"; do"#.to_string(),
        r#"    if [ -e "$f" ] || [ -L "$f" ]; then"#.to_string(),
        format!(
            r#"      r=$(realpath "$f" 2>/dev/null) && [ -n "$b" ] && {} || {{ printf '{MCP_READ_MARK} %s %s -plain\n' "$i" "$n"; return 0; }}"#,
            under_base(r#"[ -f "$r" ]"#, "false")
        ),
        format!(
            r#"      if [ "$(head -c {} < "$r" | wc -c | tr -d ' ')" -gt {cap} ]; then printf '{MCP_READ_MARK} %s %s -cap\n' "$i" "$n"; return 0; fi"#,
            cap + 1
        ),
        format!(r#"      printf '{MCP_READ_MARK} %s %s ' "$i" "$n"; head -c {cap} < "$r" | base64 | tr -d '\n'; printf '\n'; return 0"#),
        "    fi".to_string(),
        "    n=$((n+1))".to_string(),
        "  done".to_string(),
        format!(r#"  printf '{MCP_READ_MARK} %s - -\n' "$i""#),
        "}".to_string(),
    ];
    lines.extend(scopes.iter().enumerate().map(|(i, s)| {
        let files: Vec<String> = s.files.iter().map(|f| shell_quote(f)).collect();
        format!("wsp_mcp_read {i} {} {}", shell_quote(&s.base), files.join(" "))
    }));
    lines.push(format!(r#"printf '%s\n' '{MCP_READ_END}'"#));
    lines.join("\n")
}

/// golden-mcp.ts:211 `readWhole`: a line that trims to `MCP_READ_END` is there.
pub fn read_whole(stdout: &str) -> bool {
    stdout.split('\n').any(|line| js_trim(line) == MCP_READ_END)
}

// TS golden-mcp.ts:232-253
// /** What the read printed, one entry per scope in plan order; nothing when not one line carried the mark, since the
//  * read says something about every scope it was given and a run that says nothing did not run. An empty file prints
//  * three words and is a file that is there and holds nothing, never a scope with no config on the machine. */
// export function parseConfigs(stdout: string, scopes: readonly { files: readonly string[] }[]): (ScopeRead | undefined)[] | undefined {
//   const out: (ScopeRead | undefined)[] = scopes.map(() => undefined);
//   let answered = false;
//   for (const line of stdout.split("\n")) {
//     const words = line.trim().split(" ");
//     if (words[0] !== MCP_READ_MARK || words.length < 3) continue;
//     answered = true;
//     const path = scopes[Number(words[1])]?.files[Number(words[2])];
//     if (path === undefined) continue;
//     const unread = UNREAD_WORDS[words[3] ?? ""];
//     if (unread !== undefined) {
//       out[Number(words[1])] = { path, unread };
//       continue;
//     }
//     const bytes = Buffer.from(words[3] ?? "", "base64");
//     out[Number(words[1])] = { path, text: bytes.toString("utf8"), sum: configSum(bytes) };
//   }
//   return answered ? out : undefined;
// }
/// golden-mcp.ts:235 `parseConfigs`: one entry per scope; `None` when no line carried the mark. Only `files` of each
/// ask is read.
pub fn parse_configs(stdout: &str, scopes: &[ReadAsk]) -> Option<Vec<Option<ScopeRead>>> {
    let mut out: Vec<Option<ScopeRead>> = scopes.iter().map(|_| None).collect();
    let mut answered = false;
    for line in stdout.split('\n') {
        let words: Vec<&str> = js_trim(line).split(' ').collect();
        if words[0] != MCP_READ_MARK || words.len() < 3 {
            continue;
        }
        answered = true;
        let at = js_index(words[1], scopes.len())
            .and_then(|i| js_index(words[2], scopes[i].files.len()).map(|j| (i, scopes[i].files[j].clone())));
        let Some((i, path)) = at else { continue };
        let word = words.get(3).copied().unwrap_or("");
        out[i] = Some(match word {
            "-plain" => ScopeRead::Unread { path, unread: CONFIG_NOT_PLAIN.to_string() },
            "-cap" => ScopeRead::Unread { path, unread: CONFIG_PAST_CAP.to_string() },
            _ => {
                let bytes = base64_lenient(word);
                ScopeRead::File(ScopeFile { path, text: String::from_utf8_lossy(&bytes).into_owned(), sum: config_sum(&bytes) })
            }
        });
    }
    answered.then_some(out)
}

// TS golden-mcp.ts:255-264
// /** A kept definition's string as the machine reads it: a command right under a bin directory becomes its bare name,
//  * and a laptop prefix becomes what it stands for there, a flag's `--name=` left in front of it. */
// export function rewriteString(plan: McpPlan, s: string, command: boolean): string {
//   if (command) for (const d of plan.binDirs) if (s.startsWith(d) && !s.slice(d.length).includes("/")) return s.slice(d.length);
//   const flag = /^--?[\w-]+=/.exec(s);
//   const head = flag !== null ? flag[0] : "";
//   const body = s.slice(head.length);
//   for (const [from, to] of plan.rewrites) if (body.startsWith(from)) return head + to + body.slice(from.length);
//   return s;
// }
/// golden-mcp.ts:257 `rewriteString`.
pub fn rewrite_string(plan: &McpPlan, s: &str, command: bool) -> String {
    if command {
        for d in &plan.bin_dirs {
            if let Some(rest) = s.strip_prefix(d.as_str()) {
                if !rest.contains('/') {
                    return rest.to_string();
                }
            }
        }
    }
    let head = flag_head(s);
    let body = &s[head.len()..];
    for (from, to) in &plan.rewrites {
        if let Some(rest) = body.strip_prefix(from.as_str()) {
            return format!("{head}{to}{rest}");
        }
    }
    s.to_string()
}

/// golden-mcp.ts:286's answer: each scope's outcome in plan order, and the text each file should end with, in the
/// order the files were first read then first written.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditedScopes {
    pub outcomes: Vec<ScopeOutcome>,
    pub texts: IndexMap<String, String>,
}

// TS golden-mcp.ts:284-312
// /** Every scope edited in plan order, each on its file's text as the scopes before it left it: two scopes of one
//  * agent share a file, and the second reads what the first wrote. The texts are what the machine should end with. */
// function editScopes(plan: McpPlan, agents: readonly McpAgentPlan[], read: readonly (ScopeRead | undefined)[]): { outcomes: ScopeOutcome[]; texts: Map<string, string> } {
//   const lib: McpEditLib = { rewriteString: (s, command) => rewriteString(plan, s, command) };
//   const texts = new Map(read.flatMap(f => (wasRead(f) ? [[f.path, f.text] as const] : [])));
//   const outcomes: ScopeOutcome[] = [];
//   let at = 0;
//   for (const agent of agents) {
//     for (const scope of agent.scopes) {
//       const file = read[at++];
//       if (file === undefined) {
//         outcomes.push({ file: null, results: [] });
//         continue;
//       }
//       if (!wasRead(file)) {
//         outcomes.push({ file: file.path, unread: file.unread, results: [] });
//         continue;
//       }
//       try {
//         const edited = scope.format.edit(lib, { keep: scope.keep, drop: scope.drop.map(d => d.name), ...(scope.project !== undefined ? { project: scope.project } : {}) }, texts.get(file.path)!);
//         texts.set(file.path, edited.text);
//         outcomes.push({ file: file.path, results: edited.results });
//       } catch (e) {
//         outcomes.push({ file: file.path, error: e instanceof Error ? e.message : String(e), results: [] });
//       }
//     }
//   }
//   return { outcomes, texts };
// }
/// golden-mcp.ts:286 `editScopes(plan, agents, read)`: every scope through its format's `edit`, each on the text the
/// scopes before it left.
pub fn edit_scopes(plan: &McpPlan, agents: &[McpAgentPlan], read: &[Option<ScopeRead>]) -> EditedScopes {
    let mut texts: IndexMap<String, String> = read
        .iter()
        .flatten()
        .filter_map(|f| match f {
            ScopeRead::File(f) => Some((f.path.clone(), f.text.clone())),
            ScopeRead::Unread { .. } => None,
        })
        .collect();
    let mut outcomes = Vec::new();
    let mut at = 0;
    for agent in agents {
        for scope in &agent.scopes {
            let file = read.get(at).and_then(|f| f.as_ref());
            at += 1;
            let outcome = match file {
                None => ScopeOutcome::default(),
                Some(ScopeRead::Unread { path, unread }) => {
                    ScopeOutcome { file: Some(path.clone()), unread: Some(unread.clone()), ..Default::default() }
                }
                Some(ScopeRead::File(f)) => {
                    let edit_scope = McpEditScope {
                        keep: scope.keep.clone(),
                        drop: scope.drop.iter().map(|d| d.name.clone()).collect(),
                        project: scope.project.as_ref().map(|p| FromTo { from: p.from.clone(), to: p.to.clone() }),
                    };
                    let text = texts.get(&f.path).cloned().unwrap_or_default();
                    let lib = |s: &str, command: bool| rewrite_string(plan, s, command);
                    match scope.format.edit(&lib, &edit_scope, &text) {
                        Ok(edited) => {
                            texts.insert(f.path.clone(), edited.text);
                            ScopeOutcome {
                                file: Some(f.path.clone()),
                                results: edited
                                    .results
                                    .into_iter()
                                    .map(|r| EditedName { name: r.name, outcome: r.outcome.into(), command: r.command })
                                    .collect(),
                                ..Default::default()
                            }
                        }
                        Err(e) => ScopeOutcome { file: Some(f.path.clone()), error: Some(e.to_string()), ..Default::default() },
                    }
                }
            };
            outcomes.push(outcome);
        }
    }
    EditedScopes { outcomes, texts }
}

// TS golden-mcp.ts:314-335
// /** Puts every config the edit changed back on the machine, each by the one config write inside `base`, from a
//  * landing beside it; the words when one of them did not land. A landing the write never took is swept: it holds the
//  * whole config at the upload road's own mode, and the machine it sits on may be about to be sealed into an image. */
// export async function landConfigs(machine: Machine, base: string | ((path: string) => string), read: readonly (ScopeRead | undefined)[], texts: ReadonlyMap<string, string>): Promise<string | undefined> {
//   const was = new Map(read.flatMap(f => (wasRead(f) ? [[f.path, f] as const] : [])));
//   const changed = [...texts].filter(([path, text]) => was.get(path)?.text !== text).map(([path, text]) => ({ path, bytes: new TextEncoder().encode(text), landing: configLanding(path, randomBytes(6).toString("hex")) }));
//   if (changed.length === 0) return undefined;
//   const swept = async (why: string): Promise<string> => {
//     await machine.exec(`rm -f ${changed.map(c => shellQuote(c.landing)).join(" ")}`, { timeoutMs: INLINE_EXEC_MS }).catch(() => undefined);
//     return `the edited config did not land (${why})`;
//   };
//   try {
//     for (const c of changed) await landBytes(machine, c.landing, c.bytes, { timeoutMs: READ_MS });
//   } catch (e) {
//     return swept(e instanceof Error ? e.message : String(e));
//   }
//   // Each write in a subshell of its own, so its exit and its trap are its own and the first refusal stops the rest.
//   const line = changed.map(c => `(\n${configWriteLine({ file: c.path, base: typeof base === "string" ? base : base(c.path), bytes: c.bytes.length, from: c.landing, ...(was.has(c.path) ? { sum: was.get(c.path)!.sum } : {}) })}\n) || exit $?`).join("\n");
//   const res = await machine.exec(line, { timeoutMs: INLINE_EXEC_MS }).catch(refused);
//   if (res.exitCode === 0) return undefined;
//   return swept(configRefusal(res, changed[0]!.path, p => p) ?? reasonOf(res, INLINE_EXEC_MS / 1000));
// }
/// golden-mcp.ts:317 `landConfigs(machine, base, read, texts)`: the words when a config did not land. Never fails.
pub async fn land_configs(
    machine: &dyn Machine,
    base: &dyn Fn(&str) -> String,
    read: &[Option<ScopeRead>],
    texts: &IndexMap<String, String>,
) -> Option<String> {
    let was: IndexMap<&str, &ScopeFile> = read
        .iter()
        .flatten()
        .filter_map(|f| match f {
            ScopeRead::File(f) => Some((f.path.as_str(), f)),
            ScopeRead::Unread { .. } => None,
        })
        .collect();
    let mut changed = Vec::new();
    for (path, text) in texts {
        if was.get(path.as_str()).is_some_and(|f| f.text == *text) {
            continue;
        }
        let id = hex(&rand::random::<[u8; 6]>());
        let landing = config_landing(path, &id).unwrap_or_else(|e| panic!("{e:?}"));
        changed.push(Changed { path: path.clone(), bytes: text.as_bytes().to_vec(), landing });
    }
    if changed.is_empty() {
        return None;
    }
    let landings: Vec<String> = changed.iter().map(|c| c.landing.clone()).collect();
    for c in &changed {
        let opts = LandBytesOptions { timeout_ms: Some(READ_MS), ..Default::default() };
        if let Err(e) = land_bytes(machine, &c.landing, &c.bytes, opts).await {
            return Some(swept(machine, &landings, &e.to_string()).await);
        }
    }
    let line = changed
        .iter()
        .map(|c| {
            let write = ConfigWrite {
                file: c.path.clone(),
                base: base(&c.path),
                sum: was.get(c.path.as_str()).map(|f| f.sum.clone()),
                bytes: c.bytes.len() as u64,
                from: Some(c.landing.clone()),
            };
            format!("(\n{}\n) || exit $?", config_write_line(&write))
        })
        .collect::<Vec<String>>()
        .join("\n");
    let res = match machine.exec(&line, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await {
        Ok(res) => res,
        Err(e) => refused(&e.to_string()),
    };
    if res.exit_code.get() == 0 {
        return None;
    }
    let why = config_refusal(res.exit_code.get() as i32, &res.stdout, &changed[0].path, &|p: &str| p.to_string())
        .unwrap_or_else(|| reason_of(&res, inline_s()));
    Some(swept(machine, &landings, &why).await)
}

struct Own<'a> {
    row: &'a RecipeEntry,
    home: bool,
    name: String,
}

struct Changed {
    path: String,
    bytes: Vec<u8>,
    landing: String,
}

impl From<EditOutcome> for EditedOutcome {
    fn from(outcome: EditOutcome) -> Self {
        match outcome {
            EditOutcome::Written => EditedOutcome::Written,
            EditOutcome::Missing => EditedOutcome::Missing,
            EditOutcome::Dropped => EditedOutcome::Dropped,
        }
    }
}

fn is_binary_row(id: &str) -> bool {
    id.strip_prefix("tools/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(manager, _)| ["brew", "go", "cargo", "npm", "pnpm", "bun", "pipx", "uv", "hand"].contains(&manager))
}

fn encode_component(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

fn decode_component(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn flag_head(s: &str) -> &str {
    for dashes in [2usize, 1] {
        let Some(rest) = s.strip_prefix(&"--"[..dashes]) else { continue };
        let name = rest.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-').count();
        if name > 0 && rest.as_bytes().get(name) == Some(&b'=') {
            return &s[..dashes + name + 1];
        }
    }
    ""
}

fn js_index(word: &str, len: usize) -> Option<usize> {
    let n = to_number(word);
    (n.fract() == 0.0 && n >= 0.0 && (n as usize) < len).then_some(n as usize)
}

fn base64_lenient(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in text.bytes() {
        if c == b'=' {
            break;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn swept(machine: &dyn Machine, landings: &[String], why: &str) -> String {
    let cmd = format!("rm -f {}", landings.iter().map(|l| shell_quote(l)).collect::<Vec<String>>().join(" "));
    let _ = machine.exec(&cmd, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await;
    format!("the edited config did not land ({why})")
}
