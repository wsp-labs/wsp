// SPDX-License-Identifier: AGPL-3.0-only
//! The files units' F-rules (spec/files_land.md F1 and F7 to F10, spec/files_ledger.md F2 to F6 and F11): which road
//! and bound each call rides, the list after a landing that failed part way, the readers that take every line,
//! unland's parse, the exact append line, failure notes and the close's tombstones. Values from running
//! provision-files.ts at a4033dc34.
mod common;

use std::sync::{Arc, Mutex};

use indexmap::IndexMap;

use common::{box_guest, digest, other, res, write, ExecFn, FakeMachine, PutFn, RunFn};
use wsp_engine::golden_seam::{PackFiles, PackedFiles, READ_MS};
use wsp_engine::machine::{ExecOptions, Machine, RunOptions};
use wsp_engine::provision_files::*;
use wsp_engine::vault::{tar_of, TarEntry};
use wsp_wire::views::golden_image::MCP_ID_PREFIX;
use wsp_words::project_path::place_provision_paths;

type Log = Arc<Mutex<Vec<String>>>;

/// A machine that answers every call from `answer` (exit 0 and nothing where it says `None`) and logs each as
/// `exec[<timeout>] stdin=<len> <first line>` or `run[<deadline>] unlogged=<bool> <first line>`.
fn recorder(answer: impl Fn(&str) -> Option<wsp_engine::errors::Result<wsp_engine::machine::ExecResult>> + 'static) -> (FakeMachine, Log) {
    let log: Log = Arc::default();
    let answer = Arc::new(answer);
    let (seen, said) = (log.clone(), answer.clone());
    let exec: ExecFn = Arc::new(move |cmd: String, o: ExecOptions| {
        let stdin = o.stdin.as_ref().map(|s| String::from_utf8_lossy(s).into_owned()).unwrap_or_default();
        seen.lock().unwrap().push(format!("exec[{}] stdin={:?} {}", o.timeout_ms.unwrap_or(0), stdin, cmd.lines().next().unwrap_or("")));
        let a = said(&cmd);
        Box::pin(async move { a.unwrap_or_else(|| Ok(res(0, "", ""))) })
    });
    let ran = log.clone();
    let run: RunFn = Arc::new(move |script: String, o: RunOptions| {
        ran.lock().unwrap().push(format!("run[{}] unlogged={} {}", o.deadline_ms, o.unlogged, script.lines().next().unwrap_or("")));
        let a = answer(&script);
        Box::pin(async move { a.unwrap_or_else(|| Ok(res(0, "", ""))) })
    });
    let put: PutFn = Arc::new(|_, _| Box::pin(async { Ok(None) }));
    let mut m = FakeMachine::new(exec);
    m.run = Some(run);
    m.put_bytes = Some(put);
    (m, log)
}

