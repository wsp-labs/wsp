// SPDX-License-Identifier: AGPL-3.0-only
//! Pins src/project_sources.rs against the TS's own answers in tests/fixtures/core_rest/project_sources.json, written
//! by the "project sources" case of oracle/core_rest.oracle.ts: the kinds in order, the refusal for an unknown kind,
//! the default branch, what each source names as its remote, what each resolves to, the clone line and the command
//! each kind clones through.
mod rest;

use std::rc::Rc;

use futures::FutureExt;
use rest::*;
use serde_json::{json, Value};
use wsp_core::project_sources::{project_remote, project_source, project_sources, FolderRemote, ResolvedSource, SourceDeps, DEFAULT_BRANCH};
use wsp_wire::views::workspace::{ProjectSource, SeedPlan};
use wsp_words::exit::{HostError, Kind};

fn rows<'a>(v: &'a Value, key: &str) -> &'a Vec<Value> {
    v[key].as_array().unwrap_or_else(|| panic!("{key} is not a list"))
}

fn named<'a>(list: &'a [Value], name: &str) -> &'a Value {
    &list.iter().find(|r| r[0] == name).unwrap_or_else(|| panic!("no fixture row named {name}"))[1]
}

fn plan(over: Value) -> Value {
    let mut base = json!({
        "source": "/Users/dev/web",
        "remote": "git@github.com:acme/web.git",
        "branch": "feat",
        "defaultBranch": "main",
        "unpushed": null,
        "uncommitted": 0,
        "memory": null,
        "files": [],
        "remembered": false,
    });
    for (k, v) in over.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

fn deps(seeding: bool, plan: Value, remote: &str, default_branch: &str) -> SourceDeps {
    let plan: SeedPlan = wire(&plan);
    let folder = FolderRemote { remote: remote.into(), default_branch: default_branch.into() };
    SourceDeps {
        seeding,
        seed_plan: Rc::new(move |_| {
            let p = plan.clone();
            async move { Ok(p) }.boxed_local()
        }),
        folder_remote: Rc::new(move |_| {
            let f = folder.clone();
            async move { Ok(f) }.boxed_local()
        }),
    }
}

fn folder_source() -> ProjectSource {
    wire(&json!({ "kind": "folder", "path": "/Users/dev/web" }))
}

/// The oracle's answer as the Rust side states it: a seed the TS left `defaultBranch` out of reads as null here, the
/// only way `SeedPlan` holds it.
fn seed_as_held(seed: &Value) -> Value {
    let mut s = seed.clone();
    if s.get("defaultBranch").is_none() {
        s["defaultBranch"] = Value::Null;
    }
    s
}

fn resolved_json(r: &ResolvedSource) -> Value {
    let mut o = json!({ "remote": r.remote, "defaultBranch": r.default_branch, "name": r.name });
    if let Some(seed) = &r.seed {
        o["seed"] = json(seed);
    }
    o
}

fn expected_ok(row: &Value) -> Value {
    let mut ok = row["ok"].clone();
    if let Some(seed) = ok.get("seed") {
        ok["seed"] = seed_as_held(seed);
    }
    ok
}

async fn resolve(kind: &str, source: &ProjectSource, deps: &SourceDeps) -> Result<ResolvedSource, HostError> {
    project_source(kind).ok().unwrap_or_else(|| panic!("no module for {kind}")).resolve(source, deps).await
}

// fixture: project_sources.json kinds
#[test]
fn the_sources_are_folder_git_github_gitlab_in_that_order() {
    let fx = fixture("project_sources");
    let kinds: Vec<&str> = project_sources().keys().copied().collect();
    assert_eq!(json(&kinds), fx["kinds"]);
    assert_eq!(kinds, vec!["folder", "git", "github", "gitlab"]);
    for (key, module) in project_sources() {
        assert_eq!(module.kind(), key);
    }
}

// fixture: project_sources.json unknown
#[test]
fn an_unknown_kind_is_refused_with_no_kind_stamped() {
    let fx = fixture("project_sources");
    let e = project_source("svn").err().expect("svn has no module");
    assert_eq!(e.message, fx["unknown"]["err"].as_str().unwrap());
    assert_eq!(e.message, "no project source module for svn");
    assert_eq!(e.kind, None);
}

// fixture: project_sources.json defaultBranch
#[test]
fn the_default_branch_of_a_clone_that_read_nothing_is_the_ts_one() {
    let fx = fixture("project_sources");
    assert_eq!(json(&DEFAULT_BRANCH), fx["defaultBranch"]);
}

// fixture: project_sources.json remote
#[test]
fn the_remote_each_source_names_on_its_own_is_the_ts_answer_for_every_row() {
    let fx = fixture("project_sources");
    let all = rows(&fx, "remote");
    assert_eq!(all.len(), 4);
    for row in all {
        let source: ProjectSource = wire(&row[0]);
        assert_eq!(project_remote(&source), row[1].as_str().unwrap(), "remote row {}", row[0]);
    }
}

// fixture: project_sources.json resolve (folder here, folder here none)
#[test]
fn a_folder_resolves_on_this_computer_to_its_origin_and_the_branch_that_origin_names() {
    run_local(async {
        let fx = fixture("project_sources");
        let all = rows(&fx, "resolve");
        let found = deps(false, plan(json!({})), "https://github.com/acme/web.git", "trunk");
        let got = resolve("folder", &folder_source(), &found).await.expect("folder here");
        assert_eq!(resolved_json(&got), expected_ok(named(all, "folder here")), "resolve row folder here");
        assert_eq!(got.seed, None);

        let none = deps(false, plan(json!({})), "", "");
        let got = resolve("folder", &folder_source(), &none).await.expect("folder here none");
        assert_eq!(resolved_json(&got), expected_ok(named(all, "folder here none")), "resolve row folder here none");
        assert_eq!(got.default_branch, "main");
    });
}

// fixture: project_sources.json resolve (folder seeding ...)
#[test]
fn a_folder_that_seeds_another_computer_resolves_to_the_https_remote_and_carries_its_plan() {
    run_local(async {
        let fx = fixture("project_sources");
        let all = rows(&fx, "resolve");
        let cases: Vec<(&str, Value)> = vec![
            ("folder seeding ssh", plan(json!({}))),
            ("folder seeding https", plan(json!({ "remote": "https://github.com/acme/web.git", "defaultBranch": null }))),
            ("folder seeding gitlab ssh", plan(json!({ "remote": "git@gitlab.com:dev/thing.git", "defaultBranch": "" }))),
            ("folder seeding other host", plan(json!({ "remote": "git@example.com:a/b.git", "defaultBranch": null, "branch": "" }))),
            ("folder seeding http github", plan(json!({ "remote": "http://github.com/acme/web" }))),
        ];
        for (name, input) in cases {
            let d = deps(true, input, "", "");
            let got = resolve("folder", &folder_source(), &d).await.unwrap_or_else(|e| panic!("resolve row {name}: {}", e.message));
            assert_eq!(resolved_json(&got), expected_ok(named(all, name)), "resolve row {name}");
            assert!(got.seed.is_some(), "resolve row {name}: no seed");
        }
    });
}

// fixture: project_sources.json resolve (folder seeding no remote)
#[test]
fn a_folder_that_seeds_another_computer_with_no_origin_is_refused_as_invalid() {
    run_local(async {
        let fx = fixture("project_sources");
        let row = named(rows(&fx, "resolve"), "folder seeding no remote");
        let d = deps(true, plan(json!({ "remote": null })), "", "");
        let e = resolve("folder", &folder_source(), &d).await.err().expect("a folder with no origin is refused");
        assert_eq!(e.message, row["err"].as_str().unwrap(), "resolve row folder seeding no remote");
        assert_eq!(e.kind, Some(Kind::Invalid));
        assert_eq!(row["kind"], "invalid");
    });
}

// fixture: project_sources.json resolve (git, github, gitlab)
#[test]
fn git_github_and_gitlab_sources_resolve_from_what_they_name_without_asking_the_computer() {
    run_local(async {
        let fx = fixture("project_sources");
        let all = rows(&fx, "resolve");
        let d = deps(true, plan(json!({})), "", "");
        let cases = [
            ("git", "git", json!({ "kind": "git", "url": "https://example.com/team/thing.git" })),
            ("github", "github", json!({ "kind": "github", "repo": "acme/web" })),
            ("gitlab", "gitlab", json!({ "kind": "gitlab", "repo": "dev/thing" })),
        ];
        for (name, kind, source) in cases {
            let source: ProjectSource = wire(&source);
            let got = resolve(kind, &source, &d).await.unwrap_or_else(|e| panic!("resolve row {name}: {}", e.message));
            assert_eq!(resolved_json(&got), *named(all, name), "resolve row {name}");
            assert_eq!(got.seed, None, "resolve row {name}");
        }
    });
}

// fixture: project_sources.json clone
#[test]
fn the_clone_line_is_the_ts_line_for_every_row() {
    let fx = fixture("project_sources");
    let all = rows(&fx, "clone");
    assert_eq!(all.len(), 7);
    let git = project_source("git").ok().unwrap();
    let folder = project_source("folder").ok().unwrap();
    let github = project_source("github").ok().unwrap();
    let gitlab = project_source("gitlab").ok().unwrap();
    let cases = [
        ("git", git.clone_command("https://example.com/x.git", "/root/x", None)),
        ("git branch", git.clone_command("https://example.com/x.git", "/root/a b", Some("feat/x"))),
        ("folder", folder.clone_command("--upload-pack=evil", "/root/x", None)),
        ("github", github.clone_command("https://github.com/acme/web.git", "/root/web", None)),
        ("github branch ignored", github.clone_command("https://github.com/acme/web.git", "/root/web", Some("dev"))),
        ("github not a repo url", github.clone_command("weird remote", "/root/web", None)),
        ("gitlab", gitlab.clone_command("https://gitlab.com/dev/thing.git", "/root/thing", None)),
    ];
    for (name, got) in cases {
        assert_eq!(got, named(all, name).as_str().unwrap(), "clone row {name}");
    }
}

// fixture: project_sources.json cli
#[test]
fn the_command_each_kind_clones_through_is_the_ts_answer_for_every_row() {
    let fx = fixture("project_sources");
    let all = rows(&fx, "cli");
    assert_eq!(all.len(), 4);
    for row in all {
        let kind = row[0].as_str().unwrap();
        let got = project_source(kind).ok().unwrap().cli().map_or(Value::Null, |c| json!({ "bin": c.bin, "setupGit": c.setup_git, "missing": c.missing("hetzner") }));
        assert_eq!(got, row[1], "cli row {kind}");
    }
}
