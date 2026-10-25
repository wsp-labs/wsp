// SPDX-License-Identifier: AGPL-3.0-only
//! The slice of `packages/catalog` the Agents pages read that `ui::catalog` does not, written out of its built module:
//! what each agent is and where it keeps tool servers and skills.

/// One agent the catalog knows, in the catalog's order.
pub struct AgentEntry {
    pub id: &'static str,
    pub bin: &'static str,
    pub description: &'static str,
    pub mcp: Option<Mcp>,
    /// Whether it keeps a skills folder of its own, in the home and in a project.
    pub own_skills: (bool, bool),
}

/// Where an agent keeps tool servers, whether wsp turns one off there, and how its harness signs one in.
pub struct Mcp {
    pub files: &'static [&'static str],
    pub project_files: &'static [&'static str],
    pub switch: bool,
    pub login: Option<McpLogin>,
}

/// `McpLogin`: a command run with the server's quoted name after it, and the words that make it print its page and take
/// back the address; or the line typed inside the harness's own session, the name after it.
pub enum McpLogin {
    Command { command: &'static str, pasted: Option<&'static str> },
    Inside(&'static str),
}

/// How one server is signed in: the line run with the line that prints its page and takes the address back, or the line
/// typed inside the harness's own session.
#[derive(Debug, PartialEq)]
pub enum LoginLine {
    Run { line: String, pasted: Option<String> },
    Inside(String),
}

impl McpLogin {
    /// The lines for the server `name` (`command`, `pasted`, `inside` of the catalog's `McpLogin`).
    pub fn lines(&self, name: &str) -> LoginLine {
        match self {
            McpLogin::Inside(line) => LoginLine::Inside(format!("{line} {name}")),
            McpLogin::Command { command, pasted } => {
                let name = shell_quote(name);
                LoginLine::Run { line: format!("{command} {name}"), pasted: pasted.map(|p| format!("{command} {name} {p}")) }
            }
        }
    }
}

/// `shellQuote`: one word in single quotes, a quote inside it closed and escaped.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// `isSystemSkill`: the skills wsp writes and rewrites itself (`WSP_SKILL_NAME`, `SKILL_NAME`), never the person's to
/// turn off or remove.
pub fn is_system_skill(name: &str) -> bool {
    name == "wsp" || name == "wsp-machine"
}

pub const AGENTS: &[AgentEntry] = &[
    AgentEntry {
        id: "claude",
        bin: "claude",
        description: "Anthropic's coding agent for the terminal. It reads the codebase, edits files, runs commands and handles git.",
        mcp: Some(Mcp { files: &["~/.claude.json"], project_files: &[".mcp.json"], switch: false, login: Some(McpLogin::Command { command: "claude mcp login", pasted: Some("--no-browser") }) }),
        own_skills: (true, true),
    },
    AgentEntry {
        id: "codex",
        bin: "codex",
        description: "OpenAI's coding agent that runs locally in the terminal.",
        mcp: Some(Mcp { files: &["~/.codex/config.toml"], project_files: &[".codex/config.toml"], switch: true, login: Some(McpLogin::Command { command: "codex mcp login", pasted: None }) }),
        own_skills: (false, false),
    },
    AgentEntry {
        id: "gemini",
        bin: "gemini",
        description: "An open source agent that brings Gemini into the terminal.",
        mcp: Some(Mcp { files: &["~/.gemini/settings.json"], project_files: &[".gemini/settings.json"], switch: true, login: Some(McpLogin::Inside("/mcp auth")) }),
        own_skills: (true, true),
    },
    AgentEntry {
        id: "opencode",
        bin: "opencode",
        description: "The open source coding agent for the terminal.",
        mcp: Some(Mcp { files: &["~/.config/opencode/opencode.json","~/.config/opencode/opencode.jsonc"], project_files: &["opencode.json","opencode.jsonc"], switch: true, login: Some(McpLogin::Command { command: "opencode mcp auth", pasted: None }) }),
        own_skills: (false, false),
    },
    AgentEntry {
        id: "pi",
        bin: "pi",
        description: "A small coding agent for the terminal with read, bash, edit and write tools and saved sessions.",
        mcp: None,
        own_skills: (false, false),
    },
    AgentEntry {
        id: "hermes",
        bin: "hermes",
        description: "Nous Research's agent that keeps memories and skills and grows with use.",
        mcp: None,
        own_skills: (true, false),
    },
    AgentEntry {
        id: "crush",
        bin: "crush",
        description: "Charm's coding agent for the terminal. It works with the model of your choice and your own servers, language servers and skills.",
        mcp: None,
        own_skills: (false, false),
    },
    AgentEntry {
        id: "qwen",
        bin: "qwen",
        description: "An open source coding agent for the terminal from the Qwen team.",
        mcp: None,
        own_skills: (true, true),
    },
    AgentEntry {
        id: "goose",
        bin: "goose",
        description: "An open source, extensible agent that installs, runs, edits and tests code with any model.",
        mcp: None,
        own_skills: (false, false),
    },
    AgentEntry {
        id: "amp",
        bin: "amp",
        description: "The command line for Amp, a coding agent and development environment.",
        mcp: None,
        own_skills: (false, false),
    },
    AgentEntry {
        id: "cursor",
        bin: "cursor-agent",
        description: "The command line agent from the makers of the Cursor editor.",
        mcp: None,
        own_skills: (false, false),
    },
];

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use serde_json::{json, Value};

