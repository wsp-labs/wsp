// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/golden-base.test.ts at a4033dc34, and the first case of provision-floor.test.ts: the
//! base floor's plan, the download that fails, the versions read, and installBase against a guest that answers df.
mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::{ok, res, ExecFn, FakeMachine, RunFn};
use wsp_catalog::catalog::{base_floor, install_after, COMPILER_ROW};
use wsp_catalog::road_modules::{road_step, APT_INDEX, CURL_NET};
use wsp_catalog::roads::{RoadName, UV_INSTALL};
use wsp_engine::golden_seam::{
    base_installs, base_versions_cmd, carried_by_image, free_kb_cmd, guarded_road, install_base_with, parse_versions, reason_of,
    road_limit_s, versions_line, BaseOutcome, InstallBaseOptions, ToolInstall, ToolResult, ToolStage, ALREADY_ON_MACHINE, TOOLS_PATH,
};
use wsp_engine::machine::ExecResult;
use wsp_wire::views::golden_image::GoldenBaseTool;

/// dotfiles-presets.ts:5 `PRELUDE`.
// Adapted: wsp_engine::dotfiles_presets holds no PRELUDE yet, so its text is spelled here.
const PRELUDE: &str = "set -euo pipefail\nexport HOME=\"${HOME:-$(getent passwd \"$(id -u)\" | cut -d: -f6)}\"";

const VERSIONS_OUT: &str = "VERSION uv: uv 0.12.9
VERSION python3: Python 3.12.13
VERSION git: git version 2.43.0
VERSION jq: jq-1.7.1
VERSION rg: ripgrep 14.1.0 (rev 4649aa9700)
VERSION curl: curl 8.5.0 (x86_64-pc-linux-gnu) libcurl/8.5.0
VERSION cc: cc (Debian 12.2.0-14) 12.2.0
";

fn mb(n: i64) -> String {
    (n * 1024).to_string()
}

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

/// `baseInstalls()`: nothing carried, the tools PATH, no prefix, nothing left off.
fn plan() -> Vec<ToolInstall> {
    base_installs(&set(&[]), TOOLS_PATH, None, &set(&[]))
}

fn plan_carried(carried: BTreeSet<String>) -> Vec<ToolInstall> {
    base_installs(&carried, TOOLS_PATH, None, &set(&[]))
}

fn step_of(plan: &[ToolInstall], id: &str) -> ToolInstall {
    plan.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("no step {id} in the plan")).clone()
}

fn floor_ids() -> Vec<String> {
    base_floor().iter().map(|e| e.id.clone()).collect()
}

/// A manager as the road's name, as `guardedRoad(t.manager, ...)` takes it.
fn road(manager: &str) -> RoadName {
    serde_json::from_value(serde_json::Value::String(manager.to_string())).unwrap_or_else(|_| panic!("{manager} is not a road"))
}

fn tool(name: &str, version: &str) -> GoldenBaseTool {
    GoldenBaseTool { name: name.to_string(), version: version.to_string() }
}

