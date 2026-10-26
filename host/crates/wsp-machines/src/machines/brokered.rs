// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/brokered.ts` at 65f6b89c7: what a turn and a terminal carry on a machine whose
//! credentials the host keeps. Pure. Spec: spec/brokered.md.

use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_adapters::claude::landmines::{brokered_sign_in_env, BrokeredSignIn};
use wsp_adapters::codex::command::{
    brokered_codex_env, codex_for_shell, AccountModels, BrokerMode, CodexBroker,
};
use wsp_adapters::pending::CodexClaims;
use wsp_adapters::port::{
    AttachRunOptions, Attached, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory,
};
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::signin::key_env_of;
use wsp_core::adapters::{secrets_of, BROKERED_AGENTS};
use wsp_core::project_landing::{brokered_git_env, GitEnvRepo};
use wsp_wire::types::daemon_contract::BROKER_SOCKET_PATH;
use wsp_wire::types::env::HOST_TOKEN_ENV;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::runtime::{BrokerStatus, BrokerStatusClaude, BrokerStatusCodex};
use wsp_words::exit::HostError;
use wsp_words::place_name::brokered_agent_refusal;

/// The folder gh reads its config from on such a machine (brokered.ts:150).
pub const BROKERED_GH_DIR: &str = "/run/wsp-daemon/gh";

/// The folder of the agents' programs a terminal finds, one folder per agent (brokered.ts:158).
pub const BROKERED_BIN_DIR: &str = "/usr/local/libexec/wsp-terminal";

/// The variable only a terminal's shell carries, naming its thread's agent (brokered.ts:161).
pub const TERMINAL_ENV: &str = "WSP_TERMINAL";

/// The profile file that puts that agent's folder first (brokered.ts:166).
pub const TERMINAL_PROFILE: &str = "/etc/profile.d/wsp-terminal.sh";

/// The folder a terminal whose thread runs `own` finds the agents' programs in (brokered.ts:163). Unquoted.
pub fn terminal_bin_of(own: &str) -> String {
    format!("{BROKERED_BIN_DIR}/{own}")
}

/// The name the catalog gives an agent, or the id where the catalog has no such agent (brokered.ts:168).
fn name_of(id: &str) -> String {
    catalog_agents()
        .into_iter()
        .find(|agent| agent.id == id)
        .map(|agent| agent.name.clone())
        .unwrap_or_else(|| id.to_string())
}

/// The gh config line the launch text writes before whatever it runs (brokered.ts:19).
fn gh_door_line() -> String {
    let socket = shell_quote(&format!("http_unix_socket: {BROKER_SOCKET_PATH}"));
    format!("mkdir -p {BROKERED_GH_DIR} && printf '%s\\n' {socket} > {BROKERED_GH_DIR}/config.yml")
}

/// The program a terminal finds as `other`'s command where its thread runs `own` (brokered.ts:172).
pub fn other_agent_program(other: &str, own: &str) -> String {
    let said = format!(
        "wsp: {} cannot sign in on this machine. This terminal's thread runs {}, and your computer signs in only that agent here. Start a thread on {} to use it.",
        name_of(other),
        name_of(own),
        name_of(other)
    );
    [
        "#!/bin/sh".to_string(),
        format!("printf '%s\\n' {} >&2", shell_quote(&said)),
        "exit 1".to_string(),
        String::new(),
    ]
    .join("\n")
}

