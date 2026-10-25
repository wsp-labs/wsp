// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests for provision_plan, provision_step, provision_mcp, vault_export and vault_import against what the TS
//! printed (tests/fixtures/oracle/provision.json, written by oracle/provision.oracle.ts), and the empty inputs.
mod common;

use std::collections::BTreeSet;
use std::io::Read;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;

use wsp_catalog::mcp::McpFormat;
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::golden_seam::{AsideRow, DropName, McpAgentPlan, McpPlan, McpScope, PinRead, ProjectMove, ToolInstall};
use wsp_engine::provision::*;
use wsp_engine::provision_files::ProvisionLanding;
use wsp_engine::provision_mcp::*;
use wsp_engine::vault::*;

fn s(v: &Value) -> String {
    v.as_str().unwrap().to_string()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(s).collect()
}

fn step(o: &Value) -> ToolInstall {
    let opt = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_string);
    ToolInstall {
        id: s(&o["id"]),
        label: s(&o["label"]),
        manager: s(&o["manager"]),
        cmd: s(&o["cmd"]),
        after: opt("after"),
        bin: opt("bin"),
        check: opt("check"),
        present: opt("present"),
        bins: o.get("bins").map(strings),
        asks: opt("asks"),
        below: o.get("below").map(|b| wsp_engine::golden_seam::Below { node: s(&b["node"]), asks: b.get("asks").map(s) }),
        pin: o.get("pin").map(|p| PinRead { read: p.get("read").map(s), fixed: p["fixed"].as_bool().unwrap(), words: s(&p["words"]) }),
        ..ToolInstall::default()
    }
}

fn tool(id: &str, manager: &str) -> ToolInstall {
    ToolInstall { id: id.into(), label: manager.into(), manager: manager.into(), cmd: "true".into(), ..ToolInstall::default() }
}

// --- provision_plan -----------------------------------------------------------------------------------------------

#[test]
fn each_presence_test_is_the_ts_text_in_its_order_for_asked_and_for_any() {
    let o = common::oracle("provision.json");
    for case in o["presenceTests"].as_array().unwrap() {
        let st = step(&case["step"]);
        assert_eq!(presence_tests(&st, Versions::Asked), strings(&case["asked"]), "asked: {}", case["name"]);
        assert_eq!(presence_tests(&st, Versions::Any), strings(&case["any"]), "any: {}", case["name"]);
    }
}

#[test]
fn a_step_with_nothing_to_read_has_no_test_and_present_by_waiters_needs_every_waiter() {
    let o = common::oracle("provision.json");
    let mut steps =
        vec![tool("apt-index", "apt"), tool("tools/a", "apt"), tool("tools/b", "apt"), tool("lonely", "apt"), tool("tools/c", "apt")];
    steps[1].bin = Some("a".into());
    steps[1].after = Some("apt-index".into());
    steps[2].bin = Some("b".into());
    steps[2].after = Some("apt-index".into());
    steps[4].bin = Some("c".into());
    steps[4].after = Some("lonely2".into());
    let both: BTreeSet<String> = ["tools/a".to_string(), "tools/b".to_string()].into();
    let one: BTreeSet<String> = ["tools/a".to_string()].into();
    assert_eq!(present_by_what_waits(&steps, &both), strings(&o["presentByWhatWaits"]["all"]));
    assert_eq!(present_by_what_waits(&steps, &one), strings(&o["presentByWhatWaits"]["one"]));
    // `lonely` reads nothing and nothing waits on it: never present by this rule.
    assert!(!present_by_what_waits(&steps, &both).contains(&"lonely".to_string()));
    assert!(present_by_what_waits(&[], &both).is_empty());
}