/// writeStub: a script run by name on a PATH, its mode set here because the box's umask is 077.
fn write_stub(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `/\bcurl +-/.test(cmd)`: a curl as a word, spaces, then a dash.
fn curl_with_a_flag(cmd: &str) -> bool {
    cmd.match_indices("curl").any(|(i, _)| {
        let before_is_word = cmd[..i].chars().next_back().is_some_and(is_word);
        let rest = &cmd[i + 4..];
        let spaces = rest.len() - rest.trim_start_matches(' ').len();
        !before_is_word && spaces > 0 && rest[spaces..].starts_with('-')
    })
}

/// `/\bcurl +-[A-Za-z]*[fsSL]\b/.test(cmd)`: a curl typing a flag cluster whose last letter is f, s, S or L.
fn curl_with_a_loud_flag(cmd: &str) -> bool {
    cmd.match_indices("curl").any(|(i, _)| {
        let before_is_word = cmd[..i].chars().next_back().is_some_and(is_word);
        let rest = &cmd[i + 4..];
        let spaces = rest.len() - rest.trim_start_matches(' ').len();
        if before_is_word || spaces == 0 || !rest[spaces..].starts_with('-') {
            return false;
        }
        let flags = &rest[spaces + 1..];
        let letters: String = flags.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let after = flags[letters.len()..].chars().next();
        letters.ends_with(['f', 's', 'S', 'L']) && !after.is_some_and(is_word)
    })
}

/// `/curl[^\n]*\|\s*(ba)?sh/.test(cmd)`: a curl whose line pipes into a shell.
fn pipes_curl_into_a_shell(cmd: &str) -> bool {
    cmd.match_indices("curl").any(|(i, _)| {
        let line_end = cmd[i..].find('\n').map_or(cmd.len(), |n| i + n);
        cmd[i..line_end].match_indices('|').any(|(p, _)| {
            let after = cmd[i + p + 1..].trim_start();
            after.starts_with("sh") || after.starts_with("bash")
        })
    })
}

fn stage_name(s: wsp_engine::golden_seam::GoldenStage) -> String {
    serde_json::to_value(s).unwrap().as_str().unwrap().to_string()
}

struct Recorder {
    stages: Arc<Mutex<Vec<String>>>,
    stage: ToolStage,
}

impl Recorder {
    fn stages(&self) -> Vec<String> {
        self.stages.lock().unwrap().clone()
    }
}

fn recorder() -> Recorder {
    let stages: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = stages.clone();
    let stage: ToolStage = Arc::new(move |s, d| {
        let name = stage_name(s);
        seen.lock().unwrap().push(match d {
            None => name,
            Some(d) => format!("{name}:{d}"),
        });
    });
    Recorder { stages, stage }
}

/// A guest that answers df from `free`, runs every script with `answer`, and records both. `on_image` is what the
/// floor's version read prints before anything installs, which is the image as the provider ships it: bare unless a
/// test says the image already carries something.
struct Guest {
    machine: FakeMachine,
    ran: Arc<Mutex<Vec<String>>>,
    cmds: Arc<Mutex<Vec<String>>>,
}

impl Guest {
    fn ran(&self) -> Vec<String> {
        self.ran.lock().unwrap().clone()
    }
    fn cmds(&self) -> Vec<String> {
        self.cmds.lock().unwrap().clone()
    }
}

fn guest(answer: impl Fn(&str) -> Option<ExecResult> + 'static) -> Guest {
    guest_on(answer, || mb(3000), "")
}

fn guest_on(answer: impl Fn(&str) -> Option<ExecResult> + 'static, free: impl Fn() -> String + 'static, on_image: &str) -> Guest {
    let ran: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let cmds: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let reads = Arc::new(AtomicUsize::new(0));
    let answer = Arc::new(answer);
    let on_image = on_image.to_string();
    let versions_cmd = base_versions_cmd(TOOLS_PATH);
    let free_cmd = free_kb_cmd();
    let exec_cmds = cmds.clone();
    let exec_answer = answer.clone();
    let exec: ExecFn = Arc::new(move |cmd, _| {
        exec_cmds.lock().unwrap().push(cmd.clone());
        let said = if cmd == free_cmd {
            res(0, &format!("{}\n", free()), "")
        } else if cmd == versions_cmd && reads.fetch_add(1, Ordering::SeqCst) == 0 {
            res(0, &on_image, "")
        } else {
            exec_answer(&cmd).unwrap_or_else(ok)
        };
        Box::pin(async move { Ok(said) })
    });
    let run_ran = ran.clone();
    let run: RunFn = Arc::new(move |script, _| {
        run_ran.lock().unwrap().push(script.clone());
        let said = answer(&script).unwrap_or_else(ok);
        Box::pin(async move { Ok(said) })
    });
    let machine = FakeMachine { id: "m1".to_string(), exec, run: Some(run), download_url: None, upload_url: None, put_bytes: None };
    Guest { machine, ran, cmds }
}

async fn install_base(g: &Guest, rec: &Recorder) -> BaseOutcome {
    install_base_with(&g.machine, rec.stage.clone(), InstallBaseOptions::default()).await.unwrap()
}

fn id_outcome_bytes(out: &BaseOutcome) -> Vec<(&str, &str, Option<i64>)> {
    out.tools.iter().map(|t| (t.id.as_str(), t.outcome.as_str(), t.bytes)).collect()
}

fn id_outcome_note(out: &BaseOutcome) -> Vec<(&str, &str, Option<&str>)> {
    out.tools.iter().map(|t| (t.id.as_str(), t.outcome.as_str(), t.note.as_deref())).collect()
}

fn result(id: &str, label: &str, outcome: &str, bytes: Option<i64>, note: Option<&str>) -> ToolResult {
    ToolResult {
        id: id.to_string(),
        label: label.to_string(),
        outcome: outcome.to_string(),
        note: note.map(str::to_string),
        bytes,
        ..Default::default()
    }
}

// --- provision-floor.test.ts -----------------------------------------------------------------------------------

// TS: packages/engine/test/provision-floor.test.ts:32
// "leaves the C toolchain off the floor unless a picked row needs it, and keeps every other floor row"
#[test]
fn leaves_the_c_toolchain_off_the_floor_unless_a_picked_row_needs_it_and_keeps_every_other_floor_row() {
    let without: Vec<String> = base_installs(&set(&[]), TOOLS_PATH, None, &set(&[COMPILER_ROW])).into_iter().map(|s| s.id).collect();
    let whole: Vec<String> = base_installs(&set(&[]), TOOLS_PATH, None, &set(&[])).into_iter().map(|s| s.id).collect();
    let compiler_step = format!("base/{COMPILER_ROW}");
    assert!(whole.contains(&compiler_step));
    assert!(!without.contains(&compiler_step));
    let whole_without_compiler: Vec<String> = whole.into_iter().filter(|id| *id != compiler_step).collect();
    assert_eq!(whole_without_compiler, without);
}

// --- the base floor's plan -------------------------------------------------------------------------------------

// TS: packages/engine/test/golden-base.test.ts:61
// "is every floor entry by its catalog road, in order, each waiting on what it needs"
#[test]
fn is_every_floor_entry_by_its_catalog_road_in_order_each_waiting_on_what_it_needs() {
    let plan = plan();
    let got: Vec<(&str, &str, Option<&str>, Option<&str>)> =
        plan.iter().map(|t| (t.id.as_str(), t.manager.as_str(), t.after.as_deref(), t.bin.as_deref())).collect();
    assert_eq!(
        got,
        vec![
            ("base/login-path", "script", None, None),
            // curl leads the floor: the roads below that fetch a release type it, and a container image ships none.
            ("base/apt-index", "apt", None, None),
            ("base/curl", "apt", Some("base/apt-index"), Some("curl")),
            ("base/uv", "script", Some("base/curl"), Some("uv")),
            ("base/python", "script", Some("base/uv"), Some("python3")),
            ("base/git", "apt", Some("base/apt-index"), Some("git")),
            ("base/jq", "apt", Some("base/apt-index"), Some("jq")),
            ("base/ripgrep", "apt", Some("base/apt-index"), Some("rg")),
            ("base/build-essential", "apt", Some("base/apt-index"), Some("cc")),
            ("base/fd", "script", Some("base/apt-index"), Some("fd")),
            ("base/sqlite3", "apt", Some("base/apt-index"), Some("sqlite3")),
            ("base/wget", "apt", Some("base/apt-index"), Some("wget")),
            ("base/zip", "apt", Some("base/apt-index"), Some("zip")),
            ("base/xz", "apt", Some("base/apt-index"), Some("xz")),
            ("base/rsync", "apt", Some("base/apt-index"), Some("rsync")),
            ("base/ssh-server", "apt", Some("base/apt-index"), None),
        ]
    );
    let labels: Vec<&str> = plan.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "login shell PATH",
            "apt index",
            "curl",
            "uv",
            "Python 3.12",
            "git",
            "jq",
            "ripgrep",
            "C toolchain with cmake and ninja",
            "fd",
            "sqlite3",
            "wget",
            "zip and unzip",
            "xz",
            "rsync",
            "OpenSSH server"
        ]
    );
    // Node is not on the floor: a recipe row that runs on it brings it in the tools stage instead.
    assert!(!plan.iter().any(|t| t.cmd.contains("nodejs.org/dist")));
    let floor: Vec<String> = floor_ids().iter().map(|id| format!("base/{id}")).collect();
    let with_a_bin: Vec<String> = plan.iter().filter(|t| t.bin.is_some()).map(|t| t.id.clone()).collect();
    assert_eq!(floor, with_a_bin);
}

