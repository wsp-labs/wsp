// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/helpers.ts` at 65f6b89c7: the readers and constants every area of the place door
//! shares. Spec: `spec/places_helpers.md`; `listen_near` is `spec/places_pane_ports.md`'s.

use std::cell::RefCell;
use std::future::Future;
use std::io::ErrorKind;
use std::net::{Ipv6Addr, SocketAddr};
use std::path::Path;
use std::rc::Rc;
use std::task::Poll;

use ed25519_dalek::pkcs8::DecodePublicKey;
use ed25519_dalek::VerifyingKey;
use futures::channel::oneshot;
use futures::future::{select, Either, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use sha2::{Digest, Sha256};
use tokio::net::{TcpListener, TcpStream};
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::catalog::shared_on;
use wsp_catalog::signin::{key_env_of, mints_token, SignIn};
use wsp_catalog::signin_rows::{shared_file_in, SHARED_LOGINS};
use wsp_core::place_forward::PlaceForward;
use wsp_core::reach::ReachError;
use wsp_core::recipe_sync::RecipeChange;
use wsp_core::types::pending::{Clock, DaemonReach};
use wsp_engine::errors::EngineError;
use wsp_engine::provision::ProvisionPlan;
use wsp_engine::ssh_backend::{plain_path, ssh_store_vars};
use wsp_store::keys::node_base64;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::recipe_file::{RecipeFile, RecipeSignIn};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::agent_of_row;
use wsp_wire::views::place::{
    sign_in_of_row, PendingComputer, PlaceApplied, PlaceProvisionRow, PlaceProvisionRowOutcome,
    PlaceSetup, PlaceSetupStep, PlaceSync, SetupEnd, GITHUB_ROW,
};
use wsp_wire::wire::limits::is_plain_path;
use wsp_wire::wire::machine_link::MachineErrorKind;
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::exit::{HostError, Kind};
use wsp_words::recipe_file::{recipe_canon, AgentOption, RecipeOptions, ResolvedRecipe};
use wsp_words::views::place_setup_words::{
    copied_from_line, important_failures, key_held_line, token_held_line, wait_line,
    SIGNED_IN_THERE,
};
use wsp_words::words::base::plural;

use super::types::{PlaceInstalled, PlaceLogin, PlaceWiring};

pub type Cancel = Box<dyn FnOnce()>;

/// A row landed from outside a run: a [`PlaceProvisionRow`] whose label is optional (helpers.ts:44).
#[derive(Clone, Debug, PartialEq)]
pub struct LandedRow {
    pub row: PlaceProvisionRow,
    /// `None` is the TS row with no `label`; `row.label` is then ignored.
    pub label: Option<String>,
}

/// The rows with `row` landed, or `None` where it lands nothing (helpers.ts:47).
pub fn landed_on(rows: &[PlaceProvisionRow], row: &LandedRow) -> Option<Vec<PlaceProvisionRow>> {
    let Some(at) = rows.iter().position(|r| r.id == row.row.id) else {
        let label = row.label.as_ref()?;
        let mut landed = row.row.clone();
        landed.label = label.clone();
        let mut out = rows.to_vec();
        out.push(landed);
        return Some(out);
    };
    let stood = &rows[at];
    if !matches!(
        stood.outcome,
        PlaceProvisionRowOutcome::Skipped | PlaceProvisionRowOutcome::Failed
    ) {
        return None;
    }
    let mut landed = row.row.clone();
    landed.label = stood.label.clone();
    if stood.step.is_some() {
        landed.step = stood.step;
    }
    let mut out = rows.to_vec();
    out[at] = landed;
    Some(out)
}

/// `work`'s answer, or after `ms` on `clock` the error `"{what} was not answered in {round(ms / 1000)}s"`; the
/// timer is cancelled once `work` settles (helpers.ts:56).
pub async fn bounded<T>(
    clock: &dyn Clock,
    work: impl Future<Output = Result<T, HostError>>,
    ms: u64,
    what: &str,
) -> Result<T, HostError> {
    let (fire, fired) = oneshot::channel::<()>();
    let cancel = clock.schedule(
        Box::new(move || {
            let _ = fire.send(());
        }),
        ms as i64,
    );
    let work = std::pin::pin!(work);
    let outcome = select(work, fired).await;
    cancel();
    match outcome {
        Either::Left((answer, _)) => answer,
        Either::Right(_) => Err(HostError::new(format!(
            "{what} was not answered in {}s",
            (ms + 500) / 1000
        ))),
    }
}

/// Whether a public key off the wire is an ed25519 SPKI DER key in base64 (helpers.ts:81). Never an error.
pub fn reads_as_ed25519(public_key_base64: &str) -> bool {
    VerifyingKey::from_public_key_der(&node_base64(public_key_base64)).is_ok()
}

/// How long a join that has been answered has to send its prove (helpers.ts:91).
pub const JOIN_PROVE_MS: u64 = 120_000;

/// The one sentence a join whose key this host cannot verify against is refused with (helpers.ts:94).
pub const PLACE_BAD_KEY_REFUSAL: &str = "that join sent a key this host cannot verify a signature against; a place's key is ed25519, as SPKI DER in base64";

/// The refusal of a report whose home is no plain path; `None` and `""` read "no home folder" (helpers.ts:99).
pub fn place_home_refusal(said: Option<&str>) -> String {
    let what = match said {
        None | Some("") => "no home folder".to_string(),
        Some(said) => format!(
            "{}, which is not a plain path",
            serde_json::to_string(said).unwrap_or_default()
        ),
    };
    format!("that computer reported {what} for its login, so nothing on it could be reached; wsp holds a machine's home to a plain absolute path")
}

/// The one door every report passes (helpers.ts:110): the home held to a plain path, the PATH to its plain folders,
/// a harness's store folder kept only where plain.
pub fn taken_report(report: &PlaceReport) -> Result<PlaceReport, HostError> {
    let Some(home) = report.login.get("HOME").filter(|home| is_plain_path(home)) else {
        return Err(HostError::new(place_home_refusal(
            report.login.get("HOME").map(String::as_str),
        )));
    };
    let mut login = report.login.clone();
    login.insert("HOME".to_string(), home.clone());
    login.insert(
        "PATH".to_string(),
        plain_path(report.login.get("PATH").map(String::as_str)),
    );
    for name in ssh_store_vars() {
        if login
            .get(&name)
            .is_some_and(|folder| !is_plain_path(folder))
        {
            login.shift_remove(&name);
        }
    }
    Ok(PlaceReport {
        login,
        ..report.clone()
    })
}

/// The agents on a computer as this host knows them (helpers.ts:123).
pub fn agents_on(report_agents: &[String], rows: &[PlaceProvisionRow]) -> Vec<String> {
    let found: Vec<String> = rows
        .iter()
        .filter(|r| {
            matches!(
                r.outcome,
                PlaceProvisionRowOutcome::Installed | PlaceProvisionRowOutcome::Present
            )
        })
        .filter_map(|r| agent_of_row(&r.id))
        .collect();
    let mut out: IndexSet<String> = report_agents.iter().cloned().collect();
    for agent in catalog_agents() {
        if found.contains(&agent.id) {
            out.insert(agent.id.clone());
        }
    }
    out.into_iter().collect()
}

/// What stands for each agent on a computer, or `None` where the report carries no logins list (helpers.ts:137).
/// `agents` `None` is `agents_on(report, rows)`.
pub fn sign_ins_of(
    report_agents: &[String],
    report_logins: Option<&[String]>,
    vault: &IndexMap<String, String>,
    rows: &[PlaceProvisionRow],
    agents: Option<&[String]>,
) -> Option<IndexMap<String, AgentSignInState>> {
    let logins = report_logins?;
    let signed_there: Vec<String> = rows
        .iter()
        .filter(|r| {
            r.note.as_deref() == Some(SIGNED_IN_THERE)
                && matches!(
                    r.outcome,
                    PlaceProvisionRowOutcome::Installed | PlaceProvisionRowOutcome::Present
                )
        })
        .filter_map(|r| sign_in_of_row(&r.id))
        .collect();
    let owned = if agents.is_none() {
        agents_on(report_agents, rows)
    } else {
        Vec::new()
    };
    let mut words = IndexMap::new();
    for id in agents.unwrap_or(&owned) {
        let signed = match shared_login_file(id) {
            Some(file) => logins.contains(&file),
            None => signed_there.contains(id),
        };
        let word = if signed {
            AgentSignInState::SignedIn
        } else {
            vault_sign_in(id, vault)
        };
        words.insert(id.clone(), word);
    }
    Some(words)
}

/// The file under a computer's logins folder an agent's shared login writes (helpers.ts:160).
pub fn shared_login_file(agent_id: &str) -> Option<String> {
    shared_on(agent_id).map(shared_file_in)
}

/// The files every shared login writes under a computer's logins folder (helpers.ts:169).
fn shared_files() -> Vec<String> {
    SHARED_LOGINS.iter().map(shared_file_in).collect()
}

fn agent_sign_in(agent_id: &str) -> Option<&'static SignIn> {
    catalog_agents()
        .into_iter()
        .find(|a| a.id == agent_id)
        .map(|a| &a.sign_in)
}

