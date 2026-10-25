// SPDX-License-Identifier: AGPL-3.0-only
//! The words and numbers the daemon and the host agree on: the lines a computer's checks print and the scripts they run. Port of the functions of packages/protocol/src/daemon-contract.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::daemon_contract. See spec/daemon_contract.md.

// daemon-contract.ts:12
// in wsp_wire::types::daemon_contract: DAEMON_DEFAULT_HOST

// daemon-contract.ts:13
// in wsp_wire::types::daemon_contract: DAEMON_DEFAULT_PORT

// daemon-contract.ts:20
// in wsp_wire::types::daemon_contract: GUEST_WSP_HOME

// The one file the daemon reads its token from, at every auth frame; a guest is root's, so it sits under root's
// own wsp folder, where a workspace's is its own and not the computer's.
// daemon-contract.ts:23
// in wsp_wire::types::daemon_contract: DAEMON_TOKEN_PATH

// daemon-contract.ts:24
// in wsp_wire::types::daemon_contract: GUEST_INBOX_DIR

// daemon-contract.ts:25
// in wsp_wire::types::daemon_contract: GUEST_MANIFEST_PATH

// daemon-contract.ts:28
// in wsp_wire::types::daemon_contract: OPEN_SHIM_PATH

// daemon-contract.ts:29
// in wsp_wire::types::daemon_contract: XDG_OPEN_PATH

// daemon-contract.ts:33
// in wsp_wire::types::daemon_contract: WSP_WORKSPACE_APPARMOR_PATH

// daemon-contract.ts:39
// in wsp_wire::types::daemon_contract: TOOL_PREFIX

// daemon-contract.ts:44
// in wsp_wire::types::daemon_contract: RUNTIME_ROOT

// The folders of its own state the daemon makes under `RUNTIME_ROOT`, every one wsp's by its name.
// daemon-contract.ts:46
// in wsp_wire::types::daemon_contract: RUNTIME_FOLDERS

// daemon-contract.ts:49
// in wsp_wire::types::daemon_contract: RUNTIME_PROJECTS

// daemon-contract.ts:52
// in wsp_wire::types::daemon_contract: CGROUP_MOUNT

// daemon-contract.ts:53
// in wsp_wire::types::daemon_contract: WORKSPACE_CGROUPS

// daemon-contract.ts:54
// in wsp_wire::types::daemon_contract: THREAD_CGROUPS

// daemon-contract.ts:57
// in wsp_wire::types::daemon_contract: SEEDED_REFS

// daemon-contract.ts:58
// in wsp_wire::types::daemon_contract: TOOL_LINKS_DIR

// daemon-contract.ts:62
// in wsp_wire::types::daemon_contract: PLACE_FOUND_END

// daemon-contract.ts:63
// in wsp_wire::types::daemon_contract: PLACE_FOUND_MAX_BYTES

// daemon-contract.ts:66
// in wsp_wire::types::daemon_contract: GUEST_SOCKET_DIR

// Where the shim posts in the guest; root-only through the daemon's umask, unreachable from the edge.
// daemon-contract.ts:68
// in wsp_wire::types::daemon_contract: OPEN_SOCKET_PATH

// The socket a process inside a workspace on a computer somebody owns reaches its host over. The daemon of that
// computer binds one per workspace in that workspace's own wsp folder, which is bound over the folder above
// inside it, so the file is in that workspace's view and in no other's and nowhere on the computer's own. The file
// itself is the gate and no token rides this road; a fork has no such socket and dials the port instead.
// daemon-contract.ts:73
// in wsp_wire::types::daemon_contract: GUEST_DAEMON_SOCKET_PATH

// daemon-contract.ts:76
// in wsp_wire::types::daemon_contract: BROKER_PORT

// daemon-contract.ts:77
// in wsp_wire::types::daemon_contract: BROKER_SOCKET_PATH

// daemon-contract.ts:79
// in wsp_wire::types::daemon_contract: PIPE_POOL_MAX

// The longest a local connection on a door waits for a pipe before it is closed: the host away an hour.
// daemon-contract.ts:81
// in wsp_wire::types::daemon_contract: PIPE_HOLD_MAX_MS