fn logged(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

fn s(t: &str) -> String {
    t.to_string()
}

fn land(dest: &str) -> ProvisionLanding {
    ProvisionLanding { id: s("agents/claude"), label: s("Claude Code"), dest: s(dest), once: false }
}

fn tar1(rel: &str, text: &str) -> Vec<u8> {
    tar_of(&[TarEntry::File { path: s(rel), mode: 0o644, content: text.as_bytes().to_vec() }]).unwrap()
}

fn quiet() -> FilesSay {
    Arc::new(|_: &str| {})
}

/// F1: the road and bound of every call: the standing read and the append are short execs (the read fed its paths on
/// stdin), unland a long exec, the close and the servers read long runs, a config read a run marked unlogged (its
/// output is a whole config, env and headers), and the landing a logged run under `LAND_MS`.
#[tokio::test]
async fn each_call_rides_the_road_and_bound_the_ts_gives_it() {
    let (m, log) = recorder(|_| None);
    let _ = standing_digests(&m, "/h", &[s("a/b"), s("c")]).await;
    append_landing(&m, "/h", &[s("x")]).await;
    let _ = unland_files(&m, "/h", &[s(".codex")], &IndexMap::new()).await;
    close_agent_files(&m, "/h", &[], &IndexMap::new()).await;
    let _ = landed_servers(&m, "/h", quiet()).await;
    let port = machine_server_port(Arc::new(m.clone()));
    let _ = wsp_engine::provision_files::ServerPort::read(&port, &[s("/h/.claude.json")], "/h").await;
    let got = logged(&log);
    assert!(got[0].starts_with("exec[120000] stdin=\"a/b\\0c\\0\" "), "{}", got[0]);
    assert!(got[1].starts_with("exec[20000] stdin=\"\" mkdir -p "), "{}", got[1]);
    assert!(got[2].starts_with("exec[300000] stdin=\"\" "), "{}", got[2]);
    assert!(got[3].starts_with("run[300000] unlogged=false "), "{}", got[3]);
    assert!(got[4].starts_with("run[300000] unlogged=false "), "{}", got[4]);
    assert!(got[5].starts_with(&format!("run[{READ_MS}] unlogged=true ")), "{}", got[5]);
    assert_eq!(got.len(), 6);

    let (m, log) = recorder(|_| None);
    let o = LandAgentFiles {
        home: s("/h"),
        tar: tar1(".claude-cfg/a.md", "a\n"),
        lands: vec![land(".claude-cfg")],
        stood: vec![],
        say: quiet(),
        roots: vec![],
    };
    land_agent_files(&common::handle(&m), o).await.unwrap();
    let got = logged(&log);
    assert!(got[0].starts_with("exec[20000] stdin=\"\" rm -rf "), "{}", got[0]);
    assert!(got.iter().any(|c| c.starts_with("run[300000] unlogged=false set -eo pipefail")), "{got:?}");
    assert!(got.last().unwrap().starts_with("run[300000] unlogged=false set -u"), "{got:?}");
}

/// F7: the staging sweep's exit is not read (exit 1 goes on); only an `Err` fails the call, and then nothing else runs.
/// A landing that exits non-zero is `the agents' files did not land on <id> (exit <n>): <last 300 UTF-16 units>`, a
/// half surrogate pair at the cut written U+FFFD.
#[tokio::test]
async fn the_sweep_fails_the_landing_only_by_err_and_the_landing_error_keeps_the_last_300_units() {
    let ask = || LandAgentFiles {
        home: s("/h"),
        tar: tar1(".claude-cfg/a.md", "a\n"),
        lands: vec![land(".claude-cfg")],
        stood: vec![],
        say: quiet(),
        roots: vec![],
    };
    let (m, _) = recorder(|cmd| cmd.starts_with("rm -rf").then(|| Ok(res(1, "", "busy"))));
    assert!(land_agent_files(&common::handle(&m), ask()).await.is_ok());
    let (m, log) = recorder(|cmd| cmd.starts_with("rm -rf").then(|| Err(other("link down"))));
    assert_eq!(land_agent_files(&common::handle(&m), ask()).await.unwrap_err().to_string(), "link down");
    assert_eq!(logged(&log).len(), 1);
    let odd = format!("{}x", "\u{1F600}".repeat(200));
    let (m, _) = recorder(move |cmd| cmd.contains(": > \"$landing\"").then(|| Ok(res(3, "", &odd))));
    let err = land_agent_files(&common::handle(&m), ask()).await.unwrap_err().to_string();
    assert_eq!(err, format!("the agents' files did not land on spoo (exit 3): \u{FFFD}{}x", "\u{1F600}".repeat(149)));
    let (m, _) = recorder(|cmd| cmd.contains(": > \"$landing\"").then(|| Ok(res(124, "", ""))));
    assert_eq!(
        land_agent_files(&common::handle(&m), ask()).await.unwrap_err().to_string(),
        "the agents' files did not land on spoo (exit 124): "
    );
}

/// F8: every parser splits on `\n` alone, so a path's trailing `\r` stays part of the path.
#[test]
fn a_carriage_return_stays_in_the_path() {
    let landed = parse_landed("wsp-land\tinstalled\td\ta\r\nwsp-land\tkept\td\tb\r");
    assert_eq!(
        landed,
        vec![Landed { rel: s("a\r"), outcome: LandOutcome::Installed }, Landed { rel: s("b\r"), outcome: LandOutcome::Kept }]
    );
    let servers = parse_landed_servers("wsp-server\td1\tid1\r\nwsp-server\td2\tid2");
    assert_eq!(servers.into_iter().collect::<Vec<_>>(), vec![(s("id1\r"), s("d1")), (s("id2"), s("d2"))]);
    assert_eq!(own_marks("wsp-own\ta\r\nwsp-own\tb"), vec![s("a\r"), s("b")]);
    assert_eq!(outside_marks("wsp-outside\t/a\r\nwsp-outside\t/b"), vec![s("/a\r"), s("/b")]);
}

/// F9: an empty label is a label (`" <landed at>"`), and two lands on one dest are two rows that each count the same
/// landed paths.
#[test]
fn an_empty_label_is_kept_and_two_lands_on_one_dest_are_two_rows() {
    assert_eq!(file_label(Some(""), "/root", "x", &vec![]), " /root/x");
    let lands = vec![
        ProvisionLanding { id: s("a"), label: s("A"), dest: s(".claude-cfg/skills"), once: false },
        ProvisionLanding { id: s("b"), label: s("B"), dest: s(".claude-cfg/skills"), once: false },
    ];
    let landed = vec![
        Landed { rel: s(".claude-cfg/skills/x"), outcome: LandOutcome::Installed },
        Landed { rel: s(".claude-cfg/skills/y"), outcome: LandOutcome::Present },
    ];
    let rows: Vec<(String, String, Option<String>)> =
        files_rows(&lands, &landed, "/root", &vec![]).into_iter().map(|r| (r.id, r.label, r.note)).collect();
    assert_eq!(
        rows,
        vec![
            (s("files/.claude-cfg/skills"), s("A /root/.claude-cfg/skills"), Some(s("1 of 2 files"))),
            (s("files/.claude-cfg/skills"), s("B /root/.claude-cfg/skills"), Some(s("1 of 2 files"))),
        ]
    );
}

/// F2: a landing run that failed after it made `landing` (here cut at the deadline once its walk was done) gives
/// failed rows and owns nothing, and leaves `landing` and staging behind, so the close that follows writes the walk's
/// line into the list anyway.
#[tokio::test]
async fn a_landing_cut_after_its_walk_fails_its_rows_yet_the_close_lists_what_it_walked() {
    let mut g = box_guest(&[], None);
    let root = g.root_str();
    let inner = g.machine.run.clone().unwrap();
    let run: RunFn = Arc::new(move |script: String, o| {
        let inner = inner.clone();
        Box::pin(async move {
            let landing = script.contains(": > \"$landing\"");
            let r = inner(script, o).await;
            if landing {
                return Ok(res(124, "", ""));
            }
            r
        })
    });
    g.machine.run = Some(run);
    let tar = tar1(".claude-cfg/skills/a/SKILL.md", "why\n");
    let pack: PackFiles = Arc::new(move |_| {
        let tar = tar.clone();
        Box::pin(async move { Ok(PackedFiles { bytes: tar.len() as u64, unpacked: tar.len() as u64, tar, ..PackedFiles::default() }) })
    });
    let got = provision_files(
        &common::handle(&g.machine),
        ProvisionFilesAsk { home: root.clone(), lands: vec![land(".claude-cfg/skills")], pack, say: None, stores: IndexMap::new() },
    )
    .await;
    assert_eq!(got.rows.len(), 1);
    assert_eq!(got.rows[0].note.as_deref(), Some("the agents' files did not land on spoo (exit 124): "));
    assert!(got.owned.is_empty());
    let at = place_provision_paths(&root);
    assert!(std::path::Path::new(&at.landing).exists());
    close_agent_files(&g.machine, &root, &[], &IndexMap::new()).await;
    let d = digest("why\n");
    assert_eq!(std::fs::read_to_string(&at.landed).unwrap(), format!(".claude-cfg/skills/a/SKILL.md\t{d}\t{d}\n"));
}

/// F3: the servers read and the ownership read take every line: a later digest for a key wins, and a path listed twice
/// with its digest standing is marked twice (a first-line reader would mark nothing for `a` below).
#[tokio::test]
async fn the_servers_and_ownership_reads_take_every_line_not_the_first() {
    let g = box_guest(&[], None);
    let root = g.root_str();
    let at = place_provision_paths(&root);
    let key = format!("{MCP_ID_PREFIX}claude/gsc");
    write(&g.root, ".wsp/provision/landed", &format!("{key}\td1\td1\n{key}\td2\td2\n"));
    assert!(at.landed.ends_with(".wsp/provision/landed"));
    let servers = landed_servers(&g.machine, &root, quiet()).await;
    assert_eq!(servers.into_iter().collect::<Vec<_>>(), vec![(key, s("d2"))]);
    write(&g.root, "a", "A\n");
    let d = digest("A\n");
    write(&g.root, ".wsp/provision/landed", &format!("a\tx\tOLD\na\tx\t{d}\n"));
    let out = g.machine.run(&landed_files_script(&root), RunOptions::default()).await.unwrap();
    assert_eq!(own_marks(&out.stdout), vec![s("a")]);
    write(&g.root, ".wsp/provision/landed", &format!("a\tx\t{d}\na\tx\t{d}\n"));
    let out = g.machine.run(&landed_files_script(&root), RunOptions::default()).await.unwrap();
    assert_eq!(own_marks(&out.stdout), vec![s("a"), s("a")]);
}

/// F4: unland's answer: a line with no path adds `""`, the rest keeps its tabs and `\r`, and both lists sort by UTF-16
/// unit (U+10000 before U+FF5E, the reverse of byte order).
#[tokio::test]
async fn unland_reads_empty_paths_and_sorts_by_utf16_unit() {
    let said = [
        "wsp-unland\tgone",
        "wsp-unland\tkept\t",
        "wsp-unland\tkept\t\u{FF5E}",
        "wsp-unland\tkept\t\u{10000}",
        "wsp-unland\tgone\ta\r",
        "wsp-unland\tgone\tb\tc",
    ]
    .join("\n");
    let (m, _) = recorder(move |_| Some(Ok(res(0, &said, ""))));
    let got = unland_files(&m, "/h", &[s(".codex")], &IndexMap::new()).await.unwrap();
    assert_eq!(got, Unlanded { gone: vec![s(""), s("a\r"), s("b\tc")], kept: vec![s(""), s("\u{10000}"), s("\u{FF5E}")] });
}

/// F5: a list line inside the dest whose file is not there prints nothing and still leaves the list.
#[tokio::test]
async fn unland_says_nothing_of_a_path_that_is_not_there_and_drops_its_line() {
    let g = box_guest(&[], None);
    let root = g.root_str();
    write(&g.root, ".wsp/provision/landed", ".claude/skills/a/gone.md\tx\ty\n.claude/other\tx\ty\n");
    let got = unland_files(&g.machine, &root, &[s(".claude/skills/a")], &IndexMap::new()).await.unwrap();
    assert_eq!(got, Unlanded { gone: vec![], kept: vec![] });
    assert_eq!(common::read(&g.root, ".wsp/provision/landed"), ".claude/other\tx\ty\n");
}

/// F6: the append is one exec of exactly two lines: the folder made, then one printf of every line quoted.
#[tokio::test]
async fn the_append_is_one_exact_exec() {
    let seen: Log = Arc::default();
    let log = seen.clone();
    let exec: ExecFn = Arc::new(move |cmd: String, _| {
        log.lock().unwrap().push(cmd);
        Box::pin(async { Ok(res(0, "", "")) })
    });
    append_landing(&FakeMachine::new(exec), "/H", &[format!("{MCP_ID_PREFIX}claude/it's\tx\ty"), s("b")]).await;
    assert_eq!(
        logged(&seen),
        vec![format!("mkdir -p '/H/.wsp/provision' || exit 1\nprintf '%s\\n' '{MCP_ID_PREFIX}claude/it'\\''s\tx\ty' 'b' >> '/H/.wsp/provision/landing'")]
    );
}

/// F10: a failed round's note is the error's first line split on `\n` alone (`""` stays, `\r` stays); the packed line
/// prints `files` and `stood` wherever the pack gave them, zero and empty included.
#[tokio::test]
async fn failure_notes_keep_an_empty_first_line_and_the_packed_line_prints_zero() {
    for (said, want) in [("", ""), ("\nsecond", ""), ("a\r\nb", "a\r")] {
        let (m, _) = recorder(|_| None);
        let pack: PackFiles = Arc::new(move |_| Box::pin(async move { Err(other(said)) }));
        let got = provision_files(
            &common::handle(&m),
            ProvisionFilesAsk { home: s("/h"), lands: vec![land(".claude-cfg")], pack, say: None, stores: IndexMap::new() },
        )
        .await;
        assert_eq!(got.rows[0].note.as_deref(), Some(want), "{said:?}");
    }
    let (m, _) = recorder(|_| None);
    let lines: Log = Arc::default();
    let heard = lines.clone();
    let pack: PackFiles = Arc::new(|_| {
        Box::pin(async {
            Ok(PackedFiles {
                tar: vec![],
                bytes: 123,
                unpacked: 0,
                skipped: vec![],
                files: Some(0),
                stood: Some(vec![]),
                ..Default::default()
            })
        })
    });
    let _ = provision_files(
        &common::handle(&m),
        ProvisionFilesAsk {
            home: s("/h"),
            lands: vec![land("a"), land("b")],
            pack,
            say: Some(Arc::new(move |l: &str| heard.lock().unwrap().push(s(l)))),
            stores: IndexMap::new(),
        },
    )
    .await;
    assert_eq!(logged(&lines)[0], "packed on this computer: 2 paths, 0 files stand there already, 0 files packed, 123 B");
}

/// F11: the close keeps each path's first line from the landing then the old list, and a tombstone shadows its old line
/// and then leaves: one with no older line never appears.
#[tokio::test]
async fn the_close_lets_a_tombstone_shadow_then_leave() {
    let g = box_guest(&[], None);
    let root = g.root_str();
    write(&g.root, ".claude-cfg/x", "x\n");
    let dx = digest("x\n");
    write(&g.root, ".wsp/provision/landed", &format!("{MCP_ID_PREFIX}old\td1\td1\n{MCP_ID_PREFIX}keep\td2\td2\n.claude-cfg/x\tf\t{dx}\n"));
    write(&g.root, ".wsp/provision/landing", &format!("{MCP_ID_PREFIX}old\t-\t-\n{MCP_ID_PREFIX}keep\tn2\tn2\n{MCP_ID_PREFIX}new\t-\t-\n"));
    close_agent_files(&g.machine, &root, &[], &IndexMap::new()).await;
    assert_eq!(common::read(&g.root, ".wsp/provision/landed"), format!("{MCP_ID_PREFIX}keep\tn2\tn2\n.claude-cfg/x\tf\t{dx}\n"));
}
