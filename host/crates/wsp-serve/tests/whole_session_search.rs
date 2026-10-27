// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-search.test.ts at 65f6b89c7, case for case. Searching the words of every thread the host
//! still holds: the person's messages and the agent's replies, a reply's pieces read as the one message they are, one hit
//! per thread with a snippet around the words, and a thread's token reaching its own tree and nothing else.
mod common;

use common::projects::AgentsDoc;
use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::StartOptions;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};

fn as_thread(thread: &str, ws: &str, root: &str) -> Caller {
    let by: ThreadScope =
        serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": ws, "rootThreadId": root })).expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Relayed, by }
}

fn opts(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn thread_of(h: &std::rc::Rc<dyn wsp_core::types::wiring::SessionHandle>) -> String {
    j(&h.view())["threadId"].as_str().expect("a thread").to_string()
}

async fn hits(rt: &wsp_serve::AssembledRuntime, query: &str, origin: Option<Caller>) -> Vec<Value> {
    let found = rt.sessions().search(query, origin).await.expect("a search");
    j(&found)["hits"].as_array().cloned().unwrap_or_default()
}

// "searching inside messages" > "a word only in a reply finds its thread with a snippet around it, a word in the person's message finds that thread, and case is ignored"
#[test]
fn a_word_only_in_a_reply_finds_its_thread_with_a_snippet_around_it_a_word_in_the_persons_message_finds_that_thread_and_case_is_ignored() {
    drive(|| async {
        let redirect_reply = "Done. The redirect now sends every old path to the canonical host and keeps the query string.";
        let replying = Scripted::new().cwd("/root/app").pieces(2).reply(move |p| match p {
            "fix the redirect" => redirect_reply.to_string(),
            "tidy the readme" => "Rewrote the install section.".to_string(),
            _ => "ok".to_string(),
        });
        let fx = runtime(&StubBackend::new(None)).adapter("claude", replying.factory()).build();
        let rt = fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let redirect = rt.sessions().start(&ws, opts("fix the redirect"), None).await.expect("a start");
        redirect.finished().await;
        let readme = rt.sessions().start(&ws, opts("tidy the readme"), None).await.expect("a start");
        readme.finished().await;

        let found = hits(&rt, "CANONICAL", None).await;
        assert_eq!(found.len(), 1);
        assert_matches(&found[0], json!({ "workspaceId": ws, "threadId": thread_of(&redirect) }));
        assert!(found[0]["snippet"].as_str().unwrap_or_default().contains("canonical host"));
        // The words around a hit are its own message's, never the tail of the person's message run into the reply.
        assert_eq!(
            hits(&rt, "rewrote", None).await.iter().map(|h| h["snippet"].clone()).collect::<Vec<_>>(),
            [json!("Rewrote the install section.")]
        );

        // The split between the reply's two pieces falls inside no word the search can miss.
        let chars: Vec<char> = redirect_reply.chars().collect();
        let mid = chars.len() / 2;
        let across: String = chars[mid - 4..mid + 4].iter().collect();
        assert_eq!(hits(&rt, &across, None).await.iter().map(|h| h["threadId"].clone()).collect::<Vec<_>>(), [json!(thread_of(&redirect))]);

        assert_eq!(hits(&rt, "readme", None).await.iter().map(|h| h["threadId"].clone()).collect::<Vec<_>>(), [json!(thread_of(&readme))]);
        assert!(hits(&rt, "   ", None).await.is_empty());
        assert!(hits(&rt, "nowhere at all", None).await.is_empty());
        rt.close().await.expect("a close");
    });
}

// "searching inside messages" > "a thread's token finds its own tree's words and nothing another tree on the same workspace said"
#[test]
fn a_threads_token_finds_its_own_trees_words_and_nothing_another_tree_on_the_same_workspace_said() {
    drive(|| async {
        let replying = Scripted::new().cwd("/root/app").pieces(2).reply(|p| format!("reply to {p}: zebra"));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", replying.factory()).build();
        let rt = fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("a").agents(json!({ "maxMachines": 3, "maxDepth": 3 })), None)
            .await
            .expect("a workspace")
            .view
            .id;
        rt.sessions().start(&ws, opts("the person's own"), None).await.expect("a start").finished().await;
        let lead = rt.sessions().start(&ws, opts("lead"), None).await.expect("a start");
        lead.finished().await;
        let root = thread_of(&lead);
        let child = rt.sessions().start(&ws, opts("child"), Some(as_thread(&root, &ws, &root))).await.expect("a start");
        child.finished().await;
        let elsewhere = create_on(&rt, CreateOn::golden("b"), None).await.expect("a workspace").view.id;
        rt.sessions().start(&elsewhere, opts("not yours"), None).await.expect("a start").finished().await;

        let mut scoped: Vec<String> = hits(&rt, "zebra", Some(as_thread(&root, &ws, &root)))
            .await
            .iter()
            .map(|h| h["threadId"].as_str().unwrap_or_default().to_string())
            .collect();
        scoped.sort();
        let mut want = vec![root.clone(), thread_of(&child)];
        want.sort();
        assert_eq!(scoped, want);
        // The person reads every thread, as they always did.
        assert_eq!(hits(&rt, "zebra", None).await.len(), 4);
        rt.close().await.expect("a close");
    });
}

// "searching inside messages" > "answers in under a second over three hundred threads"
#[test]
fn answers_in_under_a_second_over_three_hundred_threads() {
    drive(|| async {
        let replying = Scripted::new().cwd("/root/app").pieces(2).reply(|p| {
            format!("A long reply about {p}, with enough words in it to look like a real answer from an agent that did some work.")
        });
        let fx = runtime(&StubBackend::new(None)).adapter("claude", replying.factory()).build();
        let rt = fx.rt.clone();
        let mut spaces = vec![];
        for n in 0..3 {
            spaces.push(create_on(&rt, CreateOn::golden(&format!("w{n}")), None).await.expect("a workspace").view.id);
        }
        for n in 0..300 {
            rt.sessions().start(&spaces[n % 3], opts(&format!("task number {n}")), None).await.expect("a start").finished().await;
        }
        rt.sessions().start(&spaces[1], opts("the odd one"), None).await.expect("a start").finished().await;

        let started = std::time::Instant::now();
        let found = hits(&rt, "about the odd one", None).await;
        let took = started.elapsed();
        assert_eq!(found.len(), 1);
        assert!(took < std::time::Duration::from_secs(1), "{took:?}");
        rt.close().await.expect("a close");
    });
}
