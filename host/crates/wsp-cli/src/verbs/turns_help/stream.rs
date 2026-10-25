// SPDX-License-Identifier: AGPL-3.0-only
//! `turns-help.ts:832-1173` at 65f6b89c7: why a turn failed, what a watched turn prints, the answers to its prompts, and
//! the verbs' way through a followed and a detached start. Spec: `spec/turns-stream.md`.

use std::sync::Arc;

use wsp_wire::views::session::ThreadView;
use wsp_wire::views::session_events::SessionQueuedEvent;
use wsp_wire::views::session_events_more::SessionPermissionEvent;
use wsp_wire::wire::helpers::{PermissionEffect, PermissionOption};
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::HostError;

use super::follow::SharedTurn;
use super::{Picks, Turn};
use crate::verbs::client::{BoxFuture, Frame, HostClient, OpenedFn, VerbContext};
use crate::verbs::io::TurnOut;

/// `turnFailure(turn)` (turns-help.ts:832): nothing when `result.status` is `completed`; else `result.error`
/// where there is one (an empty string is one), else the turn's `reason`, else `turn <status>` or, with no result,
/// `turn ended without a result`.
pub fn turn_failure(turn: &Turn) -> Option<String> {
    let _ = turn;
    todo!("turns-help.ts:832")
}

/// `REFUSAL_THROWS` and `turnRefusal(turn)` (turns-help.ts:844): the failure as the error every door throws for
/// it, classed by the cause the adapter stamped on the result (`refusal`): `sign-in` is `auth_refusal(failure)`
/// (kind auth, no fix); a cause with no row, or none, is a plain error of the failure's words (provider class). Nothing
/// when the turn completed. The class is read off the stamp, never off the agent's words.
pub fn turn_refusal(turn: &Turn) -> Option<HostError> {
    let _ = turn;
    todo!("turns-help.ts:844")
}

/// `WAITING` (turns-help.ts:855).
pub const WAITING: &str = "waiting behind the running turn";

/// `waitingLine(e)` (turns-help.ts:857): `send_waits_for_line(waitsFor)` where the wait names a computer, else `WAITING`.
pub fn waiting_line(e: &SessionQueuedEvent) -> String {
    let _ = e;
    todo!("turns-help.ts:857")
}

/// `JOINED[outcome](picks, waitedFor)` (turns-help.ts:859) for the two outcomes that joined: `steered` says
/// `joined the running turn` and, where picks were named, `; --model, --effort, --access, --fast dropped, it keeps its
/// own model, effort and access` (the `PICK_FLAGS` named in that order, then `--fast` where `fast` is true; none
/// named adds nothing); `queued` says `send_waited_for_line(waitedFor)` where the last wait was on a computer, else
/// `queued behind the running turn; it has ended and this turn started`. `started` and `held` have no row.
pub fn joined_line(outcome: SessionStartOutcome, picks: &Picks, waited_for: Option<&str>) -> Option<String> {
    let _ = (outcome, picks, waited_for);
    todo!("turns-help.ts:859")
}

/// `WAITING_ON_A_PERSON` (turns-help.ts:870): `the turn is waiting on a permission; wsp threads shows it as <thread_state_word(waiting)>`.
pub fn waiting_on_a_person() -> String {
    todo!("turns-help.ts:870")
}

/// What `turnStream(ctx)` (turns-help.ts:878) keeps and does, one per followed turn; every method writes through
/// `ctx.out.stream`. `text(t, messageId)` writes prose as it arrives (nothing for an empty piece); another message's
/// prose after prose opens its own paragraph (a blank line, one newline fewer where a line start already stands);
/// `line(l)` and `says(l)` write a line of their own, first closing a line the stream stopped in the middle of (`line`
/// passes through `io.muted` where there is one); `reply(text)` closes an open line and answers the finished text
/// for stdout, except `None` where prose was streamed and `io.same_screen` says the stream is the person's screen.
pub struct TurnStream {
    _private: (),
}

impl TurnStream {
    pub fn new(ctx: &VerbContext) -> TurnStream {
        let _ = ctx;
        todo!("turns-help.ts:878")
    }

    pub fn text(&mut self, t: &str, message_id: Option<&str>) {
        let _ = (t, message_id);
        todo!("turns-help.ts:878")
    }

    pub fn line(&mut self, l: &str) {
        let _ = l;
        todo!("turns-help.ts:878")
    }

    pub fn says(&mut self, l: &str) {
        let _ = l;
        todo!("turns-help.ts:878")
    }

    pub fn reply(&mut self, text: Option<&str>) -> Option<String> {
        let _ = text;
        todo!("turns-help.ts:878")
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
    todo!("turns-help.ts:929")
}

/// `answerRoads(options)` (turns-help.ts:937): the roads this prompt's own options carry, in the roads' order,
/// each beside the first option of its effect; a road whose effect no option has is left out.
pub fn roads_of<'a>(options: &'a [PermissionOption]) -> Vec<(&'static AnswerRoad, &'a PermissionOption)> {
    let _ = options;
    todo!("turns-help.ts:937")
}

