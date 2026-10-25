// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the exec side of live_host.rs against packages/collect/src/live-host.ts, the NUL refusal seam, and the
//! JavaScript regex translations. Every expected value below was printed by node 22.23 running the TS (the live-host.ts
//! copy for the process cases, the TS `detectPlugins` over `nodeHost()` for the plugin cases).
mod common;

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use wsp_catalog::catalog::{catalog_agents, mcp_agents};
use wsp_catalog::js_regex::JsRegex;
use wsp_collect::detect::commands::{called_commands, Syntax};
use wsp_collect::detect::mcp::detect_mcp;
use wsp_collect::detect::plugins::{detect_plugins, PluginsRead};
use wsp_collect::host::{Host, HostExec, Platform, RunOptions};
use wsp_collect::live_host::{spawn_run, NodeExec, NodeFs};

fn sv(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

async fn sh(script: &str, opts: RunOptions) -> Option<String> {
    NodeExec.run("sh", &sv(&["-c", script]), opts).await
}

fn budget(ms: u64) -> RunOptions {
    RunOptions { timeout_ms: Some(ms), ..RunOptions::default() }
}

/// FINDING (NIT): live-host.ts:152 spreads `process.env`, and node's `process.env` cannot read a variable whose name is
/// only digits (`process.env["7"]` is undefined, and the spread leaves it out), so the TS child never sees one.
/// live_host.rs:313-314 passes every variable. node printed no `7=x` line for a parent holding `7=x`.
#[tokio::test(flavor = "current_thread")]
async fn a_variable_named_with_digits_only_does_not_reach_the_child() {
    std::env::set_var("7", "x");
    let out = NodeExec.run("env", &[], RunOptions::default()).await.expect("env runs");
    std::env::remove_var("7");
    assert!(!out.lines().any(|l| l == "7=x"), "the child saw 7=x");
}

/// FINDING (NIT): live-host.ts:203 hands a budget past 2^31-1 ms to `setTimeout`, which node turns into 1 ms
/// ("Timeout duration was set to 1"), so the TS run is cut at once and answers undefined. live_host.rs:495 waits the
/// full budget and answers the output. node answered undefined for `sleep 0.5; echo hi` under timeoutMs 2**31.
#[tokio::test(flavor = "current_thread")]
async fn a_budget_past_two_to_the_31_ms_is_node_s_one_millisecond() {
    let env: Vec<(String, String)> = std::env::vars().collect();
    let out = spawn_run("sh", &sv(&["-c", "sleep 0.5; echo hi"]), &env, budget(1 << 31)).await.expect("no refusal");
    assert_eq!(out, None);
}

/// FINDING (NIT): live-host.ts:189 spawns through libuv, which gives the child default dispositions for every signal; a
/// host started with SIGHUP or SIGINT ignored (nohup, a `&` job of a script) hands its children the same under
/// live_host.rs:475-484, where std resets only SIGPIPE. node printed `SigIgn: 0000000000000000` in the child for a
/// parent that ignored HUP and INT. The test re-runs this binary with both ignored.
#[tokio::test(flavor = "current_thread")]
async fn a_child_gets_default_signal_dispositions_whatever_the_host_ignores() {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", "child_prints_its_ignored_signals", "--ignored", "--nocapture", "--test-threads=1"]);
    unsafe {
        cmd.pre_exec(|| {
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            libc::signal(libc::SIGINT, libc::SIG_IGN);
            Ok(())
        });
    }
    let out = cmd.output().expect("re-run");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let at = text.find("REVPROC SigIgn:").unwrap_or_else(|| panic!("no answer: {text}"));
    assert_eq!(text[at..].lines().next().unwrap(), "REVPROC SigIgn:\t0000000000000000");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "run by a_child_gets_default_signal_dispositions_whatever_the_host_ignores"]
async fn child_prints_its_ignored_signals() {
    let out = sh("grep SigIgn /proc/$$/status", RunOptions::default()).await.unwrap_or_default();
    println!("REVPROC {}", out.trim_end());
}

fn host_at(home: &Path) -> Host {
    Host {
        platform: Platform::Linux,
        home: home.to_str().unwrap().to_string(),
        shell: None,
        terminal: None,
        xdg_config_home: None,
        claude_config_dir: None,
        codex_home: None,
        stores: None,
        seeds: None,
        fs: Rc::new(NodeFs),
        exec: Rc::new(NodeExec),
    }
}

/// Claude Code's plugins alone, as the TS oracle read them: the Codex shelf asks the real `codex` of this computer.
async fn claude_plugins(home: &Path) -> PluginsRead {
    let agents: Vec<_> = catalog_agents().into_iter().filter(|a| a.id == "claude").collect();
    detect_plugins(&host_at(home), Some(&agents), &[]).await
}

fn home_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    for (rel, text) in files {
        let p = home.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    home
}

const INDEX: &str = ".claude/plugins/installed_plugins.json";
const KNOWN: &str = ".claude/plugins/known_marketplaces.json";
const NOT_READ: &str =
    "plugins: Claude Code's plugins were not read: The argument 'args[1]' must be a string without null bytes. Received ";

/// Control: the same file with a plain path. node (TS `detectPlugins` over `nodeHost()`) gave one row, `p@m`, missing
/// its folder, path `/nowhere/xy`, and nothing refused.
#[tokio::test(flavor = "current_thread")]
async fn a_plain_install_path_reads_as_one_row_with_its_folder_missing() {
    let home = home_with(&[(INDEX, r#"{"version":2,"plugins":{"p@m":[{"scope":"user","installPath":"/nowhere/xy","version":"1"}]}}"#)]);
    let read = claude_plugins(home.path()).await;
    assert!(read.refused.is_empty(), "{:?}", read.refused);
    assert_eq!(read.plugins.len(), 1);
    assert_eq!(read.plugins[0].id, "p@m");
    assert_eq!(read.plugins[0].path.as_deref(), Some("/nowhere/xy"));
}

/// JSON's escape for NUL, built here so the text of this file never spells it.
fn nul_escape() -> String {
    format!("\\u{:04}", 0)
}

/// FINDING (MINOR): plugins.ts:86 runs `sh -c <peek line>` and the line carries each install path of
/// installed_plugins.json, read by `JSON.parse`, so a JSON NUL escape in `installPath` puts a NUL in an argument. node's
/// `spawn` throws, `host.exec.run` rejects, and `detectPlugins` (plugins.ts:111) catches it into
/// `{ plugins: [], refused: ["plugins: Claude Code's plugins were not read: The argument 'args[1]' must be a string
/// without null bytes. Received ..."] }`. live_host.rs:322 turns the refusal into `None`, the second peek reads as
/// unanswered, and the Rust answers one row with its folder missing and nothing refused.
#[tokio::test(flavor = "current_thread")]
async fn a_nul_in_an_install_path_is_refused_as_node_refuses_it() {
    let index = r#"{"version":2,"plugins":{"p@m":[{"scope":"user","installPath":"/nowhere/x<NUL>y","version":"1"}]}}"#;
    let home = home_with(&[(INDEX, &index.replace("<NUL>", &nul_escape()))]);
    let read = claude_plugins(home.path()).await;
    assert!(read.plugins.is_empty(), "rows: {:?}", read.plugins.iter().map(|p| &p.id).collect::<Vec<_>>());
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert!(read.refused[0].starts_with(NOT_READ), "{:?}", read.refused);
}

/// FINDING (MINOR): the same refusal through known_marketplaces.json `installLocation`, which the shelf peeks for the
/// marketplace of a listed plugin (plugins-claude.ts:231). node answered the same refused line and no rows.
#[tokio::test(flavor = "current_thread")]
async fn a_nul_in_a_marketplace_location_is_refused_as_node_refuses_it() {
    let known = r#"{"m":{"installLocation":"/nowhere/m<NUL>z","source":{"source":"github","repo":"a/b"}}}"#.replace("<NUL>", &nul_escape());
    let home = home_with(&[
        (INDEX, r#"{"version":2,"plugins":{"p@m":[{"scope":"user","installPath":"/nowhere/xy","version":"1"}]}}"#),
        (KNOWN, &known),
    ]);
    let read = claude_plugins(home.path()).await;
    assert!(read.plugins.is_empty(), "rows: {:?}", read.plugins.iter().map(|p| &p.id).collect::<Vec<_>>());
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert!(read.refused[0].starts_with(NOT_READ), "{:?}", read.refused);
}

/// A `codex` that answers `codex app-server` the way the shelf asks: one local marketplace `lm` that config.toml names a
/// plugin of, whose `source` is `/nowhere/m`, or `/nowhere/m` NUL `z` where FAKE_CODEX_NUL is set (a JSON NUL escape).
const FAKE_CODEX: &str = r#"#!/usr/bin/env python3
import sys, json, os
SOURCE = "/nowhere/m\x00z" if os.environ.get("FAKE_CODEX_NUL") else "/nowhere/m"
for line in sys.stdin:
    line = line.strip()
    if not line.startswith("{"):
        continue
    m = json.loads(line)
    if "id" not in m:
        continue
    method = m.get("method")
    if method == "initialize":
        res = {"codexHome": "/nowhere/codex-home"}
    elif method == "plugin/list":
        res = {"marketplaces": []}
    elif method == "config/read":
        res = {"config": {"plugins": {"x@lm": {"enabled": True}}, "marketplaces": {"lm": {"source_type": "local", "source": SOURCE}}}, "layers": []}
    else:
        res = {}
    print(json.dumps({"id": m["id"], "jsonrpc": "2.0", "result": res}), flush=True)
"#;

fn with_fake_codex(nul: bool) -> Option<String> {
    let bin = tempfile::tempdir().unwrap();
    let script = bin.path().join("codex");
    std::fs::write(&script, FAKE_CODEX).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.path().display(), std::env::var("PATH").unwrap_or_default());
    let mut vars = vec![("PATH", path)];
    if nul {
        vars.push(("FAKE_CODEX_NUL", "1".to_string()));
    }
    rerun_with("child_prints_codex_plugins", 60, &vars)
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "run by the codex plugin tests"]
async fn child_prints_codex_plugins() {
    let home = tempfile::tempdir().unwrap();
    let agents: Vec<_> = catalog_agents().into_iter().filter(|a| a.id == "codex").collect();
    let read = detect_plugins(&host_at(home.path()), Some(&agents), &[]).await;
    let ids: Vec<&str> = read.plugins.iter().map(|p| p.id.as_str()).collect();
    let said = read.refused.first().is_some_and(|r| {
        r.starts_with("plugins: Codex's plugins were not read: The argument 'args[1]' must be a string without null bytes. Received ")
    });
    println!("REVPROC plugins={ids:?} refused={} refused_like_node={said}", read.refused.len());
}

/// Control: a local marketplace source with no NUL reads as one row, `x@lm`, its marketplace folder missing, nothing
/// refused (node, over the same fake `codex`).
#[test]
fn a_codex_marketplace_source_without_a_nul_reads_as_one_row() {
    let got = with_fake_codex(false).expect("child ran");
    assert_eq!(answer(&got), r#"plugins=["x@lm"] refused=0 refused_like_node=false"#);
}

/// FINDING (MINOR): plugins-codex.ts:104 peeks each local marketplace `source` that config.toml names, and the answer
/// of `codex app-server` (`config/read`) is JSON, so a JSON NUL escape in the source is a NUL in the `sh -c` line. node's `spawn`
/// throws, `detectPlugins` catches it and answers no rows and one refused line, "plugins: Codex's plugins were not
/// read: The argument 'args[1]' must be a string without null bytes. Received ...". The Rust reads the peek as
/// unanswered and answers the row `x@lm` with its marketplace missing and nothing refused.
#[test]
fn a_nul_in_a_codex_marketplace_source_is_refused_as_node_refuses_it() {
    let got = with_fake_codex(true).expect("child ran");
    assert_eq!(answer(&got), r#"plugins=[] refused=1 refused_like_node=true"#);
}

/// Re-runs this binary on one `#[ignore]`d test and answers its stdout, or None after `secs` (the child is killed).
fn rerun(test: &str, secs: u64) -> Option<String> {
    rerun_with(test, secs, &[])
}

fn rerun_with(test: &str, secs: u64, vars: &[(&str, String)]) -> Option<String> {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", test, "--ignored", "--nocapture", "--test-threads=1"]);
    for (k, v) in vars {
        command.env(k, v);
    }
    let mut child = command.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().expect("re-run");
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if child.try_wait().unwrap().is_some() {
            let mut text = String::new();
            std::io::Read::read_to_string(child.stdout.as_mut().unwrap(), &mut text).unwrap();
            return Some(text);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn answer(text: &str) -> String {
    let at = text.find("REVPROC ").unwrap_or_else(|| panic!("no answer: {text}"));
    text[at + "REVPROC ".len()..].lines().next().unwrap().to_string()
}

/// FINDING (MAJOR): commands.ts:93 `text.matchAll(FUNCTION_HEAD)` reads the whole text. commands.rs:270 reads it with
/// `FUNCTION_HEAD_RE.captures_iter(text).flatten()`, and fancy-regex's backtracking VM (the pattern holds a lookbehind)
/// stops a search at 1,000,000 backtracks and answers `Err`; `CaptureMatches::next` returns that `Err` without moving
/// on, and `.flatten()` asks again at the same place, for ever. About 1,500 blank lines in a row trip it when the line
/// after them is not itself a function head (the `\s*` of every line start runs over all of the blanks, then fails).
/// node answered ["bar"] in 99 ms for 5,000 blank lines, `echo hi`, then `foo() { :; }`, `foo`, `bar`.
#[test]
fn called_commands_over_a_run_of_blank_lines_answers_and_does_not_hang() {
    let got = rerun("child_called_commands_blank_lines", 20).expect("called_commands never returned (killed after 20 s)");
    assert_eq!(answer(&got), r#"["bar"]"#);
}

#[test]
#[ignore = "run by called_commands_over_a_run_of_blank_lines_answers_and_does_not_hang"]
fn child_called_commands_blank_lines() {
    let text = format!("{}echo hi\nfoo() {{ :; }}\nfoo\nbar\n", "\n".repeat(5000));
    println!("REVPROC {:?}", called_commands(&text, Syntax::Sh));
}

/// FINDING (MAJOR): the same loop on an rc file of ordinary lines: 30,000 lines of `export A=1` (330 KB) before the
/// one function. node answered ["bar"] in 74 ms.
#[test]
fn called_commands_over_a_330_kb_rc_file_answers_and_does_not_hang() {
    let got = rerun("child_called_commands_exports", 20).expect("called_commands never returned (killed after 20 s)");
    assert_eq!(answer(&got), r#"["bar"]"#);
}

#[test]
#[ignore = "run by called_commands_over_a_330_kb_rc_file_answers_and_does_not_hang"]
fn child_called_commands_exports() {
    let text = format!("{}foo() {{ :; }}\nfoo\nbar\n", "export A=1\n".repeat(30000));
    println!("REVPROC {:?}", called_commands(&text, Syntax::Sh));
}

/// Control: an ordinary small file answers the same names node does.
#[test]
fn called_commands_over_a_small_file_matches_node() {
    assert_eq!(called_commands("foo() { :; }\nfoo\nbar\r\nbaz() {\r\n}\n", Syntax::Sh), vec!["bar".to_string()]);
}

fn http_header_home(header_json: &str) -> tempfile::TempDir {
    home_with(&[(
        ".claude.json",
        &format!(r#"{{"mcpServers":{{"srv":{{"type":"http","url":"https://example.com/mcp","headers":{{"{header_json}":"v"}}}}}}}}"#),
    )])
}

async fn sign_in_of(header_json: &str) -> String {
    let home = http_header_home(header_json);
    let agents: Vec<_> = mcp_agents().into_iter().filter(|a| a.id == "claude").collect();
    let rows = detect_mcp(&host_at(home.path()), Some(&agents)).await;
    assert_eq!(rows.len(), 1, "{:?}", rows.iter().map(|r| &r.id).collect::<Vec<_>>());
    format!("{:?}", rows[0].sign_in)
}

/// Control: a header named X-KEY is a key (node: "key").
#[tokio::test(flavor = "current_thread")]
async fn a_header_named_x_key_is_a_key() {
    assert_eq!(sign_in_of("X-KEY").await, "Some(Key)");
}

/// FINDING (NIT): mcp.ts:276 `/key/i.test(n)` is JavaScript's non-unicode `i`, which never lets a non-ASCII letter match
/// an ASCII one, so a header named `X-` + KELVIN SIGN + `EY` is no key and the server is a "token" (node printed
/// "token"). mcp.rs:63 `n.to_lowercase().contains("key")` lowers U+212A to `k` and answers a key.
#[tokio::test(flavor = "current_thread")]
async fn a_header_with_the_kelvin_sign_for_k_is_not_a_key() {
    assert_eq!(sign_in_of(r"X-\u212aEY").await, "Some(Token)");
}

/// The 34 of the 35 JavaScript sources of the crate that `js_re` compiles translate to the same pattern in
/// wsp-catalog's `JsRegex` (checked by printing both), so switching answers the same for them; this one is the 35th.
/// FINDING (NOTE, not reachable today): FUNCTION_HEAD carries the flags `gm`. `JsRegex` turns `m` into `(?m)`, which
/// takes a line start after LF only, where JavaScript's `m` also takes one after CR, U+2028 and U+2029. Switching
/// `js_re(FUNCTION_HEAD.replace('^', LINE_START))` to `JsRegex::with_flags(FUNCTION_HEAD, "gm")` would stop finding a
/// function that follows a lone CR. node printed true for each of the three.
#[test]
fn jsregex_m_flag_does_not_take_a_line_start_after_cr_or_a_line_separator() {
    let re = JsRegex::with_flags(r"^\s*(?:function\s+)?([A-Za-z_][\w.+-]*)\s*\(\s*\)|^\s*function\s+([A-Za-z_][\w.+-]*)\b", "gm");
    let lost: Vec<&str> = ["a\rfoo() {", "a\u{2028}foo() {", "a\u{2029}foo() {"].into_iter().filter(|t| !re.is_match(t)).collect();
    assert!(lost.is_empty(), "no line start after: {lost:?}");
}

/// FINDING (NOTE, not reachable today): `JsRegex` with `i` is Unicode simple case folding, JavaScript's non-unicode `i` is
/// not: node printed false for /s/i.test("\u017f") and for /k/i.test("\u212a"). The crate's own `i` regexes
/// (credential_signals.rs) use `(?i-u:...)` and agree with node; only the catalog type would differ.
#[test]
fn jsregex_i_flag_folds_non_ascii_letters_onto_ascii_ones() {
    let long_s = JsRegex::with_flags("s", "i");
    let kelvin = JsRegex::with_flags("k", "i");
    assert!(!long_s.is_match("\u{17f}"), "/s/i matched U+017F");
    assert!(!kelvin.is_match("\u{212a}"), "/k/i matched U+212A");
}
