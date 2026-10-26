use std::cell::{Cell, RefCell};

use std::collections::BTreeSet;

use std::rc::Rc;

use std::sync::{Arc, Mutex};

use futures::channel::oneshot;

use support::*;

use wsp_core::console;

use wsp_core::types::internal::ToldAs;

use wsp_core::types::wiring::{AccessMove, SteerTaken};

use wsp_wire::types::js_number::JsNumber;

use wsp_wire::views::session::SessionStatus;

use wsp_wire::views::session_events_more::SessionPermissionEvent;

use wsp_wire::wire::runtime::{SessionAnswerOutcome, SessionAnswerResult, SessionStartOutcome};

use wsp_words::words::computer::thread_word;

use wsp_words::words::tools::{asking_line, denied_line, permission_mode_option_label};

use wsp_words::words::turn::turn_lines;

const W: &str = "w1";

const THREAD: &str = "t1";

const TURN: &str = "turn-1";

const SESS: &str = "sess-1";

const MIN: i64 = 60_000;

// ---- fixtures ----------------------------------------------------------------------------------------------------

fn world() -> (Rc<FakeHost>, Entry) {
    let host = FakeHost::new();
    let entry = host.add_entry(entry(W, json!({})));
    (host, entry)
}

/// A running row's JSON as a turn of thread `t1` on `w1` starts it, `extra` laid over.
fn doc(id: &str, extra: Value) -> Value {
    let mut d = json!({
        "id": id, "workspaceId": W, "harness": "claude", "status": "running", "threadId": THREAD, "startedAt": T0,
        "permissionMode": "bypassPermissions",
    });
    for (k, v) in extra.as_object().expect("extra is an object") {
        d[k] = v.clone();
    }
    d
}

fn turn_row(extra: Value) -> Row {
    row(serde_json::from_value(doc(TURN, extra)).expect("a session view"), TURN)
}

fn json_of(turn: &Row) -> Value {
    serde_json::to_value(&turn.borrow().view).expect("a view serializes")
}

/// The turn's live state as its row holds it now.
fn live_of(turn: &Row) -> TurnLive {
    turn.borrow().turn_live.clone().expect("the row's live state")
}

/// The row the start road keeps under the turn's id while the harness is still being reached.
fn launching(host: &FakeHost, turn: &Row) {
    host.add_row(TURN, turn.clone());
}

fn failed(message: &str) -> HostError {
    HostError::new(message)
}

fn turn_start(entry: &Entry, turn: &Row, session: &Arc<FakeSession>, slot: &Rc<RefCell<Option<OnEvent>>>) -> TurnStart {
    let (s, held) = (session.clone(), slot.clone());
    TurnStart {
        entry: entry.clone(),
        row: turn.clone(),
        written: None,
        thread_id: THREAD.into(),
        turn_id: TURN.into(),
        notify: None,
        notify_by: None,
        notify_road: None,
        turn_token: None,
        scope_device_id: None,
        outcome: SessionStartOutcome::Started,
        opening: Opening { prompt: "write it".into(), ..Opening::default() },
        resume: None,
        images_dir: None,
        snapshot: None,
        reports_edits: false,
        waiting: None,
        keep: None,
        open: Box::new(move |f: OnEvent| {
            *held.borrow_mut() = Some(f);
            Ok(s as Arc<dyn HarnessSession>)
        }),
    }
}

struct Rig {
    host: Rc<FakeHost>,
    turn: Row,
    session: Arc<FakeSession>,
    forward: OnEvent,
    handle: Rc<dyn SessionHandle>,
}

impl Rig {
    fn send(&self, doc: Value) {
        (self.forward)(adapter_event(doc))
    }
    fn row(&self) -> Row {
        self.host.rows.borrow().get(SESS).cloned().expect("the turn's row under its session id")
    }
    fn snapshot(&self) -> Option<String> {
        self.row().borrow().snapshot.clone()
    }
    fn token(&self) -> Option<String> {
        self.row().borrow().turn_token.clone()
    }
    fn pid(&self) -> Option<u32> {
        self.row().borrow().pid
    }
    fn calls_open(&self) -> usize {
        self.row().borrow().calls.as_ref().expect("calls").len()
    }
    fn end(&self, reason: &str, by_stop: bool, unreached: bool) {
        let end = self.row().borrow().end.clone().expect("the row's end");
        end(reason, Some(by_stop), Some(unreached))
    }
}

