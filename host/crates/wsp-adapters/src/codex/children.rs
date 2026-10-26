// SPDX-License-Identifier: AGPL-3.0-only
//! A turn's subagents: more threads on the same server, which its lead spawned. TS adapter-codex/src/adapter.ts:445-461
//! and 664-841 (`learn`, `endChild`, `childSigns`, `sayChild`, `sayTasks`, `onChild`). Spec: spec/codex_turn.md rule 12.
//! Each method pushes the events it says onto `out`, in order, and emits nothing itself (rule 3.3).

use indexmap::IndexMap;
use serde_json::{Map, Value};

use super::fields::{count, rec, str};
use super::items::{child_end, child_usage_of, depth_of, item_of, ChildTokens, BREAKDOWN};
use crate::pending::subagent_asked;
use crate::port::{AdapterEvent, SubagentState, SubagentUsage};
use wsp_wire::types::js_number::JsNumber;
use wsp_words::js::js_trim;

/// TS adapter.ts:446.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub state: SubagentState,
    /// The turn it has in progress, which a stop names.
    pub turn_id: Option<String>,
    /// The call that spawned it; its own thread id until one is seen.
    pub parent: String,
    pub title: Option<String>,
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub depth: Option<u32>,
    pub summary: Option<String>,
    pub usage: Option<SubagentUsage>,
}

/// A sign of a subagent and what it says of it. TS adapter.ts:691's `said`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Said {
    pub parent: Option<String>,
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub name: Option<String>,
    pub depth: Option<u32>,
}

/// The turn's subagents by thread id, in the order first seen, the tokens their calls drew, and the task count last said.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Children {
    pub children: IndexMap<String, Child>,
    pub child_tokens: ChildTokens,
    pub tasks_said: u32,
}

impl Children {
    /// Running children, a turn id or not. TS adapter.ts:664.
    pub fn running(&self) -> usize {
        self.children.values().filter(|c| c.state == SubagentState::Running).count()
    }

    /// Each running child with a turn in progress, as `(thread id, turn id)`, in order. TS adapter.ts:1198.
    pub fn running_turns(&self) -> Vec<(String, String)> {
        self.children
            .iter()
            .filter(|(_, c)| c.state == SubagentState::Running)
            .filter_map(|(task, c)| c.turn_id.as_ref().map(|turn| (task.clone(), turn.clone())))
            .collect()
    }

    /// `turn.tasks` where the count of running children with a turn changed. TS adapter.ts:667.
    pub fn say_tasks(&mut self, lead: &str, out: &mut Vec<AdapterEvent>) {
        let running = self.children.values().filter(|c| c.state == SubagentState::Running && c.turn_id.is_some()).count() as u32;
        if running == self.tasks_said {
            return;
        }
        self.tasks_said = running;
        out.push(AdapterEvent::TurnTasks { session_id: lead.to_string(), running, replied: None });
    }

    /// The `subagent` event for one child as it stands. TS adapter.ts:673.
    pub fn say_child(&self, lead: &str, task: &str, out: &mut Vec<AdapterEvent>) {
        let Some(c) = self.children.get(task) else { return };
        let running = c.state == SubagentState::Running;
        out.push(AdapterEvent::Subagent {
            session_id: lead.to_string(),
            task: task.to_string(),
            state: c.state,
            parent_tool_use_id: Some(c.parent.clone()),
            title: if running { c.title.clone() } else { None },
            summary: if running { None } else { c.summary.clone() },
            depth: if running { c.depth.map(|d| JsNumber(f64::from(d))) } else { None },
            model: if running { c.model.clone() } else { None },
            asked: if running { c.prompt.as_deref().map(subagent_asked) } else { None },
            usage: if running { None } else { c.usage.clone() },
        });
    }

    /// TS adapter.ts:691. A task equal to the lead's thread id is ignored.
    pub fn learn(&mut self, lead: &str, task: &str, said: Said, out: &mut Vec<AdapterEvent>) {
        if task == lead {
            return;
        }
        let title = said
            .prompt
            .as_deref()
            .map(|p| p.split('\n').next().unwrap_or(""))
            .map(js_trim)
            .filter(|t| !t.is_empty())
            .map(String::from)
            .or_else(|| said.name.clone());
        if !self.children.contains_key(task) {
            let fresh = Child {
                state: SubagentState::Running,
                turn_id: None,
                parent: said.parent.clone().unwrap_or_else(|| task.to_string()),
                title,
                prompt: said.prompt.clone(),
                model: said.model.clone(),
                depth: said.depth,
                summary: None,
                usage: None,
            };
            self.children.insert(task.to_string(), fresh);
            self.say_child(lead, task, out);
            self.say_tasks(lead, out);
            return;
        }
        let Some(c) = self.children.get_mut(task) else { return };
        let adds = (said.parent.is_some() && c.parent == task)
            || (said.prompt.is_some() && c.prompt.is_none())
            || (said.model.is_some() && c.model.is_none())
            || (said.depth.is_some() && c.depth.is_none())
            || (title.is_some() && c.title.is_none());
        if let Some(parent) = &said.parent {
            if c.parent == task {
                c.parent = parent.clone();
            }
        }
        if let Some(prompt) = &said.prompt {
            if c.prompt.is_none() {
                c.prompt = Some(prompt.clone());
                c.title = title.clone();
            }
        }
        c.depth = c.depth.or(said.depth);
        c.model = c.model.clone().or_else(|| said.model.clone());
        c.title = c.title.clone().or_else(|| title.clone());
        if adds && c.state == SubagentState::Running {
            self.say_child(lead, task, out);
        }
    }