/// The line a terminal's open runs on such a machine (brokered.ts:181), spec rules 5 and 6.
pub fn terminal_line(own: &str, own_program: Option<&str>) -> String {
    let put = |path: &str, text: &str, mode: &str| {
        format!(
            "printf %s {} > {path}.$$ && chmod {mode} {path}.$$ && mv -f {path}.$$ {path}",
            shell_quote(text)
        )
    };
    let folder = terminal_bin_of(own);
    let programs = catalog_agents()
        .into_iter()
        .filter(|agent| BROKERED_AGENTS.contains(&agent.id.as_str()))
        .map(|agent| {
            let file = format!("{folder}/{}", agent.bin);
            if agent.id != own {
                put(&file, &other_agent_program(&agent.id, own), "755")
            } else if let Some(program) = own_program {
                put(&file, program, "755")
            } else {
                format!("rm -f {file}")
            }
        });
    let named = format!("{BROKERED_BIN_DIR}/\"${TERMINAL_ENV}\"");
    let profile =
        format!("[ -z \"${{{TERMINAL_ENV}:-}}\" ] || [ ! -d {named} ] || PATH={named}:$PATH\n");
    let mut steps = vec![format!("mkdir -p {folder}")];
    steps.extend(programs);
    steps.push(put(TERMINAL_PROFILE, &profile, "644"));
    steps.join(" && ")
}

/// The repo of record as `brokeredEnv` reads it: `Pick<BranchOfRecord, "owner" | "name" | "public">`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoOfRecord {
    pub owner: String,
    pub name: String,
    pub public: bool,
}

/// Every turn's environment on such a machine (brokered.ts:198), in the TS key order.
pub fn brokered_env(
    token: &str,
    repo: Option<&RepoOfRecord>,
    direct: &[String],
) -> IndexMap<String, String> {
    let mut env = IndexMap::new();
    env.insert(HOST_TOKEN_ENV.to_string(), token.to_string());
    env.insert("GH_CONFIG_DIR".to_string(), BROKERED_GH_DIR.to_string());
    env.insert("GH_TOKEN".to_string(), token.to_string());
    env.insert("GH_TELEMETRY".to_string(), "false".to_string());
    let repo = repo.map(|r| GitEnvRepo {
        owner: r.owner.clone(),
        name: r.name.clone(),
        public: r.public,
    });
    env.extend(brokered_git_env(token, repo.as_ref(), direct));
    env.insert(
        "CARGO_NET_GIT_FETCH_WITH_CLI".to_string(),
        "true".to_string(),
    );
    env
}

/// The kind of credential the host's broker adds for Claude Code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaudePlaceholderKind {
    Token,
    Key,
    Gateway,
}

/// The sign-in a turn's agent is handed on such a machine (brokered.ts:215).
#[derive(Clone, Debug, PartialEq)]
pub struct Placeholders {
    pub claude_token: String,
    pub claude_kind: ClaudePlaceholderKind,
    pub vault: IndexMap<String, String>,
}

/// The variable Codex reads its key from, as the catalog names it (brokered.ts:215).
fn codex_key_env() -> Option<String> {
    catalog_agents()
        .into_iter()
        .find(|agent| agent.id == "codex")
        .and_then(|agent| key_env_of(&agent.sign_in))
        .map(str::to_string)
}

/// `placeholdersOf` (brokered.ts:215), spec rule 8.
pub fn placeholders_of(status: Option<&BrokerStatus>, token: &str) -> Placeholders {
    let claude_kind = match status.and_then(|s| s.claude.as_ref()) {
        Some(BrokerStatusClaude::Key) => ClaudePlaceholderKind::Key,
        Some(BrokerStatusClaude::Gateway) => ClaudePlaceholderKind::Gateway,
        _ => ClaudePlaceholderKind::Token,
    };
    let mut vault = IndexMap::new();
    if matches!(
        status.and_then(|s| s.codex.as_ref()),
        Some(BrokerStatusCodex::Key)
    ) {
        if let Some(name) = codex_key_env() {
            vault.insert(name, format!("sk-wsp-{token}"));
        }
    }
    Placeholders {
        claude_token: token.to_string(),
        claude_kind,
        vault,
    }
}

/// A launch on such a machine as the host reads it for an agent (brokered.ts:222 `LaunchRead`'s answer).
#[derive(Clone)]
pub struct LaunchRead {
    pub vault: IndexMap<String, String>,
    pub claude: Option<BrokeredSignIn>,
    pub codex: Option<CodexBroker>,
}

