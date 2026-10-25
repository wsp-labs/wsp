// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/daemon-contract.test.ts :315-:487 (words, numbers), :1048-:1132 (the regeneration
//! tests, the two path tests and the 4401 reasons). Skipped: :267-:313 (every op's and event's accept and reject
//! frames: zod unions and fixture files, wsp-wire's schemas), :489-:1046 (the replies: schemas and samples,
//! wsp-wire), :1116-:1119 (constants only), :1121-:1124 (reads the daemon's source: law).

#![recursion_limit = "512"]

use serde_json::{json, Map, Value};
use std::collections::HashSet;
use wsp_words::daemon_contract::*;
use wsp_words::docker_shim::docker_shim;
use wsp_words::oom::work_score_line;
use wsp_words::project_path::{place_daemon_paths, place_login_files, place_owned_paths, probe_path};
use wsp_words::words::units::NOT_ON_THIS_KIND;
use wsp_wire::types::attachments::IMAGE_MAX_BYTES;
use wsp_wire::types::daemon_contract::*;
use wsp_wire::types::oom::{DAEMON_NICE, DAEMON_OOM_SCORE_ADJ, WORK_OOM_SCORE_ADJ};
use wsp_wire::types::pull_request::PR_REPLY_BODY_MAX;
use wsp_wire::types::start::REVIEW_DIFF_MAX_BYTES;
use wsp_wire::views::session_events::{GUEST_DAEMON_DIR, GUEST_WSP_PATH};
use wsp_wire::wire::daemon::{EXEC_DEADLINE_EXIT, EXEC_OUTPUT_MAX, EXEC_TIMEOUT_DEFAULT_MS};
use wsp_wire::wire::daemon_version::{DAEMON_ROOTS_PATH, DAEMON_VERSION};
use wsp_wire::wire::limits::{EXEC_BODY_MAX, GUEST_ARGV_MAX, GUEST_CWD_MAX, GUEST_TOKEN_MAX, HTTP_URL_MAX, USAGE_STORES_MAX};
use wsp_wire::wire::machine_link::{NOT_ON_THIS_ROAD, NO_IMAGES_HERE};
use wsp_wire::wire::place_link::{host_key_refusal, PLACE_LINK_NONCE_BYTES};

const CONTRACT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../daemon/fixtures/contract");

fn committed_text(name: &str) -> Option<String> {
    std::fs::read_to_string(format!("{CONTRACT}/{name}")).ok()
}

fn regenerated(name: &str, text: &str) -> String {
    let path = std::env::temp_dir().join(format!("wsp-contract-{name}"));
    std::fs::write(&path, text).unwrap();
    path.display().to_string()
}

