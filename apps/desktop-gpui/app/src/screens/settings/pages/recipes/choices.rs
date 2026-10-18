// SPDX-License-Identifier: AGPL-3.0-only
//! What a recipe's page ticks from and how a tick moves the recipe (`add/choices.ts`), the words its lists say
//! (`add/PickLists.tsx`, `ADD_COMPUTER_WORDS`), and the weight table every size cell reads (`sizeTone`, `fmtCalls` in
//! `packages/protocol/src/words/units.ts`, the ink `lib/tone.ts` gives each tone).

use gpui_kit::Hsla;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::screens::settings::words::list_words;
use crate::ui::Palette;

/// `RecipeOptions`: what the computer running the host offers, read untyped off the wire.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RecipeOptions {
    #[serde(default)]
    pub agents: Vec<AgentOption>,
    #[serde(default)]
    pub mcp: Vec<ServerOption>,
    #[serde(default)]
    pub clis: Vec<CliOption>,
    #[serde(default)]
    pub skills: Vec<SkillOption>,
}

/// `kind` is how the agent's own sign-in works; `bytes` what it weighs where a build measured it.
#[derive(Clone, Debug, Deserialize)]
pub struct AgentOption {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub signins: Vec<String>,
    pub kind: Option<String>,
    pub bytes: Option<f64>,
}

/// `kind` is how the server signs in; `keys` what the values it carries go by.
#[derive(Clone, Debug, Deserialize)]
pub struct ServerOption {
    pub name: String,
    #[serde(default)]
    pub agents: Vec<String>,
    pub kind: Option<String>,
    pub keys: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CliOption {
    pub name: String,
    pub via: String,
    pub version: Option<String>,
    pub needs: Option<Vec<String>>,
    pub bytes: Option<f64>,
    pub calls: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SkillOption {
    pub name: String,
    pub from: String,
}

/// The tables of a recipe a row ticks, each keyed by the row's name (`TickKind` less folders).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TickKind {
    Agents,
    Mcp,
    Clis,
    Skills,
}

impl TickKind {
    fn key(self) -> &'static str {
        match self {
            TickKind::Agents => "agents",
            TickKind::Mcp => "mcp",
            TickKind::Clis => "clis",
            TickKind::Skills => "skills",
        }
    }
}

/// Whether the picks hold the row `name` of `kind`.
pub fn holds(picks: &Value, kind: TickKind, name: &str) -> bool {
    picks.get(kind.key()).and_then(|t| t.get(name)).is_some()
}

/// `tick`: one row ticked or not; ticked takes the row the options give for it, unticked takes it out.
pub fn tick(picks: &Value, kind: TickKind, name: &str, on: bool, options: &RecipeOptions) -> Value {
    let mut table = picks.get(kind.key()).and_then(Value::as_object).cloned().unwrap_or_default();
    if !on {
        table.remove(name);
    } else {
        let row = match kind {
            TickKind::Agents => {
                let signin = options.agents.iter().find(|a| a.id == name).and_then(|a| a.signins.first());
                signin.map_or_else(|| Value::Object(Map::new()), |s| serde_json::json!({ "signin": s }))
            }
            TickKind::Mcp => {
                let agents = options.mcp.iter().find(|s| s.name == name).map(|s| s.agents.clone()).unwrap_or_default();
                serde_json::json!({ "agents": agents })
            }
            TickKind::Clis => {
                let cli = options.clis.iter().find(|c| c.name == name);
                let mut row = serde_json::json!({ "via": cli.map_or("apt", |c| c.via.as_str()) });
                if let Some(needs) = cli.and_then(|c| c.needs.as_ref()) {
                    row["needs"] = serde_json::json!(needs);
                }
                row
            }
            TickKind::Skills => {
                serde_json::json!({ "from": options.skills.iter().find(|s| s.name == name).map_or("", |s| s.from.as_str()) })
            }
        };
        table.insert(name.to_owned(), row);
    }
    let mut next = picks.clone();
    next[kind.key()] = Value::Object(table);
    next
}

