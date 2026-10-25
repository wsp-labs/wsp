// SPDX-License-Identifier: AGPL-3.0-only
//! Unit base (spec/golden-base.md): golden-base.ts at a4033dc34, all of it. The floor every machine gets before the
//! picks, by each row's own road, and the versions read back once they are on.
use std::collections::BTreeSet;

use indexmap::{IndexMap, IndexSet};
use wsp_catalog::catalog::{base_floor, install_after, smoke_of, CatalogEntry};
use wsp_catalog::road_modules::{APT_INDEX, APT_UPDATE};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::js::is_js_whitespace;
use wsp_words::words::units::fmt_bytes;

use super::import_roads::{apt_index_step, path_line, road_word, via_road, ViaRoad};
use super::tools_disk::ALREADY_ON_MACHINE;
use super::tools_loop::install_tools_at;
use super::{GoldenBaseTool, GoldenStage, InstallToolsOptions, ToolInstall, ToolResult, ToolStage, BASE_VERSION_LINES, TOOLS_PATH};
use crate::dotfiles_presets::PRELUDE;
use crate::errors::Result;
use crate::exec_detached::INLINE_EXEC_MS;
use crate::machine::{ExecOptions, Machine};

const SSHD_THERE: &str = "test -x /usr/sbin/sshd";
const POLICY_RC: &str = "/usr/sbin/policy-rc.d";

fn with_env(cmd: &str) -> String {
    format!("{PRELUDE}\n{cmd}")
}

fn step_id(id: &str) -> String {
    format!("base/{id}")
}

/// golden-base.ts:15.
pub const PROFILE_PATH_FILE: &str = "/etc/profile.d/wsp-golden.sh";

// TS golden-base.ts:14-18
// /** Where a login shell reads the tools PATH: a thread's terminal is one, and it inherits nothing from the stages. */
// export const PROFILE_PATH_FILE = "/etc/profile.d/wsp-golden.sh";
// /** The base stage writes it on every golden, so a machine that never bootstraps Homebrew still answers `cargo`. */
// export const profilePathLine = (path: string): string => `printf '%s\\n' ${shellQuote(pathLine(path))} > ${PROFILE_PATH_FILE}`;
// export const PROFILE_PATH_LINE = profilePathLine(TOOLS_PATH);
/// golden-base.ts:17 `profilePathLine`: `printf '%s\n' <quoted path_line(path)> > /etc/profile.d/wsp-golden.sh`.
/// `PROFILE_PATH_LINE` is this at `TOOLS_PATH`.
pub fn profile_path_line(path: &str) -> String {
    format!(r"printf '%s\n' {} > {PROFILE_PATH_FILE}", shell_quote(&path_line(path, None)))
}

