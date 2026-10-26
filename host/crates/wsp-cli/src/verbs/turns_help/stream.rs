// SPDX-License-Identifier: AGPL-3.0-only
//! `turns-help.ts:832-1173` at 65f6b89c7: why a turn failed, what a watched turn prints, the answers to its prompts, and
//! the verbs' way through a followed and a detached start. Spec: `spec/turns-stream.md`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Map, Value};
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::oneshot;

use wsp_catalog::catalog::agent_name;
use wsp_wire::types::sub_thread::SubOut;
use wsp_wire::views::session::{SessionOrigin, ThreadCapWait, ThreadCapWaitWhy, ThreadView, TurnRefusal};
use wsp_wire::views::session_events::{
    agent_starting_line, DeltaKind, SessionDeltaEvent, SessionQueuedEvent, SessionStartingEvent, TurnStatus, NOTIFY_ME,
};
use wsp_wire::views::session_events_more::{SessionCappedEvent, SessionCappedEventWhy, SessionEvent, SessionPermissionEvent};
use wsp_wire::wire::helpers::{PermissionEffect, PermissionOption};
use wsp_wire::wire::runtime::{SessionAnswerResult, SessionStartOutcome};
use wsp_words::attachments::attachment_line;
use wsp_words::exit::{auth_refusal, HostError};
use wsp_words::place_state::{cap_started_line, cap_wait_line};
use wsp_words::thread_read::open_ask;
use wsp_words::thread_state::{needs_you_line, thread_state_word, ThreadState};
use wsp_words::words::base::lower_first;
use wsp_words::words::computer::{send_waited_for_line, send_waits_for_line, thread_word};
use wsp_words::words::thread::{AFTER_CUT_LINE, NOT_DELIVERED_LINE};
use wsp_words::words::tools::{answer_words, asking_line, tool_activity_line, tool_answered_line};
use wsp_words::words::turn::{compacted_line, retry_line, turn_settled_line, Retry};

use super::follow::{event_of, follow_heard, history, start_detached, FollowOn, RawEvent, SharedTurn};
use super::{Picks, Turn};
use crate::verbs::client::{
    flag_set, host_back, opened_thread_said, pick, stopped_under, thread_id_of, BoxFuture, CliIo, Frame, HostClient, OpenedFn, Out,
    VerbContext, HOST_RESTARTING_LINE,
};
use crate::verbs::io::TurnOut;
use crate::verbs::workspaces_help::thread_of;
use crate::zod_read::zod_read;

/// `turnFailure(turn)` (turns-help.ts:832): nothing when `result.status` is `completed`; else `result.error`
/// where there is one (an empty string is one), else the turn's `reason`, else `turn <status>` or, with no result,
/// `turn ended without a result`.
pub fn turn_failure(turn: &Turn) -> Option<String> {
    if turn.result.as_ref().is_some_and(|r| r.status == TurnStatus::Completed) {
        return None;
    }
    let named = turn.result.as_ref().and_then(|r| r.error.clone()).or_else(|| turn.reason.clone());
    match (named, &turn.result) {
        (Some(words), _) => Some(words),
        (None, Some(result)) => Some(format!("turn {}", status_word(result.status))),
        (None, None) => Some("turn ended without a result".to_owned()),
    }
}

fn status_word(status: TurnStatus) -> &'static str {
    match status {
        TurnStatus::Completed => "completed",
        TurnStatus::Interrupted => "interrupted",
        TurnStatus::Failed => "failed",
    }
}

/// `REFUSAL_THROWS` and `turnRefusal(turn)` (turns-help.ts:844): the failure as the error every door throws for
/// it, classed by the cause the adapter stamped on the result (`refusal`): `sign-in` is `auth_refusal(failure)`
/// (kind auth, no fix); a cause with no row, or none, is a plain error of the failure's words (provider class). Nothing
/// when the turn completed. The class is read off the stamp, never off the agent's words.
pub fn turn_refusal(turn: &Turn) -> Option<HostError> {
    let failure = turn_failure(turn)?;
    match turn.result.as_ref().and_then(|r| r.refusal) {
        Some(TurnRefusal::SignIn) => Some(auth_refusal(&failure)),
        None => Some(HostError::new(failure)),
    }
}

/// `WAITING` (turns-help.ts:855).
pub const WAITING: &str = "waiting behind the running turn";

/// `waitingLine(e)` (turns-help.ts:857): `send_waits_for_line(waitsFor)` where the wait names a computer, else `WAITING`.
pub fn waiting_line(e: &SessionQueuedEvent) -> String {
    match &e.waits_for {
        Some(name) => send_waits_for_line(name),
        None => WAITING.to_owned(),
    }
}