#[test]
fn a_command_outside_its_roads_folders_is_named_and_a_lookalike_folder_is_outside() {
    let o = common::oracle("provision.json");
    let brew = |bins: Option<Vec<String>>, bin: Option<&str>| {
        let mut t = tool("tools/x", "brew");
        t.bins = bins;
        t.bin = bin.map(str::to_string);
        t
    };
    let lb = || Some(vec!["/home/linuxbrew/.linuxbrew/bin".to_string()]);
    let at = |p: &str| PresentRead { path: Some(p.into()) };
    let got = vec![
        present_elsewhere(&brew(lb(), Some("rg")), Some(&at("/usr/bin/rg"))),
        present_elsewhere(&brew(lb(), Some("rg")), Some(&at("/home/linuxbrew/.linuxbrew/bin/rg"))),
        present_elsewhere(&brew(lb(), Some("rg")), Some(&at("/home/linuxbrew/.linuxbrew/binx/rg"))),
        present_elsewhere(&brew(None, Some("rg")), Some(&at("/usr/bin/rg"))),
        present_elsewhere(&brew(Some(vec!["/x".into()]), None), Some(&at("/usr/bin/rg"))),
        present_elsewhere(&brew(Some(vec!["/x".into()]), Some("rg")), Some(&PresentRead::default())),
    ];
    let want: Vec<Option<String>> = o["presentElsewhere"].as_array().unwrap().iter().map(|v| v.as_str().map(str::to_string)).collect();
    assert_eq!(got, want);
    assert_eq!(present_elsewhere(&brew(lb(), Some("rg")), None), None);
}

#[test]
fn the_counts_are_steps_landings_and_every_server_aside_kept_and_dropped() {
    let o = common::oracle("provision.json");
    let format = McpFormat::McpServersJson;
    let scope = |keep: &[&str], drop: &[&str]| McpScope {
        files: vec![],
        format,
        project: None,
        keep: keep.iter().map(|k| k.to_string()).collect(),
        drop: drop.iter().map(|d| DropName { name: d.to_string(), reason: "r".into(), keys: None }).collect(),
        travelled: None,
        own: None,
        folder: None,
        trust: None,
        edit: None,
    };
    let pack: wsp_engine::golden_seam::PackFiles = Arc::new(|_| Box::pin(async { Err(common::other("unused")) }));
    let plan = ProvisionPlan {
        recipe_at: "x".into(),
        path: "/bin".into(),
        prefix: Some("/opt/wsp".into()),
        steps: vec![tool("tools/apt", "apt"), tool("tools/npm", "npm")],
        files: Some(PlanFiles {
            lands: ["a", "b", "c"]
                .iter()
                .map(|d| ProvisionLanding { id: d.to_string(), label: d.to_string(), dest: d.to_string(), once: false })
                .collect(),
            pack,
        }),
        mcp: Some(McpPlan {
            agents: vec![McpAgentPlan {
                id: "claude".into(),
                label: "C".into(),
                scopes: vec![scope(&["a", "b"], &["c"]), scope(&[], &[])],
                aside: vec![AsideRow { id: "x".into(), name: "x".into(), reason: "r".into() }],
            }],
            guest_home: "/root".into(),
            rewrites: vec![],
            bin_dirs: vec![],
            tools: vec![],
        }),
        ..ProvisionPlan::default()
    };
    let got = provision_counts_of(&plan);
    let want = &o["provisionCountsOf"];
    assert_eq!(
        (got.tool, got.file, got.server),
        (want["tool"].as_u64().unwrap() as usize, want["file"].as_u64().unwrap() as usize, want["server"].as_u64().unwrap() as usize)
    );
    assert_eq!(provision_counts_of(&ProvisionPlan::default()), ProvisionCounts { tool: 0, file: 0, server: 0 });
}

#[test]
fn one_landing_per_dest_keeps_the_first_row_that_named_it() {
    let land = |id: &str, dest: &str| ProvisionLanding { id: id.into(), label: id.into(), dest: dest.into(), once: false };
    let got = once_per_dest(&[
        land("agents/claude", ".claude.json"),
        land("agents/mcp/claude/a", ".claude.json"),
        land("agents/claude", "CLAUDE.md"),
    ]);
    assert_eq!(got, vec![land("agents/claude", ".claude.json"), land("agents/claude", "CLAUDE.md")]);
    assert!(once_per_dest(&[]).is_empty());
}

// --- provision_step -----------------------------------------------------------------------------------------------

