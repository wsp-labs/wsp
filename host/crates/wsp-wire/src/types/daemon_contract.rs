// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/daemon-contract.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/daemon-contract.ts:12 at a3b870a6e.
pub const DAEMON_DEFAULT_HOST: &str = "0.0.0.0";

/// TS: packages/protocol/src/daemon-contract.ts:13 at a3b870a6e.
pub const DAEMON_DEFAULT_PORT: u64 = 7_070;

/// TS: packages/protocol/src/daemon-contract.ts:20 at a3b870a6e.
pub const GUEST_WSP_HOME: &str = "/root/.wsp";

/// TS: packages/protocol/src/daemon-contract.ts:23 at a3b870a6e.
pub const DAEMON_TOKEN_PATH: &str = "/root/.wsp/daemon-token";

/// TS: packages/protocol/src/daemon-contract.ts:24 at a3b870a6e.
pub const GUEST_INBOX_DIR: &str = "/root/.wsp/inbox";

/// TS: packages/protocol/src/daemon-contract.ts:25 at a3b870a6e.
pub const GUEST_MANIFEST_PATH: &str = "/root/.wsp/manifest.json";

/// TS: packages/protocol/src/daemon-contract.ts:28 at a3b870a6e.
pub const OPEN_SHIM_PATH: &str = "/usr/local/bin/wsp-open";

/// TS: packages/protocol/src/daemon-contract.ts:29 at a3b870a6e.
pub const XDG_OPEN_PATH: &str = "/usr/local/bin/xdg-open";

/// TS: packages/protocol/src/daemon-contract.ts:33 at a3b870a6e.
pub const WSP_WORKSPACE_APPARMOR_PATH: &str = "/etc/apparmor.d/wsp-workspace";

/// TS: packages/protocol/src/daemon-contract.ts:39 at a3b870a6e.
pub const TOOL_PREFIX: &str = "/opt/wsp";

/// TS: packages/protocol/src/daemon-contract.ts:44 at a3b870a6e.
pub const RUNTIME_ROOT: &str = "/wsp";

/// TS: packages/protocol/src/daemon-contract.ts:46 at a3b870a6e.
pub const RUNTIME_FOLDERS: &[&str] = &["check", "copies", "logins", "put", "run", "state"];

/// TS: packages/protocol/src/daemon-contract.ts:49 at a3b870a6e.
pub const RUNTIME_PROJECTS: &str = "projects";

/// TS: packages/protocol/src/daemon-contract.ts:52 at a3b870a6e.
pub const CGROUP_MOUNT: &str = "/sys/fs/cgroup";

/// TS: packages/protocol/src/daemon-contract.ts:53 at a3b870a6e.
pub const WORKSPACE_CGROUPS: &str = "/wsp";

/// TS: packages/protocol/src/daemon-contract.ts:54 at a3b870a6e.
pub const THREAD_CGROUPS: &str = "/wsp-threads";

/// TS: packages/protocol/src/daemon-contract.ts:57 at a3b870a6e.
pub const SEEDED_REFS: &str = "refs/wsp/seeded";

/// TS: packages/protocol/src/daemon-contract.ts:58 at a3b870a6e.
pub const TOOL_LINKS_DIR: &str = "/usr/local/bin";

/// TS: packages/protocol/src/daemon-contract.ts:62 at a3b870a6e.
pub const PLACE_FOUND_END: &str = "wsp-found-end";

/// TS: packages/protocol/src/daemon-contract.ts:63 at a3b870a6e.
pub const PLACE_FOUND_MAX_BYTES: u64 = 33_554_432;

/// TS: packages/protocol/src/daemon-contract.ts:66 at a3b870a6e.
pub const GUEST_SOCKET_DIR: &str = "/run/wsp-daemon";

/// TS: packages/protocol/src/daemon-contract.ts:68 at a3b870a6e.
pub const OPEN_SOCKET_PATH: &str = "/run/wsp-daemon/open.sock";

/// TS: packages/protocol/src/daemon-contract.ts:73 at a3b870a6e.
pub const GUEST_DAEMON_SOCKET_PATH: &str = "/root/.wsp/daemon.sock";

/// TS: packages/protocol/src/daemon-contract.ts:76 at a3b870a6e.
pub const BROKER_PORT: u64 = 7_071;

