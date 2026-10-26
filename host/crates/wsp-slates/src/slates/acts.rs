// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_acts: slates.ts:470-474, 1290-1448, 1490-1493 at 65f6b89c7. Spec: spec/slates_acts.md.
use std::collections::HashSet;
use std::rc::Rc;

use futures::FutureExt;
use serde_json::{json, Map, Value};
use wsp_words::exit::{usage_refusal, HostError, Kind};

use super::batch::BatchOptions;
use super::model::{problem, refused, said, Approval, BatchBy, EVENTS_PER_SECOND, REQUESTS_KEPT, REQUEST_KEPT_MS, SLATE_SEND_KEY};
use super::{ApprovalScope, ApproveParams, DeliveryOutcome, EventParams, RecordRef, SlatesInner};
use crate::pending::slate::expr::resolve_slate_prop;
use crate::pending::slate::steps::slate_domain_key;
use crate::pending::slate::{BatchEvent, SlateRunDecl};
use crate::pending::wire::{SlateAsk, SlateEventAnswer, SlateEventOutcome, SlatesImageAnswer};
use crate::slate_files::{always_refused, with_files};
use crate::slate_images::{slate_image, SlateImageRoads};
use crate::slate_mcp::code_key;
use crate::slate_runs::{ApproveScope, Outcome, RunBy, RunRecord, RunStart, RunState, NO_FOLDER};

