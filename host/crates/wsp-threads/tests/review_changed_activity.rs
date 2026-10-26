// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the changed activity tests (tests/activity.rs:116). The test named for the run file's agent pid now
//! gives its scope the agent's own pid as the root, and a root that is no agent stands in as one (protocol
//! views/activity.ts `standIns`), so its view reads the same whether or not the run file was read. This puts the
//! agent under a root that is not it, where the recorded pid alone decides which process is the agent.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("activity/world.rs");

fn agent_under_a_shell(run_file_names_the_agent: bool) -> Value {
    let mut out = Value::Null;
    run_local(async {
        let dir = tempfile::tempdir().expect("a folder");
        let run = dir.path().join("run-1");
        let mine = std::process::id();
        if run_file_names_the_agent {
            std::fs::write(format!("{}.agent", run.display()), format!("{mine}\n")).expect("the agent file");
        }
        let lab = Lab::new();
        lab.local_thread(T1, "w1", "go", STARTED, "running");
        lab.scopes(vec![json!({ "thread": T1, "agents": [4100] })]);
        lab.host.rows.borrow()[&format!("s-{T1}")].borrow_mut().run = Some(run.display().to_string());
        lab.here.set_procs(vec![
            proc_row(4100, json!({ "comm": "sh", "cmdline": "sh run-1.sh" })),
            proc_row(mine, json!({ "ppid": 4100, "comm": "agent-x", "cmdline": "agent-x" })),
        ]);
        out = view_of(lab.read(ActivityAsk::default(), None).await);
    });
    out
}

// AC10: the pid a running turn's run file names is the agent, whatever it runs as (protocol views/activity.ts
// placeProcs: `recorded` first, `byName` only where none was recorded).
#[test]
fn the_pid_a_run_file_names_is_the_agent_under_a_root_that_is_not_it() {
    let mine = std::process::id();
    let shell = shown(4100, 1, 0, STARTED, "agent", "sh", "sh run-1.sh", 1_000_000, 0, &[]);
    let agent = shown(mine, 4100, 1, STARTED, "agent", "agent-x", "agent-x", 1_000_000, 0, &[]);
    let with = agent_under_a_shell(true);
    assert_eq!(with["threads"], json!([thread_json(T1, "go", "w1", 2_000_000, 0, vec![shell, agent])]));
    let without = agent_under_a_shell(false);
    assert_eq!(
        without["threads"][0]["processes"][1]["kind"],
        json!("tool"),
        "with no run file, agent-x is no agent: the fixture tells the two reads apart"
    );
}
