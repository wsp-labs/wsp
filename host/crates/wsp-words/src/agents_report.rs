// SPDX-License-Identifier: AGPL-3.0-only
//! What stands on one computer or workspace for the agents: each agent with its version and sign-in word, every skill and every server, and the lines a report says. Port of the functions of packages/protocol/src/agents-report.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::agents_report. See spec/agents_report.md.

use wsp_wire::types::agents_report::AgentsTarget;
use wsp_wire::types::agents_report::PluginMissing;
use wsp_wire::types::agents_report::PluginRow;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::wire::limits::EXEC_BODY_MAX;

use crate::js::is_js_whitespace;
use crate::js::js_round;
use crate::js::locale_number;
use crate::js::number_string;
use crate::js::Grouping;

/// What a napping workspace is in a refusal's or a stale report's words: only a computer that keeps the machine's memory
/// naps it, and every other one, or one whose mode cannot be read, stops it, as workspaceWord says.
// agents-report.ts:16
// export const nappedWord = (pauseMode?: PauseMode): string => (pauseMode === "memory" ? "napping" : "stopped");
pub fn napped_word(pause_mode: Option<&PauseMode>) -> String {
    let word = if matches!(pause_mode, Some(PauseMode::Memory)) { "napping" } else { "stopped" };
    word.to_string()
}

// Whether an agent on a computer can run a turn there without anybody signing anything in: its own login stands on
// that computer, the vault this host holds has the variable that agent reads, or neither. One word per agent, worked
// out by the host from the computer's report and the vault, since the computer knows no catalog.
// agents-report.ts:21
// in wsp_wire::types::agents_report: AgentSignInState

// agents-report.ts:22
// in wsp_wire::types::agents_report: AgentSignInState

// What an agent's own login there is, as its status command says it: a key, a plan's subscription, or another
// account's OAuth. Absent where the status names none; signed out is the row's sign-in word.
// agents-report.ts:26
// in wsp_wire::types::agents_report: AgentSignInKind

// agents-report.ts:27
// in wsp_wire::types::agents_report: AgentSignInKind

// What a report is read off: a computer by the id places.list gives it (this one's included), with `project` one of
// the projects on it by its id or its name, which is what an act on that project's skills and servers names; or one
// workspace.
// agents-report.ts:32
// in wsp_wire::types::agents_report: AgentsTarget

// agents-report.ts:33
// in wsp_wire::types::agents_report: AgentsTarget

/// Whether a pty opened for this target is on a computer somebody owns, whose root home every workspace there
/// writes, so bash there reads no startup file.
// agents-report.ts:37
// export const ptyBareOn = (target: AgentsTarget): boolean => "placeId" in target && target.placeId !== HERE_PLACE_ID;
pub fn pty_bare_on(target: &AgentsTarget) -> bool {
    match target {
        AgentsTarget::V0(t) => t.place_id != HERE_PLACE_ID,
        AgentsTarget::V1(_) => false,
    }
}

// A project a report read the folders of, by the id and name projects.list gives it and its folder on that
// computer, `~`-relative under the home: every project on a computer, or a workspace's own.
// agents-report.ts:41
// in wsp_wire::types::agents_report: AgentsProject

// agents-report.ts:42
// in wsp_wire::types::agents_report: AgentsProject

/// Why a target named a project the computer does not hold.
// agents-report.ts:45
// export const noSuchAgentsProjectRefusal = (id: string, computer: string): string => `There is no project ${id} on ${computer}.`;
pub fn no_such_agents_project_refusal(id: &str, computer: &str) -> String {
    format!("There is no project {id} on {computer}.")
}

/// Why a target named a project by a name two projects on that computer share.
// agents-report.ts:48
// export const sharedAgentsProjectRefusal = (name: string, computer: string): string => `Two projects on ${computer} are named ${name}; name one by the id wsp projects shows.`;
pub fn shared_agents_project_refusal(name: &str, computer: &str) -> String {
    format!("Two projects on {computer} are named {name}; name one by the id wsp projects shows.")
}

// How the agent's binary got onto that computer: by wsp's own install under its tools folder or the setup of a
// computer you own, by the person or another installer (`own`), as a wrapper another program puts in front of it
// (`shim`), or it is not there.
// agents-report.ts:53
// in wsp_wire::types::agents_report: AgentRoad

// agents-report.ts:54
// in wsp_wire::types::agents_report: AgentRoad

// How a person signs the agent in where it stands: a device page, a code pasted back, a key, a token minted here,
// the agent's own terminal because it asks the person to pick, or no sign-in at all. Off the catalog's sign-in row.
// agents-report.ts:58
// in wsp_wire::types::agents_report: SignInRoad

// agents-report.ts:59
// in wsp_wire::types::agents_report: SignInRoad

// agents-report.ts:61
// in wsp_wire::types::agents_report: AgentRow

// agents-report.ts:98
// in wsp_wire::types::agents_report: AgentRow