// TS golden-base.ts:20-53
// const BASE_STAGE: GoldenStage = "deploying-daemon";
//
// const stepId = (id: string): string => `base/${id}`;
// const APT_STEP = stepId(APT_INDEX);
//
// /** Every base step runs under the dotfiles prelude, ahead of the tools PATH the road helper puts on. */
// const withEnv = (cmd: string): string => `${PRELUDE}\n${cmd}`;
//
// /** The note above, re-exported where the floor's own readers have always found it. */
// export { ALREADY_ON_MACHINE };
//
// /** The floor as the tools loop runs it: one guarded step per row in catalog order through the row's road, each after
//  * the row the catalog says it runs on top of; the apt index is read once, before the first row that waits on it. A
//  * row `carried` names is left out, and a row that waited on one of those waits on nothing: what it needed is there.
//  * A row `left` names is not on this floor at all: a computer somebody owns takes the C toolchain only for a picked
//  * row that builds with it. */
// export function baseInstalls(carried: ReadonlySet<string> = new Set(), path: string = TOOLS_PATH, prefix?: string, left: ReadonlySet<string> = new Set()): ToolInstall[] {
//   // A thread's terminal is a login shell and the stages export their PATH per step, so without this file the
//   // terminal finds only what the image ships. Every golden gets it, whether or not Homebrew ever bootstraps.
//   // The step runs on the job's own list; what it writes into the file is the login shell's PATH on that machine,
//   // which is the tools PATH wherever a person's shell there looks for what the recipe installed.
//   const out: ToolInstall[] = [{ id: stepId("login-path"), label: "login shell PATH", manager: "script", cmd: withEnv(`${pathLine(path, prefix)}\n${PROFILE_PATH_LINE}`), shown: `the tools PATH in ${PROFILE_PATH_FILE}` }];
//   for (const e of BASE_FLOOR) {
//     if (carried.has(e.id) || left.has(e.id)) continue;
//     const dep = installAfter(e);
//     const waits = dep !== undefined && !carried.has(dep) ? dep : undefined;
//     if (waits === APT_INDEX && !out.some(t => t.id === APT_STEP)) out.push(aptIndexStep(APT_STEP, withEnv(`${pathLine(path, prefix)}\n${APT_UPDATE}`)));
//     const step = viaRoad(e.installRoad, e.bin, path, prefix);
//     if (!("cmd" in step)) throw new Error(`${e.id}: ${step.note}`);
//     out.push({ id: stepId(e.id), label: e.name, manager: e.installRoad.road, ...step, cmd: withEnv(step.cmd), ...(waits !== undefined ? { after: stepId(waits) } : {}), bin: e.bin });
//   }
//   if (!carried.has(SSH_SERVER)) out.push(sshServerStep(path, prefix, out.some(t => t.id === APT_STEP)));
//   return out;
// }
// TS golden-base.ts:55-83
// /** The ssh server wsp's daemon starts on a workspace's loopback when an editor opens it, on every golden and on every
//  * box the floor runs on, whose forks run its own /usr: a machine that has one keeps its own and its own units as they
//  * stand, and the step is not in its plan at all. Where this step installs it, the package's own
//  * service would answer on port 22, through ssh.socket on Ubuntu 24.04 whatever ssh.service says, so both are
//  * turned off here and nowhere else. It waits on the floor's index read, and reads its own where the plan has none. */
// const SSH_SERVER = "ssh-server";
// const sshServerStep = (path: string, prefix: string | undefined, indexed: boolean): ToolInstall => ({
//   id: stepId(SSH_SERVER),
//   label: "OpenSSH server",
//   manager: "apt",
//   check: SSHD_THERE,
//   // The guard is the step's own first line, beside the read that left the step out: on a machine with an sshd of
//   // its own the units below are that machine's and not this step's to turn off. The policy file keeps the package
//   // from opening port 22 while it installs, and comes off only where this step put it there.
//   cmd: withEnv(
//     [
//       pathLine(path, prefix),
//       `if ${SSHD_THERE}; then exit 0; fi`,
//       ...(indexed ? [] : [APT_UPDATE]),
//       `if [ ! -e ${POLICY_RC} ]; then printf '#!/bin/sh\\nexit 101\\n' >${POLICY_RC}; chmod 755 ${POLICY_RC}; trap 'rm -f ${POLICY_RC}' EXIT; fi`,
//       "apt-get install -y -qq openssh-server",
//       "(systemctl disable --now ssh.socket ssh.service >/dev/null 2>&1 || true)",
//     ].join("\n"),
//   ),
//   shown: "apt-get install openssh-server",
//   ...(indexed ? { after: APT_STEP } : {}),
// });
// const SSHD_THERE = "test -x /usr/sbin/sshd";
// const POLICY_RC = "/usr/sbin/policy-rc.d";
/// golden-base.ts:36 `baseInstalls(carried, path, prefix, left)`. A floor row whose road answers a note in place of a
/// line is a catalog fault: the TS throws `<id>: <note>`, and this panics with those words.
fn ssh_server_step(path: &str, prefix: Option<&str>, indexed: bool) -> ToolInstall {
    let mut lines = vec![path_line(path, prefix), format!("if {SSHD_THERE}; then exit 0; fi")];
    if !indexed {
        lines.push(APT_UPDATE.to_string());
    }
    lines.push(format!(
        r"if [ ! -e {POLICY_RC} ]; then printf '#!/bin/sh\nexit 101\n' >{POLICY_RC}; chmod 755 {POLICY_RC}; trap 'rm -f {POLICY_RC}' EXIT; fi"
    ));
    lines.push("apt-get install -y -qq openssh-server".to_string());
    lines.push("(systemctl disable --now ssh.socket ssh.service >/dev/null 2>&1 || true)".to_string());
    ToolInstall {
        id: step_id("ssh-server"),
        label: "OpenSSH server".to_string(),
        manager: "apt".to_string(),
        check: Some(SSHD_THERE.to_string()),
        cmd: with_env(&lines.join("\n")),
        shown: Some("apt-get install openssh-server".to_string()),
        after: indexed.then(|| step_id(APT_INDEX)),
        ..Default::default()
    }
}

