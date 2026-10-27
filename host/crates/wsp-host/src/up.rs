// SPDX-License-Identifier: AGPL-3.0-only
//! `wsp up`: the host served in this terminal (`upCommand` of `cli/commands.ts`, `up` of `cli/service.ts`, `hostFor` of
//! `cli/serve.ts`). The ports are settled first, then the lock is taken, the runtime built and the host bound, its
//! address lines said, and the process stays until a stop signal ends it.
use std::net::TcpListener as StdListener;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use tokio::net::TcpListener;
use tokio::signal::unix::{signal, SignalKind};
use wsp_cli::host_lock::{
    address_lines, host_token_path, lock_path_for, program_gone, refuse_if_served, release_lock, rewrite_lock, serving_host,
    started_by_env, take_lock, BoundAt, LockPorts,
};
use wsp_cli::verbs::client::CliIo;
use wsp_core::types::api::Runtime;
use wsp_serve::serve::LogFn;
use wsp_store::own_file::write_own;
use wsp_wire::types::app_ports::{LOOPBACK, PORT_TAKEN_REFUSAL};
use wsp_wire::types::release::HostShape;
use wsp_wire::types::JsNumber;
use wsp_wire::wire::capabilities::forks_no_machines;
use wsp_words::app_ports::{
    is_loopback, is_wildcard, listen_beyond_loopback_line, loopback_threads_line, port_instead_line, port_taken_line, ports_asked,
    ports_picked_line, AppPorts, PortHolder,
};
use wsp_words::exit::{usage_refusal, HostError};

use crate::envfile::{env_file_for, parse_env_file};
use crate::paths::{basename, dirname};
use crate::server::{advertise_word, start_host, HostOptions};
use crate::state::{state_path_from, Env};
use crate::wiring::{make_runtime, person_home, Wiring};

/// How often a serving host checks that its own program is still there.
const OWN_FILES_POLL_MS: u64 = 5_000;
/// `PORT_TRIES`: how many ports above the one asked for a pick may step, the offset the door is placed at.
const PORT_TRIES: u16 = 20;
/// The first thing a host with nothing in it says.
const NO_PROJECT_YET: &str = "no projects yet; wsp add <folder> records one here, and wsp add <url> --on <computer> records one there";
const ANTHROPIC_KEY: &str = "ANTHROPIC_API_KEY";

/// Everything a person asked a host to serve with.
pub struct UpAsked {
    pub port: f64,
    pub named: bool,
    pub address: String,
    pub advertise: Option<String>,
    pub state_path: String,
    pub relay: bool,
}

/// The shell's own code for a death by that signal.
fn exit_code_of(sig: i32) -> i32 {
    match sig {
        libc::SIGINT => 130,
        libc::SIGHUP => 129,
        _ => 143,
    }
}

/// Whether binding the port on loopback is refused as in use; any other outcome, port 0 included, reads as free.
fn port_in_use(port: u16) -> bool {
    port != 0 && StdListener::bind((LOOPBACK, port)).is_err()
}

fn holder_of(port: u16, state_path: &str) -> Option<PortHolder> {
    let served = serving_host(state_path).filter(|lock| lock.port.0 == f64::from(port))?;
    let _ = served;
    Some(PortHolder::Host { state_path: state_path.to_string() })
}

enum Chosen {
    Free { port: u16, moved: Option<(u16, Option<PortHolder>)> },
    Taken { port: u16, holder: Option<PortHolder> },
}

/// `choosePorts`: the port asked for when it is free; the next free one when a port nobody named is taken; the taken
/// port and its holder when a person named it or no port inside the window is free.
fn choose_ports(port: u16, named: bool, state_path: &str) -> Chosen {
    if !port_in_use(port) {
        return Chosen::Free { port, moved: None };
    }
    let holder = holder_of(port, state_path);
    if !named {
        for step in 1..PORT_TRIES {
            let Some(next) = port.checked_add(step) else { break };
            if !port_in_use(next) {
                return Chosen::Free { port: next, moved: Some((port, holder)) };
            }
        }
    }
    Chosen::Taken { port, holder }
}

/// `pickUpPorts`: the port this run binds, or none after saying who holds the one it was told to take.
fn pick_up_ports(io: &dyn CliIo, asked: &UpAsked) -> Option<(u16, Option<(u16, Option<PortHolder>)>)> {
    let port = asked.port as u16;
    match choose_ports(port, asked.named, &asked.state_path) {
        Chosen::Free { port, moved } => Some((port, moved)),
        Chosen::Taken { port, holder } => {
            io.error(&port_taken_line(f64::from(port), holder.as_ref()));
            match choose_ports(port, false, &asked.state_path) {
                Chosen::Free { port, .. } => io.error(&port_instead_line(&AppPorts { port })),
                Chosen::Taken { .. } => io.error(PORT_TAKEN_REFUSAL),
            }
            None
        }
    }
}

/// `hereUrl(bound, port)`: where a process on this computer dials a host bound there.
fn here_url(bound: &str, port: u16) -> Option<String> {
    if is_loopback(bound) {
        return Some(format!("http://{}", wsp_words::app_ports::authority(bound, f64::from(port))));
    }
    is_wildcard(bound).then(|| format!("http://{}", wsp_words::app_ports::authority(LOOPBACK, f64::from(port))))
}

