// SPDX-License-Identifier: AGPL-3.0-only
//! provision-files.ts:303-398, 512-647 and 798-909 at a4033dc34: the list beside the job, its readers and its close,
//! the files round as a whole, the unland and the unmerge. Spec: spec/files_ledger.md.
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::{mpsc, oneshot};
use futures::future::{select, Either, LocalBoxFuture};
use futures::{FutureExt, StreamExt};
use indexmap::{IndexMap, IndexSet};
use sha2::{Digest, Sha256};
use wsp_catalog::catalog::mcp_agents;
use wsp_catalog::mcp_launch::{launch_server_road, own_server_config};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::place::{PlaceProvisionKind, PlaceProvisionRow, PlaceProvisionRowOutcome};
use wsp_words::agents_report::servers_unread_line;
use wsp_words::project_path::place_provision_paths;
use wsp_words::views::place_setup_words::{provision_list_read_line, provision_packed_line, Packed};

use super::land::{
    dest_test, file_label, key_test, land_agent_files, lands_once, once_paths_of, once_test, root_of, standing_digests, store_roots,
    LandAgentFiles, RootOf, LAND_MS, NO_DIGEST,
};
use super::{FilesSay, LandFilesResult, ProvisionLanding, StoreRoots};
use crate::errors::{EngineError, Result};
use crate::exec_detached::{INLINE_EXEC_MS, OLD_APPEND_MARKS};
use crate::golden_seam::{
    land_configs, parse_configs, parse_mcp_id, read_configs_cmd, LeaveOut, PackFiles, ReadAsk, ScopeFile, ScopeRead, StagedFile, READ_MS,
};
use crate::machine::{ExecOptions, Machine, RunOptions};

const TAB: &str = r"\t";
const NL: &str = r"\n";

/// provision-files.ts:304.
pub const SERVER_MARK: &str = "wsp-server";
/// provision-files.ts:326.
pub const OWN_MARK: &str = "wsp-own";
/// provision-files.ts:846.
pub const UNLAND_MARK: &str = "wsp-unland";

