// SPDX-License-Identifier: AGPL-3.0-only
//! What `makeRuntime` of `cli/wiring.ts` hands `create_runtime` for a fresh home with no provider key: the no-provider
//! backend in its slot, this computer as the one local workspace, the store over `state.db` beside the state file, the
//! host's own place key, and the agents a turn runs. Every port that is `wsp-setup`'s (the agents reader and its acts,
//! the skills, servers and plugins roads, the icons, the recipes, the seed, the broker) is left unwired, and the
//! runtime answers each as the TS does for a road that is missing.
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use wsp_catalog::roads::{GOLDEN_SETUP, GOLDEN_SMOKE};
use wsp_cli::version::state_writer_here;
use wsp_collect::cache_names::{CACHE_DIRS, FINDER_METADATA};
use wsp_core::adapters::harness_adapters;
use wsp_core::host_id::{host_identity, hostname};
use wsp_core::local_exec::stream::{local_exec_stream, LocalExecOptions};
use wsp_core::machine_exec::factory::MachineExecOptions;
use wsp_core::types::wiring::{AgentsReach, GoldenRecipe, LocalWiring, PlaceBackends, Platform, RuntimeOptions};
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_engine::no_provider_backend::NoProviderBackend;
use wsp_engine::project_state::agent_home;
use wsp_engine::provider_slot::ProviderSlot;
use wsp_engine::vault::CacheRule;
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::AssembledRuntime;
use wsp_store::keys::{new_place_key_pair, PlaceKeyPair};
use wsp_store::own_file::write_own;
use wsp_store::sqlite_store::sqlite_store;
use wsp_wire::types::env::PERSON_HOME_ENV;
use wsp_wire::views::harness::McpServerSpec;
use wsp_words::words::thread::{NO_MACHINES_LINE, NO_PROVIDER_LINE};

use crate::envfile::vault_now;
use crate::paths::{basename, dirname, join};
use crate::state::Env;

/// What `wsp up` is wired with beside the state file.
pub struct Wiring {
    pub state_path: String,
    /// This computer's home: where the stores are and the folder work starts under.
    pub home: String,
    pub env: Env,
    /// The loopback address a turn here dials back, once the host has bound it.
    pub here_url: Option<String>,
    /// The wsp command an agent's config is given: this program, then the words before its verb.
    pub wsp: Vec<String>,
}

pub fn person_home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
}

/// `hostPlaceKeyPath`: beside the state file, so a second state file on one computer is a second wsp with a key of its own.
fn host_place_key_path(state_path: &str) -> String {
    join(&[&dirname(state_path), "place-host-key.json"])
}

/// `hostPlaceKey`: made on the first read and kept, at the person's own mode.
pub fn host_place_key(state_path: &str) -> PlaceKeyPair {
    let path = host_place_key_path(state_path);
    if let Some(held) = fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()) {
        if let (Some(public_key), Some(pem)) = (held["publicKey"].as_str(), held["privateKeyPem"].as_str()) {
            return PlaceKeyPair { public_key: public_key.to_string(), private_key_pem: pem.to_string() };
        }
    }
    let made = new_place_key_pair();
    let text = serde_json::to_string_pretty(&serde_json::json!({"publicKey": made.public_key, "privateKeyPem": made.private_key_pem}))
        .expect("a key pair writes as JSON");
    let _ = write_own(Path::new(&dirname(&path)), &basename(&path), format!("{text}\n"));
    made
}

/// `placeNameHere`: this computer's name lowercased, what a person would type for it.
fn place_name_here() -> String {
    hostname().to_lowercase()
}

/// `hostNameHere`: the name without the `.local` a Mac's mDNS name carries.
fn host_name_here() -> String {
    let name = hostname();
    match name.len().checked_sub(6) {
        Some(at) if name.is_char_boundary(at) && name[at..].eq_ignore_ascii_case(".local") => name[..at].to_string(),
        _ => name,
    }
}

/// `placeWiring(statePath)` as far as this binary holds it: the host's pair and this computer's own row. The installer,
/// the dialler and the rest of the places' roads are `wsp-setup`'s.
pub fn place_wiring(state_path: &str, home: &str) -> PlaceWiring {
    let home = home.to_string();
    PlaceWiring {
        host_key: host_place_key(state_path),
        provider: None,
        here: Rc::new(move || HerePlace {
            name: place_name_here(),
            os: Some("linux".to_string()),
            home: Some(home.clone()),
            ..Default::default()
        }),
        host_name: Rc::new(host_name_here),
        install: None,
        dial: None,
        log: None,
        update: None,
        leave: None,
        forget_key: None,
        sudo_over: None,
        provision: None,
        github_token: None,
        undo: None,
        run_over: None,
        back: None,
    }
}

/// The one-row table of a host wired to no provider: nothing is a place here, and only its own wired word answers.
struct NoPlaces {
    backend: Arc<dyn MachineBackend>,
}

impl PlaceBackends for NoPlaces {
    fn wired(&self) -> String {
        "none".to_string()
    }
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        (place == "none").then(|| self.backend.clone())
    }
    fn list(&self) -> Vec<String> {
        Vec::new()
    }
}

