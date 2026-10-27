// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-title.test.ts at 65f6b89c7. The adapter here answers the store read and the store write the
//! way the real ones do: one shell line to the machine, its stdout the harness's own title or what became of the name. What
//! is under test is the runtime's side of it: it asks the harness for a name when a thread's first turn starts, reads the
//! store at a turn's end and on a refresh, names the session on the machine when a client renames the thread, and keeps the
//! answer on the rows a thread is folded from. Four cases drive the real Claude Code or Codex adapter (a fake app server, the
//! real reader of a slug-guarded command, the real parser of a two-line answer) and are not ported; each is named below.
#[macro_use]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::comeback::{opts, rows};
use common::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, ExecFailure, HarnessAdapter, HarnessExec, HarnessSession, HarnessStartOptions, SessionRenameWrite, SharedFuture,
    TitleTurn, TurnResult,
};
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::types::api::StartOptions;
use wsp_core::types::events::{SESSION_TITLE_REFRESH_MAX, SESSION_TITLE_TTL_MS};
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_engine::errors::EngineError;
use wsp_wire::views::harness::keeps_rename;
use wsp_wire::views::session::SessionView;
use wsp_words::views::session::fold_threads;
use wsp_words::words::thread::{machine_unreachable_line, EMPTY_TITLE_LINE};
use wsp_words::words::turn::OPENING_KEPT;

const SESSION: &str = "33333333-3333-4333-8333-333333333333";
const TITLE_COMMAND: &str = "wsp-title-read";
const RENAME_COMMAND: &str = "wsp-title-write";
const OPENING: &str = "make a server, and its tests";
const TTL: i64 = SESSION_TITLE_TTL_MS as i64;

struct Begun {
    prompt: String,
    title: Option<String>,
}

type Maker = Rc<dyn Fn(TitleTurn) -> LocalBoxFuture<'static, Option<String>>>;

#[derive(Clone)]
struct Titled {
    keeps_titles: bool,
    keeps_names: bool,
    rekeys: bool,
    announces: bool,
    session_of: Option<Rc<dyn Fn(&str) -> String>>,
    /// The Codex reader: the store's line is JSON that `parse_session_title` reads.
    codex_reader: bool,
    maker: Option<Maker>,
    reply: String,
    status: &'static str,
    hold: Option<Rc<Gate>>,
    ends_on_interrupt: bool,
    starts: Rc<RefCell<Vec<Begun>>>,
}

impl Default for Titled {
    fn default() -> Titled {
        Titled {
            keeps_titles: true,
            keeps_names: false,
            rekeys: false,
            announces: true,
            session_of: None,
            codex_reader: false,
            maker: None,
            reply: "ok".into(),
            status: "completed",
            hold: None,
            ends_on_interrupt: false,
            starts: Rc::default(),
        }
    }
}

fn titled(f: impl FnOnce(&mut Titled)) -> Titled {
    let mut t = Titled::default();
    f(&mut t);
    t
}

impl Titled {
    fn factory(&self) -> HarnessAdapterFactory {
        let me = self.clone();
        Arc::new(move |_ctx: HarnessAdapterContext| Ok(Arc::new(TitledAdapter { me: me.clone() }) as Arc<dyn HarnessAdapter>))
    }
}

fn ev(doc: Value) -> wsp_adapters::port::AdapterEvent {
    serde_json::from_value(doc).expect("an adapter event")
}

struct TitledAdapter {
    me: Titled,
}

impl HarnessAdapter for TitledAdapter {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let me = &self.me;
        me.starts.borrow_mut().push(Begun { prompt: o.prompt.clone(), title: o.title.clone() });
        let session = o.resume.clone().or_else(|| me.session_of.as_ref().map(|f| f(&o.prompt))).unwrap_or_else(|| SESSION.to_string());
        let result: TurnResult = serde_json::from_value(json!({ "status": me.status, "text": me.reply })).expect("a result");
        let on = o.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let (ended_tx, ended_rx) = oneshot::channel::<()>();
        let (announces, hold, races, r, id) = (me.announces, me.hold.clone(), me.ends_on_interrupt, result.clone(), session.clone());
        tokio::task::spawn_local(async move {
            if announces {
                on(ev(json!({ "type": "session.start", "sessionId": id })));
            }
            if let Some(hold) = hold {
                if races {
                    futures::future::select(Box::pin(hold.wait()), ended_rx).await;
                } else {
                    hold.wait().await;
                }
            }
            on(ev(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&r).expect("a result") })));
            on(ev(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(r);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move {
            match rx.await {
                Ok(r) => r,
                Err(_) => futures::future::pending().await,
            }
        }
        .boxed_local();
        let local_id = if me.rekeys { format!("66666666-0000-4000-8000-{:012}", me.starts.borrow().len()) } else { session };
        Ok(Arc::new(TitledSession { local_id, finished: finished.shared(), ended: RefCell::new(Some(ended_tx)) }))
    }

    fn steers(&self) -> bool {
        false
    }

    fn renames(&self) -> bool {
        self.me.keeps_names
    }

    fn session_title(
        &self,
        id: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        if !self.me.keeps_titles {
            return None;
        }
        let codex = self.me.codex_reader;
        Some(
            async move {
                let stdout = exec.exec(&format!("{TITLE_COMMAND} {id}"), None).await?;
                Ok(if codex {
                    wsp_adapters::codex::session_title::parse_session_title(&stdout)
                } else {
                    let said = stdout.trim().to_string();
                    (!said.is_empty()).then_some(said)
                })
            }
            .boxed_local(),
        )
    }

    fn title_for(
        &self,
        turn: TitleTurn,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let maker = self.me.maker.clone()?;
        Some(async move { Ok(maker(turn).await) }.boxed_local())
    }

    fn rename_session(
        &self,
        id: String,
        title: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        if !self.me.keeps_names {
            return None;
        }
        Some(
            async move {
                let said = exec.exec(&format!("{RENAME_COMMAND} {id} {title}"), None).await?;
                let said = said.trim();
                Ok(match said {
                    "written" => SessionRenameWrite::Written,
                    "no-session" => SessionRenameWrite::NoSession,
                    other => SessionRenameWrite::Failed { error: other.to_string() },
                })
            }
            .boxed_local(),
        )
    }
}

struct TitledSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
    ended: RefCell<Option<oneshot::Sender<()>>>,
}