// One folder a skill lives in: `~`-relative, the agent whose own folder it is (none for a folder several agents
// read), where a link points when the folder is one, and whether it is turned off: its SKILL.md renamed
// SKILL.md.off, or the plugin it comes with off, so no agent loads it.
// agents-report.ts:103
// in wsp_wire::types::agents_report: SkillPath

// agents-report.ts:104
// in wsp_wire::types::agents_report: SkillPath

// agents-report.ts:106
// in wsp_wire::types::agents_report: SkillScope

// agents-report.ts:107
// in wsp_wire::types::agents_report: SkillScope

// One skill by its name, with every folder it lives in. `description` is off its SKILL.md's frontmatter; `project`
// is the project a project skill lives in; `plugin` is the id of the plugin a plugin's skill comes with, whose name
// leads the skill's as the agent announces it (`brag:brag`).
// agents-report.ts:112
// in wsp_wire::types::agents_report: SkillRow

// agents-report.ts:113
// in wsp_wire::types::agents_report: SkillRow

// One skill skills.sh lists for a search: `id` is `<owner>/<repo>/<skill>`, what an install names.
// agents-report.ts:116
// in wsp_wire::types::agents_report: SkillHit

// agents-report.ts:117
// in wsp_wire::types::agents_report: SkillHit

// How much of a SKILL.md a preview carries.
// agents-report.ts:120
// in wsp_wire::types::agents_report: SKILL_PREVIEW_BYTES

// A skill's SKILL.md as a preview draws it: its first part, up to SKILL_PREVIEW_BYTES, and the whole file's size.
// agents-report.ts:123
// in wsp_wire::types::agents_report: SkillPreview

// agents-report.ts:124
// in wsp_wire::types::agents_report: SkillPreview

// Where an install put a skill: its one folder, and each agent's folder that got a link to it or a copy of it.
// agents-report.ts:127
// in wsp_wire::types::agents_report: SkillAdded

// agents-report.ts:128
// in wsp_wire::types::agents_report: SkillAdded

// agents-report.ts:131
// in wsp_wire::types::agents_report: skillsSearchEmptyRefusal

/// Why the skill wsp writes is never turned off or removed: the next start writes it again.
// agents-report.ts:134
// export const systemSkillRefusal = (name: string): string => `${name} is written by wsp and kept current on every start, so it is always on.`;
pub fn system_skill_refusal(name: &str) -> String {
    format!("{name} is written by wsp and kept current on every start, so it is always on.")
}

/// Why a plugin's skill is not turned off or removed on its own, with the line that turns its plugin off.
// agents-report.ts:137
// export const pluginSkillRefusal = (name: string, plugin: string, agent: string): string => `${name} comes with the plugin ${plugin}, so it goes with it; turn the plugin off with wsp plugins disable ${plugin} --agent ${agent}.`;
pub fn plugin_skill_refusal(name: &str, plugin: &str, agent: &str) -> String {
    format!(
        "{name} comes with the plugin {plugin}, so it goes with it; turn the plugin off with wsp plugins disable {plugin} --agent {agent}."
    )
}

/// Why a project's skill is not turned off: it lives in the repo, and the rename would be a change to it.
// agents-report.ts:140
// export const projectSkillOffRefusal = (name: string, path: string): string => `${name} lives in the repo at ${path}, so it is not turned off here.`;
pub fn project_skill_off_refusal(name: &str, path: &str) -> String {
    format!("{name} lives in the repo at {path}, so it is not turned off here.")
}

/// Why an act named a skill the computer does not have.
// agents-report.ts:143
// export const noSuchSkillRefusal = (name: string): string => `There is no skill named ${name} there.`;
pub fn no_such_skill_refusal(name: &str) -> String {
    format!("There is no skill named {name} there.")
}

/// Why a napping workspace's skills were not read or changed: nothing here wakes a machine.
// agents-report.ts:146
// export const nappingSkillsRefusal = (name: string, pauseMode?: PauseMode): string => `${name} is ${nappedWord(pauseMode)}, and its skills are read and changed only while it runs; wake it first`;
pub fn napping_skills_refusal(name: &str, pause_mode: Option<&PauseMode>) -> String {
    format!("{name} is {}, and its skills are read and changed only while it runs; wake it first", napped_word(pause_mode))
}

