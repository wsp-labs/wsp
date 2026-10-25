// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/ecosystems.test.ts.
// The rebuilds a new worktree runs, run for real against a branch whose lockfile is stale: none of them may leave a
// tracked file changed, since the worktree would then hold a change nobody made.
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use wsp_catalog::git_hosts::{EcosystemModule, ECOSYSTEM_MODULES};

type Env = HashMap<String, String>;

fn module(id: &str) -> &'static EcosystemModule {
    ECOSYSTEM_MODULES.iter().find(|m| m.carry.id == id).unwrap_or_else(|| panic!("no {id} module"))
}

fn base_env() -> Env {
    std::env::vars().collect()
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .envs([
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@example.com"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@example.com"),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn on_path(tool: &str) -> bool {
    Command::new("sh").args(["-c", &format!("command -v {tool}")]).stdout(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("wsp-eco-").tempdir().unwrap()
}

/// Files written into a folder of their own.
fn folder(at: &Path, files: &[(&str, String)]) -> PathBuf {
    for (name, body) in files {
        let path = at.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    at.to_path_buf()
}

/// A repo of these files, its lockfile written by `lock`, then one more commit that changes `manifest` and leaves
/// the lockfile behind it, as a branch does that added a dependency and never ran the install.
fn stale_branch(at: &Path, files: &[(&str, String)], lock: &str, manifest: &str, body: &str, env: &Env) -> PathBuf {
    folder(at, files);
    let status = Command::new("sh")
        .args(["-c", lock])
        .current_dir(at)
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "{lock}");
    git(at, &["init", "-q"]);
    git(at, &["add", "."]);
    git(at, &["commit", "-qm", "locked"]);
    fs::write(at.join(manifest), body).unwrap();
    git(at, &["commit", "-qam", "a dependency the lockfile lacks"]);
    at.to_path_buf()
}

/// The rebuild as a new worktree runs it: in the folder, with stdin closed.
fn rebuild(command: &str, cwd: &Path, env: &Env) -> Option<i32> {
    let mut child = Command::new("sh")
        .args(["-c", command])
        .current_dir(cwd)
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let _ = stdin.write_all(b"");
    drop(stdin);
    child.wait().unwrap().code()
}

mod a_module_s_rebuild_on_a_branch_whose_lockfile_is_stale {
    use super::*;

    #[test]
    fn npm_s_leaves_package_lock_json_as_the_branch_holds_it() {
        let npm = module("npm");
        if !on_path("npm") {
            return;
        }
        let top = scratch();
        let top = top.path();
        // npm writes its logs and cache under the home.
        let mut env = base_env();
        env.insert("HOME".into(), top.join("home").to_string_lossy().into_owned());
        folder(&top.join("dep"), &[("package.json", r#"{"name":"dep","version":"1.0.0"}"#.into())]);
        let app = stale_branch(
            &top.join("app"),
            &[("package.json", r#"{"name":"app","version":"1.0.0"}"#.into()), (".gitignore", "node_modules/\n".into())],
            "npm install --package-lock-only --no-audit --no-fund",
            "package.json",
            r#"{"name":"app","version":"1.0.0","dependencies":{"dep":"file:../dep"}}"#,
            &env,
        );
        rebuild(&npm.rebuild, &app, &env);
        assert_eq!(git(&app, &["status", "--porcelain"]), "");
    }

    // Skipped in TS: it.skipIf(!onPath("cargo")), and cargo is not on the PATH of the shell the suite ran in.
    #[test]
    #[ignore = "skipped in TS when cargo is not on PATH"]
    fn cargo_s_refuses_it_and_leaves_cargo_lock_as_the_branch_holds_it() {
        let cargo = module("cargo");
        if !on_path("cargo") {
            return;
        }
        let top = scratch();
        let top = top.path();
        // A case's HOME is the run's own, where rustup's default folder holds no toolchain: name the login's.
        let rustup = std::env::var("RUSTUP_HOME").unwrap_or_else(|_| format!("{}/.rustup", std::env::var("HOME").unwrap()));
        let mut env = base_env();
        env.insert("HOME".into(), top.join("home").to_string_lossy().into_owned());
        env.insert("CARGO_HOME".into(), top.join("cargo").to_string_lossy().into_owned());
        env.insert("RUSTUP_HOME".into(), rustup);
        let crate_files = |name: &str| -> Vec<(&'static str, String)> {
            vec![
                ("Cargo.toml", format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n")),
                ("src/lib.rs", String::new()),
            ]
        };
        folder(&top.join("b"), &crate_files("b"));
        let mut a_files = crate_files("a");
        a_files.push((".gitignore", "target/\n".into()));
        let manifest = format!("{}b = {{ path = \"../b\" }}\n", crate_files("a")[0].1);
        let a = stale_branch(&top.join("a"), &a_files, "cargo generate-lockfile --offline", "Cargo.toml", &manifest, &env);
        assert_ne!(rebuild(&cargo.rebuild, &a, &env), Some(0));
        assert_eq!(git(&a, &["status", "--porcelain"]), "");
    }
}
