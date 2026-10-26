// SPDX-License-Identifier: AGPL-3.0-only
//! Recording a project and reading what a folder of the person's would seed: the landing kind and deps, the menu and
//! the picks, and the add with every refusal, stage and record. Ported from packages/runtime/test/projects.test.ts,
//! project-landing.test.ts, project-seed-picks.test.ts and places-folders.test.ts at 65f6b89c7, at the area's seam over
//! `FakeProjects`; every sentence is the TS's own text (spec/projects_add.md).
//!
//! The fakes: computers are `here` (Mac), `pl_box` (box, joined) and `solari` (a provider named by its id). A folder
//! on a computer that clones is seeded only where `kind_of` and `kind_for_computer` disagree (the TS's seeding branch
//! reads `kindOf` for the road and `kindForComputer` for the sources), so the seeding rig answers `Place` for `here`.
#[path = "support/base.rs"]
mod base;
#[path = "support/projects.rs"]
mod projects;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use base::{kinded, person, plain, project, run, thread};
use indexmap::IndexMap;
use projects::{add_stages, fails, keys, menu, pj, FakeProjects, StubBackend};
use serde_json::{json, Value};
use wsp_account::projects::projects::add::{AddAsk, LandingKind, SeedWords};
use wsp_account::projects::projects::Projects;
use wsp_wire::views::workspace::{Caller, ProjectView, SeedChoice};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::exit::{HostError, Kind};

const REPO: &str = "https://github.com/spoo-me/frontend.git";
const LAB: &str = "/home/me/code/lab";
const CREATED: &str = "1970-01-01T00:16:40.000Z";

fn rig() -> (Rc<FakeProjects>, Rc<Projects>) {
    let fake = FakeProjects::new();
    let area = fake.area();
    (fake, area)
}

/// A rig where a folder here takes the road of a computer that clones, so the add seeds it.
fn seeding_rig() -> (Rc<FakeProjects>, Rc<Projects>) {
    let (fake, area) = rig();
    fake.base.kinds.borrow_mut().insert("here".to_owned(), WorkspaceKind::Place);
    (fake, area)
}

fn ask(source: &str) -> AddAsk {
    AddAsk { source: source.to_owned(), ..Default::default() }
}

fn on(source: &str, computer: &str) -> AddAsk {
    AddAsk { on: Some(computer.to_owned()), ..ask(source) }
}

fn lead() -> Caller {
    thread("t_lead", "ws_lead", "t_lead")
}

fn lab_folder() -> ProjectView {
    project("pr_lab", "lab", "here", json!({ "source": { "kind": "folder", "path": LAB }, "path": LAB }))
}

fn hold_lab(fake: &FakeProjects) -> ProjectView {
    let lab = lab_folder();
    fake.base.hold_project(lab.clone());
    lab
}

fn lab_menu() -> Value {
    serde_json::to_value(menu(LAB)).unwrap()
}

fn choice(files: &[&str], memory: bool, remember: Option<bool>) -> SeedChoice {
    SeedChoice { files: files.iter().map(|s| (*s).to_owned()).collect(), memory, commits: false, remember }
}

fn heard() -> (Rc<RefCell<Vec<String>>>, Rc<dyn Fn(&str)>) {
    let lines = Rc::new(RefCell::new(vec![]));
    let sink = lines.clone();
    (lines, Rc::new(move |l: &str| sink.borrow_mut().push(l.to_owned())))
}

fn stage(stage: &str, message: &str, elapsed: i64) -> Value {
    json!({ "type": "project.add", "projectId": "pr_11111111", "computer": "here", "stage": stage, "message": message, "elapsedMs": elapsed })
}

// ---------------------------------------------------------------------------------------------------------------
// landing kind, deps, readable computer

#[test]
fn landing_kind_by_kind() {
    // spec rule 2: the road comes off the kind of workspace a computer makes, never off its id.
    let (fake, area) = rig();
    assert_eq!(area.landing_kind("here"), LandingKind::Mac);
    assert_eq!(area.landing_kind("pl_box"), LandingKind::Box);
    assert_eq!(area.landing_kind("solari"), LandingKind::Provider);
    fake.base.kinds.borrow_mut().insert("laptop".to_owned(), WorkspaceKind::Place);
    fake.base.kinds.borrow_mut().insert("copy-of-here".to_owned(), WorkspaceKind::Local);
    assert_eq!(area.landing_kind("laptop"), LandingKind::Box);
    assert_eq!(area.landing_kind("copy-of-here"), LandingKind::Mac);
}

fn token_env() -> IndexMap<String, String> {
    [("GITHUB_TOKEN".to_owned(), "tok".to_owned())].into_iter().collect()
}

#[test]
fn landing_deps_gives_the_vault_token_only_where_github_came_from_the_vault() {
    // spec rule 3: the joined computer, the place, the backend read that may fail, the Mac's state home, the name.
    run(async {
        let (fake, area) = rig();
        fake.joined.borrow_mut().insert("pl_box".to_owned(), projects::joined("/home/maya"));
        *fake.vault_token.borrow_mut() = Some("tok".to_owned());
        fake.vault_places.borrow_mut().push("pl_box".to_owned());
        let at = area.landing_deps("pl_box").await.unwrap();
        assert_eq!(at.place_id.as_deref(), Some("pl_box"));
        assert!(at.at.is_some());
        assert_eq!(at.deps.clone_env, token_env());
        assert_eq!(at.deps.computer.as_ref().map(|c| c.home.clone()), Some("/home/maya".to_owned()));
        assert_eq!(at.deps.computer_name, "box");
        assert_eq!(at.deps.mac_state_home, "/home/me/.claude");
        assert_eq!(
            fake.base.names_of(&["landing_place", "forking_at", "folder_computer", "github_from_vault"]),
            vec!["landing_place", "forking_at", "folder_computer", "github_from_vault"]
        );
        assert_eq!(fake.base.args_of("forking_at"), vec![json!({ "placeId": "pl_box" })]);
        assert_eq!(fake.base.args_of("github_from_vault"), vec![json!({ "placeId": "pl_box" })]);

        // The vault holds a token but GitHub was not set up from it there: nothing is handed over.
        let (fake, area) = rig();
        *fake.vault_token.borrow_mut() = Some("tok".to_owned());
        let at = area.landing_deps("pl_box").await.unwrap();
        assert!(at.deps.clone_env.is_empty());
        assert!(at.deps.computer.is_none());

        // GitHub was set up from the vault there, but the vault holds no token: nothing is asked of the place.
        let (fake, area) = rig();
        fake.vault_places.borrow_mut().push("pl_box".to_owned());
        let at = area.landing_deps("pl_box").await.unwrap();
        assert!(at.deps.clone_env.is_empty());
        assert_eq!(fake.base.args_of("github_from_vault"), Vec::<Value>::new());

        // This computer names no place: no joined computer is read and no token goes, whatever the vault holds.
        let (fake, area) = rig();
        *fake.vault_token.borrow_mut() = Some("tok".to_owned());
        fake.vault_places.borrow_mut().push("here".to_owned());
        *fake.claude_home.borrow_mut() = None;
        let at = area.landing_deps("here").await.unwrap();
        assert_eq!(at.place_id, None);
        assert!(at.deps.clone_env.is_empty());
        assert_eq!(at.deps.computer_name, "Mac");
        assert_eq!(at.deps.mac_state_home, "");
        assert_eq!(fake.base.args_of("forking_at"), vec![json!({ "placeId": null })]);
        assert_eq!(fake.base.names_of(&["folder_computer", "github_from_vault"]), Vec::<String>::new());

        // A computer this host cannot read a backend for: the deps still stand and the backend is none.
        let (fake, area) = rig();
        fake.base.answer_with("forking_at", vec![fails("no backend for pl_box", None)]);
        let at = area.landing_deps("pl_box").await.unwrap();
        assert!(at.at.is_none());
        assert_eq!(at.place_id.as_deref(), Some("pl_box"));
    });
}