// TS: packages/engine/test/golden-base.test.ts:88
// "installs an ssh server on every golden, whatever its recipe, with the package's own port 22 closed while it installs and turned off only where it installed it"
#[test]
fn installs_an_ssh_server_on_every_golden_whatever_its_recipe_with_port_22_closed_while_it_installs() {
    let step = step_of(&plan_carried(floor_ids().into_iter().collect()), "base/ssh-server");
    // An image that has an sshd leaves the step before anything is installed, so its own units are never touched.
    let lines: Vec<&str> = step.cmd.split('\n').collect();
    let guard = lines.iter().position(|l| *l == "if test -x /usr/sbin/sshd; then exit 0; fi").expect("the step opens on its sshd guard");
    let first_apt = lines.iter().position(|l| l.starts_with("apt-get")).expect("the step runs apt-get");
    assert!(guard < first_apt);
    // With the whole floor carried the plan reads no index, so the step reads its own; with the floor's it waits on that.
    let install = lines.iter().position(|l| *l == "apt-get install -y -qq openssh-server").expect("the step installs openssh-server");
    let update = lines.iter().position(|l| *l == "apt-get update -qq").expect("the step reads its own index");
    assert!(update < install);
    // A policy that refuses every start holds the package's socket shut until the line that turns it off, and a
    // policy the machine already had is left as it was.
    let policy_line = "if [ ! -e /usr/sbin/policy-rc.d ]; then printf '#!/bin/sh\\nexit 101\\n' >/usr/sbin/policy-rc.d; chmod 755 /usr/sbin/policy-rc.d; trap 'rm -f /usr/sbin/policy-rc.d' EXIT; fi";
    let policy = lines.iter().position(|l| *l == policy_line).expect("the step holds port 22 shut while it installs");
    assert!(policy > guard);
    assert!(policy < install);
    assert_eq!(step.after, None);
    let waiting = step_of(&plan(), "base/ssh-server");
    assert_eq!(waiting.after.as_deref(), Some("base/apt-index"));
    assert!(!waiting.cmd.contains("apt-get update"));
    assert_eq!(lines.last().copied(), Some("(systemctl disable --now ssh.socket ssh.service >/dev/null 2>&1 || true)"));
    assert_eq!(step.check.as_deref(), Some("test -x /usr/sbin/sshd"));
}

// TS: packages/engine/test/golden-base.test.ts:111
// "writes the login shell's PATH before the floor, on every golden, so a thread's terminal finds what the stages install"
#[test]
fn writes_the_login_shells_path_before_the_floor_on_every_golden() {
    let step = step_of(&plan(), "base/login-path");
    assert!(step
        .cmd
        .contains(&format!("printf '%s\\n' 'export PATH={TOOLS_PATH} PNPM_HOME=/root/.local/share/pnpm' > /etc/profile.d/wsp-golden.sh")));
    assert_eq!(step.shown.as_deref(), Some("the tools PATH in /etc/profile.d/wsp-golden.sh"));
    // cargo comes by rustup on a golden with no Homebrew formula at all, and a login shell still finds it.
    assert!(TOOLS_PATH.contains("/root/.cargo/bin"));
}

