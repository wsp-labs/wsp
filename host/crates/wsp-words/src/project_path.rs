// SPDX-License-Identifier: AGPL-3.0-only
//! Paths as wsp reads and writes them: a project's folder and a repo inside it, the key Claude Code files a project's
//! sessions under, the folders a browser hides or guards, and every path wsp keeps on a computer it joined. Port of
//! the functions of packages/protocol/src/project-path.ts at a3b870a6e; its flat constants are
//! wsp_wire::types::project_path. See spec/project_path.md.

use serde::Serialize;

use wsp_wire::types::daemon_contract::TOOLS_PATH;
use wsp_wire::types::docker_shim::DOCKER_NOTES_FOLDER;
use wsp_wire::types::project_path::{PLACE_HELD_LINE, PLACE_LEFT_LINE, SSH_ALIAS_PREFIX};
use wsp_wire::types::shell_quote::shell_quote;

/// Whether path is the folder itself or sits inside it; a sibling that shares the prefix is not.
// project-path.ts:16
// export function underProject(path: string, root: string): boolean {
//   return path === root || path.startsWith(`${root}/`);
// }
pub fn under_project(path: &str, root: &str) -> bool {
    path == root || path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}

// A folder git named by its real path, as cwd may name it: the path itself, then each tail of it cwd sits under, since
// a cwd reached through a link that only adds leading parts (/var for /private/var on a Mac) carries fewer of them.
// project-path.ts:22
// private in the TS: port it as a private helper
// function namesOf(real: string, cwd: string): string[] {
//   const names = [real];
//   for (let at = real.indexOf("/", 1); at > 0; at = real.indexOf("/", at + 1)) if (underProject(cwd, real.slice(at))) names.push(real.slice(at));
//   return names;
// }
fn names_of(real: &str, cwd: &str) -> Vec<String> {
    let mut names = vec![real.to_string()];
    for (at, _) in real.char_indices().skip(1).filter(|(_, c)| *c == '/') {
        if under_project(cwd, &real[at..]) {
            names.push(real[at..].to_string());
        }
    }
    names
}

/// A path an agent's tool call named, absolute or from cwd, as git names it from the top of the repo at root; undefined
/// for one outside it. An absolute path is matched under root as git names it, a relative one through cwd as namesOf
/// reads it; a relative path under a link whose name differs from the real folder's own (/home/u/work for /data/w)
/// matches nothing, and its file stays in the folder's list.
// project-path.ts:32
// export function repoPathOf(path: string, cwd: string, root: string): string | undefined {
//   const parts: string[] = [];
//   for (const part of (path.startsWith("/") ? path : `${cwd}/${path}`).split("/")) {
//     if (part === "..") parts.pop();
//     else if (part !== "" && part !== ".") parts.push(part);
//   }
//   const full = `/${parts.join("/")}`;
//   const top = namesOf(root, cwd).find(t => full.startsWith(`${t}/`));
//   return top === undefined ? undefined : full.slice(top.length + 1);
// }
pub fn repo_path_of(path: &str, cwd: &str, root: &str) -> Option<String> {
    let joined = if path.starts_with('/') { path.to_string() } else { format!("{cwd}/{path}") };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        if part == ".." {
            parts.pop();
        } else if !part.is_empty() && part != "." {
            parts.push(part);
        }
    }
    let full = format!("/{}", parts.join("/"));
    let top = names_of(root, cwd).into_iter().find(|t| full.strip_prefix(t.as_str()).is_some_and(|rest| rest.starts_with('/')))?;
    Some(full[top.len() + 1..].to_string())
}

/// The worktree cwd sits in, of the ones git lists for its repo by their real paths: the deepest, since a worktree may
/// sit inside another; undefined where cwd is in none of them.
// project-path.ts:45
// export function worktreeOf(cwd: string, worktrees: readonly string[]): string | undefined {
//   let deepest: { path: string; at: number } | undefined;
//   for (const path of worktrees) {
//     const at = Math.max(-1, ...namesOf(path, cwd).filter(name => underProject(cwd, name)).map(name => name.length));
//     if (at >= 0 && (deepest === undefined || at > deepest.at)) deepest = { path, at };
//   }
//   return deepest?.path;
// }
pub fn worktree_of(cwd: &str, worktrees: &[impl AsRef<str>]) -> Option<String> {
    let mut deepest: Option<(&str, usize)> = None;
    for path in worktrees {
        let path = path.as_ref();
        let at = names_of(path, cwd).iter().filter(|name| under_project(cwd, name)).map(|name| name.encode_utf16().count()).max();
        if let Some(at) = at {
            if deepest.is_none_or(|(_, depth)| at > depth) {
                deepest = Some((path, at));
            }
        }
    }
    deepest.map(|(path, _)| path.to_string())
}

