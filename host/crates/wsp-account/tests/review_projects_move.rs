// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the export road (projects.ts:518-573): the scratch folder, the cache rule handed to the engine's export,
//! and the order of the failure line and the machine's cleanup. Expected values are printed by
//! oracle/zz-review-projects-export.oracle.ts (the TS export run on a scripted context).
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/projects.rs"]
mod projects;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use base::{person, record, run};
use projects::{fails, FakeLander, FakeProjects};
use serde_json::{json, Value};
use wsp_core::types::pending::CacheRule;
use wsp_core::types::wiring::{LandRequest, LandedProject, ProjectExportOptions, ProjectLander};
use wsp_words::exit::HostError;

extern "C" {
    fn umask(mask: u32) -> u32;
}

/// A lander that hands the engine a rule of its own and, when asked about the destination, reads what the scratch
/// folder under the temp root looks like then (the export has made it by that step and removes it after).
struct Watching {
    inner: Rc<FakeLander>,
    rule: CacheRule,
    temp: std::path::PathBuf,
    modes: RefCell<Vec<u32>>,
}

#[async_trait(?Send)]
impl ProjectLander for Watching {
    fn caches(&self) -> CacheRule {
        self.rule.clone()
    }
    async fn probe(&self, dest: &str) -> Result<Option<u64>, HostError> {
        use std::os::unix::fs::PermissionsExt;
        for entry in std::fs::read_dir(&self.temp).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name().to_string_lossy().starts_with("wsp-exported-") {
                self.modes.borrow_mut().push(entry.metadata().unwrap().permissions().mode() & 0o777);
            }
        }
        self.inner.probe(dest).await
    }
    async fn land(&self, req: LandRequest) -> Result<LandedProject, HostError> {
        self.inner.land(req).await
    }
}

fn rig(rule: CacheRule) -> (Rc<FakeProjects>, Rc<wsp_account::projects::projects::Projects>, Rc<Watching>) {
    let fake = FakeProjects::new();
    let area = fake.area();
    fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1" })));
    let lander =
        Rc::new(Watching { inner: FakeLander::new(&fake.base), rule, temp: fake.tmp.path().to_path_buf(), modes: RefCell::new(vec![]) });
    (fake, area, lander)
}

fn opts(lander: &Rc<Watching>) -> ProjectExportOptions {
    ProjectExportOptions {
        workspace_id: "ws_1".to_owned(),
        source: "/root/work/proj".to_owned(),
        dest: "/Users/dev/code/proj".to_owned(),
        replace: None,
        agents: None,
        lander: lander.clone(),
    }
}

fn rule(dirs: &[&str], files: &[&str], markers: &[&str]) -> CacheRule {
    let own = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    CacheRule { dirs: own(dirs), files: own(files), markers: own(markers) }
}

// R-PRJ-M1. The export's scratch folder holds the folder's whole archive and the agents' state, so it is private to the
// person: the TS makes it with `mkdtempSync`, mode 0700 whatever the umask (oracle: `SCRATCH_MODE_OCTAL 700`). The Rust
// makes it with `create_dir_all` (moves.rs:163), which takes the umask: 0755 under the usual 022.
#[test]
fn the_export_scratch_folder_is_private_to_the_person() {
    run(async {
        unsafe { umask(0o022) };
        let (_fake, area, lander) = rig(CacheRule::default());
        area.export(opts(&lander), Some(person())).await.unwrap();
        let modes = lander.modes.borrow().clone();
        assert_eq!(modes.len(), 1, "one scratch folder stood when the destination was probed");
        assert_eq!(modes[0], 0o700, "mode {:o}", modes[0]);
    });
}

