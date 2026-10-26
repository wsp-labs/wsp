// SPDX-License-Identifier: AGPL-3.0-only
//! Each adapter as the runtime holds it, an `Arc<dyn HarnessAdapter>`, and its capability answers against the TS adapter
//! objects: adapter-claude adapter.ts:1420-1440, adapter-codex adapter.ts:1392-1454, adapter-opencode adapter.ts:278-304,
//! adapter-cursor adapter.ts:242-264.

#[path = "common/claude_exec.rs"]
mod claude_exec;
#[path = "common/codex_scripted.rs"]
mod codex_scripted;
#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::sync::Arc;

use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::codex::{create_codex_adapter, CodexAdapterDeps};
use wsp_adapters::cursor::adapter::{create_cursor_adapter, CursorAdapterDeps};
use wsp_adapters::opencode::adapter::{create_opencode_adapter, OpenCodeAdapterDeps};
use wsp_adapters::port::{AttachmentRoad, HarnessAdapter};

use claude_exec::{FakeExec, StreamOpts};
use codex_scripted::{scripted_exec, ScriptOpts};
use codex_wire::{launcher, scripted_server, WireOpts};

const LOGIN: &str = "codex login --device-auth";

#[derive(Debug, PartialEq)]
struct Answers {
    steers: bool,
    warms: bool,
    renames: bool,
    asides: bool,
    reverts: bool,
    forks: bool,
    attachments: Option<AttachmentRoad>,
    mcp_servers: bool,
}

fn answers(adapter: Arc<dyn HarnessAdapter>) -> Answers {
    Answers {
        steers: adapter.steers(),
        warms: adapter.warms(),
        renames: adapter.renames(),
        asides: adapter.asides(),
        reverts: adapter.reverts(),
        forks: adapter.forks(),
        attachments: adapter.attachments(),
        mcp_servers: adapter.mcp_servers(),
    }
}

#[test]
fn claude_answers_as_its_ts_object_declares() {
    let exec = FakeExec::scripted(Vec::new(), StreamOpts::default());
    let adapter = create_claude_adapter(AdapterDeps::new(exec, "/root/.claude-cfg")).unwrap();
    let want = Answers {
        steers: true,
        warms: true,
        renames: true,
        asides: true,
        reverts: false,
        forks: true,
        attachments: Some(AttachmentRoad::Inline),
        mcp_servers: true,
    };
    assert_eq!(answers(Arc::new(adapter)), want);
}

#[test]
fn codex_answers_as_its_ts_object_declares() {
    let launch = launcher(scripted_server(WireOpts::default()));
    let adapter = create_codex_adapter(CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN)).unwrap();
    let want = Answers {
        steers: true,
        warms: false,
        renames: true,
        asides: true,
        reverts: true,
        forks: true,
        attachments: Some(AttachmentRoad::File),
        mcp_servers: true,
    };
    assert_eq!(answers(Arc::new(adapter)), want);
}

#[test]
fn opencode_answers_as_its_ts_object_declares() {
    let exec = scripted_exec(Vec::new(), ScriptOpts::default(), "/tmp/wsp-run/oc01");
    let adapter = create_opencode_adapter(OpenCodeAdapterDeps::new(exec.factory()));
    let want = Answers {
        steers: false,
        warms: false,
        renames: false,
        asides: false,
        reverts: false,
        forks: false,
        attachments: Some(AttachmentRoad::File),
        mcp_servers: false,
    };
    assert_eq!(answers(Arc::new(adapter)), want);
}

#[test]
fn cursor_answers_as_its_ts_object_declares() {
    let exec = scripted_exec(Vec::new(), ScriptOpts::default(), "/tmp/wsp-run/cu01");
    let adapter = create_cursor_adapter(CursorAdapterDeps::new(exec.factory()));
    let want = Answers {
        steers: false,
        warms: false,
        renames: false,
        asides: false,
        reverts: false,
        forks: false,
        attachments: None,
        mcp_servers: false,
    };
    assert_eq!(answers(Arc::new(adapter)), want);
}

// adapter.attach !== undefined where the exec factory has an attach (adapter-claude adapter.ts:1426, adapter-codex :1397).
#[test]
fn an_adapter_attaches_only_where_its_exec_factory_does() {
    let plain =
        create_claude_adapter(AdapterDeps::new(FakeExec::scripted(Vec::new(), StreamOpts::default()), "/root/.claude-cfg")).unwrap();
    assert!(!HarnessAdapter::attaches(&plain));
    let attaching = create_claude_adapter(AdapterDeps::new(FakeExec::attaching(Vec::new(), false), "/root/.claude-cfg")).unwrap();
    assert!(HarnessAdapter::attaches(&attaching));
}

// adapter-codex adapter.ts:233 declares sessionHeld; the others declare none.
#[test]
fn only_codex_holds_a_session() {
    let claude =
        create_claude_adapter(AdapterDeps::new(FakeExec::scripted(Vec::new(), StreamOpts::default()), "/root/.claude-cfg")).unwrap();
    assert!(!HarnessAdapter::holds_session(&claude));
    let launch = launcher(scripted_server(WireOpts::default()));
    let codex = create_codex_adapter(CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN)).unwrap();
    assert!(HarnessAdapter::holds_session(&codex));
}