pub fn base_installs(carried: &BTreeSet<String>, path: &str, prefix: Option<&str>, left: &BTreeSet<String>) -> Vec<ToolInstall> {
    let apt_step = step_id(APT_INDEX);
    let login_path = format!("{}\n{}", path_line(path, prefix), profile_path_line(TOOLS_PATH));
    let mut out = vec![ToolInstall {
        id: step_id("login-path"),
        label: "login shell PATH".to_string(),
        manager: "script".to_string(),
        cmd: with_env(&login_path),
        shown: Some(format!("the tools PATH in {PROFILE_PATH_FILE}")),
        ..Default::default()
    }];
    for e in base_floor() {
        if carried.contains(&e.id) || left.contains(&e.id) {
            continue;
        }
        let waits = install_after(e).filter(|dep| !carried.contains(dep));
        if waits.as_deref() == Some(APT_INDEX) && !out.iter().any(|t| t.id == apt_step) {
            out.push(apt_index_step(&apt_step, &with_env(&format!("{}\n{}", path_line(path, prefix), *APT_UPDATE))));
        }
        let (cmd, shown) = match via_road(&e.install_road, &e.bin, path, prefix) {
            ViaRoad::Step { cmd, shown } => (cmd, shown),
            ViaRoad::Note(note) => panic!("{}: {note}", e.id),
        };
        out.push(ToolInstall {
            id: step_id(&e.id),
            label: e.name.clone(),
            manager: road_word(e.install_road.name()).to_string(),
            cmd: with_env(&cmd),
            shown: Some(shown),
            after: waits.map(|w| step_id(&w)),
            bin: Some(e.bin.clone()),
            ..Default::default()
        });
    }
    if !carried.contains("ssh-server") {
        let indexed = out.iter().any(|t| t.id == apt_step);
        out.push(ssh_server_step(path, prefix, indexed));
    }
    out
}

// TS golden-base.ts:110-111
// /** The read as one exec, as the base stage runs it, on the PATH the job was planned with. */
// export const baseVersionsCmd = (path: string): string => `export PATH=${path}:$PATH\n${BASE_VERSION_LINES}`;
/// golden-base.ts:111 `baseVersionsCmd`: `export PATH=<path>:$PATH`, a newline, `BASE_VERSION_LINES`.
pub fn base_versions_cmd(path: &str) -> String {
    format!("export PATH={path}:$PATH\n{BASE_VERSION_LINES}")
}

// TS golden-base.ts:99-123
// /** The number a version line has to carry to be worth reading, as the read and the parse below each spell it in
//  * their own language: two runs of digits with a dot between them. The read keeps the first line carrying one rather
//  * than the first line, since zip prints its copyright first and `This is Zip 3.0` second, and a floor row whose
//  * version cannot be read installs on every update. */
// const VERSION_NUMBER = /\d+\.\d+(?:\.\d+)?/;
// const FIRST_VERSION_LINE = "grep -m1 -E '[0-9]+\\.[0-9]+'";
//
// /** One `VERSION <name>: <version line>` echo per floor command, an empty value for one that is not there or prints
//  * no version at all; the caller puts the tools PATH ahead. */
// export const BASE_VERSION_LINES = VERSION_CHECKS.map(c => `echo "VERSION ${c.name}: $(${c.cmd} 2>/dev/null | ${FIRST_VERSION_LINE})"`).join("\n");
//
// /** The read as one exec, as the base stage runs it, on the PATH the job was planned with. */
// export const baseVersionsCmd = (path: string): string => `export PATH=${path}:$PATH\n${BASE_VERSION_LINES}`;
//
// /** The versions the read printed, each as its number alone; a command that printed nothing, or nothing with a number in it, is left out. */
// export function parseVersions(stdout: string): GoldenBaseTool[] {
//   const out: GoldenBaseTool[] = [];
//   for (const line of stdout.split("\n")) {
//     const m = /^VERSION ([^:]+): (.*)$/.exec(line.trimEnd());
//     if (m === null) continue;
//     const version = VERSION_NUMBER.exec(m[2]!)?.[0];
//     if (version !== undefined) out.push({ name: m[1]!, version });
//   }
//   return out;
// }
/// golden-base.ts:114. Each `VERSION <name>: <line>` whose line carries `\d+\.\d+(\.\d+)?`, as that number alone;
/// lines are read with their trailing whitespace trimmed; `<name>` stops at the first colon.
fn version_number(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let run = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    for start in 0..b.len() {
        if !b[start].is_ascii_digit() || (start > 0 && b[start - 1].is_ascii_digit()) {
            continue;
        }
        let mut end = start + run(start);
        if end >= b.len() || b[end] != b'.' {
            continue;
        }
        let minor = run(end + 1);
        if minor == 0 {
            continue;
        }
        end += 1 + minor;
        if end < b.len() && b[end] == b'.' {
            let patch = run(end + 1);
            if patch > 0 {
                end += 1 + patch;
            }
        }
        return Some(s[start..end].to_string());
    }
    None
}

