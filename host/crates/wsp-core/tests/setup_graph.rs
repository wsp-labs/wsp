// SPDX-License-Identifier: AGPL-3.0-only
//! spec/setup_steps.md, the graph half; ported from packages/runtime/test/setup-graph.test.ts (7 cases) plus the
//! cases the spec pins: the order inside one pump, width 0, a lane shared three ways, `on_move`'s every call, a
//! release after the step ended, and a failure after a stop.
mod rest;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use futures::FutureExt;
use rest::*;
use tokio::sync::oneshot;
use wsp_core::setup_graph::{run_graph, GraphStep, Release, StepNext};
use wsp_words::exit::HostError;

#[derive(Default, Clone)]
struct Spec {
    name: &'static str,
    after: Vec<&'static str>,
    marks: Vec<&'static str>,
    lanes: Vec<&'static str>,
    light: bool,
    stop: bool,
    throws: bool,
}

fn spec(name: &'static str) -> Spec {
    Spec { name, ..Spec::default() }
}

impl Spec {
    fn after(mut self, a: &[&'static str]) -> Self {
        self.after = a.to_vec();
        self
    }
    fn marks(mut self, m: &[&'static str]) -> Self {
        self.marks = m.to_vec();
        self
    }
    fn lanes(mut self, l: &[&'static str]) -> Self {
        self.lanes = l.to_vec();
        self
    }
    fn light(mut self) -> Self {
        self.light = true;
        self
    }
    fn stops(mut self) -> Self {
        self.stop = true;
        self
    }
    fn throws(mut self) -> Self {
        self.throws = true;
        self
    }
}

/// Steps each held until the test lets it go, recording the order they started in (setup-graph.test.ts:8).
struct Held {
    started: Rc<RefCell<Vec<&'static str>>>,
    lets: Rc<RefCell<HashMap<&'static str, oneshot::Sender<()>>>>,
    marking: Rc<RefCell<HashMap<&'static str, Release<&'static str>>>>,
}

impl Held {
    async fn let_go(&self, name: &str) {
        settle().await;
        let tx = self.lets.borrow_mut().remove(name).unwrap_or_else(|| panic!("{name} is not running"));
        tx.send(()).unwrap();
        settle().await;
    }
    async fn mark(&self, name: &str, mark: &'static str) {
        let release = self.marking.borrow().get(name).cloned().unwrap_or_else(|| panic!("{name} never started"));
        release(mark);
        settle().await;
    }
    fn started(&self) -> Vec<&'static str> {
        self.started.borrow().clone()
    }
}

fn held(specs: Vec<Spec>) -> (Held, Vec<GraphStep<&'static str>>) {
    let h = Held { started: Rc::default(), lets: Rc::default(), marking: Rc::default() };
    let steps = specs
        .into_iter()
        .map(|s| {
            let (started, lets, marking) = (h.started.clone(), h.lets.clone(), h.marking.clone());
            let (name, stop, throws) = (s.name, s.stop, s.throws);
            GraphStep {
                name,
                after: s.after,
                marks: s.marks,
                lanes: s.lanes.iter().map(|l| l.to_string()).collect(),
                light: s.light,
                run: Box::new(move |release: Release<&'static str>| {
                    started.borrow_mut().push(name);
                    marking.borrow_mut().insert(name, release);
                    let (tx, rx) = oneshot::channel();
                    lets.borrow_mut().insert(name, tx);
                    async move {
                        rx.await.unwrap();
                        if throws {
                            return Err(HostError::new(format!("{name} broke")));
                        }
                        Ok(if stop { StepNext::Stop } else { StepNext::Go })
                    }
                    .boxed_local()
                }),
            }
        })
        .collect();
    (h, steps)
}

type Done = Rc<RefCell<Option<Result<Vec<&'static str>, HostError>>>>;

fn spawn_graph(steps: Vec<GraphStep<&'static str>>, width: usize, on_move: Option<Rc<dyn Fn(&[&'static str])>>) -> Done {
    let done: Done = Rc::default();
    let out = done.clone();
    tokio::task::spawn_local(async move {
        let r = run_graph(steps, width, on_move).await;
        *out.borrow_mut() = Some(r);
    });
    done
}

fn answered(done: &Done) -> Option<Result<Vec<&'static str>, String>> {
    done.borrow().as_ref().map(|r| r.clone().map_err(|e| e.message.clone()))
}