/// The whole of the wsp a machine carries at GUEST_WSP_PATH: two lines handing the line to the binary named, which
/// opens a session on the daemon that serves this machine. The fork's deploy writes it onto the binary its bundle
/// left, and the workspace runtime writes it into a workspace's own upper onto the init already bound inside; one
/// text, so the word means the same thing on both roads.
// daemon-contract.ts:86
// export const guestWspShim = (binary: string): string => `#!/bin/sh\nexec ${binary} wsp "$@"\n`;
pub fn guest_wsp_shim(binary: &str) -> String {
    todo!()
}

// The computer's own system directories a workspace on a computer somebody owns reads through an overlay of its
// own: its /usr is the box's /usr, and what it writes there the box does not have. A directory outside these and
// outside /root is in no workspace of that computer unless a shared tool root below brings it in.
// daemon-contract.ts:91
// in wsp_wire::types::daemon_contract: WORKSPACE_OVERLAID

// daemon-contract.ts:93
// in wsp_wire::types::daemon_contract: HOMEBREW_HOME

// daemon-contract.ts:94
// in wsp_wire::types::daemon_contract: HOMEBREW_PREFIX

// Every install root a road writes outside the overlaid trees and outside /root, bound read-only into a workspace
// on a computer somebody owns where that computer has it: a root left off this list is on the box and out of every
// workspace's sight while the PATH inside names it. The catalogue's own test holds every road to it.
// daemon-contract.ts:98
// in wsp_wire::types::daemon_contract: SHARED_TOOL_ROOTS

// daemon-contract.ts:100
// in wsp_wire::types::daemon_contract: PNPM_HOME

// The one PATH the tools on a machine sit on, in one order: the login shell of a sealed image reads it from the
// profile the image writes, every thread and exec carries it, and a workspace on a computer somebody owns boots
// with it, so the boot's own children and a person's thread find the same gcc and the same gh.
// daemon-contract.ts:104
// in wsp_wire::types::daemon_contract: TOOLS_PATH

// The same directories in the order a workspace on a computer somebody owns reads them: the folders no process
// inside can write first, then the folders under the home every workspace there shares, then the computer's own
// system directories. That home is bound into every workspace read-write, so a file planted in it under the name
// of a tool the recipe installed would otherwise be what a sibling's thread, command and pane run; under this
// order the copy the recipe put under wsp's own prefix or under Homebrew's answers first. A machine wsp forked
// keeps TOOLS_PATH: its home is root's alone, nothing there shadows, and a sealed image keeps the order it was
// sealed with.
// daemon-contract.ts:113
// in wsp_wire::types::daemon_contract: PLACE_WORKSPACE_PATH

// daemon-contract.ts:116
// in wsp_wire::types::daemon_contract: PRE_AUTH_MAX_BYTES

// daemon-contract.ts:118
// in wsp_wire::types::daemon_contract: AUTH_DEADLINE_MS

// daemon-contract.ts:120
// in wsp_wire::types::daemon_contract: TUNNEL_CAP

// How much of one file fs.read carries; the whole size travels beside it.
// daemon-contract.ts:122
// in wsp_wire::types::daemon_contract: FS_READ_CAP_BYTES

// daemon-contract.ts:125
// in wsp_wire::types::daemon_contract: FS_HASH_FILES_MAX

// daemon-contract.ts:126
// in wsp_wire::types::daemon_contract: FS_HASH_CAP_BYTES

// daemon-contract.ts:127
// in wsp_wire::types::daemon_contract: FS_HASH_PATHS_MAX

// daemon-contract.ts:129
// in wsp_wire::types::daemon_contract: SSH_KEY_MAX

// How long an editor's ssh server stands once its last session closed, before it and all it started is ended.
// daemon-contract.ts:131
// in wsp_wire::types::daemon_contract: SSH_IDLE_MS

// The most one fs.write replaces a file with, the read cap's twin: a file a pane could not read whole it does not write.
// daemon-contract.ts:133
// in wsp_wire::types::daemon_contract: FS_WRITE_CAP_BYTES

// daemon-contract.ts:135
// in wsp_wire::types::daemon_contract: FS_LIST_CAP_ENTRIES

// daemon-contract.ts:137
// in wsp_wire::types::daemon_contract: FS_FILES_CAP_ENTRIES

// daemon-contract.ts:139
// in wsp_wire::types::daemon_contract: GIT_PR_LIST_CAP

// daemon-contract.ts:141
// in wsp_wire::types::daemon_contract: GIT_PR_LIST_BODY_CAP

// daemon-contract.ts:143
// in wsp_wire::types::daemon_contract: CHECK_LOG_LINES

