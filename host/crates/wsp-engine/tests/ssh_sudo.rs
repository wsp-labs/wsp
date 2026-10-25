// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/ssh-sudo.test.ts: root over a login that is not root. Every script the ssh client
//! carries runs as root: as the login where it is root, else under sudo, told never to ask, or told to read the one
//! password line the client puts ahead of the script's own input. The read of which road a login has runs as the
//! login itself. Specs: spec/ssh_dial.md, spec/ssh_machine.md.
mod roads_common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};

use futures::FutureExt;
use roads_common::*;
use tempfile::TempDir;
use wsp_engine::machine::{ExecResult, Machine, RunOptions};
use wsp_engine::ssh_backend::{
    read_ssh_sudo, ssh_args, ssh_client, ssh_dial, try_ssh_sudo, SshArgsOptions, SshAs, SshBackend, SshBackendOptions, SshReach, SshRide,
    SshRiding, SshSudo, SshTransport, SshTransportOptions, SSH_SUDO_READ, SSH_SUDO_TRY,
};

static ENV: Mutex<()> = Mutex::new(());

fn reach() -> SshReach {
    SshReach { user: "maya".to_string(), host: "box".to_string(), port: 2222, key_path: None }
}

#[derive(Clone, Copy, PartialEq)]
enum Sudo {
    Runs,
    Refuses,
    Absent,
    WantsTty,
    Mixed,
}

/// A folder of stand-ins put first on PATH, and the process env that points at it. The lock is held for the whole
/// test, since PATH and HOME are the process's, and both are put back however the test ends.
struct Stubbed {
    dir: TempDir,
    saved_path: Option<String>,
    saved_home: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl Stubbed {
    fn new() -> Self {
        let lock = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::Builder::new().prefix("wsp-ssh-sudo-").tempdir().unwrap();
        let saved_path = std::env::var("PATH").ok();
        let saved_home = std::env::var("HOME").ok();
        std::env::set_var("PATH", format!("{}:{}", dir.path().display(), saved_path.clone().unwrap_or_default()));
        // The client makes its master socket folder under the home; this one is the test's own.
        std::env::set_var("HOME", dir.path());
        Stubbed { dir, saved_path, saved_home, _lock: lock }
    }

    fn dir(&self) -> &Path {
        self.dir.path()
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir().join(name)
    }

    fn read_file(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.file(name)).unwrap()
    }

    fn read_text(&self, name: &str) -> String {
        String::from_utf8(self.read_file(name)).unwrap()
    }

    fn exists(&self, name: &str) -> bool {
        self.file(name).exists()
    }

