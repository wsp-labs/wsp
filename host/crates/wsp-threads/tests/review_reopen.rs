// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448): the re-open after a host restart. turns.ts:791-792 reads the adapter's attach road
//! (`adapter.attach?.bind(adapter)`) and answers "cannot" before it reads the transcript (:799-804); boot.ts:531 then
//! cuts the row with RESTARTED_REASON. The port learns attach only by calling it, after the transcript read.
#[path = "support_a/mod.rs"]
mod support;

use std::rc::Rc;
use std::sync::Arc;

use support::*;

/// An adapter with no attach road (a kind whose exec factory has no `attach`, as cursor and opencode build it).
struct NoAttach;

impl HarnessAdapter for NoAttach {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("unused".into()))
    }
    fn steers(&self) -> bool {
        false
    }
}

fn left_running() -> Row {
    let doc = json!({ "threadId": "t1", "claudeSessionId": "sess-1", "startedAt": T0 - 60_000, "cwd": "/work/w1", "prompt": "fix the upload test" });
    let left = row(view("sess-1", "w1", doc), "turn-1");
    left.borrow_mut().run = Some("run-7".into());
    left
}

#[test]
fn an_adapter_with_no_attach_cannot_reopen_even_when_the_transcript_does_not_read() {
    run_local(async {
        let host = FakeHost::new();
        host.add_entry(entry("w1", json!({})));
        let adapter: Arc<dyn HarnessAdapter> = Arc::new(NoAttach);
        host.hooks.set::<dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>>(
            "TurnsCtx::adapter_for",
            Rc::new(move |_e: &Entry, a: AdapterAsk| {
                Ok(BuiltAdapter { harness: a.named.clone().unwrap_or_default(), adapter: adapter.clone() })
            }),
        );
        host.hooks.set::<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>>(
            "TurnsCtx::open_thread",
            Rc::new(|_w: &str, _t: &str| async { Err(HostError::new("the transcript of w1 does not parse")) }.boxed_local()),
        );
        let warned = wsp_core::console::capture();
        let answer = TurnsArea::new(host.clone()).reattach(left_running(), false).await;
        assert_eq!(answer, Reopened::Cannot, "turns.ts:792 answers cannot before the transcript is read, so boot cuts the row");
        assert_eq!(host.hooks.called("open_thread"), Vec::<String>::new(), "nothing is read for a harness that cannot attach");
        assert_eq!(warned.lines(), Vec::<String>::new());
    });
}
