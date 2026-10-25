// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/conversations.ts` at 65f6b89c7: `wsp conversations`, the conversations Claude Code and
//! Codex kept in a project's folder outside wsp. Spec: `spec/activity.md`.

use wsp_wire::types::conversations::ConversationsAnswer;
use wsp_words::exit::HostError;

use super::client::*;

/// `projectAsked` (conversations.ts:12-16): the project named, else the one whose folder the line was typed in
/// (`projects.list` read, a failed read taken as no projects), else none.
pub async fn project_asked(client: &dyn HostClient, named: Option<&str>, cwd: Option<&str>) -> Result<Option<String>, HostError> {
    let _ = (client, named, cwd);
    todo!("conversations.ts:12-16")
}

/// `changed` (conversations.ts:22): the last change on the person's clock to the minute, `sv-SE` form
/// `YYYY-MM-DD HH:MM` in the process's local time zone (`TZ`).
pub fn changed(at_ms: f64) -> String {
    let _ = at_ms;
    todo!("conversations.ts:22")
}

/// `conversationsLines` (conversations.ts:27-30).
pub fn conversations_lines(answer: &ConversationsAnswer) -> Vec<String> {
    let _ = answer;
    todo!("conversations.ts:27-30")
}

/// The entries of `verbs/conversations.ts`, in the TS table's order (its order is `tools/list`'s).
pub fn conversation_verbs() -> Vec<Verb> {
    vec![
    // verbs/conversations.ts:34
    Verb::Cli(CliVerb {
        name: "conversations",
        usage: "wsp conversations [<project>] [--agent <id>]".to_string(),
        about: "the conversations Claude Code and Codex kept in the project's folder and its worktrees outside wsp, newest first, each with its id, last change, size, branch and title, and whether another app holds it open; wsp run --resume <id> opens a thread on one".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_conversations,
        tool: Tool {
            description: "The conversations the agents kept in a project's folder and its worktrees on the project's computer outside wsp, Claude Code's off its transcripts and Codex's off its own app server, newest first: agent, id, title (the person's name for it, else the agent's, else its last or first prompt), firstPrompt, branch, cwd, lastAt (ms), bytes, origin (terminal, wsp or script), live (another app holds it open now), thread (the wsp thread already on it, which is the one to send into) and letsGo (where the agent refuses a second writer: how soon it lets go of one whose window closed, in its own sentence). A Claude Code conversation a script ran is left out unless it is a wsp thread's. held names an agent whose store did not answer, in two halves. run with resume opens a thread on one; one that is live needs copy.".to_string(),
            input: vec![
                ToolInput { name: "project", kind: InputKind::String, optional: true, describe: Some("the project, by the name or the id projects lists; absent from a thread, the thread's own project".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: true, describe: Some("one agent's conversations alone, one of claude, codex, opencode, cursor; absent lists every agent's".to_string()) },
            ],
            output: vec!["rows", "held"],
            stream: vec![],
            call: call_conversations,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    ]
}

/// The `conversations` tool's call (`verbs/conversations.ts:54-57`).
fn call_conversations(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/conversations.ts:54-57")
    })
}

/// `wsp conversations` (`verbs/conversations.ts:40-46`).
fn run_conversations(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/conversations.ts:40-46")
    })
}