impl SlatesInner {
    pub(crate) async fn event(self: Rc<Self>, p: EventParams) -> Result<SlateEventAnswer, HostError> {
        let now = self.deps.clock().now();
        self.requests.borrow_mut().retain(|_, (at, _)| now - *at <= REQUEST_KEPT_MS);
        let seen = self.requests.borrow().get(&p.request_id).map(|(_, answer)| answer.clone());
        if let Some(answer) = seen {
            return answer.await;
        }
        let tokens = {
            let mut buckets = self.event_buckets.borrow_mut();
            let (held, at) = buckets.get(&p.thread_id).copied().unwrap_or((EVENTS_PER_SECOND, now));
            let refilled = (held + ((now - at) as f64 / 1000.0) * EVENTS_PER_SECOND).min(EVENTS_PER_SECOND);
            buckets.insert(p.thread_id.clone(), (refilled, now));
            refilled
        };
        if tokens < 1.0 {
            return Err(refused(&problem("V754", "event-rate", "too many presses; try again in a moment", None), Kind::Conflict));
        }
        self.event_buckets.borrow_mut().insert(p.thread_id.clone(), (tokens - 1.0, now));
        {
            let mut requests = self.requests.borrow_mut();
            while requests.len() >= REQUESTS_KEPT {
                let Some(oldest) = requests.keys().next().cloned() else { break };
                requests.shift_remove(&oldest);
            }
        }

        let this = self.clone();
        let event_params = p.clone();
        let composed = self.serial(&p.thread_id, move || async move {
            let r = this.clone().need_record(&event_params.thread_id).await?;
            let snap = r.borrow().clone();
            let piece = snap.document.as_ref().and_then(|doc| doc.pieces.get(&event_params.piece));
            if snap.document.is_none() || piece.is_none() {
                return Err(usage_refusal(
                    &format!(
                        "That part of the slate is gone (piece {}, version {}; the slate is at {}).",
                        event_params.piece, event_params.version, snap.version
                    ),
                    "Press it again once the slate has redrawn.",
                ));
            }
            if let Some(action) = event_params.row_action {
                let actions = piece.and_then(|piece| piece.props.as_ref()).and_then(|props| props.get("rowActions"));
                let has_action =
                    matches!(actions, Some(Value::Array(items)) if usize::try_from(action).ok().and_then(|i| items.get(i)).is_some());
                if !has_action {
                    return Err(usage_refusal(
                        &format!("{} has no row action {action}.", event_params.piece),
                        "Press it again once the slate has redrawn.",
                    ));
                }
            }
            let mut item: Option<Value> = None;
            if let Some(scope) = &event_params.scope {
                let views = this.clone().views_for(&snap, &[]).await;
                let ctx = this.context_of(&snap, &views, None);
                let list = piece
                    .and_then(|piece| piece.props.as_ref())
                    .and_then(|props| props.get("items"))
                    .and_then(|items| resolve_slate_prop(items, &ctx));
                item = match list {
                    Some(Value::Array(items)) => usize::try_from(scope.index).ok().and_then(|i| items.get(i)).cloned(),
                    _ => scope.item.clone(),
                };
            }
            this.runs().release(&event_params.thread_id);
            let event = BatchEvent {
                piece: event_params.piece.clone(),
                kind: event_params.event.clone(),
                item,
                index: event_params.scope.as_ref().map(|scope| scope.index),
                row_action: event_params.row_action,
            };
            let options = BatchOptions {
                event: Some(event),
                request_id: Some(event_params.request_id.clone()),
                send_at: Some(now),
                ..Default::default()
            };
            let out = this.clone().batch(r.clone(), Vec::new(), BatchBy::Person, options).await?;
            let started: Vec<String> = out
                .started
                .iter()
                .filter(|run| !matches!(snap.document.as_ref().and_then(|doc| doc.runs.get(*run)), Some(SlateRunDecl::Cmd(_))))
                .cloned()
                .collect();
            Ok((r, out, started))
        });

        let this = self.clone();
        let request_id = p.request_id.clone();
        let answer = async move {
            let outcome: Result<SlateEventAnswer, HostError> = async {
                let (r, out, started) = composed.await?;
                let (thread_id, workspace_id) = {
                    let rec = r.borrow();
                    (rec.thread_id.clone(), rec.workspace_id.clone())
                };
                let landed = match this.clone().deliver_all(thread_id.clone(), workspace_id, out.sends.clone()).await {
                    Ok(landed) => landed,
                    Err(e) => {
                        let kind = e.kind.unwrap_or(Kind::Conflict);
                        return Err(HostError::with_kind(format!("This thread cannot take a message now: {}", e.message), kind));
                    }
                };
                let held = out.asks.first().cloned();
                if held.is_none() && !started.is_empty() {
                    this.mcp().pending(&thread_id, Some(5_000)).await;
                }
                let snap = r.borrow().clone();
                let asks = this.asks_of(&snap);
                let ask = match &held {
                    None => asks.into_iter().find(|a| !matches!(a, SlateAsk::Cmd(_)) && started.contains(&a.run())),
                    Some(held) => asks.into_iter().find(|a| a.run() == held.run),
                };
                if let Some((delivered, kind)) = landed {
                    let outcome = match delivered.outcome {
                        DeliveryOutcome::Started => SlateEventOutcome::Started,
                        DeliveryOutcome::Steered => SlateEventOutcome::Steered,
                        DeliveryOutcome::Queued => SlateEventOutcome::Queued,
                    };
                    return Ok(SlateEventAnswer {
                        outcome,
                        said: said(&kind, delivered.outcome).to_string(),
                        turn_id: delivered.turn_id,
                        ask,
                    });
                }
                if let Some(ask) = ask {
                    return Ok(SlateEventAnswer {
                        outcome: SlateEventOutcome::Held,
                        said: "Needs your approval".to_string(),
                        turn_id: None,
                        ask: Some(ask),
                    });
                }
                Ok(SlateEventAnswer { outcome: SlateEventOutcome::Done, said: String::new(), turn_id: None, ask: None })
            }
            .await;
            if outcome.is_err() {
                this.requests.borrow_mut().shift_remove(&request_id);
            }
            outcome
        }
        .boxed_local()
        .shared();

        self.requests.borrow_mut().insert(p.request_id.clone(), (now, answer.clone()));
        answer.await
    }