/// `JOINED[outcome](picks, waitedFor)` (turns-help.ts:859) for the two outcomes that joined: `steered` says
/// `joined the running turn` and, where picks were named, `; --model, --effort, --access, --fast dropped, it keeps its
/// own model, effort and access` (the `PICK_FLAGS` named in that order, then `--fast` where `fast` is true; none
/// named adds nothing); `queued` says `send_waited_for_line(waitedFor)` where the last wait was on a computer, else
/// `queued behind the running turn; it has ended and this turn started`. `started` and `held` have no row.
pub fn joined_line(outcome: SessionStartOutcome, picks: &Picks, waited_for: Option<&str>) -> Option<String> {
    match outcome {
        SessionStartOutcome::Steered => {
            let mut dropped: Vec<&str> = Vec::new();
            if picks.model.is_some() {
                dropped.push("--model");
            }
            if picks.effort.is_some() {
                dropped.push("--effort");
            }
            if picks.access.is_some() {
                dropped.push("--access");
            }
            if picks.fast {
                dropped.push("--fast");
            }
            if dropped.is_empty() {
                Some("joined the running turn".to_owned())
            } else {
                Some(format!("joined the running turn; {} dropped, it keeps its own model, effort and access", dropped.join(", ")))
            }
        }
        SessionStartOutcome::Queued => Some(match waited_for {
            Some(name) => send_waited_for_line(name),
            None => "queued behind the running turn; it has ended and this turn started".to_owned(),
        }),
        SessionStartOutcome::Started | SessionStartOutcome::Held => None,
    }
}

/// `WAITING_ON_A_PERSON` (turns-help.ts:870): `the turn is waiting on a permission; wsp threads shows it as <thread_state_word(waiting)>`.
pub fn waiting_on_a_person() -> String {
    format!("the turn is waiting on a permission; wsp threads shows it as {}", thread_state_word(ThreadState::Waiting))
}

/// What `turnStream(ctx)` (turns-help.ts:878) keeps and does, one per followed turn; every method writes through
/// `ctx.out.stream`. `text(t, messageId)` writes prose as it arrives (nothing for an empty piece); another message's
/// prose after prose opens its own paragraph (a blank line, one newline fewer where a line start already stands);
/// `line(l)` and `says(l)` write a line of their own, first closing a line the stream stopped in the middle of (`line`
/// passes through `io.muted` where there is one); `reply(text)` closes an open line and answers the finished text
/// for stdout, except `None` where prose was streamed and `io.same_screen` says the stream is the person's screen.
pub struct TurnStream {
    out: Out,
    io: Arc<dyn CliIo>,
    at_line_start: bool,
    streamed_prose: bool,
    said: Option<String>,
    last_was_prose: bool,
}

impl TurnStream {
    pub fn new(ctx: &VerbContext) -> TurnStream {
        TurnStream {
            out: ctx.out.clone(),
            io: ctx.io.clone(),
            at_line_start: true,
            streamed_prose: false,
            said: None,
            last_was_prose: false,
        }
    }

    pub fn text(&mut self, t: &str, message_id: Option<&str>) {
        if t.is_empty() {
            return;
        }
        let another_message = match (self.last_was_prose, self.said.as_deref(), message_id) {
            (true, Some(said), Some(id)) => id != said,
            _ => false,
        };
        if another_message {
            self.out.stream(if self.at_line_start { "\n" } else { "\n\n" });
            self.at_line_start = true;
        }
        if let Some(id) = message_id {
            self.said = Some(id.to_owned());
        }
        self.streamed_prose = true;
        self.last_was_prose = true;
        self.out.stream(t);
        self.at_line_start = t.ends_with('\n');
    }

    pub fn line(&mut self, l: &str) {
        let muted = self.io.muted(l).unwrap_or_else(|| l.to_owned());
        self.says(&muted);
    }

    pub fn says(&mut self, l: &str) {
        let breaks = if self.at_line_start { "" } else { "\n" };
        self.out.stream(&format!("{breaks}{l}\n"));
        self.at_line_start = true;
        self.last_was_prose = false;
    }

    pub fn reply(&mut self, text: Option<&str>) -> Option<String> {
        if !self.at_line_start {
            self.out.stream("\n");
        }
        self.at_line_start = true;
        if self.streamed_prose && self.io.same_screen() {
            None
        } else {
            text.map(str::to_owned)
        }
    }
}

/// `AnswerRoad.answer` (turns-help.ts:921): the verb, its help row and the line printed after that pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerVerb {
    pub verb: &'static str,
    pub about: &'static str,
    pub said: &'static str,
    /// `reasons: true`: a road whose verb takes the person's words on what to do instead, as the app's deny does.
    pub reasons: bool,
}

