// SPDX-License-Identifier: AGPL-3.0-only
//! threads_kept: agent processes kept between turns and launched ahead. threads.ts:98-301. Spec:
//! spec/threads_kept.md.
use std::cell::RefCell;
use std::rc::Rc;

use crate::threads::unit;
use futures::future::{join, join_all, ready, LocalBoxFuture, Shared};
use futures::FutureExt;
use wsp_adapters::port::{SessionFile, SessionRenameWrite};
use wsp_core::console::warn;
use wsp_core::context::Joined;
use wsp_core::types::internal::{launches_as, same_session_file, stamp_session_file, KeptLaunch, KeptProcess, SessionFileStamp};
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::ExecOptions;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::exit::HostError;
use wsp_words::js::{js_trim, js_trim_start};
use wsp_words::words::computer::thread_word;

use super::{KeptHand, PlaceMove, ThreadsArea, ThreadsCtx};

pub use wsp_wire::wire::limits::{
    agents_kept_on, AGENTS_KEPT, AGENTS_WARM, AGENT_KEEP_MS, AGENT_WARM_MS, KEPT_AGENT_MB, KEPT_MEMORY_SHARE,
};
/// `{ place, cap }` (threads.ts:132).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptBox {
    pub place: String,
    pub cap: usize,
}

/// A kept process's stamped session file size, read on its joined computer.
type BoxRead = Shared<LocalBoxFuture<'static, Option<u64>>>;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// The shell line `boxFileSize` sends (K4).
pub fn box_file_line(at: &SessionFile) -> String {
    let glob = format!("{}{}/{}", shell_quote(&at.folder), "/*".repeat(at.depth as usize), shell_quote(&at.name));
    format!("for f in {glob}; do [ -f \"$f\" ] && {{ wc -c < \"$f\"; exit 0; }}; done; echo -1")
}

/// `parseInt(text, 10)` where the answer is a safe integer, else None.
fn js_int(text: &str) -> Option<i64> {
    let text = js_trim_start(text);
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text.strip_prefix('+').unwrap_or(text)),
    };
    let end = digits.bytes().take_while(u8::is_ascii_digit).count();
    if end == 0 {
        return None;
    }
    let n = digits[..end].parse::<i64>().ok()?.checked_mul(sign)?;
    (n.unsigned_abs() <= MAX_SAFE_INTEGER).then_some(n)
}

fn working(kept: &Rc<RefCell<KeptProcess>>) -> bool {
    kept.borrow().agent.working() == Some(true)
}

/// Takes the timer out of a kept process and runs it; a second call finds nothing to run.
fn cancel_timer(kept: &Rc<RefCell<KeptProcess>>) {
    let cancel = std::mem::replace(&mut kept.borrow_mut().cancel, Box::new(|| {}));
    cancel();
}

fn close_agent(ctx: &Rc<dyn ThreadsCtx>, kept: &Rc<RefCell<KeptProcess>>, now: bool) {
    let close = kept.borrow().agent.close(now);
    ctx.spawn(unit(close));
}

/// The earliest-used process, the first in map order on a tie (threads.ts `reduce`).
fn oldest_by_used(list: Vec<(String, Rc<RefCell<KeptProcess>>)>) -> Option<String> {
    let mut best: Option<(String, f64)> = None;
    for (thread, kept) in list {
        let at = kept.borrow().used_at.0;
        match &best {
            Some((_, seen)) if at >= *seen => {}
            _ => best = Some((thread, at)),
        }
    }
    best.map(|(thread, _)| thread)
}

fn process_of(o: KeptHand, file: Option<SessionFileStamp>, box_file: Option<BoxRead>, used_at: i64) -> KeptProcess {
    KeptProcess {
        workspace_id: o.workspace_id,
        agent: o.agent,
        pid: o.pid,
        launch: o.launch,
        session: o.session,
        warm: o.warm,
        turn_token: o.turn_token,
        scope_device_id: o.scope_device_id,
        waiting: o.waiting,
        file,
        box_file,
        used_at: JsNumber(used_at as f64),
        cancel: Box::new(|| {}),
    }
}