pub fn parse_versions(stdout: &str) -> Vec<GoldenBaseTool> {
    stdout
        .split('\n')
        .filter_map(|line| {
            let rest = wsp_words::js::js_trim_end(line).strip_prefix("VERSION ")?;
            let (name, rest) = rest.split_once(':')?;
            if name.is_empty() {
                return None;
            }
            let value = rest.strip_prefix(' ')?;
            if value.contains(['\r', '\u{2028}', '\u{2029}']) {
                return None;
            }
            let version = version_number(value)?;
            Some(GoldenBaseTool { name: name.to_string(), version })
        })
        .collect()
}

// TS golden-base.ts:125-142
// /** The floor rows the provider's image already satisfies, by entry id: every command the row promises answers with
//  * a version, and where the row pins a major that version is on it. Their installs are not idempotent on an image
//  * that ships the tool (apt-get install fails where the image's own copy stands), and an install that cannot make
//  * the row truer than it is has nothing to do. A row that pins a major still runs when the image's is another: the
//  * floor promises that major. */
// export function carriedByImage(versions: readonly GoldenBaseTool[]): Set<string> {
//   const read = new Map(versions.map(v => [v.name, v.version]));
//   const out = new Set<string>();
//   for (const e of BASE_FLOOR) {
//     const names = [e.bin, ...(e.brings ?? []).map(b => b.bin)];
//     if (!names.every(n => read.has(n))) continue;
//     const major = e.major;
//     if (major !== undefined && read.get(e.bin)!.split(".").slice(0, major.version.split(".").length).join(".") !== major.version) continue;
//     out.add(e.id);
//   }
//   if (read.has(SSHD_VERSION.name)) out.add(SSH_SERVER);
//   return out;
// }
/// golden-base.ts:130 `carriedByImage`: the floor rows whose every command answered, at the pinned major where the
/// row pins one, in catalog order; then `ssh-server` when sshd answered.
pub fn carried_by_image(versions: &[GoldenBaseTool]) -> IndexSet<String> {
    let read: IndexMap<&str, &str> = versions.iter().map(|v| (v.name.as_str(), v.version.as_str())).collect();
    let mut out = IndexSet::new();
    for e in base_floor() {
        let bins = std::iter::once(e.bin.as_str()).chain(e.brings.iter().flatten().map(|b| b.bin.as_str()));
        if !bins.clone().all(|bin| read.contains_key(bin)) {
            continue;
        }
        if let Some(major) = &e.major {
            let have = read[e.bin.as_str()];
            let parts = major.version.split('.').count();
            if have.split('.').take(parts).collect::<Vec<&str>>().join(".") != major.version {
                continue;
            }
        }
        out.insert(e.id.clone());
    }
    if read.contains_key("sshd") {
        out.insert("ssh-server".to_string());
    }
    out
}

struct VersionCheck {
    name: String,
    id: Option<String>,
}

fn version_checks() -> Vec<VersionCheck> {
    let mut out = Vec::new();
    for e in base_floor() {
        out.push(VersionCheck { name: e.bin.clone(), id: Some(e.id.clone()) });
        for b in e.brings.iter().flatten() {
            out.push(VersionCheck { name: b.bin.clone(), id: None });
        }
    }
    out.push(VersionCheck { name: "sshd".to_string(), id: None });
    out
}