    use super::super::model::{agent_of_row, env_refusal, model_id_refusal, PROCESS_ENV, PROCESS_ENV_PREFIXES};
    use super::*;

    /// What `tests/catalog.mjs` reads off the built catalog and protocol. Needs node and the packages built (pnpm build).
    fn read() -> Value {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let packages = here.join("../../../packages").canonicalize().unwrap();
        let built = |name: &str| {
            let module = packages.join(name).join("dist/index.js");
            assert!(module.exists(), "{} is not built: run pnpm install && pnpm build", module.display());
            format!("file://{}", module.display())
        };
        let out = Command::new("node")
            .arg(here.join("tests/catalog.mjs"))
            .args([built("catalog"), built("protocol")])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output()
            .expect("node on PATH");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    }

    fn login_of(login: Option<&McpLogin>) -> Value {
        let Some(login) = login else { return Value::Null };
        ["notion", "my server", "o'k"]
            .iter()
            .map(|name| match login.lines(name) {
                LoginLine::Inside(line) => json!({ "name": name, "inside": line, "line": null, "pasted": null }),
                LoginLine::Run { line, pasted } => json!({ "name": name, "inside": null, "line": line, "pasted": pasted }),
            })
            .collect()
    }

    fn agent_of(a: &AgentEntry) -> Value {
        json!({
            "id": a.id,
            "name": crate::ui::catalog::name(a.id),
            "bin": a.bin,
            "planBrand": crate::ui::catalog::plan_brand(a.id),
            "description": a.description,
            "mint": crate::ui::catalog::sign_in(a.id).and_then(|s| s.mint),
            "mcp": a.mcp.as_ref().map(|m| json!({
                "files": m.files,
                "projectFiles": m.project_files,
                "switch": m.switch,
                "login": login_of(m.login.as_ref()),
            })),
            "ownSkills": [a.own_skills.0, a.own_skills.1],
        })
    }

    /// Each pair the catalog answered for an input, against what `ours` answers for it.
    fn each(read: &Value, key: &str, ours: impl Fn(&str) -> Value) {
        for pair in read[key].as_array().unwrap() {
            let input = pair[0].as_str().unwrap();
            assert_eq!(ours(input), pair[1], "{key}({input:?}) differs from the catalog's");
        }
    }

    #[test]
    fn the_copies_are_what_the_catalog_and_the_protocol_say_now() {
        let read = read();
        let theirs = read["agents"].as_array().unwrap();
        let ids = |list: &[Value]| list.iter().map(|a| a["id"].as_str().unwrap_or_default().to_owned()).collect::<Vec<_>>();
        let ours: Vec<Value> = AGENTS.iter().map(agent_of).collect();
        assert_eq!(ids(&ours), ids(theirs), "the agents, in the catalog's order");
        for (ours, theirs) in ours.iter().zip(theirs) {
            for (field, value) in theirs.as_object().unwrap() {
                assert_eq!(&ours[field], value, "{}'s {field} differs from the catalog's", theirs["id"]);
            }
        }
        each(&read, "systemSkill", |name| json!(is_system_skill(name)));
        each(&read, "agentOfRow", |id| json!(agent_of_row(id)));
        each(&read, "modelId", |id| json!(model_id_refusal(id)));
        each(&read, "env", |name| json!(env_refusal(name, "Agent")));
        assert_eq!(json!(PROCESS_ENV), read["processEnv"], "PROCESS_ENV");
        assert_eq!(json!(PROCESS_ENV_PREFIXES), read["processEnvPrefixes"], "PROCESS_ENV_PREFIXES");
    }
}