impl ThreadsArea {
    /// threads.ts:132: K1.
    pub fn kept_box(&self, workspace_id: &str) -> Option<KeptBox> {
        let entry = self.ctx.entry(workspace_id)?;
        let place = entry.borrow().record.borrow().place.clone()?;
        let cap = agents_kept_on(self.ctx.place_mem_mb(&place).unwrap_or(0.0)) as usize;
        Some(KeptBox { place, cap })
    }
    /// threads.ts:105: K2. `now`: end its tree at once.
    pub fn reap_kept(&self, thread_id: &str, now: bool) {
        let Some(kept) = self.kept_agents.borrow_mut().shift_remove(thread_id) else {
            return;
        };
        self.end_kept(thread_id, kept, now);
    }
    /// threads.ts:111: K3.
    pub fn end_kept(&self, thread_id: &str, kept: Rc<RefCell<KeptProcess>>, now: bool) {
        cancel_timer(&kept);
        let (device, workspace_id) = {
            let k = kept.borrow();
            (k.scope_device_id.clone(), k.workspace_id.clone())
        };
        if let Some(device) = device {
            let revoke = self.ctx.revoke_device(&device);
            let word = thread_word(thread_id);
            self.ctx.spawn(Box::pin(async move {
                if let Err(e) = revoke.await {
                    warn(&format!("the token of thread {word} was not taken away: {}", e.message));
                }
            }));
        }
        let place = if self.ctx.closing() { None } else { self.kept_box(&workspace_id).map(|b| b.place) };
        let Some(place) = place else {
            return close_agent(&self.ctx, &kept, now);
        };
        if !self.ctx.has_place_door() || self.ctx.place_linked(&place) {
            return close_agent(&self.ctx, &kept, now);
        }
        let stop: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::default();
        let (stop_in, kept_in, ctx_in) = (stop.clone(), kept.clone(), self.ctx.clone());
        let listener: Rc<dyn Fn(&PlaceMove)> = Rc::new(move |m: &PlaceMove| {
            let arrived = match m {
                PlaceMove::Present(p) if *p == place => true,
                PlaceMove::Removed(p) if *p == place => false,
                _ => return,
            };
            let off = stop_in.borrow_mut().take();
            if let Some(off) = off {
                off();
            }
            if arrived {
                close_agent(&ctx_in, &kept_in, now);
            }
        });
        match self.ctx.on_place_move(listener) {
            Some(off) => *stop.borrow_mut() = Some(off),
            None => close_agent(&self.ctx, &kept, now),
        }
    }
    /// threads.ts:140: K4. -1 where the agent wrote no file there.
    pub fn box_file_size(&self, workspace_id: &str, at: &SessionFile) -> LocalBoxFuture<'static, Option<i64>> {
        let Some(entry) = self.ctx.entry(workspace_id) else {
            return ready(None).boxed_local();
        };
        let machine = entry.borrow().machine.clone();
        let cmd = box_file_line(at);
        async move {
            let res = machine.exec(&cmd, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() }).await.ok()?;
            if res.exit_code.as_f64() != 0.0 {
                return None;
            }
            js_int(js_trim(&res.stdout))
        }
        .boxed_local()
    }
    /// The size read on the joined computer, started now and read later; the read runs even if nothing waits on it.
    fn start_box_read(&self, workspace_id: &str, at: &SessionFile) -> BoxRead {
        // The size is an i64 (-1 where no file was found) carried in wsp-core's u64 field; the cast round-trips.
        let read: BoxRead = self.box_file_size(workspace_id, at).map(|size| size.map(|n| n as u64)).boxed_local().shared();
        self.ctx.spawn(unit(read.clone()));
        read
    }
    /// threads.ts:160: K6.
    pub async fn kept_check(&self, thread_id: &str) {
        let Some(kept) = self.kept_agents.borrow().get(thread_id).cloned() else {
            return;
        };
        let workspace_id = kept.borrow().workspace_id.clone();
        let Some(place) = self.kept_box(&workspace_id).map(|b| b.place) else {
            return;
        };
        if !self.ctx.has_place_door() || !self.ctx.place_linked(&place) {
            return self.reap_kept(thread_id, false);
        }
        let session = kept.borrow().session.clone();
        if let Some(session) = session {
            let pending = self.host_writes.borrow().get(&session).cloned();
            if let Some(pending) = pending {
                pending.await;
            }
        }
        let (was, at) = {
            let k = kept.borrow();
            (k.box_file.clone(), k.agent.session_file())
        };
        if !self.is_kept(thread_id, &kept) || was.is_none() || at.is_none() || working(&kept) {
            return;
        }
        let (Some(was), Some(at)) = (was, at) else {
            return;
        };
        let (was, now) = join(was, self.box_file_size(&workspace_id, &at)).await;
        if let (Some(was), Some(now)) = (was.map(|n| n as i64), now) {
            if was != now && self.is_kept(thread_id, &kept) {
                self.reap_kept(thread_id, false);
            }
        }
    }
    /// threads.ts:177: K5.
    pub fn write_session(
        &self,
        harness_session_id: &str,
        write: LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>>,
    ) -> LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>> {
        let prior = self.host_writes.borrow().get(harness_session_id).cloned();
        let me = self.me.clone();
        let session = harness_session_id.to_string();
        let wrote = async move {
            let answer = match write.await {
                Ok(answer) => answer,
                Err(e) => return Err(e),
            };
            if matches!(answer, SessionRenameWrite::Written) {
                if let Some(area) = me.upgrade() {
                    area.restamp_session(&session).await;
                }
            }
            Ok(answer)
        }
        .boxed_local()
        .shared();
        let settled: Joined = {
            let wrote = wrote.clone();
            async move {
                if let Some(prior) = prior {
                    prior.await;
                }
                let _ = wrote.await;
            }
            .boxed_local()
            .shared()
        };
        self.host_writes.borrow_mut().insert(harness_session_id.to_string(), settled.clone());
        let me = self.me.clone();
        let key = harness_session_id.to_string();
        self.ctx.spawn(Box::pin(async move {
            settled.await;
            let Some(area) = me.upgrade() else {
                return;
            };
            let mut writes = area.host_writes.borrow_mut();
            if writes.get(&key).is_some_and(|held| held.peek().is_some()) {
                writes.shift_remove(&key);
            }
        }));
        wrote.boxed_local()
    }
    /// The processes kept on a session stamp its file again once the host has written it (threads.ts:180-195).
    async fn restamp_session(&self, session: &str) {
        let targets: Vec<Rc<RefCell<KeptProcess>>> =
            self.kept_agents.borrow().values().filter(|k| k.borrow().session.as_deref() == Some(session)).cloned().collect();
        let mut reads = Vec::new();
        for kept in targets {
            let (workspace_id, has_box, at) = {
                let k = kept.borrow();
                (k.workspace_id.clone(), k.box_file.is_some(), k.agent.session_file())
            };
            if let (true, Some(at)) = (has_box, at.clone()) {
                let read = self.start_box_read(&workspace_id, &at);
                kept.borrow_mut().box_file = Some(read.clone());
                reads.push(read);
                continue;
            }
            let known = kept.borrow().file.as_ref().and_then(|f| f.path.clone());
            if let Some(file) = stamp_session_file(at.as_ref(), known.as_deref()) {
                kept.borrow_mut().file = Some(file);
            }
        }
        join_all(reads).await;
    }
    /// threads.ts:206: K7.
    pub fn take_kept(
        &self,
        thread_id: &str,
        launch: Option<&KeptLaunch>,
        session: Option<&str>,
        whole: Option<&KeptLaunch>,
    ) -> Option<Rc<RefCell<KeptProcess>>> {
        let kept = self.kept_agents.borrow().get(thread_id).cloned()?;
        let (matches, session_ok, same_file) = {
            let k = kept.borrow();
            let asked = if k.warm.is_none() { launch } else { whole };
            let matches = asked.is_some_and(|asked| launches_as(&k.launch, asked) && (k.warm.is_none() || launches_as(asked, &k.launch)));
            let at = k.agent.session_file();
            (matches, k.session.as_deref() == session, same_session_file(at.as_ref(), k.file.as_ref()))
        };
        if !matches || !session_ok || (!working(&kept) && !same_file) {
            self.reap_kept(thread_id, false);
            return None;
        }
        self.kept_agents.borrow_mut().shift_remove(thread_id);
        cancel_timer(&kept);
        Some(kept)
    }
    /// threads.ts:224: K8. Nothing where it was kept (the turn's token and device stay with it only then); the hand
    /// back where the runtime refused it, so the turn closes its agent.
    pub fn hold_kept(&self, thread_id: &str, o: KeptHand) -> Option<KeptHand> {
        if self.ctx.closing() || !self.ctx.thread_records().borrow().contains_key(thread_id) {
            let close = o.agent.close(false);
            self.ctx.spawn(unit(close));
            return Some(o);
        }
        self.reap_kept(thread_id, false);
        let on_box = self.kept_box(&o.workspace_id).is_some();
        let at = o.agent.session_file();
        let file = if on_box { None } else { stamp_session_file(at.as_ref(), None) };
        let box_file = match (on_box, &at) {
            (true, Some(at)) => Some(self.start_box_read(&o.workspace_id, at)),
            _ => None,
        };
        let now = self.ctx.clock().now();
        self.keep(thread_id, process_of(o, file, box_file, now), AGENT_KEEP_MS);
        None
    }
    /// threads.ts:240: K9.
    pub fn hold_warm(&self, thread_id: &str, o: KeptHand) {
        if self.ctx.closing() {
            let process = process_of(o, None, None, 0);
            self.end_kept(thread_id, Rc::new(RefCell::new(process)), false);
            return;
        }
        let now = self.ctx.clock().now();
        self.keep(thread_id, process_of(o, None, None, now), AGENT_WARM_MS);
        self.ctx.proc_scopes_moved();
        loop {
            let standing: Vec<(String, Rc<RefCell<KeptProcess>>)> = self
                .kept_agents
                .borrow()
                .iter()
                .filter(|(_, k)| k.borrow().warm.as_ref().is_some_and(|w| !w.claimed))
                .map(|(t, k)| (t.clone(), k.clone()))
                .collect();
            if standing.len() as i64 <= AGENTS_WARM {
                return;
            }
            let Some(oldest) = oldest_by_used(standing) else {
                return;
            };
            self.reap_kept(&oldest, false);
        }
    }
    /// threads.ts:254: K10. The thread id and the process.
    pub fn warm_on(&self, workspace_id: &str, harness: &str) -> Option<(String, Rc<RefCell<KeptProcess>>)> {
        self.kept_agents
            .borrow()
            .iter()
            .find(|(_, k)| {
                let k = k.borrow();
                k.workspace_id == workspace_id && k.warm.as_ref().is_some_and(|w| w.harness == harness && !w.claimed)
            })
            .map(|(t, k)| (t.clone(), k.clone()))
    }
    /// threads.ts:257: K11.
    pub fn rewarm(&self, thread_id: &str) {
        let Some(kept) = self.kept_agents.borrow().get(thread_id).cloned() else {
            return;
        };
        cancel_timer(&kept);
        kept.borrow_mut().used_at = JsNumber(self.ctx.clock().now() as f64);
        self.arm(thread_id, kept, AGENT_WARM_MS);
    }
    /// threads.ts:266: K12.
    pub fn claim_warm(&self, workspace_id: &str, harness: &str) -> Option<String> {
        let (thread, kept) = self.warm_on(workspace_id, harness)?;
        if let Some(warm) = kept.borrow_mut().warm.as_mut() {
            warm.claimed = true;
        }
        self.ctx.proc_scopes_moved();
        Some(thread)
    }
    /// threads.ts:274: K13.
    pub(crate) fn arm(&self, thread_id: &str, kept: Rc<RefCell<KeptProcess>>, ms: i64) {
        let me = self.me.clone();
        let thread = thread_id.to_string();
        let timed = kept.clone();
        let cancel = self.ctx.clock().schedule(
            Box::new(move || {
                let Some(area) = me.upgrade() else {
                    return;
                };
                if !area.is_kept(&thread, &timed) {
                    return;
                }
                if working(&timed) {
                    area.arm(&thread, timed, ms);
                } else {
                    area.reap_kept(&thread, false);
                }
            }),
            ms,
        );
        kept.borrow_mut().cancel = Box::new(move || cancel());
    }
    /// threads.ts:281: K14.
    pub(crate) fn keep(&self, thread_id: &str, kept: KeptProcess, ms: i64) {
        let kept = Rc::new(RefCell::new(kept));
        self.arm(thread_id, kept.clone(), ms);
        self.kept_agents.borrow_mut().insert(thread_id.to_string(), kept.clone());
        self.watch_exit(thread_id, &kept);
        let workspace_id = kept.borrow().workspace_id.clone();
        let (place, cap) = match self.kept_box(&workspace_id) {
            Some(b) => (Some(b.place), b.cap),
            None => (None, AGENTS_KEPT as usize),
        };
        loop {
            let held: Vec<(String, Rc<RefCell<KeptProcess>>)> =
                self.kept_agents.borrow().iter().map(|(t, k)| (t.clone(), k.clone())).collect();
            let mut there = Vec::new();
            for (thread, other) in held {
                let ws = other.borrow().workspace_id.clone();
                if self.kept_box(&ws).map(|b| b.place) == place {
                    there.push((thread, other));
                }
            }
            if there.len() <= cap {
                return;
            }
            let idle: Vec<(String, Rc<RefCell<KeptProcess>>)> = there.into_iter().filter(|(_, other)| !working(other)).collect();
            let Some(oldest) = oldest_by_used(idle) else {
                return;
            };
            self.reap_kept(&oldest, false);
        }
    }
    /// Whether `kept` is still the process the map holds for `thread_id` (`keptAgents.get(t) === kept`).
    fn is_kept(&self, thread_id: &str, kept: &Rc<RefCell<KeptProcess>>) -> bool {
        self.kept_agents.borrow().get(thread_id).is_some_and(|k| Rc::ptr_eq(k, kept))
    }
    /// A process that exits on its own leaves the keep with its device; nothing is left to close (threads.ts:286-291).
    fn watch_exit(&self, thread_id: &str, kept: &Rc<RefCell<KeptProcess>>) {
        let exited = kept.borrow().agent.exited();
        let me = self.me.clone();
        let thread = thread_id.to_string();
        let kept = kept.clone();
        self.ctx.spawn(Box::pin(async move {
            exited.await;
            let Some(area) = me.upgrade() else {
                return;
            };
            if !area.is_kept(&thread, &kept) {
                return;
            }
            area.kept_agents.borrow_mut().shift_remove(&thread);
            cancel_timer(&kept);
            let device = kept.borrow().scope_device_id.clone();
            if let Some(device) = device {
                let _ = area.ctx.revoke_device(&device).await;
            }
        }));
    }
}