/// A command line as a person types it, in words: split at spaces, a word in single quotes as written, one in double
/// quotes with its backslash escapes, and a backslash outside quotes taking the next character as it is. Nothing is
/// expanded; a line with a quote it never closes has no words, as a shell refuses it.
// agents-report.ts:151
// export function commandWords(line: string): string[] | undefined {
//   const words: string[] = [];
//   let word: string | undefined;
//   for (let i = 0; i < line.length; i++) {
//     const c = line[i]!;
//     if (c === "'") {
//       const end = line.indexOf("'", i + 1);
//       if (end < 0) return undefined;
//       word = (word ?? "") + line.slice(i + 1, end);
//       i = end;
//     } else if (c === '"') {
//       word ??= "";
//       for (i++; i < line.length && line[i] !== '"'; i++) word += line[i] === "\\" && i + 1 < line.length ? line[++i]! : line[i]!;
//       if (i >= line.length) return undefined;
//     } else if (c === "\\" && i + 1 < line.length) word = (word ?? "") + line[++i]!;
//     else if (/\s/.test(c)) {
//       if (word !== undefined) words.push(word);
//       word = undefined;
//     } else word = (word ?? "") + c;
//   }
//   if (word !== undefined) words.push(word);
//   return words;
// }
pub fn command_words(line: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = line.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word: Option<String> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            let end = chars[i + 1..].iter().position(|&x| x == '\'').map(|p| p + i + 1)?;
            word.get_or_insert_with(String::new).extend(&chars[i + 1..end]);
            i = end;
        } else if c == '"' {
            let w = word.get_or_insert_with(String::new);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                w.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() {
                return None;
            }
        } else if c == '\\' && i + 1 < chars.len() {
            i += 1;
            word.get_or_insert_with(String::new).push(chars[i]);
        } else if is_js_whitespace(c) {
            words.extend(word.take());
        } else {
            word.get_or_insert_with(String::new).push(c);
        }
        i += 1;
    }
    words.extend(word);
    Some(words)
}

// agents-report.ts:176
// in wsp_wire::types::agents_report: unclosedQuoteRefusal

/// Why a server's name was refused: the agent's own format would not read it back as that name.
// agents-report.ts:179
// export const serverNameFormatRefusal = (agent: string): string => `${agent} cannot keep a server under that name, so nothing was written.`;
pub fn server_name_format_refusal(agent: &str) -> String {
    format!("{agent} cannot keep a server under that name, so nothing was written.")
}

// agents-report.ts:182
// in wsp_wire::types::agents_report: serverNameRefusal

/// Why an act named a server the agent's file does not define in that scope.
// agents-report.ts:185
// export const noSuchServerRefusal = (name: string, file: string): string => `There is no server named ${name} in ${file}.`;
pub fn no_such_server_refusal(name: &str, file: &str) -> String {
    format!("There is no server named {name} in {file}.")
}

/// Why an add did not write over a server of the same name.
// agents-report.ts:188
// export const serverThereRefusal = (name: string, file: string): string => `${name} is already in ${file}, so nothing was written; remove it first or pick another name.`;
pub fn server_there_refusal(name: &str, file: &str) -> String {
    format!("{name} is already in {file}, so nothing was written; remove it first or pick another name.")
}

/// Why a config that is a link out of the home, or out of the project for a project's file, was not written.
// agents-report.ts:191
// export const configLinkRefusal = (file: string, to: string): string => `${file} is a link to ${to}, outside the folder it belongs to, so wsp does not write through it.`;
pub fn config_link_refusal(file: &str, to: &str) -> String {
    format!("{file} is a link to {to}, outside the folder it belongs to, so wsp does not write through it.")
}

// The most of one agent's config wsp reads, on a machine or here, far past the 968 KB Claude file its own issues
// call bloated: the file sits in a home a workspace writes, and a link to a device there never ends.
// agents-report.ts:195
// in wsp_wire::types::agents_report: CONFIG_READ_MAX_BYTES

// agents-report.ts:197
// in wsp_wire::types::agents_report: CONFIG_NOT_PLAIN

// Why wsp left an agent's config unread: it is past CONFIG_READ_MAX_BYTES.
// agents-report.ts:199
// in wsp_wire::types::agents_report: CONFIG_PAST_CAP

/// What a server of a config wsp left unread says: the file and why, its servers in it standing as they are.
// agents-report.ts:201
// export const configUnreadLine = (file: string, why: string): string => `${file} was not read: ${why}`;
pub fn config_unread_line(file: &str, why: &str) -> String {
    format!("{file} was not read: {why}")
}

/// What a leave says of an agent's config it did not read: wsp's servers in it stay.
// agents-report.ts:203
// export const serversUnreadLine = (file: string, why: string): string => `wsp's MCP servers were not looked for in ${file}: ${why}`;
pub fn servers_unread_line(file: &str, why: &str) -> String {
    format!("wsp's MCP servers were not looked for in {file}: {why}")
}

/// Why a config was not written: a second hard link to it would keep the old text after a write by rename; and how
/// to find that link and let the write through.
// agents-report.ts:207
// export const configHardLinkRefusal = (file: string): string =>
//   `${file} has another hard link, which a write by rename would split from it, so nothing was written. See the link count with ls -li ${file} and find the other name with find / -xdev -samefile ${file}, since a hard link stays on its own volume; remove that name or give ${file} a copy of its own, then run the command again.`;
pub fn config_hard_link_refusal(file: &str) -> String {
    format!("{file} has another hard link, which a write by rename would split from it, so nothing was written. See the link count with ls -li {file} and find the other name with find / -xdev -samefile {file}, since a hard link stays on its own volume; remove that name or give {file} a copy of its own, then run the command again.")
}