fn rig_on(host: &Rc<FakeHost>, entry: &Entry, session: Arc<FakeSession>, turn: Row, tweak: impl FnOnce(&mut TurnStart)) -> Rig {
    let slot: Rc<RefCell<Option<OnEvent>>> = Rc::new(RefCell::new(None));
    let mut t = turn_start(entry, &turn, &session, &slot);
    tweak(&mut t);
    let answers = session.answers;
    host.hooks.set::<dyn Fn(&dyn HarnessSession) -> bool>("TurnsCtx::session_answers", Rc::new(move |_s: &dyn HarnessSession| answers));
    let handle = TurnsArea::new(host.clone()).run_turn(t).expect("the turn opens");
    let forward = slot.borrow().clone().expect("open handed the forward over");
    Rig { host: host.clone(), turn, session, forward, handle }
}

fn rig() -> Rig {
    let (host, entry) = world();
    let turn = turn_row(json!({}));
    launching(&host, &turn);
    rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {})
}

fn rig_with(session: Arc<FakeSession>, tweak: impl FnOnce(&mut TurnStart)) -> Rig {
    let (host, entry) = world();
    let turn = turn_row(json!({}));
    launching(&host, &turn);
    rig_on(&host, &entry, session, turn, tweak)
}

/// A turn's state with no session behind it, for the methods that need none.
fn state_on(host: &Rc<FakeHost>, entry: &Entry, turn: &Row, tweak: impl FnOnce(&mut TurnStart)) -> Rc<TurnState> {
    let slot: Rc<RefCell<Option<OnEvent>>> = Rc::new(RefCell::new(None));
    let mut t = turn_start(entry, turn, &FakeSession::new(SESS), &slot);
    tweak(&mut t);
    TurnState::new(host.clone(), &t)
}

fn only(host: &FakeHost, names: &[&str]) -> Vec<String> {
    host.hooks.calls().into_iter().filter(|c| names.iter().any(|n| c == n || c.starts_with(&format!("{n}(")))).collect()
}

fn after(host: &FakeHost, mark: usize) -> Vec<String> {
    host.hooks.calls()[mark..].to_vec()
}

fn ask_doc(id: &str) -> Value {
    json!({
        "askId": id, "toolName": "Write", "toolUseId": "toolu_1",
        "input": "{\"file_path\":\"/root/out.txt\",\"content\":\"hi\"}", "detail": "out.txt",
        "options": [
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "deny", "label": "Deny", "effect": "deny" },
            { "id": "mode:acceptEdits", "label": "acceptEdits", "effect": "mode", "mode": "acceptEdits" },
        ],
    })
}

fn ask_event(id: &str) -> Value {
    json!({ "type": "permission.ask", "sessionId": SESS, "ask": ask_doc(id) })
}

fn question_event(id: &str) -> Value {
    let mut ask = ask_doc(id);
    ask["toolName"] = json!("AskUserQuestion");
    ask["options"] = json!([
        { "id": "q0:o0", "label": "Types", "effect": "answer" },
        { "id": "q0:o1", "label": "Lint", "effect": "answer" },
    ]);
    json!({ "type": "permission.ask", "sessionId": SESS, "ask": ask })
}

fn close_event(id: &str, outcome: &str) -> Value {
    json!({ "type": "permission.close", "sessionId": SESS, "askId": id, "outcome": outcome })
}

fn session_end() -> Value {
    json!({ "type": "session.end", "sessionId": SESS, "exitCode": 0, "sawResult": true })
}

fn done_event(res: Value) -> Value {
    json!({ "type": "turn.done", "sessionId": SESS, "result": res })
}

/// The asking line a row says for one of `ask_doc`'s prompts.
fn asking(ask: &Value) -> String {
    let mut d = ask.clone();
    d["workspaceId"] = json!(W);
    d["sessionId"] = json!(SESS);
    let ev: SessionPermissionEvent = serde_json::from_value(d).expect("a permission event");
    asking_line(&ev)
}

fn closed_row(ask_id: &str, outcome: &str, option: Option<&str>) -> Value {
    let mut v = json!({
        "type": "session.permission.closed", "workspaceId": W, "sessionId": SESS, "turnId": TURN, "threadId": THREAD,
        "askId": ask_id, "outcome": outcome,
    });
    if let Some(o) = option {
        v["optionId"] = json!(o);
    }
    v
}

/// A close row on the agent's own session id, which `closeAsk` writes where the view has one (turns.ts:255).
fn on_agent_session(mut row: Value) -> Value {
    row["sessionId"] = json!("agent-sess");
    row
}

fn finish(r: &Rig, res: Value) {
    r.session.finish(result(res));
}

fn num(n: i64) -> Option<JsNumber> {
    Some(JsNumber(n as f64))
}

fn scope() -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": "t-lead", "workspaceId": "w9", "rootThreadId": "t-lead" }))
        .expect("a scope")
}

fn answered(outcome: SessionAnswerOutcome) -> SessionAnswerResult {
    SessionAnswerResult { outcome }
}