// TS: packages/engine/test/golden-base.test.ts:119
// "every step, the apt index included, names one line a person reads for what it runs, so the Build screen's step row is one row"
#[test]
fn every_step_the_apt_index_included_names_one_line_a_person_reads_for_what_it_runs() {
    for t in plan() {
        let shown = t.shown.as_deref().unwrap_or_else(|| panic!("{} has no shown line", t.id));
        assert!(!shown.contains('\n'), "{}: {shown:?}", t.id);
    }
    let all = plan();
    let shown = |id: &str| step_of(&all, id).shown.unwrap();
    assert_eq!(shown("base/apt-index"), "apt-get update");
    assert_eq!(shown("base/git"), "apt-get install git");
    // /^if ! command -v uv >\/dev\/null 2>&1; then; .*; fi$/
    let uv = shown("base/uv");
    let head = "if ! command -v uv >/dev/null 2>&1; then; ";
    assert!(uv.starts_with(head) && uv.ends_with("; fi") && uv.len() >= head.len() + "; fi".len(), "uv shown as {uv:?}");
}

// TS: packages/engine/test/golden-base.test.ts:130
// "every step runs under set -e with a home and the tools PATH, and each road is the catalog's pinned one"
#[test]
fn every_step_runs_under_set_e_with_a_home_and_the_tools_path_and_each_road_is_the_catalogs_pinned_one() {
    let all = plan();
    let cmd = |id: &str| step_of(&all, id).cmd;
    for t in &all {
        assert!(t.cmd.starts_with("set -euo pipefail\nexport HOME="), "{}", t.id);
        assert!(t.cmd.contains("\nexport PATH=/root/.local/bin:/usr/local/sbin:/usr/local/bin:"), "{}", t.id);
        assert!(!pipes_curl_into_a_shell(&t.cmd), "{} pipes curl into a shell", t.id);
    }
    assert!(cmd("base/uv").contains("astral-sh/uv/releases/download/"));
    assert!(cmd("base/uv").contains("sha256sum -c"));
    assert!(cmd("base/python")
        .ends_with("\nuv python install 3.12\nln -sfn \"$(uv python find --managed-python 3.12)\" /usr/local/bin/python3"));
    assert!(cmd("base/apt-index").ends_with("\nexport DEBIAN_FRONTEND=noninteractive\napt-get update -qq"));
    assert!(cmd("base/jq").ends_with("\nexport DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq jq"));
    // The one floor row whose road is a script over apt: Debian ships the binary under another name and the step
    // puts the name agents type on PATH.
    assert!(cmd("base/fd")
        .ends_with("\nexport DEBIAN_FRONTEND=noninteractive\napt-get install -y -qq fd-find\nln -sfn /usr/bin/fdfind /usr/local/bin/fd"));
}

// --- a download that fails -------------------------------------------------------------------------------------

// TS: packages/engine/test/golden-base.test.ts:149
// "every base step that downloads runs under the one curl function, which fails loud on an HTTP error, and types no flags of its own"
#[test]
fn every_base_step_that_downloads_runs_under_the_one_curl_function_and_types_no_flags_of_its_own() {
    let downloads: Vec<ToolInstall> = plan().into_iter().filter(|t| curl_with_a_flag(&t.cmd)).collect();
    let ids: Vec<&str> = downloads.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["base/uv", "base/python"]);
    for t in &downloads {
        let run = guarded_road(road(&t.manager), &t.cmd);
        assert!(run.contains(CURL_NET.as_str()), "{}", t.id);
        assert!(CURL_NET.contains("--fail --silent --show-error --location"));
        let net_at = run.find(CURL_NET.as_str()).unwrap();
        let curl_o_at = run.find("curl -o").unwrap_or_else(|| panic!("{}: no curl -o in the guarded run", t.id));
        assert!(net_at < curl_o_at, "{}", t.id);
        assert!(!curl_with_a_loud_flag(&t.cmd), "{} types its own curl flags", t.id);
    }
}

