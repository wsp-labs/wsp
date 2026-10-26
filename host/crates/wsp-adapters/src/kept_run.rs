// SPDX-License-Identifier: AGPL-3.0-only
//! One process serving a thread's turns one after another, each reading it through a view of its own. TS
//! packages/protocol/src/kept-run.ts:111-175. Spec: spec/port.md, keep_run.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Poll, Waker};

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::stream::{LocalBoxStream, StreamExt};
use std::sync::Arc;

use crate::port::{end_after_result, ExecFailure, ExecStream, KeptRun, SharedFuture, WriteOutcome};

type Between = Option<Arc<dyn Fn(String)>>;

/// Lines queued for one reader as they arrive, ended by hand or by the process.
#[derive(Default)]
struct Queue {
    lines: RefCell<VecDeque<String>>,
    done: Cell<bool>,
    error: RefCell<Option<ExecFailure>>,
    waker: RefCell<Option<Waker>>,
}

impl Queue {
    fn wake(&self) {
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }

    fn push(&self, line: String) {
        if self.done.get() {
            return;
        }
        self.lines.borrow_mut().push_back(line);
        self.wake();
    }

    fn end(&self, error: Option<ExecFailure>) {
        if self.done.replace(true) {
            return;
        }
        *self.error.borrow_mut() = error;
        self.wake();
    }

    /// Ends at once, answering what was not read yet: whatever the process printed after its turn closed is no turn's.
    fn cut(&self) -> Vec<String> {
        let left = self.lines.borrow_mut().drain(..).collect();
        self.end(None);
        left
    }

    fn iterate(self: &Rc<Self>) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        let queue = self.clone();
        let mut failed = false;
        futures::stream::poll_fn(move |cx| {
            if let Some(line) = queue.lines.borrow_mut().pop_front() {
                return Poll::Ready(Some(Ok(line)));
            }
            if queue.done.get() {
                if !failed {
                    failed = true;
                    if let Some(error) = queue.error.borrow_mut().take() {
                        return Poll::Ready(Some(Err(error)));
                    }
                }
                return Poll::Ready(None);
            }
            *queue.waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        })
        .boxed_local()
    }
}

struct Open {
    id: u64,
    queue: Rc<Queue>,
    settle: Rc<RefCell<Option<oneshot::Sender<Option<i32>>>>>,
}

impl Open {
    fn settle(&self, code: Option<i32>) {
        if let Some(tx) = self.settle.borrow_mut().take() {
            let _ = tx.send(code);
        }
    }
}

#[derive(Default)]
struct State {
    open: Option<Open>,
    released: bool,
    ended: bool,
    failure: Option<ExecFailure>,
    code: Option<i32>,
    turns: u64,
    views: u64,
}

struct Kept {
    stream: Arc<dyn ExecStream>,
    between: Between,
    state: Rc<RefCell<State>>,
}

impl Kept {
    fn between_line(between: &Between, line: String) {
        if let Some(between) = between {
            between(line);
        }
    }
}

struct View {
    stream: Arc<dyn ExecStream>,
    between: Between,
    state: Rc<RefCell<State>>,
    id: u64,
    queue: Rc<Queue>,
    closed: Cell<bool>,
    lines: RefCell<Option<LocalBoxStream<'static, Result<String, ExecFailure>>>>,
    settle: Rc<RefCell<Option<oneshot::Sender<Option<i32>>>>>,
    exited: SharedFuture<Option<i32>>,
}

