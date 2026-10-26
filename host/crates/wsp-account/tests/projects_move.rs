// SPDX-License-Identifier: AGPL-3.0-only
//! Moving a folder of this computer's onto a workspace's machine and a machine's folder back here, each saying its
//! stages. Ported from packages/runtime/test/project-import.test.ts and project-export.test.ts at 65f6b89c7, at the
//! area's seam: the engine's roads are the fake's (`MoveRoads`), the bundler and the lander are the TS tests' own fakes.
//! Every report line is the TS's text (the TS tests' literals; sizes through fmtBytes, counts through plural).
#[path = "support/base.rs"]
mod base;
#[path = "support/projects.rs"]
mod projects;

use std::cell::RefCell;
use std::rc::Rc;

use base::{kinded, person, plain, rec, record, run};
use projects::{agent_result, fails, keys, landed_agent, scratch_root, FakeBundler, FakeLander, FakeProjects};
use serde_json::{json, Value};
use wsp_account::projects::projects::Projects;
use wsp_account::projects::workspaces::seam::Entry;
use wsp_core::types::internal::ImportReport;
use wsp_core::types::wiring::{PackedState, ProjectExportOptions, ProjectImportOptions};
use wsp_engine::project_state::state_listing;
use wsp_words::exit::Kind;

const SOURCE: &str = "/Users/dev/code/proj";
const GUEST_SOURCE: &str = "/root/work/proj";
const DEST: &str = "/Users/dev/code/proj";

type Said = Rc<RefCell<Vec<(String, String, Option<(u64, u64)>)>>>;

fn rig() -> (Rc<FakeProjects>, Rc<Projects>, Entry) {
    let fake = FakeProjects::new();
    let area = fake.area();
    let entry = fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1" })));
    (fake, area, entry)
}

fn reporter() -> (ImportReport, Said) {
    let said: Said = Rc::new(RefCell::new(vec![]));
    let sink = said.clone();
    let report: ImportReport = Rc::new(move |stage, message, progress| {
        sink.borrow_mut().push((serde_json::to_value(stage).unwrap().as_str().unwrap().to_owned(), message.to_owned(), progress));
    });
    (report, said)
}

fn said_of(said: &Said) -> Vec<(String, String, Option<(u64, u64)>)> {
    said.borrow().clone()
}

fn line(stage: &str, message: &str, progress: Option<(u64, u64)>) -> (String, String, Option<(u64, u64)>) {
    (stage.to_owned(), message.to_owned(), progress)
}

fn secret(path: &str, bytes: u64, signals: Value) -> Value {
    json!({ "path": path, "bytes": bytes, "signals": signals })
}

/// The three secret-shaped files of project-import.test.ts's fakeBundler, with `extra` between the first and the rest.
fn secrets(extra: Vec<Value>) -> Vec<Value> {
    let mut all = vec![secret(".env", 20, json!(["name", "keys"]))];
    all.extend(extra);
    all.push(secret("config/secrets.json", 25, json!(["name", "keys"])));
    all.push(secret("keys/id_ed25519", 80, json!(["name", "mode", "pem"])));
    all
}

fn plan_of(secrets: Vec<Value>, agents: Vec<Value>, values: &[&str], repo: bool) -> Value {
    json!({ "source": SOURCE, "repo": repo, "files": 6, "bytes": 4321, "secrets": secrets, "values": values, "excluded": ["dist", "node_modules"], "skipped": [], "agents": agents })
}

fn git_config() -> Value {
    json!({ "path": ".git/config", "bytes": 300, "signals": ["url"], "rewrite": { "urls": ["https://github.com/example/proj.git"], "drop": [] } })
}

fn auth_config(urls: Value) -> Value {
    json!({ "path": ".git/config", "bytes": 300, "signals": ["keys", "url"], "rewrite": { "urls": urls, "drop": ["http.extraheader"] } })
}

fn agents_plan() -> Vec<Value> {
    vec![
        json!({ "agent": "claude", "name": "Claude Code", "sessions": 2, "bytes": 4096, "carry": "moves" }),
        json!({ "agent": "pi", "name": "Pi", "sessions": 1, "bytes": 512, "carry": "moves" }),
        json!({ "agent": "codex", "name": "Codex", "sessions": 3, "bytes": 900, "carry": "transcript-only" }),
    ]
}

