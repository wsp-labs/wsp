// SPDX-License-Identifier: AGPL-3.0-only
//! The roads that follow a pushed stream and the reads beside them (io.ts:57-143, 434-452; spec/io.md): which
//! frames reach the caller, in what order, what the host is asked, and what the socket going does.

#[path = "cli_a_common/mod.rs"]
mod common;

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use common::{Script, ScriptedHost};
use wsp_cli::verbs::client::{Dialer, HostClient};
use wsp_cli::verbs::io::{
    exec_on, export_project, host_folders, import_project, init_setup, plan_project, ExecExit, ExportRequest, FoldersAsked,
};
use wsp_wire::wire::runtime::ExecEvent;
use wsp_words::exit::{HostError, Kind};

const HOST_STOPPING_LINE: &str = "the host is restarting; the turn goes on and its reply lands in the thread";

fn script(replies: &[(&str, &str)], pushed: &[(&str, &[&str])], closes: Option<&str>) -> Script {
    Script {
        replies: replies.iter().map(|(op, f)| ((*op).to_owned(), (*f).to_owned())).collect(),
        pushed: pushed.iter().map(|(op, fs)| ((*op).to_owned(), fs.iter().map(|f| (*f).to_owned()).collect())).collect(),
        closes: closes.map(str::to_owned),
    }
}

async fn socket(host: &Arc<ScriptedHost>) -> Arc<dyn HostClient> {
    host.client(None).await.unwrap()
}

fn texts(seen: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    seen.lock().unwrap().clone()
}

mod exec {
    use super::*;