/// Whether a key is in the environment, the folder's own `.env` or the one beside the state file.
fn claude_key_found(env: &Env, cwd: &str, state_path: &str) -> bool {
    let held = |file: IndexMapOf| file.get(ANTHROPIC_KEY).is_some_and(|v| !v.is_empty());
    held(env.clone()) || held(parse_env_file(&format!("{cwd}/.env"))) || held(parse_env_file(&env_file_for(state_path)))
}

type IndexMapOf = indexmap::IndexMap<String, String>;

/// `noClaudeKeyNote(forksNothing)`.
fn no_claude_key_note(forks_nothing: bool) -> String {
    let what = if forks_nothing {
        "a thread on this computer signs in as your own agents do"
    } else {
        "new machines fork without claude credentials"
    };
    format!("note: no ANTHROPIC_API_KEY found; {what}")
}

/// What `wsp up` parses: the shared flags that shape a serving host, and the ones every line takes.
pub struct UpFlags {
    pub port: Option<String>,
    pub listen: Option<String>,
    pub advertise: Option<String>,
    pub state: Option<String>,
    pub no_relay: bool,
}

pub fn asked_from(flags: &UpFlags, env: &Env, cwd: &str, note: &dyn Fn(&str)) -> UpAsked {
    let ports = ports_asked(flags.port.as_deref(), flags.listen.as_deref());
    UpAsked {
        port: ports.port,
        named: ports.named,
        address: ports.address,
        advertise: advertise_word(flags.advertise.as_deref()),
        state_path: state_path_from(flags.state.as_deref(), env, cwd, &person_home(), note),
        relay: !flags.no_relay,
    }
}

/// The signals a serving host stops on, held from the moment it starts so one that arrives while it is still coming
/// up is answered by a close and not by the default death.
struct Stops {
    term: tokio::signal::unix::Signal,
    int: tokio::signal::unix::Signal,
    hup: tokio::signal::unix::Signal,
}

impl Stops {
    fn held() -> Stops {
        Stops {
            term: signal(SignalKind::terminate()).expect("a SIGTERM handler"),
            int: signal(SignalKind::interrupt()).expect("a SIGINT handler"),
            hup: signal(SignalKind::hangup()).expect("a SIGHUP handler"),
        }
    }

    /// The next stop signal, as its number.
    async fn next(&mut self) -> i32 {
        tokio::select! {
            _ = self.term.recv() => libc::SIGTERM,
            _ = self.int.recv() => libc::SIGINT,
            _ = self.hup.recv() => libc::SIGHUP,
        }
    }
}

/// Waits for a stop signal, or for this host's own program to go: a host whose program has gone serves out of files it
/// can no longer read.
async fn until_stopped(stops: &mut Stops, io: &dyn CliIo, program: &str) {
    let mut poll = tokio::time::interval(Duration::from_millis(OWN_FILES_POLL_MS));
    poll.tick().await;
    loop {
        tokio::select! {
            _ = stops.next() => return,
            _ = poll.tick() => {
                if cfg!(target_os = "linux") && program_gone(program) {
                    io.error(&format!("{program}, which this host runs, is gone; it stops so the next start runs what the wsp command names now"));
                    return;
                }
            }
        }
    }
}

/// The host served until it is told to stop, and the exit code that ends the process.
pub async fn up(io: Arc<dyn CliIo>, asked: UpAsked, env: Env, wsp: Vec<String>) -> Result<i32, HostError> {
    let state_path = asked.state_path.clone();
    let lock_path = lock_path_for(&state_path);
    // The lock is read before a port is stepped or anything is written: a start that is going to be refused must leave
    // the home as it found it.
    refuse_if_served(&lock_path, &state_path)?;
    let Some((port, moved)) = pick_up_ports(io.as_ref(), &asked) else { return Ok(1) };
    let std_listener =
        StdListener::bind((asked.address.as_str(), port)).map_err(|e| HostError::new(format!("listen {}:{port}: {e}", asked.address)))?;
    std_listener.set_nonblocking(true).map_err(|e| HostError::new(e.to_string()))?;
    let listener = TcpListener::from_std(std_listener).map_err(|e| HostError::new(e.to_string()))?;
    let bound = listener.local_addr().map_err(|e| HostError::new(e.to_string()))?.port();
    let started = started_by_env(&env).or(Some(HostShape::Up));
    let lock = take_lock(
        &lock_path,
        &state_path,
        &LockPorts { port: JsNumber(f64::from(port)), address: Some(asked.address.clone()), started_by: started },
    )?;
    std::env::remove_var(wsp_cli::host_lock::STARTED_BY_ENV);
    let home = person_home();
    let wiring = Wiring { state_path: state_path.clone(), home, env: env.clone(), here_url: here_url(&asked.address, bound), wsp };
    let rt = match make_runtime(&wiring, wsp_cli::verbs::cloud_of(&env)) {
        Ok(rt) => rt,
        Err(e) => {
            let _ = release_lock(&lock_path);
            return Err(e);
        }
    };
    let said = io.clone();
    let log: LogFn = Rc::new(move |line: &str| said.log(line));
    let outcome = serve(io.as_ref(), &asked, &wiring, &rt, listener, moved, log, lock).await;
    let _ = release_lock(&lock_path);
    outcome
}