/// The folder Claude Code keeps one project's sessions and memory under, its projects directory name: the resolved
/// path with every character outside A-Z a-z 0-9 replaced by a dash. Two paths that differ only in a character the
/// rule replaces collide under one key, which is the CLI's own behaviour and not something to work around. It sits
/// here because the collector reads it for the memory row, the runtime hands it to a launch as the project's key and
/// the engine moves a project's state by it, and none of the three may own the rule.
/// The machine's own copy of this rule is the engine's KEY_PY, for the one place nothing can run this.
// project-path.ts:60
// export const claudeProjectKey = (path: string): string => path.replace(/[^A-Za-z0-9]/g, "-");
pub fn claude_project_key(path: &str) -> String {
    path.encode_utf16()
        .map(|unit| match u8::try_from(unit) {
            Ok(byte) if byte.is_ascii_alphanumeric() => char::from(byte),
            _ => '-',
        })
        .collect()
}

/// Where Claude Code keeps one project's own memory under a state home: the keyed projects folder, and `memory`
/// inside it. Written once here: the collector reads it on this computer, the runtime binds it into a workspace at
/// the guest's state home and the engine finds it after a move.
// project-path.ts:65
// export const claudeMemoryDir = (stateHome: string, key: string): string => `${stateHome.replace(/\/+$/, "")}/projects/${key}/memory`;
pub fn claude_memory_dir(state_home: &str, key: &str) -> String {
    format!("{}/projects/{key}/memory", state_home.trim_end_matches('/'))
}

/// A folder's own name, its last segment: what a project is called, what the import dialog and the register line
/// call the folder, and what a permission prompt names a file by.
// project-path.ts:69
// export const folderName = (path: string): string => path.replace(/\/+$/, "").split("/").at(-1) ?? path;
pub fn folder_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}

/// A piece of work's name as a folder name: lowercase, every run of anything else one dash, nothing hanging off
/// either end, and cut short enough that the whole path stays typeable. A name with nothing usable in it reads as
/// `work`, so a copy always has a folder to land in.
// project-path.ts:74
// export function folderSlug(name: string): string {
//   const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40).replace(/-+$/g, "");
//   return slug === "" ? "work" : slug;
// }
pub fn folder_slug(name: &str) -> String {
    let mut dashed = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            dashed.push(c);
        } else if !dashed.ends_with('-') {
            dashed.push('-');
        }
    }
    let trimmed = dashed.trim_matches('-');
    let cut = &trimmed[..trimmed.len().min(40)];
    let slug = cut.trim_end_matches('-');
    if slug.is_empty() {
        "work".to_string()
    } else {
        slug.to_string()
    }
}

// project-path.ts:81
// in wsp_wire::types::project_path: SSH_ALIAS_PREFIX

// project-path.ts:82
// export const sshAlias = (name: string): string => `${SSH_ALIAS_PREFIX}${folderSlug(name)}`;
pub fn ssh_alias(name: &str) -> String {
    format!("{SSH_ALIAS_PREFIX}{}", folder_slug(name))
}

/// The machine a folder browser is walking, as far as the hidden rule cares: the home that machine reports, and
/// whether it is a Mac. Absent either way, the dot rule stands alone, which is every machine wsp forks.
// project-path.ts:86
// export interface FolderMachine {
//   readonly home?: string | null;
//   readonly mac?: boolean;
// }
/// `home` None is the TS null and undefined alike.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderMachine {
    pub home: Option<String>,
    pub mac: Option<bool>,
}