/// `AnswerRoad` (turns-help.ts:921): the key a person types while the turn is watched, and which of the prompt's
/// own options it picks, by the effect and never by the option's id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerRoad {
    pub key: &'static str,
    pub effect: PermissionEffect,
    /// What the line under the prompt says typing the key does; a road without takes the option's own label.
    pub does: Option<&'static str>,
    pub answer: Option<AnswerVerb>,
}

/// `ANSWER_ROADS` (turns-help.ts:929): `y` allow (`run it`; verb `allow`, said `allowed`), `n` deny (`refuse
/// it`; verb `deny`, said `denied`, reasons), `a` mode (no words of its own, no verb), in that order.
pub fn answer_roads() -> &'static [AnswerRoad] {
    &ANSWER_ROAD_ROWS
}

static ANSWER_ROAD_ROWS: [AnswerRoad; 3] = [
    AnswerRoad {
        key: "y",
        effect: PermissionEffect::Allow,
        does: Some("run it"),
        answer: Some(AnswerVerb {
            verb: "allow",
            about: "answers the prompt the thread is stopped on and lets the call run",
            said: "allowed",
            reasons: false,
        }),
    },
    AnswerRoad {
        key: "n",
        effect: PermissionEffect::Deny,
        does: Some("refuse it"),
        answer: Some(AnswerVerb {
            verb: "deny",
            about: "answers the prompt the thread is stopped on and refuses the call",
            said: "denied",
            reasons: true,
        }),
    },
    AnswerRoad { key: "a", effect: PermissionEffect::Mode, does: None, answer: None },
];

/// `answerRoads(options)` (turns-help.ts:937): the roads this prompt's own options carry, in the roads' order,
/// each beside the first option of its effect; a road whose effect no option has is left out.
pub fn roads_of(options: &[PermissionOption]) -> Vec<(&'static AnswerRoad, &PermissionOption)> {
    answer_roads().iter().filter_map(|road| options.iter().find(|o| o.effect == road.effect).map(|option| (road, option))).collect()
}

/// `answerKeysLine(options)` (turns-help.ts:946): `answer here: type ` then each road as `<key> to <does>` (or
/// the option's label with its first letter lowered), joined by `, `. With no road the line ends after `type `.
pub fn answer_keys_line(options: &[PermissionOption]) -> String {
    let typed: Vec<String> = roads_of(options)
        .into_iter()
        .map(|(road, option)| format!("{} to {}", road.key, road.does.map_or_else(|| lower_first(&option.label), str::to_owned)))
        .collect();
    format!("answer here: type {}", typed.join(", "))
}

/// `answerVerbsLine(threadId)` (turns-help.ts:878): `answer from another terminal: ` then `wsp thread <verb>
/// <thread_word(id)>` for each road with a verb, joined by `, or `.
pub fn answer_verbs_line(thread_id: &str) -> String {
    let verbs: Vec<String> = answer_roads()
        .iter()
        .filter_map(|road| road.answer.as_ref().map(|answer| format!("wsp thread {} {}", answer.verb, thread_word(thread_id))))
        .collect();
    format!("answer from another terminal: {}", verbs.join(", or "))
}

/// `ANSWER_IN_THE_APP` (turns-help.ts:962).
pub const ANSWER_IN_THE_APP: &str = "answer it in the app: this prompt carries its own answers";

/// `answeredLine(threadId, ask, said)` (turns-help.ts:966): `<said> on thread <thread_word(id)>: <asking_line(ask)>`.
pub fn answered_line(thread_id: &str, ask: &SessionPermissionEvent, said: &str) -> String {
    format!("{said} on thread {}: {}", thread_word(thread_id), asking_line(ask))
}

/// `noOpenAskLine(threadId)` (turns-help.ts:971).
pub fn no_open_ask_line(thread_id: &str) -> String {
    format!("thread {} is waiting on no prompt; wsp threads says which threads need you", thread_word(thread_id))
}

/// `noSuchAnswerLine(threadId, verb)` (turns-help.ts:976).
pub fn no_such_answer_line(thread_id: &str, verb: &str) -> String {
    format!("the prompt thread {} is stopped on takes no {verb}; wsp thread read shows what it asks", thread_word(thread_id))
}

/// `answerAsk(client, sessionId, askId, optionId, reason?)` (turns-help.ts:982): `sessions.answer {sessionId,
/// askId, optionId, reason?}` (`reason` left out where none); any outcome but `answered` fails with `ANSWER_WORDS[outcome]`.
pub async fn answer_ask(
    client: &dyn HostClient,
    session_id: &str,
    ask_id: &str,
    option_id: &str,
    reason: Option<&str>,
) -> Result<(), HostError> {
    let params = pick(vec![
        ("sessionId", Some(json!(session_id))),
        ("askId", Some(json!(ask_id))),
        ("optionId", Some(json!(option_id))),
        ("reason", reason.map(|reason| json!(reason))),
    ]);
    let reply = client.request("sessions.answer", params).await?;
    let answered = zod_read::<SessionAnswerResult>(&Value::Object(reply))?;
    match answer_words(answered.outcome) {
        None => Ok(()),
        Some(words) => Err(HostError::new(words)),
    }
}

