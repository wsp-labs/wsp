// SPDX-License-Identifier: AGPL-3.0-only
//! The wsp tools an agent calls from inside its own turn, and which thread a call still in flight is behind. Port of
//! the functions of packages/protocol/src/wsp-tools.ts at the cut (383708c78) (its constants, SLATE_BRIEF among them, are
//! wsp_wire::types::wsp_tools). See spec/thread_state.md.

use wsp_wire::views::harness::McpServerSpec;

// wsp-tools.ts:12
// in wsp_wire::types::wsp_tools: MCP_SERVER_NAME

// The wsp tools that start a thread or a machine for the caller, which a cap can refuse: a lead reads that refusal as
// its call's answer, and the person reads it on the lead's transcript as an error row rather than a folded call.
// wsp-tools.ts:16
// export const SPAWN_TOOLS: readonly string[] = ["run", "fork"];
// in wsp_wire::types::wsp_tools: SPAWN_TOOLS

// wsp-tools.ts:17
// export const spawnsThread = (toolName: string): boolean => {
//   const named = serverTool(toolName);
//   return named !== undefined && named.server === MCP_SERVER_NAME && SPAWN_TOOLS.includes(named.tool);
// };
pub fn spawns_thread(tool_name: &str) -> bool {
    crate::words::tools::server_tool(tool_name).is_some_and(|named| {
        named.server == wsp_wire::types::wsp_tools::MCP_SERVER_NAME
            && wsp_wire::types::wsp_tools::SPAWN_TOOLS.contains(&named.tool.as_str())
    })
}

// The slate's tools, and the name a launch serves them under again for a harness that keeps a server's tools behind
// a search until asked: Codex lists tools up front only per server (omit_tools_from, codex-rs/core/src/tools/
// spec_plan.rs at rust-v0.155.1), and an agent that never searched never found the slate.
// wsp-tools.ts:25
// export const SLATE_TOOLS = ["slate_catalog", "slate_write", "slate_state", "slate_read"] as const;
// in wsp_wire::types::wsp_tools: SLATE_TOOLS

// wsp-tools.ts:26
// in wsp_wire::types::wsp_tools: SLATE_SERVER_NAME

/// Whether a launch gives its thread a slate: its wsp server is there, scoped on this computer or the guest one a box
/// dials, and is not the server of a thread another thread started. Every adapter reads this one rule.
// wsp-tools.ts:30
// export function launchHasSlate(servers: Readonly<Record<string, Pick<McpServerSpec, "noSlate">>> | undefined): boolean {
//   const wsp = servers?.[MCP_SERVER_NAME];
//   return wsp !== undefined && wsp.noSlate !== true;
// }
/// None is the TS undefined.
pub fn launch_has_slate(servers: Option<&indexmap::IndexMap<String, McpServerSpec>>) -> bool {
    servers.and_then(|servers| servers.get(wsp_wire::types::wsp_tools::MCP_SERVER_NAME)).is_some_and(|wsp| wsp.no_slate.is_none())
}

// wsp-tools.ts:37
// in wsp_wire::types::wsp_tools: SLATE_SCRIPT_WORDS

// The slate's rules where a small model weighs them, at the end of the system prompt, for a thread whose launch
// has a slate: in the server's instructions alone, behind the person's own CLAUDE.md and skills,
// Haiku read a token file first and drew in chat. Free of the 2,048 characters an MCP server's instructions keep.
// wsp-tools.ts:42
// export const SLATE_BRIEF = [
//   "wsp slate. This session is a wsp thread with a slate, a live panel beside this conversation that the person sees. Before any other work, decide whether the request belongs on the slate. It does when the person:",
//   "- wants to see, watch, monitor or keep an eye on something, or a result kept fresh or refreshed on a schedule (\"refresh hourly\", \"live\", \"what's unread\");",
//   "- wants a dashboard, a form to fill in, a checklist or walkthrough to tick off, or something drawn: a chart over time, a donut of shares, a timeline of a run's steps, a treemap of sizes or a flow laid out as a diagram;",
//   "- wants to see screenshots or pictures: one image, a strip, a gallery, or a before and after compare, from a path on this computer or an https address;",
//   "- asks for a button or control the project has no UI for: the slate is that UI.",
//   "Then your first tool call is mcp__wsp__slate_catalog, then mcp__wsp__slate_write. Do not draw it with another tool or skill, write an HTML page, or answer in chat instead.",
//   "On the slate, a value that changes comes from a <run>: a cmd, or tool=\"server.tool\" for an MCP tool. Never call that tool yourself and type its numbers in.",
//   SLATE_SCRIPT_WORDS,
//   "A token or key the person keeps in a file stays there: the run's command reads it from the file. Never read that file or put its value in a tool call.",
//   "A one-off answer, a comparison or an explanation stays in chat.",
// ].join("\n");
// in wsp_wire::types::wsp_tools: SLATE_BRIEF