fn kept_launch() -> KeptLaunch {
    let mut picks = IndexMap::new();
    picks.insert("model".to_string(), json!("opus"));
    KeptLaunch { fixed: "claude --print".into(), picks }
}

/// A kept agent that notes how it was closed.
struct FakeKept {
    closes: Arc<Mutex<Vec<bool>>>,
}

impl KeptAgent for FakeKept {
    fn next(&self, _turn: KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("no next turn here".into()))
    }
    fn close(&self, now: bool) -> LocalBoxFuture<'static, ()> {
        self.closes.lock().unwrap().push(now);
        futures::future::ready(()).boxed_local()
    }
    fn exited(&self) -> SharedFuture<()> {
        futures::future::ready(()).boxed_local().shared()
    }
}

fn keeper(closes: &Arc<Mutex<Vec<bool>>>) -> Box<dyn KeptAgent> {
    Box::new(FakeKept { closes: closes.clone() })
}

// ---- after_told (N9) ---------------------------------------------------------------------------------------------

fn told_state(told: Option<ToldAs>) -> (Rc<FakeHost>, Rc<TurnState>) {
    let (host, entry) = world();
    let turn = turn_row(json!({}));
    turn.borrow_mut().turn_live = Some(TurnLive { told_as: told, ..TurnLive::default() });
    let state = state_on(&host, &entry, &turn, |_| {});
    (host, state)
}

fn told(status: TurnStatus, body: Option<&str>, said: Option<&str>, promised: bool) -> Option<ToldAs> {
    Some(ToldAs { status, body: body.map(String::from), said: said.map(String::from), promised })
}

// ---- turn over (N3, N18) -----------------------------------------------------------------------------------------

fn rewound() -> Value {
    json!({ "harness": "claude", "rewound": { "before": "abc", "at": 1 } })
}

fn changes_hook(host: &FakeHost, asks: &Rc<RefCell<Vec<TurnChangesAsk>>>, answer: Rc<RefCell<Option<oneshot::Receiver<bool>>>>) {
    let a = asks.clone();
    host.hooks.set::<dyn Fn(&Entry, TurnChangesAsk) -> LocalBoxFuture<'static, Result<bool, HostError>>>(
        "TurnsCtx::read_turn_changes",
        Rc::new(move |_e: &Entry, ask: TurnChangesAsk| {
            a.borrow_mut().push(ask);
            let rx = answer.borrow_mut().take();
            async move {
                match rx {
                    Some(rx) => Ok(rx.await.unwrap_or(false)),
                    None => Ok(true),
                }
            }
            .boxed_local()
        }),
    );
}

fn taking() -> (oneshot::Sender<Option<String>>, TurnSnapshot) {
    let (tx, rx) = oneshot::channel::<Option<String>>();
    let from = rx.map(|r| r.unwrap_or(None)).boxed_local().shared();
    (tx, TurnSnapshot { from: SnapshotFrom::Taking(from), cwd: "/work/w1".into() })
}

fn changes_ask(from: &str, started_at: i64, wrote: Option<BTreeSet<String>>) -> TurnChangesAsk {
    TurnChangesAsk {
        session_id: SESS.into(),
        turn_id: TURN.into(),
        thread_id: THREAD.into(),
        cwd: "/work/w1".into(),
        from: from.into(),
        started_at,
        wrote,
    }
}

// ---- the exit (N21-N26) ------------------------------------------------------------------------------------------

type Holds = Rc<RefCell<Vec<(String, KeptHand)>>>;

fn hold_hook(host: &FakeHost, takes: bool) -> Holds {
    let holds: Holds = Rc::default();
    let h = holds.clone();
    host.hooks.set::<dyn Fn(&str, KeptHand) -> Option<KeptHand>>(
        "TurnsCtx::hold_kept",
        Rc::new(move |thread: &str, hold: KeptHand| {
            if !takes {
                return Some(hold);
            }
            h.borrow_mut().push((thread.to_string(), hold));
            None
        }),
    );
    holds
}

fn keeping(closes: &Arc<Mutex<Vec<bool>>>, pid: Option<u32>) -> Arc<FakeSession> {
    let c = closes.clone();
    FakeSession::with(SESS, move |s| {
        s.pid = pid;
        *s.kept.get_mut().unwrap() = Some(keeper(&c));
    })
}

fn chain() -> Vec<&'static str> {
    vec!["refresh_title(true)", "read_checkout(true)", "ran_on_branch(t1)", "read_pull_request(true)", "read_lead_of"]
}

const CHAIN: &[&str] =
    &["refresh_title", "read_checkout", "ran_on_branch", "read_pull_request", "read_lead_of", "pull_request_state", "settle_tree"];

fn entry_for(id: &str) -> Entry {
    entry(id, json!({}))
}

fn id_of(e: &Entry) -> String {
    e.borrow().record.borrow().id.clone()
}
