// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/setup-graph.ts at 65f6b89c7: the steps of a setup after the base tools, run as their
//! work allows. Spec: spec/setup_steps.md.
use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::Hash;
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use tokio::task::spawn_local;
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
    let (done, answer) = oneshot::channel();
    let names = steps.iter().flat_map(|s| std::iter::once(s.name.clone()).chain(s.marks.iter().cloned())).collect();
    let pass = Rc::new(RefCell::new(Pass {
        names,
        ended: HashSet::new(),
        running: IndexMap::new(),
        waiting: steps,
        width,
        stopped: false,
        failure: None,
        on_move,
        done: Some(done),
    }));
    pump(&pass);
    match answer.await {
        Ok(result) => result,
        Err(_) => std::future::pending().await,
    }
}

struct Running<N> {
    lanes: Vec<String>,
    light: bool,
    marks: Vec<N>,
}

struct Pass<N> {
    names: HashSet<N>,
    ended: HashSet<N>,
    running: IndexMap<N, Running<N>>,
    waiting: Vec<GraphStep<N>>,
    width: usize,
    stopped: bool,
    failure: Option<HostError>,
    on_move: Option<Rc<dyn Fn(&[N])>>,
    done: Option<oneshot::Sender<Result<Vec<N>, HostError>>>,
}

impl<N: Clone + Eq + Hash> Pass<N> {
    fn ready(&self, s: &GraphStep<N>) -> bool {
        let after_ended = s.after.iter().all(|a| self.ended.contains(a) || !self.names.contains(a));
        let lane_free = !s.lanes.iter().any(|lane| self.running.values().any(|r| r.lanes.contains(lane)));
        let heavy = self.running.values().filter(|r| !r.light).count();
        after_ended && lane_free && (s.light || heavy < self.width)
    }
}

fn pump<N>(pass: &Rc<RefCell<Pass<N>>>)
where
    N: Clone + Eq + Hash + 'static,
{
    let mut at = 0;
    loop {
        let (step, names, on_move) = {
            let mut p = pass.borrow_mut();
            if p.stopped || at >= p.waiting.len() {
                break;
            }
            if !p.ready(&p.waiting[at]) {
                at += 1;
                continue;
            }
            let step = p.waiting.remove(at);
            p.running.insert(step.name.clone(), Running { lanes: step.lanes.clone(), light: step.light, marks: step.marks.clone() });
            let names: Vec<N> = p.running.keys().cloned().collect();
            (step, names, p.on_move.clone())
        };
        if let Some(on_move) = &on_move {
            on_move(&names);
        }
        let name = step.name.clone();
        let release = release_for(pass.clone(), step.marks.clone());
        let run = (step.run)(release);
        let shared = pass.clone();
        spawn_local(async move {
            let next = run.await;
            finish(&shared, name, next);
        });
    }
    let mut p = pass.borrow_mut();
    if !p.running.is_empty() {
        return;
    }
    if let Some(done) = p.done.take() {
        let answer = match p.failure.take() {
            Some(error) => Err(error),
            None => Ok(p.waiting.iter().map(|s| s.name.clone()).collect()),
        };
        let _ = done.send(answer);
    }
}

fn release_for<N>(pass: Rc<RefCell<Pass<N>>>, marks: Vec<N>) -> Release<N>
where
    N: Clone + Eq + Hash + 'static,
{
    Rc::new(move |mark: N| {
        {
            let mut p = pass.borrow_mut();
            if !marks.contains(&mark) || p.ended.contains(&mark) {
                return;
            }
            p.ended.insert(mark);
        }
        let again = pass.clone();
        spawn_local(async move { pump(&again) });
    })
}

fn finish<N>(pass: &Rc<RefCell<Pass<N>>>, name: N, next: Result<StepNext, HostError>)
where
    N: Clone + Eq + Hash + 'static,
{
    let on_move = {
        let mut p = pass.borrow_mut();
        let marks = p.running.shift_remove(&name).map(|r| r.marks).unwrap_or_default();
        p.ended.insert(name);
        p.ended.extend(marks);
        match next {
            Ok(StepNext::Go) => {}
            Ok(StepNext::Stop) => p.stopped = true,
            Err(error) => {
                p.failure.get_or_insert(error);
                p.stopped = true;
            }
        }
        p.on_move.clone().map(|f| (f, p.running.keys().cloned().collect::<Vec<N>>()))
    };
    if let Some((on_move, names)) = on_move {
        on_move(&names);
    }
    pump(pass);
}