/// Whether a folder browser hides this folder: a dot-named one on any machine, and the Library a Mac keeps in the
/// home itself, which the Finder hides there too. The home decides and not the path's shape, since a Mac home sits
/// wherever the login puts it (a lab account under /Users/Shared holds one), and a Library somebody made inside
/// their own work is theirs. A home holds twenty of these and none is what somebody browsing for their work is
/// looking for; they are folders like any other to everything else, so a path typed or pasted whole still opens one.
// project-path.ts:96
// export function hiddenFolder(path: string, machine: FolderMachine = {}): boolean {
//   const at = path.replace(/\/+$/, "");
//   if (folderName(at).startsWith(".")) return true;
//   const home = machine.home?.replace(/\/+$/, "");
//   return machine.mac === true && home !== undefined && home !== "" && at === `${home}/Library`;
// }
pub fn hidden_folder(path: &str, machine: Option<&FolderMachine>) -> bool {
    let at = path.trim_end_matches('/');
    folder_name(at).starts_with('.') || mac_home(machine).is_some_and(|home| at == format!("{home}/Library"))
}

// The folders of a Mac home whose contents macOS keeps behind a permission prompt per program, iCloud Drive and
// the cloud folders among them, and the media folders beside them, as paths under the home. Listing one, or reading
// any path inside it, holds the read until the person answers the prompt.
// project-path.ts:106
const MAC_GUARDED: [&str; 8] =
    ["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Music", "Library/Mobile Documents", "Library/CloudStorage"];

/// The home of a Mac machine, without its trailing slashes, where the machine says it is one and names a home.
fn mac_home(machine: Option<&FolderMachine>) -> Option<&str> {
    let machine = machine?;
    let home = machine.home.as_deref()?.trim_end_matches('/');
    (machine.mac == Some(true) && !home.is_empty()).then_some(home)
}

/// Whether a folder browser stays out of this folder unless a path was typed into it: one of the folders a Mac
/// guards in its home. Shown as a row, never looked inside on its own.
// project-path.ts:110
// export function guardedFolder(path: string, machine: FolderMachine = {}): boolean {
//   const home = machine.home?.replace(/\/+$/, "");
//   if (machine.mac !== true || home === undefined || home === "") return false;
//   const at = path.replace(/\/+$/, "");
//   return MAC_GUARDED.some(name => at === `${home}/${name}`);
// }
pub fn guarded_folder(path: &str, machine: Option<&FolderMachine>) -> bool {
    let at = path.trim_end_matches('/');
    mac_home(machine).is_some_and(|home| MAC_GUARDED.iter().any(|name| at == format!("{home}/{name}")))
}

/// The name of the folder a path sits in, empty where it sits at the root or is a bare name.
// project-path.ts:118
// export const parentFolderName = (path: string): string => {
//   const parts = path.replace(/\/+$/, "").split("/");
//   return parts.length < 2 ? "" : parts[parts.length - 2]!;
// };
pub fn parent_folder_name(path: &str) -> String {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    if parts.len() < 2 {
        String::new()
    } else {
        parts[parts.len() - 2].to_string()
    }
}

/// The PATH a daemon on a computer that runs workspaces resolves a command through, and the one every script of
/// the recipe job on such a computer exports: the tools PATH with every directory under that computer's home taken
/// out, and nothing of the unit's own PATH, which is the person's login shell's and may name the home too. A
/// workspace there has the computer's home bound in read-write, so a directory under it is a directory a process
/// inside a workspace writes, and a command found through one would run as root outside every namespace. What is
/// left is the computer's own system directories, which a workspace reads through an overlay of its own or a tool
/// root bound in read-only. The daemon's twin is probe_path in the frames crate and the contract fixture holds the
/// two to one text.
// project-path.ts:131
// export function probePath(home: string): string {
//   const at = home.replace(/\/+$/, "");
//   return TOOLS_PATH.split(":").filter(dir => dir !== "" && !underProject(dir, at)).join(":");
// }
pub fn probe_path(home: &str) -> String {
    let at = home.trim_end_matches('/');
    TOOLS_PATH.split(':').filter(|dir| !dir.is_empty() && !under_project(dir, at)).collect::<Vec<_>>().join(":")
}

/// The file naming the imported project folders a daemon may browse, one absolute path per line. It sits beside the
/// home of whichever daemon reads it: DAEMON_ROOTS_PATH is this answered for a guest, whose home is /root, and this
/// computer's own daemon answers it for the person's home.
// project-path.ts:139
// export function rootsPathIn(home: string): string {
//   return `${home.replace(/\/+$/, "")}/.wsp/roots`;
// }
pub fn roots_path_in(home: &str) -> String {
    format!("{}/.wsp/roots", home.trim_end_matches('/'))
}

