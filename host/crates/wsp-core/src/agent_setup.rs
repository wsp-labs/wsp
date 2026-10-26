// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/agent-setup.ts at 65f6b89c7: how each agent runs on each computer the person owns,
//! one document per computer and agent, held in memory. Spec: spec/agent_setups.md.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_adapters::claude::landmines::is_landmine;
use wsp_adapters::port::AgentLaunch;
use wsp_catalog::catalog::catalog_agents;
use wsp_store::store::Store;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::types::thread_defaults::{AgentSetup, AgentSetupSet, ResolvedFolder};
use wsp_words::exit::{HostError, Kind};

use crate::daemon_roots::posix_dirname;
use crate::protocol_pending::{agent_env_refusal, config_dir_refusal, no_config_dir_line, setup_patched};

/// The store collection. agent-setup.ts:14.
pub const AGENT_SETUPS: &str = "agentSetups";
/// How many links a path is followed through before it is a loop. agent-setup.ts:21.
pub const LINK_HOPS: u32 = 40;

/// `<path> goes round a loop of links`. agent-setup.ts:22.
pub fn loop_line(path: &str) -> String {
    format!("{path} goes round a loop of links")
}

/// The path with every link on its way followed, on this computer: the part that exists resolved, a link to a place
/// not made yet read for where it points, the rest kept as named. A usage refusal (`loop_line`) after more than
/// LINK_HOPS links. agent-setup.ts:27.
pub async fn real_folder_here(path: &str) -> Result<String, HostError> {
    let mut at = posix_normalize(path);
    let mut rest = String::new();
    let mut hops = 0;
    while hops <= LINK_HOPS {
        if let Ok(real) = tokio::fs::canonicalize(&at).await {
            let real = real.to_string_lossy().into_owned();
            return Ok(if rest.is_empty() { real } else { posix_normalize(&format!("{real}/{rest}")) });
        }
        if let Ok(target) = tokio::fs::read_link(&at).await {
            at = posix_resolve(&posix_dirname(&at), &target.to_string_lossy());
            hops += 1;
            continue;
        }
        if posix_dirname(&at) == at {
            return Ok(posix_normalize(path));
        }
        rest = if rest.is_empty() { posix_basename(&at) } else { posix_normalize(&format!("{}/{rest}", posix_basename(&at))) };
        at = posix_dirname(&at);
    }
    Err(usage(loop_line(path)))
}

/// The same reading on another computer, as one shell line: the resolved folder, then that computer's home resolved,
/// one a line; exit 3 with `loop_line` on stderr after LINK_HOPS links. agent-setup.ts:48.
pub fn real_folder_script(path: &str) -> String {
    [
        format!("p={}; rest=; n=0", shell_quote(&posix_normalize(path))),
        format!(
            "while [ ! -e \"$p\" ] && [ \"$p\" != / ]; do if [ -L \"$p\" ]; then n=$((n+1)); if [ \"$n\" -gt {LINK_HOPS} ]; then echo {} >&2; exit 3; fi; t=$(readlink \"$p\"); case $t in /*) p=$t ;; *) p=$(dirname \"$p\")/$t ;; esac; else rest=\"/$(basename \"$p\")$rest\"; p=$(dirname \"$p\"); fi; done",
            shell_quote(&loop_line(path))
        ),
        "d=$(cd \"$p\" 2>/dev/null && pwd -P) || d=$p".to_string(),
        "printf '%s%s\\n' \"${d%/}\" \"$rest\"".to_string(),
        "cd ~ && pwd -P".to_string(),
    ]
    .join("; ")
}

/// `<place_id>:<agent>`, the document's id. agent-setup.ts:57.
pub fn key_of(place_id: &str, agent: &str) -> String {
    format!("{place_id}:{agent}")
}

/// What a launch of one agent on one computer takes from its setup. agent-setup.ts:60.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetupLaunch {
    pub config_dir: Option<String>,
    /// The variables the launch carries, the config folder's own last for an agent that reads one.
    pub env: IndexMap<String, String>,
    pub launch: Option<AgentLaunch>,
}

/// What `set` asks of its caller: the config folder resolved on the computer it names, and the agent's name for a
/// refusal. agent-setup.ts:75.
pub struct SetupSetOptions {
    pub folder: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<ResolvedFolder, HostError>>>,
    pub agent_name: String,
}

/// Every setup, held in memory and written through to the store. Nothing is read until `load`. agent-setup.ts:78.
pub struct AgentSetups {
    store: Rc<dyn Store>,
    held: RefCell<IndexMap<String, AgentSetup>>,
}

impl AgentSetups {
    /// Reads every document of AGENT_SETUPS into memory. agent-setup.ts:82.
    pub async fn load(&self) -> Result<(), HostError> {
        for id in self.store.keys(AGENT_SETUPS).await? {
            let setup = self.store.get(AGENT_SETUPS, &id).await?.and_then(|v| serde_json::from_value::<AgentSetup>(v).ok());
            if let Some(setup) = setup {
                self.held.borrow_mut().insert(id, setup);
            }
        }
        Ok(())
    }