/// What `answerOpenAsk` answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnsweredAsk {
    pub thread_id: String,
    pub ask_id: String,
    pub option_id: String,
    pub line: String,
}

/// `answerOpenAsk(client, ref, road, reason?)` (turns-help.ts:989): `thread_of(ref)`, then the thread's whole
/// transcript (`sessions.history`) read for its open prompt (`open_ask`); none fails `no_open_ask_line`; the first
/// option whose effect is the road's, none failing `no_such_answer_line(threadId, road verb)`; then `answer_ask`
/// (`sessions.answer`); the line is `answered_line(threadId, ask, road.said)`. `road` is one with a verb.
pub async fn answer_open_ask(
    client: &dyn HostClient,
    r#ref: &str,
    road: &AnswerRoad,
    reason: Option<&str>,
) -> Result<AnsweredAsk, HostError> {
    let answer = road.answer.as_ref().expect("a road that has a verb");
    let thread = thread_of(client, r#ref).await?;
    let thread_id = thread_id_of(&thread);
    let events = history(client, &thread.workspace_id, &thread_id).await?;
    let ask = open_ask(&events, &thread_id).ok_or_else(|| HostError::new(no_open_ask_line(&thread_id)))?;
    let option = ask
        .options
        .iter()
        .find(|option| option.effect == road.effect)
        .ok_or_else(|| HostError::new(no_such_answer_line(&thread_id, answer.verb)))?;
    answer_ask(client, &ask.session_id, &ask.ask_id, &option.id, reason).await?;
    Ok(AnsweredAsk {
        thread_id: thread_id.clone(),
        ask_id: ask.ask_id.clone(),
        option_id: option.id.clone(),
        line: answered_line(&thread_id, &ask, answer.said),
    })
}

/// What `answering(ctx, say)` (turns-help.ts:1004) does for a watched turn's prompts: `opened(ask, threadId)` says
/// `needs_you_line(ask)` and then the way to answer, in the words of whoever reads: `ANSWER_IN_THE_APP` where the
/// prompt carries no road, the verbs' line where no key is offered (`--json`, or no `answer_key` on the io), the keys'
/// line where a person is at the keyboard; with keys and roads it then reads one key from the io (releasing any earlier
/// read first) and a key that picks a road sends that option through `answer_ask` on `ctx.client()`'s socket as it
/// stands then; a failure of that goes to `io.error` as its message. `closed(askId)` releases the read of that prompt
/// only; `stop()` releases whatever is open.
pub struct Answering {
    ctx: Arc<VerbContext>,
    say: Arc<dyn Fn(&str) + Send + Sync>,
    open: Option<OpenRead>,
}

/// The key read a prompt holds open. Dropping the sender is the release: the read's `until` settles.
struct OpenRead {
    ask_id: String,
    _release: oneshot::Sender<()>,
}

impl Answering {
    pub fn new(ctx: Arc<VerbContext>, say: Arc<dyn Fn(&str) + Send + Sync>) -> Answering {
        Answering { ctx, say, open: None }
    }

    pub fn opened(&mut self, ask: &SessionPermissionEvent, thread_id: &str) {
        let roads = roads_of(&ask.options);
        let keyed = !flag_set(&self.ctx.flags, "json") && self.ctx.io.answers_keys();
        (self.say)(&needs_you_line(ask));
        let way = if roads.is_empty() {
            ANSWER_IN_THE_APP.to_owned()
        } else if keyed {
            answer_keys_line(&ask.options)
        } else {
            answer_verbs_line(thread_id)
        };
        (self.say)(&way);
        if !keyed || roads.is_empty() {
            return;
        }
        self.release();
        let (release, until) = oneshot::channel::<()>();
        self.open = Some(OpenRead { ask_id: ask.ask_id.clone(), _release: release });
        let keys: Vec<String> = roads.iter().map(|(road, _)| road.key.to_owned()).collect();
        let picks: Vec<(String, String)> = roads.iter().map(|(road, option)| (road.key.to_owned(), option.id.clone())).collect();
        let (ctx, session_id, ask_id) = (self.ctx.clone(), ask.session_id.clone(), ask.ask_id.clone());
        tokio::spawn(async move {
            let typed = ctx
                .io
                .answer_key(
                    &keys,
                    Box::pin(async move {
                        let _ = until.await;
                    }),
                )
                .await;
            if let Err(e) = answer_typed(&ctx, typed, &picks, &session_id, &ask_id).await {
                ctx.io.error(&e.message);
            }
        });
    }

    pub fn closed(&mut self, ask_id: &str) {
        if self.open.as_ref().is_some_and(|open| open.ask_id == ask_id) {
            self.release();
        }
    }

    pub fn stop(&mut self) {
        self.release();
    }

    fn release(&mut self) {
        self.open = None;
    }
}

/// The answer a key typed for a prompt picks: the option of the road the key names, sent on the socket as it stands
/// when the key is typed; a key that names no road sends nothing.
async fn answer_typed(
    ctx: &VerbContext,
    typed: Option<String>,
    picks: &[(String, String)],
    session_id: &str,
    ask_id: &str,
) -> Result<(), HostError> {
    let Some(typed) = typed else { return Ok(()) };
    let Some((_, option_id)) = picks.iter().find(|(key, _)| *key == typed) else { return Ok(()) };
    let client = ctx.client(None).await?;
    answer_ask(client.as_ref(), session_id, ask_id, option_id, None).await
}

/// `SaidAbout` (turns-help.ts:1043): the line that names the thread where the workspace was inferred, and the
/// word the turn's own figure carries.
#[derive(Clone, Default)]
pub struct SaidAbout {
    pub opened: Option<Arc<dyn Fn(&str, Option<&str>) -> String + Send + Sync>>,
    pub spend: Option<String>,
}

/// `waitSaid(ctx, line)` (turns-help.ts:1055): the line on stderr, except under `--json`, which holds back a line
/// about the wait as it holds the reply's stream.
pub fn wait_said(ctx: &VerbContext, line: &str) {
    if !flag_set(&ctx.flags, "json") {
        ctx.io.error(line);
    }
}

/// `followVerb(ctx, client, start, announce, picks, said, onTurn)` (turns-help.ts:1059): the verbs' way through a
/// turn. Follows `start` as `cli` (`follow`, redialling through `host_back(ctx)`); the turn's stderr is a person's:
/// `waitSaid` for each wait (`waiting_line`) and for the agent starting (`agent_starting_line(agent_name(harness),
/// installs)`); once the turn is named: `onTurn` is handed it live, with `announce` the `thread` object is emitted with
/// the opened line (`opened_thread_said`), each line of the notice goes to stderr, a joined outcome says `joined_line`
/// (the steer's `waiting` adds `WAITING_ON_A_PERSON`), a redial says `HOST_RESTARTING_LINE` and from then the reply is
/// printed whole. Each event: `done` emits itself with the reply for stdout (`reply()` of the stream, or the text whole
/// once redialled) else is emitted bare; `capped` and `uncapped` say their lines on stderr; `start` with `afterCut`
/// says `AFTER_CUT_LINE`, and each attachment its `attachment_line` through the stream; text deltas stream; notes and
/// the compacted and retry lines are the stream's aside lines; a tool call is a `tool_activity_line` and, by its
/// `toolUseId`, its pieces are kept to read its result against (`tool_answered_line`, nothing where that has no line;
/// the call forgotten once answered); a permission is `Answering::opened`, its close `closed`; the done is followed by
/// `turn_settled_line(result, spend)`; a notify event for `me` says its text on stderr. The answering is stopped on
/// every road out. A turn that did not complete is thrown as `turn_refusal`.
pub async fn follow_verb(
    ctx: &VerbContext,
    client: Arc<dyn HostClient>,
    start: &Frame,
    announce: bool,
    picks: &Picks,
    said: &SaidAbout,
    on_turn: Option<&(dyn Fn(&SharedTurn) + Send + Sync)>,
) -> Result<Turn, HostError> {
    let shared = Arc::new(copy_context(ctx));
    let stream = Arc::new(Mutex::new(TurnStream::new(ctx)));
    let say_stream = stream.clone();
    let say: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |line: &str| say_stream.lock().unwrap().says(line));
    let (seen_tx, mut seen_rx) = unbounded_channel::<Seen>();
    let tx = seen_tx.clone();
    let queued = move |e: &SessionQueuedEvent| {
        let _ = tx.send(Seen::Queued(e.clone()));
    };
    let tx = seen_tx.clone();
    let starting = move |e: &SessionStartingEvent| {
        let _ = tx.send(Seen::Starting(e.clone()));
    };
    let tx = seen_tx.clone();
    let steered = move |e: &wsp_wire::views::session_events::SessionSteerEvent| {
        let _ = tx.send(Seen::Steered(e.waiting == Some(true)));
    };
    let tx = seen_tx.clone();
    let started = move |turn: &SharedTurn| {
        let snapshot = turn.lock().unwrap().clone();
        let _ = tx.send(Seen::Started(turn.clone(), snapshot));
    };
    let tx = seen_tx.clone();
    let redialed = move || {
        let _ = tx.send(Seen::Redialed);
    };
    let tx = seen_tx.clone();
    let event: RawEvent = Arc::new(move |frame: &Frame, turn: &Turn| {
        let _ = tx.send(Seen::Event(frame.clone(), turn.thread_id.clone()));
    });
    let on = FollowOn {
        queued: Some(Arc::new(queued)),
        starting: Some(Arc::new(starting)),
        steered: Some(Arc::new(steered)),
        started: Some(Arc::new(started)),
        redialed: Some(Arc::new(redialed)),
        event: None,
    };
    let redial_ctx = shared.clone();
    let redial = move || -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> {
        let redial_ctx = redial_ctx.clone();
        Box::pin(async move { host_back(&redial_ctx.deps, None).await })
    };
    let redial_ref: &(dyn Fn() -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> + Send + Sync) = &redial;
    let mut following = Following {
        ctx: shared.clone(),
        stream,
        asks: Answering::new(shared.clone(), say),
        calls: HashMap::new(),
        joined_waiting: false,
        waited_for: None,
        redialed: false,
        turn: None,
    };
    let follow_turn = follow_heard(client, start, SessionOrigin::Cli, &on, Some(event), Some(redial_ref));
    tokio::pin!(follow_turn);
    let result = loop {
        tokio::select! {
            biased;
            Some(seen) = seen_rx.recv() => following.handle(seen, announce, picks, said, on_turn),
            done = &mut follow_turn => break done,
        }
    };
    while let Ok(seen) = seen_rx.try_recv() {
        following.handle(seen, announce, picks, said, on_turn);
    }
    following.asks.stop();
    following.settle();
    let turn = result?;
    match turn_refusal(&turn) {
        Some(failure) => Err(failure),
        None => Ok(turn),
    }
}