/// The folder every turn and every exec starts in on a computer somebody owns, wsp's own under their home. Not the
/// home itself: a turn that starts there is one `cd` from the checkouts they work in themselves, and the first build
/// thread run on a local workspace committed inside the person's own repo from there (measured 2026-09-08). One rule
/// for this computer and for a computer joined as a place, since both are somebody's own.
// project-path.ts:147
// export function workFolderIn(home: string): string {
//   return `${home.replace(/\/+$/, "")}/wsp-work`;
// }
pub fn work_folder_in(home: &str) -> String {
    format!("{}/wsp-work", home.trim_end_matches('/'))
}

/// Everywhere the daemon on a computer the person owns keeps something, whether wsp put it there over ssh or the
/// computer dialled in as a place. A machine wsp forked is root's and lays everything under /root; a computer
/// somebody already owns is reached under their own login, so every path sits in one folder of wsp's own under
/// their home and nothing needs root to write. The host's deploy builds the machine side of this and the runtime
/// reads the token and the port back off it, which is why the rule is here and in neither of them.
// project-path.ts:156
// export function placeDaemonPaths(home: string): {
//   wsp: string;
//   dir: string;
//   bundle: string;
//   inbox: string;
//   tokenPath: string;
//   /** Where the daemon writes the port it was given, the one thing the host cannot know before it is up. */
//   portFile: string;
//   /** Where a run's script, log and exit code go: wsp's own folder and not one every login on the machine shares,
//    * since another account's temporary folder is theirs and a turn that cannot write in it would launch nothing. */
//   runDir: string;
//   /** Where the parts of a file arriving over the link are appended before the whole of it is landed on a machine
//    * this computer holds. The same folder rule: wsp's own under the login's home, never one every account shares. */
//   putDir: string;
//   openSocket: string;
//   /** The socket the daemon binds for the threads running on that computer itself, the login's alone. */
//   guestSocket: string;
//   /** Where the wsp a thread running on that computer itself sits, first on its PATH: two lines onto the daemon,
//    * which dial the socket above for that thread's sessions and nothing else. The place's own folder, never
//    * `~/.wsp/bin`, where the app and the install line keep the person's wsp on the same login. */
//   guestBin: string;
//   /** Where the docker beside that wsp appends a thread's compose notes, one file per thread (`dockerShim`). */
//   dockerNotes: string;
//   manifestPath: string;
//   profileFile: string;
//   unitDir: string;
//   binDir: string;
//   /** The browser shim: what BROWSER names there, posting each page to the daemon's socket. */
//   openShim: string;
//   rootsPath: string;
//   /** What a computer joined as a place keeps beside the daemon's own files: the wsp it belongs to, the key it
//    * proves itself with, and what its agent has printed. They sit in the same folder as everything else wsp keeps
//    * there, so one sweep takes the lot. */
//   placeFile: string;
//   placeKey: string;
//   placeLog: string;
//   /** What stood before the add at the paths outside the home it or its setup writes, so a leave takes back only what
//    * wsp made: each path NUL-terminated, written by the add's deploy before anything of wsp's lands. */
//   placeFound: string;
//   /** That record while the deploy is still writing it, renamed onto it once whole. */
//   placeFoundPart: string;
//   /** The join code the add lands for the join to spend, which the join deletes before it dials. */
//   joinCode: string;
//   /** Where the install a project's landing runs there writes its output, one log per project (`placeInstallLog`). */
//   installLogs: string;
//   /** Beside the token: the daemon's minute readings, and the folder of the sshd it starts for a workspace. */
//   readings: string;
//   sshDir: string;
// } {
//   const at = home.replace(/\/+$/, "");
//   const wsp = `${at}/.wsp`;
//   return {
//     wsp,
//     dir: `${wsp}/daemon`,
//     bundle: `${wsp}/daemon.tgz`,
//     inbox: `${wsp}/inbox`,
//     tokenPath: `${wsp}/daemon-token`,
//     portFile: `${wsp}/daemon.port`,
//     runDir: `${wsp}/run`,
//     putDir: `${wsp}/put`,
//     openSocket: `${wsp}/open.sock`,
//     guestSocket: `${wsp}/daemon.sock`,
//     guestBin: `${wsp}/place-bin`,
//     dockerNotes: `${wsp}/${DOCKER_NOTES_FOLDER}`,
//     manifestPath: `${wsp}/manifest.json`,
//     profileFile: `${wsp}/profile.sh`,
//     unitDir: `${at}/.config/systemd/user`,
//     binDir: `${at}/.local/bin`,
//     openShim: `${at}/.local/bin/wsp-open`,
//     rootsPath: rootsPathIn(at),
//     placeFile: `${wsp}/place.json`,
//     placeKey: `${wsp}/place-key.pem`,
//     placeLog: `${wsp}/place.log`,
//     placeFound: `${wsp}/place-found`,
//     placeFoundPart: `${wsp}/place-found.part`,
//     joinCode: `${wsp}/join-code`,
//     installLogs: `${wsp}/install-logs`,
//     readings: `${wsp}/readings`,
//     sshDir: `${wsp}/ssh`,
//   };
// }
/// The TS object, keys in its order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceDaemonPaths {
    pub wsp: String,
    pub dir: String,
    pub bundle: String,
    pub inbox: String,
    pub token_path: String,
    pub port_file: String,
    pub run_dir: String,
    pub put_dir: String,
    pub open_socket: String,
    pub guest_socket: String,
    pub guest_bin: String,
    pub docker_notes: String,
    pub manifest_path: String,
    pub profile_file: String,
    pub unit_dir: String,
    pub bin_dir: String,
    pub open_shim: String,
    pub roots_path: String,
    pub place_file: String,
    pub place_key: String,
    pub place_log: String,
    pub place_found: String,
    pub place_found_part: String,
    pub join_code: String,
    pub install_logs: String,
    pub readings: String,
    pub ssh_dir: String,
}

