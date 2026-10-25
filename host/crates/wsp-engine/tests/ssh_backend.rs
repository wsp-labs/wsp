// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/ssh-backend.test.ts: the dial, the read that adopts a machine, the key a machine is
//! known by and the alias a typed word names. Specs: spec/ssh_dial.md, spec/ssh_keys.md, spec/ssh_machine.md.
mod roads_common;

use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};

use futures::FutureExt;
use indexmap::IndexMap;
use roads_common::local_run;
use roads_common::*;
use wsp_engine::errors::Result;
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineLife, MachineShape, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::machine_context::probe_command;
use wsp_engine::machine_facts::{arch_of, read_values, ARCH_READ, OS_READ, SHELL_READ, SYSTEM_READ, UPTIME_READ};
use wsp_engine::ssh_backend::*;
use wsp_wire::types::js_number::JsNumber;

/// Tests that read or set HOME hold this, and give it back as it was.
static ENV: Mutex<()> = Mutex::new(());

fn env_lock() -> MutexGuard<'static, ()> {
    ENV.lock().unwrap_or_else(|e| e.into_inner())
}

struct HomeSet(Option<String>);

impl HomeSet {
    fn to(value: &str) -> Self {
        let was = std::env::var("HOME").ok();
        std::env::set_var("HOME", value);
        HomeSet(was)
    }
}

impl Drop for HomeSet {
    fn drop(&mut self) {
        match &self.0 {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

/// The login's own entry in the password database: its name and home, which is what Node's `userInfo()` reads.
fn passwd_entry() -> (String, String) {
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        assert!(!pw.is_null(), "no passwd entry for this user");
        let name = std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned();
        let dir = std::ffi::CStr::from_ptr((*pw).pw_dir).to_string_lossy().into_owned();
        (name, dir)
    }
}

/// Node's `homedir()`: `$HOME` where set and non-empty, else the password database's.
fn node_homedir() -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() => h,
        _ => passwd_entry().1,
    }
}

fn reach() -> SshReach {
    SshReach { user: "dev".into(), host: "10.0.0.5".into(), port: 2222, key_path: Some("/tmp/k/id_ed25519".into()) }
}

fn reach_of(user: &str, host: &str, port: u16) -> SshReach {
    SshReach { user: user.into(), host: host.into(), port, key_path: None }
}

fn ok() -> ExecResult {
    res(0, "", "")
}

/// What a dial riding a master the last minute left open has on its stderr: the client exchanged no key on it, so
/// nothing here says which machine answered.
const CANNED_READ: &str = "home /home/dev\nuser dev\npath /home/dev/.local/bin:/usr/bin\ncpu 8\nmemkb 16384000\n";

/// An ssh client that never leaves this computer: it answers the read every adopt makes, records every script it was
/// asked to carry, and lets a case script the answer for anything else, streaming each line as the client would.
fn fake_ssh<F>(answer: F) -> (SshTransport, Arc<Mutex<Vec<Carried>>>)
where
    F: Fn(&str) -> ExecResult + Send + Sync + 'static,
{
    let read = ssh_read_script();
    transport(move |_reach, script, opts| {
        if script == read {
            return res(0, CANNED_READ, "");
        }
        let scripted = answer(script);
        let lines: Vec<&str> = scripted.stdout.split('\n').collect();
        for line in &lines[..lines.len() - 1] {
            if let Some(on_line) = &opts.on_line {
                on_line(line);
            }
        }
        scripted
    })
}

const FOUND_KEY: &str = "ssh-ed25519 SHA256:Ge9MQ9S/Faik2WzsxidRXnoEOJRIHkJKTBkOJ903vq4";

fn known_key() -> SshHostKeyReader {
    Arc::new(|_| async { Some(FOUND_KEY.to_string()) }.boxed())
}

fn no_key() -> SshHostKeyReader {
    Arc::new(|_| async { None }.boxed())
}

/// A backend whose key reader never reaches the real client.
fn backend_of(t: SshTransport) -> SshBackend {
    SshBackend::new(SshBackendOptions { transport: Some(t), host_key: Some(known_key()), ..Default::default() })
}

/// A backend that holds no key and no known_hosts entry for any machine.
fn bare_backend_of(t: SshTransport) -> SshBackend {
    SshBackend::new(SshBackendOptions {
        transport: Some(t),
        host_key: Some(no_key()),
        known_hosts: Some(Arc::new(|_| async { KnownHostsEntry::default() }.boxed())),
        ..Default::default()
    })
}

/// What a failed call said, or the empty string where it did not fail.
fn said<T>(r: Result<T>) -> String {
    match r {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    }
}