// daemon-contract.ts:145
// in wsp_wire::types::daemon_contract: FS_SEARCH_CAP_FILES

// daemon-contract.ts:146
// in wsp_wire::types::daemon_contract: FS_SEARCH_CAP_HITS

// Bytes of patch one git.diff carries across its files, cut at a line.
// daemon-contract.ts:148
// in wsp_wire::types::daemon_contract: GIT_DIFF_CAP_BYTES

// Bytes of a pty's output kept for the next client to attach.
// daemon-contract.ts:150
// in wsp_wire::types::daemon_contract: PTY_SCROLLBACK_CAP_BYTES

// daemon-contract.ts:153
// in wsp_wire::types::daemon_contract: PROC_CMDLINE_BYTES

// daemon-contract.ts:155
// in wsp_wire::types::daemon_contract: PORT_COMMAND_BYTES

// daemon-contract.ts:157
// in wsp_wire::types::daemon_contract: PROC_CAP

// daemon-contract.ts:159
// in wsp_wire::types::daemon_contract: PID_MAX

// daemon-contract.ts:161
// in wsp_wire::types::daemon_contract: KILL_PIDS_MAX

// daemon-contract.ts:163
// in wsp_wire::types::daemon_contract: KILL_GRACE_MS

// daemon-contract.ts:167
// in wsp_wire::types::daemon_contract: APP_DATA_FOLDER

// daemon-contract.ts:168
// in wsp_wire::types::daemon_contract: APP_LOCK_FILE

// One guest message's JSON: a thread's whole transcript is the largest thing that rides the guest road.
// daemon-contract.ts:171
// in wsp_wire::types::daemon_contract: GUEST_MESSAGE_CAP_BYTES

// daemon-contract.ts:175
// in wsp_wire::types::daemon_contract: GUEST_QUEUE_CAP_FRAMES

// Guest bytes one workspace may have waiting at once, queued in its sessions and written onto the watcher's
// channel but not carried out of the socket yet. One count per workspace on a computer that runs them, and one for
// the daemon itself inside a machine, where a session names no workspace and every session shares it. A computer
// somebody joined counts the same way for the threads running on that computer itself: every one of them shares one
// count, as a machine's threads do. Four of one
// message's cap, so the largest honest thing on this road is never what fills it; a send past the cap is refused
// and its session stands.
// daemon-contract.ts:183
// in wsp_wire::types::daemon_contract: GUEST_IN_FLIGHT_CAP_BYTES

// daemon-contract.ts:190
// in wsp_wire::types::daemon_contract: GUEST_SESSIONS_PER_WORKSPACE_CAP

// The largest frame a workspace's door reads, in place of the ceiling every other socket is opened with: one
// guest message's cap and room for the envelope around it. A frame past this is refused by the framing before a
// byte of it is held or parsed, which is what makes the cap above worth having, since a frame becomes a message
// before any guest cap is read.
// daemon-contract.ts:195
// in wsp_wire::types::daemon_contract: GUEST_FRAME_CAP_BYTES

// How long a guest session stands with nobody watching it. Every watcher that arrives is told the sessions the
// machine holds, so a host that restarted picks them back up; past this span nobody is coming and the session ends
// to its guest, rather than leaving the process inside the machine waiting for the life of the workspace.
// daemon-contract.ts:199
// in wsp_wire::types::daemon_contract: GUEST_UNWATCHED_MS

// daemon-contract.ts:201
// in wsp_wire::types::daemon_contract: DAEMON_TOKEN_REFUSED

// daemon-contract.ts:202
// in wsp_wire::types::daemon_contract: DAEMON_FIRST_FRAME_NOT_AUTH

// daemon-contract.ts:203
// in wsp_wire::types::daemon_contract: DAEMON_PRE_AUTH_BYTES_EXCEEDED

// daemon-contract.ts:204
// in wsp_wire::types::daemon_contract: DAEMON_AUTH_DEADLINE_PASSED

// daemon-contract.ts:208
// in wsp_wire::types::daemon_contract: DAEMON_TOKEN_ROTATED

// daemon-contract.ts:210
// in wsp_wire::types::daemon_contract: PIPE_POOL_FULL