pub fn place_daemon_paths(home: &str) -> PlaceDaemonPaths {
    let at = home.trim_end_matches('/');
    let wsp = format!("{at}/.wsp");
    PlaceDaemonPaths {
        dir: format!("{wsp}/daemon"),
        bundle: format!("{wsp}/daemon.tgz"),
        inbox: format!("{wsp}/inbox"),
        token_path: format!("{wsp}/daemon-token"),
        port_file: format!("{wsp}/daemon.port"),
        run_dir: format!("{wsp}/run"),
        put_dir: format!("{wsp}/put"),
        open_socket: format!("{wsp}/open.sock"),
        guest_socket: format!("{wsp}/daemon.sock"),
        guest_bin: format!("{wsp}/place-bin"),
        docker_notes: format!("{wsp}/{DOCKER_NOTES_FOLDER}"),
        manifest_path: format!("{wsp}/manifest.json"),
        profile_file: format!("{wsp}/profile.sh"),
        unit_dir: format!("{at}/.config/systemd/user"),
        bin_dir: format!("{at}/.local/bin"),
        open_shim: format!("{at}/.local/bin/wsp-open"),
        roots_path: roots_path_in(at),
        place_file: format!("{wsp}/place.json"),
        place_key: format!("{wsp}/place-key.pem"),
        place_log: format!("{wsp}/place.log"),
        place_found: format!("{wsp}/place-found"),
        place_found_part: format!("{wsp}/place-found.part"),
        join_code: format!("{wsp}/join-code"),
        install_logs: format!("{wsp}/install-logs"),
        readings: format!("{wsp}/readings"),
        ssh_dir: format!("{wsp}/ssh"),
        wsp,
    }
}

/// The login files wsp puts its one line in on that computer, the first always and the rest only where they stand:
/// `.profile`, which sh reads and bash reads where neither of the others stands, then bash's own two, since a bash
/// login reads the first of the three it finds and nothing after it (bash(1), INVOCATION). Making either of bash's
/// would hide the person's own `.profile` from their next login.
// project-path.ts:242
// export const placeLoginFiles = (home: string): string[] => {
//   const at = home.replace(/\/+$/, "");
//   return [`${at}/.profile`, `${at}/.bash_profile`, `${at}/.bash_login`];
// };
pub fn place_login_files(home: &str) -> Vec<String> {
    let at = home.trim_end_matches('/');
    vec![format!("{at}/.profile"), format!("{at}/.bash_profile"), format!("{at}/.bash_login")]
}