impl HarnessSession for TitledSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let tx = self.ended.borrow_mut().take();
        async move {
            if let Some(tx) = tx {
                let _ = tx.send(());
            }
        }
        .boxed_local()
    }
}

struct Guest {
    backend: Arc<StubBackend>,
    reads: Rc<RefCell<Vec<String>>>,
    writes: Rc<RefCell<Vec<String>>>,
}

/// `titledBackend`: a guest that answers the title command with `titled()` and nothing else, counting the reads; with `wrote`, it
/// answers the write command too, in the words a real store's line prints.
fn guest(titled: impl Fn() -> Option<String> + 'static, wrote: Option<Rc<dyn Fn(&str) -> String>>) -> Guest {
    let backend = StubBackend::new(None);
    let (reads, writes) = (Rc::new(RefCell::new(vec![])), Rc::new(RefCell::new(vec![])));
    let (r, w, inner) = (reads.clone(), writes.clone(), backend.exec_impl_fn());
    backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
        if cmd.starts_with(RENAME_COMMAND) {
            w.borrow_mut().push(cmd.clone());
            let title = cmd[RENAME_COMMAND.len() + 1..].split(' ').skip(1).collect::<Vec<_>>().join(" ");
            let said = wrote.as_ref().map(|f| f(&title)).unwrap_or_default();
            return Box::pin(async move { Ok(exec_ok(&said)) });
        }
        if !cmd.starts_with(TITLE_COMMAND) {
            return inner(m, cmd, stdin);
        }
        r.borrow_mut().push(cmd);
        let said = titled().unwrap_or_default();
        // A round trip to the machine takes time: a read begun at the start of a turn is still in flight when a turn that
        // ends at once asks for its own, which the TS stub's chain of promises also gives it.
        Box::pin(async move {
            sleep_ms(5).await;
            Ok(exec_ok(&said))
        })
    }));
    Guest { backend, reads, writes }
}

/// A guest whose title read fails with what `fail()` returns while it returns something, and answers `named` otherwise.
fn failing_guest(fail: impl Fn() -> Option<EngineError> + 'static) -> Guest {
    let backend = StubBackend::new(None);
    let reads: Rc<RefCell<Vec<String>>> = Rc::default();
    let (r, inner) = (reads.clone(), backend.exec_impl_fn());
    backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
        if !cmd.starts_with(TITLE_COMMAND) {
            return inner(m, cmd, stdin);
        }
        r.borrow_mut().push(cmd);
        let failed = fail();
        Box::pin(async move {
            sleep_ms(5).await;
            match failed {
                Some(e) => Err(e),
                None => Ok(exec_ok("Building the server")),
            }
        })
    }));
    Guest { backend, reads, writes: Rc::default() }
}

struct Fix {
    fx: Fx,
    ws: String,
    clock: FakeClock,
    state: State,
}

async fn fix(backend: Arc<StubBackend>, adapters: &[(&str, &Titled)]) -> Fix {
    let clock = FakeClock::at_now();
    let state = sqlite_state();
    let mut w = runtime(&backend).store(&state.store).clock(&clock);
    for (name, t) in adapters {
        w = w.adapter(name, t.factory());
    }
    let fx = w.build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    Fix { fx, ws, clock, state }
}

type Handle = Rc<dyn wsp_core::types::wiring::SessionHandle>;

impl Fix {
    async fn start(&self, o: StartOptions) -> Handle {
        self.fx.rt.sessions().start(&self.ws, o, None).await.expect("a start")
    }
    async fn finish(&self, prompt: &str) -> Handle {
        let h = self.start(opts(prompt)).await;
        h.finished().await;
        h
    }
    async fn rows(&self) -> Vec<Value> {
        rows(&self.fx.rt, &self.ws).await
    }
    async fn title(&self) -> String {
        title_of(&self.fx.rt, &self.ws).await
    }
}

async fn title_of(rt: &wsp_serve::AssembledRuntime, ws: &str) -> String {
    let views: Vec<SessionView> = rows(rt, ws).await.iter().map(|r| serde_json::from_value(r.clone()).expect("a row")).collect();
    j(&fold_threads(&views)[0])["title"].as_str().unwrap_or_default().to_string()
}

fn thread(h: &Handle) -> String {
    j(&h.view())["threadId"].as_str().expect("a thread").to_string()
}

fn on(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), thread: Some(thread.into()), ..Default::default() }
}

/// `vi.waitFor(() => rows[0].<key> ...)`, 2 s.
macro_rules! row_is {
    ($f:expr, $key:literal, $want:expr) => {{
        let rt = $f.fx.rt.clone();
        let ws = $f.ws.clone();
        wait!([rt, ws] rows(&rt, &ws).await.first().is_some_and(|r| r[$key] == json!($want)))
    }};
}

macro_rules! row_has {
    ($f:expr, $key:literal) => {{
        let rt = $f.fx.rt.clone();
        let ws = $f.ws.clone();
        wait!([rt, ws] rows(&rt, &ws).await.first().is_some_and(|r| !r[$key].is_null()))
    }};
}

