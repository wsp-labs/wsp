// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/conversations.ts` at 65f6b89c7: `wsp conversations`, the conversations Claude Code and
//! Codex kept in a project's folder outside wsp. Spec: `spec/activity.md`.

use serde_json::Value;
use wsp_wire::types::conversations::{ConversationOrigin, ConversationsAnswer, OutsideConversation};
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::js::utf16::slice;
use wsp_words::js_date::{fields, time_value, DateZone};
use wsp_words::words::units::fmt_bytes;

use super::client::*;
use super::io::as_json;
use super::turns_help::project_of_folder;
use super::workspaces_help::projects_listed;
use crate::zod_read::zod_read;

/// `projectAsked` (conversations.ts:12-16): the project named, else the one whose folder the line was typed in
/// (`projects.list` read, a failed read taken as no projects), else none.
pub async fn project_asked(client: &dyn HostClient, named: Option<&str>, cwd: Option<&str>) -> Result<Option<String>, HostError> {
    if let Some(named) = named {
        return Ok(Some(named.to_owned()));
    }
    let Some(cwd) = cwd else { return Ok(None) };
    let projects = projects_listed(client).await.unwrap_or_default();
    Ok(project_of_folder(&projects, cwd).and_then(|project| project.get("id").and_then(Value::as_str).map(str::to_owned)))
}

/// `listed` (conversations.ts:18-20): `conversations.list` with `{project?, agent?}`, the whole reply parsed.
async fn listed(client: &dyn HostClient, project: Option<&str>, agent: Option<&str>) -> Result<ConversationsAnswer, HostError> {
    let mut params = Frame::new();
    if let Some(project) = project {
        params.insert("project".to_owned(), Value::from(project));
    }
    if let Some(agent) = agent {
        params.insert("agent".to_owned(), Value::from(agent));
    }
    let reply = client.request("conversations.list", params).await?;
    zod_read::<ConversationsAnswer>(&Value::Object(reply))
}

/// `mark` (conversations.ts:24): what a row says beside its title.
fn mark(row: &OutsideConversation) -> String {
    if let Some(thread) = &row.thread {
        format!("thread {}", slice(thread, 0, Some(8)))
    } else if row.live {
        "open elsewhere".to_owned()
    } else if matches!(row.origin, ConversationOrigin::Wsp) {
        "wsp".to_owned()
    } else {
        String::new()
    }
}

/// `changed` (conversations.ts:22): the last change on the person's clock to the minute, `sv-SE` form
/// `YYYY-MM-DD HH:MM` in the process's local time zone (`TZ`).
pub fn changed(at_ms: f64) -> String {
    let Some(t) = time_value(at_ms) else { return "Invalid Date".to_owned() };
    let f = fields(t, DateZone::system().offset_seconds(t));
    format!("{:04}-{:02}-{:02} {:02}:{:02}", f.year, f.month, f.day, f.hour, f.minute)
}

/// `conversationsLines` (conversations.ts:27-30).
pub fn conversations_lines(answer: &ConversationsAnswer) -> Vec<String> {
    let mut lines = if answer.rows.is_empty() {
        if answer.held.is_empty() {
            vec!["no Claude Code or Codex conversations in this folder".to_owned()]
        } else {
            Vec::new()
        }
    } else {
        let mut cells: Vec<Vec<String>> = vec![["AGENT", "ID", "CHANGED", "SIZE", "BRANCH", "", "TITLE"].map(str::to_owned).to_vec()];
        for r in &answer.rows {
            let size = r.bytes.as_ref().map_or_else(|| "-".to_owned(), |bytes| fmt_bytes(bytes.0));
            cells.push(vec![
                r.agent.clone(),
                r.id.clone(),
                changed(r.last_at.0),
                size,
                r.branch.clone().unwrap_or_else(|| "-".to_owned()),
                mark(r),
                r.title.clone(),
            ]);
        }
        table(&cells)
    };
    lines.extend(answer.held.iter().map(|h| format!("{}. {}", h.said, h.fix)));
    lines
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
        let project = args.get("project").and_then(Value::as_str);
        let agent = args.get("agent").and_then(Value::as_str);
        let answer = listed(&*deps.client(None).await?, project, agent).await?;
        Ok(as_json(serde_json::to_value(&answer).expect("an answer writes as JSON")))
    })
}

/// `wsp conversations` (`verbs/conversations.ts:40-46`).
fn run_conversations(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            let said = format!("wsp conversations takes one project; {} reads as a second.", ctx.args[1]);
            return Err(usage_refusal(&said, &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let project = project_asked(&*client, ctx.args.first().map(String::as_str), ctx.cwd.as_deref()).await?;
        let answer = listed(&*client, project.as_deref(), flag(&ctx.flags, "agent")).await?;
        let line = conversations_lines(&answer).join("\n");
        ctx.out.emit(&serde_json::to_value(&answer).expect("an answer writes as JSON"), Some(&line));
        Ok(0)
    })
}