#[test]
fn the_folders_a_step_walks_are_each_roads_roots_clipped_to_usr_local_and_opt_without_wsps_own() {
    let o = common::oracle("provision.json");
    let plan = |steps: Vec<ToolInstall>, agents: usize| ProvisionPlan {
        recipe_at: "x".into(),
        path: "/bin".into(),
        prefix: Some("/opt/wsp".into()),
        steps,
        agents,
        ..ProvisionPlan::default()
    };
    for pair in o["outsideRoots"]["each"].as_array().unwrap() {
        let m = s(&pair[0]);
        assert_eq!(outside_roots(&plan(vec![tool(&format!("tools/{m}"), &m)], 0), EngineStep::Clis), strings(&pair[1]), "{m}");
    }
    let mut go = tool("tools/go", "go");
    go.bins = Some(vec!["/usr/local/bin".into(), "/opt/wsp/go/bin".into(), "/usr/local/go/bin".into()]);
    assert_eq!(
        outside_roots(&plan(vec![tool("tools/npm", "npm"), tool("tools/vendor", "vendor"), go], 0), EngineStep::Clis),
        strings(&o["outsideRoots"]["mixed"])
    );
    let two = || vec![tool("tools/npm", "npm"), tool("tools/release", "release")];
    assert_eq!(outside_roots(&plan(two(), 1), EngineStep::Agents), strings(&o["outsideRoots"]["agentsOnly"]));
    assert_eq!(outside_roots(&plan(two(), 1), EngineStep::Clis), strings(&o["outsideRoots"]["clisAfterAgents"]));
    assert!(outside_roots(&plan(two(), 1), EngineStep::Skills).is_empty());
}

#[test]
fn the_last_words_are_the_trimmed_last_line_cut_at_300_and_nothing_for_blank() {
    assert_eq!(last_words("  \n first\n second line \n"), Some("second line".into()));
    assert_eq!(last_words(""), None);
    assert_eq!(last_words("   \n  "), None);
    assert_eq!(last_words(&"é".repeat(400)), Some("é".repeat(300)));
}

#[test]
fn within_is_the_folder_itself_or_under_it_never_a_lookalike() {
    assert!(within("/usr/local", "/usr/local"));
    assert!(within("/usr/local/bin", "/usr/local"));
    assert!(!within("/usr/localx", "/usr/local"));
}

// --- provision_mcp ------------------------------------------------------------------------------------------------

#[test]
fn the_servers_words_are_the_ts_sentences() {
    let o = common::oracle("provision.json");
    let l = &o["mcpLines"];
    assert_eq!(their_server_line("Claude Code", "gsc", "/root/.claude.json"), s(&l["their"]));
    assert_eq!(key_unreached_line("Codex", &["GITHUB_TOKEN".into(), "API_KEY".into()]), s(&l["unreached"]));
    assert_eq!(untrusted_line("Codex", "/root/repo", "the file is a link"), s(&l["untrusted"]));
    assert_eq!(no_copy_line("/root/.claude.json"), s(&l["noCopy"]));
    assert_eq!(server_line("agents/mcp/claude/a", "d"), "agents/mcp/claude/a\td\td");
    assert_eq!(tombstone_line("agents/mcp/claude/a"), "agents/mcp/claude/a\t-\t-");
}