/// Why a write stopped: the agent or the person wrote the file between wsp's read and its write.
// agents-report.ts:211
// export const configChangedRefusal = (file: string): string => `${file} changed while wsp was writing it, so nothing was written; try again.`;
pub fn config_changed_refusal(file: &str) -> String {
    format!("{file} changed while wsp was writing it, so nothing was written; try again.")
}

/// Why a server was not turned off or on: its agent keeps no switch per server that wsp turns.
// agents-report.ts:214
// export const noServerSwitchRefusal = (agent: string): string => `${agent} keeps no switch per server that wsp turns, so nothing was changed.`;
pub fn no_server_switch_refusal(agent: &str) -> String {
    format!("{agent} keeps no switch per server that wsp turns, so nothing was changed.")
}

/// Why an agent's servers were not changed: it keeps none in a file wsp writes.
// agents-report.ts:217
// export const noServersConfigRefusal = (agent: string): string => `${agent} keeps no MCP servers in a file wsp writes.`;
pub fn no_servers_config_refusal(agent: &str) -> String {
    format!("{agent} keeps no MCP servers in a file wsp writes.")
}

/// Why a napping workspace's servers were not changed: nothing here wakes a machine.
// agents-report.ts:220
// export const nappingServersRefusal = (name: string, pauseMode?: PauseMode): string => `${name} is ${nappedWord(pauseMode)}, and its servers are changed only while it runs; wake it first`;
pub fn napping_servers_refusal(name: &str, pause_mode: Option<&PauseMode>) -> String {
    format!("{name} is {}, and its servers are changed only while it runs; wake it first", napped_word(pause_mode))
}

// Where a plugin is installed for: every folder of the login, one project for everyone in its repo, or one project
// for this login alone.
// agents-report.ts:224
// in wsp_wire::types::agents_report: PluginScope

// agents-report.ts:225
// in wsp_wire::types::agents_report: PluginScope

// What a plugin brings, each by the name a turn gives it: its skills, slash commands and subagents, the events its
// hooks run on, its tool servers, language servers and apps.
// agents-report.ts:229
// in wsp_wire::types::agents_report: PluginBrings

// agents-report.ts:238
// in wsp_wire::types::agents_report: PluginBrings

// Why the agent loads nothing of a plugin it names: its install folder is not there (`folder`); the agent lists no
// such plugin, its marketplace a folder that is not there (`marketplace`), or for no reason it gives (`unlisted`); or
// it lists the plugin in its marketplace and not installed (`uninstalled`).
// agents-report.ts:243
// in wsp_wire::types::agents_report: PluginMissing

// agents-report.ts:244
// in wsp_wire::types::agents_report: PluginMissing

// One plugin of one agent, as `name@marketplace`, in the scope it is installed for. `on` is what the agent's own
// settings make of it there; `path` its folder, `~`-relative; `setIn` the settings file a project or local row is
// switched in; `source` where its marketplace comes from, a GitHub repo as owner/name, an address or a folder.
// agents-report.ts:249
// in wsp_wire::types::agents_report: PluginRow

// agents-report.ts:265
// in wsp_wire::types::agents_report: PluginRow

// One plugin of one agent on a target, by its id (`name@marketplace`).
// agents-report.ts:268
// in wsp_wire::types::agents_report: PluginAsk

// agents-report.ts:269
// in wsp_wire::types::agents_report: PluginAsk

/// Why a missing plugin loads in no turn there, in the agent's terms.
// agents-report.ts:272
// export function pluginMissingLine(row: Pick<PluginRow, "id" | "marketplace" | "missing" | "on">, agent: string, computer: string): string {
//   if (row.missing === "folder") return `${agent} has ${row.id} ${row.on ? "on" : "installed"}, but its folder is not on ${computer}, so no turn loads it.`;
//   if (row.missing === "uninstalled") return `The config.toml on ${computer} names ${row.id}, but ${agent} has it in its marketplace ${row.marketplace} and not installed, so no turn loads it.`;
//   const unlisted = `The config.toml on ${computer} names ${row.id}, but ${agent} lists no such plugin there.`;
//   return row.missing === "marketplace" ? `${unlisted} Its marketplace, ${row.marketplace}, is a folder that is not on ${computer}.` : unlisted;
// }
pub fn plugin_missing_line(row: &PluginRow, agent: &str, computer: &str) -> String {
    match row.missing {
        Some(PluginMissing::Folder) => {
            let on = if row.on { "on" } else { "installed" };
            format!("{agent} has {} {on}, but its folder is not on {computer}, so no turn loads it.", row.id)
        }
        Some(PluginMissing::Uninstalled) => format!(
            "The config.toml on {computer} names {}, but {agent} has it in its marketplace {} and not installed, so no turn loads it.",
            row.id, row.marketplace
        ),
        _ => {
            let unlisted = format!("The config.toml on {computer} names {}, but {agent} lists no such plugin there.", row.id);
            match row.missing {
                Some(PluginMissing::Marketplace) => {
                    format!("{unlisted} Its marketplace, {}, is a folder that is not on {computer}.", row.marketplace)
                }
                _ => unlisted,
            }
        }
    }
}