/// Whether a daemon's close refused the token, rather than cutting a socket whose auth frame came late: every daemon
/// closes both 4401, and a host that stood still past the deadline holds a token the daemon never read.
// daemon-contract.ts:213
// export const daemonRefusedToken = (code: number, reason: string): boolean => code === 4401 && reason !== DAEMON_AUTH_DEADLINE_PASSED;
pub fn daemon_refused_token(code: f64, reason: &str) -> bool {
    todo!()
}

// daemon-contract.ts:215
// in wsp_wire::types::daemon_contract: DAEMON_INVALID_JSON

// daemon-contract.ts:217
// in wsp_wire::types::daemon_contract: DAEMON_NO_TOKEN

// daemon-contract.ts:218
// export const unknownOpLine = (op: string): string => `unknown op: ${op}`;
pub fn unknown_op_line(op: &str) -> String {
    todo!()
}

/// What a create is refused with on a computer with no room for another workspace, the daemon's room check and the
/// Mac's copy road alike: what the kernel says is free, what the workspace needs, and the awake one to stop where
/// there is one.
// daemon-contract.ts:222
// export const boxFullLine = (needMb: number | string, freeMb: number | string, quietest?: { name: string; quietMin: number | string }): string =>
//   quietest === undefined
//     ? `this computer has ${freeMb} MB free and a workspace needs ${needMb} MB, and no workspace of yours is awake to stop: what is holding it is the computer's own work`
//     : `this computer has ${freeMb} MB free and a workspace needs ${needMb} MB; stop ${quietest.name}, quiet for ${quietest.quietMin} min, to make room`;
/// The awake workspace to stop: its name and how many minutes it has been quiet, as text (a number or a shell
/// expansion the script fills in).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quietest {
    pub name: String,
    pub quiet_min: String,
}

/// `need_mb` and `free_mb` are the TS `number | string`: written as given, a number already through `number_string`.
pub fn box_full_line(need_mb: &str, free_mb: &str, quietest: Option<&Quietest>) -> String {
    todo!()
}

/// What one workspace may take of a computer's memory, the rule of daemon/crates/wsp-runtime/src/size.rs: a third,
/// never the last gigabyte, and never under a gigabyte. The room check holds a create to it.
// daemon-contract.ts:228
// export const workspaceMemMb = (totalMb: number): number => Math.max(1024, Math.min(Math.floor(totalMb / 3), totalMb - 1024));
pub fn workspace_mem_mb(total_mb: f64) -> f64 {
    todo!()
}

/// The refusal every op but tunnel ops on the scoped port and ping gets on a socket whose auth frame named a port.
// daemon-contract.ts:230
// export const portScopeRefusal = (port: number | string): string => `this socket is scoped to port ${port}: only tunnel ops on it and ping are allowed`;
pub fn port_scope_refusal(port: &str) -> String {
    todo!()
}

// Directories a machine recreates, by exact name: installs, build output and tool caches. A directory is a cache only
// when its whole name is on this list; a file never is, whatever its name, since a source file called lruCache.ts is
// source. The list is spelled once here and read by the local walk, the guest's find and a box's repos listing.
// daemon-contract.ts:234
// in wsp_wire::types::daemon_contract: CACHE_DIRS

// daemon-contract.ts:260
// in wsp_wire::types::daemon_contract: REPO_DEPTH

// daemon-contract.ts:261
// in wsp_wire::types::daemon_contract: REPO_CAP

/// What a folder listing outside every root it browses is refused with, naming the roots it does browse. The
/// daemon of a computer somebody owns says it of that computer; the host lists its own and says it of this one.
// daemon-contract.ts:264
// export const foldersOutsideLine = (dir: string, roots: string, on = "that computer"): string => `${dir} is outside the folders wsp browses on ${on}: ${roots}`;
pub fn folders_outside_line(dir: &str, roots: &str, on: Option<&str>) -> String {
    todo!()
}

/// The one line the daemon prints on stdout once it is bound; whoever started it reads the port off this.
// daemon-contract.ts:266
// export const daemonListeningLine = (host: string, port: number | string): string => `wsp-daemon listening on ${host}:${port}`;
pub fn daemon_listening_line(host: &str, port: &str) -> String {
    todo!()
}

// daemon-contract.ts:273
// in wsp_wire::types::daemon_contract: DAEMON_SAMPLER_INTERVAL_MS

// daemon-contract.ts:277
// in wsp_wire::types::daemon_contract: DAEMON_PROC_INTERVAL_MS

// daemon-contract.ts:281
// in wsp_wire::types::daemon_contract: SYS_SAMPLER_STARTED