#[test]
fn a_plan_moves_with_the_home_every_path_under_it_and_nothing_beside_it() {
    let o = common::oracle("provision.json");
    let scope = |files: &[&str], format: McpFormat, project: Option<ProjectMove>| McpScope {
        files: files.iter().map(|f| f.to_string()).collect(),
        format,
        project,
        keep: vec![],
        drop: vec![],
        travelled: None,
        own: None,
        folder: None,
        trust: None,
        edit: None,
    };
    let plan = McpPlan {
        agents: vec![
            McpAgentPlan {
                id: "claude".into(),
                label: "Claude Code".into(),
                scopes: vec![
                    scope(
                        &["/root/.claude-cfg/.claude.json", "/rootx/a", "/root"],
                        McpFormat::McpServersJson,
                        Some(ProjectMove { from: "/Users/dev/repo".into(), to: "/root/repo".into() }),
                    ),
                    scope(&["/elsewhere/c.json"], McpFormat::McpServersJson, None),
                ],
                aside: vec![],
            },
            McpAgentPlan {
                id: "codex".into(),
                label: "Codex".into(),
                scopes: vec![scope(&["/root/.codex/config.toml"], McpFormat::CodexToml, None)],
                aside: vec![],
            },
        ],
        guest_home: "/root".into(),
        rewrites: vec![
            ("/Users/dev/".into(), "/root/".into()),
            ("/Users/dev/bin".into(), "/root/bin".into()),
            ("/opt/x/".into(), "/opt/y/".into()),
            ("/Users/dev".into(), "/root".into()),
        ],
        bin_dirs: vec!["/Users/dev/.local/bin/".into()],
        tools: vec![],
    };
    let a = &o["atHome"];
    let moved = at_home(&plan, "/home/dev");
    assert_eq!(moved.guest_home, s(&a["guestHome"]));
    let rewrites: Vec<(String, String)> = a["rewrites"].as_array().unwrap().iter().map(|p| (s(&p[0]), s(&p[1]))).collect();
    assert_eq!(moved.rewrites, rewrites);
    assert_eq!(moved.bin_dirs, strings(&a["binDirs"]));
    for (agent, want) in moved.agents.iter().zip(a["scopes"].as_array().unwrap()) {
        for (scope, w) in agent.scopes.iter().zip(want.as_array().unwrap()) {
            assert_eq!(scope.files, strings(&w["files"]));
            assert_eq!(
                scope.project.as_ref().map(|p| (p.from.clone(), p.to.clone())),
                w["project"].as_object().map(|p| (s(&p["from"]), s(&p["to"])))
            );
        }
    }
    let same = at_home(&plan, "/root");
    assert_eq!(same.rewrites, plan.rewrites);
    assert_eq!(same.agents[0].scopes[0].files, plan.agents[0].scopes[0].files);
}

#[test]
fn a_file_travels_only_from_under_the_home() {
    assert_eq!(
        travelled_path("/root", "/root/.claude.json"),
        Some(format!("{}/.claude.json", wsp_words::project_path::place_provision_paths("/root").staging))
    );
    assert_eq!(travelled_path("/root", "/rootx/.claude.json"), None);
    assert_eq!(travelled_path("/root", "/root"), None);
}

// --- vault ----------------------------------------------------------------------------------------------------------

