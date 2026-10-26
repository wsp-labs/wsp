// SPDX-License-Identifier: AGPL-3.0-only
//! The plan an agent keeps in its calls. TS: packages/adapter-claude/src/plans.ts at a3b870a6e.

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use crate::claude::fields::{rec, string};
use crate::port::PlanStep;
use wsp_wire::views::session_events::PlanStepState;

/// One task of the list TaskCreate and TaskUpdate keep.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanTask {
    pub id: Option<String>,
    pub text: String,
    pub state: PlanStepState,
}

/// The turn's plan as its calls so far built it. plans.ts:8.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanBook {
    /// The ids of the calls read as the plan, whose results draw nothing.
    pub calls: IndexSet<String>,
    pub tasks: Vec<PlanTask>,
    /// A TaskCreate's task index by its call id, until its result names the task's id.
    pub unnamed: IndexMap<String, usize>,
}

/// What one call is to the plan. plans.ts:24: `undefined` is NotPlan, `null` is Nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum PlanCall {
    NotPlan,
    Nothing,
    Steps(Vec<PlanStep>),
    Text(String),
}

/// plans.ts:24.
pub fn read_plan_call(name: Option<&str>, input: Option<&Value>, id: Option<&str>, book: &mut PlanBook) -> PlanCall {
    let fields = rec(input);
    match name {
        Some("TodoWrite") => {
            let Some(todos) = fields.and_then(|f| f.get("todos")).and_then(Value::as_array) else {
                return PlanCall::NotPlan;
            };
            let steps = todos
                .iter()
                .filter_map(|todo| rec(Some(todo)))
                .map(|todo| PlanStep { text: string(todo.get("content")).unwrap_or("").to_string(), state: step_state(todo.get("status")) })
                .collect();
            PlanCall::Steps(steps)
        }
        Some("ExitPlanMode") => match string(fields.and_then(|f| f.get("plan"))) {
            Some(plan) => PlanCall::Text(plan.to_string()),
            None => PlanCall::NotPlan,
        },
        Some("TaskCreate") => {
            if let Some(id) = id {
                book.unnamed.insert(id.to_string(), book.tasks.len());
            }
            let text = string(fields.and_then(|f| f.get("subject"))).unwrap_or("").to_string();
            book.tasks.push(PlanTask { id: None, text, state: PlanStepState::Pending });
            PlanCall::Steps(listed(book))
        }
        Some("TaskUpdate") => {
            let task_id = string(fields.and_then(|f| f.get("taskId")));
            let Some(at) = book.tasks.iter().position(|t| t.id.is_some() && t.id.as_deref() == task_id) else {
                return PlanCall::Nothing;
            };
            let status = fields.and_then(|f| f.get("status"));
            if status.and_then(Value::as_str) == Some("deleted") {
                book.tasks.remove(at);
            } else {
                let task = &mut book.tasks[at];
                if status.is_some() {
                    task.state = step_state(status);
                }
                if let Some(subject) = string(fields.and_then(|f| f.get("subject"))) {
                    task.text = subject.to_string();
                }
            }
            PlanCall::Steps(listed(book))
        }
        Some("TaskList") | Some("TaskGet") => PlanCall::Nothing,
        _ => PlanCall::NotPlan,
    }
}

fn listed(book: &PlanBook) -> Vec<PlanStep> {
    book.tasks.iter().map(|t| PlanStep { text: t.text.clone(), state: t.state }).collect()
}

fn step_state(status: Option<&Value>) -> PlanStepState {
    match status.and_then(Value::as_str) {
        Some("completed") => PlanStepState::Done,
        Some("in_progress") => PlanStepState::Working,
        _ => PlanStepState::Pending,
    }
}
