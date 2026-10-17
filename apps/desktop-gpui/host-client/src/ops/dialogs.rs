// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::protocol;

ops! {
    "places.add" => PlacesAddRequest => PlaceAdded,
    "places.door" => PlacesDoorRequest => Door,
    "places.mint" => PlacesMintRequest => protocol::JoinMint,
    "places.sshHosts" => PlacesSshHostsRequest => SshHosts,
    "places.setup" => PlacesSetupRequest => PlaceSetUp,
    "places.choose" => PlacesChooseRequest => Pending,
    "places.estimate" => PlacesEstimateRequest => Estimate,
    "pair.issue" => PairIssueRequest => PairCode,
    "pair.redeem" => PairRedeemRequest => protocol::PlaceJoinDevice,
    "init.answer" => InitAnswerRequest => Job,
    "init.build" => InitBuildRequest => Job,
    "init.cancel" => InitCancelRequest => Job,
    "init.draft" => InitDraftRequest => Job,
    "init.get" => InitGetRequest => Setup,
    "init.keys" => InitKeysRequest => Setup,
    "init.retry" => InitRetryRequest => Job,
    "init.signInCode" => InitSignInCodeRequest => Job,
    "init.start" => InitStartRequest => Job,
    "init.step" => InitStepRequest => Job,
}

/// An install once the computer dialled in: its pending add rides beside the place when nothing was picked.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAdded {
    pub add_id: String,
    pub place: protocol::PlaceView,
    pub host_key: Option<String>,
    pub said: Option<String>,
    pub pending: Option<protocol::PendingComputer>,
}

/// A setup under way, or the reason none started.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSetUp {
    pub add_id: String,
    pub place: protocol::PlaceView,
    pub setup: Option<protocol::PlaceSetup>,
    pub said: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Door {
    pub door: protocol::PlaceDoorView,
}

#[derive(Debug, Deserialize)]
pub struct SshHosts {
    pub hosts: Vec<protocol::SshHostSuggestion>,
}

#[derive(Debug, Deserialize)]
pub struct Pending {
    pub pending: protocol::PendingComputer,
}

#[derive(Debug, Deserialize)]
pub struct Estimate {
    pub estimate: protocol::PlaceEstimate,
}

/// A pairing code, with the fingerprint of the key this host proves where it proves one.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairCode {
    pub code: String,
    pub expires_at: f64,
    pub host_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Setup {
    pub setup: protocol::InitSetup,
}

#[derive(Debug, Deserialize)]
pub struct Job {
    pub job: protocol::InitJob,
}