/// The archive and the trip's own paths carry the clock and a random tail; the TS ran at a fixed one.
fn tmp_names(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = ["wsp-vault-in-", "wsp-vault-keep-", "wsp-vault-", "wsp-out-", ".wsp-in-"]
        .iter()
        .filter_map(|m| rest.find(m).map(|i| (i, m.len())))
        .min()
    {
        let (i, len) = at;
        out.push_str(&rest[..i + len]);
        let tail = &rest[i + len..];
        let digits = tail.chars().take_while(char::is_ascii_digit).count();
        let after = &tail[digits..];
        if digits > 0 && after.starts_with('-') {
            let rand = after[1..].chars().take_while(|c| c.is_ascii_digit() || c.is_ascii_lowercase()).count();
            out.push_str("T-R");
            rest = &after[1 + rand..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

#[test]
fn a_guest_tmp_path_is_the_folder_without_trailing_slashes_the_name_the_clock_and_six_base36() {
    let p = guest_tmp_path("wsp-out", Some("/root/.wsp//"));
    assert!(p.starts_with("/root/.wsp/wsp-out-"), "{p}");
    let tail: Vec<&str> = p["/root/.wsp/wsp-out-".len()..].split('-').collect();
    assert_eq!(tail.len(), 2);
    assert!(tail[0].len() >= 13 && tail[0].chars().all(|c| c.is_ascii_digit()));
    assert!(!tail[1].is_empty() && tail[1].len() <= 6 && tail[1].chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()));
    assert!(guest_tmp_path("x", None).starts_with("/tmp/x-"));
}

struct OkFetch;

#[async_trait(?Send)]
impl Fetch for OkFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let body = if req.method == "PUT" { vec![] } else { b"tgz-bytes-12".to_vec() };
        Ok(FetchResponse { status: 200, headers: vec![], body })
    }
}

/// The TS oracle's recorder: answers `wc -c` with 12 and `test -e` with no, every other call with exit 0.
fn recorder() -> (common::FakeMachine, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<String>>>) {
    let execs = Arc::new(Mutex::new(Vec::new()));
    let runs = Arc::new(Mutex::new(Vec::new()));
    let e = execs.clone();
    let mut m = common::FakeMachine::new(Arc::new(move |cmd: String, _| {
        e.lock().unwrap().push(cmd.clone());
        Box::pin(async move {
            Ok(common::res(
                0,
                if cmd.starts_with("wc -c") {
                    "12\n"
                } else if cmd.starts_with("test -e") {
                    "no\n"
                } else {
                    ""
                },
                "",
            ))
        })
    }));
    let r = runs.clone();
    m.run = Some(Arc::new(move |script: String, _| {
        r.lock().unwrap().push(script);
        Box::pin(async { Ok(common::ok()) })
    }));
    (m, execs, runs)
}

fn normalised(v: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    v.lock().unwrap().iter().map(|x| tmp_names(x)).collect()
}

fn want(v: &Value) -> Vec<String> {
    strings(v).iter().map(|x| tmp_names(x)).collect()
}

fn rule() -> CacheRule {
    CacheRule {
        dirs: vec!["node_modules".into(), ".next".into(), "it's".into()],
        files: vec!["dist".into()],
        markers: vec!["CACHEDIR.TAG".into(), ".wsp-cache".into()],
    }
}

#[test]
fn the_folder_export_is_the_ts_text_with_a_rule_and_with_an_empty_one() {
    let o = common::oracle("provision.json");
    assert_eq!(folder_export_script("/root/repo", &rule(), "/tmp/wsp-out-1.tgz"), s(&o["folderExportScript"]["plain"]));
    assert_eq!(folder_export_script("/srv/a b", &CacheRule::default(), "/tmp/o.tgz"), s(&o["folderExportScript"]["emptyRule"]));
}

#[tokio::test]
async fn each_export_sends_the_ts_commands_in_order_and_refuses_with_the_ts_words() {
    let o = common::oracle("provision.json");
    let fetch: Arc<dyn Fetch> = Arc::new(OkFetch);
    let cases: Vec<(&str, Vec<String>, VaultOptions)> = vec![
        ("none", vec![], VaultOptions::default()),
        ("plain", vec!["/root/.claude".into(), "relative/dir".into(), "/root/it's".into()], VaultOptions::default()),
        (
            "leave",
            vec!["/root/.claude".into()],
            VaultOptions { leave: vec!["/root/.claude/projects".into(), "/root/it's here".into()], ..VaultOptions::default() },
        ),
        (
            "rule",
            vec!["/root/repo".into(), "/root/node_modules/".into(), "/root/dist".into()],
            VaultOptions { exclude: Some(rule()), leave: vec!["/root/repo/.env".into()], ..VaultOptions::default() },
        ),
        (
            "allCaches",
            vec!["/root/node_modules".into(), "/root/dist/".into()],
            VaultOptions { exclude: Some(rule()), ..VaultOptions::default() },
        ),
        ("capped", vec!["/root/a".into()], VaultOptions { max_bytes: Some(11), ..VaultOptions::default() }),
    ];
    for (name, paths, opts) in cases {
        let (m, execs, runs) = recorder();
        let got = export_paths(&m, &paths, &VaultOptions { fetch: Some(fetch.clone()), ..opts }).await;
        let w = &o["exportPaths"][name];
        assert_eq!(normalised(&runs), want(&w["runs"]), "{name} runs");
        assert_eq!(normalised(&execs), want(&w["execs"]), "{name} execs");
        match w.get("error") {
            Some(msg) => {
                let err = got.expect_err(name);
                assert_eq!(err.to_string(), s(msg), "{name}");
                if name == "capped" {
                    assert!(matches!(err, EngineError::VaultTooLarge { bytes: 12, .. }), "{err:?}");
                }
            }
            None => assert_eq!(got.expect(name), b"tgz-bytes-12".to_vec(), "{name}"),
        }
    }
}

#[tokio::test]
async fn each_import_sends_the_ts_script_checks_the_hash_and_refuses_an_empty_archive_before_any_call() {
    let o = common::oracle("provision.json");
    let fetch: Arc<dyn Fetch> = Arc::new(OkFetch);
    let small = b"not really a tgz".to_vec();
    let cases: Vec<(&str, Vec<u8>, &str, VaultOptions)> = vec![
        ("replace", small.clone(), "/root", VaultOptions::default()),
        (
            "overlay",
            small.clone(),
            "/root/dest dir",
            VaultOptions { overlay: true, tmp_dir: Some("/root/.wsp/provision/".into()), ..VaultOptions::default() },
        ),
        (
            "leave",
            small.clone(),
            "/root",
            VaultOptions { leave: vec!["/root/.claude/projects".into(), "/root/it's".into()], ..VaultOptions::default() },
        ),
        ("empty", vec![], "/root", VaultOptions::default()),
    ];
    for (name, tar, dest, opts) in cases {
        let (m, execs, runs) = recorder();
        let got = import_into(&m, &tar, dest, &VaultOptions { fetch: Some(fetch.clone()), ..opts }).await;
        let w = &o["importInto"][name];
        assert_eq!(normalised(&runs), want(&w["runs"]), "{name} runs");
        assert_eq!(normalised(&execs), want(&w["execs"]), "{name} execs");
        match w.get("error") {
            Some(msg) => assert_eq!(got.expect_err(name).to_string(), s(msg), "{name}"),
            None => assert_eq!(got.expect(name), Imported { parts: w["parts"].as_u64().unwrap() as u32 }, "{name}"),
        }
    }
    assert!(s(&o["importInto"]["replace"]["runs"][0]).contains(&s(&o["importInto"]["sha256"])));
}

#[tokio::test]
async fn a_landing_refuses_a_relative_path_and_the_root_and_moves_the_staging_in_one_rename() {
    let o = common::oracle("provision.json");
    let fetch: Arc<dyn Fetch> = Arc::new(OkFetch);
    for (name, dest, replace) in [
        ("fresh", "/srv/app/", false),
        ("replace", "/srv/app", true),
        ("relative", "srv/app", false),
        ("root", "/", false),
        ("top", "/app", false),
    ] {
        let (m, execs, runs) = recorder();
        let got =
            land_bundle(&m, b"not really a tgz", dest, &LandOptions { fetch: Some(fetch.clone()), replace, ..LandOptions::default() })
                .await;
        let w = &o["landBundle"][name];
        assert_eq!(normalised(&runs), want(&w["runs"]), "{name} runs");
        assert_eq!(normalised(&execs), want(&w["execs"]), "{name} execs");
        match w.get("error") {
            Some(msg) => assert_eq!(got.expect_err(name).to_string(), s(msg), "{name}"),
            None => assert_eq!(got.expect(name), Imported { parts: 1 }, "{name}"),
        }
    }
}

fn entries_of(o: &Value) -> Vec<TarEntry> {
    use base64::Engine;
    o["tarOf"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let path = s(&e["path"]);
            if let Some(t) = e.get("target") {
                TarEntry::Link { path, target: s(t) }
            } else if e.get("dir").is_some() {
                TarEntry::Dir { path, mode: e["mode"].as_u64().unwrap() as u32 }
            } else {
                TarEntry::File {
                    path,
                    mode: e["mode"].as_u64().unwrap() as u32,
                    content: base64::engine::general_purpose::STANDARD.decode(s(&e["content"])).unwrap(),
                }
            }
        })
        .collect()
}