    fn recorder() -> (Arc<Mutex<Vec<String>>>, wsp_cli::verbs::io::ExecSink) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let noted = seen.clone();
        (
            seen,
            Arc::new(move |e: &ExecEvent| {
                noted.lock().unwrap().push(match e {
                    ExecEvent::ExecOutput { text, .. } => format!("out {text}"),
                    ExecEvent::ExecExit { exit_code, .. } => format!("exit {exit_code:?}"),
                })
            }),
        )
    }

    #[tokio::test]
    async fn frames_pushed_ahead_of_the_reply_are_held_and_only_this_exec_s_reach_the_caller_in_order() {
        let host = ScriptedHost::new(script(
            &[("workspaces.exec", r#"{"id":1,"ok":true,"execId":"exec-1","cwd":"/root/wsp"}"#)],
            &[(
                "workspaces.exec",
                &[
                    r#"{"type":"exec.output","execId":"exec-0","text":"someone else's"}"#,
                    r#"{"type":"exec.output","execId":"exec-1","text":"one"}"#,
                    r#"{"type":"session.delta","execId":"exec-1","text":"not an exec frame"}"#,
                    r#"{"type":"exec.output","execId":"exec-1","text":"two"}"#,
                    r#"{"type":"exec.exit","execId":"exec-1","exitCode":3}"#,
                    r#"{"type":"exec.output","execId":"exec-1","text":"after the exit"}"#,
                ],
            )],
            None,
        ));
        let client = socket(&host).await;
        let (seen, on) = recorder();
        let argv = vec!["sh".to_owned(), "-c".to_owned(), "echo hi".to_owned()];
        let run = exec_on(client.as_ref(), "ws-1", &argv, Some("/root/wsp"), on).await.unwrap();
        assert_eq!(run.exit, ExecExit { exec_id: "exec-1".into(), exit_code: Some(3), error: None });
        assert_eq!(run.ran_in.as_deref(), Some("/root/wsp"), "the folder the host answered, not the one asked");
        assert_eq!(texts(&seen), vec!["out one", "out two", "exit Some(3)"], "the first exit ends it; nothing after it is read");
        assert_eq!(
            host.asked(),
            vec![json!({ "workspaceId": "ws-1", "argv": ["sh", "-c", "echo hi"], "cwd": "/root/wsp", "op": "workspaces.exec" })]
        );
    }

    #[tokio::test]
    async fn no_cwd_asked_leaves_the_key_out_and_no_cwd_answered_leaves_ran_in_empty() {
        let host = ScriptedHost::new(script(
            &[("workspaces.exec", r#"{"id":1,"ok":true,"execId":"e"}"#)],
            &[("workspaces.exec", &[r#"{"type":"exec.exit","execId":"e","exitCode":null,"error":"the machine went away"}"#])],
            None,
        ));
        let client = socket(&host).await;
        let (_seen, on) = recorder();
        let run = exec_on(client.as_ref(), "ws-1", &["ls".to_owned()], None, on).await.unwrap();
        assert_eq!(run.ran_in, None);
        assert_eq!(run.exit, ExecExit { exec_id: "e".into(), exit_code: None, error: Some("the machine went away".into()) });
        assert_eq!(host.asked(), vec![json!({ "workspaceId": "ws-1", "argv": ["ls"], "op": "workspaces.exec" })]);
    }

    #[tokio::test]
    async fn a_host_that_stops_under_an_exec_fails_it_with_the_stopping_line_instead_of_hanging() {
        let host = ScriptedHost::new(script(&[("workspaces.exec", r#"{"id":1,"ok":true,"execId":"e"}"#)], &[], Some("workspaces.exec")));
        let client = socket(&host).await;
        let (seen, on) = recorder();
        let e = exec_on(client.as_ref(), "ws-1", &["sleep".to_owned()], None, on).await.err().expect("failed");
        assert_eq!(e.message, HOST_STOPPING_LINE);
        assert_eq!(e.kind, None);
        assert!(texts(&seen).is_empty());
    }

    #[tokio::test]
    async fn the_hold_on_pushed_frames_is_let_go_on_every_road_out() {
        let host = ScriptedHost::new(script(
            &[("workspaces.exec", r#"{"id":1,"ok":true,"execId":"e"}"#)],
            &[("workspaces.exec", &[r#"{"type":"exec.exit","execId":"e","exitCode":0}"#])],
            None,
        ));
        let client = socket(&host).await;
        let (seen, on) = recorder();
        exec_on(client.as_ref(), "ws-1", &["true".to_owned()], None, on).await.unwrap();
        host.socket(0).push(r#"{"type":"exec.output","execId":"e","text":"late"}"#);
        assert_eq!(texts(&seen), vec!["exit Some(0)"], "a frame after the end reaches nobody");
    }

    #[tokio::test]
    async fn a_refused_exec_is_the_host_s_refusal_and_nothing_is_followed() {
        let host = ScriptedHost::new(script(
            &[("workspaces.exec", r#"{"id":1,"ok":false,"error":"no such workspace","kind":"not-found"}"#)],
            &[],
            None,
        ));
        let client = socket(&host).await;
        let (_seen, on) = recorder();
        let e = exec_on(client.as_ref(), "ws-9", &["ls".to_owned()], None, on).await.err().expect("refused");
        assert_eq!((e.message.as_str(), e.kind.clone()), ("no such workspace", Some(Kind::NotFound)));
    }
}

mod export_and_import {
    use super::*;

    fn stages() -> (Arc<Mutex<Vec<String>>>, wsp_cli::verbs::io::StageSink) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let noted = seen.clone();
        (seen, Arc::new(move |f: &Value| noted.lock().unwrap().push(f["message"].as_str().unwrap_or("").to_owned())))
    }

    const EXPORTED: &str =
        r#"{"id":1,"ok":true,"exported":{"dest":"/home/me/app","files":12,"bytes":3.5,"excluded":["node_modules"],"agents":[]}}"#;

    #[tokio::test]
    async fn an_export_subscribes_before_it_asks_and_follows_only_its_own_workspace_and_destination() {
        let host = ScriptedHost::new(script(
            &[("project.export", EXPORTED)],
            &[(
                "project.export",
                &[
                    r#"{"type":"project.export","workspaceId":"ws-1","dest":"/home/me/app","stage":"packing","message":"packing"}"#,
                    r#"{"type":"project.export","workspaceId":"ws-2","dest":"/home/me/app","stage":"packing","message":"another workspace"}"#,
                    r#"{"type":"project.export","workspaceId":"ws-1","dest":"/elsewhere","stage":"packing","message":"another destination"}"#,
                    r#"{"type":"project.import","workspaceId":"ws-1","dest":"/home/me/app","message":"an import"}"#,
                    r#"{"type":"project.export","workspaceId":"ws-1","dest":"/home/me/app","stage":"done","message":"done"}"#,
                ],
            )],
            None,
        ));
        let client = socket(&host).await;
        let (seen, on) = stages();
        let req = ExportRequest { source: "/root/app".into(), dest: "/home/me/app".into(), replace: None, agents: None };
        let out = export_project(client.as_ref(), "ws-1", &req, on).await.unwrap();
        assert_eq!(serde_json::to_value(&out).unwrap()["dest"], "/home/me/app");
        assert_eq!(texts(&seen), vec!["packing", "done"]);
        assert_eq!(host.state.subscribes.load(std::sync::atomic::Ordering::SeqCst), 1, "events are asked for before the export");
        assert_eq!(
            host.asked(),
            vec![json!({ "workspaceId": "ws-1", "source": "/root/app", "dest": "/home/me/app", "op": "project.export" })]
        );
    }

    #[tokio::test]
    async fn replace_and_agents_ride_the_export_only_where_given_false_included() {
        let host = ScriptedHost::new(script(&[("project.export", EXPORTED)], &[], None));
        let client = socket(&host).await;
        let (_seen, on) = stages();
        let req = ExportRequest { source: "/a".into(), dest: "/b".into(), replace: Some(false), agents: Some(vec!["claude".into()]) };
        export_project(client.as_ref(), "ws-1", &req, on).await.unwrap();
        let asked = host.asked();
        assert_eq!(
            serde_json::to_string(&asked[0]).unwrap(),
            r#"{"workspaceId":"ws-1","source":"/a","dest":"/b","replace":false,"agents":["claude"],"op":"project.export"}"#
        );
    }

    #[tokio::test]
    async fn an_import_follows_frames_matching_its_workspace_source_and_destination() {
        let host = ScriptedHost::new(script(
            &[(
                "project.import",
                r#"{"id":1,"ok":true,"imported":{"dest":"/root/app","files":2,"bytes":10,"parts":1,"cut":[],"rewritten":[],"agents":[],"project":{"name":"app","dest":"/root/app","importedAt":"2026-10-25T00:00:00.000Z"}}}"#,
            )],
            &[(
                "project.import",
                &[
                    r#"{"type":"project.import","workspaceId":"ws-1","source":"/me/app","dest":"/root/app","message":"landing"}"#,
                    r#"{"type":"project.import","workspaceId":"ws-1","source":"/me/other","dest":"/root/app","message":"another source"}"#,
                ],
            )],
            None,
        ));
        let client = socket(&host).await;
        let (seen, on) = stages();
        let req: wsp_wire::types::project_import::ProjectImportRequest =
            serde_json::from_value(json!({ "source": "/me/app", "dest": "/root/app", "carry": [], "rewrite": [], "agents": [] })).unwrap();
        import_project(client.as_ref(), "ws-1", &req, on).await.unwrap();
        assert_eq!(texts(&seen), vec!["landing"]);
        let asked = host.asked();
        assert_eq!(asked[0]["op"], "project.import");
        assert_eq!(asked[0]["workspaceId"], "ws-1");
        assert_eq!(asked[0]["source"], "/me/app");
    }

    #[tokio::test]
    async fn an_export_the_host_stops_under_fails_with_the_stopping_line() {
        let host = ScriptedHost::new(script(&[("project.export", EXPORTED)], &[], Some("events.subscribe")));
        let client = socket(&host).await;
        let (_seen, on) = stages();
        let req = ExportRequest { source: "/a".into(), dest: "/b".into(), ..ExportRequest::default() };
        let e = export_project(client.as_ref(), "ws-1", &req, on).await.err().expect("failed");
        assert_eq!(e.message, HOST_STOPPING_LINE);
    }

    #[tokio::test]
    async fn a_plan_is_the_host_s_plan_as_it_answered() {
        let host = ScriptedHost::new(script(
            &[(
                "project.plan",
                r#"{"id":1,"ok":true,"plan":{"source":"/me/app","repo":false,"files":2,"bytes":10,"secrets":[],"excluded":[],"skipped":[],"agents":[]}}"#,
            )],
            &[],
            None,
        ));
        let plan = plan_project(socket(&host).await.as_ref(), "/me/app").await.unwrap();
        assert_eq!(plan.source, "/me/app");
        assert_eq!(host.asked(), vec![json!({ "source": "/me/app", "op": "project.plan" })]);
    }
}

mod folders {
    use super::*;

    const LISTING: &str = r#"{"id":1,"ok":true,"listing":{"dir":"/root","roots":["/root"],"folders":[],"hidden":0}}"#;
    const PLACES: &str = r#"{"id":1,"ok":true,"places":[{"id":"here","kind":"computer","name":"this mac","default":true},{"id":"p-box","kind":"computer","name":"mini","default":false}]}"#;

    fn here(folder: &str) -> Result<String, HostError> {
        Ok(format!("/resolved/{folder}"))
    }

    #[tokio::test]
    async fn a_folder_on_this_computer_is_read_through_here_and_only_true_switches_ride() {
        let host = ScriptedHost::new(script(&[("host.folders", LISTING)], &[], None));
        let asked = FoldersAsked { folder: Some("code".into()), hidden: Some(false), repos: Some(true), on: None };
        host_folders(socket(&host).await.as_ref(), &asked, &here).await.unwrap();
        assert_eq!(serde_json::to_string(&host.asked()).unwrap(), r#"[{"dir":"/resolved/code","repos":true,"op":"host.folders"}]"#);
    }

    #[tokio::test]
    async fn a_box_named_by_on_is_asked_by_its_id_and_its_folder_must_be_absolute() {
        let host = ScriptedHost::new(script(&[("host.folders", LISTING), ("places.list", PLACES)], &[], None));
        let asked = FoldersAsked { folder: Some("/srv".into()), on: Some("mini".into()), ..FoldersAsked::default() };
        host_folders(socket(&host).await.as_ref(), &asked, &here).await.unwrap();
        assert_eq!(
            serde_json::to_string(&host.asked()).unwrap(),
            r#"[{"op":"places.list"},{"dir":"/srv","on":"p-box","op":"host.folders"}]"#
        );
        let host = ScriptedHost::new(script(&[("host.folders", LISTING), ("places.list", PLACES)], &[], None));
        let asked = FoldersAsked { folder: Some("srv".into()), on: Some("mini".into()), ..FoldersAsked::default() };
        let e = host_folders(socket(&host).await.as_ref(), &asked, &here).await.err().expect("refused");
        assert_eq!(e.message, "folder is a path on mini, absolute, and got \"srv\". Give a path that opens with /, since whoever reads it works in a folder this line cannot see.");
        assert_eq!(host.ops(), vec!["places.list"], "refused before the listing is asked");
    }

    #[tokio::test]
    async fn on_naming_this_computer_reads_the_folder_here_and_sends_no_on() {
        let host = ScriptedHost::new(script(&[("host.folders", LISTING), ("places.list", PLACES)], &[], None));
        let asked = FoldersAsked { folder: Some("code".into()), on: Some("here".into()), ..FoldersAsked::default() };
        host_folders(socket(&host).await.as_ref(), &asked, &here).await.unwrap();
        assert_eq!(serde_json::to_string(&host.asked()).unwrap(), r#"[{"op":"places.list"},{"dir":"/resolved/code","op":"host.folders"}]"#);
    }

    #[tokio::test]
    async fn an_unknown_computer_is_refused_with_the_names_there_are() {
        let host = ScriptedHost::new(script(&[("places.list", PLACES)], &[], None));
        let asked = FoldersAsked { on: Some("nope".into()), ..FoldersAsked::default() };
        let e = host_folders(socket(&host).await.as_ref(), &asked, &here).await.err().expect("refused");
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.ends_with(" Run wsp computers."), "{}", e.message);
        assert!(e.message.contains("mini"), "the names there are: {}", e.message);
    }

    #[tokio::test]
    async fn no_folder_sends_no_dir() {
        let host = ScriptedHost::new(script(&[("host.folders", LISTING)], &[], None));
        host_folders(socket(&host).await.as_ref(), &FoldersAsked::default(), &here).await.unwrap();
        assert_eq!(serde_json::to_string(&host.asked()).unwrap(), r#"[{"op":"host.folders"}]"#);
    }
}

mod setup {
    use super::*;

    #[tokio::test]
    async fn the_setup_is_asked_with_an_empty_object_and_a_shape_it_does_not_read_is_an_error() {
        let host = ScriptedHost::new(script(&[("init.get", r#"{"id":1,"ok":true,"setup":{"nonsense":true}}"#)], &[], None));
        let e = init_setup(socket(&host).await.as_ref()).await;
        assert!(e.is_err(), "a setup the schema refuses is not trusted");
        assert_eq!(host.asked(), vec![json!({ "op": "init.get" })]);
    }
}