// TS golden-base.ts:85-97
// interface VersionCheck {
//   /** The name the line and the machine context show. */
//   name: string;
//   cmd: string;
//   /** The floor row whose install put it there; absent for a command that rode in with another. */
//   id?: string;
// }
//
// /** The ssh server the base stage installs where the image has none, read with the floor: sshd prints its version
//  * on stderr, which the read drops, so its own shell sends it to stdout. */
// const SSHD_VERSION: VersionCheck = { name: "sshd", cmd: "sh -c 'sshd -V 2>&1'" };
//
// const VERSION_CHECKS: readonly VersionCheck[] = [...BASE_FLOOR.flatMap(e => [{ name: e.bin, cmd: smokeOf(e), id: e.id }, ...(e.brings ?? []).map(b => ({ name: b.bin, cmd: b.version }))]), SSHD_VERSION];
// TS golden-base.ts:144-155
// /** The base stage's closing words: each command with its version and, when df moved across its install, what it cost;
//  * then every floor row nothing had to be done for, then every floor row that did not land, by its reason. */
// export function versionsLine(versions: readonly GoldenBaseTool[], results: readonly ToolResult[]): string {
//   const landed = versions.map(v => {
//     const check = VERSION_CHECKS.find(c => c.name === v.name);
//     const bytes = check?.id === undefined ? undefined : results.find(r => r.id === `base/${check.id}`)?.bytes;
//     return `${v.name} ${v.version}${bytes !== undefined && bytes > 0 ? ` (${fmtBytes(bytes)})` : ""}`;
//   });
//   const already = results.filter(r => r.note === ALREADY_ON_MACHINE).map(r => r.label);
//   const missed = results.filter(r => r.id !== APT_STEP && r.outcome !== "installed").map(r => `${r.label} ${r.outcome} (${r.note})`);
//   return [landed.join(", "), ...(already.length > 0 ? [`${ALREADY_ON_MACHINE}: ${already.join(", ")}`] : []), ...missed].filter(s => s !== "").join("; ");
// }
/// golden-base.ts:146 `versionsLine`: the stage's closing words.
pub fn versions_line(versions: &[GoldenBaseTool], results: &[ToolResult]) -> String {
    let checks = version_checks();
    let landed: Vec<String> = versions
        .iter()
        .map(|v| {
            let check = checks.iter().find(|c| c.name == v.name);
            let bytes = check.and_then(|c| c.id.as_ref()).and_then(|id| results.iter().find(|r| r.id == step_id(id)).and_then(|r| r.bytes));
            let cost = match bytes {
                Some(b) if b > 0 => format!(" ({})", fmt_bytes(b as f64)),
                _ => String::new(),
            };
            format!("{} {}{cost}", v.name, v.version)
        })
        .collect();
    let already: Vec<&str> = results.iter().filter(|r| r.note.as_deref() == Some(ALREADY_ON_MACHINE)).map(|r| r.label.as_str()).collect();
    let apt_step = step_id(APT_INDEX);
    let missed = results
        .iter()
        .filter(|r| r.id != apt_step && r.outcome != "installed")
        .map(|r| format!("{} {} ({})", r.label, r.outcome, r.note.as_deref().unwrap_or("undefined")));
    let mut parts = vec![landed.join(", ")];
    if !already.is_empty() {
        parts.push(format!("{ALREADY_ON_MACHINE}: {}", already.join(", ")));
    }
    parts.extend(missed);
    parts.into_iter().filter(|s| !s.is_empty()).collect::<Vec<String>>().join("; ")
}

/// golden-base.ts:157 `BaseOutcome`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BaseOutcome {
    pub tools: Vec<ToolResult>,
    /// What the read after the installs found; the sealed version records it.
    pub versions: Vec<GoldenBaseTool>,
    pub line: String,
}

/// golden-base.ts:169's options.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InstallBaseOptions {
    /// `caches: "keep"`; false is the TS's `"sweep"` and its absent value.
    pub keep_caches: bool,
    /// `None` is `TOOLS_PATH`.
    pub path: Option<String>,
    pub prefix: Option<String>,
    pub left: BTreeSet<String>,
}