/// What the follow's callbacks hand over, in the order the follow heard them: the callbacks run inside the follow, so
/// each one is queued here and the verb's own loop writes them, which keeps `onTurn` ahead of the first event.
enum Seen {
    Queued(SessionQueuedEvent),
    Starting(SessionStartingEvent),
    Steered(bool),
    Started(SharedTurn, Turn),
    Redialed,
    Event(Frame, String),
}

/// A tool call the turn has open: its name and the input it has streamed so far.
struct Call {
    name: String,
    input: String,
}

/// What `followVerb` keeps while a turn streams: the stream it writes, the prompts it answers, the calls it has open
/// and what the start said before the turn was named.
struct Following {
    ctx: Arc<VerbContext>,
    stream: Arc<Mutex<TurnStream>>,
    asks: Answering,
    calls: HashMap<String, Call>,
    joined_waiting: bool,
    waited_for: Option<String>,
    redialed: bool,
    turn: Option<(SharedTurn, SharedTurn)>,
}

impl Following {
    fn handle(
        &mut self,
        seen: Seen,
        announce: bool,
        picks: &Picks,
        said: &SaidAbout,
        on_turn: Option<&(dyn Fn(&SharedTurn) + Send + Sync)>,
    ) {
        match seen {
            Seen::Queued(e) => {
                self.waited_for = e.waits_for.clone();
                wait_said(&self.ctx, &waiting_line(&e));
            }
            Seen::Starting(e) => wait_said(&self.ctx, &agent_starting_line(&agent_name(&e.harness), e.installs.is_some())),
            Seen::Steered(waiting) => self.joined_waiting = waiting,
            Seen::Started(live, snapshot) => self.started(live, snapshot, announce, picks, said, on_turn),
            Seen::Redialed => {
                self.redialed = true;
                self.ctx.io.error(HOST_RESTARTING_LINE);
            }
            Seen::Event(frame, thread_id) => self.event(&frame, &thread_id, said.spend.as_deref()),
        }
    }

