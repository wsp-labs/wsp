// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wsp-tools.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/wsp-tools.ts:12 at a3b870a6e.
pub const MCP_SERVER_NAME: &str = "wsp";

/// TS: packages/protocol/src/wsp-tools.ts:16 at a3b870a6e.
pub const SPAWN_TOOLS: &[&str] = &["run", "fork"];

/// TS: packages/protocol/src/wsp-tools.ts:25 at a3b870a6e.
pub const SLATE_TOOLS: &[&str] = &["slate_catalog", "slate_write", "slate_state", "slate_read"];

/// TS: packages/protocol/src/wsp-tools.ts:26 at a3b870a6e.
pub const SLATE_SERVER_NAME: &str = "wsp_slate";

/// TS: packages/protocol/src/wsp-tools.ts:37 at a3b870a6e.
pub const SLATE_SCRIPT_WORDS: &str = "A run's script goes in a <file name=\"x.py\">, run as python3 \"$SLATE_DIR/x.py\", never in the project.";

/// TS: packages/protocol/src/wsp-tools.ts:42 at a3b870a6e.
pub const SLATE_BRIEF: &str = "wsp slate. This session is a wsp thread with a slate, a live panel beside this conversation that the person sees. Before any other work, decide whether the request belongs on the slate. It does when the person:\n- wants to see, watch, monitor or keep an eye on something, or a result kept fresh or refreshed on a schedule (\"refresh hourly\", \"live\", \"what's unread\");\n- wants a dashboard, a form to fill in, a checklist or walkthrough to tick off, or something drawn: a chart over time, a donut of shares, a timeline of a run's steps, a treemap of sizes or a flow laid out as a diagram;\n- wants to see screenshots or pictures: one image, a strip, a gallery, or a before and after compare, from a path on this computer or an https address;\n- asks for a button or control the project has no UI for: the slate is that UI.\nThen your first tool call is mcp__wsp__slate_catalog, then mcp__wsp__slate_write. Do not draw it with another tool or skill, write an HTML page, or answer in chat instead.\nOn the slate, a value that changes comes from a <run>: a cmd, or tool=\"server.tool\" for an MCP tool. Never call that tool yourself and type its numbers in.\nA run's script goes in a <file name=\"x.py\">, run as python3 \"$SLATE_DIR/x.py\", never in the project.\nA token or key the person keeps in a file stays there: the run's command reads it from the file. Never read that file or put its value in a tool call.\nA one-off answer, a comparison or an explanation stays in chat.";

/// TS: packages/protocol/src/wsp-tools.ts:60 at a3b870a6e.
pub const WSP_TOOL_TIMEOUT_SEC: u64 = 3_600;
