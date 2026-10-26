// SPDX-License-Identifier: AGPL-3.0-only
//! threads_tree: rows, trees, tokens and who is told. threads.ts:64-97, 303-362, 523-585, 642-661, 1144-1179.
//! Spec: spec/threads_tree.md.
use std::collections::VecDeque;

use indexmap::{IndexMap, IndexSet};
use wsp_core::console;
use wsp_core::types::internal::{ThreadRecord, TurnAsked};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::session::{SessionStatus, SessionView, ThreadPrompt, ThreadWaitingOn};
use wsp_wire::views::session_events::NOTIFY_ME;
use wsp_wire::views::workspace::{scope_of, Caller};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::runtime::SessionInterruptOutcome;
use wsp_words::exit::HostError;
use wsp_words::views::session::fold_threads;
use wsp_words::words::computer::thread_word;
use wsp_words::words::thread::NO_SUCH_TURN;
use wsp_words::workspace_state::is_local_workspace;
use wsp_words::wsp_tools::{threads_followed, Followed};

use super::{Entry, Row, ThreadRecords, ThreadsArea};

/// The marks an error born of a computer that did not answer carries (TR1), named after the TS classes.
pub const SILENT_MARKS: [&str; 6] =
    ["MachineUnreached", "MachineUnreachableError", "DeadlineError", "LaunchUnanswered", "PlaceAbsent", "DaemonUnanswered"];

/// threads.ts:67 `computerSilent`: TR1.
pub fn computer_silent(e: Option<&HostError>) -> bool {
    e.is_some_and(|e| e.marks.iter().any(|mark| SILENT_MARKS.contains(&mark.as_str())))
}

/// `{ notify, by?, road? }` (threads.ts:561).
#[derive(Clone, Debug, PartialEq)]
pub struct NotifyOn {
    pub notify: Vec<String>,
    pub by: Option<ThreadScope>,
    pub road: Option<WorkspaceOrigin>,
}

/// `{ by?, road? }` (threads.ts:615).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TellAs {
    pub by: Option<ThreadScope>,
    pub road: Option<WorkspaceOrigin>,
}

/// What `stopped_behind` reads of a row: its view and the tool calls it holds open.
#[derive(Clone, Debug)]
pub struct CallsOf {
    pub view: SessionView,
    pub calls: Option<IndexMap<String, wsp_core::types::internal::CallSeen>>,
}

/// What `tell_as` and `notify_end` read of a row.
#[derive(Clone, Debug, Default)]
pub struct Registered {
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<WorkspaceOrigin>,
    pub asked: Option<TurnAsked>,
}

/// `startedAt` as the TS reads it: absent is 0.
fn started_of(view: &SessionView) -> f64 {
    view.started_at.map_or(0.0, |at| at.0)
}

fn row_turns_thread(row: &SessionView, thread_id: &str) -> bool {
    row.thread_id.as_deref() == Some(thread_id)
}

impl ThreadsArea {
    /// Lends `f` the runtime's rows in map order.
    fn rows<T>(&self, f: impl FnOnce(&IndexMap<String, Row>) -> T) -> T {
        let sessions = self.ctx.sessions();
        let sessions = sessions.borrow();
        f(&sessions)
    }

    /// Lends `f` the thread records in map order.
    fn records<T>(&self, f: impl FnOnce(&IndexMap<String, ThreadRecord>) -> T) -> T {
        let records: ThreadRecords = self.ctx.thread_records();
        let records = records.borrow();
        f(&records)
    }