/// A sentence with a value in it is kept as its template: the braces name what the daemon fills in.
fn words() -> Map<String, Value> {
    let mut m = Map::new();
    let mut put = |key: &str, value: &str| {
        m.insert(key.to_string(), Value::String(value.to_string()));
    };
    put("tokenRefused", DAEMON_TOKEN_REFUSED);
    put("firstFrameNotAuth", DAEMON_FIRST_FRAME_NOT_AUTH);
    put("preAuthBytesExceeded", DAEMON_PRE_AUTH_BYTES_EXCEEDED);
    put("authDeadlinePassed", DAEMON_AUTH_DEADLINE_PASSED);
    put("tokenRotated", DAEMON_TOKEN_ROTATED);
    put("pipePoolFull", PIPE_POOL_FULL);
    put("invalidJson", DAEMON_INVALID_JSON);
    put("noToken", DAEMON_NO_TOKEN);
    put("unknownOp", &unknown_op_line("{op}"));
    put("portScopeRefusal", &port_scope_refusal("{port}"));
    put("foldersOutside", &folders_outside_line("{dir}", "{roots}", None));
    put("notOnThisRoad", NOT_ON_THIS_ROAD);
    put("notOnThisKind", NOT_ON_THIS_KIND);
    put("noImagesHere", NO_IMAGES_HERE);
    put("guestNotWatcher", GUEST_NOT_WATCHER);
    put("guestQueueFull", GUEST_QUEUE_FULL);
    put("guestInFlightFull", GUEST_IN_FLIGHT_FULL);
    put("guestWorkspaceFull", GUEST_WORKSPACE_FULL);
    put("guestUnwatched", GUEST_UNWATCHED);
    put("guestNoDaemon", &guest_no_daemon_line("{port}"));
    put("hostClosed", HOST_CLOSED_LINE);
    put("placeKeptForLink", &place_kept_for_link_line("{path}"));
    put("placeLineOut", &place_line_out_line("{lines}", "{file}"));
    put("placeLineStays", &place_line_stays_line("{lines}", "{file}", "{why}"));
    put("placeLoginPastCap", PLACE_LOGIN_PAST_CAP);
    put("placeLoginLinked", PLACE_LOGIN_LINKED);
    put("placeLoginUnread", &place_login_unread_line("{file}"));
    put("placeKeptMounted", &place_kept_mounted_line("{path}", "{mount}"));
    put("placeKeptMountsUnread", &place_kept_mounts_unread_line("{path}", "{why}"));
    put("placeRuntimeStands", &place_runtime_stands_line("{path}"));
    put("placeCgroupStands", &place_cgroup_stands_line("{path}"));
    put("placeLeaveUnsaved", &place_leave_unsaved_line(&["{lines}"]));
    put("placeUnread", &place_unread_line("{path}"));
    put("noChange", &no_change_line("{path}"));
    put("placeOutsideLeft", &place_outside_left_line("{prefix}"));
    put("placeStoodBefore", &place_stood_before_line("{path}"));
    put("placeOwnersUnknown", &place_owners_unknown_line(&["{paths}"]));
    put("onBase", &on_base_refusal("{base}"));
    put("notOnABranch", NOT_ON_A_BRANCH);
    put("nothingAhead", &nothing_ahead_line("{branch}", "{base}"));
    put("noRemote", NO_REMOTE);
    put("noHostCli", &no_host_cli_line("{host}"));
    put("noGitCredential", &no_git_credential_line("{host}", Some("{fix}")));
    put("noGitCredentialNoFix", &no_git_credential_line("{host}", None));
    put("hostKeyRefusal", &host_key_refusal("{url}"));
    put("listening", &daemon_listening_line("{host}", "{port}"));
    put("sysSamplerStarted", SYS_SAMPLER_STARTED);
    put("sysSamplerStopped", SYS_SAMPLER_STOPPED);
    put("procSamplerStarted", PROC_SAMPLER_STARTED);
    put("procSamplerStopped", PROC_SAMPLER_STOPPED);
    put("noPlaceFile", NO_PLACE_FILE_LINE);
    put("linked", &linked_line("{url}"));
    put("hostQuiet", &host_quiet_line("{url}", "{seconds}"));
    put("dialUnanswered", &dial_unanswered_line("{url}"));
    put("dialTimedOut", &dial_timed_out_line("{url}", "{seconds}"));
    put("dialFailed", &dial_failed_line("{url}", "{error}"));
    put("notAFrame", &not_a_frame_line("{url}"));
    put("authUnreadable", &auth_unreadable_line("{url}", "{error}"));
    put("hostRefused", &host_refused_line("{url}", "{refusal}"));
    put("linkOutOfOrder", &link_out_of_order_line("{url}"));
    put("linkHostUnsealed", &link_host_unsealed_line("{url}"));
    put("boxFull", &box_full_line("{need}", "{free}", Some(&Quietest { name: "{name}".to_string(), quiet_min: "{quiet}".to_string() })));
    put("boxFullOwnWork", &box_full_line("{need}", "{free}", None));
    m
}

/// The home the owned-paths fixture is rendered for: one letter, so the list reads as the shape of the paths
/// rather than as somebody's login, and the daemon's twin renders the same one.
const FIXTURE_HOME: &str = "/h";

