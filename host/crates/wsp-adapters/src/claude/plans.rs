// SPDX-License-Identifier: AGPL-3.0-only
//! The plan an agent keeps in its calls. TS: packages/adapter-claude/src/plans.ts at a3b870a6e.

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

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
    todo!()
}