    /// threads.ts:75: TR2.
    pub fn thread_runs(&self, thread_id: &str) -> bool {
        self.rows(|rows| {
            rows.values().any(|row| {
                let row = row.borrow();
                row_turns_thread(&row.view, thread_id) && row.view.status == SessionStatus::Running
            })
        })
    }
    /// threads.ts:77: TR3. The turn id and its launch in flight.
    pub fn launching_on(&self, thread_id: &str) -> Option<(String, futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>)> {
        self.rows(|rows| {
            rows.values().find_map(|row| {
                let row = row.borrow();
                if !row_turns_thread(&row.view, thread_id) {
                    return None;
                }
                row.launch.clone().map(|launch| (row.turn_id.clone(), launch))
            })
        })
    }
    /// threads.ts:84: TR3. The row id and turn id of the thread's running row that holds a handle.
    pub fn running_on(&self, thread_id: &str) -> Option<(String, String)> {
        self.rows(|rows| {
            rows.values().find_map(|row| {
                let row = row.borrow();
                (row_turns_thread(&row.view, thread_id) && row.view.status == SessionStatus::Running && row.handle.is_some())
                    .then(|| (row.view.id.clone(), row.turn_id.clone()))
            })
        })
    }
    /// threads.ts:91: TR4.
    pub fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.rows(|rows| {
            let mut latest: Option<SessionView> = None;
            for row in rows.values() {
                let row = row.borrow();
                if !row_turns_thread(&row.view, thread_id) {
                    continue;
                }
                if latest.as_ref().is_none_or(|best| started_of(&row.view) >= started_of(best)) {
                    latest = Some(row.view.clone());
                }
            }
            latest
        })
    }
    /// threads.ts:307: TR5, invariant 3.
    pub fn thread_of_token(&self, token: &str) -> Result<String, HostError> {
        let running = self.rows(|rows| {
            rows.values().find_map(|row| {
                let row = row.borrow();
                if row.turn_token.as_deref() != Some(token) || row.view.status != SessionStatus::Running {
                    return None;
                }
                row.view.thread_id.clone()
            })
        });
        if let Some(thread_id) = running {
            return Ok(thread_id);
        }
        let kept =
            self.kept_agents.borrow().iter().find(|(_, kept)| kept.borrow().turn_token == token).map(|(thread_id, _)| thread_id.clone());
        kept.ok_or_else(|| HostError::new(NO_SUCH_TURN))
    }
    /// threads.ts:318: TR6.
    pub fn tree_under(&self, thread_id: &str) -> Vec<String> {
        let mut parents: IndexMap<String, String> = IndexMap::new();
        self.rows(|rows| {
            for row in rows.values() {
                let row = row.borrow();
                if let (Some(child), Some(parent)) = (&row.view.thread_id, &row.view.parent_thread_id) {
                    parents.insert(child.clone(), parent.clone());
                }
            }
        });
        self.records(|records| {
            for (id, record) in records {
                if let Some(parent) = &record.parent_thread_id {
                    parents.insert(id.clone(), parent.clone());
                }
            }
        });
        let mut found: Vec<String> = Vec::new();
        let mut front: Vec<String> = vec![thread_id.to_string()];
        let mut steps = parents.len() + 1;
        while steps > 0 && !front.is_empty() {
            let next: Vec<String> = parents
                .iter()
                .filter(|(id, parent)| front.contains(parent) && !found.contains(id) && id.as_str() != thread_id)
                .map(|(id, _)| id.clone())
                .collect();
            found.extend(next.iter().cloned());
            front = next;
            steps -= 1;
        }
        found
    }
    /// threads.ts:333: TR7.
    pub fn restarts(&self) -> IndexMap<String, String> {
        let mut found: IndexMap<String, String> = IndexMap::new();
        self.rows(|rows| {
            for row in rows.values() {
                let row = row.borrow();
                if let (Some(replaced), Some(thread_id)) = (&row.replaces, &row.view.thread_id) {
                    found.insert(replaced.clone(), thread_id.clone());
                }
            }
        });
        self.records(|records| {
            for (id, record) in records {
                if let Some(replaced) = &record.replaces {
                    found.insert(replaced.clone(), id.clone());
                }
            }
        });
        found
    }
    /// threads.ts:346: TR8.
    pub fn drives_thread(&self, thread_id: Option<&str>, caller: Option<&Caller>) -> bool {
        match scope_of(caller) {
            None => true,
            Some(scope) => thread_id.is_some_and(|thread_id| self.ctx.root_of(thread_id).as_deref() == Some(scope.root_thread_id.as_str())),
        }
    }
    /// threads.ts:353: TR9.
    pub fn settles_thread(&self, thread_id: &str, caller: Option<&Caller>) -> bool {
        match scope_of(caller) {
            None => true,
            Some(scope) => thread_id == scope.thread_id || self.tree_under(&scope.thread_id).iter().any(|child| child == thread_id),
        }
    }
    /// threads.ts:359: TR10.
    pub fn thread_title(&self, thread_id: &str) -> String {
        let mut rows: Vec<SessionView> =
            self.rows(|rows| rows.values().map(|row| row.borrow().view.clone()).filter(|view| row_turns_thread(view, thread_id)).collect());
        rows.sort_by(|a, b| started_of(a).total_cmp(&started_of(b)));
        fold_threads(&rows).first().map(|thread| thread.title.clone()).unwrap_or_else(|| thread_word(thread_id))
    }
    /// threads.ts:527: TR11.
    pub fn stopped_behind(&self, s: &CallsOf) -> Option<ThreadWaitingOn> {
        let caller = s.view.thread_id.as_deref()?;
        let calls = s.calls.as_ref()?;
        for call in calls.values() {
            let Some(follows) = threads_followed(&call.tool_name, &call.input) else {
                continue;
            };
            let behind: Vec<String> = match follows {
                Followed::Opened => self.tree_under(caller),
                Followed::Named(named) => self.rows(|rows| {
                    rows.values()
                        .filter_map(|row| row.borrow().view.thread_id.clone())
                        .filter(|id| named.iter().any(|reference| id.starts_with(reference.as_str())))
                        .collect()
                }),
            };
            for thread_id in behind {
                let Some(ask) = self.lead_asks.borrow().get(&thread_id).cloned() else {
                    continue;
                };
                let prompt = ThreadPrompt {
                    ask_id: ask.ask_id,
                    tool_name: ask.tool_name,
                    tool_use_id: ask.tool_use_id,
                    parent_tool_use_id: ask.parent_tool_use_id,
                    input: ask.input,
                    detail: ask.detail,
                    options: ask.options,
                };
                if thread_id == caller {
                    continue;
                }
                let asked = self.rows(|rows| {
                    rows.values().find_map(|row| {
                        let row = row.borrow();
                        (row_turns_thread(&row.view, &thread_id) && row.view.status == SessionStatus::Running)
                            .then(|| (row.view.workspace_id.clone(), row.view.id.clone()))
                    })
                });
                let Some((workspace_id, session_id)) = asked else {
                    continue;
                };
                let title = self.thread_title(&thread_id);
                return Some(ThreadWaitingOn { thread_id, workspace_id, session_id, title, prompt });
            }
        }
        None
    }
    /// threads.ts:546: TR12.
    pub async fn stop_under(&self, thread_id: &str, caller: Option<Caller>) -> Vec<String> {
        let mut stopped: Vec<String> = Vec::new();
        for child in self.tree_under(thread_id).into_iter().rev() {
            let Some(row) = self.latest_on(&child) else {
                continue;
            };
            if row.status != SessionStatus::Running {
                continue;
            }
            match self.ctx.interrupt(&row.id, caller.clone()).await {
                Ok(answer) => {
                    if answer.outcome == SessionInterruptOutcome::Accepted {
                        stopped.push(child);
                    }
                }
                Err(e) => console::warn(&format!("thread {} was not stopped with its root: {}", thread_word(&child), e.message)),
            }
        }
        stopped
    }
    /// threads.ts:561: TR13.
    pub fn notify_on(&self, thread_id: &str) -> Option<NotifyOn> {
        self.rows(|rows| {
            rows.values().find_map(|row| {
                let row = row.borrow();
                if !row_turns_thread(&row.view, thread_id) {
                    return None;
                }
                row.notify.clone().map(|notify| NotifyOn { notify, by: row.notify_by.clone(), road: row.notify_road })
            })
        })
    }
    /// threads.ts:572: TR14.
    pub fn tellable(row: Option<&SessionView>) -> bool {
        row.is_some_and(|row| row.claude_session_id.is_some())
    }
    /// threads.ts:576: TR15.
    pub fn notify_reach(&self, from: &[String]) -> IndexSet<String> {
        let mut seen: IndexSet<String> = IndexSet::new();
        let mut queue: VecDeque<String> = from.iter().filter(|target| target.as_str() != NOTIFY_ME).cloned().collect();
        while let Some(at) = queue.pop_front() {
            if seen.contains(&at) {
                continue;
            }
            let targets = self.notify_on(&at).map(|on| on.notify).unwrap_or_default();
            seen.insert(at);
            queue.extend(targets.into_iter().filter(|target| target.as_str() != NOTIFY_ME));
        }
        seen
    }
    /// threads.ts:615: TR16.
    pub fn tell_as(s: &Registered) -> TellAs {
        TellAs { by: s.notify_by.clone(), road: s.notify_road }
    }
    /// threads.ts:658: TR17.
    pub fn failure_of(&self, row_id: &str, turn_id: &str) -> Option<HostError> {
        let row = self.rows(|rows| rows.get(row_id).cloned())?;
        let row = row.borrow();
        if row.turn_id != turn_id {
            return None;
        }
        row.turn_live.as_ref().and_then(|live| live.failure.clone())
    }
    /// threads.ts:1149: TR18.
    pub fn usage_computer_of(r: &WorkspaceRecord) -> String {
        match &r.place {
            Some(place) => place.clone(),
            None if is_local_workspace(Some(&r.kind)) => HERE_PLACE_ID.to_string(),
            None => r.provider.clone().unwrap_or_else(|| r.kind.as_str().to_string()),
        }
    }
    /// threads.ts:1152: TR19. `vault` `None` reads `ctx.vault()`. Answers `"token"`, `"key"` or `None`.
    pub fn vaulted_for(&self, agent: &str, login_stands: Option<bool>, vault: Option<&IndexMap<String, String>>) -> Option<&'static str> {
        let vault = match vault {
            Some(vault) => vault.clone(),
            None => self.ctx.vault(),
        };
        let secrets = wsp_core::adapters::secrets_of(&vault, agent, login_stands.unwrap_or(false));
        if secrets.oauth_token.is_some() {
            Some("token")
        } else if secrets.api_key.is_some() {
            Some("key")
        } else {
            None
        }
    }
    /// The computer a usage record of this workspace keys on, and the vaulted sign-in its harness reads.
    fn usage_context(
        &self,
        entry: &Entry,
        harness: &str,
    ) -> (wsp_machines::usage::accounts::ComputerName, Option<wsp_machines::usage::Vaulted>) {
        let record = entry.borrow().record.borrow().clone();
        let login_stands = self.ctx.login_stands(entry, harness);
        let vault = self.ctx.vault_on(entry);
        let vaulted = match self.vaulted_for(harness, Some(login_stands), Some(&vault)) {
            Some("token") => Some(wsp_machines::usage::Vaulted::Token),
            Some(_) => Some(wsp_machines::usage::Vaulted::Key),
            None => None,
        };
        let computer =
            wsp_machines::usage::accounts::ComputerName { id: Self::usage_computer_of(&record), name: self.ctx.computer_of(entry) };
        (computer, vaulted)
    }
    /// threads.ts:1158: TR20. usage.ts's `accountOf` over this workspace's computer: `(key, label)`.
    pub fn usage_account_of(&self, entry: &Entry, harness: &str, named: Option<(String, Option<String>)>) -> (String, String) {
        let agent_name =
            wsp_core::harness_catalog::harness_catalog(harness).map(|catalog| catalog.label).unwrap_or_else(|| harness.to_string());
        let (computer, vaulted) = self.usage_context(entry, harness);
        let named = named.map(|(id, label)| wsp_machines::usage::accounts::NamedAccount { id, label });
        let account = wsp_machines::usage::accounts::account_of(harness, &agent_name, named.as_ref(), vaulted, &computer);
        (account.key, account.label)
    }
    /// threads.ts:1169: TR20.
    pub async fn limit_details_due(&self, entry: &Entry, harness: &str) -> bool {
        if !wsp_core::adapters::plan_resets().contains_key(harness) {
            return false;
        }
        let Ok(limits) = self.ctx.ledger_limits().await else {
            return false;
        };
        let agent_name =
            wsp_core::harness_catalog::harness_catalog(harness).map(|catalog| catalog.label).unwrap_or_else(|| harness.to_string());
        let (computer, vaulted) = self.usage_context(entry, harness);
        let (key, _) = wsp_machines::usage::accounts::account_on_computer(harness, &agent_name, &computer, &limits, vaulted);
        wsp_machines::usage::accounts::reset_details_due(limits.iter().find(|limit| limit.key == key), self.ctx.clock().now())
    }
}