/// `tickMany`: several rows ticked or not at once, each as one tick would move it: a shift-click's range.
pub fn tick_many(picks: &Value, kind: TickKind, changes: &[(String, bool)], options: &RecipeOptions) -> Value {
    changes.iter().fold(picks.clone(), |next, (name, on)| tick(&next, kind, name, *on, options))
}

/// `copiesKeys`: whether the picks said yes to copying the ticked servers' keys.
pub fn copies_keys(picks: &Value) -> bool {
    picks.get("copyKeys") == Some(&Value::Bool(true))
}

/// `setCopyKeys`: the one answer about keys.
pub fn set_copy_keys(picks: &Value, on: bool) -> Value {
    let mut next = picks.clone();
    if let Some(table) = next.as_object_mut() {
        table.remove("copyKeys");
        if on {
            table.insert("copyKeys".to_owned(), Value::Bool(true));
        }
    }
    next
}

/// `keyedServers`: the ticked servers that carry a key, each with what its keys go by.
pub fn keyed_servers(picks: &Value, options: &RecipeOptions) -> Vec<(String, Vec<String>)> {
    options
        .mcp
        .iter()
        .filter(|s| holds(picks, TickKind::Mcp, &s.name))
        .filter_map(|s| s.keys.as_ref().filter(|k| !k.is_empty()).map(|k| (s.name.clone(), k.clone())))
        .collect()
}

/// How a picked agent signs in: its saved way, else the first the options offer, else copying the key.
pub fn agent_way(picks: &Value, agent: &AgentOption) -> String {
    picks
        .pointer(&format!("/agents/{}/signin", agent.id.replace('~', "~0").replace('/', "~1")))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| agent.signins.first().cloned())
        .unwrap_or_else(|| "vault".to_owned())
}

/// `signInWords`: what each way of signing in is called, for an agent that signs in the way `kind` names; a device
/// code is typed on a page, so signing in on the computer says so (`MACHINE_TAIL`). Token and key are
/// `signInWayLabel`'s.
pub fn way_words(way: &str, computer: &str, kind: Option<&str>) -> String {
    match way {
        "vault" => "Copy the key".to_owned(),
        "token" => "Make a token".to_owned(),
        "key" => "Use an API key".to_owned(),
        _ => {
            let on = if computer.is_empty() { "Sign in there".to_owned() } else { format!("Sign in on {computer}") };
            format!("{on}{}", if kind == Some("device") { " with a code" } else { "" })
        }
    }
}

/// `ADD_COMPUTER_WORDS.keysStay`: a ticked server whose keys the answer leaves here.
pub const KEYS_STAY: &str = "Skipped until you copy its keys.";

/// `ADD_COMPUTER_WORDS.serverSignIn`: how an MCP server signs in on the computer.
pub fn server_sign_in(kind: &str, computer: &str) -> Option<String> {
    Some(match kind {
        "none" => "No sign-in.".to_owned(),
        "key" => "Key copied.".to_owned(),
        "token" => "Token copied.".to_owned(),
        "oauth" if computer.is_empty() => "Signs in there.".to_owned(),
        "oauth" => format!("Signs in on {computer}."),
        _ => return None,
    })
}

/// A server's note on a recipe's page: its keys staying here, else how it signs in there.
pub fn server_note(picks: &Value, server: &ServerOption, computer: &str) -> Option<String> {
    let keyed = server.keys.as_ref().is_some_and(|k| !k.is_empty());
    if holds(picks, TickKind::Mcp, &server.name) && keyed && !copies_keys(picks) {
        return Some(KEYS_STAY.to_owned());
    }
    server.kind.as_deref().and_then(|kind| server_sign_in(kind, computer))
}

fn its_computers(computer: &str) -> &str {
    if computer.is_empty() {
        "its computers"
    } else {
        computer
    }
}