async fn serve(
    io: &dyn CliIo,
    asked: &UpAsked,
    wiring: &Wiring,
    rt: &Rc<wsp_serve::AssembledRuntime>,
    listener: TcpListener,
    moved: Option<(u16, Option<PortHolder>)>,
    log: LogFn,
    lock: wsp_cli::host_lock::HostLock,
) -> Result<i32, HostError> {
    let mut stops = Stops::held();
    let state_path = &asked.state_path;
    let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| "/".into());
    if serves_nothing(rt).await {
        io.log(NO_PROJECT_YET);
    }
    let joined = joined_a_computer(rt).await;
    let mut raw = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut raw);
    let auth_token = URL_SAFE_NO_PAD.encode(raw);
    let token_path = host_token_path(state_path);
    write_own(Path::new(&dirname(&token_path)), &basename(&token_path), &auth_token).map_err(|e| HostError::new(e.to_string()))?;
    let web_dir = std::env::var("WSP_WEB_DIR")
        .map_err(|_| HostError::new("WSP_WEB_DIR names the folder of the built web app; this build finds none beside itself"))?;
    let handle = match start_host(HostOptions {
        runtime: rt.clone(),
        wiring: wiring.clone(),
        web_dir,
        listener,
        address: asked.address.clone(),
        asked_port: asked.port as u16,
        advertise: asked.advertise.clone(),
        state_path: state_path.clone(),
        auth_token,
        version: wsp_cli::version::VERSION.to_string(),
        road: Some("up".to_string()),
        log,
        door_open: joined,
        product_usage_off: None,
    })
    .await
    {
        Ok(handle) => handle,
        Err(e) => {
            let _ = rt.close().await;
            return Err(e);
        }
    };
    let mut written = lock.clone();
    written.port = JsNumber(f64::from(handle.port));
    written.address = Some(asked.address.clone());
    let _ = rewrite_lock(&lock_path_for(state_path), &written);
    if let Some((taken, holder)) = &moved {
        io.log(&ports_picked_line(&AppPorts { port: handle.port }, f64::from(*taken), holder.as_ref()));
    }
    for line in address_lines(state_path, &BoundAt { port: JsNumber(f64::from(handle.port)), address: Some(asked.address.clone()) }, None) {
        io.log(&line);
    }
    if !is_loopback(&asked.address) {
        io.log(&listen_beyond_loopback_line(&asked.address));
    }
    if here_url(&asked.address, handle.port).is_none() {
        io.log(&loopback_threads_line(&asked.address));
    }
    if !claude_key_found(&env_of_process(), &cwd, state_path) {
        io.log(&no_claude_key_note(forks_no_machines(&rt.backend().capabilities())));
    }
    let program = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    until_stopped(&mut stops, io, &program).await;
    let closing = handle.close();
    tokio::pin!(closing);
    tokio::select! {
        done = &mut closing => match done {
            Ok(()) => Ok(0),
            Err(e) => {
                io.error(&format!("host close failed: {}", e.message));
                Ok(1)
            }
        },
        sig = stops.next() => Ok(exit_code_of(sig)),
    }
}

fn env_of_process() -> Env {
    std::env::vars().collect()
}

/// `servesNothing`: no image yet and no workspace. A start that reads either through a body that is not in yet reads
/// as serving something.
async fn serves_nothing(rt: &Rc<wsp_serve::AssembledRuntime>) -> bool {
    let rt = rt.clone();
    tokio::task::spawn_local(async move {
        let golden = rt.golden().get(None).await.ok().flatten();
        let held = rt.workspaces().list(None).await.map(|all| all.len()).unwrap_or(1);
        wsp_wire::views::golden_image::holds_nothing(golden.as_ref(), held)
    })
    .await
    .unwrap_or(false)
}

/// A computer that already joined dials the port its place file names, so the door binds as this host starts.
async fn joined_a_computer(rt: &Rc<wsp_serve::AssembledRuntime>) -> bool {
    let Some(places) = rt.places() else { return false };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    tokio::task::spawn_local(async move { places.list(now).await.map(|rows| rows.iter().any(is_joined_computer)).unwrap_or(false) })
        .await
        .unwrap_or(false)
}

fn is_joined_computer(row: &wsp_wire::views::place::PlaceView) -> bool {
    matches!(row.kind, wsp_wire::views::place_kind::PlaceKind::Computer) && row.joined_at.is_some()
}

/// A refusal of the line itself, before anything is read: `wsp up` takes no word.
pub fn takes_no_word(words: &[String]) -> Option<HostError> {
    (!words.is_empty()).then(|| {
        usage_refusal(
            &format!("wsp up takes no word, and got {}.", words[0]),
            "usage: wsp up [--port <n>] [--listen <addr>] [--advertise <url>] [--no-relay] [--service]",
        )
    })
}