/// Why an act named a plugin the report does not list for that agent.
// agents-report.ts:280
// export const noSuchPluginRefusal = (id: string, agent: string): string => `There is no plugin ${id} for ${agent} there, so nothing was switched.`;
pub fn no_such_plugin_refusal(id: &str, agent: &str) -> String {
    format!("There is no plugin {id} for {agent} there, so nothing was switched.")
}

/// What to do where the agent's plugins could not be read, so whether one is there is not known.
// agents-report.ts:283
// export const pluginsNotReadFix = (agent: string): string => `Read again once ${agent} answers there, then switch the plugin.`;
pub fn plugins_not_read_fix(agent: &str) -> String {
    format!("Read again once {agent} answers there, then switch the plugin.")
}

/// Why a missing plugin is not switched.
// agents-report.ts:286
// export const missingPluginRefusal = (line: string): string => `${line} Nothing was switched.`;
pub fn missing_plugin_refusal(line: &str) -> String {
    format!("{line} Nothing was switched.")
}

/// Why a project's or a project's local plugin is not switched here: its settings file lives in the project.
// agents-report.ts:289
// export const projectPluginRefusal = (id: string, file: string): string => `${id} is set in the repo at ${file}, so it is not switched here.`;
pub fn project_plugin_refusal(id: &str, file: &str) -> String {
    format!("{id} is set in the repo at {file}, so it is not switched here.")
}

/// Why an agent's plugins are not switched by wsp.
// agents-report.ts:292
// export const noPluginSwitchRefusal = (agent: string): string => `${agent} keeps no plugin switch wsp turns, so nothing was switched.`;
pub fn no_plugin_switch_refusal(agent: &str) -> String {
    format!("{agent} keeps no plugin switch wsp turns, so nothing was switched.")
}

/// Why a napping workspace's plugins were not switched: nothing here wakes a machine.
// agents-report.ts:295
// export const nappingPluginsRefusal = (name: string, pauseMode?: PauseMode): string => `${name} is ${nappedWord(pauseMode)}, and its plugins are switched only while it runs; wake it first`;
pub fn napping_plugins_refusal(name: &str, pause_mode: Option<&PauseMode>) -> String {
    format!("{name} is {}, and its plugins are switched only while it runs; wake it first", napped_word(pause_mode))
}

// One property of a tool's input, off its input schema: its name, the type its schema gives it where it gives one,
// and whether a call has to carry it.
// agents-report.ts:299
// in wsp_wire::types::agents_report: McpToolParam

// agents-report.ts:300
// in wsp_wire::types::agents_report: McpToolParam

// One tool a server lists, as its tools/list answers it.
// agents-report.ts:303
// in wsp_wire::types::agents_report: McpTool

// agents-report.ts:304
// in wsp_wire::types::agents_report: McpTool

// How a server is reached, shown and never run: the command with every value of the person's hidden, or the host
// of its url.
// agents-report.ts:308
// in wsp_wire::types::agents_report: McpRowTransport

// agents-report.ts:309
// in wsp_wire::types::agents_report: McpRowTransport

// A server's state: nothing to sign in as its config says, unchecked (`open`); a token its config takes from the
// environment (`env-key`); its address answered with no sign-in asked (`connected`); a sign-in its harness holds
// (`signed-in`); one it needs; one that failed; or no way to tell without connecting (`unknown`).
// agents-report.ts:314
// in wsp_wire::types::agents_report: McpAuth

// agents-report.ts:315
// in wsp_wire::types::agents_report: McpAuth

// Where a page that returns to localhost reaches the harness: `here`, the computer the browser is on; `relay`, a
// computer whose callback port this host forwards from here; `none`, a computer it does not.
// agents-report.ts:319
// in wsp_wire::types::agents_report: PageReach

// agents-report.ts:320
// in wsp_wire::types::agents_report: PageReach

// `home`: Claude Code's servers kept for the home folder itself; `local`: those it keeps for a project's folder, in its
// own file; `project`: a file in the project's folder or a folder above it.
// agents-report.ts:324
// in wsp_wire::types::agents_report: McpScope

// agents-report.ts:325
// in wsp_wire::types::agents_report: McpScope

// One MCP server of one agent's config on a target, in the scope the report read it from; the person's own when
// unsaid.
// agents-report.ts:329
// in wsp_wire::types::agents_report: ServerAsk

// agents-report.ts:330
// in wsp_wire::types::agents_report: ServerAsk

// One MCP server to write into an agent's config on a target, the project's with `project`: a command with its
// arguments and variables, or an address with its headers, as the person typed them. `env` and `headers` carry the
// values, which go into that file and nowhere else.
// agents-report.ts:335
// in wsp_wire::types::agents_report: ServerAdd