/// The log a project's landing on that computer writes its install's output to, which a leave takes with its folder.
// project-path.ts:248
// export const placeInstallLog = (home: string, projectId: string): string => `${placeDaemonPaths(home).installLogs}/${projectId}.log`;
pub fn place_install_log(home: &str, project_id: &str) -> String {
    format!("{}/{project_id}.log", place_daemon_paths(home).install_logs)
}

/// How much of a box's place file is read: a real one is well under a kilobyte, and the box is the untrusted side.
// project-path.ts:251
const HELD_PLACE_READ_BYTES: i64 = 65_536;

// project-path.ts:255
// in wsp_wire::types::project_path: PLACE_LEFT_LINE

// project-path.ts:260
// in wsp_wire::types::project_path: PLACE_HELD_LINE

/// The place file on a box, read over ssh before anything of wsp's lands, before a forward's new port is written
/// into it, and before a new login is kept as that box's: nothing after its own line where it holds none. Where the
/// join on that box would refuse what stands as broken (an empty or unreadable file, a dangling link, a key with no
/// file beside it) it says so, since the join only refuses once the bundle has landed. Every line exits 0.
// project-path.ts:266
// export const heldPlaceScript = (home: string): string => {
//   const at = placeDaemonPaths(home);
//   const [file, key] = [shellQuote(at.placeFile), shellQuote(at.placeKey)];
//   return [
//     `echo; echo ${PLACE_HELD_LINE}`,
//     `if [ -e ${file} ]; then { [ -f ${file} ] && [ ! -s ${file} ] && echo '${PLACE_LEFT_LINE} file'; head -c ${HELD_PLACE_READ_BYTES} ${file} 2>/dev/null || echo '${PLACE_LEFT_LINE} file'; }`,
//     `elif [ -L ${file} ]; then echo '${PLACE_LEFT_LINE} file'`,
//     `elif [ -e ${key} ] || [ -L ${key} ]; then echo '${PLACE_LEFT_LINE} key'; fi; true`,
//   ].join("\n");
// };
pub fn held_place_script(home: &str) -> String {
    let at = place_daemon_paths(home);
    let (file, key) = (shell_quote(&at.place_file), shell_quote(&at.place_key));
    [
        format!("echo; echo {PLACE_HELD_LINE}"),
        format!("if [ -e {file} ]; then {{ [ -f {file} ] && [ ! -s {file} ] && echo '{PLACE_LEFT_LINE} file'; head -c {HELD_PLACE_READ_BYTES} {file} 2>/dev/null || echo '{PLACE_LEFT_LINE} file'; }}"),
        format!("elif [ -L {file} ]; then echo '{PLACE_LEFT_LINE} file'"),
        format!("elif [ -e {key} ] || [ -L {key} ]; then echo '{PLACE_LEFT_LINE} key'; fi; true"),
    ]
    .join("\n")
}

/// What the job that puts the recipe on a computer you own keeps there: the scratch its long steps run under, the
/// log it appends a line to as it goes, and the outcome it writes at the end, which is what a person at that
/// computer's own shell reads without the host. All of it under the one folder wsp already owns there, so a
/// workspace on that computer never sees it: every workspace has its own `.wsp` bound over the computer's.
// project-path.ts:281
// export function placeProvisionPaths(home: string): { dir: string; runDir: string; log: string; result: string; staging: string; landed: string; landing: string; asked: string } {
//   const dir = `${placeDaemonPaths(home).wsp}/provision`;
//   // staging: the person's own agent files as they came off their computer, before a single one is landed.
//   // landed: what wsp put in the agents' homes there and what it left there, so a later run knows its own copy
//   // from a file the person has since written; landing is that list while the run is still going on.
//   // asked: the paths one read of what stands there was given, written down because they come over on that read's
//   // own input and a shell cannot hold them any other way; the read takes it away again.
//   return { dir, runDir: `${dir}/run`, log: `${dir}/log`, result: `${dir}/result.json`, staging: `${dir}/files`, landed: `${dir}/landed`, landing: `${dir}/landing`, asked: `${dir}/asked` };
// }
/// The TS object, keys in its order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceProvisionPaths {
    pub dir: String,
    pub run_dir: String,
    pub log: String,
    pub result: String,
    pub staging: String,
    pub landed: String,
    pub landing: String,
    pub asked: String,
}