/// `localWorkFolder`: where every turn and every exec on this computer starts, never the person's own home.
pub fn local_work_folder(home: &str) -> String {
    join(&[home, "wsp-work"])
}

fn platform_here() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Darwin
    } else {
        Platform::Linux
    }
}

/// `localWiring(home, env, ..., statePath)` without the daemon: this computer as a workspace through the local
/// backend, a child process per turn, and the person's own login environment for every turn.
fn local_wiring(wiring: &Wiring) -> LocalWiring {
    let root = local_work_folder(&wiring.home);
    let run_dir = wsp_cli::host_lock::host_run_dir(&wiring.state_path);
    let person = wiring.env.get(PERSON_HOME_ENV).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| wiring.home.clone());
    let backend = Arc::new(LocalBackend::new(LocalBackendOptions { root: root.clone().into(), env: Some(wiring.env.clone()) }));
    let exec_root = root.clone();
    let home_env = wiring.env.clone();
    let home_person = person.clone();
    let env_all = wiring.env.clone();
    LocalWiring {
        backend,
        folder: root,
        exec_stream: Rc::new(move |opts: Option<MachineExecOptions>, waiting| {
            let mut local = LocalExecOptions::new(exec_root.clone(), run_dir.clone());
            if let Some(opts) = opts {
                local.idle_ms = opts.idle_ms;
                local.deadline_ms = opts.deadline_ms;
                local.now = opts.now;
                local.poll_ms = opts.poll_ms;
                local.reading = opts.reading;
            }
            local_exec_stream(local, waiting)
        }),
        home: Rc::new(move |id| agent_home(&home_person, id, Some(&home_env))),
        home_dir: wiring.home.clone(),
        roots_path: wsp_cli::host_lock::host_roots_path(&wiring.state_path),
        env: Rc::new(move || {
            let mut env = env_all.clone();
            env.insert("HOME".to_string(), person.clone());
            env
        }),
        daemon_road: None,
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: None,
        platform: platform_here(),
        close: None,
    }
}

fn mcp_server_spec(wiring: &Wiring) -> McpServerSpec {
    let (command, rest) = wiring.wsp.split_first().expect("a wsp command");
    let mut args: Vec<String> = rest.to_vec();
    args.extend(["mcp".to_string(), "--state".to_string(), wiring.state_path.clone()]);
    McpServerSpec { command: command.clone(), args, no_slate: None }
}

/// The words a host wired to no provider says every road that needs a machine in: how to join a computer, or how to
/// add a provider where one added can be the one this host forks on.
fn no_machines_line(cloud: bool) -> &'static str {
    if cloud {
        NO_PROVIDER_LINE
    } else {
        NO_MACHINES_LINE
    }
}

/// `makeRuntime(keys, statePath, ...)` for a host with no provider key.
pub fn make_runtime(wiring: &Wiring, cloud: bool) -> Rc<AssembledRuntime> {
    let line = no_machines_line(cloud);
    let slot = ProviderSlot::new(Arc::new(NoProviderBackend::new(Some(line.to_string()))));
    let backend = slot.backend();
    let argv: Vec<String> = std::env::args().collect();
    let exec = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| "/".to_string());
    let store = Rc::new(sqlite_store(Path::new(&wiring.state_path), state_writer_here(&argv, &exec, &cwd)));
    let state_path = wiring.state_path.clone();
    let places: Rc<dyn PlaceBackends> = Rc::new(NoPlaces { backend: backend.clone() });
    let adapters: IndexMap<String, _> = harness_adapters().into_iter().map(|(id, factory)| (id.to_string(), factory)).collect();
    let opts = RuntimeOptions {
        no_machines_line: Some(line.to_string()),
        cloud_projects: Some(cloud),
        places: Some(places),
        backend: Some(backend),
        agents: Some(AgentsReach { here_url: wiring.here_url.clone(), wsp_mcp: Some(mcp_server_spec(wiring)) }),
        local: Some(local_wiring(wiring)),
        place_links: Some(Rc::new(place_wiring(&wiring.state_path, &wiring.home))),
        store: Some(store),
        state_path: Some(wiring.state_path.clone()),
        adapters,
        vault: Some(Rc::new(move || vault_now(&state_path))),
        golden_recipe: Some(GoldenRecipe { setup: GOLDEN_SETUP.clone(), smoke: GOLDEN_SMOKE.to_string(), ..Default::default() }),
        host_id: Some(host_identity(None)),
        vault_caches: Some(cache_rule()),
        ..Default::default()
    };
    wsp_serve::create_runtime(opts)
}

/// `CACHE_RULE` of project-bundle.ts: the directory list by name, the Finder file, and the venv and checkout markers.
fn cache_rule() -> CacheRule {
    CacheRule {
        dirs: CACHE_DIRS.iter().map(|d| d.to_string()).collect(),
        files: vec![FINDER_METADATA.to_string()],
        markers: vec!["pyvenv.cfg".to_string(), ".git".to_string()],
    }
}