#[test]
fn readable_computer_answers_an_absent_computers_sentence() {
    // spec rule 4: no backend and a place named: the backend's own refusal; nothing asked otherwise.
    run(async {
        let (fake, area) = rig();
        let absent = "box is not answering; it connects on its own when it is on";
        fake.base.answer_with("landing_backend", vec![fails(absent, None)]);
        let refusal = area.readable_computer(None, Some("pl_box")).await.unwrap_err();
        assert_eq!(refusal, plain(absent));
        assert_eq!(fake.base.args_of("landing_backend"), vec![json!({ "placeId": "pl_box" })]);
        let backend: Arc<dyn wsp_engine::machine::MachineBackend> = Arc::new(StubBackend);
        area.readable_computer(Some(&backend), Some("pl_box")).await.unwrap();
        area.readable_computer(None, None).await.unwrap();
        assert_eq!(fake.base.args_of("landing_backend").len(), 1);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// seed_plan

fn ticked(plan: &wsp_wire::views::workspace::SeedPlan) -> Vec<bool> {
    plan.files.iter().map(|f| f.ticked).collect()
}

#[test]
fn seed_plan_ticks_from_the_remembered_choice() {
    // spec rule 5, project-landing.test.ts:164: the catalogue's ticks with nothing remembered, the remembered files
    // with a choice; nothing is read whole, nothing stored, and no computer row is read where none is named.
    run(async {
        let (fake, area) = rig();
        let plan = area.seed_plan(LAB, None).await.unwrap();
        assert_eq!(ticked(&plan), vec![true, true, false, false, true]);
        assert!(!plan.remembered);
        assert_eq!(plan.provider, None);
        assert_eq!(
            fake.base.names_of(&["ready", "homes_here", "seed_wiring_plan", "computer_rows"]),
            vec!["ready", "homes_here", "seed_wiring_plan"]
        );
        assert_eq!(fake.base.args_of("seed_wiring_plan"), vec![json!({ "folder": LAB, "homes": { "claude": "/home/me/.claude" } })]);
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());

        fake.base.store.seed("seed-choices", LAB, json!({ "files": [".env"], "memory": false, "commits": false, "remember": true }));
        let plan = area.seed_plan(LAB, None).await.unwrap();
        assert!(plan.remembered);
        assert_eq!(ticked(&plan), vec![true, false, false, false, false]);
        let mut want = lab_menu();
        want["remembered"] = json!(true);
        for (i, t) in [true, false, false, false, false].into_iter().enumerate() {
            want["files"][i]["ticked"] = json!(t);
        }
        assert_eq!(serde_json::to_value(&plan).unwrap(), want);
    });
}

#[test]
fn stays_ticked_for_a_computer_of_the_person_s_own_remembered_or_not_and_is_unticke() {
    // project-landing.test.ts:164: a file of values stays ticked for a computer of the person's own and is unticked for
    // a provider place, which the plan names; the remembered choice does not change that.
    run(async {
        let (fake, area) = rig();
        let own = area.seed_plan(LAB, Some("pl_box")).await.unwrap();
        assert_eq!((own.provider, ticked(&own)), (None, vec![true, true, false, false, true]));
        let cloud = area.seed_plan(LAB, Some("solari")).await.unwrap();
        assert_eq!((cloud.provider, ticked(&cloud)), (Some(true), vec![true, true, false, false, false]));
        let unknown = area.seed_plan(LAB, Some("nowhere")).await.unwrap();
        assert_eq!(unknown.provider, None);
        fake.base.store.seed("seed-choices", LAB, json!({ "files": [".env"], "memory": false, "commits": false, "remember": true }));
        let box_plan = area.seed_plan(LAB, Some("box")).await.unwrap();
        assert_eq!((box_plan.remembered, ticked(&box_plan)), (true, vec![true, false, false, false, false]));
        let remembered = area.seed_plan(LAB, Some("solari")).await.unwrap();
        assert_eq!(
            (remembered.remembered, remembered.provider, ticked(&remembered)),
            (true, Some(true), vec![true, false, false, false, false])
        );
        assert_eq!(fake.base.names_of(&["computer_rows"]).len(), 5);
    });
}

#[test]
fn seed_plan_on_a_provider_marks_provider_and_unticks_secrets() {
    // spec rule 5 and its edge case: a provider row named by `on` marks the plan and unticks every file of values or
    // key, even one the remembered choice names.
    run(async {
        let (fake, area) = rig();
        fake.base.store.seed(
            "seed-choices",
            LAB,
            json!({ "files": [".env", "deploy.pem"], "memory": false, "commits": false, "remember": true }),
        );
        let plan = area.seed_plan(LAB, Some("solari")).await.unwrap();
        let mut want = lab_menu();
        want["provider"] = json!(true);
        want["remembered"] = json!(true);
        for (i, t) in [true, false, false, false, false].into_iter().enumerate() {
            want["files"][i]["ticked"] = json!(t);
        }
        assert_eq!(serde_json::to_value(&plan).unwrap(), want);
        // By the name a table prints for it, in any case.
        let by_name = area.seed_plan(LAB, Some("Solari")).await.unwrap();
        assert_eq!(by_name.provider, Some(true));
        assert_eq!(fake.base.args_of("seed_wiring_plan").len(), 2);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// seed

fn seed_answer(seed: &wsp_wire::views::workspace::ProjectSeed) -> Value {
    serde_json::to_value(seed).unwrap()
}

fn rows(picked: &[&str]) -> Value {
    let all = [
        (".env", false, 1126, "config", "environment file"),
        (".env.local", false, 132, "config", "environment file"),
        (".git-credentials", false, 300, "never", "never travels"),
        ("node_modules", true, 831520768u64, "rebuilt", "rebuilt on the box"),
        ("deploy.pem", false, 4096, "secret", "stays on this computer"),
    ];
    Value::Array(
        all.iter()
            .map(|(p, d, b, k, w)| json!({ "path": p, "dir": d, "bytes": b, "kind": k, "why": w, "picked": picked.contains(p) }))
            .collect(),
    )
}

fn put_prior(fake: &FakeProjects, files: &[&str]) {
    fake.base.store.seed("seed-choices", LAB, json!({ "files": files, "memory": false, "commits": false, "remember": true }));
}

fn saved_files(fake: &FakeProjects) -> Value {
    fake.base.store.row("seed-choices", LAB).unwrap()["files"].clone()
}

#[test]
fn print_the_menu_with_nothing_picked_where_the_folder_has_no_saved_picks_and_save_() {
    // project-seed-picks.test.ts:75.
    run(async {
        let (fake, area) = rig();
        hold_lab(&fake);
        let seed = area.seed("lab", SeedWords::default(), None).await.unwrap();
        assert_eq!(
            seed_answer(&seed),
            json!({ "project": "lab", "folder": LAB, "rows": rows(&[]), "picked": { "paths": [], "bytes": 0, "missing": [] }, "saved": false })
        );
        assert_eq!(fake.base.store.row("seed-choices", LAB), None);
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
    });
}

#[test]
fn seed_refuses_a_project_not_a_folder_here() {
    // spec rule 6, project-seed-picks.test.ts:166: a project on a box (or a repo) is refused naming the box.
    run(async {
        let (fake, area) = rig();
        fake.base.hold_project(project("pr_web", "web", "pl_box", json!({ "source": { "kind": "git", "url": REPO } })));
        let want = base::refused(
            "web lives on box, and picks are read from folders on this computer only. A sub-thread of web starts with no git-ignored files.",
            "A sub-thread of web starts with no git-ignored files.",
            Kind::Usage,
        );
        assert_eq!(area.seed("web", SeedWords::default(), None).await.unwrap_err(), want);
        // A folder that lives on the box is refused the same way, and a repo recorded here is no folder.
        fake.base.hold_project(project("pr_boxdir", "boxdir", "pl_box", json!({})));
        let want = base::refused(
            "boxdir lives on box, and picks are read from folders on this computer only. A sub-thread of boxdir starts with no git-ignored files.",
            "A sub-thread of boxdir starts with no git-ignored files.",
            Kind::Usage,
        );
        assert_eq!(area.seed("boxdir", SeedWords { save: true, ..Default::default() }, None).await.unwrap_err(), want);
        fake.base.hold_project(project("pr_repo", "repo", "here", json!({ "source": { "kind": "git", "url": REPO } })));
        let err = area.seed("repo", SeedWords::default(), None).await.unwrap_err();
        assert_eq!(err.message, "repo lives on Mac, and picks are read from folders on this computer only. A sub-thread of repo starts with no git-ignored files.");
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
        assert_eq!(fake.base.names_of(&["seed_wiring_plan"]), Vec::<String>::new());
    });
}

#[test]
fn invariant_3_a_thread_cannot_save_seed_picks() {
    // spec rule 6, project-seed-picks.test.ts:153: a thread reads the menu and is refused the save, by the scope read
    // off its origin; the same words from the person save.
    run(async {
        let (fake, area) = rig();
        hold_lab(&fake);
        fake.base.answer_with("project_of_scope", vec![json!("pr_lab")]);
        let keep = SeedWords { keep: Some(vec![".env".to_owned()]), ..Default::default() };
        let read = area.seed("lab", keep.clone(), Some(lead())).await.unwrap();
        assert_eq!(read.picked.paths, vec![".env".to_owned()]);
        assert_eq!(fake.base.args_of("project_of_scope"), vec![json!({ "scoped": true })]);

        let refusal = area.seed("lab", SeedWords { save: true, ..keep.clone() }, Some(lead())).await.unwrap_err();
        let said =
            "picking the files that leave this computer is the person's act. Hand them the line wsp projects seed lab --keep .env --yes";
        assert_eq!(refusal, base::refused(said, "Hand them the line wsp projects seed lab --keep .env --yes", Kind::Usage));
        assert_eq!(fake.base.store.row("seed-choices", LAB), None);
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
        // The refusal comes before the folder is read: one plan read for the first, none for the second.
        assert_eq!(fake.base.args_of("seed_wiring_plan").len(), 1);

        let saved = area.seed("lab", SeedWords { save: true, ..keep }, Some(person())).await.unwrap();
        assert!(saved.saved);
        assert_eq!(
            fake.base.store.row("seed-choices", LAB),
            Some(json!({ "files": [".env"], "memory": false, "commits": false, "remember": true }))
        );
    });
}

#[test]
fn read_as_a_provider_s_menu_since_only_a_sub_thread_reads_them_a_key_stays_here_is() {
    // project-seed-picks.test.ts:94: a key stays here, is refused by name, and a save keeps the folder's own pick of it
    // until it is cut by name.
    run(async {
        let (fake, area) = rig();
        hold_lab(&fake);
        put_prior(&fake, &[".env", "deploy.pem"]);
        let seed = area.seed("lab", SeedWords::default(), None).await.unwrap();
        assert_eq!(seed_answer(&seed)["picked"], json!({ "paths": [".env"], "bytes": 1126, "missing": [] }));
        let refusal =
            area.seed("lab", SeedWords { keep: Some(vec!["deploy.pem".to_owned()]), ..Default::default() }, None).await.unwrap_err();
        assert_eq!(
            refusal,
            kinded(
                "deploy.pem holds values or a key, which stay on this computer for a provider place; only a box of your own gets them",
                Kind::Invalid
            )
        );
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
        // A box's add reads this same record, so a save made off the provider's menu keeps the key in it.
        area.seed("lab", SeedWords { cut: Some(vec![".env".to_owned()]), save: true, ..Default::default() }, None).await.unwrap();
        assert_eq!(
            fake.base.store.row("seed-choices", LAB),
            Some(json!({ "files": ["deploy.pem"], "memory": false, "commits": false, "remember": true }))
        );
        area.seed("lab", SeedWords { cut: Some(vec!["deploy.pem".to_owned()]), save: true, ..Default::default() }, None).await.unwrap();
        assert_eq!(saved_files(&fake), json!([]));
        assert_eq!(fake.base.store.writes.borrow().clone(), vec!["put seed-choices/".to_owned() + LAB; 2]);
    });
}

#[test]
fn seed_save_keeps_held_back_files_unless_cut_by_name() {
    // spec rule 6: held = the files of values a provider never gets, from the prior choice, less the cuts with their
    // trailing slashes stripped; the other flags of the prior choice are kept.
    run(async {
        let (fake, area) = rig();
        hold_lab(&fake);
        fake.base.store.seed(
            "seed-choices",
            LAB,
            json!({ "files": [".env", "deploy.pem"], "memory": true, "commits": true, "remember": true }),
        );
        area.seed("lab", SeedWords { keep: Some(vec![".env.local".to_owned()]), save: true, ..Default::default() }, None).await.unwrap();
        assert_eq!(
            fake.base.store.row("seed-choices", LAB),
            Some(json!({ "files": [".env", ".env.local", "deploy.pem"], "memory": true, "commits": true, "remember": true }))
        );
        // A cut of the key by name, written with a trailing slash as a folder is, drops it from the record.
        area.seed("lab", SeedWords { cut: Some(vec!["deploy.pem/".to_owned()]), save: true, ..Default::default() }, None).await.unwrap();
        assert_eq!(saved_files(&fake), json!([".env", ".env.local"]));
        // Without save, the record is left as it stands.
        let before = fake.base.store.writes.borrow().len();
        area.seed("lab", SeedWords { none: true, ..Default::default() }, None).await.unwrap();
        assert_eq!(fake.base.store.writes.borrow().len(), before);
    });
}

#[test]
fn move_by_none_then_keep_then_cut_a_folder_s_slash_named_or_not() {
    // project-seed-picks.test.ts:108.
    run(async {
        let (fake, area) = rig();
        hold_lab(&fake);
        let words = |keep: &[&str], cut: &[&str], none: bool, save: bool| SeedWords {
            keep: if keep.is_empty() { None } else { Some(keep.iter().map(|s| (*s).to_owned()).collect()) },
            cut: if cut.is_empty() { None } else { Some(cut.iter().map(|s| (*s).to_owned()).collect()) },
            none,
            save,
        };
        area.seed("lab", words(&[".env", ".env.local"], &[], false, true), None).await.unwrap();
        let picked =
            |s: &wsp_wire::views::workspace::ProjectSeed| s.rows.iter().filter(|r| r.picked).map(|r| r.path.clone()).collect::<Vec<_>>();
        assert_eq!(picked(&area.seed("lab", words(&[], &[], true, false), None).await.unwrap()), Vec::<String>::new());
        assert_eq!(picked(&area.seed("lab", words(&["node_modules/"], &[], true, false), None).await.unwrap()), vec!["node_modules"]);
        assert_eq!(picked(&area.seed("lab", words(&[], &[".env"], false, false), None).await.unwrap()), vec![".env.local"]);
        let emptied = area.seed("lab", words(&[], &[], true, true), None).await.unwrap();
        assert_eq!((emptied.picked.paths.len(), emptied.picked.bytes.0 .0, emptied.saved), (0, 0.0, true));
    });
}

#[test]
fn keep_a_saved_pick_whose_file_is_gone_on_a_save_and_name_it_until_it_is_cut() {
    // project-seed-picks.test.ts:118.
    run(async {
        let (fake, area) = rig();
        hold_lab(&fake);
        put_prior(&fake, &[".env", ".env.old"]);
        let read = area.seed("lab", SeedWords::default(), None).await.unwrap();
        assert_eq!(seed_answer(&read)["picked"], json!({ "paths": [".env"], "bytes": 1126, "missing": [".env.old"] }));
        assert!(!read.rows.iter().any(|r| r.path == ".env.old"));
        let kept = area
            .seed("lab", SeedWords { keep: Some(vec![".env.local".to_owned()]), save: true, ..Default::default() }, None)
            .await
            .unwrap();
        assert_eq!(kept.picked.missing, vec![".env.old".to_owned()]);
        assert_eq!(saved_files(&fake), json!([".env", ".env.local", ".env.old"]));
        let refused =
            area.seed("lab", SeedWords { keep: Some(vec![".env.old".to_owned()]), ..Default::default() }, None).await.unwrap_err();
        assert_eq!(
            refused,
            kinded(".env.old is not on the seed menu; git tracks it or it is not there, and the menu holds .env, .env.local, .git-credentials, node_modules, deploy.pem", Kind::Invalid)
        );
        let cut =
            area.seed("lab", SeedWords { cut: Some(vec![".env.old".to_owned()]), save: true, ..Default::default() }, None).await.unwrap();
        assert!(cut.picked.missing.is_empty());
        assert_eq!(saved_files(&fake), json!([".env", ".env.local"]));
    });
}

#[test]
fn refuse_a_path_git_tracks_or_that_is_not_there_a_login_and_git_as_usage_in_the_ad() {
    // project-seed-picks.test.ts:135: each refusal is the words' own sentence, kind invalid, and nothing is stored.
    run(async {
        let (fake, area) = rig();
        hold_lab(&fake);
        let shown = ".env, .env.local, .git-credentials, node_modules, deploy.pem";
        let not_in = |p: &str| format!("{p} is not on the seed menu; git tracks it or it is not there, and the menu holds {shown}");
        let git = |p: &str| format!("{p} is the repository's own folder, which a seed never carries: the computer clones the repo itself");
        let words = |keep: &[&str], cut: &[&str], save: bool| SeedWords {
            keep: if keep.is_empty() { None } else { Some(keep.iter().map(|s| (*s).to_owned()).collect()) },
            cut: if cut.is_empty() { None } else { Some(cut.iter().map(|s| (*s).to_owned()).collect()) },
            none: false,
            save,
        };
        for (o, line) in [
            (words(&["package.json"], &[], false), not_in("package.json")),
            (words(&[], &["src"], false), not_in("src")),
            (words(&[], &[".env.locl"], false), not_in(".env.locl")),
            (
                words(&[".git-credentials"], &[], false),
                ".git-credentials is logins, a login, and a login never travels in a seed; the computer's image carries its own sign-in"
                    .to_owned(),
            ),
            (words(&[".git"], &[], false), git(".git")),
            (words(&[".git/config"], &[], true), git(".git/config")),
        ] {
            let refused = area.seed("lab", o, None).await.unwrap_err();
            assert_eq!(refused, kinded(&line, Kind::Invalid), "{line}");
        }
        assert_eq!(fake.base.store.row("seed-choices", LAB), None);
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// add: refusals

fn setup_none(_: &FakeProjects) {}

#[test]
fn add_refuses_in_the_ts_order() {
    // spec rule 7.1-7.9: one row per refusal, each before anything is written or emitted, and a row that breaks two
    // rules at once is refused by the earlier one.
    run(async {
        type Setup = Box<dyn Fn(&FakeProjects)>;
        let usage_cloud = base::refused(
            "Solari holds no project of its own on this host. Record the project on this computer or on a computer you added.",
            "Record the project on this computer or on a computer you added.",
            Kind::Usage,
        );
        let computer_line = "that is a computer, not a project; wsp add user@host joins it, and a project is a folder here or a repo's url with --on <computer>";
        let into_repo = "--into is where a repo is cloned; a folder is a project where it already is";
        let no_place = "no place named nowhere; you have Mac, box, Solari";
        let into_here = "--into is a folder on Mac; drop --on to clone there, or drop --into to clone on the computer you named";
        let clone_into = "https://github.com/spoo-me/frontend.git is a repo; name the folder to clone it in with --into <folder>";
        let same = |name: &str, computer: &str| {
            kinded(
                &format!("that source is already a project on {computer}, {name}; one source on one computer is one project"),
                Kind::Conflict,
            )
        };
        let hold_frontend = |computer: &'static str| -> Setup {
            Box::new(move |f: &FakeProjects| {
                f.base.hold_project(project(
                    "pr_held",
                    "frontend",
                    computer,
                    json!({ "source": { "kind": "git", "url": REPO }, "path": "/home/maya/frontend" }),
                ))
            })
        };
        let cloud_off: Setup = Box::new(|f: &FakeProjects| f.cloud_projects.set(Some(false)));
        let cloud_off_too: Setup = Box::new(|f: &FakeProjects| f.cloud_projects.set(Some(false)));
        let ready_fails: Setup = Box::new(|f: &FakeProjects| f.base.answer_with("ready", vec![fails("this host is starting", None)]));
        let recording_refused: Setup =
            Box::new(|f: &FakeProjects| f.base.answer_with("refuse_recording", vec![fails("a thread records no project", Some("usage"))]));
        let cases: Vec<(&str, Setup, AddAsk, Option<Caller>, HostError)> = vec![
            ("7.1 the host is not ready", ready_fails, ask(LAB), None, plain("this host is starting")),
            ("7.1 a thread may not record", recording_refused, ask(LAB), Some(lead()), kinded("a thread records no project", Kind::Usage)),
            ("7.2 a word that is no source", Box::new(setup_none), ask("nonsense"), None, plain("wsp add takes user@host or an ssh alias for a computer of yours, a folder on this computer for a project here, or a repo with --on <computer> for a project there: its url, owner/repo on github, or gitlab.com/owner/repo")),
            ("7.2 a computer", Box::new(setup_none), ask("maya@srv"), None, plain(computer_line)),
            ("7.2 a computer beats --into", Box::new(setup_none), AddAsk { into: Some("/x".to_owned()), ..ask("maya@srv") }, None, plain(computer_line)),
            ("7.2 a folder with --into", Box::new(setup_none), AddAsk { into: Some("/x".to_owned()), ..ask(LAB) }, None, plain(into_repo)),
            ("7.2 --into beats the unknown place", Box::new(setup_none), AddAsk { into: Some("/x".to_owned()), ..on(LAB, "nowhere") }, None, plain(into_repo)),
            ("7.4 a repo with no computer names the ones that clone", Box::new(setup_none), ask(REPO), None, plain("https://github.com/spoo-me/frontend.git is a repo: clone it here with --into <folder>, or name the computer that clones it with --on box | solari")),
            ("7.4 a place nobody holds", Box::new(setup_none), on(REPO, "nowhere"), None, plain(no_place)),
            ("7.4 the unknown place beats --into", Box::new(setup_none), AddAsk { into: Some("/x".to_owned()), ..on(REPO, "nowhere") }, None, plain(no_place)),
            ("7.5 --into names this computer's folder", Box::new(setup_none), AddAsk { into: Some("/x".to_owned()), ..on(REPO, "box") }, None, plain(into_here)),
            ("7.5 --into beats the cloud refusal", cloud_off_too, AddAsk { into: Some("/x".to_owned()), ..on(REPO, "solari") }, None, plain(into_here)),
            ("7.6 the cloud holds no project", Box::new(|f: &FakeProjects| f.cloud_projects.set(Some(false))), on(REPO, "solari"), None, usage_cloud.clone()),
            ("7.6 the cloud refusal beats the folder rule", cloud_off, on(LAB, "solari"), None, usage_cloud),
            ("7.7 a folder on a provider", Box::new(setup_none), on(LAB, "solari"), None, plain("solari takes a repo it can clone, not a folder on this computer; give the repo's url, or add the folder here with no --on")),
            ("7.7 a folder on a joined computer (the TS reads the kind without the join)", Box::new(setup_none), on(LAB, "pl_box"), None, plain("box takes a repo it can clone, not a folder on this computer; give the repo's url, or add the folder here with no --on")),
            ("7.8 a repo on this computer with no --into", Box::new(setup_none), on(REPO, "here"), None, plain(clone_into)),
            ("7.8 the clone folder beats the same-source rule", hold_frontend("here"), on(REPO, "here"), None, plain(clone_into)),
            ("7.9 the same source on the same computer", hold_frontend("pl_box"), on(REPO, "box"), None, same("frontend", "box")),
        ];
        for (name, setup, ask, origin, want) in cases {
            let (fake, area) = rig();
            setup(&fake);
            let before = fake.base.projects.borrow().len();
            let err = area.add(ask, origin).await.unwrap_err();
            assert_eq!(err, want, "{name}");
            assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new(), "{name}");
            assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new(), "{name}");
            assert_eq!(fake.base.projects.borrow().len(), before, "{name}");
            assert_eq!(
                fake.base.names_of(&[
                    "remember_project",
                    "road_land",
                    "clone_here",
                    "seed_wiring_pack",
                    "source_resolve",
                    "landing_place",
                    "git_top_of"
                ]),
                Vec::<String>::new(),
                "{name}"
            );
        }
        // The host not ready asks nothing else; a refused recording asks for no row.
        let (fake, area) = rig();
        fake.base.answer_with("ready", vec![fails("this host is starting", None)]);
        let _ = area.add(ask(LAB), None).await;
        assert_eq!(fake.base.names(), vec!["ready"]);
        let (fake, area) = rig();
        fake.base.answer_with("refuse_recording", vec![fails("a thread records no project", Some("usage"))]);
        let _ = area.add(ask(LAB), Some(lead())).await;
        assert_eq!(fake.base.args_of("refuse_recording"), vec![json!({ "word": LAB, "scoped": true })]);
        assert_eq!(fake.base.names_of(&["computer_rows"]), Vec::<String>::new());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// add: a folder or a clone here

#[test]
fn add_clones_a_repo_here_into_the_named_folder_and_records_the_folder() {
    // spec rule 7.8, projects.test.ts:148 and :169: a repo with --into on this computer is cloned there, then the folder
    // is added again with the rest of the ask (name, base, createdAt) and neither --into nor --on.
    run(async {
        let (fake, area) = rig();
        let (lines, report) = heard();
        let cloned = "/home/me/Downloads/frontend";
        fake.base.answer_with("clone_here", vec![json!(cloned)]);
        let given = AddAsk {
            into: Some("/home/me/Downloads".to_owned()),
            name: Some("spoo".to_owned()),
            base: Some("release".to_owned()),
            created_at: Some("2026-01-02T03:04:05.000Z".to_owned()),
            report: Some(report),
            ..ask(REPO)
        };
        let got = area.add(given, None).await.unwrap();
        assert_eq!(
            fake.base.args_of("clone_here"),
            vec![json!({ "word": REPO, "source": { "kind": "git", "url": REPO }, "into": "/home/me/Downloads" })]
        );
        let want = json!({
            "id": "pr_11111111", "name": "spoo", "computer": "here", "source": { "kind": "folder", "path": cloned }, "path": cloned,
            "remote": "git@github.com:acme/frontend.git", "defaultBranch": "main", "memoryKey": "-home-me-Downloads-frontend",
            "memoryDir": "/home/me/.claude/projects/-home-me-Downloads-frontend/memory", "base": "release", "createdAt": "2026-01-02T03:04:05.000Z",
        });
        assert_eq!(pj(&got.project), want);
        assert_eq!(got.notice, None);
        assert_eq!(fake.base.store.row("projects", "pr_11111111"), Some(want.clone()));
        assert_eq!(fake.base.store.writes.borrow().clone(), vec!["put projects/pr_11111111"]);
        assert_eq!(fake.base.events.borrow().clone(), vec![json!({ "type": "project.added", "project": want })]);
        assert_eq!(fake.base.names_of(&["clone_here", "remember_project"]), vec!["clone_here", "remember_project"]);
        // The second add is the folder's: its own recording check, and no landing on this computer.
        let words: Vec<Value> = fake.base.args_of("refuse_recording").iter().map(|a| a["word"].clone()).collect();
        assert_eq!(words, vec![json!(REPO), json!(cloned)]);
        assert_eq!(fake.base.names_of(&["ready"]).len(), 2);
        assert!(lines.borrow().is_empty());
        assert_eq!(fake.base.names_of(&["road_land"]), Vec::<String>::new());
    });
}

#[test]
fn says_git_s_own_last_line_when_the_clone_fails_with_how_to_sign_in_where_the_fail() {
    // projects.test.ts:237: the clone's own refusal reaches the caller and nothing is recorded.
    run(async {
        let (fake, area) = rig();
        let said = "fatal: repository 'https://github.com/me/private.git' not found; sign in with gh auth login, or use the repo's ssh url";
        fake.base.answer_with("clone_here", vec![fails(said, None)]);
        let err = area
            .add(AddAsk { into: Some("/home/me/gone/code".to_owned()), ..ask("https://github.com/me/private.git") }, None)
            .await
            .unwrap_err();
        assert_eq!(err, plain(said));
        assert_eq!(fake.base.args_of("clone_here").len(), 1);
        nothing_recorded_after_clone(&fake);
    });
}

fn nothing_recorded_after_clone(fake: &FakeProjects) {
    assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
    assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
    assert!(fake.base.projects.borrow().is_empty());
}

#[test]
fn add_records_a_folder_here_with_its_git_top_and_no_landing() {
    // spec rules 7.10-7.15, projects.test.ts:75 and :96: a folder under a repo keeps the repo's top beside its path; the
    // road that copies the folder lands nothing, so no stage is said, the record is kept at once.
    run(async {
        let (fake, area) = rig();
        let folder = "/home/me/code/lab/packages/host";
        fake.base.answer_with("git_top_of", vec![json!(LAB)]);
        let (lines, report) = heard();
        let got = area.add(AddAsk { report: Some(report), ..ask(folder) }, None).await.unwrap();
        let want = json!({
            "id": "pr_11111111", "name": "host", "computer": "here", "source": { "kind": "folder", "path": folder }, "path": folder,
            "remote": "git@github.com:acme/host.git", "defaultBranch": "main", "memoryKey": "-home-me-code-lab-packages-host",
            "memoryDir": "/home/me/.claude/projects/-home-me-code-lab-packages-host/memory", "git": { "top": LAB }, "createdAt": CREATED,
        });
        assert_eq!(pj(&got.project), want);
        assert_eq!(fake.base.args_of("git_top_of"), vec![json!({ "path": folder })]);
        assert_eq!(fake.base.args_of("road_lands_at_add"), vec![json!({ "kind": "mac", "seeding": false })]);
        assert_eq!(fake.base.args_of("source_resolve"), vec![json!({ "source": { "kind": "folder", "path": folder }, "seeding": false })]);
        assert_eq!(fake.base.store.row("projects", "pr_11111111"), Some(want.clone()));
        assert_eq!(fake.base.events.borrow().clone(), vec![json!({ "type": "project.added", "project": want })]);
        assert_eq!(add_stages(&fake.base), Vec::<(String, String)>::new());
        assert!(lines.borrow().is_empty());
        assert_eq!(fake.base.names_of(&["road_land", "seed_wiring_pack", "seed_wiring_plan", "landing_backend"]), Vec::<String>::new());
        // The project is held for the next read.
        assert_eq!(fake.base.projects.borrow().keys().cloned().collect::<Vec<_>>(), vec!["pr_11111111"]);
    });
}

#[test]
fn a_folder_that_is_no_git_repo_is_a_project_with_no_branches() {
    // projects.test.ts:87.
    run(async {
        let (fake, area) = rig();
        let got = area.add(ask("/home/me/plain"), None).await.unwrap();
        assert_eq!(got.project.path, "/home/me/plain");
        assert_eq!(got.project.git, None);
        assert!(pj(&got.project).get("git").is_none());
        assert_eq!(fake.base.args_of("git_top_of"), vec![json!({ "path": "/home/me/plain" })]);
    });
}

#[test]
fn add_writes_the_project_keys_in_the_ts_order() {
    // spec rule 7.12: id, name, computer, source, path, remote, defaultBranch, memoryKey, memoryDir, base?, git?,
    // createdAt; the id and the age are named only by a recipe folder moving a project, else drawn and read off the clock.
    run(async {
        let (fake, area) = rig();
        fake.base.answer_with("git_top_of", vec![json!(LAB)]);
        let named = AddAsk {
            base: Some("release".to_owned()),
            id: Some("pr_fixed".to_owned()),
            created_at: Some("2026-01-02T03:04:05.000Z".to_owned()),
            ..ask(LAB)
        };
        let got = area.add(named, None).await.unwrap();
        let order =
            ["id", "name", "computer", "source", "path", "remote", "defaultBranch", "memoryKey", "memoryDir", "base", "git", "createdAt"];
        assert_eq!(keys(&pj(&got.project)), order);
        assert_eq!(keys(&fake.base.store.row("projects", "pr_fixed").unwrap()), order);
        assert_eq!(keys(&fake.base.events.borrow()[0]["project"]), order);
        assert_eq!((got.project.id.as_str(), got.project.created_at.as_str()), ("pr_fixed", "2026-01-02T03:04:05.000Z"));
        // The id was not drawn: the next add takes the first draw, and its age is the clock's, which moved on.
        fake.base.clock.advance(61_000);
        fake.base.script.borrow_mut().shift_remove("git_top_of");
        let next = area.add(ask("/home/me/other"), None).await.unwrap();
        assert_eq!((next.project.id.as_str(), next.project.created_at.as_str()), ("pr_11111111", "1970-01-01T00:17:41.000Z"));
        assert_eq!(
            keys(&pj(&next.project)),
            ["id", "name", "computer", "source", "path", "remote", "defaultBranch", "memoryKey", "memoryDir", "createdAt"]
        );
        assert_eq!(fake.base.store.writes.borrow().clone(), vec!["put projects/pr_fixed", "put projects/pr_11111111"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// add: repos on a computer that clones

#[test]
fn a_repo_s_url_on_a_computer_that_clones_records_the_clone_s_own_path_inside_a_cop() {
    // projects.test.ts:111: the provider's road path, a record of its own, and the same source again refused by name.
    run(async {
        let (fake, area) = rig();
        let got = area.add(AddAsk { name: Some("spoo-landing".to_owned()), ..on(REPO, "solari") }, None).await.unwrap();
        assert_eq!(
            (got.project.computer.as_str(), got.project.name.as_str(), got.project.path.as_str()),
            ("solari", "spoo-landing", "/root/spoo-landing")
        );
        assert_eq!(pj(&got.project)["source"], json!({ "kind": "git", "url": REPO }));
        assert_eq!(got.project.remote, REPO);
        // A repo the computer clones by itself is recorded here: nothing lands at the add, nothing is said.
        assert_eq!(fake.base.args_of("road_lands_at_add"), vec![json!({ "kind": "provider", "seeding": false })]);
        assert_eq!(fake.base.names_of(&["road_land"]), Vec::<String>::new());
        assert_eq!(add_stages(&fake.base), Vec::<(String, String)>::new());
        let again = area.add(on(REPO, "solari"), None).await.unwrap_err();
        assert_eq!(
            again,
            kinded("that source is already a project on solari, spoo-landing; one source on one computer is one project", Kind::Conflict)
        );
        assert_eq!(fake.base.store.writes.borrow().clone(), vec!["put projects/pr_11111111"]);
    });
}

#[test]
fn is_cloned_through_the_host_s_own_command_line_which_the_computer_s_image_is_chec() {
    // project-landing.test.ts:127, the add half: an owner/repo is a github source with its https remote, and nothing runs
    // on the computer at the add.
    run(async {
        let (fake, area) = rig();
        let got = area.add(on("spoo-me/frontend", "solari"), None).await.unwrap();
        assert_eq!(pj(&got.project)["source"], json!({ "kind": "github", "repo": "spoo-me/frontend" }));
        assert_eq!(got.project.remote, "https://github.com/spoo-me/frontend.git");
        assert_eq!(got.project.name, "frontend");
        assert_eq!(fake.base.names_of(&["road_land", "clone_here"]), Vec::<String>::new());
    });
}

#[test]
fn a_gitlab_repo_is_named_with_its_host_and_cloned_through_glab() {
    // project-landing.test.ts:144, the add half.
    run(async {
        let (fake, area) = rig();
        let got = area.add(on("gitlab.com/dev/thing", "solari"), None).await.unwrap();
        assert_eq!(pj(&got.project)["source"], json!({ "kind": "gitlab", "repo": "dev/thing" }));
        assert_eq!(got.project.name, "thing");
        assert_eq!(fake.base.names_of(&["road_land"]), Vec::<String>::new());
    });
}

#[test]
fn is_cloned_by_the_add_into_the_home_of_the_login_that_computer_was_joined_with_as() {
    // places-folders.test.ts:20, the add half: the joined computer's road lands the clone in its login's home; the
    // record keeps no checkout of its own; the stages are said on the computer's name.
    run(async {
        let (fake, area) = rig();
        fake.joined.borrow_mut().insert("pl_box".to_owned(), projects::joined("/home/maya"));
        fake.base.answer_with(
            "road_land_reports",
            vec![json!([["cloning", "cloning https://github.com/spoo-me/spoo-ts into /home/maya/landing-906", 30]])],
        );
        let spoo = "https://github.com/spoo-me/spoo-ts";
        let got = area.add(AddAsk { name: Some("landing-906".to_owned()), ..on(spoo, "box") }, None).await.unwrap();
        assert_eq!(got.project.path, "/home/maya/landing-906");
        assert_eq!(got.project.checkout, None);
        assert_eq!(got.project.computer, "pl_box");
        let landing = &fake.base.args_of("road_land")[0];
        assert_eq!(landing["deps"]["joinedHome"], "/home/maya");
        assert_eq!(landing["deps"]["computerName"], "box");
        assert_eq!(landing["seed"], Value::Null);
        let stages = add_stages(&fake.base);
        assert_eq!(
            stages,
            vec![
                ("planned".to_owned(), "landing-906 from https://github.com/spoo-me/spoo-ts, nothing seeded.".to_owned()),
                ("cloning".to_owned(), "cloning https://github.com/spoo-me/spoo-ts into /home/maya/landing-906".to_owned()),
                (
                    "done".to_owned(),
                    "landing-906 pr_11111111: https://github.com/spoo-me/spoo-ts on box, at /home/maya/landing-906".to_owned()
                ),
            ]
        );
        assert_eq!(fake.base.events.borrow()[0]["computer"], "pl_box");
    });
}

#[test]
fn is_refused_in_that_computer_s_own_absent_sentence_while_it_is_not_connected_with() {
    // places-folders.test.ts:38: no backend and a place named: the absent sentence, before anything is made or recorded.
    run(async {
        let (fake, area) = rig();
        let absent = "srv is not answering; it connects on its own when it is on";
        fake.base.answer_with("forking_at", vec![fails("no backend for pl_box", None)]);
        fake.base.answer_with("landing_backend", vec![fails(absent, None)]);
        let err = area
            .add(AddAsk { name: Some("landing-906".to_owned()), ..on("https://github.com/spoo-me/spoo-ts", "box") }, None)
            .await
            .unwrap_err();
        assert_eq!(err, plain(absent));
        assert_eq!(fake.base.args_of("landing_backend"), vec![json!({ "placeId": "pl_box" })]);
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
        assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
        assert_eq!(fake.base.names_of(&["road_land", "remember_project", "source_resolve"]), Vec::<String>::new());
        assert!(fake.base.projects.borrow().is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// add: seeding a computer that clones

fn ready_to_seed(fake: &FakeProjects) {
    fake.base.answer_with("road_land", vec![json!({ "fields": { "checkout": "/root/lab", "seeded": { "files": 1, "bytes": 12, "memory": "landed", "commits": 0, "at": CREATED } } })]);
}

#[test]
fn add_needs_a_seed_choice_to_seed_a_computer_that_clones() {
    // spec rule 7.11: nothing of the person's folder leaves this computer unasked.
    run(async {
        let (fake, area) = seeding_rig();
        let err = area.add(ask(LAB), None).await.unwrap_err();
        let said = "/home/me/code/lab would seed the project on that computer, so what travels is yours to pick: read the menu and add it again with your ticks, or --yes for the ones the catalogue ticks itself";
        assert_eq!(err, kinded(said, Kind::Invalid));
        assert_eq!(fake.base.args_of("source_resolve"), vec![json!({ "source": { "kind": "folder", "path": LAB }, "seeding": true })]);
        assert_eq!(fake.base.args_of("seed_wiring_plan").len(), 1);
        nothing_recorded_seeding(&fake);
        // The road that copies the folder here reads no menu and needs no choice.
        let (fake, area) = rig();
        area.add(ask(LAB), None).await.unwrap();
        assert_eq!(fake.base.args_of("seed_wiring_plan"), Vec::<Value>::new());
    });
}

fn nothing_recorded_seeding(fake: &FakeProjects) {
    assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
    assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
    assert_eq!(fake.base.names_of(&["remember_project", "road_land", "seed_wiring_pack"]), Vec::<String>::new());
}

fn recorded_lab() -> Value {
    json!({
        "id": "pr_11111111", "name": "lab", "computer": "here", "source": { "kind": "folder", "path": LAB }, "path": "/root/lab",
        "remote": "https://github.com/acme/lab.git", "defaultBranch": "main", "memoryKey": "-home-me-code-lab",
        "memoryDir": "/home/me/.claude/projects/-home-me-code-lab/memory", "checkout": "/root/lab",
        "seeded": { "files": 1, "bytes": 12, "memory": "landed", "commits": 0, "at": CREATED }, "createdAt": CREATED,
    })
}

const DONE_LAB: &str = "lab pr_11111111: /home/me/code/lab on Mac, at /root/lab\nopen a thread on it with: wsp run 'lab' \"<message>\"";

#[test]
fn add_lands_a_seed_reporting_each_stage_and_says_done_once() {
    // spec rules 7.13-7.16: the planned line, the pack, the road's own stages, the record kept and announced, then done
    // once; every line also reaches the caller's own log; elapsedMs is read off the clock.
    run(async {
        let (fake, area) = seeding_rig();
        ready_to_seed(&fake);
        fake.base.answer_with(
            "road_land_reports",
            vec![json!([["cloning", "cloning https://github.com/acme/lab.git into /root/lab", 250], ["seeding", "1 file seeded", 500]])],
        );
        let (lines, report) = heard();
        let picked = choice(&[".env.local"], true, None);
        let got = area.add(AddAsk { seed: Some(picked.clone()), report: Some(report), ..ask(LAB) }, None).await.unwrap();
        let planned = "lab from /home/me/code/lab.";
        let cloning = "cloning https://github.com/acme/lab.git into /root/lab";
        assert_eq!(
            fake.base.events.borrow().clone(),
            vec![
                stage("planned", planned, 0),
                stage("cloning", cloning, 250),
                stage("seeding", "1 file seeded", 750),
                json!({ "type": "project.added", "project": recorded_lab() }),
                stage("done", DONE_LAB, 750),
            ]
        );
        assert_eq!(keys(&fake.base.events.borrow()[0]), ["type", "projectId", "computer", "stage", "message", "elapsedMs"]);
        assert_eq!(*lines.borrow(), vec![planned.to_owned(), cloning.to_owned(), "1 file seeded".to_owned(), DONE_LAB.to_owned()]);
        assert_eq!(pj(&got.project), recorded_lab());
        assert_eq!(got.notice, None);
        assert_eq!(fake.base.store.row("projects", "pr_11111111"), Some(recorded_lab()));
        assert_eq!(fake.base.store.writes.borrow().clone(), vec!["put projects/pr_11111111"]);
        assert_eq!(
            fake.base.names_of(&["emit", "seed_wiring_pack", "road_land", "remember_project"]),
            vec!["emit", "seed_wiring_pack", "road_land", "emit", "emit", "remember_project", "emit", "emit"]
        );
        // The pack and the road are handed one menu (ticked as the add read it) and the person's choice.
        let packed = &fake.base.args_of("seed_wiring_pack")[0];
        let mut menu_here = lab_menu();
        for (i, t) in [true, true, false, false, true].into_iter().enumerate() {
            menu_here["files"][i]["ticked"] = json!(t);
        }
        assert_eq!(packed["plan"], menu_here);
        assert_eq!(packed["choice"], serde_json::to_value(&picked).unwrap());
        assert_eq!(packed["homes"], json!({ "claude": "/home/me/.claude" }));
        let landing = &fake.base.args_of("road_land")[0];
        assert_eq!(landing["seed"], json!({ "tar": "seed", "choice": serde_json::to_value(&picked).unwrap(), "plan": menu_here }));
        assert_eq!(landing["project"]["id"], "pr_11111111");
        assert_eq!(landing["project"].get("checkout"), None);
        assert_eq!(fake.base.args_of("road_lands_at_add"), vec![json!({ "kind": "box", "seeding": true })]);
    });
}

#[test]
fn add_says_logins_left_behind_as_a_stage_and_a_notice() {
    // spec rule 7.16: a login found inside a folder they ticked stays here; it is said as a seeding stage before the
    // landing and again in the answer's notice, after the landed notice, joined by "; ".
    run(async {
        let left = "2 files inside the folders you ticked stayed on this computer: .git-credentials, deploy/.netrc";
        let (fake, area) = seeding_rig();
        ready_to_seed(&fake);
        fake.base.answer_with(
            "seed_wiring_pack",
            vec![json!({ "tar": "seed", "files": 1, "bytes": 12, "commits": 0, "left": [".git-credentials", "deploy/.netrc"] })],
        );
        let got = area.add(AddAsk { seed: Some(choice(&[".env.local"], false, None)), ..ask(LAB) }, None).await.unwrap();
        assert_eq!(got.notice.as_deref(), Some(left));
        assert_eq!(add_stages(&fake.base).iter().map(|s| s.0.clone()).collect::<Vec<_>>(), vec!["planned", "seeding", "done"]);
        assert_eq!(add_stages(&fake.base)[1].1, left);
        assert_eq!(
            fake.base.names_of(&["emit", "seed_wiring_pack", "road_land"]),
            vec!["emit", "seed_wiring_pack", "emit", "road_land", "emit", "emit"]
        );
        // The notice never reaches the record: it is a field of the answer alone.
        assert_eq!(pj(&got.project), recorded_lab());
        assert_eq!(fake.base.store.row("projects", "pr_11111111"), Some(recorded_lab()));

        // The landed notice comes first.
        let (fake, area) = seeding_rig();
        fake.base.answer_with(
            "seed_wiring_pack",
            vec![json!({ "tar": "seed", "files": 1, "bytes": 12, "commits": 2, "left": [".git-credentials", "deploy/.netrc"] })],
        );
        fake.base.answer_with("road_land", vec![json!({ "fields": { "checkout": "/root/lab" }, "notice": "2 commits stayed on this computer: the remote's git refused them" })]);
        let got = area.add(AddAsk { seed: Some(choice(&[".env.local"], false, None)), ..ask(LAB) }, None).await.unwrap();
        assert_eq!(
            got.notice.as_deref(),
            Some(format!("2 commits stayed on this computer: the remote's git refused them; {left}").as_str())
        );
        assert_eq!(got.project.checkout.as_deref(), Some("/root/lab"));

        // Only the landed notice, and no notice at all.
        let (fake, area) = seeding_rig();
        fake.base.answer_with("road_land", vec![json!({ "fields": {}, "notice": "the remote's git refused the commits" })]);
        let got = area.add(AddAsk { seed: Some(choice(&[], false, None)), ..ask(LAB) }, None).await.unwrap();
        assert_eq!(got.notice.as_deref(), Some("the remote's git refused the commits"));
        let (fake, area) = seeding_rig();
        let got = area.add(AddAsk { seed: Some(choice(&[], false, None)), ..ask(LAB) }, None).await.unwrap();
        assert_eq!(got.notice, None);
        assert_eq!(add_stages(&fake.base).iter().map(|s| s.0.clone()).collect::<Vec<_>>(), vec!["planned", "done"]);
    });
}

#[test]
fn a_failed_landing_reports_failed_and_records_nothing() {
    // spec rule 7.16: a failure anywhere after planned reports failed with its message and propagates; and a failure of
    // the keep after the landing reads failed too (kept as the TS has it).
    run(async {
        let seeded = || AddAsk { seed: Some(choice(&[".env.local"], false, None)), ..ask(LAB) };

        let (fake, area) = seeding_rig();
        let said = "git clone failed: fatal: repository 'https://github.com/acme/lab.git' not found";
        fake.base.answer_with("road_land", vec![fails(said, None)]);
        let (lines, report) = heard();
        let err = area.add(AddAsk { report: Some(report), ..seeded() }, None).await.unwrap_err();
        assert_eq!(err, plain(said));
        assert_eq!(fake.base.events.borrow().clone(), vec![stage("planned", "lab from /home/me/code/lab.", 0), stage("failed", said, 0)]);
        assert_eq!(*lines.borrow(), vec!["lab from /home/me/code/lab.".to_owned(), said.to_owned()]);
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
        assert!(fake.base.projects.borrow().is_empty());
        assert_eq!(fake.base.names_of(&["remember_project"]), Vec::<String>::new());

        // The pack fails: the road is never asked.
        let (fake, area) = seeding_rig();
        fake.base.answer_with("seed_wiring_pack", vec![fails("could not read /home/me/code/lab/.env.local: permission denied", None)]);
        let err = area.add(seeded(), None).await.unwrap_err();
        assert_eq!(err.message, "could not read /home/me/code/lab/.env.local: permission denied");
        assert_eq!(
            add_stages(&fake.base),
            vec![("planned".to_owned(), "lab from /home/me/code/lab.".to_owned()), ("failed".to_owned(), err.message.clone())]
        );
        assert_eq!(fake.base.names_of(&["road_land", "remember_project"]), Vec::<String>::new());

        // The record cannot be kept after the computer holds the project: failed is said, nothing is stored or announced.
        let (fake, area) = seeding_rig();
        ready_to_seed(&fake);
        fake.base.answer_with("remember_project", vec![fails("disk full", None)]);
        let err = area.add(seeded(), None).await.unwrap_err();
        assert_eq!(err, plain("disk full"));
        assert_eq!(add_stages(&fake.base).iter().map(|s| s.0.clone()).collect::<Vec<_>>(), vec!["planned", "failed"]);
        assert_eq!(fake.base.events.borrow().iter().filter(|e| e["type"] == "project.added").count(), 0);
        assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
    });
}

fn provider_menu() -> Value {
    let mut plan = lab_menu();
    plan["provider"] = json!(true);
    plan
}

#[test]
fn add_remembers_a_seed_choice_with_held_back_files() {
    // spec rule 7.14: with remember and a folder source the choice is stored under the folder, with the files of values a
    // provider never gets held back from the prior choice, each once.
    run(async {
        let resolved = || json!({ "name": "lab", "remote": "https://github.com/acme/lab.git", "defaultBranch": "main", "seed": provider_menu(), "seedMemoryKey": null });
        let (fake, area) = seeding_rig();
        fake.base.answer_with("source_resolve", vec![resolved()]);
        fake.base.store.seed(
            "seed-choices",
            LAB,
            json!({ "files": [".env", "deploy.pem"], "memory": false, "commits": false, "remember": true }),
        );
        area.add(AddAsk { seed: Some(choice(&[".env.local"], true, Some(true))), ..ask(LAB) }, None).await.unwrap();
        assert_eq!(
            fake.base.store.row("seed-choices", LAB),
            Some(json!({ "files": [".env.local", "deploy.pem"], "memory": true, "commits": false, "remember": true }))
        );
        assert_eq!(fake.base.store.writes.borrow().clone(), vec!["put projects/pr_11111111".to_owned(), format!("put seed-choices/{LAB}")]);
        let packed = &fake.base.args_of("seed_wiring_pack")[0];
        assert_eq!(packed["plan"], provider_menu());
        assert_eq!(fake.base.events.borrow().iter().filter(|e| e["type"] == "project.added").count(), 1);

        // A held-back file the choice already names is stored once.
        let (fake, area) = seeding_rig();
        fake.base.answer_with("source_resolve", vec![resolved()]);
        fake.base.store.seed(
            "seed-choices",
            LAB,
            json!({ "files": [".env", "deploy.pem"], "memory": false, "commits": false, "remember": true }),
        );
        area.add(AddAsk { seed: Some(choice(&[".env", "deploy.pem"], false, Some(true))), ..ask(LAB) }, None).await.unwrap();
        assert_eq!(fake.base.store.row("seed-choices", LAB).unwrap()["files"], json!([".env", "deploy.pem"]));

        // No remember: the folder's record is left as it stands.
        let (fake, area) = seeding_rig();
        fake.base.answer_with("source_resolve", vec![resolved()]);
        area.add(AddAsk { seed: Some(choice(&[".env.local"], false, None)), ..ask(LAB) }, None).await.unwrap();
        assert_eq!(fake.base.store.writes.borrow().clone(), vec!["put projects/pr_11111111"]);
    });
}

#[test]
fn a_repo_add_with_remember_stores_no_seed_choice() {
    // spec edge case: `o.seed.remember` on a repo source stores nothing, and nothing is packed.
    run(async {
        let (fake, area) = rig();
        area.add(AddAsk { seed: Some(choice(&[".env"], false, Some(true))), ..on(REPO, "box") }, None).await.unwrap();
        assert_eq!(fake.base.store.writes.borrow().clone(), vec!["put projects/pr_11111111"]);
        assert_eq!(fake.base.names_of(&["seed_wiring_pack"]), Vec::<String>::new());
        assert_eq!(
            add_stages(&fake.base).iter().map(|s| s.1.clone()).collect::<Vec<_>>()[0],
            "frontend from https://github.com/spoo-me/frontend.git, nothing seeded."
        );
    });
}
