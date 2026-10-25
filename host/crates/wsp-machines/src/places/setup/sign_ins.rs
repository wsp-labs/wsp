// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/setup.ts:374-631` at 65f6b89c7: the sign-ins step and the GitHub step of a run.
//! Spec: `spec/places_setup_sign_ins.md`.

use std::rc::Rc;

use wsp_engine::provision::ProvisionPlan;
use wsp_wire::types::agents_report::AgentsSignInEvent;
use wsp_words::views::place_setup_words::SignInWay;
use wsp_wire::views::place::{PlaceProvisionRow, PlaceSetupStep};
use wsp_words::exit::HostError;

use super::super::types::SignInHandle;
use super::run::SetupRun;

/// How a wait starts the sign-in it follows, every step it reaches going to the emit (setup.ts:80).
pub type SignInStart = Rc<dyn Fn(Rc<dyn Fn(AgentsSignInEvent)>) -> futures::future::LocalBoxFuture<'static, Result<SignInHandle, HostError>>>;

/// How a wait lands: the road, the note a signed-in row carries, the fix a failed one carries (setup.ts:377).
#[derive(Clone, Default)]
pub struct SignInHow {
    pub start: Option<SignInStart>,
    pub note: Option<String>,
    pub fix: Option<String>,
}

impl SetupRun {
    /// A sign-in on that computer that waits on the person (setup.ts:377).
    pub fn sign_in_there(self: &Rc<Self>, ask: &str, row: &str, label: &str, row_step: PlaceSetupStep, settled: Option<Rc<dyn Fn(bool)>>, how: SignInHow) {
        let _ = (ask, row, label, row_step, settled, how);
        todo!()
    }

    /// Whether an agent is signed in on that computer (setup.ts:453).
    pub async fn agent_signed_in(self: &Rc<Self>, agent: &str, plan: &ProvisionPlan) -> bool {
        let _ = (agent, plan);
        todo!()
    }

    /// An agent's sign-in on that computer, started only where it is not signed in there already (setup.ts:469).
    pub async fn agent_sign_in(self: &Rc<Self>, agent: &str, plan: &ProvisionPlan) {
        let _ = (agent, plan);
        todo!()
    }

    /// What a row of an agent with several ways says to do, the way picked first (setup.ts:483).
    pub fn ways_fix(&self, agent: &str, label: &str, first: Option<SignInWay>) -> Option<String> {
        let _ = (agent, label, first);
        todo!()
    }

    /// A sign-in picked to take a token or a key from this host's vault (setup.ts:494). `None` where a wait started.
    pub fn vault_way(self: &Rc<Self>, agent: &str, label: &str, way: SignInWay) -> Option<PlaceProvisionRow> {
        let _ = (agent, label, way);
        todo!()
    }

    /// A sign-in a retry runs again for a fresh page (setup.ts:508).
    pub async fn again_sign_in(self: &Rc<Self>, agent: &str, plan: &ProvisionPlan) {
        let _ = (agent, plan);
        todo!()
    }

    /// Whether an agent did not install (setup.ts:517).
    pub fn not_installed(&self, agent: &str) -> bool {
        let _ = agent;
        todo!()
    }

    /// The sign-ins step (setup.ts:522).
    pub async fn sign_ins(self: &Rc<Self>, plan: &ProvisionPlan) -> Vec<PlaceProvisionRow> {
        let _ = plan;
        todo!()
    }

    /// An agent's login as this computer holds it, put where every thread there reads it (setup.ts:559).
    pub async fn login_copied(self: &Rc<Self>, agent: &str, label: &str, plan: &ProvisionPlan) -> Option<PlaceProvisionRow> {
        let _ = (agent, label, plan);
        todo!()
    }

    /// gh's own status there, over the vault's token on the run's input where one is given (setup.ts:588).
    pub async fn gh_status(self: &Rc<Self>, plan: &ProvisionPlan, token: Option<&str>) -> Result<wsp_engine::machine::ExecResult, HostError> {
        let _ = (plan, token);
        todo!()
    }

    /// gh's login on that computer through the sign-in relay (setup.ts:593).
    pub async fn github_sign_in(self: &Rc<Self>, plan: &ProvisionPlan) {
        let _ = plan;
        todo!()
    }

    /// The GitHub step (setup.ts:603).
    pub async fn github(self: &Rc<Self>, plan: &ProvisionPlan) -> Result<Vec<PlaceProvisionRow>, HostError> {
        let _ = plan;
        todo!()
    }
}