// TS golden-base.ts:165-192
// /** Installs the floor and reads the versions back; the tools loop names each failure alone and skips what waited on
//  * it. The same read runs first, against the image as the provider ships it: a row it already satisfies is recorded
//  * as on the machine, in catalog order beside the rows that ran, and nothing is installed over it. `caches` is the
//  * loop's own: a builder becomes an image and sweeps, a computer somebody owns keeps the caches that are theirs. */
// export async function installBase(machine: Machine, stage: (stage: GoldenStage, detail?: string) => void, opts: Pick<InstallToolsOptions, "caches" | "path" | "prefix"> & { left?: ReadonlySet<string> } = {}): Promise<BaseOutcome> {
//   const path = opts.path ?? TOOLS_PATH;
//   const left = opts.left ?? new Set<string>();
//   const onImage = await machine.exec(baseVersionsCmd(path), { timeoutMs: INLINE_EXEC_MS });
//   const carried = carriedByImage(parseVersions(onImage.stdout));
//   const { left: _left, ...loop } = opts;
//   const { tools } = await installTools(machine, baseInstalls(carried, path, opts.prefix, left), stage, BASE_STAGE, loop);
//   const ran = new Map(tools.map(t => [t.id, t]));
//   const floor = new Set(BASE_FLOOR.map(e => stepId(e.id)));
//   // The floor's rows read in catalog order whether they ran or the image already had them; the loop's other steps
//   // keep their place ahead of them.
//   const results: ToolResult[] = [
//     ...tools.filter(t => !floor.has(t.id)),
//     ...BASE_FLOOR.flatMap((e): ToolResult[] => {
//       if (left.has(e.id) && !carried.has(e.id)) return [];
//       if (carried.has(e.id)) return [{ id: stepId(e.id), label: e.name, outcome: "installed", note: ALREADY_ON_MACHINE }];
//       const result = ran.get(stepId(e.id));
//       return result === undefined ? [] : [result];
//     }),
//   ];
//   const read = await machine.exec(baseVersionsCmd(path), { timeoutMs: INLINE_EXEC_MS });
//   const versions = parseVersions(read.stdout);
//   return { tools: results, versions, line: versionsLine(versions, results) };
// }
/// golden-base.ts:169 `installBase(machine, stage, opts)`: the read against the image, the loop under
/// `deploying-daemon`, the read after. A rejected exec or run is the `Err`.
pub async fn install_base_with(machine: &dyn Machine, stage: ToolStage, opts: InstallBaseOptions) -> Result<BaseOutcome> {
    let path = opts.path.clone().unwrap_or_else(|| TOOLS_PATH.to_string());
    let on_image = machine.exec(&base_versions_cmd(&path), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await?;
    let carried: BTreeSet<String> = carried_by_image(&parse_versions(&on_image.stdout)).into_iter().collect();
    let installs = base_installs(&carried, &path, opts.prefix.as_deref(), &opts.left);
    let loop_opts = InstallToolsOptions {
        keep_caches: opts.keep_caches,
        path: Some(path.clone()),
        prefix: opts.prefix.clone(),
        ..Default::default()
    };
    let tools = install_tools_at(machine, &installs, stage, GoldenStage::DeployingDaemon, loop_opts).await?.tools;
    let ran: IndexMap<String, ToolResult> = tools.iter().map(|t| (t.id.clone(), t.clone())).collect();
    let floor: BTreeSet<String> = base_floor().iter().map(|e| step_id(&e.id)).collect();
    let mut results: Vec<ToolResult> = tools.iter().filter(|t| !floor.contains(&t.id)).cloned().collect();
    for e in base_floor() {
        if opts.left.contains(&e.id) && !carried.contains(&e.id) {
            continue;
        }
        if carried.contains(&e.id) {
            results.push(ToolResult {
                id: step_id(&e.id),
                label: e.name.clone(),
                outcome: "installed".to_string(),
                note: Some(ALREADY_ON_MACHINE.to_string()),
                ..Default::default()
            });
        } else if let Some(row) = ran.get(&step_id(&e.id)) {
            results.push(row.clone());
        }
    }
    let read = machine.exec(&base_versions_cmd(&path), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await?;
    let versions = parse_versions(&read.stdout);
    let line = versions_line(&versions, &results);
    Ok(BaseOutcome { tools: results, versions, line })
}

/// golden-base.ts:169 `installBase(machine, stage, { caches: "keep", path, prefix, left })`, as provision runs it.
pub async fn install_base(
    machine: &dyn Machine,
    stage: ToolStage,
    path: &str,
    prefix: Option<&str>,
    left: &BTreeSet<String>,
) -> Result<BaseOutcome> {
    install_base_with(
        machine,
        stage,
        InstallBaseOptions { keep_caches: true, path: Some(path.to_string()), prefix: prefix.map(str::to_string), left: left.clone() },
    )
    .await
}
