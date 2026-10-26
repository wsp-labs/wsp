// SPDX-License-Identifier: AGPL-3.0-only
//! The context reads of a start that throw in the TS (sessions.ts:300-330: `opensIn`, `ofThreadsRepository`,
//! `elsewhereRefusal`, `awayFor` read `projectHeld`; `placeName(projectHeld(project).computer)`): the throw is the answer and
//! nothing is minted after it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_start/world.rs");

fn gone(project: &str) -> HostError {
    HostError::new(format!("no project {project}"))
}

// sessions.ts:300 `ctx.opensIn(entry.record, origin)`.
#[test]
fn an_opening_rule_that_throws_fails_the_start_as_it_is() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w9", "lead-t"));
        let fx = fx();
        hook!(
            fx.host,
            "SessionsCtx::opens_in",
            dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<bool, HostError>,
            |_r: &WorkspaceRecord, _o: Option<&Caller>| Err(gone("p1"))
        );
        let e = fx.refused(go("hi"), Some(child)).await;
        assert_eq!(said(&e), said(&gone("p1")));
        before_the_mint(&fx.host);
    })
}

// sessions.ts:301 `ctx.ofThreadsRepository(origin, record.project)`.
#[test]
fn a_repository_read_that_throws_fails_the_start_as_it_is() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w9", "lead-t"));
        let fx = fx();
        hook!(
            fx.host,
            "SessionsCtx::opens_in",
            dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<bool, HostError>,
            |_r: &WorkspaceRecord, _o: Option<&Caller>| Ok(false)
        );
        hook!(
            fx.host,
            "SessionsCtx::of_threads_repository",
            dyn Fn(Option<&Caller>, &str) -> Result<bool, HostError>,
            |_o: Option<&Caller>, _p: &str| Err(gone("p1"))
        );
        let e = fx.refused(go("hi"), Some(child)).await;
        assert_eq!(said(&e), said(&gone("p1")));
        before_the_mint(&fx.host);
    })
}

// sessions.ts:302 `ctx.elsewhereRefusal(origin, project, name)`.
#[test]
fn a_folder_refusal_read_that_throws_fails_the_start_as_it_is() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w9", "lead-t"));
        let fx = fx();
        hook!(
            fx.host,
            "SessionsCtx::opens_in",
            dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<bool, HostError>,
            |_r: &WorkspaceRecord, _o: Option<&Caller>| Ok(false)
        );
        hook!(
            fx.host,
            "SessionsCtx::of_threads_repository",
            dyn Fn(Option<&Caller>, &str) -> Result<bool, HostError>,
            |_o: Option<&Caller>, _p: &str| Ok(true)
        );
        hook!(
            fx.host,
            "SessionsCtx::elsewhere_refusal",
            dyn Fn(Option<&Caller>, &str, &str) -> Result<Option<HostError>, HostError>,
            |_o: Option<&Caller>, _p: &str, _n: &str| Err(gone("p1"))
        );
        let e = fx.refused(go("hi"), Some(child)).await;
        assert_eq!(said(&e), said(&gone("p1")));
        before_the_mint(&fx.host);
    })
}

// sessions.ts:303 `ctx.awayFor(entry.record, origin, "start", workspaceId)`.
#[test]
fn a_machine_refusal_read_that_throws_fails_the_start_as_it_is() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w9", "lead-t"));
        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", json!({ "name": "lab", "kind": "cloud" })));
        hook!(
            fx.host,
            "SessionsCtx::opens_in",
            dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<bool, HostError>,
            |_r: &WorkspaceRecord, _o: Option<&Caller>| Ok(false)
        );
        hook!(
            fx.host,
            "SessionsCtx::of_threads_repository",
            dyn Fn(Option<&Caller>, &str) -> Result<bool, HostError>,
            |_o: Option<&Caller>, _p: &str| Ok(true)
        );
        hook!(
            fx.host,
            "SessionsCtx::away_for",
            dyn Fn(&WorkspaceRecord, Option<&Caller>, &str, &str) -> Result<Option<HostError>, HostError>,
            |_r: &WorkspaceRecord, _o: Option<&Caller>, _a: &str, _w: &str| Err(gone("p1"))
        );
        let e = fx.refused(go("hi"), Some(child)).await;
        assert_eq!(said(&e), said(&gone("p1")));
        before_the_mint(&fx.host);
    })
}

// sessions.ts:323 `ctx.placeName(ctx.projectHeld(r.project).computer)` for the asking record and the sent one.
#[test]
fn a_computer_name_read_that_throws_fails_a_send_across_computers_as_it_is() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "wbox", "lead-t"));
        let fx = fx_on(Agent { road: Some(AttachmentRoad::Inline), ..Default::default() });
        fx.host.add_entry(entry("wbox", json!({ "name": "box", "kind": "place", "project": "pbox" })));
        fx.thread_on("t1", "w1", "claude");
        hook!(
            fx.host,
            "SessionsCtx::acts_on",
            dyn Fn(&WorkspaceRecord, Option<&Caller>) -> bool,
            |r: &WorkspaceRecord, _o: Option<&Caller>| r.id != "w1"
        );
        hook!(fx.host, "SessionsCtx::project_computer_name", dyn Fn(&str) -> Result<String, HostError>, |p: &str| Err(gone(p)));
        let sent = StartOptions { thread: Some("t1".into()), ..go("the box is done") };
        let e = fx.refused(sent, Some(child)).await;
        assert_eq!(said(&e), said(&gone("pbox")));
        before_the_mint(&fx.host);
    })
}
