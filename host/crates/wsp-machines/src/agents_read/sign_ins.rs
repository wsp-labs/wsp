// SPDX-License-Identifier: AGPL-3.0-only
//! agents-read.ts:312-350, 426-439, 486-605 at 65f6b89c7: one watched sign-in per agent or server on a target. Spec:
//! `spec/agents_read_sign_ins.md`.
use std::rc::Rc;

use wsp_wire::types::agents_report::{AgentsSignInRun, AgentsTarget, SignInLine};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::{AgentsReads, Follower, Following, Running, SignInAsk};

impl AgentsReads {
    /// agents-read.ts:339 (N1): the JSON of `[target, agent, server or null, terminal, login]`.
    pub fn key_of(target: &AgentsTarget, ask: &SignInAsk) -> String {
        todo!()
    }

    /// agents-read.ts:340-350 (N3).
    pub fn follow(&self, sign_in_id: &str, run: &Rc<Running>, emit: Follower) -> Following {
        todo!()
    }

    /// agents-read.ts:325-333: a sign-in as listed.
    pub fn listed(sign_in_id: &str, run: &Running) -> AgentsSignInRun {
        todo!()
    }

    /// agents-read.ts:334-336 (N7, N9): drops ended sign-ins strictly older than `SIGN_IN_ENDED_KEPT_MS`.
    pub fn prune_ended(&self) {
        todo!()
    }

    /// agents-read.ts:429-439 (N5): the failed install's words, or none.
    pub async fn gh_first(&self, target: &AgentsTarget, ask: &SignInAsk, installing: Rc<dyn Fn()>) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// agents-read.ts:486-577 (N1, N2, N4, N6, N7, N8).
    pub async fn sign_in(self: &Rc<Self>, target: AgentsTarget, ask: SignInAsk, emit: Follower, origin: Option<Caller>) -> Result<Following, HostError> {
        todo!()
    }

    /// agents-read.ts:578-581 (N9).
    pub fn sign_ins(&self) -> Vec<AgentsSignInRun> {
        todo!()
    }

    /// agents-read.ts:582-586 (N10).
    pub fn sign_in_follow(&self, sign_in_id: &str, emit: Follower) -> Result<Following, HostError> {
        todo!()
    }

    /// agents-read.ts:587-591 (N10).
    pub async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError> {
        todo!()
    }

    /// agents-read.ts:592-596 (N8, N10).
    pub fn sign_in_stop(&self, sign_in_id: &str) -> Result<(), HostError> {
        todo!()
    }

    /// agents-read.ts:597-605 (N11).
    pub async fn sign_in_line(&self, target: &AgentsTarget, ask: &SignInAsk, origin: Option<&Caller>) -> Result<SignInLine, HostError> {
        todo!()
    }
}