/// What an agent signs in with in a terminal's shell (brokered.ts:228).
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalSignIn {
    pub env: IndexMap<String, String>,
    pub program: Option<String>,
}

/// `terminalSignIn` (brokered.ts:243), spec rule 9. `now_ms` is the clock's, which wsp-adapters' `codex_for_shell`
/// takes; an adapter refusal is answered as `Err` with its words.
pub fn terminal_sign_in(
    own: &str,
    read: &dyn Fn(Option<&str>) -> Option<LaunchRead>,
    token: &str,
    now_ms: i64,
) -> Result<Option<TerminalSignIn>, String> {
    match own {
        "claude" => Ok(read(Some(token))
            .and_then(|launch| launch.claude)
            .map(|claude| TerminalSignIn {
                env: brokered_sign_in_env(&claude),
                program: None,
            })),
        "codex" => {
            let Some(codex) = read(None).and_then(|launch| launch.codex) else {
                return Ok(None);
            };
            let vault = read(Some(token))
                .map(|launch| launch.vault)
                .unwrap_or_default();
            let api_key = secrets_of(&vault, "codex", false).api_key;
            let env = brokered_codex_env(&codex, api_key.as_deref())?;
            let program = codex_for_shell(&codex, &terminal_bin_of("codex"), now_ms)?;
            Ok(Some(TerminalSignIn {
                env,
                program: Some(program),
            }))
        }
        _ => Ok(None),
    }
}

/// `codexBrokerOf` (brokered.ts:249), spec rule 10.
pub fn codex_broker_of(
    status: Option<&BrokerStatus>,
    claims: Option<&CodexClaims>,
    models: Option<AccountModels>,
) -> Option<CodexBroker> {
    match (status.and_then(|s| s.codex.as_ref()), claims) {
        (Some(BrokerStatusCodex::Key), _) => Some(CodexBroker {
            mode: BrokerMode::Apikey,
            providers: None,
        }),
        (Some(_), Some(claims)) => Some(CodexBroker {
            mode: BrokerMode::Chatgpt {
                claims: claims.clone(),
                models,
            },
            providers: None,
        }),
        _ => None,
    }
}

/// `unbrokeredRefusal` (brokered.ts:255), spec rule 11: kind `usage`.
pub fn unbrokered_refusal(harness: &str) -> Option<HostError> {
    if BROKERED_AGENTS.contains(&harness) {
        return None;
    }
    let roads: Vec<String> = BROKERED_AGENTS.iter().map(|id| name_of(id)).collect();
    Some(brokered_agent_refusal(&name_of(harness), &roads))
}

/// `withGhDoor` (brokered.ts:261), spec rule 12.
#[allow(clippy::arc_with_non_send_sync)]
pub fn with_gh_door(exec: Arc<dyn ExecStreamFactory>) -> Arc<dyn ExecStreamFactory> {
    Arc::new(GhDoor { exec })
}

/// The road `with_gh_door` wraps: it writes gh's config ahead of a launch that points gh at that config.
struct GhDoor {
    exec: Arc<dyn ExecStreamFactory>,
}

impl ExecStreamFactory for GhDoor {
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        if options.env.get("GH_CONFIG_DIR").map(String::as_str) == Some(BROKERED_GH_DIR) {
            let wrapped = format!("{} && {command}", gh_door_line());
            return self.exec.launch(&wrapped, options);
        }
        self.exec.launch(command, options)
    }

    fn attach(
        &self,
        run: &str,
        options: AttachRunOptions,
    ) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        self.exec.attach(run, options)
    }

    fn sweep(
        &self,
        keep: &[String],
    ) -> Option<LocalBoxFuture<'static, Result<Vec<String>, ExecFailure>>> {
        self.exec.sweep(keep)
    }
}