/// The binary the wsp shim's fixture is rendered onto: the shim's one hole, kept as its template the way a
/// sentence with a value in it is, since the path differs on each road that writes the shim.
const SHIM_BINARY: &str = "{binary}";

/// The home the probe path's fixture is rendered for: root's, since every directory the tools PATH names under a
/// home is under that one and a made-up home would take nothing off the list.
const PROBE_HOME: &str = "/root";

fn numbers() -> Map<String, Value> {
    let value = json!({
        "daemonVersion": DAEMON_VERSION,
        "execBodyMax": EXEC_BODY_MAX,
        "prReplyBodyMax": PR_REPLY_BODY_MAX,
        "guestMessageCapBytes": GUEST_MESSAGE_CAP_BYTES,
        "guestQueueCapFrames": GUEST_QUEUE_CAP_FRAMES,
        "guestInFlightCapBytes": GUEST_IN_FLIGHT_CAP_BYTES,
        "guestSessionsPerWorkspaceCap": GUEST_SESSIONS_PER_WORKSPACE_CAP,
        "guestFrameCapBytes": GUEST_FRAME_CAP_BYTES,
        "guestUnwatchedMs": GUEST_UNWATCHED_MS,
        "guestTokenMax": GUEST_TOKEN_MAX,
        "guestArgvMax": GUEST_ARGV_MAX,
        "guestCwdMax": GUEST_CWD_MAX,
        "usageStoresMax": USAGE_STORES_MAX,
        "execOutputMax": EXEC_OUTPUT_MAX,
        "execTimeoutDefaultMs": EXEC_TIMEOUT_DEFAULT_MS,
        "execDeadlineExit": EXEC_DEADLINE_EXIT,
        "placeLinkNonceBytes": PLACE_LINK_NONCE_BYTES,
        "preAuthMaxBytes": PRE_AUTH_MAX_BYTES,
        "authDeadlineMs": AUTH_DEADLINE_MS,
        "tunnelCap": TUNNEL_CAP,
        "fsReadCapBytes": FS_READ_CAP_BYTES,
        // The most one fs.image carries is the cap on any image wsp takes.
        "fsImageCapBytes": IMAGE_MAX_BYTES,
        "fsHashFilesMax": FS_HASH_FILES_MAX,
        "fsHashCapBytes": FS_HASH_CAP_BYTES,
        "fsHashPathsMax": FS_HASH_PATHS_MAX,
        "fsWriteCapBytes": FS_WRITE_CAP_BYTES,
        "fsListCapEntries": FS_LIST_CAP_ENTRIES,
        "fsFilesCapEntries": FS_FILES_CAP_ENTRIES,
        "gitPrListCap": GIT_PR_LIST_CAP,
        "gitPrListBodyCap": GIT_PR_LIST_BODY_CAP,
        "checkLogLines": CHECK_LOG_LINES,
        "reviewDiffMaxBytes": REVIEW_DIFF_MAX_BYTES,
        "fsSearchCapFiles": FS_SEARCH_CAP_FILES,
        "fsSearchCapHits": FS_SEARCH_CAP_HITS,
        "gitDiffCapBytes": GIT_DIFF_CAP_BYTES,
        "ptyScrollbackCapBytes": PTY_SCROLLBACK_CAP_BYTES,
        "procCmdlineBytes": PROC_CMDLINE_BYTES,
        "portCommandBytes": PORT_COMMAND_BYTES,
        "procCap": PROC_CAP,
        "pidMax": PID_MAX,
        "killPidsMax": KILL_PIDS_MAX,
        "killGraceMs": KILL_GRACE_MS,
        "sshKeyMax": SSH_KEY_MAX,
        "sshIdleMs": SSH_IDLE_MS,
        "openBodyMax": HTTP_URL_MAX,
        "daemonDefaultHost": DAEMON_DEFAULT_HOST,
        "daemonDefaultPort": DAEMON_DEFAULT_PORT,
        "daemonSamplerIntervalMs": DAEMON_SAMPLER_INTERVAL_MS,
        "daemonProcIntervalMs": DAEMON_PROC_INTERVAL_MS,
        "guestWspHome": GUEST_WSP_HOME,
        "workspaceOverlaid": WORKSPACE_OVERLAID,
        "homebrewHome": HOMEBREW_HOME,
        "homebrewPrefix": HOMEBREW_PREFIX,
        "sharedToolRoots": SHARED_TOOL_ROOTS,
        "toolsPath": TOOLS_PATH,
        "placeWorkspacePath": PLACE_WORKSPACE_PATH,
        "daemonTokenPath": DAEMON_TOKEN_PATH,
        "daemonRootsPath": DAEMON_ROOTS_PATH,
        "guestInboxDir": GUEST_INBOX_DIR,
        "guestManifestPath": GUEST_MANIFEST_PATH,
        "openShimPath": OPEN_SHIM_PATH,
        "xdgOpenPath": XDG_OPEN_PATH,
        "workspaceApparmorPath": WSP_WORKSPACE_APPARMOR_PATH,
        "toolPrefix": TOOL_PREFIX,
        "runtimeRoot": RUNTIME_ROOT,
        "runtimeFolders": RUNTIME_FOLDERS,
        "runtimeProjects": RUNTIME_PROJECTS,
        "cgroupMount": CGROUP_MOUNT,
        "workspaceCgroups": WORKSPACE_CGROUPS,
        "threadCgroups": THREAD_CGROUPS,
        "seededRefs": SEEDED_REFS,
        "toolLinksDir": TOOL_LINKS_DIR,
        "placeFoundEnd": PLACE_FOUND_END,
        "placeFoundMaxBytes": PLACE_FOUND_MAX_BYTES,
        "loginReadMaxBytes": LOGIN_READ_MAX_BYTES,
        "placeReadMaxBytes": PLACE_READ_MAX_BYTES,
        "openSocketPath": OPEN_SOCKET_PATH,
        "guestDaemonSocketPath": GUEST_DAEMON_SOCKET_PATH,
        "brokerPort": BROKER_PORT,
        "brokerSocketPath": BROKER_SOCKET_PATH,
        "pipePoolMax": PIPE_POOL_MAX,
        "pipeHoldMaxMs": PIPE_HOLD_MAX_MS,
        "guestDaemonDir": GUEST_DAEMON_DIR,
        "guestWspPath": GUEST_WSP_PATH,
        "daemonOomScoreAdj": DAEMON_OOM_SCORE_ADJ,
        "daemonNice": DAEMON_NICE,
        "workOomScoreAdj": WORK_OOM_SCORE_ADJ,
        "workScoreLine": work_score_line(None),
        "cacheDirs": CACHE_DIRS,
        "repoDepth": REPO_DEPTH,
        "repoCap": REPO_CAP,
    });
    match value {
        Value::Object(map) => map,
        other => panic!("{other}"),
    }
}