/// `ADD_COMPUTER_WORDS.copyKeys`, after `COPY_KEYS_WORD`.
pub fn copy_keys(computer: &str) -> String {
    format!("Copy the keys to {}", its_computers(computer))
}

/// `ADD_COMPUTER_WORDS.keysNamed`: each keyed server with what its keys go by.
pub fn keys_named(servers: &[(String, Vec<String>)]) -> String {
    let named: Vec<String> = servers.iter().map(|(name, keys)| format!("{name}: {}", keys.join(", "))).collect();
    format!("{}.", named.join("; "))
}

/// `ADD_COMPUTER_WORDS.leaveKeys`.
pub fn leave_keys(here: &str) -> String {
    format!("Leave them on {here}")
}

/// `ADD_COMPUTER_WORDS.keysLeft`.
pub fn keys_left(names: &[String], computer: &str) -> String {
    let one = names.len() == 1;
    format!(
        "{} {} skipped on {} until you copy {}.",
        list_words(names),
        if one { "is" } else { "are" },
        its_computers(computer),
        if one { "its keys" } else { "their keys" }
    )
}

/// A CLI's note: how it installs, and what it needs.
pub fn cli_note(cli: &CliOption) -> String {
    match &cli.needs {
        None => cli.via.clone(),
        Some(needs) => format!("{}. Needs {}.", cli.via, needs.join(", ")),
    }
}

/// The CLIs a recipe holds, the ones the agents ran most first, ties in the order offered.
pub fn ticked_clis<'a>(picks: &Value, options: &'a RecipeOptions) -> Vec<&'a CliOption> {
    let mut clis: Vec<&CliOption> = options.clis.iter().filter(|c| holds(picks, TickKind::Clis, &c.name)).collect();
    clis.sort_by(|a, b| b.calls.unwrap_or(0.).total_cmp(&a.calls.unwrap_or(0.)));
    clis
}

/// The skills a recipe's page shows of the ones it holds.
pub const FIRST_SKILLS: usize = 5;

/// The line under the first skills: how many more the recipe holds.
pub fn more_skills(n: usize) -> String {
    format!("{n} more skills on this recipe.")
}

/// `fmtCalls`: how often the agents ran a tool, grouped in thousands as en-US writes it.
pub fn fmt_calls(n: f64) -> String {
    let whole = n.round() as i64;
    let digits = whole.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (ix, ch) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sign = if whole < 0 { "-" } else { "" };
    format!("{sign}{grouped} {}", if whole == 1 { "call" } else { "calls" })
}

/// `SizeTone`: the tone a size is read in, by weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizeTone {
    Danger,
    Warning,
    Yellow,
    Muted,
}

const MIB: f64 = 1024. * 1024.;

/// `sizeTone`: a gigabyte and over the danger tone, 200 MB and over the warning tone, 50 MB and over the yellow tone,
/// anything under muted.
pub fn size_tone(bytes: f64) -> SizeTone {
    if bytes >= 1024. * MIB {
        SizeTone::Danger
    } else if bytes >= 200. * MIB {
        SizeTone::Warning
    } else if bytes >= 50. * MIB {
        SizeTone::Yellow
    } else {
        SizeTone::Muted
    }
}