/// provision-files.ts:310 `landedServersScript`. Byte for byte the TS's text.
pub fn landed_servers_script(home: &str) -> String {
    let at = place_provision_paths(home);
    let mut lines: Vec<String> = vec!["set -u".to_string()];
    lines.extend(key_test());
    lines.extend([
        format!("ledger={}", shell_quote(&at.landed)),
        r#"[ -f "$ledger" ] || exit 0"#.to_string(),
        format!(r#"tab=$(printf "{TAB}")"#),
        r#"while IFS="$tab" read -r id from at; do"#.to_string(),
        format!(r#"  wsp_key "$id" && printf 'wsp-server{TAB}%s{TAB}%s{NL}' "$at" "$id""#),
        r#"done < "$ledger""#.to_string(),
        "exit 0".to_string(),
    ]);
    lines.join("\n")
}

/// provision-files.ts:334 `landedFilesScript`. Byte for byte the TS's text; `landed_files_script("{home}")` plus a
/// newline is `daemon/fixtures/contract/landed-files.sh`.
pub fn landed_files_script(home: &str) -> String {
    let at = place_provision_paths(home);
    [
        "set -u".to_string(),
        format!("home={}; ledger={}", shell_quote(home), shell_quote(&at.landed)),
        r#"[ -f "$ledger" ] || exit 0"#.to_string(),
        format!(r#"tab=$(printf "{TAB}")"#),
        r#"while IFS="$tab" read -r rel from at; do"#.to_string(),
        r#"  dest="$home/$rel""#.to_string(),
        r#"  [ -f "$dest" ] || continue"#.to_string(),
        r#"  d=$(sha256sum "$dest" | cut -d" " -f1)"#.to_string(),
        format!(r#"  [ "$d" = "$at" ] && printf 'wsp-own{TAB}%s{NL}' "$rel""#),
        r#"done < "$ledger""#.to_string(),
        "exit 0".to_string(),
    ]
    .join("\n")
}

/// provision-files.ts:357 `closeFilesScript(home, once = [], roots = [])`. Byte for byte the TS's text.
pub fn close_files_script(home: &str, once: &[String], roots: &StoreRoots) -> String {
    let at = place_provision_paths(home);
    let q = |s: &str| shell_quote(s);
    let mut lines: Vec<String> = vec!["set -u".to_string()];
    lines.extend(once_test(once));
    lines.extend(key_test());
    lines.extend(dest_test(roots));
    lines.extend([
        format!("home={}; stage={}; ledger={}; landing={}; log={}", q(home), q(&at.staging), q(&at.landed), q(&at.landing), q(&at.log)),
        format!(r#"rm -f "$log"{OLD_APPEND_MARKS}"#),
        r#"[ -f "$landing" ] || exit 0"#.to_string(),
        format!(r#"tab=$(printf "{TAB}")"#),
        r#": > "$ledger.new" || exit 1"#.to_string(),
        r#"while IFS="$tab" read -r rel from at; do"#.to_string(),
        format!(r#"  if wsp_key "$rel"; then printf "%s{TAB}%s{TAB}%s{NL}" "$rel" "$from" "$at" >> "$ledger.new"; continue; fi"#),
        r#"  wsp_dest "$rel""#.to_string(),
        r#"  [ -f "$dest" ] || continue"#.to_string(),
        format!(r#"  printf "%s{TAB}%s{TAB}%s{NL}" "$rel" "$from" "$(sha256sum "$dest" | cut -d" " -f1)" >> "$ledger.new""#),
        r#"done < "$landing""#.to_string(),
        r#"[ ! -f "$ledger" ] || while IFS="$tab" read -r rel from at; do"#.to_string(),
        format!(r#"  wsp_once "$rel" || printf "%s{TAB}%s{TAB}%s{NL}" "$rel" "$from" "$at" >> "$ledger.new""#),
        r#"done < "$ledger""#.to_string(),
        format!(r#"awk -F"{TAB}" '!seen[$1]++ && $3 != "{NO_DIGEST}"' "$ledger.new" > "$ledger.keep" || exit 1"#),
        r#"mv "$ledger.keep" "$ledger""#.to_string(),
        r#"rm -f "$ledger.new""#.to_string(),
        r#"rm -rf "$stage" "$landing""#.to_string(),
        "exit 0".to_string(),
    ]);
    lines.join("\n")
}

/// provision-files.ts:515 `serverDigest`: the sha256 hex of the entry's UTF-8 bytes; `None` for no entry.
pub fn server_digest(entry: Option<&str>) -> Option<String> {
    entry.map(|text| Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect())
}

/// provision-files.ts:519 `parseLandedServers`: row id to digest; a line counts with the mark first and more than
/// two tab fields; the id is every field from the third on joined by tabs; a later line for an id overwrites the
/// value and keeps the id's first place.
pub fn parse_landed_servers(stdout: &str) -> IndexMap<String, String> {
    let mut keys = IndexMap::new();
    for line in stdout.split('\n') {
        let words: Vec<&str> = line.split('\t').collect();
        if words[0] == SERVER_MARK && words.len() > 2 {
            keys.insert(words[2..].join("\t"), words[1].to_string());
        }
    }
    keys
}

/// provision-files.ts:531 `landedServers`: empty where the run rejected or exited non-zero, and then nothing is said;
/// else says `provision_list_read_line(keys)` once.
pub async fn landed_servers(machine: &dyn Machine, home: &str, say: FilesSay) -> IndexMap<String, String> {
    let res = match machine.run(&landed_servers_script(home), RunOptions { deadline_ms: LAND_MS, ..Default::default() }).await {
        Ok(res) if res.exit_code.get() == 0 => res,
        _ => return IndexMap::new(),
    };
    let keys = parse_landed_servers(&res.stdout);
    say(&provision_list_read_line(keys.len() as f64));
    keys
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
    MachineServerPort { machine }
}

#[async_trait(?Send)]
impl ServerPort for MachineServerPort {
    /// provision-files.ts:555: `run` with deadline `LAND_MS`; `""` where it rejected or exited non-zero.
    async fn run(&self, script: &str) -> String {
        match self.machine.run(script, RunOptions { deadline_ms: LAND_MS, ..Default::default() }).await {
            Ok(res) if res.exit_code.get() == 0 => res.stdout,
            _ => String::new(),
        }
    }
    /// provision-files.ts:559: `read_configs_cmd([{files, base}])` run unlogged with deadline `READ_MS`; `None` where
    /// it rejected, exited non-zero, or the parse answered nothing; else the parse's first entry.
    async fn read(&self, files: &[String], base: &str) -> Result<Option<ScopeRead>> {
        let asked = [ReadAsk { files: files.to_vec(), base: base.to_string() }];
        let options = RunOptions { deadline_ms: READ_MS, unlogged: true, ..Default::default() };
        let res = match self.machine.run(&read_configs_cmd(&asked), options).await {
            Ok(res) if res.exit_code.get() == 0 => res,
            _ => return Ok(None),
        };
        Ok(parse_configs(&res.stdout, &asked).and_then(|scopes| scopes.into_iter().next().flatten()))
    }
    /// provision-files.ts:566: `land_configs(machine, base, [file], {file.path: text})`; its words are the error.
    async fn write(&self, file: &ScopeFile, text: &str, base: &str) -> Result<()> {
        let read = [Some(ScopeRead::File(file.clone()))];
        let texts = IndexMap::from([(file.path.clone(), text.to_string())]);
        let base = base.to_string();
        match land_configs(&*self.machine, &|_: &str| base.clone(), &read, &texts).await {
            Some(failure) => Err(EngineError::Other { message: failure, code: None }),
            None => Ok(()),
        }
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
    match out {
        ServersOut::Took { path, names } => vec![format!("{} (out of {path})", names.join(", "))],
        ServersOut::Unread { path, unread } => vec![servers_unread_line(path, unread)],
    }
}

/// provision-files.ts:586 `unmergeServers(port, home, stores = {})`.
pub async fn unmerge_servers(port: &dyn ServerPort, home: &str, stores: &IndexMap<String, String>) -> Result<Vec<ServersOut>> {
    let keys = parse_landed_servers(&port.run(&landed_servers_script(home)).await);
    let mut out = Vec::new();
    for agent in mcp_agents() {
        let Some(mcp) = &agent.mcp else {
            continue;
        };
        let mut mine: Vec<OwnKey> = Vec::new();
        for (id, digest) in &keys {
            if let Some(key) = parse_mcp_id(id)?.filter(|key| key.agent == agent.id) {
                mine.push(OwnKey { name: key.name, home: key.home, folder: key.folder, digest: digest.clone() });
            }
        }
        if mine.is_empty() {
            continue;
        }
        let config = own_server_config(&agent.id, &mcp.files, home, stores.get(&agent.id).map(String::as_str));
        let mut files: IndexMap<String, Reading> = IndexMap::new();
        reading_under(&mut files, &config.files, &config.base)
            .push(Under { project: None, keys: mine.iter().filter(|k| !k.home && k.folder.is_none()).cloned().collect() });
        reading_under(&mut files, &config.files, &config.base)
            .push(Under { project: Some(home.to_string()), keys: mine.iter().filter(|k| k.home).cloned().collect() });
        let folders: IndexSet<String> = mine.iter().filter_map(|k| k.folder.clone()).collect();
        for folder in &folders {
            let Some(carried) = launch_server_road(&agent.id).map(|road| road.carry(folder, &config.files, &config.base)) else {
                continue;
            };
            reading_under(&mut files, &carried.files, &carried.base).push(Under {
                project: carried.folder.clone(),
                keys: mine.iter().filter(|k| k.folder.as_deref() == Some(folder.as_str())).cloned().collect(),
            });
        }
        'files: for reading in files.values() {
            let Ok(read) = port.read(&reading.files, &reading.base).await else {
                continue;
            };
            let Some(read) = read else {
                continue;
            };
            let file = match read {
                ScopeRead::Unread { path, unread } => {
                    out.push(ServersOut::Unread { path, unread });
                    continue;
                }
                ScopeRead::File(file) => file,
            };
            let mut text = file.text.clone();
            let mut took: Vec<String> = Vec::new();
            for under in &reading.under {
                let names: Vec<String> = under
                    .keys
                    .iter()
                    .filter(|k| {
                        let entry = mcp.format.entry_of(&text, &k.name, under.project.as_deref());
                        server_digest(entry.as_deref()).as_deref() == Some(k.digest.as_str())
                    })
                    .map(|k| k.name.clone())
                    .collect();
                if names.is_empty() {
                    continue;
                }
                text = match mcp.format.remove(&text, &names, under.project.as_deref()) {
                    Ok(removed) => removed,
                    Err(_) => continue 'files,
                };
                took.extend(names);
            }
            if took.is_empty() || text == file.text {
                continue;
            }
            if port.write(&file, &text, &reading.base).await.is_err() {
                continue;
            }
            out.push(ServersOut::Took { path: file.path.clone(), names: took });
        }
    }
    Ok(out)
}

/// provision-files.ts:640 `ownMarks`: in the order printed; drops a line without the mark, an empty path, an
/// absolute one, or one with a `..` part.
pub fn own_marks(stdout: &str) -> Vec<String> {
    stdout
        .split('\n')
        .filter_map(|line| {
            let words: Vec<&str> = line.split('\t').collect();
            let rel = words[1..].join("\t");
            if words[0] != OWN_MARK || rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|part| part == "..") {
                return None;
            }
            Some(rel)
        })
        .collect()
}

/// provision-files.ts:802 `appendLanding`: nothing for no lines; else one inline exec, its failure swallowed.
pub async fn append_landing(machine: &dyn Machine, home: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let at = place_provision_paths(home);
    let quoted: Vec<String> = lines.iter().map(|line| shell_quote(line)).collect();
    let script =
        format!("mkdir -p {} || exit 1\nprintf '%s\\n' {} >> {}", shell_quote(&at.dir), quoted.join(" "), shell_quote(&at.landing));
    let _ = machine.exec(&script, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await;
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
    let say: FilesSay = o.say.clone().unwrap_or_else(|| Arc::new(|_: &str| {}));
    let roots = store_roots(&o.home, &o.stores);
    match files_round(machine, &o, &say, &roots).await {
        Ok(result) => result,
        Err(e) => {
            let message = e.to_string();
            let note = message.split('\n').next().unwrap_or_default().to_string();
            LandFilesResult {
                rows: o
                    .lands
                    .iter()
                    .map(|l| PlaceProvisionRow {
                        id: format!("files/{}", l.dest),
                        label: file_label(Some(l.label.as_str()), &o.home, &l.dest, &roots),
                        outcome: PlaceProvisionRowOutcome::Failed,
                        kind: Some(PlaceProvisionKind::File),
                        step: None,
                        note: Some(note.clone()),
                        fix: None,
                        keys: None,
                        ms: None,
                        earlier: None,
                        project: None,
                    })
                    .collect(),
                owned: IndexMap::new(),
                skipped: Vec::new(),
            }
        }
    }
}

type LeaveOutRequest = (Vec<StagedFile>, oneshot::Sender<Vec<String>>);

/// The pack's leave-out callback runs on this machine, which the pack's `'static` closure cannot hold: it sends each
/// request to the loop in `files_round`, which owns the machine borrow and answers.
async fn files_round(machine: &dyn Machine, o: &ProvisionFilesAsk, say: &FilesSay, roots: &StoreRoots) -> Result<LandFilesResult> {
    let once = once_paths_of(&o.lands);
    let (requests, mut asked) = mpsc::unbounded::<LeaveOutRequest>();
    let leave_out: LeaveOut = Arc::new(move |staged: Vec<StagedFile>| {
        let requests = requests.clone();
        async move {
            let (reply, answer) = oneshot::channel();
            requests.unbounded_send((staged, reply)).map_err(|_| leave_out_gone())?;
            answer.await.map_err(|_| leave_out_gone())
        }
        .boxed_local()
    });
    let mut packing = (o.pack)(Some(leave_out));
    let packed = loop {
        match select(packing, asked.next()).await {
            Either::Left((packed, _)) => break packed?,
            Either::Right((None, rest)) => break rest.await?,
            Either::Right((Some((staged, reply)), rest)) => {
                packing = rest;
                let _ = reply.send(left_out(machine, &o.home, roots, &once, &staged).await);
            }
        }
    };
    let packed_line =
        Packed { bytes: packed.bytes as f64, files: packed.files.map(|n| n as f64), stood: packed.stood.as_ref().map(|s| s.len() as f64) };
    say(&provision_packed_line(o.lands.len() as f64, &packed_line));
    let landed = land_agent_files(
        machine,
        LandAgentFiles {
            home: o.home.clone(),
            tar: packed.tar.clone(),
            lands: o.lands.clone(),
            stood: packed.stood.clone().unwrap_or_default(),
            say: say.clone(),
            roots: roots.clone(),
        },
    )
    .await?;
    let left = packed.skipped.iter().enumerate().map(|(i, s)| PlaceProvisionRow {
        id: format!("left-out/{i}"),
        label: s.path.clone(),
        outcome: PlaceProvisionRowOutcome::Skipped,
        kind: Some(PlaceProvisionKind::File),
        step: None,
        note: Some(s.note.clone()),
        fix: None,
        keys: None,
        ms: None,
        earlier: None,
        project: None,
    });
    let mut rows = landed.rows;
    rows.extend(left);
    let mut skipped = packed.skipped.clone();
    skipped.extend(landed.skipped);
    Ok(LandFilesResult { rows, owned: landed.owned, skipped })
}

fn leave_out_gone() -> EngineError {
    EngineError::Other { message: "the files round stopped answering what is left out".to_string(), code: None }
}

/// provision-files.ts:823: the paths the pack leaves out, which are the staged files whose bytes already stand there
/// and whose line in the list agrees. The read hashes under the home alone, so a file whose agent's folder moved to a
/// store always travels; a path that lands once always travels.
async fn left_out(machine: &dyn Machine, home: &str, roots: &StoreRoots, once: &[String], staged: &[StagedFile]) -> Vec<String> {
    let under_home: Vec<String> = staged.iter().filter(|f| root_of(home, roots, &f.dest).root == home).map(|f| f.dest.clone()).collect();
    let Some(standing) = standing_digests(machine, home, &under_home).await else {
        return Vec::new();
    };
    staged
        .iter()
        .filter(|f| {
            !lands_once(&f.dest, once) && standing.at.get(&f.dest) == Some(&f.digest) && standing.listed.get(&f.dest) == Some(&f.digest)
        })
        .map(|f| f.dest.clone())
        .collect()
}

/// provision-files.ts:852 `unlandScript(home, dests, roots = [])`. Byte for byte the TS's text.
pub fn unland_script(home: &str, dests: &[String], roots: &StoreRoots) -> String {
    let at = place_provision_paths(home);
    let q = |s: &str| shell_quote(s);
    let patterns: Vec<String> = dests.iter().flat_map(|d| [q(d), format!("{}/*", q(d))]).collect();
    let mut lines: Vec<String> = vec!["set -u".to_string()];
    lines.extend(dest_test(roots));
    lines.extend([
        format!("home={}; ledger={}", q(home), q(&at.landed)),
        r#"[ -f "$ledger" ] || exit 0"#.to_string(),
        format!(r#"tab=$(printf "{TAB}")"#),
        r#": > "$ledger.new" || exit 1"#.to_string(),
        r#"while IFS="$tab" read -r rel from at; do"#.to_string(),
        format!(r#"  case "$rel" in ({}) ;; (*) printf "%s{TAB}%s{TAB}%s{NL}" "$rel" "$from" "$at" >> "$ledger.new"; continue ;; esac"#, patterns.join("|")),
        r#"  case "/$rel/" in (*/../*) continue ;; esac"#.to_string(),
        r#"  wsp_dest "$rel""#.to_string(),
        r#"  if [ ! -e "$dest" ] && [ "$dest" != "$home/$rel" ]; then root=$home; sub=$rel; dest="$home/$rel"; fi"#.to_string(),
        r#"  [ -e "$dest" ] || continue"#.to_string(),
        r#"  if [ "$(readlink -f -- "$dest")" = "$(readlink -f -- "$root")/$sub" ] && [ -f "$dest" ] && [ "$(sha256sum "$dest" | cut -d" " -f1)" = "$at" ]; then"#.to_string(),
        format!(r#"    rm -f -- "$dest" && printf 'wsp-unland{TAB}gone{TAB}%s{NL}' "$rel""#),
        "  else".to_string(),
        format!(r#"    printf 'wsp-unland{TAB}kept{TAB}%s{NL}' "$rel""#),
        "  fi".to_string(),
        r#"done < "$ledger""#.to_string(),
        r#"mv "$ledger.new" "$ledger" || exit 1"#.to_string(),
    ]);
    for d in dests {
        let RootOf { root, sub, moved } = root_of(home, roots, d);
        let under: Vec<(String, String)> = if sub.is_empty() {
            Vec::new()
        } else if moved {
            vec![(root.clone(), sub.clone()), (home.to_string(), d.clone())]
        } else {
            vec![(root.clone(), sub.clone())]
        };
        for (r, s) in under {
            lines.push(format!(
                r#"[ "$(readlink -f -- {a} 2>/dev/null)" = "$(readlink -f -- {b})/"{c} ] && find {a} -depth -type d -empty -delete 2>/dev/null"#,
                a = q(&format!("{r}/{s}")),
                b = q(&r),
                c = q(&s)
            ));
        }
    }
    lines.push("exit 0".to_string());
    lines.join("\n")
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
    if dests.is_empty() {
        return Some(Unlanded::default());
    }
    let script = unland_script(home, dests, &store_roots(home, stores));
    let res = machine.exec(&script, ExecOptions { timeout_ms: Some(LAND_MS), ..Default::default() }).await.ok()?;
    if res.exit_code.get() != 0 {
        return None;
    }
    let mut out = Unlanded::default();
    for line in res.stdout.split('\n') {
        let words: Vec<&str> = line.split('\t').collect();
        if words[0] != UNLAND_MARK {
            continue;
        }
        let rest = words.iter().skip(2).copied().collect::<Vec<&str>>().join("\t");
        match words.get(1).copied() {
            Some("gone") => out.gone.push(rest),
            Some("kept") => out.kept.push(rest),
            _ => {}
        }
    }
    out.gone.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    out.kept.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    Some(out)
}

/// provision-files.ts:907 `closeAgentFiles(machine, home, once = [], stores = {})`: one run, its failure swallowed.
pub async fn close_agent_files(machine: &dyn Machine, home: &str, once: &[String], stores: &IndexMap<String, String>) {
    let script = close_files_script(home, once, &store_roots(home, stores));
    let _ = machine.run(&script, RunOptions { deadline_ms: LAND_MS, ..Default::default() }).await;
}

/// A boxed future the ledger's callers hand around.
pub type Pending<T> = LocalBoxFuture<'static, T>;

#[derive(Clone)]
struct OwnKey {
    name: String,
    home: bool,
    folder: Option<String>,
    digest: String,
}

struct Under {
    project: Option<String>,
    keys: Vec<OwnKey>,
}

struct Reading {
    files: Vec<String>,
    base: String,
    under: Vec<Under>,
}

fn reading_under<'a>(files: &'a mut IndexMap<String, Reading>, list: &[String], base: &str) -> &'a mut Vec<Under> {
    &mut files.entry(list.join("\0")).or_insert_with(|| Reading { files: list.to_vec(), base: base.to_string(), under: Vec::new() }).under
}