// R-PRJ-M2. The engine's export takes the cache rule whole: names to leave when they are folders, names when they are
// files, and marker files whose folder is a cache (vault.ts:145-150 reads the three apart; projects.ts:540 passes
// `o.lander.caches` itself, oracle: `EXPORTFOLDER_RULE_IS_LANDER_CACHES true`). The Rust hands the road one flat list
// of the three (moves.rs:214), so a file named like a cache folder and a marker named like one cannot be told apart.
#[test]
fn the_road_is_told_which_cache_names_are_folders_files_or_markers() {
    run(async {
        let mut told = Vec::new();
        for caches in [rule(&["cache"], &[], &[]), rule(&[], &["cache"], &[]), rule(&[], &[], &["cache"])] {
            let (fake, area, lander) = rig(caches);
            area.export(opts(&lander), Some(person())).await.unwrap();
            told.push(fake.base.args_of("export_folder").pop().expect("the folder was packed")["caches"].clone());
        }
        assert_ne!(told[0], told[1], "a folder named cache and a file named cache are one list: {told:?}");
        assert_ne!(told[0], told[2], "a folder named cache and a marker named cache are one list: {told:?}");
        assert_ne!(told[1], told[2]);
    });
}

// R-PRJ-M3. A failed export says so first and cleans the machine after: the TS reports in the `catch` (projects.ts:566)
// and sends `rm -rf` in the `finally` (:570). Oracle ORDER: `bus project.export failed`, then `machine.exec rm -rf`.
// The Rust cleans the machine inside `export_run` and says the failure after (moves.rs:177-181 then :724).
#[test]
fn a_failed_export_says_so_before_it_cleans_the_machine() {
    run(async {
        let (fake, area, lander) = rig(CacheRule::default());
        fake.base.answer_with("lander_land", vec![fails("the lander refused", None)]);
        let err = area.export(opts(&lander), Some(person())).await.unwrap_err();
        assert_eq!(err.message, "the lander refused");
        let names = fake.base.names_of(&["emit", "machine_exec"]);
        let cleaned = names.iter().position(|n| n == "machine_exec").expect("the machine was cleaned");
        assert_eq!(&names[cleaned..], ["machine_exec"], "events after the cleanup: {names:?}");
        let events = fake.base.events.borrow().clone();
        assert_eq!(events.last().unwrap()["stage"], "failed");
    });
}

// R-PRJ-M4. The import's parts are written on the machine in the folder the machine's kind names: /tmp on a machine wsp
// made, wsp's own folder under the login's home on a computer somebody else owns, where a folder every account shares is
// one another account can sit in first (engine/src/vault.ts:173-176). So the TS asks the kind for it, `tmpDir:
// ctx.moduleOf(entry.record.kind).scratch(entry)`, as it builds the call to `landBundle` (projects.ts:75) and again for
// the agents' state (:89). The Rust declares `scratch` on its roads (moves.rs:60) and never asks it, and `land_bundle`
// and `import_into` carry no folder, so a road left to itself writes the parts in the shared one.
#[test]
fn the_import_asks_the_machines_kind_where_its_parts_are_written() {
    run(async {
        let fake = FakeProjects::new();
        let area = fake.area();
        let entry = fake.base.hold(record("ws_1", "place", "pr_x", json!({ "name": "task-1" })));
        let plan = json!({ "source": "/Users/dev/code/proj", "repo": true, "files": 6, "bytes": 4321, "secrets": [], "values": [], "excluded": [], "skipped": [], "agents": [] });
        let bundler = projects::FakeBundler::new(&fake.base, plan);
        let opts = wsp_core::types::wiring::ProjectImportOptions {
            workspace_id: "ws_1".to_owned(),
            source: "/Users/dev/code/proj".to_owned(),
            dest: "/home/dev/proj".to_owned(),
            replace: None,
            carry: None,
            rewrite: None,
            agents: None,
            bundler,
        };
        let report: wsp_core::types::internal::ImportReport = Rc::new(|_stage, _message, _progress| Ok(()));
        area.copy_import(&entry, opts, report).await.unwrap();
        assert_eq!(fake.base.names_of(&["scratch", "land_bundle"]), ["scratch", "land_bundle"]);
    });
}