/// One JSON fixture: the committed file read as a value equals the fresh one, and its text is the fresh one's
/// two-space layout and a closing newline.
fn json_equals_its_regeneration(name: &str, fresh: Value) {
    let text = format!("{}\n", serde_json::to_string_pretty(&fresh).unwrap());
    let regenerated = regenerated(name, &text);
    let committed: Option<Value> = committed_text(name).map(|t| serde_json::from_str(&t).unwrap());
    assert_eq!(
        committed,
        Some(fresh),
        "daemon/fixtures/contract/{name} is behind the protocol. The regenerated file is at {regenerated}: copy it over daemon/fixtures/contract/{name} and commit it"
    );
    assert_eq!(committed_text(name).unwrap(), text);
}

fn text_equals_its_regeneration(name: &str, text: &str) {
    let regenerated = regenerated(name, text);
    let committed = committed_text(name);
    assert!(
        committed.is_some(),
        "daemon/fixtures/contract/{name} is missing. The regenerated file is at {regenerated}: copy it there and commit it"
    );
    assert_eq!(
        committed.unwrap(),
        text,
        "daemon/fixtures/contract/{name} is behind the protocol. The regenerated file is at {regenerated}: copy it over and commit it"
    );
}

mod the_words_and_numbers_are_what_this_package_exports {
    use super::*;