fn count(reads: &Rc<RefCell<Vec<String>>>) -> usize {
    reads.borrow().len()
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

fn lines_of(w: &wsp_core::console::Captured, starts: &str) -> Vec<String> {
    w.lines().into_iter().filter(|l| l.starts_with(starts)).collect()
}

/// A promise the test opens with a value, for a title question the harness has not answered yet.
struct Answer {
    gate: Rc<Gate>,
    value: RefCell<Option<String>>,
}

fn answer() -> Rc<Answer> {
    Rc::new(Answer { gate: Gate::new(), value: RefCell::new(None) })
}

impl Answer {
    fn open(&self, v: Option<&str>) {
        *self.value.borrow_mut() = v.map(str::to_string);
        self.gate.open();
    }
}

fn asking(asked: &Rc<RefCell<Vec<TitleTurn>>>, a: Option<Rc<Answer>>, fixed: Option<&'static str>) -> Maker {
    let asked = asked.clone();
    Rc::new(move |t| {
        asked.borrow_mut().push(t);
        let a = a.clone();
        Box::pin(async move {
            match a {
                Some(a) => {
                    a.gate.wait().await;
                    a.value.borrow().clone()
                }
                None => fixed.map(str::to_string),
            }
        })
    })
}

// "the harness's own title on a thread" > "is read from the harness's store when the turn ends and is what every client folds the thread by"
#[test]
fn is_read_from_the_harness_store_when_the_turn_ends_and_is_what_every_client_folds_the_thread_by() {
    drive(|| async {
        let g = guest(|| s("Building the server"), None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        f.finish("make a server, and its tests").await;
        row_has!(f, "harnessTitle");
        assert_eq!(*g.reads.borrow(), [format!("{TITLE_COMMAND} {SESSION}")]);
        assert_eq!(f.title().await, "Building the server");
    });
}

// "..." > "follows a rename made inside the harness: the refresh past the window reads it, the listing after shows it"
#[test]
fn follows_a_rename_made_inside_the_harness_the_refresh_past_the_window_reads_it_the_listing_after_shows_it() {
    drive(|| async {
        let named = Rc::new(RefCell::new("Building the server".to_string()));
        let n = named.clone();
        let g = guest(move || Some(n.borrow().clone()), None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        f.finish("make a server").await;
        row_has!(f, "harnessTitle");
        assert_eq!(f.title().await, "Building the server");

        *named.borrow_mut() = "the sidebar's own name".into();
        // Inside the window nothing is read at all.
        assert_eq!(f.title().await, "Building the server");
        f.clock.advance(TTL + 1);
        // Past it the read goes out, but a row that already carries a title is answered from the index rather than
        // waited on, so a wedged guest cannot empty the listing; the rename is on the row by the listing after.
        assert_eq!(f.title().await, "Building the server");
        row_is!(f, "harnessTitle", "the sidebar's own name");
        assert_eq!(f.title().await, "the sidebar's own name");
    });
}

// "..." > "costs one read per session per window however often a client refreshes"
#[test]
fn costs_one_read_per_session_per_window_however_often_a_client_refreshes() {
    drive(|| async {
        let g = guest(|| s("Building the server"), None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        f.finish("make a server").await;
        let r = g.reads.clone();
        wait!([r] count(&r) == 1);
        for _ in 0..5 {
            f.rows().await;
        }
        assert_eq!(count(&g.reads), 1);
        f.clock.advance(TTL + 1);
        f.rows().await;
        // The row carries its title, so the listing does not wait on the read it sends; it is at the guest a turn later.
        let r = g.reads.clone();
        wait!([r] count(&r) == 2);
    });
}

// "..." > "keeps the title it last read while the machine naps, and asks nothing of a machine that is not running"
#[test]
fn keeps_the_title_it_last_read_while_the_machine_naps_and_asks_nothing_of_a_machine_that_is_not_running() {
    drive(|| async {
        let named = Rc::new(RefCell::new(s("Building the server")));
        let n = named.clone();
        let g = guest(move || n.borrow().clone(), None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        f.finish("make a server").await;
        let r = g.reads.clone();
        wait!([r] count(&r) == 1);
        row_has!(f, "harnessTitle");
        f.fx.rt.workspaces().nap(&f.ws, None).await.expect("a nap");
        *named.borrow_mut() = None;
        assert_eq!(f.title().await, "Building the server");
        assert_eq!(count(&g.reads), 1);
    });
}

// "..." > "outlives the process: a host that started again reads the title back off the session index"
#[test]
fn outlives_the_process_a_host_that_started_again_reads_the_title_back_off_the_session_index() {
    drive(|| async {
        let named = Rc::new(RefCell::new(s("Building the server")));
        let n = named.clone();
        let g = guest(move || n.borrow().clone(), None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        f.finish("make a server").await;
        // The row's own document, which is what the next process reads the index back out of.
        let (store, ws) = (f.state.store.clone(), f.ws.clone());
        wait!([store, ws] store.get("sessions", &ws).await.expect("a read").is_some_and(|d| d["sessions"][0]["harnessTitle"] == "Building the server"));
        assert_eq!(count(&g.reads), 1);

        *named.borrow_mut() = None;
        let again = runtime(&g.backend).store(&f.state.store).adapter("claude", titled(|_| {}).factory()).build();
        assert_eq!(title_of(&again.rt, &f.ws).await, "Building the server");
    });
}

// "..." > "fails one row's title, never the listing, when the adapter refuses the id on the calling stack"
// Not ported: it builds the real Codex reader (`createCodexAdapter`), whose command guard throws on a slug that is not plain.

// "..." > "asks about the newest sessions only, and never more than the cap in one refresh"
#[test]
fn asks_about_the_newest_sessions_only_and_never_more_than_the_cap_in_one_refresh() {
    drive(|| async {
        // The store answers nothing throughout, so no row ever carries a title and the refresh below waits on every
        // read it starts: the count and the ids are the refresh's own.
        let g = guest(|| None, None);
        let t = titled(|t| t.session_of = Some(Rc::new(|p| p["turn ".len()..].to_string())));
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        let ids: Vec<String> = (0..=SESSION_TITLE_REFRESH_MAX).map(|i| format!("s-{i:02}")).collect();
        for id in &ids {
            f.clock.advance(1_000);
            // A row's startedAt is the wall clock, which the pick sorts on, so the turns are spaced in it and not only
            // in the runtime's own clock.
            sleep_ms(2).await;
            f.finish(&format!("turn {id}")).await;
        }
        let (r, n) = (g.reads.clone(), ids.len());
        wait!([r, n] count(&r) >= n);
        // One listing inside the window, which waits on every read still in flight from a turn's end.
        f.rows().await;

        g.reads.borrow_mut().clear();
        f.clock.advance(TTL + 1);
        f.rows().await;
        let mut asked: Vec<String> = g.reads.borrow().iter().map(|c| c.split(' ').nth(1).unwrap_or_default().to_string()).collect();
        asked.sort();
        let mut want: Vec<String> = ids[1..].to_vec();
        want.sort();
        assert_eq!(asked, want);
    });
}

// "..." > "asks once per harness session however many of a thread's turns carry it"
#[test]
fn asks_once_per_harness_session_however_many_of_a_threads_turns_carry_it() {
    drive(|| async {
        let g = guest(|| None, None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|t| t.rekeys = true))]).await;
        let first = f.finish("make a server").await;
        f.clock.advance(1_000);
        f.start(on("and tests", &thread(&first))).await.finished().await;
        // Two rows, one harness session: the adapter minted its own local id per turn, as the codex one does.
        assert_eq!(f.rows().await.iter().filter(|v| v["claudeSessionId"] == SESSION).count(), 2);

        g.reads.borrow_mut().clear();
        f.clock.advance(TTL + 1);
        f.rows().await;
        assert_eq!(*g.reads.borrow(), [format!("{TITLE_COMMAND} {SESSION}")]);
    });
}

// "..." > "leaves the opening turn's words alone for a harness that keeps no title, and asks its machine nothing"
#[test]
fn leaves_the_opening_turns_words_alone_for_a_harness_that_keeps_no_title_and_asks_its_machine_nothing() {
    drive(|| async {
        let g = guest(|| s("never asked"), None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|t| t.keeps_titles = false))]).await;
        f.finish("make a server\nand its tests").await;
        assert_eq!(f.title().await, "make a server");
        assert!(f.rows().await[0].get("harnessTitle").is_none());
        assert!(g.reads.borrow().is_empty());
    });
}

