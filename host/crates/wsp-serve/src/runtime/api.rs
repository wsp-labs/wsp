// SPDX-License-Identifier: AGPL-3.0-only
//! The objects `Runtime` hands out for its areas (runtime.ts:1017-1103): wsp-core's API traits are foreign to the
//! crates that hold the areas (the orphan rule), so each is a newtype over the context that forwards to its areas.
//! `agents` holds the agents, preferences, conversations and image; `golden` the golden, usage, status and harnesses.
use std::rc::Rc;

use super::ctx::Ctx;

mod agents;
mod golden;

pub(crate) struct AgentsApiOf(pub(crate) Rc<Ctx>);
pub(crate) struct PreferencesApiOf(pub(crate) Rc<Ctx>);
pub(crate) struct ConversationsApiOf(pub(crate) Rc<Ctx>);
pub(crate) struct ImageApiOf(pub(crate) Rc<Ctx>);
pub(crate) struct GoldenApiOf(pub(crate) Rc<Ctx>);
pub(crate) struct UsageDoorOf(pub(crate) Rc<Ctx>);
pub(crate) struct OriginStatusOf(pub(crate) Rc<Ctx>);
pub(crate) struct HarnessesApiOf(pub(crate) Rc<Ctx>);