/// How long the read of which shared logins stand on a computer gets (helpers.ts:166).
pub const LOGINS_READ_MS: u64 = 10_000;

/// The script that prints which shared logins stand under that logins folder (helpers.ts:173).
pub fn shared_logins_script(logins: &str) -> String {
    let files: Vec<String> = shared_files().iter().map(|f| shell_quote(f)).collect();
    format!(
        "cd {} 2>/dev/null || exit 0\nfor f in {}; do if [ -e \"$f\" ] || [ -L \"$f\" ]; then [ -d \"$f\" ] || printf '%s\\n' \"$f\"; fi; done",
        shell_quote(logins),
        files.join(" ")
    )
}

/// A logins list with each shared login's file as `found` says it stands now; `None` where that is the list
/// already (helpers.ts:179).
pub fn relisted_logins(listed: &[String], found: &[String]) -> Option<Vec<String>> {
    let shared = shared_files();
    let mut next: Vec<String> = listed
        .iter()
        .filter(|f| !shared.contains(f))
        .cloned()
        .collect();
    next.extend(found.iter().filter(|f| shared.contains(f)).cloned());
    next.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    (next.as_slice() != listed).then_some(next)
}

/// `vault-key` where this host's vault holds the agent's token or key, else `none` (helpers.ts:186).
pub fn vault_sign_in(agent_id: &str, vault: &IndexMap<String, String>) -> AgentSignInState {
    let sign_in = agent_sign_in(agent_id);
    let token = sign_in
        .and_then(mints_token)
        .and_then(|t| vault.get(&t.token_env));
    let key = sign_in
        .and_then(key_env_of)
        .is_some_and(|name| vault.contains_key(name));
    if token.is_some() || key {
        AgentSignInState::VaultKey
    } else {
        AgentSignInState::None
    }
}

