// SPDX-License-Identifier: AGPL-3.0-only
//! machine_reads' K-rules (spec/machine_reads.md K1 to K6): the first line a failure names, node's `dirname` in the
//! landing line, the stores a login reaches, the login shell's last line, disk-sync's JS line breaks, and the preview's
//! ten-minute boundary. Values from running target-line.ts, disk-sync.ts and preview.ts at a4033dc34.
#[path = "rest_common/mod.rs"]
mod common;

use std::sync::Arc;

use indexmap::IndexMap;

use common::{exit, ok, FakeMachine};
use wsp_engine::daemon_targets::Platform;
use wsp_engine::disk_sync::{disk_use_of, sync_disk, DiskReading};
use wsp_engine::machine::PreviewReach;
use wsp_engine::preview::{preview_is_fresh, refresh_preview_token};
use wsp_engine::target_line::{land_as_login, login_shell_path, stores_reached, target_login, GivenLogin, TargetLogin};
use wsp_wire::types::JsNumber;

fn login(home: &str, run_as: Option<&str>) -> TargetLogin {
    TargetLogin { platform: Platform::Linux, home: home.into(), path: None, user: "ada".into(), run_as: run_as.map(String::from) }
}

/// K1: the failure line is `(stderr || stdout)` chosen *before* the trim, so a blank stderr wins and names nothing, and
/// its first line keeps a `\r` and inner trailing spaces.
#[tokio::test]
async fn a_failure_names_the_first_line_of_stderr_chosen_before_the_trim() {
    let prefix = "the computer did not say who its lines run as: ";
    for (stdout, stderr, tail) in [("boom\nmore", "  \n", ""), ("", "", ""), ("  out line\r\nsecond", "", "out line\r"), ("", "boom\r\nmore", "boom\r"), ("", "a  \nb", "a  ")] {
        let (o, e) = (stdout.to_string(), stderr.to_string());
        let m = FakeMachine::exec("m1", move |_, _| Ok(exit(1, &o, &e)));
        let err = target_login(&m, &GivenLogin::default()).await.unwrap_err().to_string();
        assert_eq!(err, format!("{prefix}{tail}"), "{stdout:?} {stderr:?}");
    }
}

/// K2: the plain landing makes node's `posix.dirname(dest)`: trailing slashes cut first, no other normalising, `.` where
/// no slash is left, `/` or `//` for a root cut.
#[tokio::test]
async fn the_landing_line_makes_nodes_dirname_of_the_dest() {
    for (dest, dir) in [
        ("plain.txt", "."),
        ("/b", "/"),
        ("/a/b", "/a"),
        ("/a/b/", "/a"),
        ("/a/b//", "/a"),
        ("a//b", "a/"),
        ("//b", "//"),
        ("/a/./b", "/a/."),
        ("a/b/c.txt", "a/b"),
        ("./x", "."),
        ("x/", "."),
        ("", "."),
        ("/", "/"),
        ("a b/c", "a b"),
    ] {
        let mut m = FakeMachine::new("m1");
        m.put_bytes_fn = Some(Arc::new(|_, _, _| Ok(None)));
        land_as_login(&m, &login("/home/ada", None), dest, b"x", false, None).await.unwrap();
        let write = m.cmds().into_iter().find(|c| c.contains("&& cat '")).unwrap_or_else(|| panic!("no write line for {dest:?}"));
        let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        assert!(write.contains(&format!("mkdir -p {} && cat '", q(dir))), "{dest:?}: {write}");
        assert!(write.ends_with(&format!("/bytes' > {}", q(dest))), "{dest:?}: {write}");
    }
}