// agents-report.ts:345
// in wsp_wire::types::agents_report: ServerAdd

// agents-report.ts:347
// in wsp_wire::types::agents_report: McpRow

// agents-report.ts:373
// in wsp_wire::types::agents_report: McpRow

/// Why a server that needs the person's sign-in or key is off on a cloud machine, in the servers list, the app's rows
/// and a slate's refusal alike.
// agents-report.ts:377
// export const cloudServerOffLine = (name: string): string => `${name} runs only on a computer where you signed in to it; a cloud machine holds no sign-in or key of yours.`;
pub fn cloud_server_off_line(name: &str) -> String {
    format!("{name} runs only on a computer where you signed in to it; a cloud machine holds no sign-in or key of yours.")
}

// What one server said when it was started once, or asked once over its address, on the person's click: its sign-in
// as that connect found it, and its tools. `holder` is the agent whose harness holds the server's sign-in, which
// this host never reads, so no list comes back for it; `refused` is why nothing came back.
// agents-report.ts:382
// in wsp_wire::types::agents_report: ServerToolsAnswer

// agents-report.ts:389
// in wsp_wire::types::agents_report: ServerToolsAnswer

/// Why a server started for its tools gave none: it had not answered when its time was up, and was stopped.
// agents-report.ts:392
// export const serverToolsLateRefusal = (ms: number): string => `Did not answer in ${Math.round(ms / 1000)} s.`;
pub fn server_tools_late_refusal(ms: f64) -> String {
    format!("Did not answer in {} s.", number_string(js_round(ms / 1000.0)))
}

// agents-report.ts:394
// in wsp_wire::types::agents_report: AgentsReport

// agents-report.ts:417
// in wsp_wire::types::agents_report: AgentsReport

/// Why a cloud account's row has no report: nothing stands there between forks.
// agents-report.ts:420
// export const providerAgentsRefusal = (name: string): string =>
//   `${name} keeps no computer to read, since every workspace there is forked fresh from the image; name a workspace there, or edit the image`;
pub fn provider_agents_refusal(name: &str) -> String {
    format!("{name} keeps no computer to read, since every workspace there is forked fresh from the image; name a workspace there, or edit the image")
}

/// Why a napping workspace read nothing: a read never wakes a machine, and none was read while it ran.
// agents-report.ts:424
// export const nappingAgentsRefusal = (name: string, pauseMode?: PauseMode): string => `${name} is ${nappedWord(pauseMode)} and was not read while it ran; wake it to read what stands there`;
pub fn napping_agents_refusal(name: &str, pause_mode: Option<&PauseMode>) -> String {
    format!("{name} is {} and was not read while it ran; wake it to read what stands there", napped_word(pause_mode))
}

/// Why a napping workspace's server was not started: nothing here wakes a machine.
// agents-report.ts:427
// export const nappingToolsRefusal = (name: string, pauseMode?: PauseMode): string => `${name} is ${nappedWord(pauseMode)}, and a server is started there only while it runs; wake it to list the tools`;
pub fn napping_tools_refusal(name: &str, pause_mode: Option<&PauseMode>) -> String {
    format!("{name} is {}, and a server is started there only while it runs; wake it to list the tools", napped_word(pause_mode))
}

/// Why a computer that runs every line as root refused to read: the lines would run as root in somebody's home.
// agents-report.ts:430
// export const noRunuserRefusal = (user: string): string => `this computer runs wsp as root and has no runuser to run as ${user}, the owner of the home, so nothing was read`;
pub fn no_runuser_refusal(user: &str) -> String {
    format!("this computer runs wsp as root and has no runuser to run as {user}, the owner of the home, so nothing was read")
}

/// Why nothing was sent to a computer whose login PATH, which every line there carries, leaves no room in one frame
/// of the daemon's for the line itself.
// agents-report.ts:434
// export const loginPathRefusal = (chars: number): string =>
//   `this computer's login PATH is ${chars.toLocaleString("en-US")} characters, which leaves no room for a command in one frame of the daemon's ${EXEC_BODY_MAX.toLocaleString("en-US")}, so nothing was sent; shorten it in the login's shell startup files`;
pub fn login_path_refusal(chars: f64) -> String {
    format!(
        "this computer's login PATH is {} characters, which leaves no room for a command in one frame of the daemon's {}, so nothing was sent; shorten it in the login's shell startup files",
        locale_number(chars, Grouping::EnUs, 0, 3),
        locale_number(EXEC_BODY_MAX as f64, Grouping::EnUs, 0, 3)
    )
}

/// Why a computer that runs every line as root refused to read: the home it names is not there, so there is no
/// owner to hand the lines to and running them as root is not an answer.
// agents-report.ts:439
// export const noHomeRefusal = (home: string): string => `this computer runs wsp as root and its home ${home} is not there, so nothing was read`;
pub fn no_home_refusal(home: &str) -> String {
    format!("this computer runs wsp as root and its home {home} is not there, so nothing was read")
}