// daemon-contract.ts:282
// in wsp_wire::types::daemon_contract: SYS_SAMPLER_STOPPED

// daemon-contract.ts:283
// in wsp_wire::types::daemon_contract: PROC_SAMPLER_STARTED

// daemon-contract.ts:284
// in wsp_wire::types::daemon_contract: PROC_SAMPLER_STOPPED

// daemon-contract.ts:287
// in wsp_wire::types::daemon_contract: NO_PLACE_FILE_LINE

// daemon-contract.ts:288
// export const linkedLine = (url: string): string => `linked to the host at ${url}`;
pub fn linked_line(url: &str) -> String {
    todo!()
}

// daemon-contract.ts:289
// export const hostQuietLine = (url: string, seconds: number | string): string => `the host at ${url} sent nothing for ${seconds}s; cutting the link and dialling again`;
pub fn host_quiet_line(url: &str, seconds: &str) -> String {
    todo!()
}

// daemon-contract.ts:290
// export const dialUnansweredLine = (url: string): string => `${url} did not answer the dial`;
pub fn dial_unanswered_line(url: &str) -> String {
    todo!()
}

// daemon-contract.ts:291
// export const dialTimedOutLine = (url: string, seconds: number | string): string => `${url} did not answer in ${seconds}s`;
pub fn dial_timed_out_line(url: &str, seconds: &str) -> String {
    todo!()
}

// daemon-contract.ts:292
// export const dialFailedLine = (url: string, error: string): string => `${url} could not be dialled: ${error}`;
pub fn dial_failed_line(url: &str, error: &str) -> String {
    todo!()
}

// daemon-contract.ts:293
// export const notAFrameLine = (url: string): string => `${url} sent something that is not a frame`;
pub fn not_a_frame_line(url: &str) -> String {
    todo!()
}

// daemon-contract.ts:294
// export const authUnreadableLine = (url: string, error: string): string => `${url} answered place.auth with something this computer cannot read: ${error}`;
pub fn auth_unreadable_line(url: &str, error: &str) -> String {
    todo!()
}

// daemon-contract.ts:295
// export const hostRefusedLine = (url: string, refusal: string): string => `${url}: ${refusal}`;
pub fn host_refused_line(url: &str, refusal: &str) -> String {
    todo!()
}

/// What a host that answered the handshake with no key agreement of its own is passed over with: it runs a wsp
/// older than this one, and a link neither end can seal is one this computer does not hold.
// daemon-contract.ts:298
// export const linkHostUnsealedLine = (url: string): string => `the host at ${url} agreed no key for this link; it runs an older wsp`;
pub fn link_host_unsealed_line(url: &str) -> String {
    todo!()
}

/// What an address that answered something the handshake's order does not allow is passed over with: the id a frame
/// carries says nothing about who sent it, so the order is the only thing a place holds a host to before the key.
// daemon-contract.ts:301
// export const linkOutOfOrderLine = (url: string): string => `${url} answered out of order; nothing was sent to it and the next address is tried`;
pub fn link_out_of_order_line(url: &str) -> String {
    todo!()
}

// daemon-contract.ts:304
// in wsp_wire::types::daemon_contract: GUEST_NOT_WATCHER

// daemon-contract.ts:306
// in wsp_wire::types::daemon_contract: GUEST_QUEUE_FULL

// daemon-contract.ts:309
// in wsp_wire::types::daemon_contract: GUEST_IN_FLIGHT_FULL

// daemon-contract.ts:313
// in wsp_wire::types::daemon_contract: GUEST_WORKSPACE_FULL

// daemon-contract.ts:316
// in wsp_wire::types::daemon_contract: GUEST_UNWATCHED

/// Why one path a leave would have taken is still there: a folder on the way to it under the home is a link, and a
/// workspace on a computer somebody owns writes in that home, so following it would take the computer's own file of
/// that name. Said on both roads a leave runs on, and pinned to one text by the contract fixture.
// daemon-contract.ts:320
// export const placeKeptForLinkLine = (path: string): string => `nothing was removed at ${path}: a folder on the way to it is a link`;
pub fn place_kept_for_link_line(path: &str) -> String {
    todo!()
}

/// What a leave says it took out of a person's own login file: wsp's lines there, and the file. Said on both roads a
/// leave runs on, and pinned to one text by the contract fixture.
// daemon-contract.ts:323
// export const placeLineOutLine = (lines: string, file: string): string => `${lines} (out of ${file})`;
pub fn place_line_out_line(lines: &str, file: &str) -> String {
    todo!()
}