// TS: setup-graph.test.ts:39
#[test]
fn start_together_up_to_the_width_each_once_what_it_waits_on_ended_never_two_on_one_lane() {
    run_local(async {
        let (g, steps) = held(vec![
            spec("agents").lanes(&["installs"]),
            spec("skills").lanes(&["files"]),
            spec("clis").after(&["agents"]).lanes(&["installs"]),
            spec("mcp").after(&["agents", "clis"]).lanes(&["files"]),
            spec("folders"),
        ]);
        let moves: Rc<RefCell<Vec<Vec<&'static str>>>> = Rc::default();
        let m = moves.clone();
        let done = spawn_graph(steps, 2, Some(Rc::new(move |now: &[&'static str]| m.borrow_mut().push(now.to_vec()))));
        settle().await;
        assert_eq!(g.started(), ["agents", "skills"]);
        g.let_go("skills").await;
        assert_eq!(g.started(), ["agents", "skills", "folders"]);
        g.let_go("agents").await;
        assert_eq!(g.started(), ["agents", "skills", "folders", "clis"]);
        g.let_go("folders").await;
        g.let_go("clis").await;
        assert_eq!(g.started().last(), Some(&"mcp"));
        g.let_go("mcp").await;
        assert_eq!(answered(&done), Some(Ok(vec![])));
        assert_eq!(moves.borrow().iter().map(|m| m.len()).max(), Some(2));
        assert_eq!(moves.borrow().last(), Some(&vec![]));
    });
}

// TS: setup-graph.test.ts:65
#[test]
fn runs_one_at_a_time_at_width_one_and_a_light_step_takes_no_room() {
    run_local(async {
        let (g, steps) = held(vec![spec("a"), spec("b"), spec("signins").light()]);
        let done = spawn_graph(steps, 1, None);
        settle().await;
        assert_eq!(g.started(), ["a", "signins"]);
        g.let_go("signins").await;
        g.let_go("a").await;
        assert_eq!(g.started(), ["a", "signins", "b"]);
        g.let_go("b").await;
        assert_eq!(answered(&done), Some(Ok(vec![])));
    });
}

// TS: setup-graph.test.ts:77
#[test]
fn starts_nothing_more_once_a_step_says_stop_lets_the_running_end_and_answers_what_never_started() {
    run_local(async {
        let (g, steps) = held(vec![spec("agents").stops(), spec("skills"), spec("clis").after(&["agents"])]);
        let done = spawn_graph(steps, 2, None);
        g.let_go("agents").await;
        assert_eq!(g.started(), ["agents", "skills"]);
        assert_eq!(answered(&done), None, "skills still runs");
        g.let_go("skills").await;
        assert_eq!(answered(&done), Some(Ok(vec!["clis"])));
    });
}

// TS: setup-graph.test.ts:86
#[test]
fn starts_a_step_waiting_on_a_mark_once_the_step_holding_it_lets_it_go_while_that_step_still_runs() {
    run_local(async {
        let (g, steps) = held(vec![spec("clis").marks(&["gh"]), spec("github").after(&["gh"]), spec("folders").after(&["github"])]);
        let done = spawn_graph(steps, 3, None);
        settle().await;
        assert_eq!(g.started(), ["clis"]);
        g.mark("clis", "other").await;
        assert_eq!(g.started(), ["clis"], "a mark the step does not hold lets nothing go");
        g.mark("clis", "gh").await;
        assert_eq!(g.started(), ["clis", "github"]);
        g.let_go("github").await;
        assert_eq!(g.started(), ["clis", "github", "folders"]);
        g.let_go("folders").await;
        g.let_go("clis").await;
        assert_eq!(answered(&done), Some(Ok(vec![])));
    });
}

// TS: setup-graph.test.ts:107
#[test]
fn lets_a_steps_marks_go_at_its_end_where_it_never_let_them_go_itself_and_a_mark_no_step_holds_counts_as_ended() {
    run_local(async {
        let (g, steps) = held(vec![spec("clis").marks(&["gh"]), spec("github").after(&["gh"]), spec("docs").after(&["nobody"])]);
        let done = spawn_graph(steps, 3, None);
        settle().await;
        assert_eq!(g.started(), ["clis", "docs"]);
        g.let_go("clis").await;
        assert_eq!(g.started(), ["clis", "docs", "github"]);
        g.let_go("docs").await;
        g.let_go("github").await;
        assert_eq!(answered(&done), Some(Ok(vec![])));
    });
}

// TS: setup-graph.test.ts:119
#[test]
fn answers_a_step_waiting_on_a_mark_as_never_started_where_the_step_holding_it_never_ran() {
    run_local(async {
        let (g, steps) = held(vec![spec("agents").stops(), spec("clis").after(&["agents"]).marks(&["gh"]), spec("github").after(&["gh"])]);
        let done = spawn_graph(steps, 3, None);
        g.let_go("agents").await;
        assert_eq!(g.started(), ["agents"]);
        assert_eq!(answered(&done), Some(Ok(vec!["clis", "github"])));
    });
}

// TS: setup-graph.test.ts:126
#[test]
fn throws_the_first_error_once_every_running_step_ended() {
    run_local(async {
        let (g, steps) = held(vec![spec("a").throws(), spec("b"), spec("c").after(&["a"])]);
        let done = spawn_graph(steps, 2, None);
        g.let_go("a").await;
        assert_eq!(answered(&done), None, "b still runs");
        g.let_go("b").await;
        assert_eq!(answered(&done), Some(Err("a broke".to_string())));
        assert_eq!(g.started(), ["a", "b"]);
    });
}

// spec: G1, G3 (every step that can start in one pump starts in the order given, each run called at once)
#[test]
fn one_pump_starts_every_ready_step_in_the_order_given_and_skips_one_on_a_busy_lane() {
    run_local(async {
        let (g, steps) = held(vec![
            spec("a").lanes(&["x"]),
            spec("b").lanes(&["x"]),
            spec("c").lanes(&["y"]),
            spec("d").lanes(&["x", "z"]),
            spec("e").lanes(&["z"]),
        ]);
        let done = spawn_graph(steps, 5, None);
        settle().await;
        assert_eq!(g.started(), ["a", "c", "e"]);
        g.let_go("a").await;
        assert_eq!(g.started(), ["a", "c", "e", "b"], "b is before d in the order given, and d's z is still held by e");
        g.let_go("e").await;
        g.let_go("b").await;
        assert_eq!(g.started(), ["a", "c", "e", "b", "d"]);
        g.let_go("c").await;
        g.let_go("d").await;
        assert_eq!(answered(&done), Some(Ok(vec![])));
    });
}

// spec: G2 (width counts steps that are not light; width 0 starts none of them)
#[test]
fn width_zero_starts_only_light_steps_and_answers_the_rest_as_never_started() {
    run_local(async {
        let (g, steps) = held(vec![spec("a"), spec("ask").light(), spec("b")]);
        let done = spawn_graph(steps, 0, None);
        settle().await;
        assert_eq!(g.started(), ["ask"]);
        g.let_go("ask").await;
        assert_eq!(answered(&done), Some(Ok(vec!["a", "b"])));
    });
}

// spec: G2 (no steps at all answers at once)
#[test]
fn no_steps_answers_nothing_never_started_at_once() {
    run_local(async {
        let (_g, steps) = held(vec![]);
        let calls: Rc<RefCell<Vec<Vec<&'static str>>>> = Rc::default();
        let c = calls.clone();
        let done = spawn_graph(steps, 2, Some(Rc::new(move |now: &[&'static str]| c.borrow_mut().push(now.to_vec()))));
        settle().await;
        assert_eq!(answered(&done), Some(Ok(vec![])));
        assert_eq!(*calls.borrow(), Vec::<Vec<&str>>::new(), "nothing started, nothing moved");
    });
}

// spec: G5 (on_move hears the running names in start order after every start and every end)
#[test]
fn on_move_hears_every_start_and_every_end_with_the_running_steps_in_start_order() {
    run_local(async {
        let (g, steps) = held(vec![spec("a"), spec("b"), spec("c").after(&["a"])]);
        let calls: Rc<RefCell<Vec<Vec<&'static str>>>> = Rc::default();
        let c = calls.clone();
        let done = spawn_graph(steps, 2, Some(Rc::new(move |now: &[&'static str]| c.borrow_mut().push(now.to_vec()))));
        settle().await;
        g.let_go("a").await;
        g.let_go("b").await;
        g.let_go("c").await;
        assert_eq!(answered(&done), Some(Ok(vec![])));
        assert_eq!(*calls.borrow(), vec![vec!["a"], vec!["a", "b"], vec!["b"], vec!["b", "c"], vec!["c"], vec![]]);
    });
}

// spec: G4 (a release after the step ended, or of a mark already let go, does nothing)
#[test]
fn a_release_after_its_step_ended_starts_nothing_twice() {
    run_local(async {
        let (g, steps) = held(vec![spec("clis").marks(&["gh"]), spec("github").after(&["gh"])]);
        let done = spawn_graph(steps, 1, None);
        settle().await;
        g.mark("clis", "gh").await;
        assert_eq!(g.started(), ["clis"], "the width is full: github waits for room, not for the mark");
        g.let_go("clis").await;
        assert_eq!(g.started(), ["clis", "github"]);
        g.mark("clis", "gh").await;
        assert_eq!(g.started(), ["clis", "github"]);
        g.let_go("github").await;
        assert_eq!(answered(&done), Some(Ok(vec![])));
    });
}

// spec: G6 (the first failure wins over a later one and over a stop; the run still waits for every running step)
#[test]
fn the_first_failure_is_the_answer_whatever_ends_after_it() {
    run_local(async {
        let (g, steps) = held(vec![spec("a").throws(), spec("b").throws(), spec("c").stops(), spec("d")]);
        let done = spawn_graph(steps, 3, None);
        settle().await;
        assert_eq!(g.started(), ["a", "b", "c"]);
        g.let_go("b").await;
        g.let_go("a").await;
        assert_eq!(answered(&done), None, "c still runs");
        g.let_go("c").await;
        assert_eq!(answered(&done), Some(Err("b broke".to_string())));
        assert_eq!(g.started(), ["a", "b", "c"], "nothing starts after a failure");
    });
}