// Where one sign-in started from the app stands, pushed to the socket that started it and no other: `running` until
// the tool prints a page, `waiting` once it has, with that page, the code the tool printed beside it where it prints
// one, and `paste` where what the page hands back is typed into the tool; then `signed-in`, or `failed` with the
// tool's own last words. A sign-in run in the person's own terminal names the pty it runs in there, `ptyId`, which
// the page attaches to and types into.
// agents-report.ts:446
// in wsp_wire::types::agents_report: AgentsSignInEvent

// agents-report.ts:456
// in wsp_wire::types::agents_report: AgentsSignInEvent

// One sign-in the host runs, or ended within SIGN_IN_ENDED_KEPT_MS, as agents.signIns lists it: what it signs in and
// where, for a server the row it was started from (its scope, and the project's id for a project's), and the last
// step it took, which a window that missed its steps draws.
// agents-report.ts:461
// in wsp_wire::types::agents_report: AgentsSignInRun

// agents-report.ts:471
// in wsp_wire::types::agents_report: AgentsSignInRun

// How long the host keeps an ended sign-in's last step for a window whose socket dropped before it heard it.
// agents-report.ts:474
// in wsp_wire::types::agents_report: SIGN_IN_ENDED_KEPT_MS

// Something written changed what a report there reads: a sign-in, a key in the vault, the wsp tools in a config.
// No target: every report, which is what a key in this host's vault changes.
// agents-report.ts:478
// in wsp_wire::types::agents_report: AgentsChangedEvent

// agents-report.ts:479
// in wsp_wire::types::agents_report: AgentsChangedEvent

// One sign-in as a line on the computer it runs on, for the person's own terminal: what runs first (the store's
// folder, where the login lives outside the home), the command, the environment it runs with, and the tool's own
// status line afterwards. Paths and commands only, never a value.
// agents-report.ts:484
// in wsp_wire::types::agents_report: SignInLine

// agents-report.ts:485
// in wsp_wire::types::agents_report: SignInLine

/// A paste the agent's token check refused.
// agents-report.ts:488
// export const notTokenRefusal = (agent: string): string => `That is not a ${agent} token.`;
pub fn not_token_refusal(agent: &str) -> String {
    format!("That is not a {agent} token.")
}

/// Why an agent has no key for this host to keep.
// agents-report.ts:491
// export const noVaultKeyRefusal = (agent: string): string => `${agent} takes no token or key this host keeps.`;
pub fn no_vault_key_refusal(agent: &str) -> String {
    format!("{agent} takes no token or key this host keeps.")
}

/// Why an agent's sign-in is not run for the app: it asks the person to pick, so it runs in their terminal.
// agents-report.ts:494
// export const signInTerminalRefusal = (agent: string, line: string): string => `${agent} asks you to pick while it signs in, so it runs in your terminal: ${line}`;
pub fn sign_in_terminal_refusal(agent: &str, line: &str) -> String {
    format!("{agent} asks you to pick while it signs in, so it runs in your terminal: {line}")
}

/// Why an agent has no sign-in to run: it signs in with a token or key this host keeps, or not at all.
// agents-report.ts:497
// export const signInVaultRefusal = (agent: string): string => `${agent} has no sign-in to run on a computer; it reads a token or key this host keeps.`;
pub fn sign_in_vault_refusal(agent: &str) -> String {
    format!("{agent} has no sign-in to run on a computer; it reads a token or key this host keeps.")
}

/// A token made here that did not land. None of what the command printed is said, since the token may be in it.
// agents-report.ts:500
// export const mintFailedLine = (mint: string): string => `${mint} ended without a token wsp could read`;
pub fn mint_failed_line(mint: &str) -> String {
    format!("{mint} ended without a token wsp could read")
}

/// Why a server's sign-in is not run for the app, with the line the person runs instead.
// agents-report.ts:503
// export const serverSignInCopyRefusal = (agent: string, line: string, why: "inside" | "callback"): string =>
//   why === "inside" ? `${agent} signs a server in inside its own session: ${line}` : `${agent} finishes a server's sign-in on a page at localhost, which reaches only the computer your browser is on; run ${line} in a terminal there.`;
pub fn server_sign_in_copy_refusal(agent: &str, line: &str, why: /*"inside" | "callback"*/ &str) -> String {
    if why == "inside" {
        format!("{agent} signs a server in inside its own session: {line}")
    } else {
        format!("{agent} finishes a server's sign-in on a page at localhost, which reaches only the computer your browser is on; run {line} in a terminal there.")
    }
}

/// A project's server the agent leaves out, since it reads that project's file only in a folder it trusts.
// agents-report.ts:507
// export const serverUntrustedLine = (agent: string, name: string, file: string, folder: string): string =>
//   `${agent} reads ${file} only in a folder it trusts, and ${folder} is not one, so ${name} is not set up for ${agent} there. Trust that folder in ${agent} to use it.`;
pub fn server_untrusted_line(agent: &str, name: &str, file: &str, folder: &str) -> String {
    format!("{agent} reads {file} only in a folder it trusts, and {folder} is not one, so {name} is not set up for {agent} there. Trust that folder in {agent} to use it.")
}