/// What a leave says of wsp's lines it left in a person's own login file, and why. Said on both roads a leave runs
/// on, and pinned to one text by the contract fixture.
// daemon-contract.ts:326
// export const placeLineStaysLine = (lines: string, file: string, why: string): string => `${lines} stays in ${file}: ${why}`;
pub fn place_line_stays_line(lines: &str, file: &str, why: &str) -> String {
    todo!()
}

// The most of a person's own login file a leave reads, far past any real one: the file sits in a home a workspace
// writes, and a leave runs as root.
// daemon-contract.ts:329
// in wsp_wire::types::daemon_contract: LOGIN_READ_MAX_BYTES

// The most of the place file or its key a read takes, far past the few hundred bytes a join writes: both sit in a
// home a workspace writes, read by the host's leave and at every one of the daemon's dials.
// daemon-contract.ts:332
// in wsp_wire::types::daemon_contract: PLACE_READ_MAX_BYTES

// daemon-contract.ts:334
// in wsp_wire::types::daemon_contract: PLACE_LOGIN_PAST_CAP

// daemon-contract.ts:337
// in wsp_wire::types::daemon_contract: PLACE_LOGIN_LINKED

/// What a leave says of a login file it did not read, since no plain file stands there. Said on both roads a leave
/// runs on, and pinned to one text by the contract fixture.
// daemon-contract.ts:340
// export const placeLoginUnreadLine = (file: string): string => `wsp's line was not looked for in ${file}: ${PLACE_LOGIN_LINKED}`;
pub fn place_login_unread_line(file: &str) -> String {
    todo!()
}

/// Why wsp's install folder is still there after a leave: the list in it of what the setup wrote outside the home
/// still has lines, which only a leave cut short leaves, and the folder holds that list for a leave that finishes.
/// Said on both roads a leave runs on, and pinned to one text by the contract fixture.
// daemon-contract.ts:344
// export const placeOutsideLeftLine = (prefix: string): string => `nothing was removed at ${prefix}: the leave did not finish taking what the setup wrote outside the home, which ${prefix}/landed still lists`;
pub fn place_outside_left_line(prefix: &str) -> String {
    todo!()
}

/// Why the runtime's folder is still there after a leave: something is mounted under it, which only a workspace still
/// running there holds, and taking the folder would reach through that mount. Said on both roads a leave runs on, and
/// pinned to one text by the contract fixture.
// daemon-contract.ts:348
// export const placeKeptMountedLine = (path: string, mount: string): string => `nothing was removed at ${path}: ${mount} is mounted under it; stop what runs there and leave again`;
pub fn place_kept_mounted_line(path: &str, mount: &str) -> String {
    todo!()
}

/// Why the runtime's folder is still there after a leave: the mount table could not be read, so whether a workspace
/// still running there reads through a mount under it is not known. Said on both roads a leave runs on, and pinned to
/// one text by the contract fixture.
// daemon-contract.ts:352
// export const placeKeptMountsUnreadLine = (path: string, why: string): string => `nothing was removed at ${path}: the mount table could not be read (${why}), so what is mounted under it is not known`;
pub fn place_kept_mounts_unread_line(path: &str, why: &str) -> String {
    todo!()
}

/// Why the runtime's folder is still there after a leave that tried to take it: something in it would not go. Said on
/// both roads a leave runs on, and pinned to one text by the contract fixture.
// daemon-contract.ts:355
// export const placeRuntimeStandsLine = (path: string): string => `${path} still stands: the leave could not remove all of it; remove what is left there by hand`;
pub fn place_runtime_stands_line(path: &str) -> String {
    todo!()
}

/// Why a cgroup wsp made is still there after a leave: a process still stands in it or under it, and the leave ends
/// none. Said on both roads a leave runs on, and pinned to one text by the contract fixture.
// daemon-contract.ts:358
// export const placeCgroupStandsLine = (path: string): string => `${path} still stands: a process is still in it; end it and remove the cgroup by hand`;
pub fn place_cgroup_stands_line(path: &str) -> String {
    todo!()
}

