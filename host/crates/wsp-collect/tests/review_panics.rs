// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-collect for panics, hangs and stack overflows that a file's content, a command's output or the
//! environment can reach. Each failing test names the TS answer it asserts.
#![allow(clippy::result_large_err, clippy::type_complexity)]
mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::{FutureExt, StreamExt};
use indexmap::IndexMap;
use wsp_collect::host::{Host, HostExec, HostFs, Platform, RunOptions, Stat, StatKind};

/// Runs `f` on its own thread and gives its answer, or `None` when it has not answered in `secs` seconds (a hang).
fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs)).ok()
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    common::fake_host::block_on(f)
}

/// A computer whose files are what `texts` says for each path and whose commands answer what `answers` says.
struct ChaosFs {
    texts: Rc<dyn Fn(&str) -> Option<String>>,
}

fn kind_of_path(path: &str) -> StatKind {
    let name = path.rsplit('/').next().unwrap_or("");
    if name.contains('.') && !name.ends_with('/') && !path.ends_with("/ghostty/themes") {
        StatKind::File
    } else {
        StatKind::Dir
    }
}

impl HostFs for ChaosFs {
    fn stat<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<Stat>> {
        async move {
            let kind = kind_of_path(path);
            let bytes = (self.texts)(path).map_or(0, |t| t.len() as u64);
            Some(Stat { kind, bytes, mtime_ms: 1000.5 })
        }
        .boxed_local()
    }
    fn list<'a>(&'a self, _dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        async move {
            [
                "pdf",
                ".zshrc",
                "a.fish",
                "x.pub",
                "id_rsa",
                "config",
                "known_hosts",
                "0123456789abcdef0123456789abcdef_tokens.json",
                "mcp-remote-0.1.0",
                "\u{e9}\u{4e16}",
                "a b",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        }
        .boxed_local()
    }
    fn read_text<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<String>> {
        async move { (self.texts)(path) }.boxed_local()
    }
    fn walk<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        async move { vec![format!("{dir}/p1/a.jsonl"), format!("{dir}/p1/sub/b.jsonl"), format!("{dir}/c.json"), format!("{dir}/s/SKILL.md")] }.boxed_local()
    }
    fn lines<'a>(&'a self, path: &'a str, _holding: Option<&'a str>) -> LocalBoxStream<'a, std::io::Result<String>> {
        let lines: Vec<std::io::Result<String>> = match (self.texts)(path) {
            Some(t) => t.split('\n').map(|l| Ok(l.to_string())).collect(),
            None => Vec::new(),
        };
        futures::stream::iter(lines).boxed_local()
    }
}

struct ChaosExec {
    answers: Rc<dyn Fn(&str, &[String]) -> Option<String>>,
}

impl HostExec for ChaosExec {
    fn which<'a>(&'a self, _bin: &'a str) -> LocalBoxFuture<'a, bool> {
        async move { true }.boxed_local()
    }
    fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], _opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
        async move { (self.answers)(cmd, args) }.boxed_local()
    }
}

fn chaos_host(
    platform: Platform,
    texts: impl Fn(&str) -> Option<String> + 'static,
    answers: impl Fn(&str, &[String]) -> Option<String> + 'static,
) -> Host {
    let mut stores = IndexMap::new();
    let mut seeds = IndexMap::new();
    for id in ["claude", "codex", "gemini", "opencode", "cursor", "pi", "hermes"] {
        stores.insert(id.to_string(), format!("/Users/dev/.store/{id}"));
        seeds.insert(id.to_string(), format!("/Users/dev/.seed/{id}"));
    }
    Host {
        platform,
        home: "/Users/dev".to_string(),
        shell: Some("/bin/zsh".to_string()),
        terminal: Some("ghostty".to_string()),
        xdg_config_home: None,
        claude_config_dir: None,
        codex_home: None,
        stores: Some(stores),
        seeds: Some(seeds),
        fs: Rc::new(ChaosFs { texts: Rc::new(texts) }),
        exec: Rc::new(ChaosExec { answers: Rc::new(answers) }),
    }
}

fn in_child(case: &str) -> std::process::Output {
    let exe = std::env::current_exe().unwrap();
    std::process::Command::new(exe)
        .args(["--exact", "child_probe", "--nocapture", "--test-threads=1"])
        .env("REVIEW_PANICS_CASE", case)
        .output()
        .unwrap()
}

fn died(out: &std::process::Output) -> String {
    use std::os::unix::process::ExitStatusExt;
    let err = String::from_utf8_lossy(&out.stderr);
    let last: Vec<&str> = err.lines().rev().take(3).collect();
    format!("status {:?} signal {:?}; stderr tail {:?}", out.status.code(), out.status.signal(), last)
}

/// Not a test of its own: the body the parent tests run in a copy of this binary, named by REVIEW_PANICS_CASE.
#[test]
fn child_probe() {
    let Ok(case) = std::env::var("REVIEW_PANICS_CASE") else { return };
    let (name, n) = case.split_once(':').unwrap();
    let n: usize = n.parse().unwrap();
    match name {
        "eval" => {
            let text = format!("{}X_TOKEN=1\n", "eval ".repeat(n));
            let got = wsp_collect::detect::shell_rc::strip_exports(&text, None, &mut Vec::new(), false);
            println!("CHILD names {:?}", got.names);
        }
        _ => panic!("no such case {case}"),
    }
}