    pub(crate) async fn approve(self: Rc<Self>, p: ApproveParams) -> Result<(), HostError> {
        let r = self.clone().need_record(&p.thread_id).await?;
        if p.key == SLATE_SEND_KEY {
            let this = self.clone();
            let scope = p.scope;
            return self
                .serial(&p.thread_id, move || async move {
                    {
                        let mut rec = r.borrow_mut();
                        rec.sends_allowed = if scope == ApprovalScope::Refuse { None } else { Some(this.deps.clock().now()) };
                    }
                    this.clone().save(r).await
                })
                .await;
        }
        let domain_prefix = slate_domain_key("");
        if let Some(domain) = p.key.strip_prefix(domain_prefix.as_str()) {
            if domain.is_empty() || !domain.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-') {
                return Err(usage_refusal(&format!("{} names no domain.", p.key), "Approve a link as domain:example.com."));
            }
            let this = self.clone();
            let (key, domain, scope) = (p.key.clone(), domain.to_string(), p.scope);
            return self
                .serial(&p.thread_id, move || async move {
                    match scope {
                        ApprovalScope::Thread => {
                            let at = this.deps.clock().now();
                            let approval =
                                Approval { state: "allowed".into(), at, run: None, cmd: Some(format!("links to {domain}")), scripts: None };
                            r.borrow_mut().approvals.insert(key, approval);
                        }
                        ApprovalScope::Refuse => {
                            r.borrow_mut().approvals.shift_remove(&key);
                        }
                        ApprovalScope::Once => {}
                    }
                    this.clone().save(r).await
                })
                .await;
        }
        if self.mcp().owns(&p.thread_id, &p.key) {
            let this = self.clone();
            let (key, scope) = (p.key.clone(), p.scope);
            return self
                .serial(&p.thread_id, move || async move {
                    let names = {
                        let rec = r.borrow();
                        this.approval_names(&rec, &key)
                    };
                    let at = this.deps.clock().now();
                    let thread = r.borrow().thread_id.clone();
                    if scope == ApprovalScope::Refuse {
                        let (run, cmd, scripts) = names;
                        r.borrow_mut().approvals.insert(key.clone(), Approval { state: "refused".into(), at, run, cmd, scripts });
                        this.clone().save(r).await?;
                        this.mcp().deny(&thread, &key);
                        return Ok(());
                    }
                    if scope == ApprovalScope::Thread {
                        let (run, cmd, scripts) = names;
                        r.borrow_mut().approvals.insert(key.clone(), Approval { state: "allowed".into(), at, run, cmd, scripts });
                    }
                    let snap = r.borrow().clone();
                    let views = this.clone().views_for(&snap, &[]).await;
                    this.views_now.borrow_mut().insert(thread.clone(), views);
                    this.clone().save(r).await?;
                    let asked = if scope == ApprovalScope::Thread { ApproveScope::Always } else { ApproveScope::Once };
                    this.mcp().approve(&thread, &key, asked);
                    Ok(())
                })
                .await;
        }
        let snap = r.borrow().clone();
        let named: Vec<String> = match snap.document.as_ref() {
            Some(doc) => doc
                .runs
                .iter()
                .filter(|(_, declared)| matches!(declared, SlateRunDecl::Cmd(_)))
                .filter(|(_, declared)| self.runs().key(&self.approval_decl(&snap, declared)) == p.key)
                .map(|(name, _)| name.clone())
                .collect(),
            None => Vec::new(),
        };
        if named.is_empty() {
            return Err(usage_refusal(
                &format!("this slate declares no command with approval key {}.", p.key),
                "Read the slate again and approve what it asks now.",
            ));
        }
        if p.scope == ApprovalScope::Thread {
            if let Some(doc) = snap.document.as_ref() {
                for run in &named {
                    let Some(declared) = doc.runs.get(run) else { continue };
                    let pin = self.pin_for(&p.thread_id, declared);
                    if let Some(said) = always_refused(run, &pin) {
                        return Err(usage_refusal(&said, "Approve it with scope once."));
                    }
                }
            }
        }
        let this = self.clone();
        let (key, scope) = (p.key.clone(), p.scope);
        self.serial(&p.thread_id, move || async move {
            if scope == ApprovalScope::Refuse {
                let names = {
                    let rec = r.borrow();
                    this.approval_names(&rec, &key)
                };
                let (run, cmd, scripts) = names;
                let at = this.deps.clock().now();
                r.borrow_mut().approvals.insert(key.clone(), Approval { state: "refused".into(), at, run, cmd, scripts });
                this.clone().save(r.clone()).await?;
                let thread = r.borrow().thread_id.clone();
                for run in &named {
                    this.runs().deny(&thread, run);
                }
                return Ok(());
            }
            if scope == ApprovalScope::Thread {
                let names = {
                    let rec = r.borrow();
                    this.approval_names(&rec, &key)
                };
                let (run, cmd, scripts) = names;
                let at = this.deps.clock().now();
                r.borrow_mut().approvals.insert(key.clone(), Approval { state: "allowed".into(), at, run, cmd, scripts });
            }
            let snap = r.borrow().clone();
            let views = this.clone().views_for(&snap, &[]).await;
            this.views_now.borrow_mut().insert(snap.thread_id.clone(), views);
            let thread = snap.thread_id.clone();
            let always = if scope == ApprovalScope::Thread { ApproveScope::Always } else { ApproveScope::Once };
            for run in &named {
                let Some(rec_value) = r.borrow().values.get(run).cloned() else { continue };
                let Some(last) = RunRecord::from_json(&rec_value) else { continue };
                let state = last.state();
                if !matches!(state, Some(RunState::Held) | Some(RunState::Running)) {
                    continue;
                }
                this.started_by.borrow_mut().insert(format!("{thread}\u{0}{run}"), RunBy::Person);
                if this.runs().approve(&thread, run, always).is_some() || state == Some(RunState::Running) {
                    continue;
                }
                let current = r.borrow().clone();
                let Some(declared) = current.document.as_ref().and_then(|doc| doc.runs.get(run)).cloned() else { continue };
                let SlateRunDecl::Cmd(cmd) = &declared else { continue };
                let Some(folder) = this.folder_for(&thread, &declared) else {
                    let mut failed = Map::new();
                    failed.insert("state".into(), json!("failed"));
                    failed.insert("why".into(), json!(NO_FOLDER));
                    failed.insert("exit".into(), Value::Null);
                    failed.insert("runs".into(), json!(last.runs()));
                    failed.insert("endedAt".into(), json!(this.deps.clock().now()));
                    this.moved(&thread, run, RunRecord(failed));
                    continue;
                };
                let again = this.runs().start(RunStart {
                    thread_id: thread.clone(),
                    run: run.clone(),
                    decl: this.approval_decl(&current, &declared),
                    by: RunBy::Person,
                    folder,
                    inputs: this.inputs_of(r.clone(), &cmd),
                    last: Some(last),
                    asleep: false,
                });
                if again.outcome == Outcome::Held {
                    this.runs().approve(&thread, run, always);
                }
            }
            if scope == ApprovalScope::Thread {
                let now_snap = r.borrow().clone();
                this.arm_timers(&now_snap, &HashSet::new());
            }
            this.clone().save(r).await
        })
        .await
    }

    pub(crate) async fn revoke(self: Rc<Self>, thread_id: &str, key: &str) -> Result<(), HostError> {
        let r = self.clone().need_record(thread_id).await?;
        let this = self.clone();
        let key = key.to_string();
        self.serial(thread_id, move || async move {
            if key == SLATE_SEND_KEY {
                if r.borrow().sends_allowed.is_none() {
                    return Err(usage_refusal(
                        "this thread's reactions were not allowed to message the agent.",
                        "Read its approvals again.",
                    ));
                }
                r.borrow_mut().sends_allowed = None;
                return this.clone().save(r).await;
            }
            if !r.borrow().approvals.contains_key(&key) {
                return Err(usage_refusal(&format!("this thread holds no approval {key}."), "Read its approvals again."));
            }
            r.borrow_mut().approvals.shift_remove(&key);
            let snap = r.borrow().clone();
            let mut covered: Vec<String> = Vec::new();
            if let Some(doc) = snap.document.as_ref() {
                for (name, declared) in &doc.runs {
                    let covers = match declared {
                        SlateRunDecl::Cmd(_) => this.runs().key(&this.approval_decl(&snap, declared)) == key,
                        _ => {
                            let args = this.mcp_args_of(r.clone(), declared)();
                            let decl = with_files(Some(doc), declared);
                            code_key(&decl, &args) == key
                        }
                    };
                    if covers {
                        covered.push(name.clone());
                    }
                }
            }
            let thread = snap.thread_id.clone();
            for run in &covered {
                this.runs().cancel(&thread, run);
                this.mcp().cancel(&thread, run);
            }
            let after = r.borrow().clone();
            let stopped: HashSet<String> = covered.into_iter().collect();
            this.arm_timers(&after, &stopped);
            this.clone().save(r).await
        })
        .await
    }

    pub(crate) async fn cancel(self: Rc<Self>, thread_id: &str, run: &str) -> Result<(), HostError> {
        self.clone().need_record(thread_id).await?;
        self.runs().cancel(thread_id, run);
        self.mcp().cancel(thread_id, run);
        Ok(())
    }

    pub(crate) async fn image(self: Rc<Self>, thread_id: &str, src: &str, have: Option<&str>) -> Result<SlatesImageAnswer, HostError> {
        let r: RecordRef = self.clone().need_record(thread_id).await?;
        let allowed_record = r.clone();
        let roads = SlateImageRoads {
            thread: thread_id.to_string(),
            have: have.map(String::from),
            folder: self.deps.thread(thread_id).and_then(|facts| facts.folder),
            on: self.deps.image_on(thread_id),
            allowed: Rc::new(move |domain: &str| {
                let key = slate_domain_key(domain);
                allowed_record.borrow().approvals.get(&key).is_some_and(|approval| approval.state == "allowed")
            }),
            fetch: None,
        };
        Ok(slate_image(src, roads).await)
    }
}