fn pairs(login: &SshLogin) -> Vec<(String, String)> {
    login.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn vals(of: &[(&str, &str)]) -> IndexMap<String, String> {
    of.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn strs(of: &[&str]) -> Vec<String> {
    of.iter().map(|s| s.to_string()).collect()
}

fn has(args: &[String], word: &str) -> bool {
    args.iter().any(|a| a == word)
}

fn pair_at(args: &[String], flag: &str) -> Vec<String> {
    let i = args.iter().position(|a| a == flag).unwrap_or_else(|| panic!("no {flag} in {args:?}"));
    args[i..i + 2].to_vec()
}

/// The word after a flag in the way the TS reads it: `args[args.indexOf(flag) + 1]`, which is the first word where the
/// flag is missing.
fn after(args: &[String], flag: &str) -> String {
    let i = args.iter().position(|a| a == flag).map_or(0, |i| i + 1);
    args[i].clone()
}

type Asked = Arc<Mutex<Vec<Vec<String>>>>;

/// A local run that records into a log the case shares between several runs.
fn run_into<F>(asked: &Asked, answer: F) -> SshLocalRun
where
    F: Fn(&str, &[String]) -> ExecResult + Send + Sync + 'static,
{
    let log = asked.clone();
    Arc::new(move |file: String, args: Vec<String>, _timeout: u64| {
        let mut line = vec![file.clone()];
        line.extend(args.iter().cloned());
        log.lock().unwrap().push(line);
        let out = answer(&file, &args);
        async move { out }.boxed()
    })
}

fn asked_of(asked: &Asked) -> Vec<Vec<String>> {
    asked.lock().unwrap().clone()
}

fn is_sentence_break(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    (0..chars.len()).any(|i| chars[i] == '.' && chars[i + 1..].iter().position(|c| !c.is_whitespace()).is_some_and(|j| j > 0))
}

#[test]
fn user_at_host_names_the_dial_with_the_port_and_key_the_person_gave() {
    assert_eq!(parse_ssh_address("dev@box", None, None).unwrap(), reach_of("dev", "box", 22));
    assert_eq!(parse_ssh_address("dev@box:2222", None, None).unwrap(), reach_of("dev", "box", 2222));
    assert_eq!(
        parse_ssh_address("dev@box", Some(2200), Some("/tmp/k")).unwrap(),
        SshReach { user: "dev".into(), host: "box".into(), port: 2200, key_path: Some("/tmp/k".into()) }
    );
    assert!(said(parse_ssh_address("box", None, None)).contains("name the machine as user@host"));
}

#[test]
fn the_client_runs_the_script_under_bash_c_never_a_login_shell() {
    let args = ssh_args(&reach(), "echo 'it works'", &SshArgsOptions::default());
    let n = args.len();
    assert!(has(&args, "BatchMode=yes"));
    assert_eq!(pair_at(&args, "-p"), strs(&["-p", "2222"]));
    assert_eq!(pair_at(&args, "-i"), strs(&["-i", "/tmp/k/id_ed25519"]));
    assert!(!has(&args, "-lc"));
    // The script is one quoted word at the end, the outer bash's $1, so a quote inside it reaches the machine as
    // written; the word before it is the road to root, which the login's own uid picks at the far end.
    assert_eq!(args[n - 6..n - 2].to_vec(), vec!["dev@10.0.0.5".to_string(), "bash".into(), "-c".into(), args[n - 3].clone()]);
    assert!(args[n - 3].contains("exec sudo -n bash -c"));
    assert_eq!(args[n - 2..].to_vec(), strs(&["wsp", "'echo '\\''it works'\\'''"]));
    assert!(!has(&ssh_args(&reach_of("dev", "box", 22), "true", &SshArgsOptions::default()), "-i"));
}

#[test]
fn a_machine_with_no_name_given_is_called_what_its_address_calls_it() {
    assert_eq!(ssh_machine_name(&reach_of("dev", "box.example.com", 22)), "box");
    assert_eq!(ssh_machine_name(&reach_of("dev", "box", 22)), "box");
    assert_eq!(ssh_machine_name(&reach_of("dev", "10.0.0.5", 22)), "10.0.0.5");
}

#[tokio::test]
async fn adopt_dials_once_reads_the_machines_own_login_and_size() {
    let (t, carried) = fake_ssh(|_| ok());
    let backend = backend_of(t);
    let adopted = backend.adopt(&reach()).await.unwrap();
    assert_eq!(scripts(&carried), vec![ssh_read_script()]);
    assert_eq!(
        adopted.shape,
        MachineShape { cpu: Some(JsNumber(8.0)), mem_mb: Some(JsNumber(16000.0)), disk_gb: None, created_at: None, used_bytes: None }
    );
    // The turn's environment is the machine's own login: its home, who it runs as, and the PATH their shell gives.
    assert_eq!(
        pairs(&adopted.login),
        vec![
            ("HOME".to_string(), "/home/dev".to_string()),
            ("USER".to_string(), "dev".to_string()),
            ("PATH".to_string(), "/home/dev/.local/bin:/usr/bin".to_string()),
        ]
    );
    assert_eq!(adopted.machine.id(), ssh_machine_id(&reach()).unwrap());
}

#[test]
fn holds_each_folder_of_the_machines_own_path_to_the_rule() {
    // A folder carrying whitespace or a quote is dropped at this door, so nothing downstream escapes it twenty times.
    assert_eq!(plain_path(Some("/home/dev/.local/bin:/usr/bin")), "/home/dev/.local/bin:/usr/bin");
    assert_eq!(plain_path(Some("/usr/bin:/opt/a\"b/bin:/bin")), "/usr/bin:/bin");
    assert_eq!(plain_path(Some("/usr/bin:/opt/my tools/bin")), "/usr/bin");
    assert_eq!(plain_path(Some("/usr/bin:/opt/$(id)/bin:/opt/`id`/bin")), "/usr/bin");
    // An empty entry is the working directory, which is a folder nobody meant to put on a PATH.
    assert_eq!(plain_path(Some("/usr/bin::/bin")), "/usr/bin:/bin");
    assert_eq!(plain_path(Some("./bin:/bin")), "/bin");
    // A machine that answers with nothing usable leaves a harness missing rather than every command missing.
    for answered in [None, Some(""), Some("relative:also/relative"), Some("\"")] {
        assert_eq!(plain_path(answered), DEFAULT_REMOTE_PATH, "{answered:?}");
    }
}

#[tokio::test]
async fn a_machine_that_does_not_answer_the_dial_is_refused_with_the_clients_own_words() {
    let (t, _) = transport(|_, _, _| {
        res(
            255,
            "",
            "debug1: Offering public key: /tmp/k/id_ed25519\ndebug1: No more authentication methods to try.\ndev@10.0.0.5: Permission denied (publickey).\n",
        )
    });
    let failed = said(backend_of(t).adopt(&reach()).await);
    assert!(failed.contains("Permission denied (publickey)."), "{failed}");
    assert!(!failed.contains("debug1:"), "{failed}");
}

#[tokio::test]
async fn a_refusal_carries_none_of_sshs_note_about_the_key_accept_new_wrote() {
    let (t, _) = transport(|_, _, _| {
        res(
            255,
            "",
            "Warning: Permanently added '10.0.0.5' (ED25519) to the list of known hosts.\ndev@10.0.0.5: Permission denied (publickey).\n",
        )
    });
    let failed = said(backend_of(t).adopt(&reach()).await);
    assert!(failed.contains("Permission denied (publickey)."), "{failed}");
    assert!(!failed.contains("Permanently added"), "{failed}");
    assert!(!failed.contains("known hosts"), "{failed}");
}

#[tokio::test]
async fn a_dial_that_exchanged_no_key_still_hands_back_the_machines_identity() {
    let (t, carried) = fake_ssh(|_| ok());
    let adopted = backend_of(t).adopt(&reach()).await.unwrap();
    // The dial rode a master the last minute left open, so the client logged no key exchange on it. What the record
    // stands on is the entry that client already holds for the machine, which is there either way.
    assert_eq!(adopted.host_key.as_deref(), Some(FOUND_KEY));
    // Two machines are the same machine when they hold the same key for the same login, whatever the address.
    assert_eq!(ssh_identity(adopted.host_key.as_deref().unwrap(), "dev"), format!("{FOUND_KEY} as dev"));
    // No dial asks the client for its debug log any more: a warm master has none to give, and the log rode the
    // stderr of the one command whose failure the person reads.
    assert!(!has(&ssh_args(&reach(), "true", &SshArgsOptions::default()), "LogLevel=DEBUG"));
    assert!(!has(&ssh_dial_args(&reach()), "LogLevel=DEBUG"));
    adopted.machine.exec("true", ExecOptions::default()).await.unwrap();
    assert_eq!(scripts(&carried), vec![ssh_read_script(), "true".to_string()]);
}

#[tokio::test]
async fn a_machine_the_client_holds_no_key_for_is_recorded_without_an_identity() {
    let (t, _) = fake_ssh(|_| ok());
    assert_eq!(bare_backend_of(t).adopt(&reach()).await.unwrap().host_key, None);
}

#[tokio::test]
async fn the_machines_own_words_for_its_home_are_held_to_a_plain_path() {
    async fn home(answer: &str) -> String {
        let stdout = format!("home {answer}\nuser dev\npath /usr/bin\ncpu 1\nmemkb 1024\n");
        let (t, _) = transport(move |_, _, _| res(0, &stdout, ""));
        match backend_of(t).adopt(&reach()).await {
            Ok(a) => a.login["HOME"].clone(),
            Err(e) => e.to_string(),
        }
    }
    // What a machine answering with shell in its home would land in the launch: refused at the one door instead.
    assert!(home("/home/dev x; touch /tmp/pwned").await.contains("is not a plain path"));
    assert!(home("/home/$(id -un)").await.contains("is not a plain path"));
    assert!(home("/home/dev`whoami`").await.contains("is not a plain path"));
    assert!(home("relative/home").await.contains("is not a plain path"));
    assert!(home("").await.contains("no home folder"));
    // A space is a home on macOS and stays a home: the paths built from it are quoted where they land in a command.
    assert_eq!(home("/Users/John Smith").await, "/Users/John Smith");
    assert_eq!(home("/root").await, "/root");
}

#[tokio::test]
async fn the_store_folder_each_harness_reads_is_asked_of_the_login_shell() {
    async fn read(line: &str) -> SshLogin {
        let stdout = format!("home /root\nuser root\npath /usr/bin\n{line}cpu 1\nmemkb 1024\n");
        let (t, _) = transport(move |_, _, _| res(0, &stdout, ""));
        backend_of(t).adopt(&reach()).await.unwrap().login
    }
    assert!(ssh_store_vars().contains(&"CLAUDE_CONFIG_DIR".to_string()));
    // The read asks the machine's own shell for each store variable the catalog names, in the same call as PATH.
    let script = ssh_read_script();
    assert!(script.contains("bash --noprofile --norc -c"));
    assert!(script.contains("printf \"store:CLAUDE_CONFIG_DIR %s"));
    let login = read("store:CLAUDE_CONFIG_DIR /root/.claude-cfg\n").await;
    assert_eq!(login.get("HOME").map(String::as_str), Some("/root"));
    assert_eq!(login.get("CLAUDE_CONFIG_DIR").map(String::as_str), Some("/root/.claude-cfg"));
    // A machine that names none leaves the harness on its default, and one that names shell is left there too.
    assert_eq!(read("").await.get("CLAUDE_CONFIG_DIR"), None);
    assert_eq!(read("store:CLAUDE_CONFIG_DIR /root/x; id\n").await.get("CLAUDE_CONFIG_DIR"), None);
}

#[test]
fn opens_no_login_file_of_the_machines_own() {
    // A computer somebody owns is worked as its own root by the host, and that root's home is the one every
    // workspace on it writes, so a profile or an rc file under it is a file a workspace wrote.
    let script = ssh_read_script();
    assert!(script.contains("bash --noprofile --norc -c"));
    assert!(!script.contains("bash -lc"));
    assert!(!script.contains("bash -l "));
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let marker = dir.path().join("sourced");
    for rc in [".profile", ".bash_profile", ".bashrc"] {
        std::fs::write(dir.path().join(rc), format!("echo {rc} >> {}\n", marker.display())).unwrap();
    }
    let out = Command::new("bash").args(["-c", &format!("export HOME={home}\n{script}")]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let values = read_values(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(values["home"], home);
    assert_eq!(values["user"], passwd_entry().0);
    assert!(!values["path"].is_empty());
    assert!(values["cpu"].parse::<f64>().unwrap() > 0.0);
    assert!(!marker.exists());
}

#[tokio::test]
async fn reads_which_login_shell_the_machines_root_runs() {
    // sshd hands every command wsp sends to this login's own shell with -c before the bash -c the client names, so
    // which shell it is decides whether a file under the login's home runs first. The read asks getent, which runs
    // no shell of the machine's, and is the same line the context probe reads.
    let script = ssh_read_script();
    assert!(script.contains(SHELL_READ));
    assert!(script.contains("printf \"shell %s\\n\" \"$shell\""));
    assert!(probe_command(None, None, None).contains(SHELL_READ));
    let said_line = |line: &str| {
        let stdout = format!("home /root\nuser root\n{line}path /usr/bin\ncpu 1\nmemkb 1024\n");
        transport(move |_, _, _| res(0, &stdout, "")).0
    };
    assert_eq!(read_ssh_machine(&reach(), &said_line("shell zsh\n")).await.unwrap().shell.as_deref(), Some("zsh"));
    assert_eq!(read_ssh_machine(&reach(), &said_line("shell bash\n")).await.unwrap().shell.as_deref(), Some("bash"));
    // A read that named none carries none: which shells a road may work through is that road's rule, not this one's.
    assert_eq!(read_ssh_machine(&reach(), &said_line("")).await.unwrap().shell, None);
    let fish = bare_backend_of(said_line("shell fish\n")).adopt(&reach()).await.unwrap();
    assert_eq!(fish.shell.as_deref(), Some("fish"));

    // What the line answers on this computer, run for real: a name and nothing more, and no file of a home read.
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let marker = dir.path().join("sourced");
    for rc in [".profile", ".bash_profile", ".bashrc", ".zshenv"] {
        std::fs::write(dir.path().join(rc), format!("echo {rc} >> {}\n", marker.display())).unwrap();
    }
    // A folder holding this computer's bash and nothing else, so the read finds no getent whichever system this
    // runs on: that is the shape of a machine whose passwd cannot be read, which is what the fallbacks are for.
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let real_bash = Command::new("sh").args(["-c", "command -v bash"]).output().unwrap();
    let real_bash = String::from_utf8_lossy(&real_bash.stdout).trim().to_string();
    std::os::unix::fs::symlink(&real_bash, bin.join("bash")).unwrap();
    let answers = |env: &[(&str, &str)]| -> String {
        let mut cmd = Command::new(&real_bash);
        cmd.args(["-c", &format!("{SHELL_READ}\nprintf \"shell %s\\n\" \"$shell\"")]).env_clear();
        cmd.env("HOME", &home).env("PATH", &bin);
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        read_values(&String::from_utf8_lossy(&out.stdout))["shell"].clone()
    };
    let here = answers(&[("PATH", &std::env::var("PATH").unwrap())]);
    assert!(!here.is_empty() && here.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)), "{here}");
    // Behind the passwd entry, the login's own SHELL, which sshd sets for every session it opens: a machine
    // carrying no getent names its own shell rather than the word that would let it through.
    assert_eq!(answers(&[("SHELL", "/usr/bin/zsh")]), "zsh");
    assert_eq!(answers(&[("SHELL", "/usr/local/bin/fish")]), "fish");
    // And behind that bash, which is what every reader takes an answer of nothing for. bash fills SHELL from the
    // passwd database where it is unset, so the word has to be emptied rather than left out to reach this.
    assert_eq!(answers(&[("SHELL", "")]), "bash");
    assert!(!marker.exists());
}

#[test]
fn every_command_to_one_machine_rides_one_master_connection() {
    let _env = env_lock();
    let args = ssh_args(&reach(), "true", &SshArgsOptions::default());
    assert!(has(&args, "ControlMaster=auto"));
    assert!(has(&args, &format!("ControlPersist={SSH_CONTROL_PERSIST_S}")));
    assert!(has(&args, &format!("ControlPath={}", ssh_control_path(&reach(), None))));
    // One socket per machine, named by the dial and short enough for a unix socket whatever the host name is.
    let other_key = SshReach { key_path: Some("/tmp/k/other".into()), ..reach() };
    assert_eq!(ssh_control_path(&other_key, None), ssh_control_path(&reach(), None));
    let other_port = SshReach { port: 22, ..reach() };
    assert_ne!(ssh_control_path(&other_port, None), ssh_control_path(&reach(), None));
    let long = reach_of("dev", &"a".repeat(200), 22);
    let path = ssh_control_path(&long, Some("/tmp"));
    assert!(path.len() < 100, "{} is {} long", path, path.len());
}

#[test]
fn the_master_sockets_live_in_a_folder_only_the_person_can_read() {
    use std::os::unix::fs::PermissionsExt;
    let _env = env_lock();
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().to_str().unwrap().to_string();
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    // Whoever holds a control socket holds every command that rides it, so the folder is wsp's own and nobody
    // else's: not the shared temp folder, whose name anyone could work out and sit on first.
    let dir = make_ssh_control_dir(Some(&home)).unwrap();
    assert_eq!(dir, tmp.path().join(".wsp").join("ssh"));
    assert_eq!(mode(&dir), 0o700);
    assert!(ssh_control_path(&reach(), Some(dir.to_str().unwrap())).starts_with(&format!("{}/", dir.display())));
    // With no home named it is the person's own, never the folder every login on this computer shares.
    assert_eq!(ssh_control_dir(None), std::path::Path::new(&node_homedir()).join(".wsp").join("ssh"));
    // A folder left looser by something else is tightened rather than trusted.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let again = make_ssh_control_dir(Some(&home)).unwrap();
    assert_eq!(mode(&again), 0o700);
}

#[tokio::test]
async fn exec_and_run_carry_the_script_to_the_machine() {
    let (t, carried) = fake_ssh(|script| if script == "exit 7" { res(7, "", "") } else { res(0, "a\nb\n", "") });
    let backend = backend_of(t);
    let machine = backend.adopt(&reach()).await.unwrap().machine;
    assert_eq!(machine.exec("exit 7", ExecOptions::default()).await.unwrap().exit_code, 7);
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen = lines.clone();
    let out = machine
        .run(
            "printf 'a\\nb\\n'",
            RunOptions {
                deadline_ms: 5_000,
                on_line: Some(Arc::new(move |l: &str| seen.lock().unwrap().push(l.to_string()))),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(*lines.lock().unwrap(), strs(&["a", "b"]));
    assert_eq!(out.exit_code, 0);
    assert_eq!(scripts(&carried)[1..].to_vec(), strs(&["exit 7", "printf 'a\\nb\\n'"]));
    let reaches: Vec<SshReach> = carried.lock().unwrap().iter().map(|c| c.reach.clone()).collect();
    assert_eq!(reaches, vec![reach(); 3]);
}

#[tokio::test]
async fn the_moves_only_a_machine_wsp_forks_takes_are_refused() {
    let (t, _) = fake_ssh(|_| ok());
    let backend = backend_of(t);
    // Read as the runtime holds it, through the seam, so a refusal is proven on the interface every road calls.
    let machine: Arc<dyn Machine> = backend.adopt(&reach()).await.unwrap().machine;
    let snap = said(machine.snapshot("x", MachineLife { first_life: true }, SnapshotOptions::default()).await);
    assert!(snap.contains("cannot be snapshotted"), "{snap}");
    assert!(said(machine.pause(None).await).contains("cannot be paused"));
    assert!(said(machine.resume(None).await).contains("cannot be resumed"));
    assert!(!machine.offers().preview_url);
    assert!(said(machine.download_url("/x").await).contains("no signed download URL"));
    assert!(said(machine.upload_url("/x").await).contains("no signed upload URL"));
    // Nothing here ends the person's machine: kill is a no-op.
    assert_eq!(machine.kill().await.unwrap(), ());
    assert_eq!(machine.state().await.unwrap(), MachineState::Running);
}

/// A client that answers the adopt read with the chip word this case is about, and nothing else scripted. Every other
/// line is what an Ubuntu box prints, so only the one answer under test differs between cases.
fn reads_arch(line: &str) -> SshTransport {
    let stdout = format!("home /home/dev\n{line}user dev\npath /usr/bin:/bin\ncpu 8\nmemkb 16384000\n");
    let read = ssh_read_script();
    transport(move |_, script, _| if script == read { res(0, &stdout, "") } else { ok() }).0
}

#[test]
fn asks_for_it_on_the_read_that_adopts_the_machine() {
    // One printf on the read that already runs, not a round trip of its own: the chip is wanted before anything is
    // sent to the machine, which is the same moment the home and the login PATH are wanted.
    assert!(ssh_read_script().contains(ARCH_READ));
    assert!(ARCH_READ.contains("uname -m"));
}

#[tokio::test]
async fn carries_the_arch_word_back_whatever_it_is() {
    // Every word a machine may print for itself rides through unread: Linux prints x86_64 or aarch64, a Mac prints
    // arm64, and a board nobody builds for prints its own. Which of them wsp has a daemon for is decided against a
    // table above this file, so nothing here turns an unknown word into nothing.
    for word in ["x86_64", "aarch64", "arm64", "riscv64"] {
        let read = read_ssh_machine(&reach(), &reads_arch(&format!("arch {word}\n"))).await.unwrap();
        assert_eq!(read.arch.as_deref(), Some(word));
        let adopted = bare_backend_of(reads_arch(&format!("arch {word}\n"))).adopt(&reach()).await.unwrap();
        assert_eq!(adopted.arch.as_deref(), Some(word));
    }
}

#[tokio::test]
async fn answers_no_arch_where_the_machine_printed_none() {
    // A machine whose uname printed nothing leaves the key present and empty, and one on a shell that never ran the
    // line leaves it absent. Neither is a chip, and the caller refuses rather than guessing at one.
    for line in ["", "arch \n"] {
        assert_eq!(read_ssh_machine(&reach(), &reads_arch(line)).await.unwrap().arch, None, "{line:?}");
        assert_eq!(bare_backend_of(reads_arch(line)).adopt(&reach()).await.unwrap().arch, None, "{line:?}");
    }
    assert_eq!(arch_of(&vals(&[])), None);
    assert_eq!(arch_of(&vals(&[("arch", "")])), None);
    assert_eq!(arch_of(&vals(&[("arch", "x86_64")])).as_deref(), Some("x86_64"));
}

#[tokio::test]
async fn asks_uname_s_ahead_of_the_chip() {
    let reads = |lines: &str| {
        let stdout = format!("home /home/dev\n{lines}user dev\npath /usr/bin:/bin\ncpu 8\nmemkb 16384000\n");
        let read = ssh_read_script();
        transport(move |_, script, _| if script == read { res(0, &stdout, "") } else { ok() }).0
    };
    assert!(SYSTEM_READ.contains("uname -s"));
    let script = ssh_read_script();
    let system_at = script.find(SYSTEM_READ).expect("the read asks for the system");
    let arch_at = script.find(ARCH_READ).expect("the read asks for the chip");
    assert!(system_at < arch_at);
    for word in ["Linux", "Darwin", "FreeBSD"] {
        let read = read_ssh_machine(&reach(), &reads(&format!("system {word}\narch x86_64\n"))).await.unwrap();
        assert_eq!(read.system.as_deref(), Some(word));
        let adopted = bare_backend_of(reads(&format!("system {word}\n"))).adopt(&reach()).await.unwrap();
        assert_eq!(adopted.system.as_deref(), Some(word));
    }
    assert_eq!(read_ssh_machine(&reach(), &reads("system \n")).await.unwrap().system, None);
}

/// What an Ubuntu box prints for the facts script, as its own shell would.
const UBUNTU: &str = "pretty Ubuntu 24.04.3 LTS\nmac \nkernel Linux 6.8.0-79-generic\nuptime 96521.42\nboot \nhome /home/dev\n";

/// The machine handle as the runtime holds it, through the seam every kind answers on.
async fn machine_of<F>(answer: F) -> (Arc<dyn Machine>, Arc<Mutex<Vec<Carried>>>)
where
    F: Fn(&str) -> ExecResult + Send + Sync + 'static,
{
    let (t, carried) = fake_ssh(answer);
    let machine: Arc<dyn Machine> = backend_of(t).adopt(&reach()).await.unwrap().machine;
    (machine, carried)
}

#[tokio::test]
async fn the_three_the_pane_waits_on_are_read_off_the_machine_itself() {
    let facts_script = ssh_facts_script();
    let own = facts_script.clone();
    let (machine, carried) = machine_of(move |script| if script == own { res(0, UBUNTU, "") } else { ok() }).await;
    assert!(machine.offers().facts);
    let facts = machine.facts().await.unwrap();
    assert_eq!(
        facts,
        wsp_engine::machine::MachineFacts {
            os: "Ubuntu 24.04.3 LTS".into(),
            uptime_ms: JsNumber(96_521_420.0),
            folder: "/home/dev".into()
        }
    );
    // One round trip per read, and the same lines this computer's own machine is read with: one reader, two callers.
    assert_eq!(scripts(&carried)[1..].to_vec(), vec![facts_script.clone()]);
    assert!(facts_script.contains(&OS_READ.join("\n")));
    assert!(facts_script.contains(&UPTIME_READ.join("\n")));
    // Nothing is held: a machine somebody owns is rebooted and upgraded under wsp rather than by it.
    machine.facts().await.unwrap();
    assert_eq!(scripts(&carried)[1..].to_vec(), vec![facts_script.clone(), facts_script]);
}

#[tokio::test]
async fn a_mac_over_ssh_says_when_it_booted() {
    let now_s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let boot_sec = now_s - 7_200;
    let mac = format!(
        "pretty \nmac 15.6.1\nkernel Darwin 24.6.0\nuptime \nboot {{ sec = {boot_sec}, usec = 12 }} Thu Sep 10 17:24:09 2026\nhome /Users/maya\n"
    );
    let facts_script = ssh_facts_script();
    let (machine, _) = machine_of(move |script| if script == facts_script { res(0, &mac, "") } else { ok() }).await;
    let facts = machine.facts().await.unwrap();
    assert_eq!(facts.os, "macOS 15.6.1");
    assert_eq!(facts.folder, "/Users/maya");
    assert!(facts.uptime_ms.0 >= 7_200_000.0, "{}", facts.uptime_ms.0);
    assert!(facts.uptime_ms.0 < 7_205_000.0, "{}", facts.uptime_ms.0);
}

#[tokio::test]
async fn says_why_the_rows_sit_at_pending_once() {
    let stderr = Arc::new(Mutex::new("ssh: connect to host 10.0.0.5 port 2222: Connection refused\n".to_string()));
    let said_stderr = stderr.clone();
    let facts_script = ssh_facts_script();
    let (t, _) = transport(move |_, script, _| if script == facts_script { res(255, "", &said_stderr.lock().unwrap()) } else { ok() });
    let warned = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = warned.clone();
    let machine = SshMachine::with_warn(reach(), t, Arc::new(move |w: &str| sink.lock().unwrap().push(w.to_string()))).unwrap();
    // The caller shows pending and swallows the refusal, so a read that fails every 15 seconds would otherwise
    // say nothing anywhere; it says it here, and not four times an hour.
    for _ in 0..3 {
        let _ = machine.facts().await;
    }
    assert_eq!(warned.lock().unwrap().len(), 1);
    assert!(warned.lock().unwrap()[0].contains("Connection refused"));
    *stderr.lock().unwrap() = "ssh: Permission denied (publickey).\n".to_string();
    let _ = machine.facts().await;
    assert_eq!(warned.lock().unwrap().len(), 2);
    assert!(warned.lock().unwrap()[1].contains("Permission denied"));
    assert_eq!(
        *warned.lock().unwrap(),
        vec![
            "dev@10.0.0.5 did not say what it is over ssh (exit 255): ssh: connect to host 10.0.0.5 port 2222: Connection refused"
                .to_string(),
            "dev@10.0.0.5 did not say what it is over ssh (exit 255): ssh: Permission denied (publickey).".to_string(),
        ]
    );
}

#[tokio::test]
async fn a_machine_that_did_not_answer_leaves_the_rows_waiting() {
    let facts_script = ssh_facts_script();
    let (machine, _) = machine_of(move |script| {
        if script == facts_script {
            res(255, "", "debug1: Connecting to 10.0.0.5\nssh: connect to host 10.0.0.5 port 2222: Connection refused\n")
        } else {
            ok()
        }
    })
    .await;
    let refused = said(machine.facts().await);
    assert!(refused.contains("Connection refused"), "{refused}");
    assert!(!refused.contains("debug1:"), "{refused}");
    // A machine that answered the dial but not the lines is the same: no name of this computer's stands in for it.
    let (quiet, _) = machine_of(|_| res(0, "\n", "")).await;
    let quiet_said = said(quiet.facts().await);
    assert!(quiet_said.contains("did not say what it is over ssh"), "{quiet_said}");
}

/// What `ssh -G` answered on this computer for the dial, cut to the lines the read uses. A client that spells its
/// own folder with a tilde is spelled that way here too, since older ones do.
fn config_text() -> String {
    [
        "user dev",
        "hostname 10.0.0.5",
        "port 2222",
        "userknownhostsfile /Users/dev/.ssh/known_hosts ~/.ssh/known_hosts2",
        "globalknownhostsfile /etc/ssh/ssh_known_hosts /etc/ssh/ssh_known_hosts2",
        "hostkeyalgorithms ssh-ed25519-cert-v01@openssh.com,rsa-sha2-512-cert-v01@openssh.com,ssh-ed25519,ecdsa-sha2-nistp256,rsa-sha2-512,rsa-sha2-256,ssh-rsa",
    ]
    .join("\n")
}

/// What `ssh-keygen -F` printed for a machine known by a key of each type: its own comment lines, the rsa entry ahead
/// of the others in the file, and the ed25519 one hashed, which is how a Debian client writes every entry. The keys
/// are a throwaway set made for these cases; their fingerprints below are what `ssh-keygen -lf` printed.
fn found_text() -> String {
    [
        "# Host [10.0.0.5]:2222 found: line 3 ",
        "[10.0.0.5]:2222 ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAAAgQDdxs0iI8J5qm7+4hxS2LKxvEImRo4YOgbsh7o/q6LcNs+/euQcFejNmlxWKGPcPx6ZAFZEvYmdwe/yDUYsoAH66WB32nACpcm8ELAXEkXxQZEjmr8daugXOpWv0qDzNu/2u5+eMJJJRNDDDupbkvmCM74T9cCAVkHDjisNybGndQ==",
        "# Host [10.0.0.5]:2222 found: line 4 ",
        "|1|g4CYAxJ751PqMLyUC5r0xLd/ov4=|8lv3RgXEmKyoPgL0K37Xx452RDA= ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIC6sV8jQCzynkpUOM40rRIjA7tPstUSjC+A/LVMnAZhP",
        "# Host [10.0.0.5]:2222 found: line 5 ",
        "[10.0.0.5]:2222 ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBAK+5Ayg30RfuGD20d/q2F5UrbqEc1RarPuGTfhzyv0OHw3UiZIAlCu+Eg145j6r4sxS0Jm0uSKkMggV0oW70bY=",
        "",
    ]
    .join("\n")
}

const ED25519: &str = "ssh-ed25519 SHA256:Ge9MQ9S/Faik2WzsxidRXnoEOJRIHkJKTBkOJ903vq4";
const RSA: &str = "ssh-rsa SHA256:00VO3BTKQbJeAg/nTIZvht3husrdVHRx5/E4JC9fFBE";
const ECDSA: &str = "ecdsa-sha2-nistp256 SHA256:AgZa9U1SScxiIabgv4cA75Tb8rufTUtnU35jEB/IQWk";

fn algorithms() -> String {
    read_values(&config_text())["hostkeyalgorithms"].clone()
}

/// What `ssh-keygen -F` prints for a machine whose key this client trusts through an authority: the marker line for
/// the signing key and nothing of the machine's own, which is what a cold dial leaves behind as much as a warm one,
/// since accept-new writes no entry for a certificate it could already check. Measured on OpenSSH 9.6 against a
/// local sshd holding a CA signed host certificate.
fn ca_found_text() -> String {
    [
        "# Host [10.0.0.5]:2222 found: line 1 CA",
        "@cert-authority [10.0.0.5]:2222 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIKfANGYQHGkxSRKmhym/quMT78KoIKc7giNRg8nHlfF9",
        "",
    ]
    .join("\n")
}

/// The signing key the line above carries, as a fingerprint: what a record would stand on if the marker line were read
/// as a machine's own, which would make every machine that authority signed one machine.
const AUTHORITY_KEY: &str = "ssh-ed25519 SHA256:IGh+ehqXAjwW77irxqOmwHmu3koeqPEHgvUh/f71i34";

/// What `ssh-keyscan -p 2222 10.0.0.5` answered: the machine's own key, which is the key its certificate signs.
const OFFERED: &str = "[10.0.0.5]:2222 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOqrJ12qHMroq65CsKPtn/r4PuMQCbWynvVaYYSkR73z\n";
const OFFERED_KEY: &str = "ssh-ed25519 SHA256:VWJESUiByqqBIKEH1I0uAkXiG6HL1qBI3XJke8oNyMs";

const KNOWN_HOSTS: &str = "/Users/dev/.ssh/known_hosts";

fn keyscan_args() -> Vec<String> {
    strs(&["ssh-keyscan", "-T", "5", "-p", "2222", "10.0.0.5"])
}

#[tokio::test]
async fn the_key_is_read_off_the_clients_own_entry_with_nothing_dialled() {
    let config = config_text();
    let found = found_text();
    let (run, asked) = local_run(move |file, args| {
        if file == "ssh" {
            return res(0, &config, "");
        }
        let named = after(args, "-f");
        if named == KNOWN_HOSTS {
            res(0, &found, "")
        } else {
            res(255, "", &format!("Cannot stat {named}: No such file or directory\n"))
        }
    });
    assert_eq!(known_host_key(&reach(), &run).await.as_deref(), Some(ED25519));
    // The client is asked where it looks and what it prefers there, for the dial as it would make it, and nothing
    // is dialled: this is why the read costs nothing on a master a turn already has open.
    let asked = asked_of(&asked);
    let mut dial = vec!["ssh".to_string(), "-G".to_string()];
    dial.extend(ssh_dial_args(&reach()));
    dial.push("dev@10.0.0.5".to_string());
    assert_eq!(asked[0], dial);
    let keygen: Vec<Vec<String>> = asked[1..].iter().map(|a| a[..3].to_vec()).collect();
    assert_eq!(keygen, vec![strs(&["ssh-keygen", "-F", "[10.0.0.5]:2222"]); 4]);
    // A client that cannot say what it would do leaves the machine without an identity rather than with a guess.
    let (broken, _) = local_run(|_, _| res(255, "", "Bad configuration\n"));
    assert_eq!(known_host_key(&reach(), &broken).await, None);
    let found = found_text();
    let (no_target, _) = local_run(move |file, _| if file == "ssh" { res(0, "port 2222\n", "") } else { res(0, &found, "") });
    assert_eq!(known_host_key(&reach(), &no_target).await, None);
}

#[test]
fn the_entry_is_looked_up_under_the_name_and_port_the_client_writes() {
    let _env = env_lock();
    let values = read_values(&config_text());
    assert_eq!(known_host_target(&values).as_deref(), Some("[10.0.0.5]:2222"));
    // ssh's own port is written with no brackets, and an alias the person set is the whole name the client writes.
    let mut plain_port = values.clone();
    plain_port.insert("port".into(), "22".into());
    assert_eq!(known_host_target(&plain_port).as_deref(), Some("10.0.0.5"));
    let mut aliased = values.clone();
    aliased.insert("hostkeyalias".into(), "box-behind-a-tunnel".into());
    assert_eq!(known_host_target(&aliased).as_deref(), Some("box-behind-a-tunnel"));
    assert_eq!(known_host_target(&vals(&[("port", "2222")])), None);
    assert_eq!(
        known_host_files(&values, Some("/Users/dev")),
        strs(&["/Users/dev/.ssh/known_hosts", "/Users/dev/.ssh/known_hosts2", "/etc/ssh/ssh_known_hosts", "/etc/ssh/ssh_known_hosts2",])
    );
    // A person who turned a file off named no path to read.
    let off = vals(&[("userknownhostsfile", "none"), ("globalknownhostsfile", "none")]);
    assert_eq!(known_host_files(&off, None), Vec::<String>::new());
    // A tilde is the password database entry, which is what ssh expands one from: a home pointed elsewhere for
    // this process moves the file wsp reads nowhere, because it moves the file ssh reads nowhere.
    let elsewhere = std::env::temp_dir().join("wsp-not-a-home");
    let _home = HomeSet::to(elsewhere.to_str().unwrap());
    let tilde = vals(&[("userknownhostsfile", "~/.ssh/known_hosts")]);
    let expected = std::path::Path::new(&passwd_entry().1).join(".ssh").join("known_hosts");
    assert_eq!(known_host_files(&tilde, None), vec![expected.to_string_lossy().into_owned()]);
}

#[test]
fn the_key_picked_is_the_type_a_dial_would_negotiate() {
    let found = found_text();
    let algorithms = algorithms();
    // The rsa entry is first in the file and the ed25519 one is hashed, and what comes back is still the key this
    // client prefers, so two records of one machine cannot answer with two different keys for it.
    assert_eq!(host_key_found(&found, &algorithms).as_deref(), Some(ED25519));
    // An rsa-sha2 signature is made by an ssh-rsa key, so a client preferring those picks that entry.
    assert_eq!(host_key_found(&found, "rsa-sha2-512,ssh-ed25519").as_deref(), Some(RSA));
    assert_eq!(host_key_found(&found, "ecdsa-sha2-nistp256").as_deref(), Some(ECDSA));
    // Nothing to pick: a machine no entry names, a client that prefers none of the types it is known by, and a line
    // that marks an authority or a revoked key rather than naming a machine's own.
    assert_eq!(host_key_found("", &algorithms), None);
    assert_eq!(host_key_found(&found, "ssh-dss"), None);
    let marked = found
        .split('\n')
        .map(|line| if line.starts_with('#') { line.to_string() } else { format!("@revoked {line}") })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(host_key_found(&marked, &algorithms), None);
}

#[tokio::test]
async fn a_machine_trusted_through_an_authority_is_known_by_the_key_its_certificate_signs() {
    let asked: Asked = Arc::default();
    let config = config_text();
    let ca = ca_found_text();
    let run = run_into(&asked, move |file, args| {
        if file == "ssh" {
            return res(0, &config, "");
        }
        if file == "ssh-keyscan" {
            return res(0, OFFERED, "# 10.0.0.5:2222 SSH-2.0-OpenSSH_9.6\n");
        }
        let named = after(args, "-f");
        if named == KNOWN_HOSTS {
            res(0, &ca, "")
        } else {
            res(255, "", &format!("Cannot stat {named}\n"))
        }
    });
    assert_eq!(known_host_key(&reach(), &run).await.as_deref(), Some(OFFERED_KEY));
    // The machine is asked at the host and port the client resolved for the dial, not at the word that was typed.
    assert_eq!(asked_of(&asked).last().unwrap(), &keyscan_args());
    // The authority's own key is never the answer: every machine it signed would then be the same machine.
    assert_ne!(known_host_key(&reach(), &run).await.as_deref(), Some(AUTHORITY_KEY));
    // One machine, one identity, whichever of its addresses a record was made under: the client resolves both to
    // the same host, and what the machine offers there is what both records stand on.
    let by_name = SshReach { host: "box".into(), ..reach() };
    assert_eq!(known_host_key(&by_name, &run).await.as_deref(), Some(OFFERED_KEY));
}

#[tokio::test]
async fn only_a_machine_the_client_knows_by_nothing_else_is_asked() {
    let scans: Arc<Mutex<Vec<String>>> = Arc::default();
    let run = |found: String, offered: &'static str| -> SshLocalRun {
        let scans = scans.clone();
        let config = config_text();
        local_run(move |file, args| {
            if file == "ssh" {
                return res(0, &config, "");
            }
            if file == "ssh-keyscan" {
                scans.lock().unwrap().push(args.join(" "));
                return if offered.is_empty() { res(1, "", "") } else { res(0, offered, "") };
            }
            if after(args, "-f") == KNOWN_HOSTS {
                res(0, &found, "")
            } else {
                res(255, "", "")
            }
        })
        .0
    };
    // A client holding the machine's own entry beside the authority's line reads it there and dials nothing.
    let both = format!("{}{}", ca_found_text(), found_text());
    assert_eq!(known_host_key(&reach(), &run(both, OFFERED)).await.as_deref(), Some(ED25519));
    // A machine the client holds no line for at all is not one an authority stands behind: nothing is asked of it.
    assert_eq!(known_host_key(&reach(), &run(String::new(), OFFERED)).await, None);
    assert_eq!(*scans.lock().unwrap(), Vec::<String>::new());
    // The machine was asked and said nothing: the record goes without an identity rather than with a guess.
    assert_eq!(known_host_key(&reach(), &run(ca_found_text(), "")).await, None);
    assert_eq!(*scans.lock().unwrap(), strs(&["-T 5 -p 2222 10.0.0.5"]));
}

#[tokio::test]
async fn the_file_and_the_name_the_entry_is_written_under_come_off_one_reading() {
    // A config of the shape the review found: the person types a short name and the client dials, and writes the
    // entry under, the address behind it. A line built from the typed word would remove nothing.
    let behind_a_name = ["user dev", "hostname 10.0.0.5", "port 2222", "userknownhostsfile /Users/dev/.ssh/known_hosts_work"].join("\n");
    let run = |config: String| local_run(move |file, _| if file == "ssh" { res(0, &config, "") } else { res(255, "", "") }).0;
    assert_eq!(
        known_hosts_written(&reach_of("dev", "box", 2222), &run(behind_a_name.clone())).await,
        KnownHostsEntry { file: Some("/Users/dev/.ssh/known_hosts_work".into()), target: Some("[10.0.0.5]:2222".into()) }
    );
    // An alias the person set is the whole name the client writes, and ssh's own port carries no brackets.
    let aliased = known_hosts_written(&reach(), &run(format!("{behind_a_name}\nhostkeyalias box-behind-a-tunnel"))).await;
    assert_eq!(aliased.target.as_deref(), Some("box-behind-a-tunnel"));
    let plain_port = known_hosts_written(&reach(), &run("user dev\nhostname 10.0.0.5\nport 22".to_string())).await;
    assert_eq!(plain_port.target.as_deref(), Some("10.0.0.5"));
    // A client that cannot say what it would do names neither half, which leaves a screen its own default.
    let (broken, _) = local_run(|_, _| res(255, "", "Bad configuration\n"));
    assert_eq!(known_hosts_written(&reach(), &broken).await, KnownHostsEntry::default());
}

#[tokio::test]
async fn the_key_a_machine_itself_answers_with_is_one_read_of_its_ssh_port() {
    let asked: Asked = Arc::default();
    let run = |proxy: &str| -> SshLocalRun {
        let config = format!("{}\n{proxy}", config_text());
        run_into(&asked, move |file, _| {
            if file == "ssh" {
                res(0, &config, "")
            } else if file == "ssh-keyscan" {
                res(0, OFFERED, "# 10.0.0.5:2222 SSH-2.0-OpenSSH_9.6\n")
            } else {
                res(255, "", "")
            }
        })
    };
    // The machine is asked at the host and port the client resolved for the dial, and the key that comes back is
    // the one of the types this client prefers, so what a person is shown is what a dial would negotiate.
    assert_eq!(offered_host_key(&reach(), &run("")).await, OfferedKey { key: Some(OFFERED_KEY.into()), stopped_by: None });
    assert_eq!(asked_of(&asked).last().unwrap(), &keyscan_args());
    // Nothing is dialled where the client would reach the machine through a jump or a command of the person's: the
    // scan dials the resolved name itself, and whatever answered there is not that machine. The refusal names the
    // config line that stopped it, so the person knows what to read the key off instead.
    assert_eq!(
        offered_host_key(&reach(), &run("proxyjump bastion.example.com")).await,
        OfferedKey { key: None, stopped_by: Some("ProxyJump".into()) }
    );
    assert_eq!(
        offered_host_key(&reach(), &run("proxycommand nc %h %p")).await,
        OfferedKey { key: None, stopped_by: Some("ProxyCommand".into()) }
    );
    assert_eq!(asked_of(&asked).iter().filter(|a| a[0] == "ssh-keyscan").count(), 1);
    // A machine that answers no scan leaves the person with neither a key nor a reason of the config's.
    let config = config_text();
    let (no_scan, _) = local_run(move |file, _| if file == "ssh" { res(0, &config, "") } else { res(1, "", "") });
    assert_eq!(offered_host_key(&reach(), &no_scan).await, OfferedKey::default());
}

#[tokio::test]
async fn a_machine_reached_through_a_jump_is_asked_nothing() {
    let scans: Arc<Mutex<Vec<String>>> = Arc::default();
    let run = |proxy: &str| -> SshLocalRun {
        let scans = scans.clone();
        let config = format!("{}\n{proxy}", config_text());
        let ca = ca_found_text();
        local_run(move |file, args| {
            if file == "ssh" {
                return res(0, &config, "");
            }
            if file == "ssh-keyscan" {
                scans.lock().unwrap().push(args.join(" "));
                return res(0, OFFERED, "");
            }
            if after(args, "-f") == KNOWN_HOSTS {
                res(0, &ca, "")
            } else {
                res(255, "", "")
            }
        })
        .0
    };
    // The scan dials the resolved name itself, which on this network is some other machine or nothing at all, and
    // whatever answered there would be written down as this machine's identity.
    for proxy in ["proxyjump bastion.example.com", "proxycommand nc %h %p"] {
        assert_eq!(known_host_key(&reach(), &run(proxy)).await, None, "{proxy}");
    }
    assert_eq!(*scans.lock().unwrap(), Vec::<String>::new());
    // ssh prints neither line where the person set neither, so the machine is asked as it was.
    assert_eq!(known_host_key(&reach(), &run("")).await.as_deref(), Some(OFFERED_KEY));
    assert_eq!(scans.lock().unwrap().len(), 1);
}

/// A client whose config holds a spoo block with a HostName, a User and a Port of its own, the way the owner's does,
/// and nothing else: any other word comes back as its own hostname, which is what `ssh -G` prints for a name no block
/// renames. Every call is kept, so a case can say nothing was asked.
fn alias_config() -> (SshLocalRun, Asked) {
    local_run(|_file, args| {
        let word = args.last().unwrap().to_lowercase();
        let stdout = if word == "spoo" {
            "host spoo\nuser root\nhostname 178.156.161.168\nport 2222\n".to_string()
        } else {
            format!("host {word}\nuser dev\nhostname {word}\nport 22\n")
        };
        res(0, &stdout, "")
    })
}

#[tokio::test]
async fn a_bare_word_ssh_renames_is_the_alias_itself() {
    let (run, asked) = alias_config();
    assert_eq!(ssh_word_reach("spoo", None, None, &run).await.unwrap(), reach_of("root", "spoo", 2222));
    assert_eq!(asked_of(&asked), vec![strs(&["ssh", "-G", "spoo"])]);
}

#[tokio::test]
async fn a_port_or_a_key_the_person_typed_wins() {
    let (run, _) = alias_config();
    assert_eq!(
        ssh_word_reach("spoo", Some(2200), Some("/tmp/k/id"), &run).await.unwrap(),
        SshReach { user: "root".into(), host: "spoo".into(), port: 2200, key_path: Some("/tmp/k/id".into()) }
    );
}

#[tokio::test]
async fn a_word_ssh_does_not_rename_is_refused() {
    let (run, _) = alias_config();
    assert!(said(ssh_word_reach("nonsense", None, None, &run).await).contains(&ssh_word_refusal("nonsense")));
    assert!(!is_sentence_break(&ssh_word_refusal("nonsense")), "{}", ssh_word_refusal("nonsense"));
    let (shouting, _) = local_run(|_, _| res(0, "user dev\nhostname box\nport 22\n", ""));
    assert!(said(ssh_word_reach("BOX", None, None, &shouting).await).contains(&ssh_word_refusal("BOX")));
    let (broken, _) = local_run(|_, _| res(255, "", "Bad configuration option\n"));
    assert!(said(ssh_word_reach("spoo", None, None, &broken).await).contains(&ssh_word_refusal("spoo")));
}

#[tokio::test]
async fn a_numeric_word_is_refused_without_asking() {
    let (rewriting, asked) = local_run(|_, _| res(0, "user dev\nhostname 127.0.0.1\nport 22\n", ""));
    for word in ["123", "1.2.3", "2130706433", "0x7f000001", "0177.1"] {
        let refused = said(ssh_word_reach(word, None, None, &rewriting).await);
        assert!(refused.contains(&ssh_word_refusal(word)), "{word}: {refused}");
    }
    assert_eq!(asked_of(&asked), Vec::<Vec<String>>::new());
}

#[tokio::test]
async fn a_typed_login_is_read_as_it_always_was() {
    let (run, asked) = alias_config();
    assert_eq!(ssh_word_reach("root@spoo", None, None, &run).await.unwrap(), reach_of("root", "spoo", 22));
    let refused = said(ssh_word_reach("-oProxyCommand=touch", None, None, &run).await);
    assert!(refused.contains(&ssh_word_refusal("-oProxyCommand=touch")), "{refused}");
    assert_eq!(asked_of(&asked), Vec::<Vec<String>>::new());
}

#[tokio::test]
async fn is_the_hostname_the_config_gives_an_alias() {
    let (run, asked) = local_run(|_, args| {
        if args.last().map(String::as_str) == Some("root@me") {
            res(0, "user root\nhostname 127.0.0.1\nport 22\n", "")
        } else {
            res(0, "user root\nhostname 178.156.161.168\nport 2222\n", "")
        }
    });
    let spoo = reach_of("root", "spoo", 2222);
    assert_eq!(ssh_host_name(&spoo, &run).await.as_deref(), Some("178.156.161.168"));
    let mut dial = strs(&["ssh", "-G"]);
    dial.extend(ssh_dial_args(&spoo));
    dial.push("root@spoo".to_string());
    assert_eq!(asked_of(&asked)[0], dial);
    assert_eq!(ssh_host_name(&reach_of("root", "me", 22), &run).await.as_deref(), Some("127.0.0.1"));
    let (broken, _) = local_run(|_, _| res(255, "", "Bad configuration option\n"));
    assert_eq!(ssh_host_name(&reach_of("root", "spoo", 22), &broken).await.as_deref(), Some("spoo"));
    let backend = SshBackend::new(SshBackendOptions {
        host_name: Some(Arc::new(|_| async { Some("10.0.0.9".to_string()) }.boxed())),
        ..Default::default()
    });
    assert_eq!(backend.host_name_for(&reach_of("root", "spoo", 22)).await.as_deref(), Some("10.0.0.9"));
}

#[tokio::test]
async fn is_no_address_where_a_jump_carries_the_dial() {
    let (run, _) = local_run(|_, args| match args.last().map(String::as_str) {
        Some("root@inner") => res(0, "user root\nhostname 127.0.0.1\nproxyjump bastion\n", ""),
        Some("root@piped") => res(0, "user root\nhostname 127.0.0.1\nproxycommand nc %h %p\n", ""),
        _ => res(0, "", ""),
    });
    assert_eq!(ssh_host_name(&reach_of("root", "inner", 22), &run).await, None);
    assert_eq!(ssh_host_name(&reach_of("root", "piped", 22), &run).await, None);
}