    fn started(
        &mut self,
        live: SharedTurn,
        started: Turn,
        announce: bool,
        picks: &Picks,
        said: &SaidAbout,
        on_turn: Option<&(dyn Fn(&SharedTurn) + Send + Sync)>,
    ) {
        let view: SharedTurn = Arc::new(Mutex::new(started.clone()));
        if let Some(on_turn) = on_turn {
            on_turn(&view);
        }
        self.turn = Some((live, view));
        if announce {
            let thread = pick(vec![
                ("type", Some(json!("thread"))),
                ("id", Some(json!(started.thread_id))),
                ("workspaceId", Some(json!(started.session.workspace_id))),
                ("harness", Some(json!(started.session.harness))),
                ("startedBy", started.session.started_by.as_ref().map(|origin| json!(origin))),
            ]);
            let line = opened_thread_said(&started.thread_id, said.opened.as_deref(), started.session.cwd.as_deref());
            self.ctx.out.emit(&Value::Object(thread), Some(&line));
        }
        for line in started.notice.iter().flat_map(|notice| notice.split('\n')) {
            self.ctx.io.error(line);
        }
        if let Some(line) = joined_line(started.outcome, picks, self.waited_for.as_deref()) {
            self.ctx.io.error(&line);
        }
        if self.joined_waiting {
            self.ctx.io.error(&waiting_on_a_person());
        }
    }

