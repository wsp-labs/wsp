// SPDX-License-Identifier: AGPL-3.0-only
//! provision-files.ts:303-398, 512-647 and 798-909 at a4033dc34: the list beside the job, its readers and its close,
//! the files round as a whole, the unland and the unmerge. Spec: spec/files_ledger.md.
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use super::{FilesSay, LandFilesResult, ProvisionLanding, StoreRoots};
use crate::errors::Result;
use crate::golden_seam::{PackFiles, ScopeFile, ScopeRead};
use crate::machine::Machine;

/// provision-files.ts:304.
pub const SERVER_MARK: &str = "wsp-server";
/// provision-files.ts:326.
pub const OWN_MARK: &str = "wsp-own";
/// provision-files.ts:846.
pub const UNLAND_MARK: &str = "wsp-unland";

/// provision-files.ts:310 `landedServersScript`. Byte for byte the TS's text.
pub fn landed_servers_script(home: &str) -> String {
    todo!()
}

/// provision-files.ts:334 `landedFilesScript`. Byte for byte the TS's text; `landed_files_script("{home}")` plus a
/// newline is `daemon/fixtures/contract/landed-files.sh`.
pub fn landed_files_script(home: &str) -> String {
    todo!()
}

/// provision-files.ts:357 `closeFilesScript(home, once = [], roots = [])`. Byte for byte the TS's text.
pub fn close_files_script(home: &str, once: &[String], roots: &StoreRoots) -> String {
    todo!()
}

/// provision-files.ts:515 `serverDigest`: the sha256 hex of the entry's UTF-8 bytes; `None` for no entry.
pub fn server_digest(entry: Option<&str>) -> Option<String> {
    todo!()
}

/// provision-files.ts:519 `parseLandedServers`: row id to digest; a line counts with the mark first and more than
/// two tab fields; the id is every field from the third on joined by tabs; a later line for an id overwrites the
/// value and keeps the id's first place.
pub fn parse_landed_servers(stdout: &str) -> IndexMap<String, String> {
    todo!()
}

/// provision-files.ts:531 `landedServers`: empty where the run rejected or exited non-zero, and then nothing is said;
/// else says `provision_list_read_line(keys)` once.
pub async fn landed_servers(machine: &dyn Machine, home: &str, say: FilesSay) -> IndexMap<String, String> {
    todo!()
}

/// provision-files.ts:541 `ServerPort`: the three things taking wsp's servers back out needs of the computer.
#[async_trait(?Send)]
pub trait ServerPort {
    /// Runs a script there and answers what it printed; empty where it would not run.
    async fn run(&self, script: &str) -> String;
    /// The first of an agent's files that is there, with its text or why it was left unread; `None` where none is.
    async fn read(&self, files: &[String], base: &str) -> Result<Option<ScopeRead>>;
    /// That file, as it was read, with this text by the one config write inside `base`.
    async fn write(&self, file: &ScopeFile, text: &str, base: &str) -> Result<()>;
}

/// provision-files.ts:553 `machineServerPort`.
pub struct MachineServerPort {
    pub machine: Arc<dyn Machine>,
}

/// provision-files.ts:553 `machineServerPort(machine)`.
pub fn machine_server_port(machine: Arc<dyn Machine>) -> MachineServerPort {
    todo!()
}

#[async_trait(?Send)]
impl ServerPort for MachineServerPort {
    /// provision-files.ts:555: `run` with deadline `LAND_MS`; `""` where it rejected or exited non-zero.
    async fn run(&self, script: &str) -> String {
        todo!()
    }
    /// provision-files.ts:559: `read_configs_cmd([{files, base}])` run unlogged with deadline `READ_MS`; `None` where
    /// it rejected, exited non-zero, or the parse answered nothing; else the parse's first entry.
    async fn read(&self, files: &[String], base: &str) -> Result<Option<ScopeRead>> {
        todo!()
    }
    /// provision-files.ts:566: `land_configs(machine, base, [file], {file.path: text})`; its words are the error.
    async fn write(&self, file: &ScopeFile, text: &str, base: &str) -> Result<()> {
        todo!()
    }
}

/// provision-files.ts:574 `ServersOut`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServersOut {
    Took { path: String, names: Vec<String> },
    Unread { path: String, unread: String },
}

/// provision-files.ts:577 `serversOutLines`: one line, `servers_unread_line(path, unread)` or `<names joined by ",
/// "> (out of <path>)`.
pub fn servers_out_lines(out: &ServersOut) -> Vec<String> {
    todo!()
}

/// provision-files.ts:586 `unmergeServers(port, home, stores = {})`.
pub async fn unmerge_servers(port: &dyn ServerPort, home: &str, stores: &IndexMap<String, String>) -> Vec<ServersOut> {
    todo!()
}

/// provision-files.ts:640 `ownMarks`: in the order printed; drops a line without the mark, an empty path, an
/// absolute one, or one with a `..` part.
pub fn own_marks(stdout: &str) -> Vec<String> {
    todo!()
}

/// provision-files.ts:802 `appendLanding`: nothing for no lines; else one inline exec, its failure swallowed.
pub async fn append_landing(machine: &dyn Machine, home: &str, lines: &[String]) {
    todo!()
}

/// provision-files.ts:814 `provisionFiles`' argument. `say: None` is the TS's absent say: every stage unsaid.
#[derive(Clone)]
pub struct ProvisionFilesAsk {
    pub home: String,
    pub lands: Vec<ProvisionLanding>,
    pub pack: PackFiles,
    pub say: Option<FilesSay>,
    pub stores: IndexMap<String, String>,
}

/// provision-files.ts:814 `provisionFiles`. Never fails: a pack or a landing that fails is one failed row per
/// planned path with the error's first line.
pub async fn provision_files(machine: &dyn Machine, o: ProvisionFilesAsk) -> LandFilesResult {
    todo!()
}

/// provision-files.ts:852 `unlandScript(home, dests, roots = [])`. Byte for byte the TS's text.
pub fn unland_script(home: &str, dests: &[String], roots: &StoreRoots) -> String {
    todo!()
}

/// provision-files.ts:889 `unlandFiles`' answer, each list sorted by UTF-16 code unit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unlanded {
    pub gone: Vec<String>,
    pub kept: Vec<String>,
}

/// provision-files.ts:889 `unlandFiles(machine, home, dests, stores = {})`: empty lists with no exec for no dests;
/// `None` where the exec rejected or exited non-zero.
pub async fn unland_files(machine: &dyn Machine, home: &str, dests: &[String], stores: &IndexMap<String, String>) -> Option<Unlanded> {
    todo!()
}

/// provision-files.ts:907 `closeAgentFiles(machine, home, once = [], stores = {})`: one run, its failure swallowed.
pub async fn close_agent_files(machine: &dyn Machine, home: &str, once: &[String], stores: &IndexMap<String, String>) {
    todo!()
}

/// A boxed future the ledger's callers hand around.
pub type Pending<T> = LocalBoxFuture<'static, T>;