fn import_opts(bundler: &Rc<FakeBundler>, dest: &str, carry: &[&str], rewrite: &[&str], agents: &[&str]) -> ProjectImportOptions {
    let words = |w: &[&str]| if w.is_empty() { None } else { Some(w.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()) };
    ProjectImportOptions {
        workspace_id: "ws_1".to_owned(),
        source: SOURCE.to_owned(),
        dest: dest.to_owned(),
        replace: None,
        carry: words(carry),
        rewrite: words(rewrite),
        agents: words(agents),
        bundler: bundler.clone(),
    }
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// The scene's reports for a copy import, and the sentence it ends on.
async fn scene(plan: Value, provider: bool, carry: &[&str], rewrite: &[&str], agents: &[&str], dest: &str) -> (Vec<(String, String, Option<(u64, u64)>)>, String) {
    let (fake, area, entry) = rig();
    fake.base.answer_with("provider_row", vec![json!(provider)]);
    let bundler = FakeBundler::new(&fake.base, plan);
    let (report, said) = reporter();
    let landed = area.copy_import(&entry, import_opts(&bundler, dest, carry, rewrite, agents), report).await.unwrap();
    (said_of(&said), landed.done)
}

// ---------------------------------------------------------------------------------------------------------------
// copy_import

#[test]
fn copy_import_reports_each_stage_as_the_ts_words_it() {
    // spec copy rules 1-7 and 11: every scene's reports whole, in order, with the progress each carries.
    run(async {
        let planned = "6 files, 4 KB and the repository; 3 secret-shaped files; 2 caches left behind.";
        let upload = |dest: &str| vec![line("uploading", "Uploading 2 KB.", Some((0, 2048))), line("uploading", "Part 1 of 1, 2 KB of 2 KB.", Some((2048, 2048))), line("landing", &format!("Landing at {dest}."), None)];

        // project-import.test.ts:108: a provider, one secret carried, the file of values cut.
        let (got, done) = scene(plan_of(secrets(vec![]), vec![], &[".env"], true), true, &["config/secrets.json"], &[], &[], "/root/work/proj").await;
        let mut want = vec![
            line("planned", planned, None),
            line("consented", "Carrying config/secrets.json; cut .env, keys/id_ed25519.", None),
            line("packing", "Packing 4 files.", None),
        ];
        want.extend(upload("/root/work/proj"));
        assert_eq!(got, want);
        assert_eq!(done, "4 files, 4 KB, landed at /root/work/proj.");

        // :169: a file of values carried onto a provider's machine is cut, and the consented line says so.
        let (got, _) = scene(plan_of(secrets(vec![]), vec![], &[".env"], true), true, &[".env", "keys/id_ed25519"], &[], &[], "/root/proj").await;
        assert_eq!(got[1], line("consented", "Carrying keys/id_ed25519; cut .env, config/secrets.json.", None));
        assert_eq!(got[2], line("packing", "Packing 4 files.", None));

        // :190: a rewrite the person accepted.
        let (got, done) = scene(plan_of(secrets(vec![git_config()]), vec![], &[".env"], true), true, &["config/secrets.json"], &[".git/config"], &[], "/root/proj").await;
        assert_eq!(got[0], line("planned", "6 files, 4 KB and the repository; 4 secret-shaped files; 2 caches left behind.", None));
        assert_eq!(got[1], line("consented", "Carrying config/secrets.json; rewriting .git/config to https://github.com/example/proj.git; cut .env, keys/id_ed25519.", None));
        assert_eq!(got[2], line("packing", "Packing 4 files.", None));
        assert_eq!(done, "4 files, 4 KB, landed at /root/proj.");
        let (got, _) = scene(plan_of(secrets(vec![auth_config(json!(["https://github.com/example/proj.git"]))]), vec![], &[".env"], true), true, &[], &[".git/config"], &[], "/root/other").await;
        assert_eq!(got[1], line("consented", "Rewriting .git/config to https://github.com/example/proj.git without http.extraheader; cut .env, config/secrets.json, keys/id_ed25519.", None));
        assert_eq!(got[2], line("packing", "Packing 3 files.", None));
        let (got, _) = scene(plan_of(secrets(vec![auth_config(json!([]))]), vec![], &[".env"], true), true, &[], &[".git/config"], &[], "/root/third").await;
        assert_eq!(got[1], line("consented", "Rewriting .git/config without http.extraheader; cut .env, config/secrets.json, keys/id_ed25519.", None));

        // Nothing secret-shaped and nothing cut, no repository: the short sentences.
        let (got, done) = scene(plan_of(vec![], vec![], &[], false), false, &[], &[], &[], "/root/bare").await;
        assert_eq!(got[0], line("planned", "6 files, 4 KB; 0 secret-shaped files; 2 caches left behind.", None));
        assert_eq!(got[1], line("consented", "No secret-shaped files.", None));
        assert_eq!(got[2], line("packing", "Packing 6 files.", None));
        assert_eq!(done, "6 files, 4 KB, landed at /root/bare.");

        // :354: with no agent named nothing about them is read and the consented line says so.
        let (got, _) = scene(plan_of(secrets(vec![]), agents_plan(), &[".env"], true), true, &[], &[], &[], "/root/proj").await;
        assert_eq!(got[1], line("consented", "No secret-shaped file travels; cut .env, config/secrets.json, keys/id_ed25519. No agent sessions travel.", None));

        // The file in two parts: each part says its share, and the done line says how many.
        let (fake, area, entry) = rig();
        fake.base.answer_with("land_bundle_parts", vec![json!([[1, 2, 1024, 2048], [2, 2, 2048, 2048]])]);
        let bundler = FakeBundler::new(&fake.base, plan_of(secrets(vec![]), vec![], &[".env"], true));
        let (report, said) = reporter();
        let landed = area.copy_import(&entry, import_opts(&bundler, "/root/proj", &["config/secrets.json"], &[], &[]), report).await.unwrap();
        assert_eq!(
            said_of(&said)[3..],
            vec![
                line("uploading", "Uploading 2 KB.", Some((0, 2048))),
                line("uploading", "Part 1 of 2, 1 KB of 2 KB.", Some((1024, 2048))),
                line("uploading", "Part 2 of 2, 2 KB of 2 KB.", Some((2048, 2048))),
                line("landing", "Landing at /root/proj.", None),
            ]
        );
        assert_eq!(landed.done, "4 files, 4 KB, landed at /root/proj in 2 parts.");
        assert_eq!(landed.result.parts.0, 2.0);
    });
}

#[test]
fn copy_import_holds_values_back_from_a_provider() {
    // spec copy rule 3, project-import.test.ts:181: a file of values the scan never flagged is cut for a provider and
    // not counted as packed; for a computer of the person's own it travels.
    run(async {
        let (fake, area, entry) = rig();
        let bundler = FakeBundler::new(&fake.base, plan_of(secrets(vec![]), vec![], &[".env", ".envrc"], true));
        let (report, said) = reporter();
        let landed = area.copy_import(&entry, import_opts(&bundler, "/root/proj", &["keys/id_ed25519"], &[], &[]), report).await.unwrap();
        let got = said_of(&said);
        assert_eq!(got[1].1, "Carrying keys/id_ed25519; cut .env, .envrc, config/secrets.json.");
        assert_eq!(got[2].1, "Packing 3 files.");
        assert_eq!(fake.base.args_of("bundler_pack"), vec![json!({ "carry": ["keys/id_ed25519"], "rewrite": [], "provider": true })]);
        assert_eq!(landed.result.cut, strings(&[".env", ".envrc", "config/secrets.json"]));

        let (fake, area, entry) = rig();
        fake.base.answer_with("provider_row", vec![json!(false)]);
        let bundler = FakeBundler::new(&fake.base, plan_of(secrets(vec![]), vec![], &[".env", ".envrc"], true));
        let (report, said) = reporter();
        let landed = area.copy_import(&entry, import_opts(&bundler, "/root/proj", &["keys/id_ed25519"], &[], &[]), report).await.unwrap();
        let got = said_of(&said);
        assert_eq!(got[1].1, "Carrying keys/id_ed25519; cut .env, config/secrets.json.");
        assert_eq!(got[2].1, "Packing 4 files.");
        assert_eq!(fake.base.args_of("bundler_pack"), vec![json!({ "carry": ["keys/id_ed25519"], "rewrite": [], "provider": false })]);
        assert_eq!(landed.result.cut, strings(&[".env", "config/secrets.json"]));
    });
}

fn moved_state() -> PackedState {
    PackedState {
        tar: vec![9u8; 3072],
        agents: vec![
            agent_result(json!({ "agent": "claude", "files": 1, "bytes": 26, "outcome": "moved" })),
            agent_result(json!({ "agent": "pi", "files": 1, "bytes": 18, "outcome": "carried" })),
            agent_result(json!({ "agent": "codex", "files": 1, "bytes": 25, "outcome": "transcript-only" })),
        ],
        merges: vec![("codex".to_owned(), "/tmp/wsp-merge-0000/codex.py".to_owned())],
    }
}

#[test]
fn copy_import_lands_sessions_and_merges_rows() {
    // spec copy rules 8-11, project-import.test.ts:235 and :307: the agents named travel with the machine's presence
    // per agent, land as an overlay after the project, the merges run there, and the result says what became of each.
    run(async {
        let (fake, area, entry) = rig();
        fake.base.answer_with("agents_on_machine", vec![json!(["claude", "codex"])]);
        fake.base.answer_with("merge_on_machine", vec![json!({ "agent": "codex", "files": 1, "bytes": 25, "outcome": "moved", "rows": 3 })]);
        let bundler = FakeBundler::new(&fake.base, plan_of(secrets(vec![]), agents_plan(), &[".env"], true));
        *bundler.state.borrow_mut() = Some(moved_state());
        let (report, said) = reporter();
        let landed = area.copy_import(&entry, import_opts(&bundler, "/root/work/proj", &["config/secrets.json"], &[], &["claude", "pi", "codex"]), report).await.unwrap();
        let sentence = "Claude Code moved, Pi carried unchanged since it is not on the machine, Codex moved, 3 rows merged";
        assert_eq!(
            said_of(&said),
            vec![
                line("planned", "6 files, 4 KB and the repository; 3 secret-shaped files; 2 caches left behind.", None),
                line("consented", "Carrying config/secrets.json; cut .env, keys/id_ed25519. Sessions travel for Claude Code (2 sessions), Pi (1 session), Codex (3 sessions).", None),
                line("packing", "Packing 4 files.", None),
                line("uploading", "Uploading 2 KB.", Some((0, 2048))),
                line("uploading", "Part 1 of 1, 2 KB of 2 KB.", Some((2048, 2048))),
                line("landing", "Landing at /root/work/proj.", None),
                line("uploading", "Uploading 3 session files and the rows to merge, 3 KB.", Some((0, 3072))),
                line("uploading", "Part 1 of 1, 3 KB of 3 KB.", Some((3072, 3072))),
                line("landing", "Merging rows into Codex.", None),
                line("landing", &format!("Landing sessions: {sentence}."), None),
            ]
        );
        assert_eq!(landed.done, format!("4 files, 4 KB, landed at /root/work/proj; sessions: {sentence}."));
        assert_eq!(
            serde_json::to_value(&landed.result).unwrap(),
            json!({
                "dest": "/root/work/proj", "files": 4, "bytes": 4000, "parts": 1, "cut": [".env", "keys/id_ed25519"], "rewritten": [],
                "agents": [
                    { "agent": "claude", "files": 1, "bytes": 26, "outcome": "moved" },
                    { "agent": "pi", "files": 1, "bytes": 18, "outcome": "carried" },
                    { "agent": "codex", "files": 1, "bytes": 25, "outcome": "moved", "rows": 3 },
                ],
                "project": { "name": "proj", "dest": "/root/work/proj", "importedAt": "2026-10-25T00:00:00.000Z", "size": 4000 },
            })
        );
        let ran = ["bundler_plan", "provider_row", "bundler_pack", "agents_on_machine", "guest_agent_homes", "bundler_pack_state", "land_bundle", "import_into", "merge_on_machine"];
        assert_eq!(fake.base.names_of(&ran), ran.to_vec());
        assert_eq!(fake.base.args_of("agents_on_machine"), vec![json!({ "id": "ws_1", "agents": ["claude", "pi", "codex"] })]);
        assert_eq!(
            fake.base.args_of("bundler_pack_state"),
            vec![json!({ "dest": "/root/work/proj", "agents": [
                { "agent": "claude", "home": "/root/.claude-cfg", "present": true },
                { "agent": "pi", "home": "/root/.pi/agent", "present": false },
                { "agent": "codex", "home": "/root/.codex", "present": true },
            ] })]
        );
        assert_eq!(fake.base.args_of("land_bundle"), vec![json!({ "id": "ws_1", "bytes": 2048, "dest": "/root/work/proj", "replace": null })]);
        assert_eq!(fake.base.args_of("import_into"), vec![json!({ "id": "ws_1", "bytes": 3072 })]);
        let merged = &fake.base.args_of("merge_on_machine")[0];
        assert_eq!(merged["script"], "/tmp/wsp-merge-0000/codex.py");
        assert_eq!(merged["agent"], json!({ "agent": "codex", "files": 1, "bytes": 25, "outcome": "transcript-only" }));

        // Rows alone still travel (:307): no session file, a merge to run, and the sentence for rows already there.
        let (fake, area, entry) = rig();
        fake.base.answer_with("merge_on_machine", vec![json!({ "agent": "hermes", "files": 0, "bytes": 0, "outcome": "moved", "rows": 0 })]);
        let hermes = json!({ "agent": "hermes", "name": "Hermes Agent", "sessions": 1, "bytes": 0, "carry": "transcript-only" });
        let bundler = FakeBundler::new(&fake.base, plan_of(secrets(vec![]), vec![hermes], &[".env"], true));
        *bundler.state.borrow_mut() = Some(PackedState {
            tar: vec![9u8; 1024],
            agents: vec![agent_result(json!({ "agent": "hermes", "files": 0, "bytes": 0, "outcome": "transcript-only" }))],
            merges: vec![("hermes".to_owned(), "/tmp/wsp-merge-0000/hermes.py".to_owned())],
        });
        let (report, said) = reporter();
        let landed = area.copy_import(&entry, import_opts(&bundler, "/root/work/other", &[], &[], &["hermes"]), report).await.unwrap();
        let got = said_of(&said);
        assert_eq!(got[6], line("uploading", "Uploading the rows to merge, 1 KB.", Some((0, 1024))));
        assert_eq!(got[8], line("landing", "Merging rows into Hermes Agent.", None));
        assert_eq!(got[9], line("landing", "Landing sessions: Hermes Agent moved, its rows already there.", None));
        assert_eq!(landed.done, "3 files, 4 KB, landed at /root/work/other; sessions: Hermes Agent moved, its rows already there.");

        // An agent the plan could not read never travels, even when named (:340), and nothing of its state is packed.
        let (fake, area, entry) = rig();
        let unreadable = json!({ "agent": "opencode", "name": "OpenCode", "sessions": 0, "bytes": 0, "carry": "transcript-only", "error": "file is not a database" });
        let mut with = agents_plan();
        with.push(unreadable);
        let bundler = FakeBundler::new(&fake.base, plan_of(secrets(vec![]), with, &[".env"], true));
        *bundler.state.borrow_mut() = Some(PackedState {
            tar: vec![9u8; 1024],
            agents: vec![agent_result(json!({ "agent": "claude", "files": 1, "bytes": 26, "outcome": "moved" }))],
            merges: vec![],
        });
        let (report, said) = reporter();
        let landed = area.copy_import(&entry, import_opts(&bundler, "/root/proj", &[], &[], &["claude", "opencode"]), report).await.unwrap();
        assert_eq!(
            said_of(&said)[1],
            line("consented", "No secret-shaped file travels; cut .env, config/secrets.json, keys/id_ed25519. Sessions travel for Claude Code (2 sessions); Pi, Codex stay; OpenCode could not be read (file is not a database).", None)
        );
        assert_eq!(fake.base.args_of("bundler_pack_state")[0]["agents"], json!([{ "agent": "claude", "home": "/root/.claude-cfg", "present": true }]));
        assert_eq!(landed.result.agents.iter().map(|a| a.agent.clone()).collect::<Vec<_>>(), vec!["claude"]);
        assert_eq!(landed.done, "3 files, 4 KB, landed at /root/proj; sessions: Claude Code moved.");
    });
}

#[test]
fn copy_import_refuses_an_agent_the_catalog_lacks() {
    // spec copy rule 8: an agent with no home in the guest's table is a plain error naming it, before anything uploads.
    run(async {
        let (fake, area, entry) = rig();
        fake.base.answer_with("guest_agent_homes", vec![json!({ "claude": "/root/.claude-cfg" })]);
        let bundler = FakeBundler::new(&fake.base, plan_of(secrets(vec![]), agents_plan(), &[".env"], true));
        *bundler.state.borrow_mut() = Some(moved_state());
        let (report, said) = reporter();
        let refusal = area.copy_import(&entry, import_opts(&bundler, "/root/proj", &[], &[], &["claude", "codex"]), report).await.unwrap_err();
        assert_eq!(refusal, plain("codex is not an agent the catalog knows"));
        assert_eq!(said_of(&said).iter().map(|s| s.0.clone()).collect::<Vec<_>>(), vec!["planned", "consented", "packing"]);
        assert_eq!(fake.base.names_of(&["bundler_pack_state", "land_bundle", "import_into", "merge_on_machine"]), Vec::<String>::new());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// import

fn import_event(stage: &str, message: &str, elapsed: i64) -> Value {
    json!({ "type": "project.import", "workspaceId": "ws_1", "source": SOURCE, "dest": "/root/work/proj", "stage": stage, "message": message, "elapsedMs": elapsed })
}

fn plain_import(fake: &FakeProjects) -> ProjectImportOptions {
    import_opts(&FakeBundler::new(&fake.base, plan_of(secrets(vec![]), vec![], &[], true)), "/root/work/proj", &[], &[], &[])
}

#[test]
fn import_adds_the_dest_to_landed_before_the_roots_write() {
    // spec import: the folder is named on the record before the roots write (so a write queued behind lists it),
    // unique and in order, then persisted; the stream says what the kind said, then done.
    run(async {
        let (fake, area, entry) = rig();
        entry.borrow().record.borrow_mut().landed = Some(strings(&["/root/a"]));
        fake.base.answer_with("kind_import_advance", vec![json!(40)]);
        fake.base.answer_with("kind_import_reports", vec![json!([["planned", "6 files, 4 KB; 0 secret-shaped files; 0 caches left behind.", null], ["uploading", "Part 1 of 1, 1 KB of 1 KB.", [1024, 1024]]])]);
        let result = area.import(plain_import(&fake), Some(person())).await.unwrap();
        let ran = ["spawn_guard", "entry_of", "copy_blocked", "kind_import", "write_daemon_roots", "persist"];
        assert_eq!(fake.base.names_of(&ran), ran.to_vec());
        assert_eq!(fake.base.args_of("spawn_guard"), vec![json!({ "act": "import", "scoped": false })]);
        assert_eq!(fake.base.args_of("entry_of")[0]["id"], "ws_1");
        assert_eq!(fake.base.args_of("write_daemon_roots"), vec![json!({ "id": "ws_1", "strict": true, "landed": ["/root/a", "/root/work/proj"] })]);
        assert_eq!(rec(&entry).landed, Some(strings(&["/root/a", "/root/work/proj"])));
        assert_eq!(fake.base.persisted.borrow().len(), 1);
        assert_eq!(fake.base.persisted.borrow()[0].landed, Some(strings(&["/root/a", "/root/work/proj"])));
        let mut progress = import_event("uploading", "Part 1 of 1, 1 KB of 1 KB.", 40);
        progress["bytes"] = json!(1024);
        progress["total"] = json!(1024);
        let events = fake.base.events.borrow().clone();
        assert_eq!(
            events,
            vec![import_event("planned", "6 files, 4 KB; 0 secret-shaped files; 0 caches left behind.", 40), progress, import_event("done", "4 files, 4 KB, landed at /root/work/proj.", 40)]
        );
        assert_eq!(keys(&events[1]), vec!["type", "workspaceId", "source", "dest", "stage", "message", "elapsedMs", "bytes", "total"]);
        assert_eq!(result.dest, "/root/work/proj");
        assert_eq!(result.files.0, 4.0);

        // The same folder again names it once, and an import onto a record that named none starts the list.
        let again = area.import(plain_import(&fake), None).await.unwrap();
        assert_eq!(again.dest, "/root/work/proj");
        assert_eq!(rec(&entry).landed, Some(strings(&["/root/a", "/root/work/proj"])));
        let (fake, area, entry) = rig();
        area.import(plain_import(&fake), None).await.unwrap();
        assert_eq!(rec(&entry).landed, Some(strings(&["/root/work/proj"])));
    });
}

#[test]
fn a_refused_roots_write_takes_back_this_imports_dest_alone() {
    // spec import, project-import.test.ts:154 and folder-threads.test.ts:1258: the dest comes back off the record
    // unless it was there before, another import's folder stays, an empty list is removed, the refusal propagates.
    run(async {
        let refusal = "could not make /root/work/proj browsable on the machine: mkdir: read-only file system";
        for (before, meanwhile, after, name) in [
            (Some(vec!["/root/a"]), None, Some(vec!["/root/a"]), "another folder stays"),
            (None, None, None, "an empty list goes"),
            (Some(vec!["/root/work/proj"]), None, Some(vec!["/root/work/proj"]), "a dest already there stays"),
            (Some(vec!["/root/a"]), Some("/root/c"), Some(vec!["/root/a", "/root/c"]), "an import that landed meanwhile keeps its own"),
        ] {
            let (fake, area, entry) = rig();
            entry.borrow().record.borrow_mut().landed = before.map(|b| strings(&b));
            fake.base.answer_with("write_daemon_roots", vec![fails(refusal, None)]);
            if let Some(other) = meanwhile {
                fake.base.answer_with("write_daemon_roots_meanwhile", vec![json!(other)]);
            }
            let err = area.import(plain_import(&fake), None).await.unwrap_err();
            assert_eq!(err, plain(refusal), "{name}");
            assert_eq!(rec(&entry).landed, after.map(|a| strings(&a)), "{name}");
            assert_eq!(fake.base.persisted.borrow().len(), 0, "{name}");
            assert_eq!(fake.base.events.borrow().clone(), vec![import_event("failed", refusal, 0)], "{name}");
        }
    });
}

#[test]
fn import_reports_failed_on_any_failure() {
    // spec import: any failure after the copy-block check reports failed with its message; one before it reports nothing.
    run(async {
        // The kind's own import fails, after saying one stage.
        let (fake, area, entry) = rig();
        fake.base.answer_with("kind_import_reports", vec![json!([["planned", "6 files, 4 KB; 0 secret-shaped files; 0 caches left behind.", null]])]);
        fake.base.answer_with("kind_import", vec![fails("/root/work/proj already exists on the machine; import with replace to overwrite it", Some("exists"))]);
        let err = area.import(plain_import(&fake), None).await.unwrap_err();
        assert_eq!(err.message, "/root/work/proj already exists on the machine; import with replace to overwrite it");
        assert_eq!(
            fake.base.events.borrow().clone(),
            vec![import_event("planned", "6 files, 4 KB; 0 secret-shaped files; 0 caches left behind.", 0), import_event("failed", &err.message, 0)]
        );
        assert_eq!(fake.base.names_of(&["write_daemon_roots", "persist"]), Vec::<String>::new());
        assert_eq!(rec(&entry).landed, None);

        // The record write fails after the copy landed.
        let (fake, area, _entry) = rig();
        fake.base.answer_with("persist", vec![fails("disk full", None)]);
        let err = area.import(plain_import(&fake), None).await.unwrap_err();
        assert_eq!(err, plain("disk full"));
        assert_eq!(fake.base.events.borrow().clone(), vec![import_event("failed", "disk full", 0)]);

        // A copy the machine's computer blocks, and a guard that refuses, say nothing on the stream.
        let (fake, area, _entry) = rig();
        fake.base.answer_with("copy_blocked", vec![fails("this computer does not take a copy", Some("usage"))]);
        let err = area.import(plain_import(&fake), None).await.unwrap_err();
        assert_eq!(err, kinded("this computer does not take a copy", Kind::Usage));
        assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
        assert_eq!(fake.base.names_of(&["kind_import"]), Vec::<String>::new());
        let (fake, area, _entry) = rig();
        fake.base.answer_with("spawn_guard", vec![fails("a thread may not import", Some("usage"))]);
        let err = area.import(plain_import(&fake), Some(base::thread("t_lead", "ws_lead", "t_lead"))).await.unwrap_err();
        assert_eq!(err, kinded("a thread may not import", Kind::Usage));
        assert_eq!(fake.base.names_of(&["spawn_guard", "entry_of"]), vec!["spawn_guard"]);
    });
}

#[test]
fn import_refuses_a_gone_workspace() {
    // spec import: the action refusal is a plain error, thrown before the stream starts; project-import.test.ts:370.
    run(async {
        for (over, want) in [
            (json!({ "phase": "gone", "gone": "the provider no longer lists the machine" }), "task-1's machine is gone with its disk, so work that was not pushed is lost; rebuild it to import, which brings back its home folder from the last saved nap (the provider no longer lists the machine)"),
            (json!({ "phase": "napping" }), "Workspace is paused; wake it to import"),
            (json!({ "phase": "waking" }), "Workspace is waking; imports open when it is running"),
        ] {
            let fake = FakeProjects::new();
            let area = fake.area();
            fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1" })).tap(over));
            let err = area.import(plain_import(&fake), None).await.unwrap_err();
            assert_eq!(err, plain(want));
            assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
            assert_eq!(fake.base.names_of(&["copy_blocked", "kind_import", "write_daemon_roots", "persist"]), Vec::<String>::new());
            assert_eq!(fake.base.args_of("bundler_plan"), Vec::<Value>::new());
        }
        // An unknown workspace is the context's own refusal.
        let fake = FakeProjects::new();
        let area = fake.area();
        let mut ask = plain_import(&fake);
        ask.workspace_id = "ws_nope".to_owned();
        let err = area.import(ask, None).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_nope", Kind::NotFound));
        assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
    });
}

trait Tap {
    fn tap(self, over: Value) -> Self;
}

impl Tap for wsp_core::types::wiring::WorkspaceRecord {
    fn tap(self, over: Value) -> Self {
        let mut v = serde_json::to_value(&self).unwrap();
        base::merge(&mut v, over);
        serde_json::from_value(v).unwrap()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// export

fn export_opts(lander: &Rc<FakeLander>, replace: Option<bool>, agents: Option<&[&str]>) -> ProjectExportOptions {
    ProjectExportOptions {
        workspace_id: "ws_1".to_owned(),
        source: GUEST_SOURCE.to_owned(),
        dest: DEST.to_owned(),
        replace,
        agents: agents.map(strings),
        lander: lander.clone(),
    }
}

fn export_event(stage: &str, message: &str, progress: Option<(u64, u64)>) -> Value {
    let mut e = json!({ "type": "project.export", "workspaceId": "ws_1", "source": GUEST_SOURCE, "dest": DEST, "stage": stage, "message": message, "elapsedMs": 0 });
    if let Some((bytes, total)) = progress {
        e["bytes"] = json!(bytes);
        e["total"] = json!(total);
    }
    e
}

fn scratch_is_gone(fake: &FakeProjects) {
    assert_eq!(std::fs::read_dir(fake.tmp.path()).unwrap().count(), 0, "the scratch folder is removed");
}

fn homes() -> indexmap::IndexMap<String, String> {
    [("claude", "/root/.claude-cfg"), ("codex", "/root/.codex"), ("pi", "/root/.pi/agent"), ("hermes", "/root/.hermes")].into_iter().map(|(a, h)| (a.to_owned(), h.to_owned())).collect()
}

const LISTED: [&str; 2] = ["/root/.claude-cfg/projects/-root-work-proj", "/root/.codex/sessions/2026/09/06/rollout-t1.jsonl"];

fn listing_stdout() -> String {
    format!("{}\n{}\n{{SCRATCH}}/root/.codex/state_5.sqlite\n{{SCRATCH}}/root/.hermes/state.db\n", LISTED[0], LISTED[1])
}

fn three_agents(lander: &FakeLander) {
    *lander.agents.borrow_mut() = vec![
        landed_agent("Claude Code", json!({ "agent": "claude", "files": 3, "bytes": 300, "outcome": "moved", "sessions": 2 })),
        landed_agent("Codex", json!({ "agent": "codex", "files": 1, "bytes": 100, "outcome": "transcript-only", "sessions": 2, "skipped": 1 })),
        landed_agent("Hermes Agent", json!({ "agent": "hermes", "files": 0, "bytes": 0, "outcome": "nothing", "sessions": 1 })),
    ];
}

#[test]
fn export_lands_folder_and_state_and_says_done() {
    // spec export, project-export.test.ts:110: the folder and only the paths the listing names come home, both land
    // through the lander, every stage is said, every agent is in the result, the machine and the scratch are cleaned.
    run(async {
        let (fake, area, _entry) = rig();
        fake.base.answer_with("machine_run", vec![json!({ "exitCode": 0, "stdout": listing_stdout(), "stderr": "" })]);
        let lander = FakeLander::new(&fake.base);
        three_agents(&lander);
        let result = area.export(export_opts(&lander, None, None), Some(person())).await.unwrap();
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            json!({
                "dest": DEST, "files": 3, "bytes": 4000, "excluded": ["dist", "node_modules"],
                "agents": [
                    { "agent": "claude", "files": 3, "bytes": 300, "outcome": "moved", "sessions": 2 },
                    { "agent": "codex", "files": 1, "bytes": 100, "outcome": "transcript-only", "sessions": 2, "skipped": 1 },
                    { "agent": "hermes", "files": 0, "bytes": 0, "outcome": "nothing", "sessions": 1 },
                ],
            })
        );
        let done = "3 files, 4 KB, landed at /Users/dev/code/proj; 2 caches left behind; sessions: Claude Code (2 sessions) moved, Codex (2 sessions) transcripts landed but not yet in its session list here, 1 indexed rollout not under sessions/ skipped, Hermes Agent (1 session) had nothing to bring.";
        let events = fake.base.events.borrow().clone();
        assert_eq!(
            events,
            vec![
                export_event("packing", "Packing /root/work/proj on the machine.", None),
                export_event("downloading", "The folder: 0 B of 6 B.", Some((0, 6))),
                export_event("downloading", "The folder: 6 B of 6 B.", Some((6, 6))),
                export_event("packing", "Packing the agents' state for it on the machine.", None),
                export_event("downloading", "Agent state: 0 B of 5 B.", Some((0, 5))),
                export_event("downloading", "Agent state: 5 B of 5 B.", Some((5, 5))),
                export_event("landing", "Landing at /Users/dev/code/proj.", None),
                export_event("done", done, None),
            ]
        );
        assert_eq!(keys(&events[1]), vec!["type", "workspaceId", "source", "dest", "stage", "message", "elapsedMs", "bytes", "total"]);
        let ran = ["spawn_guard", "entry_of", "copy_blocked", "guest_agent_homes", "lander_probe", "export_folder", "machine_run", "export_paths_into", "lander_land", "machine_exec"];
        assert_eq!(fake.base.names_of(&ran), ran.to_vec());
        assert_eq!(fake.base.args_of("spawn_guard"), vec![json!({ "act": "export", "scoped": false })]);
        assert_eq!(fake.base.args_of("lander_probe"), vec![json!({ "dest": DEST })]);

        // The listing is the engine's, written for the scratch root the machine is later told to remove.
        let run_args = &fake.base.args_of("machine_run")[0];
        let script = run_args["script"].as_str().unwrap();
        let root = scratch_root(script).expect("the listing names a scratch root");
        assert_eq!(script, state_listing(&homes(), GUEST_SOURCE, &root, None).unwrap());
        assert_eq!(run_args["deadlineMs"], 120_000);
        assert_eq!(fake.base.args_of("machine_exec"), vec![json!({ "id": "ws_1", "cmd": format!("rm -rf '{root}'"), "timeoutMs": 20_000 })]);
        // The copies travel from the scratch root that mirrors the homes, the rest from the root of the machine.
        let archive = fake.base.args_of("export_folder")[0]["archive"].as_str().unwrap().to_owned();
        assert_eq!(fake.base.args_of("export_folder")[0]["source"], GUEST_SOURCE);
        let state_archive = fake.base.args_of("export_paths_into")[0]["archive"].as_str().unwrap().to_owned();
        assert_eq!(
            fake.base.args_of("export_paths_into")[0]["groups"],
            json!([["/", [LISTED[0], LISTED[1]]], [root, [format!("{root}/root/.codex/state_5.sqlite"), format!("{root}/root/.hermes/state.db")]]])
        );
        // The lander is handed both archives as they stood on disk, and the homes the state mirrors.
        let landing = &fake.base.args_of("lander_land")[0];
        assert_eq!((landing["source"].clone(), landing["dest"].clone(), landing["replace"].clone()), (json!(GUEST_SOURCE), json!(DEST), json!(false)));
        assert_eq!(landing["archive"], archive);
        assert_eq!(landing["state"], json!({ "archive": state_archive, "homes": homes(), "agents": null }));
        assert_eq!(landing["unread"], Value::Null);
        let seen = lander.seen.borrow().clone();
        assert_eq!((seen[0].1.as_str(), seen[0].2.as_deref()), ("FOLDER", Some("STATE")));
        let scratch = std::path::Path::new(&archive).parent().unwrap();
        assert!(scratch.starts_with(fake.tmp.path()));
        assert!(scratch.file_name().unwrap().to_string_lossy().starts_with("wsp-exported-"));
        assert!(!scratch.exists());
        scratch_is_gone(&fake);
    });
}

#[test]
fn a_store_the_machine_cannot_read_brings_nothing_from_that_home_it_is_not_in_the_a() {
    // project-export.test.ts:167: the listing's marked line becomes the lander's `unread`, and the lander's failed
    // row carries the same sentence into the done line.
    run(async {
        use base64::Engine;
        let (fake, area, _entry) = rig();
        let unread = json!({ "agent": "hermes", "store": "/root/.hermes/state.db", "why": "file is not a database" });
        let marked = format!("!unread {}", base64::engine::general_purpose::STANDARD.encode(unread.to_string()));
        let stdout = format!("{}\n{{SCRATCH}}/root/.codex/state_5.sqlite\n{marked}\n", LISTED[0]);
        fake.base.answer_with("machine_run", vec![json!({ "exitCode": 0, "stdout": stdout, "stderr": "" })]);
        let lander = FakeLander::new(&fake.base);
        let sentence = "could not read /root/.hermes/state.db on the machine, so nothing from it travelled: file is not a database";
        *lander.agents.borrow_mut() = vec![landed_agent("Hermes Agent", json!({ "agent": "hermes", "files": 0, "bytes": 0, "outcome": "failed", "error": sentence }))];
        let result = area.export(export_opts(&lander, None, None), None).await.unwrap();
        assert_eq!(fake.base.args_of("lander_land")[0]["unread"], json!([unread]));
        assert_eq!(serde_json::to_value(&result.agents).unwrap(), json!([{ "agent": "hermes", "files": 0, "bytes": 0, "outcome": "failed", "error": sentence }]));
        assert!(fake.base.events.borrow().last().unwrap()["message"].as_str().unwrap().contains(&format!("Hermes Agent failed: {sentence}")));
        // Neither the store nor the home beside it is asked for: only the path and the copy come.
        let groups = fake.base.args_of("export_paths_into")[0]["groups"].clone();
        assert!(!groups.to_string().contains(".hermes"));
    });
}

#[test]
fn export_refuses_a_standing_dest_unless_replace() {
    // spec export, project-export.test.ts:187: a dest that stands is refused with kind exists before the machine is
    // asked anything, and replaced when asked.
    run(async {
        let (fake, area, _entry) = rig();
        let lander = FakeLander::new(&fake.base);
        fake.base.answer_with("lander_probe", vec![json!(12)]);
        let err = area.export(export_opts(&lander, None, None), None).await.unwrap_err();
        let said = "/Users/dev/code/proj already exists on this computer with 12 files; export with replace to overwrite it";
        assert_eq!((err.message.as_str(), err.kind.clone()), (said, Some(Kind::Exists)));
        assert_eq!(fake.base.names_of(&["export_folder", "machine_run", "export_paths_into", "lander_land", "machine_exec"]), Vec::<String>::new());
        assert_eq!(fake.base.events.borrow().clone(), vec![export_event("failed", said, None)]);
        scratch_is_gone(&fake);

        fake.base.events.borrow_mut().clear();
        let result = area.export(export_opts(&lander, Some(true), None), None).await.unwrap();
        let landing = &fake.base.args_of("lander_land")[0];
        assert_eq!((landing["replace"].clone(), landing["state"].clone()), (json!(true), Value::Null));
        assert!(result.agents.is_empty());
        let stages: Vec<Value> = fake.base.events.borrow().iter().map(|e| e["stage"].clone()).collect();
        assert_eq!(stages, vec!["packing", "downloading", "downloading", "landing", "done"]);
        assert_eq!(
            fake.base.events.borrow().last().unwrap()["message"],
            "3 files, 4 KB, landed at /Users/dev/code/proj; 2 caches left behind; no agent sessions for it on the machine."
        );
        // A listing that named nothing still leaves the machine without the scratch root it was given.
        let cmd = fake.base.args_of("machine_exec")[0]["cmd"].as_str().unwrap().to_owned();
        assert!(cmd.starts_with("rm -rf '/tmp/wsp-state-") && cmd.ends_with('\''), "{cmd}");
        scratch_is_gone(&fake);
    });
}

#[test]
fn agents_named_narrow_the_roots_looked_for_on_the_machine_and_reach_the_lander() {
    // project-export.test.ts:211.
    run(async {
        let (fake, area, _entry) = rig();
        fake.base.answer_with("machine_run", vec![json!({ "exitCode": 0, "stdout": "/root/.claude-cfg/projects/-root-work-proj\n", "stderr": "" })]);
        let lander = FakeLander::new(&fake.base);
        *lander.agents.borrow_mut() = vec![landed_agent("Claude Code", json!({ "agent": "claude", "files": 3, "bytes": 300, "outcome": "moved", "sessions": 2 }))];
        let result = area.export(export_opts(&lander, None, Some(&["claude", "pi"])), None).await.unwrap();
        let script = fake.base.args_of("machine_run")[0]["script"].as_str().unwrap().to_owned();
        let cmd = fake.base.args_of("machine_exec")[0]["cmd"].as_str().unwrap().to_owned();
        let root = cmd.strip_prefix("rm -rf '").and_then(|c| c.strip_suffix('\'')).expect("the machine is told to remove the scratch root").to_owned();
        assert_eq!(script, state_listing(&homes(), GUEST_SOURCE, &root, Some(&strings(&["claude", "pi"]))).unwrap());
        assert_eq!(fake.base.args_of("lander_land")[0]["state"]["agents"], json!(["claude", "pi"]));
        assert_eq!(result.agents.iter().map(|a| a.agent.clone()).collect::<Vec<_>>(), vec!["claude"]);
        assert_eq!(fake.base.args_of("export_paths_into")[0]["groups"], json!([["/", ["/root/.claude-cfg/projects/-root-work-proj"]]]));
    });
}

#[test]
fn a_folder_that_is_not_on_the_machine_a_napping_workspace_and_an_unknown_one_are_r() {
    // project-export.test.ts:226: the lander is never asked to land; the probe runs for a folder not there only.
    run(async {
        let (fake, area, _entry) = rig();
        let lander = FakeLander::new(&fake.base);
        fake.base.answer_with("export_folder_fails", vec![fails("/root/work/proj is not a folder on the machine", None)]);
        let err = area.export(export_opts(&lander, None, None), None).await.unwrap_err();
        assert_eq!(err, plain("/root/work/proj is not a folder on the machine"));
        assert_eq!(fake.base.events.borrow().last().unwrap(), &export_event("failed", "/root/work/proj is not a folder on the machine", None));
        assert_eq!(fake.base.args_of("lander_probe"), vec![json!({ "dest": DEST })]);

        let fake = FakeProjects::new();
        let area = fake.area();
        fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1", "phase": "napping" })));
        let lander = FakeLander::new(&fake.base);
        let err = area.export(export_opts(&lander, None, None), None).await.unwrap_err();
        assert_eq!(err, plain("Workspace is paused; wake it to export"));
        let mut gone = export_opts(&lander, None, None);
        gone.workspace_id = "ws_nope".to_owned();
        let err = area.export(gone, None).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_nope", Kind::NotFound));
        assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
        assert_eq!(fake.base.names_of(&["lander_probe", "lander_land", "copy_blocked"]), Vec::<String>::new());
    });
}

#[test]
fn export_refuses_a_gone_workspace_and_a_blocked_copy_before_the_stream_starts() {
    // spec export: the action refusal is a plain error and copy_blocked comes after it, both before `began`.
    run(async {
        let fake = FakeProjects::new();
        let area = fake.area();
        fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1", "phase": "gone" })));
        let lander = FakeLander::new(&fake.base);
        let err = area.export(export_opts(&lander, None, None), None).await.unwrap_err();
        assert_eq!(err, plain("task-1's machine is gone with its disk, so work that was not pushed is lost; rebuild it to export, which brings back its home folder from the last saved nap"));
        assert_eq!(fake.base.names_of(&["copy_blocked", "lander_probe"]), Vec::<String>::new());

        let (fake, area, _entry) = rig();
        fake.base.answer_with("copy_blocked", vec![fails("this computer does not take a copy", Some("usage"))]);
        let lander = FakeLander::new(&fake.base);
        let err = area.export(export_opts(&lander, None, None), None).await.unwrap_err();
        assert_eq!(err, kinded("this computer does not take a copy", Kind::Usage));
        assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
        assert_eq!(fake.base.names_of(&["lander_probe", "export_folder"]), Vec::<String>::new());
        scratch_is_gone(&fake);
    });
}

#[test]
fn export_cleans_the_machine_and_the_scratch_on_failure() {
    // spec export, "Always": where a listing ran, rm -rf of its scratch root (a failure of it ignored), and the scratch
    // folder removed, whatever failed.
    run(async {
        // The folder's pack fails before any listing ran: no rm on the machine, the scratch folder gone, failed said.
        let (fake, area, _entry) = rig();
        fake.base.answer_with("export_folder_fails", vec![fails("packing /root/work/proj on the machine failed (exit 2): tar: no space left", None)]);
        let lander = FakeLander::new(&fake.base);
        let err = area.export(export_opts(&lander, None, None), None).await.unwrap_err();
        assert_eq!(err.message, "packing /root/work/proj on the machine failed (exit 2): tar: no space left");
        assert_eq!(fake.base.names_of(&["machine_run", "machine_exec", "lander_land"]), Vec::<String>::new());
        assert_eq!(fake.base.events.borrow().last().unwrap(), &export_event("failed", &err.message, None));
        scratch_is_gone(&fake);

        // The listing fails: the last 200 UTF-16 units of its stderr, and the scratch root is still removed.
        let (fake, area, _entry) = rig();
        let stderr = format!("{}{}", "a".repeat(150), "\u{1F600}".repeat(30));
        fake.base.answer_with("machine_run", vec![json!({ "exitCode": 3, "stdout": "", "stderr": stderr })]);
        let lander = FakeLander::new(&fake.base);
        let err = area.export(export_opts(&lander, None, None), None).await.unwrap_err();
        let tail = format!("{}{}", "a".repeat(140), "\u{1F600}".repeat(30));
        assert_eq!(err, plain(&format!("could not look for agent state on the machine: {tail}")));
        assert_eq!(fake.base.events.borrow().last().unwrap(), &export_event("failed", &err.message, None));
        assert_eq!(fake.base.names_of(&["export_paths_into", "lander_land"]), Vec::<String>::new());
        let script = fake.base.args_of("machine_run")[0]["script"].as_str().unwrap().to_owned();
        let root = scratch_root(&script).unwrap();
        assert_eq!(fake.base.args_of("machine_exec"), vec![json!({ "id": "ws_1", "cmd": format!("rm -rf '{root}'"), "timeoutMs": 20_000 })]);
        scratch_is_gone(&fake);

        // The landing fails after both archives came: cleaned the same way.
        let (fake, area, _entry) = rig();
        fake.base.answer_with("machine_run", vec![json!({ "exitCode": 0, "stdout": format!("{}\n", LISTED[0]), "stderr": "" })]);
        fake.base.answer_with("lander_land", vec![fails("could not write /Users/dev/code/proj: permission denied", None)]);
        let lander = FakeLander::new(&fake.base);
        let err = area.export(export_opts(&lander, None, None), None).await.unwrap_err();
        assert_eq!(err.message, "could not write /Users/dev/code/proj: permission denied");
        assert_eq!(fake.base.events.borrow().last().unwrap(), &export_event("failed", &err.message, None));
        assert_eq!(fake.base.names_of(&["machine_exec"]), vec!["machine_exec"]);
        scratch_is_gone(&fake);

        // A machine that will not take the rm is ignored: the export stands.
        let (fake, area, _entry) = rig();
        fake.base.answer_with("machine_exec", vec![fails("machine is gone", None)]);
        let lander = FakeLander::new(&fake.base);
        let result = area.export(export_opts(&lander, None, None), None).await.unwrap();
        assert_eq!(result.dest, DEST);
        assert_eq!(fake.base.events.borrow().last().unwrap()["stage"], "done");
        scratch_is_gone(&fake);
    });
}

#[test]
fn export_asks_the_machine_nothing_when_no_agent_has_a_home() {
    // spec export: an empty listing is never run, so there is nothing to remove afterwards.
    run(async {
        let (fake, area, _entry) = rig();
        fake.base.answer_with("guest_agent_homes", vec![json!({})]);
        let lander = FakeLander::new(&fake.base);
        let result = area.export(export_opts(&lander, None, None), None).await.unwrap();
        assert_eq!(fake.base.names_of(&["machine_run", "machine_exec"]), Vec::<String>::new());
        assert!(result.agents.is_empty());
        assert_eq!(fake.base.args_of("lander_land")[0]["state"], Value::Null);
        scratch_is_gone(&fake);
    });
}