/// TS: packages/protocol/src/daemon-contract.ts:77 at a3b870a6e.
pub const BROKER_SOCKET_PATH: &str = "/run/wsp-daemon/broker.sock";

/// TS: packages/protocol/src/daemon-contract.ts:79 at a3b870a6e.
pub const PIPE_POOL_MAX: u64 = 64;

/// TS: packages/protocol/src/daemon-contract.ts:81 at a3b870a6e.
pub const PIPE_HOLD_MAX_MS: u64 = 3_600_000;

/// TS: packages/protocol/src/daemon-contract.ts:91 at a3b870a6e.
pub const WORKSPACE_OVERLAID: &[&str] = &["/usr", "/etc", "/opt", "/var", "/srv"];

/// TS: packages/protocol/src/daemon-contract.ts:93 at a3b870a6e.
pub const HOMEBREW_HOME: &str = "/home/linuxbrew";

/// TS: packages/protocol/src/daemon-contract.ts:94 at a3b870a6e.
pub const HOMEBREW_PREFIX: &str = "/home/linuxbrew/.linuxbrew";

/// TS: packages/protocol/src/daemon-contract.ts:98 at a3b870a6e.
pub const SHARED_TOOL_ROOTS: &[&str] = &["/home/linuxbrew"];

/// TS: packages/protocol/src/daemon-contract.ts:100 at a3b870a6e.
pub const PNPM_HOME: &str = "/root/.local/share/pnpm";

/// TS: packages/protocol/src/daemon-contract.ts:104 at a3b870a6e.
pub const TOOLS_PATH: &str = "/root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// TS: packages/protocol/src/daemon-contract.ts:113 at a3b870a6e.
pub const PLACE_WORKSPACE_PATH: &str = "/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/.local/bin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// TS: packages/protocol/src/daemon-contract.ts:116 at a3b870a6e.
pub const PRE_AUTH_MAX_BYTES: u64 = 4_096;

/// TS: packages/protocol/src/daemon-contract.ts:118 at a3b870a6e.
pub const AUTH_DEADLINE_MS: u64 = 5_000;

/// TS: packages/protocol/src/daemon-contract.ts:120 at a3b870a6e.
pub const TUNNEL_CAP: u64 = 64;

/// TS: packages/protocol/src/daemon-contract.ts:122 at a3b870a6e.
pub const FS_READ_CAP_BYTES: u64 = 2_097_152;

/// TS: packages/protocol/src/daemon-contract.ts:125 at a3b870a6e.
pub const FS_HASH_FILES_MAX: u64 = 32;

/// TS: packages/protocol/src/daemon-contract.ts:126 at a3b870a6e.
pub const FS_HASH_CAP_BYTES: u64 = 4_194_304;

/// TS: packages/protocol/src/daemon-contract.ts:127 at a3b870a6e.
pub const FS_HASH_PATHS_MAX: u64 = 256;

/// TS: packages/protocol/src/daemon-contract.ts:129 at a3b870a6e.
pub const SSH_KEY_MAX: u64 = 1_024;

/// TS: packages/protocol/src/daemon-contract.ts:131 at a3b870a6e.
pub const SSH_IDLE_MS: u64 = 300_000;

/// TS: packages/protocol/src/daemon-contract.ts:133 at a3b870a6e.
pub const FS_WRITE_CAP_BYTES: u64 = 2_097_152;

/// TS: packages/protocol/src/daemon-contract.ts:135 at a3b870a6e.
pub const FS_LIST_CAP_ENTRIES: u64 = 10_000;

/// TS: packages/protocol/src/daemon-contract.ts:137 at a3b870a6e.
pub const FS_FILES_CAP_ENTRIES: u64 = 20_000;

/// TS: packages/protocol/src/daemon-contract.ts:139 at a3b870a6e.
pub const GIT_PR_LIST_CAP: u64 = 50;

/// TS: packages/protocol/src/daemon-contract.ts:141 at a3b870a6e.
pub const GIT_PR_LIST_BODY_CAP: u64 = 4_000;

/// TS: packages/protocol/src/daemon-contract.ts:143 at a3b870a6e.
pub const CHECK_LOG_LINES: u64 = 300;

/// TS: packages/protocol/src/daemon-contract.ts:145 at a3b870a6e.
pub const FS_SEARCH_CAP_FILES: u64 = 5_000;

