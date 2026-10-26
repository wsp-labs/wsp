// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the projects door and the add: the order of what `seedInto` and `seedPlan` ask, and what `add` stores for
//! a remembered seed choice. Expected values are printed by oracle/zz-review-projects-door.oracle.ts (the TS door on a
//! scripted context) and read off projects.ts:394-395, :170 and :306.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/projects.rs"]
mod projects;

use std::rc::Rc;

use base::{plain, project, run};
use projects::{menu, pj, FakeProjects};
use serde_json::{json, Value};
use wsp_account::projects::projects::add::AddAsk;
use wsp_account::projects::projects::Projects;
use wsp_wire::views::workspace::SeedChoice;
use wsp_wire::views::workspace_kind::WorkspaceKind;

const LAB: &str = "/home/me/code/lab";

fn rig() -> (Rc<FakeProjects>, Rc<Projects>) {
    let fake = FakeProjects::new();
    let area = fake.area();
    (fake, area)
}

fn hold_box(fake: &FakeProjects) {
    let web = project("pr_box", "web", "pl_box", json!({ "createdAt": "2026-10-01T00:00:00.000Z" }));
    fake.base.hold_project(web.clone());
    fake.base.store.seed("projects", &web.id, pj(&web));
}

// R-PRJ-D1. A road with no `seedInto` refuses before it reads anything of the person's: projects.ts:394 throws, and
// :395 packs the folder only after. Oracle: `SEEDINTO cloud ... []` (no pack, no homes read). The Rust packs the
// ticked files first and asks the road after (door.rs:166-168), so a provider's project costs a read of the folder.
#[test]
fn a_road_with_no_folder_refuses_before_the_files_are_packed() {
    run(async {
        let (fake, area) = rig();
        hold_box(&fake);
        let plan = projects::menu("/home/me/web");
        let refusal = area.seed_into("pr_box", plan, vec![".env".to_owned()]).await.unwrap_err();
        assert_eq!(refusal, plain("web keeps no folder of its own on box to copy files into"));
        assert_eq!(fake.base.args_of("seed_wiring_pack").len(), 0, "the folder was packed for a road that cannot take it");
        assert_eq!(fake.base.args_of("homes_here").len(), 0);
    });
}

// R-PRJ-D2. With no seed reader wired the TS says so first: `seedWiring().pack({..., homes: await ctx.homesHere()})`
// evaluates the callee before the argument, so `homesHere` is never called (oracle: `SEEDINTO place seedWired: false []`).
// The Rust reads the homes first (door.rs:166), so a failing or slow `homesHere` is what a host with no reader answers.
#[test]
fn seed_into_on_a_host_with_no_seed_reader_says_so_before_it_reads_the_homes() {
    run(async {
        let (fake, area) = rig();
        hold_box(&fake);
        *fake.seed.borrow_mut() = None;
        fake.base.answer_with("road_seed_into:box", vec![json!(null)]);
        let refusal = area.seed_into("pr_box", projects::menu("/home/me/web"), vec![".env".to_owned()]).await.unwrap_err();
        assert_eq!(refusal.message, NO_READER);
        assert_eq!(fake.base.args_of("homes_here").len(), 0, "seed_into read the homes first");
    });
}

// R-PRJ-D3. The same order in the menu: `seedWiring().plan(folder, await ctx.homesHere())` (projects.ts:170; oracle:
// `SEEDPLAN seedWired: false []`). The Rust reads the homes first (add.rs:284).
#[test]
fn seed_plan_on_a_host_with_no_seed_reader_says_so_before_it_reads_the_homes() {
    run(async {
        let (fake, area) = rig();
        *fake.seed.borrow_mut() = None;
        let refusal = area.seed_plan(LAB, None).await.unwrap_err();
        assert_eq!(refusal.message, NO_READER);
        assert_eq!(fake.base.args_of("homes_here").len(), 0, "seed_plan read the homes first");
    });
}

const NO_READER: &str = "this runtime cannot read a folder on this computer, so a folder cannot seed a project elsewhere; the host that serves the app wires the reader";

fn provider_menu() -> Value {
    let mut plan = serde_json::to_value(menu(LAB)).unwrap();
    plan["provider"] = json!(true);
    plan
}

// R-PRJ-D4. The remembered choice is `{...o.seed, files: [...new Set([...o.seed.files, ...held])]}` (projects.ts:306),
// so a file the choice names twice is stored once (oracle: `DEDUP ["a","b","c"]` for the same expression). The Rust
// copies the choice's own list and dedups only the held files (add.rs:780-785).
#[test]
fn a_remembered_choice_stores_each_file_once_even_when_it_names_one_twice() {
    run(async {
        let (fake, area) = rig();
        fake.base.kinds.borrow_mut().insert("here".to_owned(), WorkspaceKind::Place);
        fake.base.answer_with(
            "source_resolve",
            vec![json!({ "name": "lab", "remote": "https://github.com/acme/lab.git", "defaultBranch": "main", "seed": provider_menu(), "seedMemoryKey": null })],
        );
        let twice = SeedChoice { files: vec![".env".to_owned(), ".env".to_owned()], memory: false, commits: false, remember: Some(true) };
        area.add(AddAsk { source: LAB.to_owned(), seed: Some(twice), ..Default::default() }, None).await.unwrap();
        assert_eq!(fake.base.store.row("seed-choices", LAB).unwrap()["files"], json!([".env"]));
    });
}