    fn settle(&self) {
        if let Some((live, view)) = &self.turn {
            *view.lock().unwrap() = live.lock().unwrap().clone();
        }
    }

    fn event(&mut self, frame: &Frame, thread_id: &str, spend: Option<&str>) {
        let event = event_of(frame);
        if let Some(SessionEvent::SessionDoneEvent(done)) = &event {
            let reply = self.stream.lock().unwrap().reply(done.result.text.as_deref());
            let line = if self.redialed { done.result.text.clone() } else { reply };
            self.emit(frame, line.as_deref());
        } else {
            self.emit(frame, None);
        }
        let Some(event) = event else { return };
        match &event {
            SessionEvent::SessionCappedEvent(capped) => self.ctx.io.error(&cap_wait_line(&cap_wait_of(capped))),
            SessionEvent::SessionUncappedEvent(uncapped) => self.ctx.io.error(&cap_started_line(uncapped)),
            SessionEvent::SessionStartEvent(start) => {
                if start.after_cut.is_some() {
                    self.ctx.io.error(AFTER_CUT_LINE);
                }
                for file in start.attachments.iter().flatten() {
                    self.line(&attachment_line(file));
                }
            }
            SessionEvent::SessionDeltaEvent(delta) => self.delta(delta),
            SessionEvent::SessionCompactedEvent(compacted) => {
                self.line(&compacted_line(compacted.before.map(|n| n.0), compacted.after.map(|n| n.0)));
            }
            SessionEvent::SessionRetryEvent(retry) => {
                self.line(&retry_line(&Retry { attempt: retry.attempt.as_f64(), of: retry.of.map(|n| n.as_f64()), why: retry.why.clone() }))
            }
            SessionEvent::SessionPermissionEvent(ask) => self.asks.opened(ask, thread_id),
            SessionEvent::SessionPermissionClosedEvent(closed) => self.asks.closed(&closed.ask_id),
            SessionEvent::SessionDoneEvent(done) => self.line(&turn_settled_line(&done.result, spend)),
            SessionEvent::SessionNotifyEvent(notify) if notify.notify == NOTIFY_ME => self.ctx.io.error(&notify.text),
            _ => {}
        }
    }

    fn delta(&mut self, delta: &SessionDeltaEvent) {
        match delta.kind {
            DeltaKind::Text => self.stream.lock().unwrap().text(&delta.text, delta.message_id.as_deref()),
            DeltaKind::Note => self.line(&delta.text),
            DeltaKind::ToolUse => {
                self.line(&tool_activity_line(delta.tool_name.as_deref(), &delta.text));
                if let Some(id) = &delta.tool_use_id {
                    let open = self.calls.remove(id);
                    let name = delta
                        .tool_name
                        .clone()
                        .or_else(|| open.as_ref().map(|call| call.name.clone()))
                        .unwrap_or_else(|| "tool".to_owned());
                    let input = format!("{}{}", open.map(|call| call.input).unwrap_or_default(), delta.text);
                    self.calls.insert(id.clone(), Call { name, input });
                }
            }
            DeltaKind::ToolResult => {
                let call = delta.tool_use_id.as_ref().and_then(|id| self.calls.remove(id));
                let (name, input) = match call {
                    Some(call) => (Some(call.name), call.input),
                    None => (None, String::new()),
                };
                if let Some(answer) = tool_answered_line(name.as_deref(), &input, &delta.text, delta.is_error) {
                    self.line(&answer);
                }
            }
            DeltaKind::Thinking => {}
        }
    }

    fn line(&self, l: &str) {
        self.stream.lock().unwrap().line(l);
    }

    fn emit(&self, frame: &Frame, line: Option<&str>) {
        self.ctx.out.emit(&Value::Object(frame.clone()), line);
    }
}

fn cap_wait_of(capped: &SessionCappedEvent) -> ThreadCapWait {
    ThreadCapWait {
        place_id: capped.place_id.clone(),
        place: capped.place.clone(),
        running: capped.running,
        at_once: capped.at_once,
        why: capped.why.as_ref().map(|why| match why {
            SessionCappedEventWhy::Slot => ThreadCapWaitWhy::Slot,
            SessionCappedEventWhy::Minute => ThreadCapWaitWhy::Minute,
        }),
    }
}