    fn put(&self, name: &str, body: &str) {
        let path = self.file(name);
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// An ssh that runs what follows the login here, as sshd hands it to a shell; an id that answers the login's uid;
    /// and a sudo that writes down how it was asked and the line it read where it was told to read one, then runs
    /// the command.
    fn stubs(&self, uid: u32, sudo: Sudo) {
        let dir = self.dir().display();
        self.put("ssh", "while [ $# -gt 0 ]; do case \"$1\" in *@*) shift; break ;; esac; shift; done\nexec /bin/sh -c \"$*\"");
        self.put("id", &format!("[ \"$1\" = -u ] && echo {uid} || /usr/bin/id \"$@\""));
        if sudo == Sudo::Absent {
            let _ = std::fs::remove_file(self.file("sudo"));
            return;
        }
        let mut lines = vec![format!("printf '%s\\n' \"$*\" > {dir}/sudo-args")];
        if sudo == Sudo::Refuses {
            lines.push("echo 'sudo: a password is required' >&2".to_string());
            lines.push("exit 1".to_string());
        }
        if sudo == Sudo::WantsTty {
            lines.push("echo 'sudo: sorry, you must have a tty to run sudo' >&2".to_string());
            lines.push("exit 1".to_string());
        }
        if sudo == Sudo::Mixed {
            // One NOPASSWD entry among password ones: the list passes, the validate wants the password.
            lines.push("case \"$*\" in \"-n -v\") echo 'sudo: a password is required' >&2; exit 1 ;; \"-n -l bash -c true\") echo /usr/bin/bash; exit 0 ;; esac".to_string());
        }
        lines.push(format!("case \"$1\" in -S) IFS= read -r line; printf '%s' \"$line\" > {dir}/sudo-read ;; esac"));
        lines.push("while [ $# -gt 0 ] && [ \"$1\" != bash ]; do shift; done".to_string());
        lines.push("exec \"$@\"".to_string());
        self.put("sudo", &lines.join("\n"));
    }
}

impl Drop for Stubbed {
    fn drop(&mut self) {
        match &self.saved_path {
            Some(v) => std::env::set_var("PATH", v),
            None => std::env::remove_var("PATH"),
        }
        match &self.saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

async fn client(script: &str, opts: SshTransportOptions) -> ExecResult {
    let carry = ssh_client();
    carry(reach(), script.to_string(), opts).await
}

fn ride(ride: SshRide) -> SshTransportOptions {
    SshTransportOptions { ride, ..Default::default() }
}

#[tokio::test]
async fn as_it_is_on_a_root_login_with_sudo_never_asked() {
    let env = Stubbed::new();
    env.stubs(0, Sudo::Runs);
    let said = client("echo \"it's $((1 + 1))\"", SshTransportOptions::default()).await;
    assert_eq!((said.exit_code, said.stdout.as_str()), (0, "it's 2\n"));
    assert!(!env.exists("sudo-args"));
}

#[tokio::test]
async fn under_sudo_n_on_a_login_that_is_not_root() {
    let env = Stubbed::new();
    env.stubs(1000, Sudo::Runs);
    std::env::set_var("SUDO_USER", "maya");
    let said = client("echo \"[$SUDO_USER] it's $((1 + 1))\"", SshTransportOptions::default()).await;
    std::env::remove_var("SUDO_USER");
    assert_eq!((said.exit_code, said.stdout.as_str()), (0, "[] it's 2\n"));
    assert!(env.read_text("sudo-args").starts_with("-n bash -c "), "{}", env.read_text("sudo-args"));
}

#[tokio::test]
async fn with_a_password_sudo_reads_the_one_line_ahead_of_the_scripts_input() {
    let env = Stubbed::new();
    env.stubs(1000, Sudo::Runs);
    let bytes = b"line one\nline two, no newline at the end".to_vec();
    let opts = SshTransportOptions {
        ride: SshRide::RootByPassword("Tq not-a-real pw!".to_string()),
        stdin: Some(bytes.clone()),
        ..Default::default()
    };
    let said = client(&format!("cat > {}/landed; echo done", env.dir().display()), opts).await;
    assert_eq!((said.exit_code, said.stdout.as_str()), (0, "done\n"));
    assert_eq!(env.read_text("sudo-read"), "Tq not-a-real pw!");
    assert_eq!(env.read_file("landed"), bytes);
    // Told to read the password off its input, to say nothing as it asks, and to ask again whatever it cached, so
    // the line is always sudo's.
    assert!(env.read_text("sudo-args").starts_with("-S -k -p  bash -c "), "{}", env.read_text("sudo-args"));
}

#[test]
fn a_password_never_rides_the_command_line() {
    let args = ssh_args(&reach(), "true", &SshArgsOptions { as_: SshAs::RootByPassword, stdin: true, ..Default::default() });
    assert!(!args.join(" ").contains("Tq"));
    assert!(!args.iter().any(|a| a == "-n"));
}

#[tokio::test]
async fn as_the_login_itself_for_the_reads_that_ask_about_the_login() {
    let env = Stubbed::new();
    env.stubs(1000, Sudo::Refuses);
    let said = client("id -u", ride(SshRide::Login)).await;
    assert_eq!((said.exit_code, said.stdout.as_str()), (0, "1000\n"));
    ssh_dial(&reach(), &ssh_client()).await.unwrap();
    assert!(!env.exists("sudo-args"));
    let args = ssh_args(&reach(), "id -u", &SshArgsOptions { as_: SshAs::Login, ..Default::default() });
    assert_eq!(args[args.len() - 3..].to_vec(), vec!["bash", "-c", "'id -u'"]);
}

#[tokio::test]
async fn stops_with_sudos_own_line_on_a_login_whose_sudo_asks() {
    let env = Stubbed::new();
    env.stubs(1000, Sudo::Refuses);
    let said = client("echo ran", SshTransportOptions::default()).await;
    assert_eq!(said.exit_code, 1);
    assert_eq!(said.stdout, "");
    assert!(said.stderr.contains("a password is required"), "{}", said.stderr);
}

#[test]
fn reads_root_free_asks_tty_none() {
    let env = Stubbed::new();
    // The read run by a shell here against the stand-ins, with PATH holding nothing else where sudo is absent.
    let read = |uid: u32, sudo: Sudo| -> String {
        env.stubs(uid, sudo);
        let path = if sudo == Sudo::Absent { env.dir().display().to_string() } else { format!("{}:/usr/bin:/bin", env.dir().display()) };
        let ran = Command::new("/bin/sh").args(["-c", SSH_SUDO_READ]).env_clear().env("PATH", path).output().unwrap();
        String::from_utf8(ran.stdout).unwrap().trim().to_string()
    };
    assert_eq!(read(0, Sudo::Refuses), "WSP_SUDO root");
    assert_eq!(read(1000, Sudo::Runs), "WSP_SUDO free");
    // A list of the command, never the command: the read runs before the box's key is compared and opens no
    // session as root.
    assert_eq!(env.read_text("sudo-args"), "-n -l bash -c true\n");
    assert_eq!(read(1000, Sudo::Refuses), "WSP_SUDO asks");
    // A sudoers that mixes one NOPASSWD command with password ones lists bash without a password and validates
    // only with one: it asks, since every script after the read is refused under sudo -n.
    assert_eq!(read(1000, Sudo::Mixed), "WSP_SUDO asks");
    // Defaults requiretty: sudo -n says so in its own line before any password is in play.
    assert_eq!(read(1000, Sudo::WantsTty), "WSP_SUDO tty");
    assert_eq!(read(1000, Sudo::Absent), "WSP_SUDO none");
}

/// A transport that answers the read with `road` and the try of a password the way sudo would.
fn answering_box(road: &str, tried_stdout: Option<&str>, tried_stderr: Option<&str>) -> (SshTransport, Arc<Mutex<Vec<Carried>>>) {
    let (road, tried_stdout, tried_stderr) = (road.to_string(), tried_stdout.map(str::to_string), tried_stderr.map(str::to_string));
    transport(move |_, script, _| {
        if script == SSH_SUDO_READ {
            return res(0, &format!("WSP_SUDO {road}\n"), "");
        }
        res(if tried_stdout.is_none() { 1 } else { 0 }, tried_stdout.as_deref().unwrap_or(""), tried_stderr.as_deref().unwrap_or(""))
    })
}

#[tokio::test]
async fn reads_the_road_as_the_login() {
    let (taken, asked) = answering_box("asks", Some("WSP_SUDO taken\n"), None);
    assert_eq!(read_ssh_sudo(&reach(), &taken).await.unwrap(), SshSudo::Asks);
    {
        let asked = asked.lock().unwrap();
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].script, SSH_SUDO_READ);
        assert!(asked[0].opts.timeout_ms.is_some());
        assert_eq!(asked[0].opts.ride, SshRide::Login);
        assert!(asked[0].opts.on_line.is_none());
        assert!(asked[0].opts.stdin.is_none());
    }
    assert_eq!(try_ssh_sudo(&reach(), "pw one", &taken).await, SshSudo::Taken);
    let asked = asked.lock().unwrap().clone();
    assert_eq!(asked[1].script, SSH_SUDO_TRY);
    assert_eq!(asked[1].opts.ride, SshRide::Login);
    assert_eq!(asked[1].opts.stdin.as_deref(), Some(b"pw one\n".as_slice()));
    assert!(!SSH_SUDO_TRY.contains("pw one"));

