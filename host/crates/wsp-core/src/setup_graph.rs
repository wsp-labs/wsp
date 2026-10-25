// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/setup-graph.ts at 65f6b89c7: the steps of a setup after the base tools, run as their
//! work allows. Spec: spec/setup_steps.md.
use std::hash::Hash;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use wsp_words::exit::HostError;

/// What a step answers: go on, or start nothing more and let what runs end. setup-graph.ts:18.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepNext {
    Go,
    Stop,
}

/// Lets one of the step's own marks go; a mark the step does not hold, or one already let go, does nothing.
pub type Release<N> = Rc<dyn Fn(N)>;

/// One step. Step names and mark names share one type, as the TS's `after: (S | M)[]` does. setup-graph.ts:7.
pub struct GraphStep<N> {
    pub name: N,
    /// The steps and marks this one waits on; a name no step in the graph is or marks counts as ended.
    pub after: Vec<N>,
    /// The parts of its work another step may wait on alone; empty is none.
    pub marks: Vec<N>,
    /// What it must hold alone while it runs.
    pub lanes: Vec<String>,
    /// Holds no room of the width.
    pub light: bool,
    /// Called once, when the step starts, with its release; the future answers whether the run goes on.
    pub run: Box<dyn FnOnce(Release<N>) -> LocalBoxFuture<'static, Result<StepNext, HostError>>>,
}

/// Runs the steps, in the order given wherever two could start, at most `width` that are not light at once, never two
/// on one lane; answers the names of the steps that never started, in the order given. A step that fails stops the run
/// as a `Stop` does, and the first such error is the answer once every running step has ended. `on_move` hears the
/// running steps' names, in the order they started, after every start and every end. setup-graph.ts:23.
pub async fn run_graph<N>(steps: Vec<GraphStep<N>>, width: usize, on_move: Option<Rc<dyn Fn(&[N])>>) -> Result<Vec<N>, HostError>
where
    N: Clone + Eq + Hash + 'static,
{
    todo!()
}