    pub fn get(&self, place_id: &str, agent: &str) -> Option<AgentSetup> {
        self.held.borrow().get(&key_of(place_id, agent)).cloned()
    }

    /// Whether the person turned the agent off on that computer.
    pub fn off(&self, place_id: &str, agent: &str) -> bool {
        self.held.borrow().get(&key_of(place_id, agent)).is_some_and(|setup| setup.on.is_some())
    }

    /// agent-setup.ts:87.
    pub fn launch_of(&self, place_id: &str, agent: &str) -> SetupLaunch {
        let setup = self.get(place_id, agent);
        let dropped = |name: &str| agent == "claude" && is_landmine(name);
        let launch = setup
            .as_ref()
            .filter(|s| s.program.is_some() || s.args.is_some())
            .map(|s| AgentLaunch { program: s.program.clone(), args: s.args.clone() });
        let config_dir = setup.as_ref().and_then(|s| s.config_dir.clone());
        let mut env: IndexMap<String, String> = setup
            .as_ref()
            .and_then(|s| s.env.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|(name, _)| agent_env_refusal(name, agent, Some(&dropped)).is_none())
            .collect();
        if let (Some(dir), Some(variable)) = (&config_dir, state_env_of(agent)) {
            env.insert(variable, dir.clone());
        }
        SetupLaunch { config_dir, env, launch }
    }

    /// The change over what stands, checked first and refused with a usage error; the store written before memory.
    /// agent-setup.ts:101.
    pub async fn set(
        &self,
        place_id: &str,
        agent: &str,
        change: AgentSetupSet,
        o: SetupSetOptions,
    ) -> Result<Option<AgentSetup>, HostError> {
        let key = key_of(place_id, agent);
        let mut patch = change.clone();
        if let Some(Some(dir)) = &change.config_dir {
            if state_env_of(agent).is_none() {
                return Err(usage(no_config_dir_line(&o.agent_name)));
            }
            if !dir.starts_with('/') {
                return Err(usage(format!("{}'s config folder must be an absolute path, and got {dir}", o.agent_name)));
            }
            let real = (o.folder)(dir.as_str()).await?;
            if let Some(refused) = config_dir_refusal(&o.agent_name, dir, &real) {
                return Err(usage(refused));
            }
            patch.config_dir = Some(Some(real.folder));
        }
        let dropped = |name: &str| agent == "claude" && is_landmine(name);
        for (name, value) in change.env.iter().flatten() {
            if value.is_some() {
                if let Some(refused) = agent_env_refusal(&name.0, &o.agent_name, Some(&dropped)) {
                    return Err(usage(refused));
                }
            }
        }
        let kept = self.get(place_id, agent);
        match setup_patched(kept.as_ref(), &patch) {
            None => {
                self.store.delete(AGENT_SETUPS, &key).await?;
                self.held.borrow_mut().shift_remove(&key);
                Ok(None)
            }
            Some(next) => {
                let value = serde_json::to_value(&next).map_err(|e| HostError::new(e.to_string()))?;
                self.store.put(AGENT_SETUPS, &key, value).await?;
                self.held.borrow_mut().insert(key, next.clone());
                Ok(Some(next))
            }
        }
    }
}

/// agent-setup.ts:78.
pub fn agent_setups(store: Rc<dyn Store>) -> AgentSetups {
    AgentSetups { store, held: RefCell::new(IndexMap::new()) }
}

fn usage(sentence: impl Into<String>) -> HostError {
    HostError::with_kind(sentence, Kind::Usage)
}

fn state_env_of(agent: &str) -> Option<String> {
    catalog_agents().into_iter().find(|a| a.id == agent).and_then(|a| a.state_home_env.clone())
}

/// Node's `path.posix.normalize`: `.` and empty segments fold, `..` pops where it can, a trailing slash stays.
fn posix_normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let absolute = path.starts_with('/');
    let trailing = path.ends_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => match parts.last() {
                Some(last) if *last != ".." => {
                    parts.pop();
                }
                _ if !absolute => parts.push(".."),
                _ => {}
            },
            kept => parts.push(kept),
        }
    }
    let mut out = parts.join("/");
    if out.is_empty() && !absolute {
        out.push('.');
    }
    if trailing && !out.is_empty() && !out.ends_with('/') {
        out.push('/');
    }
    if absolute {
        format!("/{out}")
    } else {
        out
    }
}

/// Node's `path.posix.resolve` of a base and a link: an absolute link stands alone, a relative one joins the base; no
/// trailing slash is kept but on the root.
fn posix_resolve(base: &str, link: &str) -> String {
    let joined = if link.starts_with('/') { posix_normalize(link) } else { posix_normalize(&format!("{base}/{link}")) };
    if joined.len() > 1 {
        joined.trim_end_matches('/').to_string()
    } else {
        joined
    }
}

fn posix_basename(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}