impl ExecStream for View {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        self.lines.borrow_mut().take().unwrap_or_else(|| futures::stream::empty().boxed_local())
    }
    fn run(&self) -> Option<String> {
        self.stream.run()
    }
    fn pid(&self) -> Option<u32> {
        self.stream.pid()
    }
    fn teardown(&self) {
        self.stream.teardown()
    }
    fn kill(&self) {
        self.stream.kill()
    }
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        if self.closed.get() {
            return async { Ok(WriteOutcome::Gone) }.boxed_local();
        }
        self.stream.write(line)
    }
    fn close_input(&self) {
        if self.closed.replace(true) {
            return;
        }
        if self.state.borrow().released {
            self.stream.close_input();
            return;
        }
        let was_open = {
            let mut state = self.state.borrow_mut();
            if state.open.as_ref().is_some_and(|o| o.id == self.id) {
                state.open = None;
                true
            } else {
                false
            }
        };
        let _ = was_open;
        for line in self.queue.cut() {
            Kept::between_line(&self.between, line);
        }
        self.stream.rest();
        if let Some(tx) = self.settle.borrow_mut().take() {
            let _ = tx.send(Some(0));
        }
    }
    fn write_after(&self, line: String, after: crate::port::Settle) -> Option<()> {
        self.stream.write_after(line, after)
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.exited.clone()
    }
    fn signalled(&self) -> Option<String> {
        if self.state.borrow().ended {
            self.stream.signalled()
        } else {
            None
        }
    }
}

impl KeptRun for Kept {
    fn turn(&self) -> (Arc<dyn ExecStream>, Option<u64>) {
        let first = {
            let mut state = self.state.borrow_mut();
            let first = state.turns == 0;
            state.turns += 1;
            first
        };
        let from = if first { None } else { self.stream.next_turn() };
        let stale = {
            let open = self.state.borrow_mut().open.take();
            open.map(|o| o.queue.cut()).unwrap_or_default()
        };
        for line in stale {
            Kept::between_line(&self.between, line);
        }
        let queue = Rc::new(Queue::default());
        let (tx, rx) = oneshot::channel::<Option<i32>>();
        let settle = Rc::new(RefCell::new(Some(tx)));
        let exited: SharedFuture<Option<i32>> = async move { rx.await.unwrap_or(None) }.boxed_local().shared();
        let id = {
            let mut state = self.state.borrow_mut();
            state.views += 1;
            state.views
        };
        let open = Open { id, queue: queue.clone(), settle: settle.clone() };
        {
            let mut state = self.state.borrow_mut();
            if state.ended {
                queue.end(state.failure.clone());
                open.settle(state.code);
            }
            state.open = Some(open);
        }
        let view = View {
            stream: self.stream.clone(),
            between: self.between.clone(),
            state: self.state.clone(),
            id,
            queue: queue.clone(),
            closed: Cell::new(false),
            lines: RefCell::new(Some(queue.iterate())),
            settle,
            exited,
        };
        (Arc::new(view), from)
    }

    fn release(&self) {
        self.state.borrow_mut().released = true;
    }

    fn up(&self) -> bool {
        let state = self.state.borrow();
        !state.ended && !state.released
    }

    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.stream.exited()
    }

    fn close(&self, wait_ms: u64, grace_ms: u64) -> LocalBoxFuture<'static, ()> {
        self.state.borrow_mut().released = true;
        self.stream.close_input();
        let stream = self.stream.clone();
        async move { end_after_result(stream.as_ref(), wait_ms, grace_ms).await }.boxed_local()
    }
}

/// One reader for a process's whole life, handing each turn a view of it; `between` takes each line printed while no
/// turn is open.
pub fn keep_run(stream: Arc<dyn ExecStream>, between: Between) -> Arc<dyn KeptRun> {
    let state = Rc::new(RefCell::new(State::default()));
    let reader = (stream.clone(), between.clone(), state.clone());
    tokio::task::spawn_local(async move {
        let (stream, between, state) = reader;
        let mut lines = stream.lines();
        let mut failure = None;
        while let Some(item) = lines.next().await {
            match item {
                Ok(line) => {
                    let queue = state.borrow().open.as_ref().map(|o| o.queue.clone());
                    match queue {
                        Some(queue) => queue.push(line),
                        None => Kept::between_line(&between, line),
                    }
                }
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        let code = stream.exited().await;
        let mut state = state.borrow_mut();
        state.ended = true;
        state.failure = failure.clone();
        state.code = code;
        if let Some(open) = &state.open {
            open.queue.end(failure);
            open.settle(code);
        }
    });
    Arc::new(Kept { stream, between, state })
}