pub fn place_provision_paths(home: &str) -> PlaceProvisionPaths {
    let dir = format!("{}/provision", place_daemon_paths(home).wsp);
    PlaceProvisionPaths {
        run_dir: format!("{dir}/run"),
        log: format!("{dir}/log"),
        result: format!("{dir}/result.json"),
        staging: format!("{dir}/files"),
        landed: format!("{dir}/landed"),
        landing: format!("{dir}/landing"),
        asked: format!("{dir}/asked"),
        dir,
    }
}

/// Every path a leave takes off a computer joined as a place, in the order they go: the place file, its key and the
/// agent's log, then everything the daemon, an installer over ssh and the recipe's job put under wsp's own folder,
/// then the browser shim and its xdg-open name. That folder itself is not here: the app and the install line keep
/// the person's own wsp in it and a host on the same login keeps its state there, so a leave takes it only once it
/// is empty, and every write under it, the daemon's or the host's over the link, needs its own row here. The work folder is not here either: what the person's threads wrote there is theirs. The daemon sweeps
/// by this list when its host asks over the link and wsp leave sweeps by it at the terminal.
// project-path.ts:297
// export function placeOwnedPaths(home: string): string[] {
//   const at = placeDaemonPaths(home);
//   return [
//     at.placeFile,
//     at.placeKey,
//     at.placeLog,
//     at.placeFound,
//     at.placeFoundPart,
//     at.joinCode,
//     at.dir,
//     placeProvisionPaths(home).dir,
//     at.bundle,
//     at.inbox,
//     at.tokenPath,
//     at.rootsPath,
//     at.profileFile,
//     at.openSocket,
//     at.guestSocket,
//     `${at.guestBin}/wsp`,
//     `${at.guestBin}/docker`,
//     at.guestBin,
//     at.dockerNotes,
//     at.runDir,
//     at.portFile,
//     at.manifestPath,
//     at.putDir,
//     at.installLogs,
//     at.readings,
//     at.sshDir,
//     at.openShim,
//     `${at.binDir}/xdg-open`,
//   ];
// }
pub fn place_owned_paths(home: &str) -> Vec<String> {
    let at = place_daemon_paths(home);
    vec![
        at.place_file.clone(),
        at.place_key.clone(),
        at.place_log.clone(),
        at.place_found.clone(),
        at.place_found_part.clone(),
        at.join_code.clone(),
        at.dir.clone(),
        place_provision_paths(home).dir,
        at.bundle.clone(),
        at.inbox.clone(),
        at.token_path.clone(),
        at.roots_path.clone(),
        at.profile_file.clone(),
        at.open_socket.clone(),
        at.guest_socket.clone(),
        format!("{}/wsp", at.guest_bin),
        format!("{}/docker", at.guest_bin),
        at.guest_bin.clone(),
        at.docker_notes.clone(),
        at.run_dir.clone(),
        at.port_file.clone(),
        at.manifest_path.clone(),
        at.put_dir.clone(),
        at.install_logs.clone(),
        at.readings.clone(),
        at.ssh_dir.clone(),
        at.open_shim.clone(),
        format!("{}/xdg-open", at.bin_dir),
    ]
}

/// What a stand-in provider keeps in the folder a harness names for it: the records every host on that state file
/// reads, so two of them see one fleet, and a folder per machine standing in for that machine's disk. The layout is
/// written here because the harness that seeds the records and the host that answers out of them both name it and
/// neither may guess.
// project-path.ts:335
// export const standInRecordsPath = (root: string): string => `${root.replace(/\/+$/, "")}/records.json`;
pub fn stand_in_records_path(root: &str) -> String {
    format!("{}/records.json", root.trim_end_matches('/'))
}

// project-path.ts:336
// export const standInMachinePath = (root: string, machineId: string): string => `${root.replace(/\/+$/, "")}/machines/${machineId}`;
pub fn stand_in_machine_path(root: &str, machine_id: &str) -> String {
    format!("{}/machines/{machine_id}", root.trim_end_matches('/'))
}