#[test]
fn the_tar_inside_the_gzip_is_the_ts_bytes_with_a_split_name_a_link_a_dir_and_utf8() {
    use base64::Engine;
    let o = common::oracle("provision.json");
    let gz = tar_of_at(&entries_of(&o), o["tarOf"]["mtimeSecs"].as_u64().unwrap()).unwrap();
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(&gz[..]).read_to_end(&mut tar).unwrap();
    let want = base64::engine::general_purpose::STANDARD.decode(s(&o["tarOf"]["tarBase64"])).unwrap();
    assert_eq!(tar.len(), want.len());
    assert!(tar == want, "the ustar bytes differ from the TS's");
}

#[test]
fn no_entry_is_two_zero_blocks() {
    let gz = tar_of_at(&[], 0).unwrap();
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(&gz[..]).read_to_end(&mut tar).unwrap();
    assert_eq!(tar, vec![0u8; 1024]);
}

#[test]
fn what_fits_a_ustar_header_counts_bytes_cuts_leading_slashes_and_splits_at_a_slash() {
    let o = common::oracle("provision.json");
    for pair in o["fitsTar"]["paths"].as_array().unwrap() {
        assert_eq!(fits_tar(&s(&pair[0]), None), pair[1].as_bool().unwrap(), "{}", s(&pair[0]).len());
    }
    for case in o["fitsTar"]["targets"].as_array().unwrap() {
        assert_eq!(fits_tar(&s(&case[0]), Some(&s(&case[1]))), case[2].as_bool().unwrap(), "{}", s(&case[1]).len());
    }
}