/// TS: packages/protocol/src/daemon-contract.ts:146 at a3b870a6e.
pub const FS_SEARCH_CAP_HITS: u64 = 500;

/// TS: packages/protocol/src/daemon-contract.ts:148 at a3b870a6e.
pub const GIT_DIFF_CAP_BYTES: u64 = 2_097_152;

/// TS: packages/protocol/src/daemon-contract.ts:150 at a3b870a6e.
pub const PTY_SCROLLBACK_CAP_BYTES: u64 = 262_144;

/// TS: packages/protocol/src/daemon-contract.ts:153 at a3b870a6e.
pub const PROC_CMDLINE_BYTES: u64 = 200;

/// TS: packages/protocol/src/daemon-contract.ts:155 at a3b870a6e.
pub const PORT_COMMAND_BYTES: u64 = 512;

/// TS: packages/protocol/src/daemon-contract.ts:157 at a3b870a6e.
pub const PROC_CAP: u64 = 1_000;

/// TS: packages/protocol/src/daemon-contract.ts:159 at a3b870a6e.
pub const PID_MAX: u64 = 4_194_304;

/// TS: packages/protocol/src/daemon-contract.ts:161 at a3b870a6e.
pub const KILL_PIDS_MAX: u64 = 256;

/// TS: packages/protocol/src/daemon-contract.ts:163 at a3b870a6e.
pub const KILL_GRACE_MS: u64 = 2_000;

/// TS: packages/protocol/src/daemon-contract.ts:167 at a3b870a6e.
pub const APP_DATA_FOLDER: &str = "desktop";

/// TS: packages/protocol/src/daemon-contract.ts:168 at a3b870a6e.
pub const APP_LOCK_FILE: &str = "SingletonLock";

/// TS: packages/protocol/src/daemon-contract.ts:171 at a3b870a6e.
pub const GUEST_MESSAGE_CAP_BYTES: u64 = 4_194_304;

/// TS: packages/protocol/src/daemon-contract.ts:175 at a3b870a6e.
pub const GUEST_QUEUE_CAP_FRAMES: u64 = 256;

/// TS: packages/protocol/src/daemon-contract.ts:183 at a3b870a6e.
pub const GUEST_IN_FLIGHT_CAP_BYTES: u64 = 16_777_216;

/// TS: packages/protocol/src/daemon-contract.ts:190 at a3b870a6e.
pub const GUEST_SESSIONS_PER_WORKSPACE_CAP: u64 = 64;

/// TS: packages/protocol/src/daemon-contract.ts:195 at a3b870a6e.
pub const GUEST_FRAME_CAP_BYTES: u64 = 4_259_840;

/// TS: packages/protocol/src/daemon-contract.ts:199 at a3b870a6e.
pub const GUEST_UNWATCHED_MS: u64 = 600_000;

/// TS: packages/protocol/src/daemon-contract.ts:201 at a3b870a6e.
pub const DAEMON_TOKEN_REFUSED: &str = "daemon token refused; the host holds the current one";

/// TS: packages/protocol/src/daemon-contract.ts:202 at a3b870a6e.
pub const DAEMON_FIRST_FRAME_NOT_AUTH: &str = "the first frame must be auth";

/// TS: packages/protocol/src/daemon-contract.ts:203 at a3b870a6e.
pub const DAEMON_PRE_AUTH_BYTES_EXCEEDED: &str = "too many bytes before the auth frame";

/// TS: packages/protocol/src/daemon-contract.ts:204 at a3b870a6e.
pub const DAEMON_AUTH_DEADLINE_PASSED: &str = "no auth frame arrived in time";

/// TS: packages/protocol/src/daemon-contract.ts:208 at a3b870a6e.
pub const DAEMON_TOKEN_ROTATED: &str = "the daemon token was rotated; dial again with the current one";

/// TS: packages/protocol/src/daemon-contract.ts:210 at a3b870a6e.
pub const PIPE_POOL_FULL: &str = "pipe pool full";

/// TS: packages/protocol/src/daemon-contract.ts:215 at a3b870a6e.
pub const DAEMON_INVALID_JSON: &str = "invalid json";

/// TS: packages/protocol/src/daemon-contract.ts:217 at a3b870a6e.
pub const DAEMON_NO_TOKEN: &str = "daemon refuses to start without an auth token";