/// A second copy of the verb's context, for the callbacks and the answer read that outlive the borrow of `ctx`: the
/// dialer, the io and the output are shared, so the copy writes and dials exactly as the original does.
fn copy_context(ctx: &VerbContext) -> VerbContext {
    VerbContext {
        deps: ctx.deps.clone(),
        args: ctx.args.clone(),
        flags: ctx.flags.clone(),
        io: ctx.io.clone(),
        out: ctx.out.clone(),
        aim: ctx.aim.clone(),
        usage: ctx.usage.clone(),
    }
}

/// `beforeSending(client, read)` (turns-help.ts:1141): the read's answer; a read that fails on a socket the host
/// stopped (close code 4001) fails `NOT_DELIVERED_LINE` instead, any other failure as it is.
pub async fn before_sending<T: Send>(client: &dyn HostClient, read: BoxFuture<'_, Result<T, HostError>>) -> Result<T, HostError> {
    match read.await {
        Ok(value) => Ok(value),
        Err(_) if stopped_under(client) => Err(HostError::new(NOT_DELIVERED_LINE)),
        Err(e) => Err(e),
    }
}

/// `detachVerb(ctx, client, start, picks, opened, sub, said)` (turns-help.ts:1153): `start_detached` as `cli`,
/// redialling through `host_back(ctx)`, each wait said as `wait_said(waiting_line)`; then the notice's lines on stderr;
/// a held turn with `session.capped` says `cap_wait_line` and any other joined outcome its `joined_line`; and the
/// thread object (`turn_view` plus `sub` and `notice: said` where given) is emitted with the opened line.
pub async fn detach_verb(
    ctx: &VerbContext,
    client: Arc<dyn HostClient>,
    start: &Frame,
    picks: &Picks,
    opened: Option<OpenedFn<'_>>,
    sub: Option<&SubOut>,
    said: Option<&str>,
) -> Result<(), HostError> {
    let shared = Arc::new(copy_context(ctx));
    let waited_for: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let (waited, wait_ctx) = (waited_for.clone(), shared.clone());
    let on_queued: Arc<dyn Fn(&SessionQueuedEvent) + Send + Sync> = Arc::new(move |e: &SessionQueuedEvent| {
        *waited.lock().unwrap() = e.waits_for.clone();
        wait_said(&wait_ctx, &waiting_line(e));
    });
    let redial_ctx = shared.clone();
    let redial = move || -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> {
        let redial_ctx = redial_ctx.clone();
        Box::pin(async move { host_back(&redial_ctx.deps, None).await })
    };
    let redial_ref: &(dyn Fn() -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> + Send + Sync) = &redial;
    let turn = start_detached(client, start, SessionOrigin::Cli, Some(on_queued), Some(redial_ref)).await?;
    let waited = waited_for.lock().unwrap().clone();
    for line in turn.notice.iter().flat_map(|notice| notice.split('\n')) {
        shared.io.error(line);
    }
    match (turn.outcome, turn.session.capped.as_ref()) {
        (SessionStartOutcome::Held, Some(capped)) => shared.io.error(&cap_wait_line(capped)),
        (outcome, _) => {
            if let Some(line) = joined_line(outcome, picks, waited.as_deref()) {
                shared.io.error(&line);
            }
        }
    }
    let mut view = match serde_json::to_value(turn_view(&turn)) {
        Ok(Value::Object(view)) => view,
        _ => Map::new(),
    };
    if let Some(sub) = sub {
        view.insert("sub".to_owned(), serde_json::to_value(sub).unwrap_or(Value::Null));
    }
    if let Some(said) = said {
        view.insert("notice".to_owned(), json!(said));
    }
    let line = opened_thread_said(&turn.thread_id, opened, turn.session.cwd.as_deref());
    shared.out.emit(&Value::Object(view), Some(&line));
    Ok(())
}

/// `turnView(turn)` (turns-help.ts:1165): `threadId`, `workspaceId`, `harness`, `text` (the result's text or `""`,
/// present only where the turn has a result), `outcome`, `capped` (only a held turn's), `afterCut: true` (only where set).
pub fn turn_view(turn: &Turn) -> TurnOut {
    let capped = match turn.outcome {
        SessionStartOutcome::Held => turn.session.capped.clone(),
        _ => None,
    };
    TurnOut {
        thread_id: turn.thread_id.clone(),
        workspace_id: turn.session.workspace_id.clone(),
        harness: turn.session.harness.clone(),
        text: turn.result.as_ref().map(|result| result.text.clone().unwrap_or_default()),
        outcome: turn.outcome,
        capped,
        after_cut: turn.after_cut.then_some(true),
        notice: None,
    }
}

/// Threads a refusal needs: `thread_of` is `workspaces_help::thread_of`; a stale view is `ThreadView`.
#[allow(dead_code)]
type _Thread = ThreadView;
