// SPDX-License-Identifier: AGPL-3.0-only
//! Review of join A (#2476), must-fix 2 beyond `send_detached`: a step that throws rejects the graph in the TS
//! (setup-graph.ts:49-53, the rejection handler of `s.run(...)`), so a step that panics must end the run with an
//! error, never leave `run_graph` waiting for good.
use std::time::Duration;

use futures::FutureExt;
use wsp_core::setup_graph::{run_graph, GraphStep, StepNext};
use wsp_words::exit::HostError;

#[test]
fn a_step_that_panics_ends_the_graph_and_never_hangs() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async {
        let steps = vec![GraphStep {
            name: "breaks".to_string(),
            after: vec![],
            marks: vec![],
            lanes: vec![],
            light: false,
            run: Box::new(|_release| async { panic!("the step broke") as Result<StepNext, HostError> }.boxed_local()),
        }];
        let answered = tokio::time::timeout(Duration::from_secs(5), run_graph(steps, 1, None)).await;
        let ended = answered.expect("the graph answers, not waits for good");
        assert!(ended.is_err(), "a step that died ends the graph with an error");
    });
}