#[test]
fn a_path_or_link_ustar_cannot_hold_is_refused_with_the_ts_words() {
    let o = common::oracle("provision.json");
    let long = "a".repeat(303);
    assert_eq!(
        tar_of_at(&[TarEntry::File { path: long, mode: 0o644, content: vec![] }], 0).unwrap_err().to_string(),
        s(&o["tarRefusals"]["tooLong"])
    );
    assert_eq!(
        tar_of_at(&[TarEntry::Link { path: "l".into(), target: "t".repeat(101) }], 0).unwrap_err().to_string(),
        s(&o["tarRefusals"]["linkTooLong"])
    );
}

#[test]
fn an_upload_refusal_names_the_part_the_reason_the_attempts_and_the_limit() {
    let refusal = r#"{"error":"Payload Too Large","limit":33554432}"#;
    assert_eq!(
        upload_failure(413, refusal, 1, 1, 1),
        "vault import upload failed: HTTP 413 Payload Too Large; the upload takes at most 33554432 bytes per PUT"
    );
    assert_eq!(
        upload_failure(502, "  bad gateway \n", 2, 3, 3),
        "vault import upload failed on part 2 of 3: HTTP 502 bad gateway after 3 attempts"
    );
    assert_eq!(upload_failure(500, "", 1, 1, 2), "vault import upload failed: HTTP 500 after 2 attempts");
    assert_eq!(
        upload_failure(400, r#"{"error":7,"limit":"x"}"#, 1, 1, 1),
        r#"vault import upload failed: HTTP 400 {"error":7,"limit":"x"}"#
    );
    assert_eq!(upload_failure(400, &"z".repeat(400), 1, 1, 1), format!("vault import upload failed: HTTP 400 {}", "z".repeat(300)));
}

#[test]
fn kept_paths_judge_the_last_name_and_refuse_when_every_path_is_a_cache() {
    let got = kept_paths(&["/root/repo".into(), "/root/node_modules/".into(), "/root/dist".into(), "/root/distx".into()], &rule()).unwrap();
    assert_eq!(got, vec!["/root/repo".to_string(), "/root/distx".to_string()]);
    assert!(kept_paths(&["/root/node_modules".into()], &rule()).is_err());
}

#[test]
fn a_tar_path_from_the_root_cuts_one_slash_and_keeps_a_relative_path() {
    assert_eq!(tar_path("/", "/root/a"), "root/a");
    assert_eq!(tar_path("/", "relative/dir"), "relative/dir");
    assert_eq!(tar_path("/", "//double"), "/double");
    assert_eq!(tar_path("/srv/mirror", "/srv/mirror/root/.claude"), "root/.claude");
    assert_eq!(leave_flags("/", &[]), "");
}

#[test]
fn an_existing_destination_is_kind_exists_with_the_ts_words() {
    let e = dest_exists("/Users/dev/out", 1);
    assert!(matches!(e, EngineError::Exists { .. }));
    assert_eq!(e.to_string(), "/Users/dev/out already exists on this computer with 1 file; export with replace to overwrite it");
    let m = exists_on_machine("/srv/app");
    assert!(matches!(m, EngineError::Exists { .. }));
    assert_eq!(m.to_string(), "/srv/app already exists on the machine; import with replace to overwrite it");
}