// TS: packages/engine/test/golden-base.test.ts:161
// "leaves curl's own error as the step's reason: the uv script under bash, with a curl on PATH that fails the way a 404 does"
#[test]
fn leaves_curls_own_error_as_the_steps_reason_the_uv_script_under_bash_with_a_curl_that_fails_like_a_404() {
    // uname answers x86_64 so the script reaches its download on this computer; the curl stand-in fails as curl 7.88 does on a 404.
    let dir = tempfile::Builder::new().prefix("wsp-base-curl-").tempdir().unwrap();
    write_stub(&dir.path().join("uname"), "#!/bin/sh\necho x86_64\n");
    write_stub(&dir.path().join("curl"), "#!/bin/sh\necho \"curl: (22) The requested URL returned error: 404\" >&2\nexit 22\n");
    let mut parts: Vec<String> = vec![PRELUDE.to_string()];
    parts.extend(road_step(RoadName::Script).env);
    parts.push(UV_INSTALL.clone());
    let script = parts.join("\n");
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(&script)
        .env_clear()
        .env("HOME", dir.path())
        .env("PATH", format!("{}:/usr/bin:/bin", dir.path().display()))
        .output()
        .expect("bash runs");
    assert_eq!(out.status.code(), Some(22));
    let failed = ExecResult {
        exit_code: out.status.code().map(i64::from).unwrap_or(-1).into(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    };
    assert_eq!(reason_of(&failed, road_limit_s(RoadName::Script) as f64), "curl: (22) The requested URL returned error: 404");
}

// --- the versions read -----------------------------------------------------------------------------------------

// TS: packages/engine/test/golden-base.test.ts:177
// "asks each floor command for its version on the tools PATH, unzip with zip, and nothing of the node that left it"
#[test]
fn asks_each_floor_command_for_its_version_on_the_tools_path_unzip_with_zip_and_nothing_of_the_node() {
    let read = base_versions_cmd(TOOLS_PATH);
    let first = r"grep -m1 -E '[0-9]+\.[0-9]+'";
    assert!(read.starts_with("export PATH=/root/.local/bin:"));
    assert!(!read.contains("VERSION node:"));
    assert!(!read.contains("VERSION npm:"));
    assert!(read.contains(&format!("echo \"VERSION python3: $(python3 --version 2>/dev/null | {first})\"")));
    assert!(read.contains(&format!("echo \"VERSION rg: $(rg --version 2>/dev/null | {first})\"")));
    assert!(read.contains(&format!("echo \"VERSION unzip: $(unzip -v 2>/dev/null | {first})\"")));
    assert!(read.contains(&format!("echo \"VERSION git: $(git --version 2>/dev/null | {first})\"")));
    // sshd prints its version on stderr, which the read drops, so its own shell sends it to stdout.
    assert!(read.contains(&format!("echo \"VERSION sshd: $(sh -c 'sshd -V 2>&1' 2>/dev/null | {first})\"")));
    assert_eq!(read.split('\n').filter(|l| l.starts_with("echo \"VERSION")).count(), 17);
}

// TS: packages/engine/test/golden-base.test.ts:191
// "keeps the first line of a read that carries a version, so zip reads carried and the floor has no apt index to run"
#[test]
fn keeps_the_first_line_of_a_read_that_carries_a_version_so_zip_reads_carried_and_no_apt_index_runs() {
    // Every floor command answering under a real bash, zip with the two lines it really prints on Ubuntu: its
    // version is on the second, behind a copyright line with no version number on it at all.
    let dir = tempfile::Builder::new().prefix("wsp-floor-read-").tempdir().unwrap();
    for e in base_floor() {
        let own = match &e.major {
            None => "9.9.9".to_string(),
            Some(m) => format!("{}.0", m.version),
        };
        let mut commands = vec![(e.bin.clone(), own)];
        commands.extend(e.brings.iter().flatten().map(|b| (b.bin.clone(), "9.9.9".to_string())));
        for (bin, version) in commands {
            write_stub(&dir.path().join(&bin), &format!("#!/bin/sh\necho \"{bin} {version}\"\n"));
        }
    }
    // sshd as OpenSSH prints it: on stderr, which the read's own shell turns round.
    write_stub(&dir.path().join("sshd"), "#!/bin/sh\necho 'OpenSSH_9.6p1 Ubuntu-3ubuntu13.16, OpenSSL 3.0.13 30 Jan 2024' >&2\nexit 1\n");
    write_stub(
        &dir.path().join("zip"),
        &[
            "#!/bin/sh",
            "echo \"Copyright (c) 1990-2008 Info-ZIP - Type 'zip \\\"-L\\\"' for software license.\"",
            "echo 'This is Zip 3.0 (July 5th 2008), by Info-ZIP.'",
            "",
        ]
        .join("\n"),
    );
    // The scratch folder goes ahead of the tools PATH the read exports, which itself carries /usr/bin: this
    // computer's own zip would answer otherwise, and what is under test is the read, not this machine.
    let cmd = base_versions_cmd(TOOLS_PATH).replacen(TOOLS_PATH, &format!("{}:{TOOLS_PATH}", dir.path().display()), 1);
    let out = std::process::Command::new("bash").arg("-c").arg(&cmd).env_clear().env("PATH", "/usr/bin:/bin").output().expect("bash runs");
    assert_eq!(out.status.code(), Some(0));
    let versions = parse_versions(&String::from_utf8_lossy(&out.stdout));
    assert!(versions.contains(&tool("zip", "3.0")), "versions were {versions:?}");

    // With every row of the floor on the machine, nothing of it installs and no apt index is read for it: the
    // index runs before the first row that waits on it and no row is left to wait.
    let carried = carried_by_image(&versions);
    assert!(versions.contains(&tool("sshd", "9.6")), "versions were {versions:?}");
    let mut got: Vec<String> = carried.iter().cloned().collect();
    got.sort();
    let mut want = floor_ids();
    want.push("ssh-server".to_string());
    want.sort();
    assert_eq!(got, want);
    let planned: Vec<String> = plan_carried(carried.iter().cloned().collect()).into_iter().map(|t| t.id).collect();
    assert_eq!(planned, vec!["base/login-path".to_string()]);
}

// TS: packages/engine/test/golden-base.test.ts:219
// "keeps the version number out of each tool's own wording, and leaves out a command that printed nothing"
#[test]
fn keeps_the_version_number_out_of_each_tools_own_wording_and_leaves_out_a_command_that_printed_nothing() {
    assert_eq!(
        parse_versions(VERSIONS_OUT),
        vec![
            tool("uv", "0.12.9"),
            tool("python3", "3.12.13"),
            tool("git", "2.43.0"),
            tool("jq", "1.7.1"),
            tool("rg", "14.1.0"),
            tool("curl", "8.5.0"),
            tool("cc", "12.2.0"),
        ]
    );
    assert_eq!(parse_versions("VERSION node: v22.23.2\nVERSION zip: \nVERSION rg: no digits here\nnoise\n"), vec![tool("node", "22.23.2")]);
    assert_eq!(parse_versions(""), Vec::<GoldenBaseTool>::new());
}

// TS: packages/engine/test/golden-base.test.ts:233
// "the stage line names each version with what its install cost, and a floor entry that did not land by its reason"
#[test]
fn the_stage_line_names_each_version_with_what_its_install_cost_and_a_floor_entry_that_did_not_land_by_its_reason() {
    let versions: Vec<GoldenBaseTool> = parse_versions(VERSIONS_OUT).into_iter().filter(|v| v.name != "cc").collect();
    let mib = 1024 * 1024;
    let line = versions_line(
        &versions,
        &[
            result("base/uv", "uv", "installed", Some(42 * mib), None),
            result("base/python", "Python 3.12", "installed", Some(70 * mib), None),
            result("base/apt-index", "apt index", "installed", None, None),
            result("base/git", "git", "installed", Some(0), None),
            result("base/jq", "jq", "installed", Some(2 * mib), None),
            result("base/ripgrep", "ripgrep", "installed", Some(6 * mib), None),
            result("base/curl", "curl", "installed", Some(0), None),
            result("base/fd", "fd", "failed", None, Some("E: Unable to locate package fd-find")),
        ],
    );
    assert_eq!(
        line,
        "uv 0.12.9 (42 MB), python3 3.12.13 (70 MB), git 2.43.0, jq 1.7.1 (2 MB), rg 14.1.0 (6 MB), curl 8.5.0; fd failed (E: Unable to locate package fd-find)"
    );
    assert_eq!(versions_line(&[], &[]), "");
}

// --- installBase -----------------------------------------------------------------------------------------------

fn the_whole_floor_installed() -> Vec<(&'static str, &'static str, Option<i64>)> {
    [
        "base/login-path",
        "base/apt-index",
        "base/ssh-server",
        "base/curl",
        "base/uv",
        "base/python",
        "base/git",
        "base/jq",
        "base/ripgrep",
        "base/build-essential",
        "base/fd",
        "base/sqlite3",
        "base/wget",
        "base/zip",
        "base/xz",
        "base/rsync",
    ]
    .into_iter()
    .map(|id| (id, "installed", Some(0)))
    .collect()
}

// TS: packages/engine/test/golden-base.test.ts:251
// "runs the floor under the base stage, reads the versions once after, and its line carries them with the sizes df saw"
#[tokio::test(flavor = "multi_thread")]
async fn runs_the_floor_under_the_base_stage_reads_the_versions_once_after_and_its_line_carries_the_sizes_df_saw() {
    let free = Arc::new(Mutex::new(3000_i64));
    let answer_free = free.clone();
    let read_free = free.clone();
    let g = guest_on(
        move |script| {
            if script.contains("nodejs.org/dist") {
                *answer_free.lock().unwrap() -= 250;
                return Some(res(0, "NODE_HAVE v18.20.4\nNODE_INSTALLED v22.23.2\n", ""));
            }
            if script.contains("apt-get install -y -qq build-essential cmake ninja-build") {
                *answer_free.lock().unwrap() -= 400;
                return Some(ok());
            }
            if script.starts_with("export PATH=") && script.contains("VERSION curl:") {
                return Some(res(0, VERSIONS_OUT, ""));
            }
            None
        },
        move || mb(*read_free.lock().unwrap()),
        "",
    );
    let rec = recorder();
    let out = install_base(&g, &rec).await;
    let stages = rec.stages();
    assert_eq!(stages[0], "deploying-daemon:login shell PATH (1/16)");
    assert!(stages.contains(&"deploying-daemon:uv (4/16)".to_string()));
    assert!(stages.contains(&"deploying-daemon:C toolchain with cmake and ninja (9/16)".to_string()));
    assert!(stages.contains(&"deploying-daemon:rsync (15/16)".to_string()));
    assert!(stages.contains(&"deploying-daemon:OpenSSH server (16/16)".to_string()));
    assert!(stages.iter().all(|s| s.starts_with("deploying-daemon")), "stages were {stages:?}");
    let mut want = the_whole_floor_installed();
    want.iter_mut().find(|r| r.0 == "base/build-essential").unwrap().2 = Some(400 * 1024 * 1024);
    assert_eq!(id_outcome_bytes(&out), want);
    assert_eq!(out.line, "uv 0.12.9, python3 3.12.13, git 2.43.0, jq 1.7.1, rg 14.1.0, curl 8.5.0, cc 12.2.0 (400 MB)");
    // Once against the image as it arrives, once after the floor ran: the first says what there is nothing to do for.
    assert_eq!(g.cmds().iter().filter(|c| c.contains("VERSION curl:")).count(), 2);
    assert_eq!(g.ran().len(), 17);
}

// TS: packages/engine/test/golden-base.test.ts:297
// "reads df once between installs, and sizes an install after the rescue from the reading the cleanup left"
#[tokio::test(flavor = "multi_thread")]
async fn reads_df_once_between_installs_and_sizes_an_install_after_the_rescue_from_the_reading_the_cleanup_left() {
    let free = Arc::new(Mutex::new(1800_i64));
    let answer_free = free.clone();
    let read_free = free.clone();
    let g = guest_on(
        move |script| {
            if script.contains("rm -rf /root/.npm") {
                *answer_free.lock().unwrap() = 3000;
                return Some(ok());
            }
            if script.contains("nodejs.org/dist") {
                *answer_free.lock().unwrap() -= 250;
                return Some(res(0, "NODE_INSTALLED v22.23.2\n", ""));
            }
            None
        },
        move || mb(*read_free.lock().unwrap()),
        "",
    );
    let rec = recorder();
    let out = install_base(&g, &rec).await;
    assert_eq!(rec.stages()[0], "deploying-daemon:1.8 GB free, under the 2 GB floor; cleaning up before skipping");
    assert_eq!(id_outcome_bytes(&out), the_whole_floor_installed());
    // One read before the loop; the rescue's sweep reads before and after itself and the loop reads once more after
    // it; one after each of the sixteen installs; the closing sweep and line read three more.
    let free_cmd = free_kb_cmd();
    assert_eq!(g.cmds().iter().filter(|c| **c == free_cmd).count(), 23);
}

// TS: packages/engine/test/golden-base.test.ts:336
// "a step that fails is named on the stage and in the line, and what waited on it is skipped by its name"
#[tokio::test(flavor = "multi_thread")]
async fn a_step_that_fails_is_named_on_the_stage_and_in_the_line_and_what_waited_on_it_is_skipped_by_its_name() {
    let g = guest(|script| {
        if script.contains("apt-get update -qq") {
            Some(res(100, "", "E: Could not get lock /var/lib/apt/lists/lock"))
        } else if script.contains("VERSION curl:") {
            Some(res(0, "VERSION uv: uv 0.12.9\nVERSION python3: Python 3.12.13\n", ""))
        } else {
            None
        }
    });
    let rec = recorder();
    let out = install_base(&g, &rec).await;
    // The index leads the floor now, so an index that fails takes curl with it, and the roads that fetch a release
    // wait on curl: the cascade names each row by what it waited on.
    assert_eq!(
        id_outcome_note(&out),
        vec![
            ("base/login-path", "installed", None),
            ("base/apt-index", "failed", Some("E: Could not get lock /var/lib/apt/lists/lock")),
            ("base/ssh-server", "skipped", Some("apt index did not install")),
            ("base/curl", "skipped", Some("apt index did not install")),
            ("base/uv", "skipped", Some("curl did not install")),
            ("base/python", "skipped", Some("uv did not install")),
            ("base/git", "skipped", Some("apt index did not install")),
            ("base/jq", "skipped", Some("apt index did not install")),
            ("base/ripgrep", "skipped", Some("apt index did not install")),
            ("base/build-essential", "skipped", Some("apt index did not install")),
            ("base/fd", "skipped", Some("apt index did not install")),
            ("base/sqlite3", "skipped", Some("apt index did not install")),
            ("base/wget", "skipped", Some("apt index did not install")),
            ("base/zip", "skipped", Some("apt index did not install")),
            ("base/xz", "skipped", Some("apt index did not install")),
            ("base/rsync", "skipped", Some("apt index did not install")),
        ]
    );
    assert_eq!(
        out.line,
        "uv 0.12.9, python3 3.12.13; OpenSSH server skipped (apt index did not install); curl skipped (apt index did not install); uv skipped (curl did not install); Python 3.12 skipped (uv did not install); git skipped (apt index did not install); jq skipped (apt index did not install); ripgrep skipped (apt index did not install); C toolchain with cmake and ninja skipped (apt index did not install); fd skipped (apt index did not install); sqlite3 skipped (apt index did not install); wget skipped (apt index did not install); zip and unzip skipped (apt index did not install); xz skipped (apt index did not install); rsync skipped (apt index did not install)"
    );
    assert!(rec.stages().contains(&"deploying-daemon:1 installed, 1 failed: apt index (E: Could not get lock /var/lib/apt/lists/lock), 14 skipped: curl, git, jq, ripgrep, C toolchain with cmake and ninja, fd, sqlite3, wget, zip and unzip, xz, rsync, OpenSSH server (apt index did not install); uv (curl did not install); Python 3.12 (uv did not install); caches swept; 2.9 GB free".to_string()), "stages were {:?}", rec.stages());
}

// TS: packages/engine/test/golden-base.test.ts:364
// "a floor step that fails is recorded by the last line its installer wrote, not a generic one"
#[tokio::test(flavor = "multi_thread")]
async fn a_floor_step_that_fails_is_recorded_by_the_last_line_its_installer_wrote_not_a_generic_one() {
    let g = guest(|script| {
        if script.contains("apt-get install -y -qq fd-find") {
            Some(res(100, "Reading package lists...\n", "E: Unable to locate package fd-find\n"))
        } else if script.contains("VERSION curl:") {
            Some(res(0, "VERSION node: v22.23.2\n", ""))
        } else {
            None
        }
    });
    let out = install_base(&g, &recorder()).await;
    let fd = out.tools.iter().find(|t| t.id == "base/fd").expect("a base/fd row");
    assert_eq!(fd.outcome, "failed");
    assert_eq!(fd.note.as_deref(), Some("E: Unable to locate package fd-find"));
    assert_eq!(out.line, "node 22.23.2; fd failed (E: Unable to locate package fd-find)");
}

// TS: packages/engine/test/golden-base.test.ts:371
// "a floor row the provider's image already satisfies reads as on the machine and installs nothing; what waited on it still runs"
#[tokio::test(flavor = "multi_thread")]
async fn a_floor_row_the_providers_image_already_satisfies_reads_as_on_the_machine_and_installs_nothing() {
    // A row the image already satisfies read failed while the tool was there, since its install is not idempotent.
    // The row taken here promises a second command too, so both have to answer before the row counts as carried;
    // curl and git ride along to prove an apt row counts the same.
    let on_image = [
        "VERSION zip: Zip 3.0 (July 5th 2008)",
        "VERSION unzip: UnZip 6.00 of 20 April 2009",
        "VERSION curl: curl 8.5.0",
        "VERSION git: git version 2.43.0",
        "",
    ]
    .join("\n");
    let g = guest_on(|script| script.contains("VERSION curl:").then(|| res(0, VERSIONS_OUT, "")), || mb(3000), &on_image);
    let rec = recorder();
    let out = install_base(&g, &rec).await;
    let row = |id: &str| out.tools.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("no row {id}")).clone();
    assert_eq!(row("base/zip"), result("base/zip", "zip and unzip", "installed", None, Some(ALREADY_ON_MACHINE)));
    for id in ["base/curl", "base/git"] {
        assert_eq!(row(id).outcome, "installed", "{id}");
        assert_eq!(row(id).note.as_deref(), Some(ALREADY_ON_MACHINE), "{id}");
    }
    // Nothing was typed for them, and the rows that waited on curl and on the index ran all the same.
    let ran = g.ran();
    assert!(!ran.iter().any(|script| script.contains("apt-get install -y -qq zip unzip")));
    assert!(!ran.iter().any(|script| script.contains("apt-get install -y -qq git")));
    assert_eq!(row("base/uv").outcome, "installed");
    assert!(ran.iter().any(|script| script.contains("astral-sh/uv/releases")));
    // The floor keeps its catalog order whether a row ran or the image had it, and the line says which were there.
    let ids: Vec<&str> = out.tools.iter().map(|t| t.id.as_str()).collect();
    let mut want: Vec<String> = vec!["base/login-path".to_string(), "base/apt-index".to_string(), "base/ssh-server".to_string()];
    want.extend(floor_ids().iter().map(|id| format!("base/{id}")));
    assert_eq!(ids, want.iter().map(String::as_str).collect::<Vec<_>>());
    // The words are true whichever road put the tool there: the provider's image, or an earlier run of this stage.
    assert!(out.line.contains("already on the machine: curl, git, zip and unzip"), "line was {:?}", out.line);
    assert!(!rec.stages().iter().any(|s| s.contains("zip and unzip (")));
}