    /// TS adapter.ts:713. True where the child was running and has now ended, so the caller settles the lead.
    pub fn end_child(&mut self, lead: &str, task: &str, state: SubagentState, out: &mut Vec<AdapterEvent>) -> bool {
        let Some(c) = self.children.get_mut(task) else { return false };
        if c.state != SubagentState::Running {
            return false;
        }
        c.state = state;
        c.turn_id = None;
        self.say_child(lead, task, out);
        self.say_tasks(lead, out);
        true
    }

    /// The subagents a collab call or an activity item names, wherever on the server it was printed. TS adapter.ts:723.
    /// True where any child ended.
    pub fn child_signs(&mut self, lead: &str, item: &Map<String, Value>, done: bool, out: &mut Vec<AdapterEvent>) -> bool {
        let id = str(item.get("id")).unwrap_or("").to_string();
        let mut ended = false;
        if str(item.get("type")) == Some("collabAgentToolCall") {
            let receivers: Vec<String> = item
                .get("receiverThreadIds")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect();
            let tool = str(item.get("tool"));
            let status = str(item.get("status"));
            if tool == Some("spawnAgent") && status != Some("failed") {
                for task in &receivers {
                    let said = Said {
                        parent: Some(id.clone()),
                        prompt: str(item.get("prompt")).map(String::from),
                        model: str(item.get("model")).filter(|m| !m.is_empty()).map(String::from),
                        ..Said::default()
                    };
                    self.learn(lead, task, said, out);
                }
            }
            if tool == Some("closeAgent") && done && status == Some("completed") {
                for task in &receivers {
                    ended |= self.end_child(lead, task, SubagentState::Done, out);
                }
            }
        }
        if str(item.get("type")) == Some("subAgentActivity") {
            let Some(task) = str(item.get("agentThreadId")) else { return ended };
            if task == lead {
                return ended;
            }
            match str(item.get("kind")) {
                Some("started") => {
                    let path = str(item.get("agentPath"));
                    let said = Said {
                        parent: Some(id),
                        name: path.map(|p| p.rsplit('/').next().unwrap_or("").to_string()),
                        depth: path.map(depth_of),
                        ..Said::default()
                    };
                    self.learn(lead, task, said, out);
                }
                Some("completed") => ended |= self.end_child(lead, task, SubagentState::Done, out),
                Some("interrupted") => ended |= self.end_child(lead, task, SubagentState::Stopped, out),
                _ => {}
            }
        }
        ended
    }

    /// Everything else a subagent's thread prints on the lead's stdout: its turn, its tokens, its summary and the signs
    /// in its items. The caller has already counted its commands and kept its changes. TS adapter.ts:791. True where a
    /// child ended.
    pub fn on_child(
        &mut self,
        lead: &str,
        task: &str,
        method: &str,
        params: &Map<String, Value>,
        emitted_at_ms: Option<f64>,
        out: &mut Vec<AdapterEvent>,
    ) -> bool {
        match method {
            "turn/started" => {
                let turn = str(rec(params.get("turn")).and_then(|t| t.get("id"))).map(String::from);
                self.learn(lead, task, Said::default(), out);
                let Some(c) = self.children.get_mut(task) else { return false };
                if let Some(id) = turn {
                    c.turn_id = Some(id);
                }
                let restarted = c.state != SubagentState::Running;
                if restarted {
                    c.state = SubagentState::Running;
                    c.summary = None;
                    self.say_child(lead, task, out);
                }
                self.say_tasks(lead, out);
                false
            }
            "turn/completed" => {
                let turn = rec(params.get("turn"));
                let id = turn.and_then(|t| str(t.get("id")));
                let open = self.children.get(task).and_then(|c| c.turn_id.as_deref());
                if let (Some(open), Some(id)) = (open, id) {
                    if id != open {
                        return false;
                    }
                }
                let status = turn.and_then(|t| str(t.get("status")));
                self.end_child(lead, task, child_end(status), out)
            }
            "item/started" | "item/completed" => {
                let Some(item) = item_of(params) else { return false };
                let done = method == "item/completed";
                if done && str(item.get("type")) == Some("agentMessage") {
                    if let Some(c) = self.children.get_mut(task) {
                        c.summary = str(item.get("text")).map(String::from);
                    }
                }
                self.child_signs(lead, &item, done, out)
            }
            "thread/tokenUsage/updated" => {
                let token_usage = rec(params.get("tokenUsage"));
                let Some(last) = token_usage.and_then(|u| rec(u.get("last"))) else { return false };
                let tokens = count(last.get("inputTokens")).unwrap_or(0.0) + count(last.get("outputTokens")).unwrap_or(0.0);
                out.push(AdapterEvent::TurnUsage { session_id: lead.to_string(), tokens, context: None, window: None, at: emitted_at_ms });
                for (name, key) in BREAKDOWN {
                    if let Some(n) = count(last.get(key)) {
                        *self.child_tokens.entry(name.to_string()).or_insert(0.0) += n;
                    }
                }
                let usage = child_usage_of(token_usage.and_then(|u| rec(u.get("total"))));
                let Some(c) = self.children.get_mut(task) else { return false };
                let Some(usage) = usage else { return false };
                c.usage = Some(usage.clone());
                if c.state == SubagentState::Running {
                    out.push(AdapterEvent::Subagent {
                        session_id: lead.to_string(),
                        task: task.to_string(),
                        state: SubagentState::Running,
                        parent_tool_use_id: Some(c.parent.clone()),
                        title: None,
                        summary: None,
                        depth: None,
                        model: None,
                        asked: None,
                        usage: Some(usage),
                    });
                }
                false
            }
            _ => false,
        }
    }
}