// "..." > "keeps the opening turn's words when the store has no title for the session, and asks again rather than caching the miss forever"
#[test]
fn keeps_the_opening_turns_words_when_the_store_has_no_title_for_the_session_and_asks_again_rather_than_caching_the_miss_forever() {
    drive(|| async {
        let named = Rc::new(RefCell::new(None::<String>));
        let n = named.clone();
        let g = guest(move || n.borrow().clone(), None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        f.finish("make a server").await;
        let r = g.reads.clone();
        wait!([r] count(&r) == 1);
        assert_eq!(f.title().await, "make a server");
        *named.borrow_mut() = s("Building the server");
        f.clock.advance(TTL + 1);
        assert_eq!(f.title().await, "Building the server");
    });
}

fn unreachable_title(fail: impl Fn() -> Option<EngineError> + 'static, words: &'static str) {
    drive(move || async move {
        let g = failing_guest(fail);
        let warned = Rc::new(capture());
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        f.finish("make a server").await;
        let w = warned.clone();
        wait!([w] lines_of(&w, "no title for session").len() == 1);
        for _ in 0..3 {
            f.clock.advance(TTL + 1);
            f.rows().await;
        }
        sleep_ms(20).await;
        assert_eq!(lines_of(&warned, "no title for session"), [format!("no title for session {} on {}: {words}", &SESSION[..8], f.ws)]);
        assert_eq!(count(&g.reads), 1);
    });
}

// "a title read that fails" > "says once, with the provider's status, that a machine the provider cannot reach gave no title, and reads nothing more while the mark stands"
#[test]
fn says_once_with_the_providers_status_that_a_machine_the_provider_cannot_reach_gave_no_title_and_reads_nothing_more_while_the_mark_stands()
{
    unreachable_title(
        || {
            Some(EngineError::machine_unreachable(
                "m1",
                "Sandbox is not reachable",
                503,
                &machine_unreachable_line("Sandbox is not reachable"),
            ))
        },
        "503 Sandbox is not reachable",
    );
}

// "..." > "says once that a machine the provider cannot run commands on gave no title, with the status, and reads nothing more while the mark stands"
#[test]
fn says_once_that_a_machine_the_provider_cannot_run_commands_on_gave_no_title_with_the_status_and_reads_nothing_more_while_the_mark_stands()
{
    unreachable_title(|| Some(EngineError::exec_failed("m1", "exec failed", 502)), "502 exec failed");
}

// "..." > "says a store read that keeps failing for another reason once, until a read answers"
#[test]
fn says_a_store_read_that_keeps_failing_for_another_reason_once_until_a_read_answers() {
    drive(|| async {
        let failing = Rc::new(Cell::new(true));
        let fl = failing.clone();
        let g = failing_guest(move || fl.get().then(|| EngineError::Other { message: "timed out after 15000 ms".into(), code: None }));
        let warned = Rc::new(capture());
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        let line = format!("no title for session {} on {}: timed out after 15000 ms", &SESSION[..8], f.ws);
        f.finish("make a server").await;
        let w = warned.clone();
        wait!([w] lines_of(&w, "no title for session").len() == 1);
        for _ in 0..3 {
            f.clock.advance(TTL + 1);
            f.rows().await;
        }
        assert_eq!(count(&g.reads), 4);
        assert_eq!(lines_of(&warned, "no title for session"), std::slice::from_ref(&line));
        failing.set(false);
        f.clock.advance(TTL + 1);
        f.rows().await;
        row_is!(f, "harnessTitle", "Building the server");
        failing.set(true);
        f.clock.advance(TTL + 1);
        f.rows().await;
        let w = warned.clone();
        wait!([w] lines_of(&w, "no title for session").len() == 2);
        assert_eq!(lines_of(&warned, "no title for session"), [line.clone(), line]);
        assert_eq!(count(&g.reads), 6);
    });
}

// "the title the harness makes for a thread" > "asks the harness once when the turn starts, from the opening turn alone and on the cheapest model its catalog lists, so the seed goes before the turn ends"
#[test]
fn asks_the_harness_once_when_the_turn_starts_from_the_opening_turn_alone_and_on_the_cheapest_model_its_catalog_lists_so_the_seed_goes_before_the_turn_ends(
) {
    drive(|| async {
        let (turn, ans) = (Gate::new(), answer());
        let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
        let g = guest(|| None, None);
        let t = titled(|t| {
            t.reply = "the server is up on 3000".into();
            t.hold = Some(turn.clone());
            t.maker = Some(asking(&asked, Some(ans.clone()), None));
        });
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        let handle = f.start(opts(OPENING)).await;
        // The question goes out at the start, with no reply to carry; the seed stands only while the harness thinks.
        let a = asked.clone();
        wait!([a] a.borrow().len() == 1);
        assert_eq!(*asked.borrow(), [TitleTurn { opening: OPENING.into(), reply: None, model: s("claude-haiku-5-5") }]);
        assert_eq!(f.title().await, OPENING);

        ans.open(Some("Seed thread titles here"));
        row_is!(f, "titleSource", "auto");
        assert_eq!(f.title().await, "Seed thread titles here");
        assert_eq!(f.rows().await[0]["status"], "running");

        turn.open();
        handle.finished().await;
        sleep_ms(5).await;
        assert_eq!(asked.borrow().len(), 1);
        assert_eq!(f.title().await, "Seed thread titles here");
    });
}

// "..." > "asks nothing for a turn whose harness never named a session, since its start never came to anything"
#[test]
fn asks_nothing_for_a_turn_whose_harness_never_named_a_session_since_its_start_never_came_to_anything() {
    drive(|| async {
        let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
        let g = guest(|| None, None);
        let t = titled(|t| {
            t.announces = false;
            t.maker = Some(asking(&asked, None, Some("Seed thread titles here")));
        });
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        f.finish(OPENING).await;
        sleep_ms(5).await;
        assert!(asked.borrow().is_empty());
        assert_eq!(f.title().await, OPENING);
    });
}

// "..." > "writes the name it made into the harness's own store, so the harness's own list says the same"
#[test]
fn writes_the_name_it_made_into_the_harness_own_store_so_the_harness_own_list_says_the_same() {
    drive(|| async {
        let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let t = titled(|t| {
            t.keeps_names = true;
            t.maker = Some(asking(&asked, None, Some("Seed thread titles here")));
        });
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        f.finish(OPENING).await;
        let w = g.writes.clone();
        wait!([w] count(&w) == 1);
        assert_eq!(*g.writes.borrow(), [format!("{RENAME_COMMAND} {SESSION} Seed thread titles here")]);
    });
}

// "..." > "never replaces what the harness itself calls the session, which is the person's, and does not ask for a title at all"
#[test]
fn never_replaces_what_the_harness_itself_calls_the_session_which_is_the_persons_and_does_not_ask_for_a_title_at_all() {
    drive(|| async {
        let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
        let g = guest(|| s("the sidebar's own name"), None);
        let t = titled(|t| t.maker = Some(asking(&asked, None, Some("Seed thread titles here"))));
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        f.finish(OPENING).await;
        row_has!(f, "harnessTitle");
        assert_eq!(f.title().await, "the sidebar's own name");
        // What the harness's own store calls a session is the person's, whether they typed it or the harness made it.
        assert_eq!(f.rows().await[0]["titleSource"], "person");
        assert!(asked.borrow().is_empty());
    });
}

// "..." > "asks for a name when the store's title is only the opening words, as codex writes them when a thread starts"
#[test]
fn asks_for_a_name_when_the_stores_title_is_only_the_opening_words_as_codex_writes_them_when_a_thread_starts() {
    drive(|| async {
        // The codex CLI names a thread from its opening words the moment it starts, so that title is the seed under
        // another roof, not a person's name; the head of the opening words is what a longer brief leaves in the column.
        for stored in [OPENING, "make a server"] {
            let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
            let g = guest(move || s(stored), None);
            let t = titled(|t| t.maker = Some(asking(&asked, None, Some("Seed thread titles here"))));
            let f = fix(g.backend.clone(), &[("claude", &t)]).await;
            f.finish(OPENING).await;
            row_is!(f, "titleSource", "auto");
            assert_eq!(asked.borrow().len(), 1, "{stored}");
            assert_eq!(f.title().await, "Seed thread titles here");
        }
    });
}

// "..." > "asks for a name when the store's title is the whole of an opening longer than the row keeps"
#[test]
fn asks_for_a_name_when_the_stores_title_is_the_whole_of_an_opening_longer_than_the_row_keeps() {
    drive(|| async {
        // Codex writes every line of the opening into its title column, and the row holds only the opening's head.
        let brief = format!("make a server, and its tests. {}", "Read the ticket and every comment on it before the code. ".repeat(40));
        assert!(brief.len() > OPENING_KEPT);
        let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
        let b = brief.clone();
        let g = guest(move || Some(b.clone()), None);
        let t = titled(|t| t.maker = Some(asking(&asked, None, Some("Seed thread titles here"))));
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        f.finish(&brief).await;
        row_is!(f, "titleSource", "auto");
        assert_eq!(asked.borrow().len(), 1);
        assert_eq!(f.title().await, "Seed thread titles here");
    });
}

// "..." > "keeps the name it made when the store still answers with the opening words, since a seed is no news to a named row"
#[test]
fn keeps_the_name_it_made_when_the_store_still_answers_with_the_opening_words_since_a_seed_is_no_news_to_a_named_row() {
    drive(|| async {
        // Codex's title column keeps the opening words until the name lands in its own column, and a refresh that reads it
        // meanwhile, or after a write the store refused, may not take the made name away.
        let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
        let g = guest(|| s(OPENING), None);
        let t = titled(|t| t.maker = Some(asking(&asked, None, Some("Seed thread titles here"))));
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        f.finish(OPENING).await;
        row_is!(f, "titleSource", "auto");
        let before = count(&g.reads);
        f.clock.advance(TTL + 1);
        f.rows().await;
        let (r, b) = (g.reads.clone(), before);
        wait!([r, b] count(&r) > b);
        f.rows().await;
        assert_eq!(f.title().await, "Seed thread titles here");
        assert_eq!(f.rows().await[0]["titleSource"], "auto");
    });
}

// "..." > "throws away an answer that lands after a rename: the person's name stands"
#[test]
fn throws_away_an_answer_that_lands_after_a_rename_the_persons_name_stands() {
    drive(|| async {
        let ans = answer();
        let named = Rc::new(RefCell::new(None::<String>));
        let n = named.clone();
        let g = guest(move || n.borrow().clone(), None);
        let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
        let t = titled(|t| t.maker = Some(asking(&asked, Some(ans.clone()), None)));
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        f.finish(OPENING).await;
        let a = asked.clone();
        wait!([a] a.borrow().len() == 1);
        // The harness is thinking about a title; meanwhile the person renames the session inside the harness itself.
        *named.borrow_mut() = s("the sidebar's own name");
        f.clock.advance(TTL + 1);
        row_is!(f, "harnessTitle", "the sidebar's own name");

        ans.open(Some("Seed thread titles here"));
        let (rt, ws) = (f.fx.rt.clone(), f.ws.clone());
        wait!([rt, ws] rows(&rt, &ws).await.len() == 1);
        assert_eq!(f.title().await, "the sidebar's own name");
        assert_eq!(f.rows().await[0]["titleSource"], "person");
    });
}

// "..." > "keeps the opening words when the harness answers with something that is not a title, and asks nothing more"
// Not ported: it parses the answer with the real Claude Code reader (`createClaudeAdapter().titleFor`) over a `claude -p` command.

// "..." > "keeps the name it made through the thread's later turns, which the fold titles by"
#[test]
fn keeps_the_name_it_made_through_the_threads_later_turns_which_the_fold_titles_by() {
    drive(|| async {
        let asked: Rc<RefCell<Vec<TitleTurn>>> = Rc::default();
        let g = guest(|| None, None);
        let t = titled(|t| {
            t.rekeys = true;
            t.maker = Some(asking(&asked, None, Some("Seed thread titles here")));
        });
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        let first = f.finish(OPENING).await;
        row_is!(f, "titleSource", "auto");
        f.start(on("and now the tests", &thread(&first))).await.finished().await;
        assert_eq!(f.title().await, "Seed thread titles here");
    });
}

// "..." > "names a thread the start named, at the launch and in the harness's store, and asks for no title of its own"
#[test]
fn names_a_thread_the_start_named_at_the_launch_and_in_the_harness_store_and_asks_for_no_title_of_its_own() {
    drive(|| async {
        let (turn, asked): (Rc<Gate>, Rc<RefCell<Vec<TitleTurn>>>) = (Gate::new(), Rc::default());
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let t = titled(|t| {
            t.keeps_names = true;
            t.hold = Some(turn.clone());
            t.maker = Some(asking(&asked, None, Some("Seed thread titles here")));
        });
        let f = fix(g.backend.clone(), &[("claude", &t)]).await;
        let handle = f.start(StartOptions { title: s(" Ticket 411 review\nand its branch "), ..opts(OPENING) }).await;
        // The name stands from the first second, before any turn has ended.
        assert_eq!(f.title().await, "Ticket 411 review");
        assert_eq!(f.rows().await[0]["titleSource"], "person");
        assert_eq!(t.starts.borrow()[0].title.as_deref(), Some("Ticket 411 review"));
        let w = g.writes.clone();
        wait!([w] count(&w) == 1);
        assert_eq!(*g.writes.borrow(), [format!("{RENAME_COMMAND} {SESSION} Ticket 411 review")]);

        turn.open();
        handle.finished().await;
        sleep_ms(5).await;
        assert!(asked.borrow().is_empty());
        assert_eq!(f.title().await, "Ticket 411 review");
    });
}

// "..." > "writes the name again at the turn's end when codex had not written the thread when it was given"
// Not ported: it runs the real Codex adapter's rename against a fake app server (`fakeAppServer`) through real bash.

// "..." > "keeps the name a start gave on a codex thread whose store titles it with the whole brief, whether its turn answers or fails"
#[test]
fn keeps_the_name_a_start_gave_on_a_codex_thread_whose_store_titles_it_with_the_whole_brief_whether_its_turn_answers_or_fails() {
    drive(|| async {
        // Codex fills its title column with the opening message whole, every line of it, and leaves the name column empty
        // until a rename lands there; that title is the opening words, never a person's name.
        let brief = "You are the cold reviewer for wsp-map #1671: the desktop app\nupdating itself in place.\n\nRead the diff first.";
        for status in ["completed", "failed"] {
            let b = brief.to_string();
            let g = guest(move || Some(json!({ "name": null, "title": b }).to_string()), None);
            let t = titled(|t| {
                t.codex_reader = true;
                t.status = status;
                t.rekeys = true;
            });
            let f = fix(g.backend.clone(), &[("codex", &t)]).await;
            f.start(StartOptions { harness: s("codex"), title: s("Review 1671 self update"), ..opts(brief) }).await.finished().await;
            let r = g.reads.clone();
            wait!([r] count(&r) > 0);
            sleep_ms(5).await;
            assert_eq!(f.title().await, "Review 1671 self update", "{status}");
            assert_eq!(f.rows().await[0]["titleSource"], "person", "{status}");
        }
    });
}

// "..." > "keeps the name a start gave on a codex thread started with a file, whose store titles it with the prompt the files were handed in"
#[test]
fn keeps_the_name_a_start_gave_on_a_codex_thread_started_with_a_file_whose_store_titles_it_with_the_prompt_the_files_were_handed_in() {
    drive(|| async {
        // A message with files reaches the agent as its words and then the block naming every landed path, and Codex
        // titles the thread with that whole prompt.
        let brief = "You are the cold reviewer for wsp-map #1671.\nRead the diff first.";
        let t = titled(|t| {
            t.codex_reader = true;
            t.rekeys = true;
        });
        let seen = t.starts.clone();
        let g = guest(move || seen.borrow().first().map(|s| json!({ "name": null, "title": s.prompt }).to_string()), None);
        let f = fix(g.backend.clone(), &[("codex", &t)]).await;
        let notes = wsp_wire::types::attachments::Attachment {
            media_type: "text/plain".into(),
            bytes: "bm90ZXM=".into(),
            name: Some("notes.txt".into()),
        };
        f.start(StartOptions {
            harness: s("codex"),
            title: s("Review 1671 self update"),
            attachments: Some(vec![notes]),
            request_id: s("req_a"),
            ..opts(brief)
        })
        .await
        .finished()
        .await;
        assert!(t.starts.borrow()[0].prompt.contains("Attached files:"));
        let (rt, ws) = (f.fx.rt.clone(), f.ws.clone());
        wait!([rt, ws] rows(&rt, &ws).await.first().is_some_and(|r| r["status"] != "running"));
        sleep_ms(5).await;
        assert_eq!(f.title().await, "Review 1671 self update");
        assert_eq!(f.rows().await[0]["titleSource"], "person");
    });
}

// "..." > "refuses a start named with nothing at all"
#[test]
fn refuses_a_start_named_with_nothing_at_all() {
    drive(|| async {
        let g = guest(|| None, None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        let refused =
            f.fx.rt.sessions().start(&f.ws, StartOptions { title: s("  \n "), ..opts(OPENING) }, None).await.err().expect("a refusal");
        assert!(refused.message.contains(EMPTY_TITLE_LINE), "{}", refused.message);
    });
}

fn typed(prompt: &str, name: &Rc<RefCell<Option<String>>>) -> (Guest, Rc<RefCell<Option<String>>>) {
    let _ = prompt;
    let (n, w) = (name.clone(), name.clone());
    (
        guest(
            move || n.borrow().clone(),
            Some(Rc::new(move |title: &str| {
                *w.borrow_mut() = Some(title.to_string());
                "written".to_string()
            })),
        ),
        name.clone(),
    )
}

// "naming a thread from wsp" > "writes the name into the harness's own store and keeps it on every row of the thread"
#[test]
fn writes_the_name_into_the_harness_own_store_and_keeps_it_on_every_row_of_the_thread() {
    drive(|| async {
        let (g, _) = typed("", &Rc::new(RefCell::new(None)));
        let f = fix(
            g.backend.clone(),
            &[(
                "claude",
                &titled(|t| {
                    t.keeps_names = true;
                    t.rekeys = true;
                }),
            )],
        )
        .await;
        let first = f.finish("make a server").await;
        f.clock.advance(1_000);
        let second = f.start(on("and tests", &thread(&first))).await;
        second.finished().await;

        let renamed = f.fx.rt.sessions().rename(&second.id(), "the name he typed in wsp", None).await.expect("a rename");
        assert_eq!(j(&renamed), json!({ "outcome": "renamed" }));
        assert_eq!(*g.writes.borrow(), [format!("{RENAME_COMMAND} {SESSION} the name he typed in wsp")]);
        let rows: Vec<Value> = f.rows().await.into_iter().filter(|v| v["claudeSessionId"] == SESSION).collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter().map(|v| v["harnessTitle"].clone()).collect::<Vec<_>>(),
            ["the name he typed in wsp", "the name he typed in wsp"]
        );
        assert_eq!(f.title().await, "the name he typed in wsp");
        // The harness's own store is where the name now lives, so the next read past the window brings it back.
        f.clock.advance(TTL + 1);
        assert_eq!(f.title().await, "the name he typed in wsp");
    });
}

// "..." > "outlives the process: the name is on the session index the next host reads back"
#[test]
fn outlives_the_process_the_name_is_on_the_session_index_the_next_host_reads_back() {
    drive(|| async {
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let f = fix(g.backend.clone(), &[("claude", &titled(|t| t.keeps_names = true))]).await;
        let session = f.finish("make a server").await;
        f.fx.rt.sessions().rename(&session.id(), "the name", None).await.expect("a rename");
        let stored = f.state.store.get("sessions", &f.ws).await.expect("a read").expect("a document");
        assert_eq!(stored["sessions"][0]["harnessTitle"], "the name");
    });
}

// "..." > "answers unsupported for a harness that keeps no name of a person's, and asks its machine nothing"
#[test]
fn answers_unsupported_for_a_harness_that_keeps_no_name_of_a_persons_and_asks_its_machine_nothing() {
    drive(|| async {
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        let session = f.finish("make a server").await;
        assert_eq!(
            j(&f.fx.rt.sessions().rename(&session.id(), "the name", None).await.expect("a rename")),
            json!({ "outcome": "unsupported" })
        );
        assert!(g.writes.borrow().is_empty());
        assert_eq!(f.title().await, "make a server");
    });
}

// "..." > "answers no-session when the store took nothing, and the row keeps the title it had"
#[test]
fn answers_no_session_when_the_store_took_nothing_and_the_row_keeps_the_title_it_had() {
    drive(|| async {
        let g = guest(|| s("Building the server"), Some(Rc::new(|_| "no-session".to_string())));
        let f = fix(g.backend.clone(), &[("claude", &titled(|t| t.keeps_names = true))]).await;
        let session = f.finish("make a server").await;
        row_has!(f, "harnessTitle");
        assert_eq!(
            j(&f.fx.rt.sessions().rename(&session.id(), "the name", None).await.expect("a rename")),
            json!({ "outcome": "no-session" })
        );
        assert_eq!(f.title().await, "Building the server");
    });
}

// "..." > "answers failed with the machine's own line when the store refused the write, and the row keeps the title it had"
#[test]
fn answers_failed_with_the_machines_own_line_when_the_store_refused_the_write_and_the_row_keeps_the_title_it_had() {
    drive(|| async {
        let g = guest(|| s("Building the server"), Some(Rc::new(|_| "database is locked".to_string())));
        let f = fix(g.backend.clone(), &[("claude", &titled(|t| t.keeps_names = true))]).await;
        let session = f.finish("make a server").await;
        row_has!(f, "harnessTitle");
        // Not no-session: a store that refused the write said nothing about which sessions it has.
        assert_eq!(
            j(&f.fx.rt.sessions().rename(&session.id(), "the name", None).await.expect("a rename")),
            json!({ "outcome": "failed", "error": "database is locked" })
        );
        assert_eq!(f.title().await, "Building the server");
    });
}

// "..." > "answers no-session for a thread whose harness never named a session, and asks the machine nothing"
#[test]
fn answers_no_session_for_a_thread_whose_harness_never_named_a_session_and_asks_the_machine_nothing() {
    drive(|| async {
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let f = fix(
            g.backend.clone(),
            &[(
                "claude",
                &titled(|t| {
                    t.keeps_names = true;
                    t.announces = false;
                }),
            )],
        )
        .await;
        let session = f.finish("make a server").await;
        // The turn ended without a session.start, so the row carries no harness id and the store is keyed by nothing.
        assert!(f.rows().await[0].get("claudeSessionId").is_none());
        assert_eq!(
            j(&f.fx.rt.sessions().rename(&session.id(), "the name", None).await.expect("a rename")),
            json!({ "outcome": "no-session" })
        );
        assert!(g.writes.borrow().is_empty());
        assert_eq!(f.title().await, "make a server");
    });
}

// "..." > "answers not-found for a session this runtime does not hold"
#[test]
fn answers_not_found_for_a_session_this_runtime_does_not_hold() {
    drive(|| async {
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let f = fix(g.backend.clone(), &[("claude", &titled(|t| t.keeps_names = true))]).await;
        assert_eq!(
            j(&f.fx.rt.sessions().rename("no-such-session", "the name", None).await.expect("a rename")),
            json!({ "outcome": "not-found" })
        );
    });
}

// "..." > "refuses a name that is nothing but space, before it asks anything of the machine"
#[test]
fn refuses_a_name_that_is_nothing_but_space_before_it_asks_anything_of_the_machine() {
    drive(|| async {
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let f = fix(g.backend.clone(), &[("claude", &titled(|t| t.keeps_names = true))]).await;
        let session = f.finish("make a server").await;
        let refused = f.fx.rt.sessions().rename(&session.id(), "   ", None).await.expect_err("a refusal");
        assert!(refused.message.contains(EMPTY_TITLE_LINE), "{}", refused.message);
        assert!(g.writes.borrow().is_empty());
    });
}

// "..." > "refuses while the machine is not up, since the name goes into a store on it"
#[test]
fn refuses_while_the_machine_is_not_up_since_the_name_goes_into_a_store_on_it() {
    drive(|| async {
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let f = fix(g.backend.clone(), &[("claude", &titled(|t| t.keeps_names = true))]).await;
        let session = f.finish("make a server").await;
        f.fx.rt.workspaces().nap(&f.ws, None).await.expect("a nap");
        let refused = f.fx.rt.sessions().rename(&session.id(), "the name", None).await.expect_err("a refusal");
        assert!(refused.message.contains("wake it to rename"), "{}", refused.message);
        assert!(g.writes.borrow().is_empty());
    });
}

// "..." > "names the session the harness announced, whatever the runtime calls the row"
#[test]
fn names_the_session_the_harness_announced_whatever_the_runtime_calls_the_row() {
    drive(|| async {
        let g = guest(|| None, Some(Rc::new(|_| "written".to_string())));
        let f = fix(
            g.backend.clone(),
            &[(
                "claude",
                &titled(|t| {
                    t.keeps_names = true;
                    t.rekeys = true;
                }),
            )],
        )
        .await;
        let session = f.finish("make a server").await;
        assert_ne!(session.id(), SESSION);
        f.fx.rt.sessions().rename(&session.id(), "the name", None).await.expect("a rename");
        assert_eq!(*g.writes.borrow(), [format!("{RENAME_COMMAND} {SESSION} the name")]);
    });
}

// "the agents whose store keeps a name" > "rides on the harness's catalog row from its adapter, as steers does, and is written nowhere else"
// Not ported: it builds every real adapter of `HARNESS_ADAPTERS` and compares each catalog row with the adapter's own `renameSession`.

// "..." > "a rename of the workspace leaves a running turn alone: it ends with its reply, on the same thread, under the new name"
#[test]
fn a_rename_of_the_workspace_leaves_a_running_turn_alone_it_ends_with_its_reply_on_the_same_thread_under_the_new_name() {
    drive(|| async {
        let turn = Gate::new();
        let g = guest(|| None, None);
        let f = fix(
            g.backend.clone(),
            &[(
                "claude",
                &titled(|t| {
                    t.reply = "the server is up on 3000".into();
                    t.hold = Some(turn.clone());
                }),
            )],
        )
        .await;
        let handle = f.start(opts("make a server, and its tests")).await;

        // The thread is addressed by id inside, so the name it runs under is the record's alone.
        assert_matches(
            &j(&f.fx.rt.workspaces().rename(&f.ws, "the name he typed", None).await.expect("a rename")),
            json!({ "id": f.ws, "name": "the name he typed" }),
        );
        let running = f.rows().await.remove(0);
        assert_matches(&running, json!({ "workspaceId": f.ws, "status": "running" }));

        turn.open();
        assert_matches(&j(&handle.finished().await), json!({ "status": "completed", "text": "the server is up on 3000" }));
        let ended = f.rows().await.remove(0);
        assert_matches(&ended, json!({ "id": running["id"], "workspaceId": f.ws, "status": "completed" }));
        assert_eq!(g.backend.machines().len(), 1);
        assert!(!g.backend.machine(0).killed.get());
    });
}

// "..." > "an adapter that carries no write says so on its row, and a client reads that as a no"
#[test]
fn an_adapter_that_carries_no_write_says_so_on_its_row_and_a_client_reads_that_as_a_no() {
    drive(|| async {
        let g = guest(|| None, None);
        let f = fix(g.backend.clone(), &[("claude", &titled(|_| {}))]).await;
        let listed = f.fx.rt.harnesses().list(Some(f.ws.clone()), None, None).await.expect("a catalog");
        let row = listed.first().expect("a row").clone();
        assert_matches(&j(&row), json!({ "harness": "claude", "renames": false, "source": "table" }));
        // The row came from the table, so it is no answer yet; the same row from the machine is a no.
        assert!(keeps_rename(Some(&row)));
        let mut from_harness = row.clone();
        from_harness.source = wsp_wire::views::harness::HarnessCatalogSource::Harness;
        assert!(!keeps_rename(Some(&from_harness)));
    });
}

// "a turn the host ended for its machine going away" > "asks that machine for no title, so a delete writes no line about a read that could not land"
#[test]
fn asks_that_machine_for_no_title_so_a_delete_writes_no_line_about_a_read_that_could_not_land() {
    drive(|| async {
        let warned = Rc::new(capture());
        let turn = Gate::new();
        let g = guest(|| s("Building the server"), None);
        let f = fix(
            g.backend.clone(),
            &[(
                "claude",
                &titled(|t| {
                    t.hold = Some(turn.clone());
                    t.ends_on_interrupt = true;
                }),
            )],
        )
        .await;
        // Read off the stored index rather than the listing: a listing of its own asks the harness for the titles it
        // is missing, and what this case is about is the read the turn's own ending starts.
        let status = |store: Rc<dyn wsp_store::store::Store>, ws: String| async move {
            store.get("sessions", &ws).await.expect("a read").and_then(|d| d["sessions"][0]["status"].as_str().map(str::to_string))
        };
        f.start(opts("build it")).await;
        let (store, ws, st) = (f.state.store.clone(), f.ws.clone(), status);
        wait!([store, ws, st] st(store.clone(), ws.clone()).await.as_deref() == Some("running"));

        // The machine is taken down after the threads on it are ended, and that window is where the read used to
        // land: the record is still live here, so the title would be asked of a machine on its way out.
        let machine = g.backend.machine(0);
        let kill = Gate::new();
        let (real, waited, m) = (machine.kill_fn(), kill.clone(), machine.clone());
        let _ = m;
        machine.set_kill(Rc::new(move || {
            let (real, waited) = (real.clone(), waited.clone());
            Box::pin(async move {
                waited.wait().await;
                real().await
            })
        }));
        // The one read a turn pays for at its start, which is the harness's own name for the session: what this
        // case counts is every read after the delete began.
        let r = g.reads.clone();
        wait!([r] count(&r) == 1);
        let asked = count(&g.reads);
        let (rt, ws) = (f.fx.rt.clone(), f.ws.clone());
        let deleted = tokio::task::spawn_local(async move { rt.workspaces().delete(&ws, None).await });
        let (store, ws, st) = (f.state.store.clone(), f.ws.clone(), status);
        wait!([store, ws, st] st(store.clone(), ws.clone()).await.as_deref() != Some("running"));
        // A window wide enough for the read the turn's ending would have started, with the record still live and
        // the machine still there to answer it.
        sleep_ms(100).await;
        assert_eq!(count(&g.reads), asked);
        kill.open();
        deleted.await.expect("the delete").expect("a delete");

        sleep_ms(50).await;
        assert_eq!(count(&g.reads), asked);
        assert_eq!(machine.exec_log().iter().filter(|c| c.starts_with(TITLE_COMMAND)).count(), asked);
        assert!(lines_of(&warned, "").iter().all(|l| !l.contains("no title for session")));
        turn.open();
        f.fx.rt.close().await.expect("a close");
    });
}