// wsp-tools.ts:60
// in wsp_wire::types::wsp_tools: WSP_TOOL_TIMEOUT_SEC

// Where each wsp call that does not answer until another thread's turn is over names the thread it is behind: the
// input field holding the references, or `opened` for the call that follows the thread it starts, which has no id
// until the call has made one. Adding a verb that blocks is a row here and nothing else.
// wsp-tools.ts:65
// private in the TS: port it as a private helper
// const FOLLOWS: Readonly<Record<string, { readonly field: string } | { readonly opened: true }>> = {
//   send: { field: "thread" },
//   run: { opened: true },
//   threads_wait: { field: "threads" },
// };

/// What a thread's running tool call is waiting behind, read off the call alone: the threads it named, by whatever
/// reference the caller used, or `opened` for a call that follows the thread it started. Nothing for another
/// server's tool, for a wsp verb that answers out of the host alone, and for a detached call, which answers the
/// moment the turn starts and follows nobody.
// wsp-tools.ts:75
// export function threadsFollowed(call: { toolName: string; input: string }): { readonly named: readonly string[] } | { readonly opened: true } | undefined {
//   const named = serverTool(call.toolName);
//   if (named?.server !== MCP_SERVER_NAME) return undefined;
//   const follows = FOLLOWS[named.tool];
//   if (follows === undefined) return undefined;
//   let input: unknown;
//   try {
//     input = JSON.parse(call.input);
//   } catch {
//     return undefined;
//   }
//   if (typeof input !== "object" || input === null) return undefined;
//   const args = input as Record<string, unknown>;
//   if (args["detach"] === true) return undefined;
//   if ("opened" in follows) return follows;
//   const value = args[follows.field];
//   const refs = (Array.isArray(value) ? value : [value]).filter((ref): ref is string => typeof ref === "string" && ref.length > 0);
//   return refs.length === 0 ? undefined : { named: refs };
// }
/// Which threads a call follows: the ones it names, or the one it opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Followed {
    Named(Vec<String>),
    Opened,
}

/// `call` is the tool call's name and its input text.
pub fn threads_followed(tool_name: &str, input: &str) -> Option<Followed> {
    enum Follow {
        Field(&'static str),
        Opened,
    }
    let named = crate::words::tools::server_tool(tool_name)?;
    if named.server != wsp_wire::types::wsp_tools::MCP_SERVER_NAME {
        return None;
    }
    let follows = match named.tool.as_str() {
        "send" => Follow::Field("thread"),
        "run" => Follow::Opened,
        "threads_wait" => Follow::Field("threads"),
        _ => return None,
    };
    // A JSON array is an object to JavaScript's typeof, so it parses and names no field.
    let args = match crate::v8_json::parse_json_lax(input)? {
        serde_json::Value::Object(args) => Some(args),
        serde_json::Value::Array(_) => None,
        _ => return None,
    };
    if args.as_ref().and_then(|args| args.get("detach")) == Some(&serde_json::Value::Bool(true)) {
        return None;
    }
    match follows {
        Follow::Opened => Some(Followed::Opened),
        Follow::Field(field) => {
            let candidates: Vec<&serde_json::Value> = match args.as_ref().and_then(|args| args.get(field)) {
                Some(serde_json::Value::Array(items)) => items.iter().collect(),
                Some(value) => vec![value],
                None => Vec::new(),
            };
            let refs: Vec<String> = candidates
                .into_iter()
                .filter_map(|value| match value {
                    serde_json::Value::String(text) if !text.is_empty() => Some(text.clone()),
                    _ => None,
                })
                .collect();
            if refs.is_empty() {
                None
            } else {
                Some(Followed::Named(refs))
            }
        }
    }
}