/// K3: with a login handed to, only stores at or under the home by string prefix (`<home>/`), no normalising, map
/// order; with none handed to, every store; a home with a trailing slash reaches only stores under `<home>//`.
#[test]
fn the_stores_a_login_reaches_are_a_plain_prefix_test() {
    let stores: IndexMap<String, String> = [
        ("claude", "/home/ada/.claude"),
        ("codex", "/home/ada"),
        ("sib", "/home/adam/x"),
        ("slash", "/home/ada/"),
        ("empty", ""),
        ("opt", "/opt/x"),
        ("deep", "/home/ada/a/b"),
        ("dotdot", "/home/ada/../x"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let keys = |m: IndexMap<String, String>| m.into_keys().collect::<Vec<_>>();
    assert_eq!(keys(stores_reached(&login("/home/ada", Some("ada")), &stores)), ["claude", "codex", "slash", "deep", "dotdot"]);
    assert_eq!(keys(stores_reached(&login("/home/ada", None), &stores)).len(), 8);
    assert_eq!(keys(stores_reached(&login("/home/ada/", Some("ada")), &stores)), ["slash"]);
    let few: IndexMap<String, String> = [("a", "/"), ("b", ""), ("c", "x")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    assert_eq!(keys(stores_reached(&login("", Some("ada")), &few)), ["a", "b"]);
}

/// K4: the login shell's PATH is the last `\n` line trimmed: a trailing `\r` goes with the trim, but `\r\n` leaves an
/// empty last line, which is no PATH.
#[tokio::test]
async fn the_login_shells_path_is_the_last_line_so_crlf_reads_none() {
    for (out, want) in [("/usr/bin", Some("/usr/bin")), ("hi\n/usr/bin", Some("/usr/bin")), ("/usr/bin\r", Some("/usr/bin")), ("/usr/bin\r\n", None), ("  /a:/b  ", Some("/a:/b")), ("", None)] {
        let o = out.to_string();
        let m = FakeMachine::exec("m1", move |_, _| Ok(ok(&o)));
        assert_eq!(login_shell_path(&m, &login("/home/ada", None)).await.as_deref(), want, "{out:?}");
    }
}

/// K5: disk-sync reads its counters with JS's multiline `^`/`$`, which break at `\r`, U+2028 and U+2029 as well as `\n`;
/// `\d` is ASCII; the first match wins; a number past f64's range is not a reading. The df line is the first line
/// that is exactly two ASCII numbers.
#[tokio::test]
async fn disk_sync_breaks_lines_as_js_does() {
    for (out, want) in [
        ("Dirty: 1\r\nWriteback: 2\r\n", Some((1.0, 2.0))),
        ("Dirty: 1\rWriteback: 2\r", Some((1.0, 2.0))),
        ("Dirty: 1\u{2028}Writeback: 2\u{2028}", Some((1.0, 2.0))),
        ("Dirty: 1 \nWriteback: 2\n", None),
        ("Dirty: \u{661}\nWriteback: 2\n", None),
        ("Dirty: 1\nDirty: 9\nWriteback: 2\nWriteback: 8\n", Some((1.0, 2.0))),
        ("Dirty: 007\nWriteback: 000\n", Some((7.0, 0.0))),
        ("Dirty: 99999999999999999999999\nWriteback: 2\n", Some((1e23, 2.0))),
    ] {
        let o = out.to_string();
        let m = FakeMachine::exec("m1", move |_, _| Ok(ok(&o)));
        let got = sync_disk(&m).await.unwrap().map(|r: DiskReading| (r.dirty_kb, r.writeback_kb));
        assert_eq!(got, want, "{out:?}");
    }
    let nines = format!("Dirty: {}\nWriteback: 2\n", "9".repeat(400));
    let m = FakeMachine::exec("m1", move |_, _| Ok(ok(&nines)));
    assert_eq!(sync_disk(&m).await.unwrap(), None);
    let used = |s: &str| disk_use_of(s).map(|d| (d.used_bytes, d.free_bytes));
    assert_eq!(used("Filesystem\r\n12 34\r\n"), Some((12_288.0, 34_816.0)));
    assert_eq!(used("12 34 \n99 1"), Some((101_376.0, 1_024.0)));
    assert_eq!(used("12\t34"), None);
    assert_eq!(used("\u{661}\u{662} 34"), None);
}

/// K6: a reach is fresh only with more than ten minutes left (exactly 600000 is stale); a fresh reach is answered as it
/// is even by a machine with no preview road; a stale one there is refused with the exact sentence.
#[tokio::test]
async fn the_preview_boundary_is_strict_and_a_fresh_reach_needs_no_road() {
    let now = 1_000_000_i64;
    let reach = |at: i64| PreviewReach { url: "https://p".into(), token: "t".into(), expires_at: JsNumber(at as f64) };
    assert!(!preview_is_fresh(&reach(now + 600_000), now));
    assert!(preview_is_fresh(&reach(now + 600_001), now));
    let m = FakeMachine::new("m1");
    assert_eq!(refresh_preview_token(&m, 3000, Some(&reach(now + 600_001)), now).await.unwrap(), reach(now + 600_001));
    assert_eq!(refresh_preview_token(&m, 3000, None, now).await.unwrap_err().to_string(), "machine m1 is on a backend without preview URLs");
}