/// TS: packages/collect/src/detect/shell-rc.ts:182 `assignedNames(run.slice(1).join(" "))` for eval's words recurses;
/// node answers at 500 evals and throws a catchable RangeError at 1000. shell_rc.rs:395 is the same recursion and the
/// process dies of a stack overflow (SIGABRT) from 1000 evals, a 5 KB line, on a 2 MB thread stack.
#[test]
fn a_line_of_many_nested_evals_does_not_take_the_process_down() {
    let out = in_child("eval:1500");
    assert!(out.status.success(), "the child died: {}", died(&out));
}

/// TS: shell-rc.ts:324 `HEREDOC.exec(line.slice(i))` is O(1) per `<` (node scanned a 100 KB line of `<a` pairs in 44 ms and
/// a 320 KB one in 189 ms); shell_rc_strip.rs:192 `chars[i..].iter().collect()` copies the rest of the line into a
/// String at every `<`, so the cost is the square of the line: 20 KB took 1.4 s, 80 KB 21 s and 160 KB 91 s in a debug build.
#[test]
fn a_line_of_100_kb_with_many_less_than_signs_is_scanned_at_once() {
    use wsp_collect::detect::shell_rc_strip::{scan_line, Open};
    let line = "<a".repeat(50_000);
    let got = within(10, move || scan_line(&line, &Open::default()).heredoc);
    assert_eq!(got, Some(None), "scan_line did not answer in 10 s");
}

/// TS: seed.ts:45 adds the sizes of the rows broken out of a folder as floats; seed.rs:46 sums them as i64, so two
/// `du` lines of 9223372036854775807 KiB overflow (a panic in a debug build, a wrapped size in the release host).
#[test]
fn a_seed_menu_with_two_enormous_du_sizes_does_not_panic() {
    use wsp_collect::seed::{seed_menu, SeedOptions};
    let du = "9223372036854775807\t/Users/dev/proj/.claude/settings.local.json\n9223372036854775807\t/Users/dev/proj/.claude/.env\n1\t/Users/dev/proj/.claude\n";
    let host = chaos_host(
        Platform::Linux,
        |_| None,
        move |cmd, args| {
            let joined = args.join(" ");
            match cmd {
                "git" if joined.contains("--directory") => Some(".claude/\0".to_string()),
                "git" if joined.contains("ls-files") => Some(".claude/settings.local.json\0.claude/.env\0".to_string()),
                "du" => Some(du.to_string()),
                _ => None,
            }
        },
    );
    let o = SeedOptions { claude_state_home: "/Users/dev/.claude".to_string() };
    let got = catch_unwind(AssertUnwindSafe(|| block_on(seed_menu(&host, "/Users/dev/proj", &o))));
    assert!(got.is_ok(), "seed_menu panicked on two du sizes near i64::MAX");
}

/// TS: history/tally.ts:160 `total += b.calls` adds JavaScript numbers; history/cache.rs:51 reads 1.8446744073709552e19 as
/// u64::MAX and history/tally.rs:190 (and :160) add u64s, so two such buckets overflow (a panic in debug, a wrapped
/// count in release). Only a hand-edited cache file carries such counts.
#[test]
fn two_cached_buckets_of_enormous_call_counts_do_not_overflow_the_tally() {
    use wsp_collect::history::cache::{file_history_cache, HistoryCache};
    use wsp_collect::history::tally::usage_of;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");
    let bucket =
        |s: &str| format!(r#"{{"session":"{s}","calls":18446744073709551615,"commands":{{"ls":18446744073709551615}},"installs":{{}}}}"#);
    let text = format!(r#"{{"version":1,"parse":2,"files":{{"/f":{{"stamp":"1:1","buckets":[{},{}]}}}}}}"#, bucket("a"), bucket("b"));
    std::fs::write(&path, text).unwrap();
    let cache = file_history_cache(&path);
    let buckets = cache.get("/f", "1:1").expect("the cache holds the file");
    let got = catch_unwind(AssertUnwindSafe(|| usage_of(&buckets, None)));
    assert!(got.is_ok(), "usage_of panicked adding two call counts of u64::MAX");
}

/// TS: terminal.ts:55 `kittyFont` answered "Fira Code" for this 600 KB kitty.conf. terminal.rs:91 `group1` reads
/// KITTY_LINE's `Err(BacktrackLimitExceeded)` (its lookbehind puts it in fancy-regex's VM) as no match: `None`.
#[test]
fn kitty_font_is_found_in_a_kitty_conf_of_600_kb() {
    let conf = format!("{}font_family Fira Code\n", "# a comment line to make the file long\n".repeat(600_000 / 40));
    assert_eq!(wsp_collect::detect::terminal::kitty_font(&conf).as_deref(), Some("Fira Code"));
}

/// TS: terminal.ts:48 `weztermFont` answered "Hack"; terminal.rs:122 `WEZ_FONT.captures(..).ok().flatten()` reads the
/// same error as no font and falls back to the built-in "JetBrains Mono" (1.2 MB of Lua).
#[test]
fn wezterm_font_is_found_in_a_lua_file_of_1200_kb() {
    let lua = format!("{}return {{ font = wezterm.font('Hack') }}\n", "local x = 1 + 1 -- long\n".repeat(1_200_000 / 20));
    assert_eq!(wsp_collect::detect::terminal::wezterm_font(&lua).as_deref(), Some("Hack"));
}

/// TS: terminal.ts:64 `alacrittyFont` answered "Hack"; terminal.rs:91 gives `None` for 1.2 MB of toml.
#[test]
fn alacritty_font_is_found_in_a_toml_file_of_1200_kb() {
    let toml = format!("{}[font.normal]\nfamily = \"Hack\"\n", "# a comment line to make the file long\n".repeat(1_200_000 / 40));
    assert_eq!(wsp_collect::detect::terminal::alacritty_font(&toml).as_deref(), Some("Hack"));
}