/// TS: packages/protocol/src/daemon-contract.ts:234 at a3b870a6e.
/// A Set in the TS, in insertion order.
pub const CACHE_DIRS: &[&str] = &["node_modules", ".pnpm-store", "venv", ".venv", "virtenv", "site-packages", "__pycache__", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".tox", "dist", "build", "out", "target", "coverage", ".cache", ".parcel-cache", ".next", ".nuxt", ".turbo", ".gradle"];

/// TS: packages/protocol/src/daemon-contract.ts:260 at a3b870a6e.
pub const REPO_DEPTH: u64 = 5;

/// TS: packages/protocol/src/daemon-contract.ts:261 at a3b870a6e.
pub const REPO_CAP: u64 = 400;

/// TS: packages/protocol/src/daemon-contract.ts:273 at a3b870a6e.
pub const DAEMON_SAMPLER_INTERVAL_MS: u64 = 2_000;

/// TS: packages/protocol/src/daemon-contract.ts:277 at a3b870a6e.
pub const DAEMON_PROC_INTERVAL_MS: u64 = 2_000;

/// TS: packages/protocol/src/daemon-contract.ts:281 at a3b870a6e.
pub const SYS_SAMPLER_STARTED: &str = "sys sampler started";

/// TS: packages/protocol/src/daemon-contract.ts:282 at a3b870a6e.
pub const SYS_SAMPLER_STOPPED: &str = "sys sampler stopped";

/// TS: packages/protocol/src/daemon-contract.ts:283 at a3b870a6e.
pub const PROC_SAMPLER_STARTED: &str = "proc sampler started";

/// TS: packages/protocol/src/daemon-contract.ts:284 at a3b870a6e.
pub const PROC_SAMPLER_STOPPED: &str = "proc sampler stopped";

/// TS: packages/protocol/src/daemon-contract.ts:287 at a3b870a6e.
pub const NO_PLACE_FILE_LINE: &str = "no place file here, so there is no host to dial; wsp join <address> --code <code> makes this computer a place";

/// TS: packages/protocol/src/daemon-contract.ts:304 at a3b870a6e.
pub const GUEST_NOT_WATCHER: &str = "only the socket that sent guest.watch may answer or close a guest session";

/// TS: packages/protocol/src/daemon-contract.ts:306 at a3b870a6e.
pub const GUEST_QUEUE_FULL: &str = "the host has not read this session for too long";

/// TS: packages/protocol/src/daemon-contract.ts:309 at a3b870a6e.
pub const GUEST_IN_FLIGHT_FULL: &str = "too many guest bytes are waiting to be read here; send this one again";

/// TS: packages/protocol/src/daemon-contract.ts:313 at a3b870a6e.
pub const GUEST_WORKSPACE_FULL: &str = "this folder already holds as many guest sessions as it may; end one and run it again";

/// TS: packages/protocol/src/daemon-contract.ts:316 at a3b870a6e.
pub const GUEST_UNWATCHED: &str = "the host stopped watching; run it again";

/// TS: packages/protocol/src/daemon-contract.ts:329 at a3b870a6e.
pub const LOGIN_READ_MAX_BYTES: u64 = 1_048_576;

/// TS: packages/protocol/src/daemon-contract.ts:332 at a3b870a6e.
pub const PLACE_READ_MAX_BYTES: u64 = 65_536;

/// TS: packages/protocol/src/daemon-contract.ts:334 at a3b870a6e.
pub const PLACE_LOGIN_PAST_CAP: &str = "it is over 1 MiB, more than a leave reads of a login file";

/// TS: packages/protocol/src/daemon-contract.ts:337 at a3b870a6e.
pub const PLACE_LOGIN_LINKED: &str = "it is a link, has a second name or is not a plain file, and wsp writes through none of those in a login file";

/// TS: packages/protocol/src/daemon-contract.ts:380 at a3b870a6e.
pub const HOST_CLOSED_LINE: &str = "the host closed the connection";

/// TS: packages/protocol/src/daemon-contract.ts:389 at a3b870a6e.
pub const NOT_ON_A_BRANCH: &str = "this workspace is not on a branch, so there is nothing to bring back yet";

/// TS: packages/protocol/src/daemon-contract.ts:393 at a3b870a6e.
pub const NO_REMOTE: &str = "this project has no remote to push to";
