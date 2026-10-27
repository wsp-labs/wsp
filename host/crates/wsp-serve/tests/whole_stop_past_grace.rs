// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/stop-past-grace.test.ts at 65f6b89c7, case for case. A turn stopped while its agent ignores
//! SIGTERM, driven end to end: the runtime, the Claude adapter and a real process on this computer, with a stand-in for
//! the CLI that says one more thing on the TERM and then waits for the KILL. Each test names the TS test above it.
mod common;

use std::path::Path;
use std::sync::Arc;

use common::stubs::{gate_loop, write_stub};
use common::*;
use indexmap::IndexMap;
use wsp_adapters::claude::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::HarnessAdapter;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_wire::types::place_word::HERE_PLACE_ID;

const GRACE_MS: u64 = 300;
/// Slower than the grace, so no poll falls between the TERM and the KILL.
const POLL_MS: u64 = 1_500;

/// The stand-in `claude`: says one line, writes its process group to `mark`, then works until a KILL, saying one more
/// line on the TERM. Any other call (the catalog probe, a title) answers nothing.
fn stand_in(mark: &str) -> String {
    format!(
        r#"#!/bin/sh
for a; do [ "$prev" = --session-id ] && sid=$a; prev=$a; done
[ -n "$sid" ] || exit 0
last() {{
cat <<EOF
{{"type":"assistant","message":{{"model":"claude-opus-5-5","id":"msg_2","type":"message","role":"assistant","content":[{{"type":"text","text":"Stopped with src/a.ts half written."}}]}},"parent_tool_use_id":null,"session_id":"$sid"}}
EOF
}}
trap last TERM
read -r _
cat <<EOF
{{"type":"system","subtype":"init","cwd":"$PWD","session_id":"$sid","tools":["Bash"],"model":"claude-opus-5-5"}}
{{"type":"assistant","message":{{"model":"claude-opus-5-5","id":"msg_1","type":"message","role":"assistant","content":[{{"type":"text","text":"Editing src/a.ts."}}]}},"parent_tool_use_id":null,"session_id":"$sid"}}
EOF
ps -o pgid= -p $$ > '{mark}.tmp' && mv '{mark}.tmp' '{mark}'
{gate}
"#,
        gate = gate_loop(&format!("{mark}.stop"), "", 30)
    )
}

/// The Claude adapter with a grace of 300 ms, as the TS wires it for this test.
fn claude_with_grace() -> HarnessAdapterFactory {
    Arc::new(|ctx| {
        let mut deps = AdapterDeps::new(ctx.exec_stream.clone(), (ctx.home)("claude"));
        deps.base_env = Some(ctx.env.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect());
        deps.interrupt_grace_ms = Some(GRACE_MS);
        let adapter = create_claude_adapter(deps).map_err(|e| wsp_words::exit::HostError::new(e.0))?;
        Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
    })
}

fn group_of(mark: &Path) -> Option<i32> {
    std::fs::read_to_string(mark).ok()?.trim().parse().ok()
}

/// The turn's process group, which outlives the runtime: a red run leaves it waiting on a KILL that never came.
struct Group(std::cell::Cell<Option<i32>>);

impl Drop for Group {
    fn drop(&mut self) {
        if let Some(group) = self.0.get() {
            let _ = std::process::Command::new("kill").args(["-KILL", "--", &format!("-{group}")]).output();
        }
    }
}

// "a turn stopped while its agent ignores SIGTERM" > "keeps in its transcript the line the agent printed after the last poll, before the KILL"
#[test]
fn keeps_in_its_transcript_the_line_the_agent_printed_after_the_last_poll_before_the_kill() {
    drive(|| async {
        let root = tempfile::tempdir().expect("a folder");
        let mark = root.path().join("working");
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).expect("a folder");
        write_stub(&bin.join("claude"), &stand_in(&mark.display().to_string()));
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
        let local = fake_local_with(root.path(), Some(IndexMap::from([("PATH".to_string(), path)])), |o| o.poll_ms = Some(POLL_MS));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", claude_with_grace()).with(move |o| o.local = Some(local)).build();
        let rt = &*fx.rt;
        let group = Group(std::cell::Cell::new(None));
        let ws = create_on(rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace").view.id;
        let handle =
            rt.sessions().start(&ws, StartOptions { prompt: "edit src/a.ts".into(), ..Default::default() }, None).await.expect("a start");
        let said = || async { serde_json::to_string(&rt.sessions().history(&ws, None).await.expect("a history")).expect("JSON") };

        until_for(5_000, &mut || async { mark.exists() }).await;
        group.0.set(group_of(&mark));
        until_for(5_000, &mut || async { said().await.contains("Editing src/a.ts.") }).await;
        let stopped = rt.sessions().interrupt(&handle.id(), None, None).await.expect("an interrupt");
        assert_eq!(j(&stopped)["outcome"], "accepted");
        handle.finished().await;
        assert!(said().await.contains("Stopped with src/a.ts half written."));
        rt.close().await.expect("a close");
    });
}