// TS: packages/engine/test/golden-base.test.ts:395
// "a floor row that pins a major runs when the image carries another, and the apt index is left out when no apt row needs it"
#[test]
fn a_floor_row_that_pins_a_major_runs_when_the_image_carries_another_and_the_apt_index_is_left_out() {
    let older = ["VERSION python3: Python 3.11.2", "VERSION uv: uv 0.12.9", ""].join("\n");
    let names = |carried: &indexmap::IndexSet<String>| carried.iter().cloned().collect::<Vec<String>>();
    // Python 3.12 is what the floor promises: an image on another major is not a floor that is there.
    assert_eq!(names(&carried_by_image(&parse_versions(&older))), vec!["uv".to_string()]);
    assert_eq!(
        names(&carried_by_image(&parse_versions("VERSION python3: Python 3.12.1\nVERSION uv: uv 0.12.9\n"))),
        vec!["uv".to_string(), "python".to_string()]
    );
    // zip promises unzip too, so a zip with no unzip beside it is not the floor's row.
    assert_eq!(names(&carried_by_image(&parse_versions("VERSION zip: Zip 3.0\n"))), Vec::<String>::new());
    // The index is read for the rows that wait on it; an image that carries every one of them is read for nothing.
    let waiting_on_apt: Vec<String> =
        base_floor().into_iter().filter(|e| install_after(e).as_deref() == Some(APT_INDEX)).map(|e| e.id.clone()).collect();
    assert!(!plan_carried(waiting_on_apt.iter().cloned().collect()).iter().any(|t| t.id == "base/apt-index"));
    assert!(plan_carried(waiting_on_apt[1..].iter().cloned().collect()).iter().any(|t| t.id == "base/apt-index"));
    // A row whose own dependency the image carries waits on nothing rather than on a step the plan no longer holds.
    assert_eq!(step_of(&plan_carried(set(&["curl"])), "base/uv").after, None);
    assert_eq!(step_of(&plan_carried(set(&["curl"])), "base/git").after.as_deref(), Some("base/apt-index"));
}

// TS: packages/engine/test/golden-base.test.ts:411
// "an install that exits 0 without its command on PATH is a failure, not a version"
#[tokio::test(flavor = "multi_thread")]
async fn an_install_that_exits_0_without_its_command_on_path_is_a_failure_not_a_version() {
    let g = guest(|script| {
        if script.contains("for b in") && script.contains("command -v") {
            Some(res(0, "missing fd\n", ""))
        } else if script.contains("VERSION curl:") {
            Some(res(0, "VERSION node: v22.23.2\n", ""))
        } else {
            None
        }
    });
    let out = install_base(&g, &recorder()).await;
    let fd = out.tools.iter().find(|t| t.id == "base/fd").expect("a base/fd row");
    assert_eq!(fd.outcome, "failed");
    assert_eq!(fd.note.as_deref(), Some("fd is not on PATH after the install"));
    assert_eq!(out.line, "node 22.23.2; fd failed (fd is not on PATH after the install)");
}