    let (wrong, _) =
        answering_box("asks", None, Some("Sorry, try again.\nsudo: no password was provided\nsudo: 1 incorrect password attempt\n"));
    assert_eq!(try_ssh_sudo(&reach(), "pw", &wrong).await, SshSudo::Wrong);
    let (none, _) = answering_box("asks", None, Some("maya is not in the sudoers file.\n"));
    assert_eq!(try_ssh_sudo(&reach(), "pw", &none).await, SshSudo::None);
    let (tty, _) = answering_box("asks", None, Some("sudo: sorry, you must have a tty to run sudo\n"));
    assert_eq!(try_ssh_sudo(&reach(), "pw", &tty).await, SshSudo::Tty);
    for (road, want) in [("root", SshSudo::Root), ("free", SshSudo::Free), ("tty", SshSudo::Tty), ("none", SshSudo::None)] {
        assert_eq!(read_ssh_sudo(&reach(), &answering_box(road, None, None).0).await.unwrap(), want);
    }
}

#[tokio::test]
async fn says_sshs_own_line_where_the_login_does_not_stand() {
    let (refused, _) = transport(|_, _, _| res(255, "", "maya@box: Permission denied (publickey).\n"));
    let said = read_ssh_sudo(&reach(), &refused).await.unwrap_err();
    assert!(said.to_string().contains("Permission denied (publickey)."), "{said}");
}

#[tokio::test]
async fn a_backend_riding_a_password_hands_it_to_every_script() {
    let (transport, carried) = transport(|_, _, _| res(0, "home /root\n", ""));
    let opts = SshBackendOptions { transport: Some(transport), host_key: Some(Arc::new(|_| async { None }.boxed())), ..Default::default() };
    let base = SshBackend::new(opts);
    let rides = || -> Vec<SshRide> { carried.lock().unwrap().iter().map(|c| c.opts.ride.clone()).collect() };
    let pw = SshRiding::SudoPassword("pw".to_string());
    let adopted = base.riding(pw.clone()).adopt(&reach()).await.unwrap();
    adopted.machine.run("true", RunOptions { deadline_ms: 1000, ..Default::default() }).await.unwrap();
    let by_password = SshRide::RootByPassword("pw".to_string());
    assert_eq!(rides(), vec![by_password.clone(), by_password]);
    // The backend it came from carries none, and a read as the login through the riding one carries none either.
    base.adopt(&reach()).await.unwrap();
    assert_eq!(rides().last().unwrap(), &SshRide::Root);
    let _ = base.riding(pw).sudo_for(&reach()).await;
    assert_eq!(rides().last().unwrap(), &SshRide::Login);
}

// Adapted: the TS forces an asLogin read with a sudoPassword past the type checker. SshRide has no variant that
// carries both, so this is a Login ride with stdin, and its output carries none of a password.
#[tokio::test]
async fn puts_the_password_ahead_of_the_scripts_input_only_on_the_road_that_runs_it_under_sudo() {
    let env = Stubbed::new();
    env.stubs(1000, Sudo::Refuses);
    let opts = SshTransportOptions { ride: SshRide::Login, stdin: Some(b"own\n".to_vec()), ..Default::default() };
    let said = client("cat; echo end", opts).await;
    assert_eq!((said.exit_code, said.stdout.as_str()), (0, "own\nend\n"));
    assert!(!said.stdout.contains("Tq-pw") && !said.stderr.contains("Tq-pw"));
    assert!(!env.exists("sudo-read"));
}