/// `answerKeysLine(options)` (turns-help.ts:946): `answer here: type ` then each road as `<key> to <does>` (or
/// the option's label with its first letter lowered), joined by `, `. With no road the line ends after `type `.
pub fn answer_keys_line(options: &[PermissionOption]) -> String {
    let _ = options;
    todo!("turns-help.ts:946")
}

/// `answerVerbsLine(threadId)` (turns-help.ts:878): `answer from another terminal: ` then `wsp thread <verb>
/// <thread_word(id)>` for each road with a verb, joined by `, or `.
pub fn answer_verbs_line(thread_id: &str) -> String {
    let _ = thread_id;
    todo!("turns-help.ts:878")
}

/// `ANSWER_IN_THE_APP` (turns-help.ts:962).
pub const ANSWER_IN_THE_APP: &str = "answer it in the app: this prompt carries its own answers";

/// `answeredLine(threadId, ask, said)` (turns-help.ts:966): `<said> on thread <thread_word(id)>: <asking_line(ask)>`.
pub fn answered_line(thread_id: &str, ask: &SessionPermissionEvent, said: &str) -> String {
    let _ = (thread_id, ask, said);
    todo!("turns-help.ts:966")
}

/// `noOpenAskLine(threadId)` (turns-help.ts:971).
pub fn no_open_ask_line(thread_id: &str) -> String {
    let _ = thread_id;
    todo!("turns-help.ts:971")
}

/// `noSuchAnswerLine(threadId, verb)` (turns-help.ts:976).
pub fn no_such_answer_line(thread_id: &str, verb: &str) -> String {
    let _ = (thread_id, verb);
    todo!("turns-help.ts:976")
}

/// `answerAsk(client, sessionId, askId, optionId, reason?)` (turns-help.ts:982): `sessions.answer {sessionId,
/// askId, optionId, reason?}` (`reason` left out where none); any outcome but `answered` fails with `ANSWER_WORDS[outcome]`.
pub async fn answer_ask(client: &dyn HostClient, session_id: &str, ask_id: &str, option_id: &str, reason: Option<&str>) -> Result<(), HostError> {
    let _ = (client, session_id, ask_id, option_id, reason);
    todo!("turns-help.ts:982")
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
pub async fn answer_open_ask(client: &dyn HostClient, r#ref: &str, road: &AnswerRoad, reason: Option<&str>) -> Result<AnsweredAsk, HostError> {
    let _ = (client, r#ref, road, reason);
    todo!("turns-help.ts:989")
}

/// What `answering(ctx, say)` (turns-help.ts:1004) does for a watched turn's prompts: `opened(ask, threadId)` says
/// `needs_you_line(ask)` and then the way to answer, in the words of whoever reads: `ANSWER_IN_THE_APP` where the
/// prompt carries no road, the verbs' line where no key is offered (`--json`, or no `answer_key` on the io), the keys'
/// line where a person is at the keyboard; with keys and roads it then reads one key from the io (releasing any earlier
/// read first) and a key that picks a road sends that option through `answer_ask` on `ctx.client()`'s socket as it
/// stands then; a failure of that goes to `io.error` as its message. `closed(askId)` releases the read of that prompt
/// only; `stop()` releases whatever is open.
pub struct Answering {
    _private: (),
}

impl Answering {
    pub fn new(ctx: Arc<VerbContext>, say: Arc<dyn Fn(&str) + Send + Sync>) -> Answering {
        let _ = (ctx, say);
        todo!("turns-help.ts:1004")
    }

    pub fn opened(&mut self, ask: &SessionPermissionEvent, thread_id: &str) {
        let _ = (ask, thread_id);
        todo!("turns-help.ts:1004")
    }

    pub fn closed(&mut self, ask_id: &str) {
        let _ = ask_id;
        todo!("turns-help.ts:1004")
    }

    pub fn stop(&mut self) {
        todo!("turns-help.ts:1004")
    }
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
    let _ = (ctx, line);
    todo!("turns-help.ts:1055")
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
    let _ = (ctx, client, start, announce, picks, said, on_turn);
    todo!("turns-help.ts:1059")
}

/// `beforeSending(client, read)` (turns-help.ts:1141): the read's answer; a read that fails on a socket the host
/// stopped (close code 4001) fails `NOT_DELIVERED_LINE` instead, any other failure as it is.
pub async fn before_sending<T: Send>(client: &dyn HostClient, read: BoxFuture<'_, Result<T, HostError>>) -> Result<T, HostError> {
    let _ = (client, read);
    todo!("turns-help.ts:1141")
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
    sub: Option<&wsp_wire::types::sub_thread::SubOut>,
    said: Option<&str>,
) -> Result<(), HostError> {
    let _ = (ctx, client, start, picks, opened, sub, said);
    todo!("turns-help.ts:1153")
}

/// `turnView(turn)` (turns-help.ts:1165): `threadId`, `workspaceId`, `harness`, `text` (the result's text or `""`,
/// present only where the turn has a result), `outcome`, `capped` (only a held turn's), `afterCut: true` (only where set).
pub fn turn_view(turn: &Turn) -> TurnOut {
    let _ = turn;
    todo!("turns-help.ts:1165")
}

/// Threads a refusal needs: `thread_of` is `workspaces_help::thread_of`; a stale view is `ThreadView`.
#[allow(dead_code)]
type _Thread = ThreadView;