impl SizeTone {
    /// `TONE_TEXT`: the ink each tone is read in.
    pub fn ink(self, p: &Palette) -> Hsla {
        match self {
            SizeTone::Danger => p.error_foreground,
            SizeTone::Warning => p.warning_foreground,
            SizeTone::Yellow => p.yellow_foreground,
            SizeTone::Muted => p.muted_foreground,
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;

    /// The options and the file the web's own values below were read off, with `node --experimental-strip-types`
    /// over `add/choices.ts` and `@wsp/protocol`.
    pub fn options() -> RecipeOptions {
        serde_json::from_value(json!({
            "agents": [{ "id": "claude", "name": "Claude Code", "signins": ["machine", "vault"], "kind": "browser" }, { "id": "codex", "name": "Codex", "signins": [], "kind": "device" }],
            "mcp": [{ "name": "docs", "agents": ["claude"], "kind": "oauth" }, { "name": "linear", "agents": ["claude", "codex"], "kind": "key", "keys": ["LINEAR_API_KEY"] }, { "name": "sentry", "agents": ["codex"], "keys": ["SENTRY_TOKEN", "SENTRY_ORG"] }],
            "clis": [{ "name": "pnpm", "via": "npm", "calls": 3 }, { "name": "gh", "via": "apt", "needs": ["git"], "version": "2.60.1", "calls": 1234, "bytes": 62914560 }],
            "skills": [{ "name": "unslop", "from": "~/.claude/skills/unslop", "linked": false }],
            "plugins": [], "configs": []
        }))
        .unwrap()
    }

    pub fn file() -> Value {
        json!({ "name": "Web app", "agents": { "claude": { "signin": "machine" } }, "mcp": { "docs": { "agents": ["claude"] } }, "clis": { "pnpm": { "via": "npm" } }, "skills": {}, "plugins": {}, "folders": {}, "configs": { "git": {} } })
    }

    fn with(key: &str, table: Value) -> Value {
        let mut next = file();
        next[key] = table;
        next
    }

    #[test]
    fn a_tick_writes_the_row_the_web_writes() {
        let (o, f) = (options(), file());
        let docs = json!({ "agents": ["claude"] });
        assert_eq!(
            tick(&f, TickKind::Agents, "codex", true, &o),
            with("agents", json!({ "claude": { "signin": "machine" }, "codex": {} }))
        );
        let bare = with("agents", json!({}));
        assert_eq!(tick(&bare, TickKind::Agents, "claude", true, &o), f, "the first way offered");
        let linear = json!({ "docs": docs, "linear": { "agents": ["claude", "codex"] } });
        assert_eq!(tick(&f, TickKind::Mcp, "linear", true, &o), with("mcp", linear));
        assert_eq!(tick(&f, TickKind::Mcp, "nope", true, &o), with("mcp", json!({ "docs": docs, "nope": { "agents": [] } })));
        let gh = json!({ "pnpm": { "via": "npm" }, "gh": { "via": "apt", "needs": ["git"] } });
        assert_eq!(tick(&f, TickKind::Clis, "gh", true, &o), with("clis", gh));
        assert_eq!(tick(&f, TickKind::Clis, "zz", true, &o), with("clis", json!({ "pnpm": { "via": "npm" }, "zz": { "via": "apt" } })));
        let skill = json!({ "unslop": { "from": "~/.claude/skills/unslop" } });
        assert_eq!(tick(&f, TickKind::Skills, "unslop", true, &o), with("skills", skill.clone()));
        assert_eq!(tick(&f, TickKind::Clis, "pnpm", false, &o), with("clis", json!({})));
        assert_eq!(
            tick(&json!({ "name": "x" }), TickKind::Skills, "unslop", true, &o),
            json!({ "name": "x", "skills": skill }),
            "a table not there yet"
        );
    }

    #[test]
    fn many_ticks_move_as_each_would() {
        let changes = [("docs".to_owned(), false), ("linear".to_owned(), true), ("sentry".to_owned(), true)];
        let want = with("mcp", json!({ "linear": { "agents": ["claude", "codex"] }, "sentry": { "agents": ["codex"] } }));
        assert_eq!(tick_many(&file(), TickKind::Mcp, &changes, &options()), want);
    }

    #[test]
    fn the_keys_answer_is_one_yes_or_nothing() {
        let mut yes = file();
        yes["copyKeys"] = json!(true);
        assert_eq!(set_copy_keys(&file(), true), yes);
        assert_eq!(set_copy_keys(&yes, false), file());
        assert_eq!((copies_keys(&file()), copies_keys(&yes)), (false, true));
        let keyed = tick_many(&file(), TickKind::Mcp, &[("linear".to_owned(), true), ("sentry".to_owned(), true)], &options());
        let want = vec![
            ("linear".to_owned(), vec!["LINEAR_API_KEY".to_owned()]),
            ("sentry".to_owned(), vec!["SENTRY_TOKEN".to_owned(), "SENTRY_ORG".to_owned()]),
        ];
        assert_eq!(keyed_servers(&keyed, &options()), want);
        assert_eq!(keyed_servers(&file(), &options()), vec![], "docs carries no key");
    }

    #[test]
    fn the_weight_table_and_the_counts_read_as_the_protocol_reads_them() {
        let at = [0., 50. * MIB - 1., 50. * MIB, 200. * MIB - 1., 200. * MIB, 1024. * MIB - 1., 1024. * MIB];
        use SizeTone::*;
        assert_eq!(at.map(size_tone), [Muted, Muted, Yellow, Yellow, Warning, Warning, Danger]);
        assert_eq!([0., 1., 2., 1234., 1234567.].map(fmt_calls), ["0 calls", "1 call", "2 calls", "1,234 calls", "1,234,567 calls"]);
    }

    #[test]
    fn the_lists_say_what_the_web_says() {
        let o = options();
        assert_eq!(way_words("vault", "mini", None), "Copy the key");
        assert_eq!(way_words("machine", "mini", Some("browser")), "Sign in on mini");
        assert_eq!(way_words("machine", "mini", Some("device")), "Sign in on mini with a code");
        assert_eq!(way_words("machine", "", None), "Sign in there");
        assert_eq!((way_words("token", "mini", None), way_words("key", "", None)), ("Make a token".into(), "Use an API key".into()));
        assert_eq!(agent_way(&file(), &o.agents[0]), "machine");
        assert_eq!(agent_way(&json!({ "agents": { "claude": {} } }), &o.agents[0]), "machine", "the first way offered");
        assert_eq!(agent_way(&file(), &o.agents[1]), "vault");
        assert_eq!(server_sign_in("oauth", "mini").as_deref(), Some("Signs in on mini."));
        assert_eq!(server_sign_in("oauth", "").as_deref(), Some("Signs in there."));
        assert_eq!(server_sign_in("none", "").as_deref(), Some("No sign-in."));
        let linear = with("mcp", json!({ "linear": { "agents": ["claude"] } }));
        assert_eq!(server_note(&linear, &o.mcp[1], "mini").as_deref(), Some(KEYS_STAY));
        assert_eq!(server_note(&set_copy_keys(&linear, true), &o.mcp[1], "mini").as_deref(), Some("Key copied."));
        assert_eq!(server_note(&file(), &o.mcp[2], "mini"), None, "a server whose sign-in nobody read says nothing");
        let keyed = keyed_servers(&tick(&file(), TickKind::Mcp, "sentry", true, &o), &o);
        assert_eq!(copy_keys(""), "Copy the keys to its computers");
        assert_eq!(copy_keys("mini"), "Copy the keys to mini");
        assert_eq!(keys_named(&keyed), "sentry: SENTRY_TOKEN, SENTRY_ORG.");
        assert_eq!(leave_keys("zingzy's MacBook"), "Leave them on zingzy's MacBook");
        assert_eq!(keys_left(&["sentry".into()], ""), "sentry is skipped on its computers until you copy its keys.");
        assert_eq!(keys_left(&["a".into(), "b".into()], "mini"), "a and b are skipped on mini until you copy their keys.");
        assert_eq!((cli_note(&o.clis[0]), cli_note(&o.clis[1])), ("npm".into(), "apt. Needs git.".into()));
        let both = tick(&file(), TickKind::Clis, "gh", true, &o);
        assert_eq!(ticked_clis(&both, &o).iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["gh", "pnpm"], "most called first");
        assert_eq!(more_skills(3), "3 more skills on this recipe.");
    }
}