/// A server the agent's own command, run with the config its threads read, says it does not have.
// agents-report.ts:511
// export const serverNotSetUpLine = (agent: string, name: string): string => `${name} is not set up for ${agent} where its threads read their config. Add it again with Add a tool server.`;
pub fn server_not_set_up_line(agent: &str, name: &str) -> String {
    format!("{name} is not set up for {agent} where its threads read their config. Add it again with Add a tool server.")
}

// agents-report.ts:514
// in wsp_wire::types::agents_report: addToolsHereRefusal

/// Why the wsp tools are not written for an agent whose MCP config wsp does not know.
// agents-report.ts:517
// export const addToolsNoConfigRefusal = (agent: string): string => `${agent} has no MCP config wsp knows, so the wsp tools were not written`;
pub fn add_tools_no_config_refusal(agent: &str) -> String {
    format!("{agent} has no MCP config wsp knows, so the wsp tools were not written")
}

/// A C0 control character, DEL or a C1 control character: what an interactive terminal acts on rather than shows, so a
/// name holding one is never a name wsp runs anything by.
// agents-report.ts:521
// export const hasControlChar = (s: string): boolean => /[\x00-\x1f\x7f-\x9f]/.test(s);
pub fn has_control_char(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}'))
}

/// The text with every control character taken out, for a name that reaches a log line.
// agents-report.ts:524
// export const withoutControlChars = (s: string): string => s.replace(/[\x00-\x1f\x7f-\x9f]/g, "");
pub fn without_control_chars(s: &str) -> String {
    s.chars().filter(|c| !matches!(*c, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}')).collect()
}

/// Why a server a config names was left out of the report.
// agents-report.ts:527
// export const controlNameRefusal = (file: string): string => `${file} names a server with a control character in its name, which was left out.`;
pub fn control_name_refusal(file: &str) -> String {
    format!("{file} names a server with a control character in its name, which was left out.")
}

// agents-report.ts:530
// in wsp_wire::types::agents_report: controlSignInRefusal

// agents-report.ts:533
// in wsp_wire::types::agents_report: noSignInRefusal

/// Why a napping workspace ran no sign-in: nothing here wakes a machine.
// agents-report.ts:536
// export const nappingSignInRefusal = (name: string, pauseMode?: PauseMode): string => `${name} is ${nappedWord(pauseMode)}, and a sign-in runs there only while it runs; wake it to sign in`;
pub fn napping_sign_in_refusal(name: &str, pause_mode: Option<&PauseMode>) -> String {
    format!("{name} is {}, and a sign-in runs there only while it runs; wake it to sign in", napped_word(pause_mode))
}

/// What a sign-in on a computer says while the tool its login runs goes on there first.
// agents-report.ts:539
// export const installingFirstLine = (name: string): string => `Installing ${name} first.`;
pub fn installing_first_line(name: &str) -> String {
    format!("Installing {name} first.")
}

/// A sign-in run in a terminal that the person left (Esc, Ctrl-C, quitting the tool) before it signed in.
// agents-report.ts:542
// export const closedBeforeSignInLine = (name: string): string => `${name} closed before it signed in.`;
pub fn closed_before_sign_in_line(name: &str) -> String {
    format!("{name} closed before it signed in.")
}

/// A sign-in run in a terminal that ended where its status could not be read after, a dropped link say: the person
/// may well have signed in, so it is not said they left.
// agents-report.ts:546
// export const signInUncheckedLine = (name: string): string => `${name} ended, but its sign-in could not be checked.`;
pub fn sign_in_unchecked_line(name: &str) -> String {
    format!("{name} ended, but its sign-in could not be checked.")
}

/// Why a sign-in on a computer did not start: the tool its login runs would not go on there, in that install's own last
/// words; the fix is the command line's, where the person reads it after `wsp add <computer> --sign-in`.
// agents-report.ts:550
// export const toolNotInstalledLine = (name: string, said: string): string => `${name} would not install there, so its sign-in did not start: ${said.replace(/\.$/, "")}.`;
pub fn tool_not_installed_line(name: &str, said: &str) -> String {
    format!("{name} would not install there, so its sign-in did not start: {}.", said.strip_suffix('.').unwrap_or(said))
}

// agents-report.ts:551
// export const toolNotInstalledFix = (name: string): string => `Put ${name} on there yourself, then sign in again.`;
pub fn tool_not_installed_fix(name: &str) -> String {
    format!("Put {name} on there yourself, then sign in again.")
}

// agents-report.ts:555
// in wsp_wire::types::agents_report: SIGN_IN_LINE_REFUSAL

// agents-report.ts:558
// in wsp_wire::types::agents_report: AGENTS_KEY_REFUSAL