/// What a sign-in row says the vault hands an agent's turns on a computer picked `way` (helpers.ts:195).
pub fn vault_held_line(
    agent_id: &str,
    vault: &IndexMap<String, String>,
    way: Option<RecipeSignIn>,
    here: &str,
) -> String {
    let sign_in = agent_sign_in(agent_id);
    let Some(token) = sign_in.and_then(mints_token) else {
        return copied_from_line(here);
    };
    let key = sign_in
        .and_then(key_env_of)
        .is_some_and(|name| vault.contains_key(name));
    if (matches!(way, Some(RecipeSignIn::Key)) && key) || !vault.contains_key(&token.token_env) {
        key_held_line(here)
    } else {
        token_held_line(here)
    }
}

/// The vault as a computer's picks hand it to the turns there (helpers.ts:205): a token dropped for an agent picked
/// to take its key where the key is held.
pub fn picked_vault(
    vault: &IndexMap<String, String>,
    picks: Option<&RecipeFile>,
) -> IndexMap<String, String> {
    let mut dropped: Vec<&str> = Vec::new();
    for (name, row) in picks.into_iter().flat_map(|p| p.agents.iter()) {
        if !matches!(row.signin, Some(RecipeSignIn::Key)) {
            continue;
        }
        let sign_in = agent_sign_in(&name.0);
        if let (Some(token), Some(key)) =
            (sign_in.and_then(mints_token), sign_in.and_then(key_env_of))
        {
            if vault.contains_key(key) {
                dropped.push(&token.token_env);
            }
        }
    }
    if dropped.is_empty() {
        return vault.clone();
    }
    vault
        .iter()
        .filter(|(name, _)| !dropped.contains(&name.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// The picks a computer can be set up from, as this host can serve them (helpers.ts:217).
pub fn servable_options(
    options: &RecipeOptions,
    vault: &IndexMap<String, String>,
    held: &IndexSet<String>,
) -> RecipeOptions {
    let agents = options
        .agents
        .iter()
        .map(|a| {
            let offers_vault = a.signins.iter().any(|w| matches!(w, RecipeSignIn::Vault));
            if offers_vault
                && !matches!(vault_sign_in(&a.id, vault), AgentSignInState::VaultKey)
                && !held.contains(&a.id)
            {
                AgentOption {
                    signins: a
                        .signins
                        .iter()
                        .filter(|w| !matches!(w, RecipeSignIn::Vault))
                        .cloned()
                        .collect(),
                    ..a.clone()
                }
            } else {
                a.clone()
            }
        })
        .collect();
    RecipeOptions {
        agents,
        ..options.clone()
    }
}

/// The agent homes on this computer, as `agents.homesHere` answers them.
pub type HomesHere =
    Rc<dyn Fn() -> LocalBoxFuture<'static, Result<IndexMap<String, String>, HostError>>>;

/// The login an agent shares into every thread, as this computer holds it; `None` where missing or empty
/// (helpers.ts:224).
pub async fn login_here(homes: HomesHere, agent_id: &str) -> Option<Vec<u8>> {
    let shared = shared_on(agent_id)?;
    let home = homes().await.ok()?.get(agent_id).cloned()?;
    let held = tokio::fs::read(Path::new(&home).join(&shared.file))
        .await
        .ok()?;
    (!held.is_empty()).then_some(held)
}

/// How long between writes of a linked place's last seen (helpers.ts:232).
pub const SEEN_EVERY_MS: u64 = 60_000;

/// What a socket is closed with when a newer link for the same place arrives (helpers.ts:236).
pub const REPLACED: &str = "replaced by a newer link";

/// The websocket a place's link rides, as the door uses it (the TS `ws` WebSocket).
pub trait LinkSocket {
    /// `readyState === OPEN`: a socket that is closing is not open.
    fn is_open(&self) -> bool;
    fn close(&self, code: u16, reason: &str);
    /// `once("close")`: called once with the close's code and reason. Answers an id for [`LinkSocket::off_close`].
    fn on_close(&self, f: Box<dyn FnOnce(u16, String)>) -> u64;
    fn off_close(&self, id: u64);
    /// The daemon road over this socket: wsp-core's `connect_daemon` with the seal and the event callback
    /// (link.ts:170). On the socket so the attach can be driven over a fake one.
    fn connect(
        self: Rc<Self>,
        seal: Option<wsp_store::keys::Seal>,
        on_event: Rc<dyn Fn(&wsp_wire::wire::daemon_events::DaemonEvent)>,
    ) -> Rc<dyn DaemonReach>;
}

/// The forward a place's panes ride, opened at the first pane that asks (helpers.ts:243).
pub type HeldForward = Shared<LocalBoxFuture<'static, Result<Rc<PlaceForward>, HostError>>>;

/// One linked place as the door holds it (helpers.ts:238).
pub struct Live {
    pub socket: Rc<dyn LinkSocket>,
    pub reach: Rc<dyn DaemonReach>,
    /// The last-seen timer; cancelled when the link goes.
    pub seen: Cancel,
    pub forward: Option<HeldForward>,
}

/// One connection riding a forward: what the views write a tunnel's bytes into.
pub trait ForwardConn {
    fn write(&self, bytes: &[u8]);
    fn end(&self);
    fn destroy(&self);
}

/// A loopback listener a forward holds; [`LoopbackListener::close`] frees its port.
pub struct LoopbackListener {
    pub addr: SocketAddr,
    pub(crate) close: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl LoopbackListener {
    /// Stops accepting and frees the port; a second close does nothing.
    pub fn close(&self) {
        let close = self.close.borrow_mut().take();
        if let Some(close) = close {
            close();
        }
    }
}

/// One port on this computer carried to one port on a place (helpers.ts:248).
pub struct Forward {
    pub server: Rc<LoopbackListener>,
    pub local_port: u16,
    /// The connections riding it now, by the tunnel id the far side knows each by.
    pub conns: Rc<RefCell<IndexMap<String, Rc<dyn ForwardConn>>>>,
    /// Takes away what a port opened for a pane holds beyond its first listener.
    pub stop: Option<Rc<dyn Fn()>>,
}

/// A connection accepted on a loopback listener.
pub type OnConn = Rc<dyn Fn(tokio::net::TcpStream)>;

const NEAR_PORTS: u16 = 20;

/// Listens on `port` on both loopback families, or on the first free one up to 20 above it (helpers.ts:262).
pub async fn listen_near(
    port: u16,
    on_conn: OnConn,
) -> Result<(Vec<Rc<LoopbackListener>>, u16), HostError> {
    let last = port.saturating_add(NEAR_PORTS);
    for at in port..=last {
        let v4_addr = SocketAddr::from(([127, 0, 0, 1], at));
        let Ok(v4) = TcpListener::bind(v4_addr).await else {
            continue;
        };
        let v6_addr = SocketAddr::from((Ipv6Addr::LOCALHOST, at));
        match TcpListener::bind(v6_addr).await {
            Ok(v6) => {
                return Ok((
                    vec![
                        serve(v4, v4_addr, Rc::clone(&on_conn)),
                        serve(v6, v6_addr, on_conn),
                    ],
                    at,
                ))
            }
            Err(e) if e.kind() != ErrorKind::AddrInUse => {
                return Ok((vec![serve(v4, v4_addr, on_conn)], at))
            }
            Err(_) => drop(v4),
        }
    }
    Err(HostError::new(format!(
        "no port from {port} to {} is free on this computer",
        u32::from(port) + u32::from(NEAR_PORTS)
    )))
}

/// Accepts until closed; closing drops the listener at once, so the port is free when close returns.
fn serve(listener: TcpListener, addr: SocketAddr, on_conn: OnConn) -> Rc<LoopbackListener> {
    let slot = Rc::new(RefCell::new(Some(listener)));
    let task = wsp_engine::local_task::try_spawn_local({
        let slot = Rc::clone(&slot);
        async move {
            while let Some(stream) = accept_one(&slot).await {
                on_conn(stream);
            }
        }
    });
    let stop = task.map(|task| task.abort_handle());
    let close = Box::new(move || {
        let listener = slot.borrow_mut().take();
        drop(listener);
        if let Some(stop) = &stop {
            stop.abort();
        }
    });
    Rc::new(LoopbackListener {
        addr,
        close: RefCell::new(Some(close)),
    })
}

/// The next connection the slot's listener accepts; none once it is closed or fails.
async fn accept_one(slot: &RefCell<Option<TcpListener>>) -> Option<TcpStream> {
    std::future::poll_fn(|cx| {
        let slot = slot.borrow();
        let Some(listener) = slot.as_ref() else {
            return Poll::Ready(None);
        };
        match listener.poll_accept(cx) {
            Poll::Ready(Ok((stream, _))) => Poll::Ready(Some(stream)),
            Poll::Ready(Err(_)) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    })
    .await
}

/// How long a machine frame waits for its answer when the caller named no bound (helpers.ts:286).
pub const LINK_FRAME_MS: u64 = 300_000;
/// How long a request the host may ask again waits for the computer to open a socket again (helpers.ts:291).
pub const RELINK_WAIT_MS: u64 = 120_000;
/// How long a place gets to say what its backend is (helpers.ts:295).
pub const BACKEND_FACTS_MS: u64 = 10_000;
/// How long an install waits for that answer before it answers the person (helpers.ts:298).
pub const ADD_FACTS_MS: u64 = 2_000;
pub const CAPACITY_MS: u64 = 5_000;
/// How long one dial of a computer gets (helpers.ts:302).
pub const DIAL_MS: u64 = 20_000;
/// How long a computer has to dial back after its own join wrote its place file (helpers.ts:306).
pub const JOIN_WAIT_MS: u64 = 90_000;
/// Finished adds kept beside the running ones (helpers.ts:309).
pub const ADDS_KEPT: usize = 20;
pub const ADD_RUNNING_LINE: &str = "an add under that id is still running";
pub const ADD_RUNNING_FIX: &str = "Leave the id out, or name the add afresh.";

/// What the box itself said while that wait ran out, where a log road and its ssh login exist (helpers.ts:318).
pub async fn box_said(wiring: &PlaceWiring, installed: &PlaceInstalled) -> Vec<String> {
    let (Some(log), Some(ssh)) = (wiring.log.as_ref(), installed.ssh.as_ref()) else {
        return Vec::new();
    };
    let login = PlaceLogin {
        ssh: ssh.clone(),
        key_path: installed.ssh_key_path.clone(),
        host_key: None,
    };
    log.log(&login).await.unwrap_or_default()
}

/// One document per add that has not reached Set up (helpers.ts:325).
pub const PENDING: &str = "pending-computers";

/// A pending add as the store keeps it (helpers.ts:330): the client's view and the door's own fields.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRecord {
    #[serde(flatten)]
    pub view: PendingComputer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

/// A pending add as a client reads it (helpers.ts:333).
pub fn pending_view(p: &PendingRecord) -> PendingComputer {
    p.view.clone()
}

pub const ADD_STOPPED_LINE: &str = "the host stopped while wsp was being installed there; what the install put there was taken back";
pub const ADD_STOPPED_FIX: &str = "Add it again.";
pub const ADD_NOT_TAKEN_BACK_LINE: &str = "the host stopped while wsp was being installed there, and what the install put there could not be taken back";

/// One sync of a computer to the recipe it follows (helpers.ts:345).
#[derive(Clone, Debug)]
pub struct SyncJob {
    pub file: RecipeFile,
    pub items: IndexMap<String, String>,
    pub hash: String,
    pub changes: Vec<RecipeChange>,
    pub steps: IndexSet<PlaceSetupStep>,
    /// The rows added or changed, by `<kind>/<row>`.
    pub moved: IndexSet<String>,
    pub before: RecipeFile,
    pub state: PlaceSync,
}

/// How a setup came out (helpers.ts:359).
pub fn setup_outcome(
    setup: &PlaceSetup,
    applied: Option<&PlaceApplied>,
) -> (SetupEnd, Option<String>) {
    let mut said: Vec<String> = setup.waiting.iter().map(wait_line).collect();
    said.extend(
        important_failures(applied)
            .iter()
            .map(|r| format!("{}: {}", r.label, r.note.as_deref().unwrap_or("failed"))),
    );
    if !said.is_empty() {
        return (SetupEnd::NeedsYou, Some(said.join("; ")));
    }
    let missed = applied.map_or(0, |a| {
        a.rows
            .iter()
            .filter(|r| r.outcome == PlaceProvisionRowOutcome::Failed)
            .count()
    });
    let said = (missed > 0).then(|| format!("{} did not install", plural(missed as f64, "row")));
    (SetupEnd::Ready, said)
}

/// The refusal an estimate gets on a runtime whose provisioner weighs nothing (helpers.ts:367).
pub const NO_ESTIMATE_LINE: &str =
    "this runtime weighs no picks; the host that serves the app wires the reader";
pub const LOG_READ_MS: u64 = 10_000;
pub const UNDO_MS: u64 = 300_000;
pub const SIGN_IN_SLACK_MS: u64 = 5_000;
pub const GITHUB_MS: u64 = 30_000;

/// The plan with the CLIs `ids` first, each behind the steps it needs (helpers.ts:382).
pub fn cli_first(plan: &ProvisionPlan, ids: &[String]) -> ProvisionPlan {
    let (head, clis) = plan.steps.split_at(plan.agents.min(plan.steps.len()));
    let mut first: Vec<usize> = Vec::new();
    for id in ids {
        let mut chain: Vec<usize> = Vec::new();
        let mut at = clis.iter().position(|s| s.id == *id);
        while let Some(i) = at {
            if first.contains(&i) || chain.contains(&i) {
                break;
            }
            chain.insert(0, i);
            at = clis[i]
                .after
                .as_ref()
                .and_then(|after| clis.iter().position(|s| s.id == *after));
        }
        first.extend(chain);
    }
    if first.is_empty() {
        return plan.clone();
    }
    let mut steps = head.to_vec();
    steps.extend(first.iter().map(|&i| clis[i].clone()));
    steps.extend(
        clis.iter()
            .enumerate()
            .filter(|(i, _)| !first.contains(i))
            .map(|(_, s)| s.clone()),
    );
    ProvisionPlan {
        steps,
        ..plan.clone()
    }
}

/// A setup row the engine read off that computer (helpers.ts:400).
pub fn engine_row(r: &PlaceProvisionRow) -> bool {
    r.step
        .is_some_and(|step| !matches!(step, PlaceSetupStep::Signins | PlaceSetupStep::Folders))
        && r.id != GITHUB_ROW
}

pub const INSTALLS: &str = "installs";
pub const FILES: &str = "files";
pub const PROBE_MS: u64 = 30_000;
pub const SIGNIN_STATUS_MS: u64 = 30_000;

/// The message up to its first newline (helpers.ts:410).
pub fn first_line_of(e: &HostError) -> String {
    e.message.split('\n').next().unwrap_or_default().to_string()
}

/// The hash of what a computer was set up with (helpers.ts:413).
pub fn picks_hash(picks: &RecipeFile) -> String {
    let canon = recipe_canon(&ResolvedRecipe {
        file: picks.clone(),
        items: IndexMap::new(),
    });
    hex::encode(Sha256::digest(canon.as_bytes()))
}

pub const PROVISION_LOG_EVERY_MS: u64 = 2_000;
pub const PROVISION_LOG_LINES: usize = 50;
pub const UPDATE_WAIT_MS: u64 = 60_000;
pub const UPDATE_POLL_MS: u64 = 500;
pub const UNMERGE_MS: u64 = 60_000;

/// The outcome words a row is read by (`installed`, `present`, `skipped`, `failed`), re-exported for the areas.
pub type Outcome = PlaceProvisionRowOutcome;

/// The word a refusal's kind is written with.
fn kind_word(kind: MachineErrorKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// A daemon's refusal as the host error its caller gets, with the kind, status and code it was refused with
/// (reach.ts:162 `refused`).
pub(crate) fn refused_host(e: ReachError) -> HostError {
    let mut host = HostError::new(e.message);
    host.kind = e.kind.map(|k| Kind::parse(&kind_word(k)));
    host.status = e.status;
    host.code = e.code;
    host
}

/// A daemon's refusal as the engine error a machine road gets: a refusal that carries a kind is the provider's own
/// kind of error, so a machine the far side lost reads `missing` (reach.ts:162, link-backend.ts:62).
pub(crate) fn refused_engine(e: ReachError) -> EngineError {
    let kind = e.kind.and_then(|k| {
        serde_json::from_value::<wsp_engine::errors::ErrorKind>(serde_json::Value::String(
            kind_word(k),
        ))
        .ok()
    });
    match (kind, e.status) {
        (Some(kind), Some(status)) => EngineError::Provider(wsp_engine::errors::WspError {
            kind,
            status: status as u16,
            code: e.code,
            message: e.message,
            request_id: None,
            window: None,
            said: None,
        }),
        (None, None) => EngineError::Other {
            message: e.message,
            code: e.code,
        },
        (kind, status) => EngineError::Refused {
            message: e.message,
            kind,
            status: status.map(|s| s as u16),
            code: e.code,
        },
    }
}