/// Why a leave stopped before it removed anything: checkouts under the runtime's folder hold work no remote has, one
/// line each as the daemon's read names them. Said on both roads a leave runs on, and pinned to one text by the
/// contract fixture.
// daemon-contract.ts:362
// export const placeLeaveUnsavedLine = (lines: readonly string[]): string => `nothing was removed: ${lines.join("; ")}; that work is on this computer alone`;
pub fn place_leave_unsaved_line(lines: &[impl AsRef<str>]) -> String {
    todo!()
}

/// A checkout or a workspace under the runtime's folder a leave could not read at all. Said on both roads a leave runs
/// on, and pinned to one text by the contract fixture.
// daemon-contract.ts:365
// export const placeUnreadLine = (path: string): string => `${path}: could not read what is not pushed`;
pub fn place_unread_line(path: &str) -> String {
    todo!()
}

/// What a discard or a commit naming a file git sees no change in is refused with, by the daemon and by the host's own
/// read before a question is asked.
// daemon-contract.ts:368
// export const noChangeLine = (path: string): string => `${path} has no change`;
pub fn no_change_line(path: &str) -> String {
    todo!()
}

/// Why a path outside the home a leave would have taken is still there: it stood before wsp was added, as the add
/// wrote down, so it is the computer's own. Said on both roads a leave runs on, and pinned to one text by the
/// contract fixture.
// daemon-contract.ts:372
// export const placeStoodBeforeLine = (path: string): string => `${path} stays: it was there before wsp was added`;
pub fn place_stood_before_line(path: &str) -> String {
    todo!()
}

/// Why a leave took nothing at the paths outside the home it would have: no whole record says what stood there before
/// wsp was added, so they may be the computer's own. Said on both roads a leave runs on, and pinned to one text by the
/// contract fixture.
// daemon-contract.ts:376
// export const placeOwnersUnknownLine = (paths: readonly string[]): string =>
//   `nothing was removed at ${paths.join(" or ")}: no whole record says what stood there before wsp was added, so it may be this computer's own; remove what wsp put there by hand`;
pub fn place_owners_unknown_line(paths: &[impl AsRef<str>]) -> String {
    todo!()
}

// daemon-contract.ts:380
// in wsp_wire::types::daemon_contract: HOST_CLOSED_LINE

/// What a guest process prints when nothing answers on its own machine's daemon port.
// daemon-contract.ts:382
// export const guestNoDaemonLine = (port: number | string): string => `this machine's wsp daemon is not answering on 127.0.0.1:${port}`;
pub fn guest_no_daemon_line(port: &str) -> String {
    todo!()
}

/// What a bring back on the branch the work started from is refused with: wsp makes no branch and pushes no base,
/// so the commits move onto a branch of their own first.
// daemon-contract.ts:386
// export const onBaseRefusal = (base: string): string =>
//   `this workspace is on ${base}, the branch it started from; move the commits onto a branch of their own and bring back again`;
pub fn on_base_refusal(base: &str) -> String {
    todo!()
}

// daemon-contract.ts:389
// in wsp_wire::types::daemon_contract: NOT_ON_A_BRANCH

/// What a bring back of a branch the base already holds every commit of is refused with.
// daemon-contract.ts:391
// export const nothingAheadLine = (branch: string, base: string): string => `${branch} has no commits that ${base} lacks, so there is nothing to bring back`;
pub fn nothing_ahead_line(branch: &str, base: &str) -> String {
    todo!()
}

// daemon-contract.ts:393
// in wsp_wire::types::daemon_contract: NO_REMOTE

/// What the pull request is answered with where the git host's own command line is not on the machine: the push
/// stands, so the bring back carries this beside it as a note rather than failing.
// daemon-contract.ts:396
// export const noHostCliLine = (host: string): string => `no signed-in command line for ${host} is on this computer; the branch is pushed and the pull request waits for one`;
pub fn no_host_cli_line(host: &str) -> String {
    todo!()
}

/// What a push git refused for want of an https credential is refused with: nothing reached the remote, so this is
/// the bring back's own refusal and not a note beside a landed push. The fix is the git host's own to name, since
/// only that host's module knows which command signs its command line in, and a host wsp knows no module for gets
/// the sentence with no fix in it rather than a command that would do nothing there.
// daemon-contract.ts:401
// export const noGitCredentialLine = (host: string, fix?: string): string =>
//   `this computer has no git credential for ${host}, so nothing was pushed${fix === undefined ? "" : `; ${fix}, then bring back again`}`;
pub fn no_git_credential_line(host: &str, fix: Option<&str>) -> String {
    todo!()
}