    #[test]
    fn words_json_equals_its_regeneration() {
        json_equals_its_regeneration("words.json", Value::Object(words()));
    }

    #[test]
    fn numbers_json_equals_its_regeneration() {
        json_equals_its_regeneration("numbers.json", Value::Object(numbers()));
    }

    // The order a leave walks, which the daemon's own list is held to: the provision folder after the daemon's,
    // and never wsp's own folder, which holds the person's own wsp and a host's state on the same login.
    #[test]
    fn place_paths_json_equals_its_regeneration() {
        json_equals_its_regeneration("place-paths.json", json!(place_owned_paths(FIXTURE_HOME)));
    }

    // The login files wsp writes its line in and every leave takes it back out of, the daemon's over the link among them.
    #[test]
    fn place_login_files_json_equals_its_regeneration() {
        json_equals_its_regeneration("place-login-files.json", json!(place_login_files(FIXTURE_HOME)));
    }

    #[test]
    fn probe_path_txt_equals_its_regeneration_so_what_the_daemon_runs_a_command_through() {
        let text = format!("{}\n", probe_path(PROBE_HOME));
        text_equals_its_regeneration("probe-path.txt", &text);
    }

    #[test]
    fn guest_wsp_shim_sh_equals_its_regeneration_so_the_word_inside_a_fork_and_inside_a() {
        text_equals_its_regeneration("guest-wsp-shim.sh", &guest_wsp_shim(SHIM_BINARY));
    }

    #[test]
    fn docker_shim_sh_equals_its_regeneration_so_the_docker_on_the_host_s_computer_and() {
        text_equals_its_regeneration("docker-shim.sh", &docker_shim("{bin}", "{notes}"));
    }

    #[test]
    fn puts_every_file_the_daemon_inside_a_machine_writes_for_itself_under_one_folder_b() {
        // A workspace on a computer somebody joined has this folder of its own bound over the computer's, so a path
        // that slipped out of it would be written into a /root every workspace there shares: the second workspace's
        // deploy would rewrite the first one's token.
        for path in [DAEMON_TOKEN_PATH, GUEST_INBOX_DIR, GUEST_MANIFEST_PATH, DAEMON_ROOTS_PATH, GUEST_DAEMON_SOCKET_PATH] {
            assert!(path.starts_with(&format!("{GUEST_WSP_HOME}/")), "{path}");
        }
        // And they are the names a daemon uses under the home of a computer somebody joined: one daemon, one rule.
        let at = place_daemon_paths("/root");
        assert_eq!(
            json!({ "wsp": at.wsp, "token": at.token_path, "inbox": at.inbox, "manifest": at.manifest_path, "roots": at.roots_path }),
            json!({
                "wsp": GUEST_WSP_HOME,
                "token": DAEMON_TOKEN_PATH,
                "inbox": GUEST_INBOX_DIR,
                "manifest": GUEST_MANIFEST_PATH,
                "roots": DAEMON_ROOTS_PATH,
            })
        );
        // The binary the host deploys is not one of them: it is the host's to land and sits beside the folder.
        assert!(!GUEST_DAEMON_DIR.starts_with(GUEST_WSP_HOME));
    }

    #[test]
    fn holds_every_4401_reason_once_and_the_listening_line_names_a_host_and_a_port() {
        let w = words();
        let reasons: HashSet<&str> = ["tokenRefused", "firstFrameNotAuth", "preAuthBytesExceeded", "authDeadlinePassed"]
            .iter()
            .map(|k| w[*k].as_str().unwrap())
            .collect();
        assert_eq!(reasons.len(), 4);
        assert_eq!(w["listening"], "wsp-daemon listening on {host}:{port}");
        assert_eq!(w["unknownOp"], "unknown op: {op}");
    }
}
